//! Question analysis persistence helper.
//!
//! Note: Primary summarization for both standard and co-pilot meetings runs through
//! the unified summarizer (`crate::summary::processor::generate_meeting_summary`).
//! This module provides question-level analysis with the configured model and
//! never falls back to rule-based extraction.

use crate::copilot::app_llm::AppLlm;
use crate::copilot::embedding::AppEmbedder;
use crate::database::repositories::copilot::CopilotRepository;
use copilot_core::json::to_json_array;
use copilot_core::{
    analyze_tagged_meeting, LlmClient, Question, TaggedQuestion, TranscriptTurn, UserTag,
};
use serde::Serialize;
use sqlx::SqlitePool;
use std::path::PathBuf;

pub struct AnalysisConfig {
    pub embed_model_dir: Option<PathBuf>,
    pub app_data_dir: Option<PathBuf>,
    pub alpha: f32,
    pub top_k: usize,
}

impl Default for AnalysisConfig {
    fn default() -> Self {
        Self {
            embed_model_dir: None,
            app_data_dir: None,
            alpha: 0.5,
            top_k: 5,
        }
    }
}

/// What the analysis did, so the UI can say which engine produced the answers.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisReport {
    pub questions: usize,
    /// "rules" or "<provider>/<model>"
    pub engine: String,
    /// Set when the model could not be used and the rules fallback ran.
    pub note: Option<String>,
    /// Questions that actually consumed a model call (Not asked ones usually don't).
    pub model_calls: usize,
}

async fn set_stage(pool: &SqlitePool, meeting_id: &str, stage: &str) -> Result<(), String> {
    sqlx::query("UPDATE meetings SET processing_stage = ? WHERE id = ?")
        .bind(stage)
        .bind(meeting_id)
        .execute(pool)
        .await
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Run post-call analysis for a meeting and persist the results.
pub async fn run_analysis(
    pool: &SqlitePool,
    meeting_id: &str,
    config: AnalysisConfig,
) -> Result<AnalysisReport, String> {
    let result = run_analysis_inner(pool, meeting_id, config).await;
    if result.is_err() {
        let _ = set_stage(pool, meeting_id, "failed").await;
    }
    result
}

async fn run_analysis_inner(
    pool: &SqlitePool,
    meeting_id: &str,
    config: AnalysisConfig,
) -> Result<AnalysisReport, String> {
    let llm = AppLlm::resolve(pool, config.app_data_dir.clone())
        .await
        .map_err(|why| format!("Summary model unavailable: {why}"))?;
    let engine = llm.label();
    run_analysis_with_client(pool, meeting_id, config, Box::new(llm), engine).await
}

pub(crate) async fn run_analysis_with_client(
    pool: &SqlitePool,
    meeting_id: &str,
    config: AnalysisConfig,
    client: Box<dyn LlmClient>,
    engine: String,
) -> Result<AnalysisReport, String> {
    let questions = CopilotRepository::list_questions(pool, meeting_id)
        .await
        .map_err(|e| e.to_string())?;
    let segments = CopilotRepository::list_segments(pool, meeting_id)
        .await
        .map_err(|e| e.to_string())?;

    if questions.is_empty() {
        set_stage(pool, meeting_id, "complete").await?;
        return Ok(AnalysisReport { questions: 0, engine, note: None, model_calls: 0 });
    }
    if segments.is_empty() {
        return Err("no transcript segments to analyse".to_string());
    }

    // From here on the tags are locked (see `CopilotRepository::tags_locked`).
    set_stage(pool, meeting_id, "indexing").await?;

    let tagged: Vec<TaggedQuestion> = questions
        .iter()
        .map(|q| TaggedQuestion {
            question: Question {
                id: q.id.clone(),
                sort_order: q.sort_order,
                main_question: q.main_question.clone(),
                sample_questions: q.sample_questions(),
                sample_answers: q.sample_answers(),
            },
            tag: q.user_tag.as_deref().and_then(UserTag::from_str),
        })
        .collect();
    let core_turns: Vec<TranscriptTurn> = segments
        .iter()
        .map(|s| TranscriptTurn {
            seq: s.seq,
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            channel: s.channel,
            speaker_label: s.speaker_label.clone(),
            text: s.text.clone(),
        })
        .collect();

    set_stage(pool, meeting_id, "evaluating_questions").await?;
    let embedder = AppEmbedder::load(config.embed_model_dir.as_deref());
    let (alpha, top_k) = (config.alpha, config.top_k);

    let verdicts = tokio::task::spawn_blocking(move || {
        analyze_tagged_meeting(&tagged, &core_turns, &embedder, alpha, top_k, client.as_ref())
    })
    .await
    .map_err(|e| format!("analysis task panicked: {e}"))?
    .map_err(|e| e.to_string())?;

    set_stage(pool, meeting_id, "extracting_notes").await?;

    let model_calls = verdicts.iter().filter(|v| v.model_called).count();
    let mut tx = pool.begin().await.map_err(|e| e.to_string())?;
    for qv in &verdicts {
        let rating = qv.verdict.rating.map(|r| r.as_str());
        sqlx::query(
            "UPDATE meeting_questions
             SET status = ?, rating = ?, extracted_answer = ?, missing_details = ?,
                 confidence = ?, evidence_segment_ids = ?, tag_note = ?
             WHERE id = ? AND manually_overridden = 0",
        )
        .bind(qv.verdict.status.as_str())
        .bind(rating)
        .bind(&qv.verdict.extracted_answer)
        .bind(&qv.verdict.missing_details)
        .bind(qv.verdict.confidence as f64)
        .bind(to_json_array(&qv.verdict.evidence_segment_ids))
        .bind(&qv.note)
        .bind(&qv.question_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    }
    sqlx::query("UPDATE meetings SET analysis_model = ?, processing_stage = 'complete' WHERE id = ?")
        .bind(&engine)
        .bind(meeting_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| e.to_string())?;
    tx.commit().await.map_err(|e| e.to_string())?;

    Ok(AnalysisReport { questions: verdicts.len(), engine, note: None, model_calls })
}

/// Analysis progress for the post-meeting screen.
#[derive(Debug, Clone, Serialize)]
pub struct AnalysisStatus {
    /// None (never run) | indexing | evaluating_questions | extracting_notes | complete | failed
    pub stage: Option<String>,
    pub engine: Option<String>,
}

pub async fn analysis_status(pool: &SqlitePool, meeting_id: &str) -> Result<AnalysisStatus, String> {
    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT processing_stage, analysis_model FROM meetings WHERE id = ?")
            .bind(meeting_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?;
    let (stage, engine) = row.unwrap_or((None, None));
    Ok(AnalysisStatus { stage, engine })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;

    async fn pool() -> (SqlitePool, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sqlite");
        let opts = SqliteConnectOptions::from_str(path.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        (pool, dir)
    }

    /// A recorded co-pilot meeting with three questions and a short transcript.
    /// Returns the draft question ids in order: salary, notice, team.
    async fn recorded_meeting(pool: &SqlitePool) -> Vec<String> {
        let mut bank = Vec::new();
        for (q, sq, sa) in [
            ("What is the base salary?", vec!["How much is the compensation?".to_string()], vec!["The base is 50k".to_string()]),
            ("What is the notice period?", vec![], vec![]),
            ("How big is the engineering team?", vec![], vec![]),
        ] {
            bank.push(CopilotRepository::bank_add(pool, q, &sq, &sa, None).await.unwrap());
        }
        let ids = CopilotRepository::begin_draft(pool, "draft-x", &bank).await.unwrap();

        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('rec','Call','2026-01-01','2026-01-01')")
            .execute(pool).await.unwrap();
        for (i, text) in [
            "Thanks for joining the call today.",
            "What is the base salary for this role?",
            "The base salary is 50k with a signing bonus.",
            "The engineering team is twelve people across two squads.",
        ]
        .iter()
        .enumerate()
        {
            sqlx::query("INSERT INTO transcripts (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time, duration) VALUES (?, 'rec', ?, '2026-01-01', ?, ?, 4.0)")
                .bind(format!("tr{i}")).bind(text).bind(i as f64 * 4.0).bind(i as f64 * 4.0 + 4.0)
                .execute(pool).await.unwrap();
        }
        ids
    }

    /// Missing model fails closed without falling back to rule-based extraction.
    #[tokio::test]
    async fn missing_model_fails_closed_without_rules_fallback() {
        let (pool, _dir) = pool().await;
        let _ids = recorded_meeting(&pool).await;
        CopilotRepository::finalize_recording(&pool, "draft-x", "rec").await.unwrap();

        let err = run_analysis(&pool, "rec", AnalysisConfig::default()).await.unwrap_err();
        assert!(err.contains("Summary model unavailable"), "unexpected error: {err}");
        let st = analysis_status(&pool, "rec").await.unwrap();
        assert_eq!(st.stage.as_deref(), Some("failed"));
    }

    /// The whole co-pilot data path, minus audio capture: question set -> draft
    /// session -> manual tags during the call -> recording attached -> Generate
    /// Summary -> tag-aware verdicts + notes persisted, tags locked.
    #[tokio::test]
    async fn tags_steer_the_post_call_question_summary() {
        let (pool, _dir) = pool().await;
        let ids = recorded_meeting(&pool).await;
        // During the call: salary tagged Answered, notice tagged Not asked,
        // team left untagged (but discussed).
        CopilotRepository::set_user_tag(&pool, &ids[0], Some("answered")).await.unwrap();
        CopilotRepository::set_user_tag(&pool, &ids[1], Some("not_asked")).await.unwrap();
        CopilotRepository::finalize_recording(&pool, "draft-x", "rec").await.unwrap();

        let report = run_analysis_with_client(
            &pool,
            "rec",
            AnalysisConfig::default(),
            Box::new(copilot_core::HeuristicLlm),
            "test-llm".into(),
        )
        .await
        .unwrap();
        assert_eq!(report.questions, 3);
        assert_eq!(report.engine, "test-llm");
        assert!(report.note.is_none());
        let st = analysis_status(&pool, "rec").await.unwrap();
        assert_eq!(st.stage.as_deref(), Some("complete"));

        let qs = CopilotRepository::list_questions(&pool, "rec").await.unwrap();
        let segs = CopilotRepository::list_segments(&pool, "rec").await.unwrap();
        let known: Vec<String> = segs.iter().map(|s| format!("t{}", s.seq)).collect();
        for q in &qs {
            // Coverage/quality invariant: rating only when answered.
            if q.status != "answered" {
                assert!(q.rating.is_none());
            }
            // Every cited id references a real segment; non-not_asked verdicts cite one.
            for id in &q.evidence_segment_ids() {
                assert!(known.contains(id), "evidence {id} must reference a real segment");
            }
            if q.status != "not_asked" {
                assert!(!q.evidence_segment_ids().is_empty());
            }
        }

        // Tagged Answered: searched, answered, written up, no note.
        assert_eq!(qs[0].user_tag.as_deref(), Some("answered"));
        assert_eq!(qs[0].status, "answered");
        assert!(qs[0].extracted_answer.as_deref().unwrap().contains("50k"));
        assert_eq!(qs[0].tag_note, None);
        // Tagged Not asked: skipped, explicit text, nothing invented.
        assert_eq!(qs[1].status, "not_asked");
        assert_eq!(qs[1].extracted_answer.as_deref(), Some("Not discussed in this call."));
        assert!(qs[1].evidence_segment_ids().is_empty());
        // Untagged: best-effort search found it, and it is flagged.
        assert_ne!(qs[2].status, "not_asked");
        assert_eq!(qs[2].tag_note.as_deref(), Some("Not explicitly tagged."));

        // Tags are locked once the summary exists.
        assert!(CopilotRepository::set_user_tag(&pool, &ids[2], Some("asked")).await.is_err());
    }

    #[tokio::test]
    async fn a_tag_contradicted_by_the_transcript_is_reported_not_overridden() {
        let (pool, _dir) = pool().await;
        let ids = recorded_meeting(&pool).await;
        // The salary question was clearly discussed, but the interviewer tapped Not asked.
        CopilotRepository::set_user_tag(&pool, &ids[0], Some("not_asked")).await.unwrap();
        // And tapped Asked on a question nobody raised.
        CopilotRepository::set_user_tag(&pool, &ids[1], Some("asked")).await.unwrap();
        CopilotRepository::finalize_recording(&pool, "draft-x", "rec").await.unwrap();
        run_analysis_with_client(
            &pool,
            "rec",
            AnalysisConfig::default(),
            Box::new(copilot_core::HeuristicLlm),
            "test-llm".into(),
        )
        .await
        .unwrap();

        let qs = CopilotRepository::list_questions(&pool, "rec").await.unwrap();
        assert_eq!(qs[0].user_tag.as_deref(), Some("not_asked"), "the tag itself is kept");
        assert_ne!(qs[0].status, "not_asked", "the answer is surfaced");
        assert!(qs[0].tag_note.as_deref().unwrap().starts_with("Tagged Not asked"));
        assert_eq!(qs[1].status, "not_asked");
        assert_eq!(qs[1].extracted_answer, None, "no invented answer");
        assert!(qs[1].tag_note.as_deref().unwrap().starts_with("Tagged Asked"));
    }
}

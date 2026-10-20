//! Persistence for Intake Co-Pilot question sets, meeting questions and transcript segments.
//!
//! Status and rating are orthogonal; every write path that touches ratings
//! enforces that `rating` is NULL unless `status = 'answered'`.

use crate::database::models_copilot::{
    to_json_array, BankQuestionModel, MeetingQuestionModel, QuestionBankGroupModel,
    TranscriptSegmentModel,
};
use serde::{Deserialize, Serialize};
use sqlx::{Error as SqlxError, SqlitePool};

/// A question as submitted by the ingest UI or importer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewQuestion {
    pub main_question: String,
    #[serde(default)]
    pub sample_questions: Vec<String>,
    #[serde(default)]
    pub sample_answers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IngestError {
    TooManyRows(usize),
    EmptyQuestion(usize),
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IngestError::TooManyRows(n) => write!(f, "too many questions: {n} (max 500)"),
            IngestError::EmptyQuestion(i) => write!(f, "question at row {i} is empty"),
        }
    }
}

pub const MAX_QUESTIONS: usize = 500;

/// Validate a batch of imported questions before persisting any of them.
pub fn validate_ingest(questions: &[NewQuestion]) -> Result<(), IngestError> {
    if questions.len() > MAX_QUESTIONS {
        return Err(IngestError::TooManyRows(questions.len()));
    }
    for (i, q) in questions.iter().enumerate() {
        if q.main_question.trim().is_empty() {
            return Err(IngestError::EmptyQuestion(i + 1));
        }
    }
    Ok(())
}

pub struct CopilotRepository;

impl CopilotRepository {
    pub async fn list_questions(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Vec<MeetingQuestionModel>, SqlxError> {
        sqlx::query_as::<_, MeetingQuestionModel>(
            "SELECT id, meeting_id, sort_order, main_question, sample_questions_json,
                    sample_answers_json, status, rating, extracted_answer, missing_details,
                    confidence, evidence_segment_ids, asked_timestamp_ms, user_ticked,
                    user_notes, manually_overridden, user_tag, tag_note
             FROM meeting_questions WHERE meeting_id = ? ORDER BY sort_order ASC",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
    }

    /// The interviewer's manual tag for a question (`None` clears it). User input,
    /// saved with the session. Editable until the question summary is generated:
    /// once analysis has started or finished for the meeting, tags are locked
    /// (verdicts stay correctable through `override_verdict`).
    pub async fn set_user_tag(
        pool: &SqlitePool,
        question_id: &str,
        tag: Option<&str>,
    ) -> Result<(), SqlxError> {
        if let Some(t) = tag {
            if copilot_core::UserTag::from_str(t).is_none() {
                return Err(SqlxError::Protocol(format!("invalid tag: {t}")));
            }
        }
        let stage: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT m.processing_stage FROM meeting_questions q
             JOIN meetings m ON m.id = q.meeting_id WHERE q.id = ?",
        )
        .bind(question_id)
        .fetch_optional(pool)
        .await?;
        let Some((stage,)) = stage else {
            return Err(SqlxError::Protocol("question not found".into()));
        };
        if Self::tags_locked(stage.as_deref()) {
            return Err(SqlxError::Protocol(
                "tags are locked once the summary has been generated".into(),
            ));
        }
        sqlx::query("UPDATE meeting_questions SET user_tag = ? WHERE id = ?")
            .bind(tag)
            .bind(question_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Tags are editable before the first analysis and after a failed one.
    pub fn tags_locked(processing_stage: Option<&str>) -> bool {
        !matches!(processing_stage, None | Some("failed"))
    }

    pub async fn set_notes(
        pool: &SqlitePool,
        question_id: &str,
        notes: &str,
    ) -> Result<(), SqlxError> {
        sqlx::query("UPDATE meeting_questions SET user_notes = ? WHERE id = ?")
            .bind(notes)
            .bind(question_id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Manual override of an automated verdict. Marks the row as overridden.
    ///
    /// Returns an error if the status/rating combination is invalid (rating set
    /// without an answer).
    pub async fn override_verdict(
        pool: &SqlitePool,
        question_id: &str,
        status: &str,
        rating: Option<&str>,
        extracted_answer: Option<&str>,
    ) -> Result<(), SqlxError> {
        const VALID_STATUS: [&str; 3] = ["not_asked", "asked_not_answered", "answered"];
        const VALID_RATING: [&str; 3] = ["Poor", "Partial", "Strong"];
        if !VALID_STATUS.contains(&status) {
            return Err(SqlxError::Protocol(format!("invalid status: {status}")));
        }
        if let Some(r) = rating {
            if !VALID_RATING.contains(&r) {
                return Err(SqlxError::Protocol(format!("invalid rating: {r}")));
            }
            if status != "answered" {
                return Err(SqlxError::Protocol(
                    "rating must be NULL unless status == answered".to_string(),
                ));
            }
        }
        sqlx::query(
            "UPDATE meeting_questions
             SET status = ?, rating = ?, extracted_answer = COALESCE(?, extracted_answer),
                 manually_overridden = 1
             WHERE id = ?",
        )
        .bind(status)
        .bind(rating)
        .bind(extracted_answer)
        .bind(question_id)
        .execute(pool)
        .await?;
        Ok(())
    }

    pub async fn list_segments(
        pool: &SqlitePool,
        meeting_id: &str,
    ) -> Result<Vec<TranscriptSegmentModel>, SqlxError> {
        sqlx::query_as::<_, TranscriptSegmentModel>(
            "SELECT id, meeting_id, seq, start_ms, end_ms, channel, speaker_label, text
             FROM transcript_segments WHERE meeting_id = ? ORDER BY seq ASC",
        )
        .bind(meeting_id)
        .fetch_all(pool)
        .await
    }

    /// Ensure a `meetings` row exists for a co-pilot session.
    ///
    /// Without this, inserting into `meeting_questions` or `transcript_segments`
    /// fails with FOREIGN KEY constraint failed (SQLite error 787), because those
    /// tables reference `meetings(id)`.
    pub async fn ensure_meeting(
        pool: &SqlitePool,
        meeting_id: &str,
        title: &str,
        mode: &str,
    ) -> Result<(), SqlxError> {
        if meeting_id.trim().is_empty() {
            return Err(SqlxError::Protocol("meeting_id cannot be empty".into()));
        }
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT OR IGNORE INTO meetings (id, title, created_at, updated_at, meeting_mode)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(meeting_id)
        .bind(title)
        .bind(&now)
        .bind(&now)
        .bind(mode)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Attach a finished recording to a co-pilot meeting.
    ///
    /// The recorder saves the call as its own `meetings` row (new id) with the
    /// transcript in `transcripts`. The co-pilot questions were created under a
    /// placeholder meeting id before recording started. This re-keys the questions
    /// onto the saved meeting, drops the placeholder, and copies the transcript into
    /// `transcript_segments` (the table the analysis reads). Atomic.
    pub async fn finalize_recording(
        pool: &SqlitePool,
        copilot_meeting_id: &str,
        saved_meeting_id: &str,
    ) -> Result<u64, SqlxError> {
        if copilot_meeting_id.trim().is_empty() || saved_meeting_id.trim().is_empty() {
            return Err(SqlxError::Protocol("meeting ids cannot be empty".into()));
        }
        let mut tx = pool.begin().await?;

        if copilot_meeting_id != saved_meeting_id {
            sqlx::query("UPDATE meetings SET meeting_mode = 'copilot' WHERE id = ?")
            .bind(saved_meeting_id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE meeting_questions SET meeting_id = ? WHERE meeting_id = ?")
                .bind(saved_meeting_id)
                .bind(copilot_meeting_id)
                .execute(&mut *tx)
                .await?;
            // Questions are re-keyed first, so the cascade removes nothing of value.
            sqlx::query("DELETE FROM meetings WHERE id = ?")
                .bind(copilot_meeting_id)
                .execute(&mut *tx)
                .await?;
        }

        sqlx::query("DELETE FROM transcript_segments WHERE meeting_id = ?")
            .bind(saved_meeting_id)
            .execute(&mut *tx)
            .await?;
        let copied = sqlx::query(
            "INSERT INTO transcript_segments
               (id, meeting_id, seq, start_ms, end_ms, channel, speaker_label, text)
             SELECT 'seg-' || id, meeting_id,
                    ROW_NUMBER() OVER (ORDER BY COALESCE(audio_start_time, 0), timestamp, id) - 1,
                    CAST(ROUND(COALESCE(audio_start_time, 0) * 1000) AS INTEGER),
                    CAST(ROUND(COALESCE(audio_end_time, audio_start_time, 0) * 1000) AS INTEGER),
                    0, speaker, transcript
             FROM transcripts WHERE meeting_id = ?",
        )
        .bind(saved_meeting_id)
        .execute(&mut *tx)
        .await?
        .rows_affected();

        tx.commit().await?;
        Ok(copied)
    }

    // ---- Reusable question bank ----

    pub async fn bank_list(pool: &SqlitePool) -> Result<Vec<BankQuestionModel>, SqlxError> {
        sqlx::query_as::<_, BankQuestionModel>(
            "SELECT id, bank_id, question, sample_questions_json, sample_answers_json, created_at, updated_at
             FROM question_bank ORDER BY created_at ASC, rowid ASC",
        )
        .fetch_all(pool)
        .await
    }

    /// Start (or restart) a co-pilot session: drop abandoned drafts from earlier
    /// sessions, create this draft, and fill it with exactly the set's questions
    /// (a retry after a failed start never duplicates them).
    pub async fn begin_draft(
        pool: &SqlitePool,
        draft_id: &str,
        set_question_ids: &[String],
    ) -> Result<Vec<String>, SqlxError> {
        if !draft_id.starts_with("draft-") {
            return Err(SqlxError::Protocol("draft id must start with 'draft-'".into()));
        }
        sqlx::query("DELETE FROM meetings WHERE id LIKE 'draft-%' AND id != ?")
            .bind(draft_id)
            .execute(pool)
            .await?;
        Self::ensure_meeting(pool, draft_id, "Intake co-pilot meeting", "copilot").await?;
        sqlx::query("DELETE FROM meeting_questions WHERE meeting_id = ?")
            .bind(draft_id)
            .execute(pool)
            .await?;
        Self::add_bank_questions(pool, draft_id, set_question_ids).await
    }

    // ---- Named question banks (headers) ----

    pub const DEFAULT_BANK_ID: &'static str = "bank-default";
    pub const DEFAULT_BANK_NAME: &'static str = "General";

    pub async fn group_list(pool: &SqlitePool) -> Result<Vec<QuestionBankGroupModel>, SqlxError> {
        sqlx::query_as::<_, QuestionBankGroupModel>(
            "SELECT id, name, created_at, updated_at FROM question_bank_groups
             ORDER BY created_at ASC, rowid ASC",
        )
        .fetch_all(pool)
        .await
    }

    fn clean_bank_name(name: &str) -> Result<String, SqlxError> {
        let n = name.trim();
        if n.is_empty() {
            return Err(SqlxError::Protocol("question bank name cannot be empty".into()));
        }
        if n.chars().count() > 120 {
            return Err(SqlxError::Protocol("question bank name is too long (max 120)".into()));
        }
        Ok(n.to_string())
    }

    fn map_name_conflict(e: SqlxError, name: &str) -> SqlxError {
        match &e {
            SqlxError::Database(db) if db.is_unique_violation() => SqlxError::Protocol(format!(
                "a question bank named '{name}' already exists"
            )),
            _ => e,
        }
    }

    pub async fn group_create(pool: &SqlitePool, name: &str) -> Result<String, SqlxError> {
        let name = Self::clean_bank_name(name)?;
        let id = format!("bank-{}", uuid::Uuid::new_v4());
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO question_bank_groups (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&name)
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await
        .map_err(|e| Self::map_name_conflict(e, &name))?;
        Ok(id)
    }

    pub async fn group_rename(pool: &SqlitePool, id: &str, name: &str) -> Result<(), SqlxError> {
        let name = Self::clean_bank_name(name)?;
        let res = sqlx::query("UPDATE question_bank_groups SET name = ?, updated_at = ? WHERE id = ?")
            .bind(&name)
            .bind(chrono::Utc::now().to_rfc3339())
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| Self::map_name_conflict(e, &name))?;
        if res.rows_affected() == 0 {
            return Err(SqlxError::Protocol("question bank not found".into()));
        }
        Ok(())
    }

    /// Delete a bank and the questions saved in it. Questions already copied into
    /// past meetings are independent rows and are not affected.
    pub async fn group_delete(pool: &SqlitePool, id: &str) -> Result<(), SqlxError> {
        let mut tx = pool.begin().await?;
        sqlx::query("DELETE FROM question_bank WHERE bank_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM question_bank_groups WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Create the named question set, or replace its questions if it exists
    /// (case-insensitive match). One transaction: a bad import changes nothing.
    /// Returns `(set id, created)`.
    pub async fn group_import(
        pool: &SqlitePool,
        name: &str,
        questions: &[NewQuestion],
    ) -> Result<(String, bool), SqlxError> {
        let name = Self::clean_bank_name(name)?;
        if questions.is_empty() {
            return Err(SqlxError::Protocol("no questions to import".into()));
        }
        validate_ingest(questions).map_err(|e| SqlxError::Protocol(e.to_string()))?;
        let mut tx = pool.begin().await?;
        let now = chrono::Utc::now().to_rfc3339();
        let existing: Option<(String,)> =
            sqlx::query_as("SELECT id FROM question_bank_groups WHERE lower(name) = lower(?)")
                .bind(&name)
                .fetch_optional(&mut *tx)
                .await?;
        let (id, created) = match existing {
            Some((id,)) => {
                sqlx::query("DELETE FROM question_bank WHERE bank_id = ?")
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE question_bank_groups SET updated_at = ? WHERE id = ?")
                    .bind(&now)
                    .bind(&id)
                    .execute(&mut *tx)
                    .await?;
                (id, false)
            }
            None => {
                let id = format!("bank-{}", uuid::Uuid::new_v4());
                sqlx::query("INSERT INTO question_bank_groups (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)")
                    .bind(&id)
                    .bind(&name)
                    .bind(&now)
                    .bind(&now)
                    .execute(&mut *tx)
                    .await?;
                (id, true)
            }
        };
        for qn in questions {
            // Insert order = CSV row order (lists order by created_at, then rowid).
            sqlx::query(
                "INSERT INTO question_bank
                 (id, bank_id, question, sample_questions_json, sample_answers_json, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(format!("bank-{}", uuid::Uuid::new_v4()))
            .bind(&id)
            .bind(qn.main_question.trim())
            .bind(to_json_array(&qn.sample_questions))
            .bind(to_json_array(&qn.sample_answers))
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok((id, created))
    }

    /// Resolve the bank a new question goes to: the given one (must exist) or the
    /// default "General" bank, created on demand.
    async fn resolve_bank(pool: &SqlitePool, bank_id: Option<&str>) -> Result<String, SqlxError> {
        match bank_id {
            Some(id) => {
                let found: Option<(String,)> =
                    sqlx::query_as("SELECT id FROM question_bank_groups WHERE id = ?")
                        .bind(id)
                        .fetch_optional(pool)
                        .await?;
                found
                    .map(|r| r.0)
                    .ok_or_else(|| SqlxError::Protocol("question bank not found".into()))
            }
            None => {
                let now = chrono::Utc::now().to_rfc3339();
                sqlx::query(
                    "INSERT OR IGNORE INTO question_bank_groups (id, name, created_at, updated_at)
                     VALUES (?, ?, ?, ?)",
                )
                .bind(Self::DEFAULT_BANK_ID)
                .bind(Self::DEFAULT_BANK_NAME)
                .bind(&now)
                .bind(&now)
                .execute(pool)
                .await?;
                Ok(Self::DEFAULT_BANK_ID.to_string())
            }
        }
    }

    pub async fn bank_add(
        pool: &SqlitePool,
        main_question: &str,
        sample_questions: &[String],
        sample_answers: &[String],
        bank_id: Option<&str>,
    ) -> Result<String, SqlxError> {
        if main_question.trim().is_empty() {
            return Err(SqlxError::Protocol("question cannot be empty".into()));
        }
        let bank = Self::resolve_bank(pool, bank_id).await?;
        let id = format!("bank-{}", uuid::Uuid::new_v4());
        let now = chrono::Utc::now().to_rfc3339();
        sqlx::query(
            "INSERT INTO question_bank
             (id, bank_id, question, sample_questions_json, sample_answers_json, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&id)
        .bind(&bank)
        .bind(main_question.trim())
        .bind(to_json_array(sample_questions))
        .bind(to_json_array(sample_answers))
        .bind(&now)
        .bind(&now)
        .execute(pool)
        .await?;
        Ok(id)
    }

    pub async fn bank_add_many(
        pool: &SqlitePool,
        questions: &[NewQuestion],
        bank_id: Option<&str>,
    ) -> Result<Vec<String>, SqlxError> {
        validate_ingest(questions).map_err(|e| SqlxError::Protocol(e.to_string()))?;
        let bank = Self::resolve_bank(pool, bank_id).await?;
        let mut tx = pool.begin().await?;
        let now = chrono::Utc::now().to_rfc3339();
        let mut ids = Vec::with_capacity(questions.len());
        for qn in questions {
            let id = format!("bank-{}", uuid::Uuid::new_v4());
            sqlx::query(
                "INSERT INTO question_bank
                 (id, bank_id, question, sample_questions_json, sample_answers_json, created_at, updated_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&id)
            .bind(&bank)
            .bind(qn.main_question.trim())
            .bind(to_json_array(&qn.sample_questions))
            .bind(to_json_array(&qn.sample_answers))
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
            ids.push(id);
        }
        tx.commit().await?;
        Ok(ids)
    }

    pub async fn bank_delete(pool: &SqlitePool, id: &str) -> Result<(), SqlxError> {
        sqlx::query("DELETE FROM question_bank WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await?;
        Ok(())
    }

    /// Append the selected bank questions to a meeting's question set.
    pub async fn add_bank_questions(
        pool: &SqlitePool,
        meeting_id: &str,
        bank_ids: &[String],
    ) -> Result<Vec<String>, SqlxError> {
        Self::ensure_meeting(pool, meeting_id, "Intake co-pilot meeting", "copilot").await?;
        let existing: (i64,) = sqlx::query_as(
            "SELECT COALESCE(MAX(sort_order), -1) FROM meeting_questions WHERE meeting_id = ?",
        )
        .bind(meeting_id)
        .fetch_one(pool)
        .await?;

        let mut tx = pool.begin().await?;
        let mut order = existing.0 + 1;
        let mut ids = Vec::new();
        for bid in bank_ids {
            let bank = sqlx::query_as::<_, BankQuestionModel>(
                "SELECT id, bank_id, question, sample_questions_json, sample_answers_json, created_at, updated_at
                 FROM question_bank WHERE id = ?",
            )
            .bind(bid)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(b) = bank {
                let id = format!("question-{}", uuid::Uuid::new_v4());
                sqlx::query(
                    "INSERT INTO meeting_questions
                     (id, meeting_id, sort_order, main_question, sample_questions_json, sample_answers_json)
                     VALUES (?, ?, ?, ?, ?, ?)",
                )
                .bind(&id)
                .bind(meeting_id)
                .bind(order)
                .bind(&b.question)
                .bind(&b.sample_questions_json)
                .bind(&b.sample_answers_json)
                .execute(&mut *tx)
                .await?;
                ids.push(id);
                order += 1;
            }
        }
        tx.commit().await?;
        Ok(ids)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn q(main: &str) -> NewQuestion {
        NewQuestion {
            main_question: main.to_string(),
            sample_questions: vec![],
            sample_answers: vec![],
        }
    }

    #[test]
    fn ingest_rejects_too_many_rows() {
        let questions: Vec<NewQuestion> = (0..501).map(|i| q(&format!("q{i}"))).collect();
        assert_eq!(
            validate_ingest(&questions),
            Err(IngestError::TooManyRows(501))
        );
    }

    #[test]
    fn ingest_rejects_empty_question_with_row_number() {
        let questions = vec![q("ok"), q("   ")];
        assert_eq!(
            validate_ingest(&questions),
            Err(IngestError::EmptyQuestion(2))
        );
    }

    #[test]
    fn ingest_accepts_valid_batch() {
        assert!(validate_ingest(&[q("a"), q("b")]).is_ok());
    }

    use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
    use std::str::FromStr;

    async fn test_pool() -> SqlitePool {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sqlite");
        let opts = SqliteConnectOptions::from_str(path.to_str().unwrap())
            .unwrap()
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal);
        let pool = SqlitePoolOptions::new().connect_with(opts).await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        std::mem::forget(dir); // keep the temp dir alive for the test process
        pool
    }

    #[tokio::test]
    async fn foreign_key_violation_happens_without_a_meeting_row() {
        let pool = test_pool().await;
        let bad = sqlx::query(
            "INSERT INTO transcript_segments (id, meeting_id, seq, start_ms, end_ms, channel, text)
             VALUES ('s1','m-1',0,0,1,0,'x')",
        )
        .execute(&pool)
        .await;
        assert!(bad.is_err(), "expected FK violation without a meeting row");
    }

    #[tokio::test]
    async fn ensure_meeting_makes_segment_inserts_succeed() {
        let pool = test_pool().await;
        let insert = |meeting: &'static str| {
            let pool = pool.clone();
            async move {
                sqlx::query(
                    "INSERT INTO transcript_segments (id, meeting_id, seq, start_ms, end_ms, channel, text)
                     VALUES ('s1', ?, 0, 0, 1, 0, 'hello')",
                )
                .bind(meeting)
                .execute(&pool)
                .await
            }
        };
        assert!(insert("m-2").await.is_err(), "FK violation without a meeting row");
        CopilotRepository::ensure_meeting(&pool, "m-2", "Intake co-pilot", "copilot").await.unwrap();
        assert!(insert("m-2").await.is_ok());
    }

    #[tokio::test]
    async fn question_bank_roundtrip() {
        let pool = test_pool().await;
        let id = CopilotRepository::bank_add(
            &pool,
            "What is the salary?",
            &["Compensation?".to_string()],
            &["50k base".to_string()],
            None,
        )
        .await
        .unwrap();
        let list = CopilotRepository::bank_list(&pool).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].sample_questions(), vec!["Compensation?"]);
        assert_eq!(list[0].sample_answers(), vec!["50k base"]);

        CopilotRepository::bank_delete(&pool, &id).await.unwrap();
        assert!(CopilotRepository::bank_list(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn add_bank_questions_copies_into_meeting() {
        let pool = test_pool().await;
        let b = CopilotRepository::bank_add(&pool, "Team size?", &[], &[], None)
            .await
            .unwrap();
        let ids = CopilotRepository::add_bank_questions(&pool, "m-3", &[b])
            .await
            .unwrap();
        assert_eq!(ids.len(), 1);
        let qs = CopilotRepository::list_questions(&pool, "m-3").await.unwrap();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0].main_question, "Team size?");
    }

    #[tokio::test]
    async fn finalize_recording_moves_questions_and_copies_transcript() {
        let pool = test_pool().await;
        let b = CopilotRepository::bank_add(&pool, "Team size?", &[], &[], None).await.unwrap();
        CopilotRepository::add_bank_questions(&pool, "placeholder", &[b]).await.unwrap();

        // What the recorder saves on stop: a separate meeting + transcripts rows.
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('saved','t','2026-01-01','2026-01-01')")
            .execute(&pool).await.unwrap();
        for (i, (txt, start)) in [("second", 5.0), ("first", 1.0)].iter().enumerate() {
            sqlx::query("INSERT INTO transcripts (id, meeting_id, transcript, timestamp, audio_start_time, audio_end_time, duration) VALUES (?, 'saved', ?, '2026-01-01', ?, ?, 1.0)")
                .bind(format!("tr{i}")).bind(txt).bind(start).bind(start + 1.0)
                .execute(&pool).await.unwrap();
        }

        let copied = CopilotRepository::finalize_recording(&pool, "placeholder", "saved").await.unwrap();
        assert_eq!(copied, 2);

        let qs = CopilotRepository::list_questions(&pool, "saved").await.unwrap();
        assert_eq!(qs.len(), 1, "questions follow the recorded meeting");
        assert!(CopilotRepository::list_questions(&pool, "placeholder").await.unwrap().is_empty());
        let gone: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM meetings WHERE id = 'placeholder'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(gone.0, 0, "placeholder row removed");

        let segs = CopilotRepository::list_segments(&pool, "saved").await.unwrap();
        assert_eq!(segs.iter().map(|s| s.text.as_str()).collect::<Vec<_>>(), vec!["first", "second"]);
        assert_eq!(segs[0].seq, 0);
        assert_eq!(segs[0].start_ms, 1000);
        assert_eq!(segs[1].end_ms, 6000);

        let mode: (String,) = sqlx::query_as("SELECT meeting_mode FROM meetings WHERE id = 'saved'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(mode.0, "copilot");

        // Idempotent: a second call neither duplicates segments nor fails.
        CopilotRepository::finalize_recording(&pool, "saved", "saved").await.unwrap();
        assert_eq!(CopilotRepository::list_segments(&pool, "saved").await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn user_tags_persist_follow_the_recording_and_lock_after_summary() {
        let pool = test_pool().await;
        let b = CopilotRepository::bank_add(&pool, "Budget?", &[], &[], None).await.unwrap();
        let ids = CopilotRepository::begin_draft(&pool, "draft-t", &[b]).await.unwrap();
        let qid = &ids[0];

        // Untagged by default; each tap is saved immediately on the draft session.
        assert_eq!(CopilotRepository::list_questions(&pool, "draft-t").await.unwrap()[0].user_tag, None);
        CopilotRepository::set_user_tag(&pool, qid, Some("asked")).await.unwrap();
        CopilotRepository::set_user_tag(&pool, qid, Some("answered")).await.unwrap();
        assert!(CopilotRepository::set_user_tag(&pool, qid, Some("Answered")).await.is_err(), "exact values only");
        assert!(CopilotRepository::set_user_tag(&pool, "nope", Some("asked")).await.is_err());

        // The tag travels with the question when the recording is attached.
        sqlx::query("INSERT INTO meetings (id, title, created_at, updated_at) VALUES ('rec','t','2026-01-01','2026-01-01')")
            .execute(&pool).await.unwrap();
        CopilotRepository::finalize_recording(&pool, "draft-t", "rec").await.unwrap();
        let q = &CopilotRepository::list_questions(&pool, "rec").await.unwrap()[0];
        assert_eq!(q.user_tag.as_deref(), Some("answered"));

        // Still editable after the call, until the summary is generated.
        CopilotRepository::set_user_tag(&pool, qid, None).await.unwrap();
        CopilotRepository::set_user_tag(&pool, qid, Some("not_asked")).await.unwrap();
        sqlx::query("UPDATE meetings SET processing_stage = 'complete' WHERE id = 'rec'")
            .execute(&pool).await.unwrap();
        let locked = CopilotRepository::set_user_tag(&pool, qid, Some("asked")).await.unwrap_err();
        assert!(locked.to_string().contains("locked"));
        // A failed analysis unlocks them again.
        sqlx::query("UPDATE meetings SET processing_stage = 'failed' WHERE id = 'rec'")
            .execute(&pool).await.unwrap();
        CopilotRepository::set_user_tag(&pool, qid, Some("asked")).await.unwrap();
    }

    #[tokio::test]
    async fn multiple_named_banks_hold_their_own_questions() {
        let pool = test_pool().await;
        let sales = CopilotRepository::group_create(&pool, "  Sales intake ").await.unwrap();
        let hr = CopilotRepository::group_create(&pool, "HR screening").await.unwrap();

        CopilotRepository::bank_add(&pool, "Budget?", &["Spend?".into()], &["10k".into()], Some(&sales))
            .await.unwrap();
        CopilotRepository::bank_add_many(&pool, &[q("Notice period?"), q("Salary?")], Some(&hr))
            .await.unwrap();
        // No bank given -> the default "General" bank is created on demand.
        CopilotRepository::bank_add(&pool, "Anything else?", &[], &[], None).await.unwrap();

        let groups = CopilotRepository::group_list(&pool).await.unwrap();
        let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, vec!["Sales intake", "HR screening", "General"], "trimmed, creation order");

        let items = CopilotRepository::bank_list(&pool).await.unwrap();
        let in_bank = |id: &str| -> Vec<String> {
            items.iter().filter(|i| i.bank_id.as_deref() == Some(id)).map(|i| i.question.clone()).collect()
        };
        assert_eq!(in_bank(&sales), vec!["Budget?"]);
        assert_eq!(in_bank(&hr), vec!["Notice period?", "Salary?"], "bulk order preserved");
        assert_eq!(in_bank(CopilotRepository::DEFAULT_BANK_ID), vec!["Anything else?"]);
    }

    #[tokio::test]
    async fn bank_names_are_unique_ignoring_case_and_never_empty() {
        let pool = test_pool().await;
        CopilotRepository::group_create(&pool, "Sales").await.unwrap();
        let dup = CopilotRepository::group_create(&pool, "sales").await.unwrap_err();
        assert!(dup.to_string().contains("already exists"), "{dup}");
        assert!(CopilotRepository::group_create(&pool, "   ").await.is_err());

        let other = CopilotRepository::group_create(&pool, "HR").await.unwrap();
        let clash = CopilotRepository::group_rename(&pool, &other, "SALES").await.unwrap_err();
        assert!(clash.to_string().contains("already exists"));
        CopilotRepository::group_rename(&pool, &other, "People").await.unwrap();
        assert!(CopilotRepository::group_rename(&pool, "nope", "X").await.is_err());
    }

    #[tokio::test]
    async fn adding_to_a_missing_bank_fails_and_deleting_a_bank_removes_its_questions() {
        let pool = test_pool().await;
        assert!(CopilotRepository::bank_add(&pool, "Q", &[], &[], Some("missing")).await.is_err());

        let a = CopilotRepository::group_create(&pool, "A").await.unwrap();
        let b = CopilotRepository::group_create(&pool, "B").await.unwrap();
        let qa = CopilotRepository::bank_add(&pool, "in A", &[], &[], Some(&a)).await.unwrap();
        CopilotRepository::bank_add(&pool, "in B", &[], &[], Some(&b)).await.unwrap();

        // A question already used in a meeting survives deleting its bank.
        CopilotRepository::add_bank_questions(&pool, "m-del", &[qa]).await.unwrap();
        CopilotRepository::group_delete(&pool, &a).await.unwrap();

        let items = CopilotRepository::bank_list(&pool).await.unwrap();
        assert_eq!(items.iter().map(|i| i.question.as_str()).collect::<Vec<_>>(), vec!["in B"]);
        assert_eq!(CopilotRepository::group_list(&pool).await.unwrap().len(), 1);
        assert_eq!(CopilotRepository::list_questions(&pool, "m-del").await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn group_import_creates_then_replaces_atomically() {
        let pool = test_pool().await;
        let (id, created) = CopilotRepository::group_import(&pool, "  Sales intake ", &[q("A"), q("B")]).await.unwrap();
        assert!(created);
        // Same name in another case updates the same set and replaces its questions.
        let (id2, created2) = CopilotRepository::group_import(&pool, "SALES INTAKE", &[q("C")]).await.unwrap();
        assert_eq!(id, id2);
        assert!(!created2);
        let items = CopilotRepository::bank_list(&pool).await.unwrap();
        assert_eq!(items.iter().map(|i| i.question.as_str()).collect::<Vec<_>>(), vec!["C"]);
        assert_eq!(CopilotRepository::group_list(&pool).await.unwrap().len(), 1);

        // Bad input changes nothing.
        assert!(CopilotRepository::group_import(&pool, "Sales intake", &[]).await.is_err());
        assert!(CopilotRepository::group_import(&pool, "Sales intake", &[q("ok"), q("  ")]).await.is_err());
        assert!(CopilotRepository::group_import(&pool, "  ", &[q("x")]).await.is_err());
        assert_eq!(CopilotRepository::bank_list(&pool).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn drafts_are_replaced_hidden_and_never_duplicate_questions() {
        let pool = test_pool().await;
        let set = CopilotRepository::group_create(&pool, "S").await.unwrap();
        let a = CopilotRepository::bank_add(&pool, "A?", &[], &[], Some(&set)).await.unwrap();
        let b = CopilotRepository::bank_add(&pool, "B?", &[], &[], Some(&set)).await.unwrap();
        let ids = vec![a, b];

        CopilotRepository::begin_draft(&pool, "draft-1", &ids).await.unwrap();
        // retry of the same session: still exactly the set's questions
        CopilotRepository::begin_draft(&pool, "draft-1", &ids).await.unwrap();
        assert_eq!(CopilotRepository::list_questions(&pool, "draft-1").await.unwrap().len(), 2);
        // a new session purges the abandoned draft
        CopilotRepository::begin_draft(&pool, "draft-2", &ids).await.unwrap();
        assert!(CopilotRepository::list_questions(&pool, "draft-1").await.unwrap().is_empty());
        assert!(CopilotRepository::begin_draft(&pool, "meeting-x", &ids).await.is_err());

        // drafts are not meetings: the sidebar query never returns them
        let listed = crate::database::repositories::meeting::MeetingsRepository::get_meetings(&pool).await.unwrap();
        assert!(listed.iter().all(|m| !m.id.starts_with("draft-")));
    }
}

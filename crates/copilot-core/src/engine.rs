//! End-to-end analysis orchestration (Chunk 6).
//!
//! Ties windowing, embedding, hybrid retrieval, prompting, constrained parsing
//! and rubric validation together for a whole question set, plus the notes pass.

use crate::embedding::Embedder;
use crate::heuristic::HeuristicLlm;
use crate::model::{Question, QuestionStatus, TranscriptTurn, UserTag, Verdict};
use crate::schema::Notes;
use crate::window::window_transcript;
use crate::{analyze_notes, analyze_question, AnalysisRequest, AnalyzeError, LlmClient};

#[derive(Debug, Clone, PartialEq)]
pub struct QuestionVerdict {
    pub question_id: String,
    pub verdict: Verdict,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AnalysisOutcome {
    pub verdicts: Vec<QuestionVerdict>,
    pub notes: Option<Notes>,
}

/// Analyse every question against the transcript, optionally extracting notes.
///
/// Windows are embedded once and reused for every question.
pub fn analyze_meeting(
    questions: &[Question],
    turns: &[TranscriptTurn],
    embedder: &dyn Embedder,
    alpha: f32,
    top_k: usize,
    client: &dyn LlmClient,
    extract_notes: bool,
) -> Result<AnalysisOutcome, AnalyzeError> {
    if turns.is_empty() || questions.is_empty() {
        return Ok(AnalysisOutcome {
            verdicts: Vec::new(),
            notes: None,
        });
    }

    let windows = window_transcript(turns, 4, 1);
    let window_embeddings: Vec<Vec<f32>> =
        windows.iter().map(|w| embedder.embed(&w.text)).collect();
    let known: Vec<String> = turns.iter().map(|t| format!("t{}", t.seq)).collect();

    let mut verdicts = Vec::with_capacity(questions.len());
    for q in questions {
        let query_embedding = embedder.embed(&q.retrieval_phrasings().join(" \n "));
        let req = AnalysisRequest {
            question: q,
            turns,
            window_embeddings: &window_embeddings,
            query_embedding,
            alpha,
            top_k,
            known_segment_ids: &known,
            user_tag: None,
            include_tag: false,
        };
        let verdict = analyze_question(req, client)?;
        verdicts.push(QuestionVerdict {
            question_id: q.id.clone(),
            verdict,
        });
    }

    let notes = if extract_notes {
        let transcript = turns
            .iter()
            .map(|t| {
                let speaker = t.speaker_label.clone().unwrap_or_else(|| {
                    if t.channel == 0 {
                        "User".to_string()
                    } else {
                        "Participant".to_string()
                    }
                });
                format!("{speaker}: {}", t.text.trim())
            })
            .collect::<Vec<_>>()
            .join("\n");
        Some(analyze_notes(&transcript, client)?)
    } else {
        None
    };

    Ok(AnalysisOutcome { verdicts, notes })
}

/// Answer text for a question the interviewer tagged Not asked (and the
/// transcript does not contradict), or any question whose status is `not_asked`.
pub const NOT_ASKED_ANSWER: &str = "Not discussed in this call.";
/// Flag on every question the interviewer left untagged.
pub const UNTAGGED_NOTE: &str = "Not explicitly tagged.";

/// A question plus the interviewer's live tag (user input, may be absent).
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedQuestion {
    pub question: Question,
    pub tag: Option<UserTag>,
}

/// Result of the tag-aware analysis for one question.
#[derive(Debug, Clone, PartialEq)]
pub struct TaggedVerdict {
    pub question_id: String,
    pub tag: Option<UserTag>,
    pub verdict: Verdict,
    /// "Not explicitly tagged." for untagged questions, or a discrepancy between
    /// the tag and the transcript. Never silently dropped.
    pub note: Option<String>,
    /// Whether `client` (the reasoning model) was called for this question.
    pub model_called: bool,
}

fn not_asked_verdict() -> Verdict {
    Verdict {
        status: QuestionStatus::NotAsked,
        rating: None,
        extracted_answer: Some(NOT_ASKED_ANSWER.to_string()),
        missing_details: None,
        confidence: 1.0,
        evidence_segment_ids: Vec::new(),
    }
}

fn discrepancy(tag: UserTag, status: QuestionStatus) -> Option<String> {
    use QuestionStatus::*;
    match (tag, status) {
        (UserTag::NotAsked, AskedNotAnswered | Answered) => Some(
            "Tagged Not asked, but the transcript shows this question was discussed.".into(),
        ),
        (UserTag::Asked | UserTag::Answered, NotAsked) => Some(format!(
            "Tagged {}, but no supporting lines were found in the transcript.",
            tag.label()
        )),
        (UserTag::Answered, AskedNotAnswered) => Some(
            "Tagged Answered, but the transcript does not contain a clear answer.".into(),
        ),
        _ => None,
    }
}

/// Post-call question summary driven by the interviewer's tags.
///
/// * `Asked` / `Answered`: retrieve + reason with `client`; the tag is passed as
///   guidance (Answered = an answer very likely exists).
/// * `NotAsked`: no model call. A cheap rule-based probe checks the transcript;
///   only if it clearly contradicts the tag is `client` consulted, and the
///   discrepancy is reported. Otherwise the answer is "Not asked in this call."
/// * Untagged: best-effort `client` analysis, flagged "Not explicitly tagged."
///
/// Verdicts always follow the transcript; a disagreeing tag produces a note,
/// never a silent override in either direction.
pub fn analyze_tagged_meeting(
    questions: &[TaggedQuestion],
    turns: &[TranscriptTurn],
    embedder: &dyn Embedder,
    alpha: f32,
    top_k: usize,
    client: &dyn LlmClient,
) -> Result<Vec<TaggedVerdict>, AnalyzeError> {
    if questions.is_empty() {
        return Ok(Vec::new());
    }
    let windows = window_transcript(turns, 4, 1);
    let window_embeddings: Vec<Vec<f32>> =
        windows.iter().map(|w| embedder.embed(&w.text)).collect();
    let known: Vec<String> = turns.iter().map(|t| format!("t{}", t.seq)).collect();

    let mut out = Vec::with_capacity(questions.len());
    for tq in questions {
        let q = &tq.question;
        let request = |include_tag: bool| AnalysisRequest {
            question: q,
            turns,
            window_embeddings: &window_embeddings,
            query_embedding: embedder.embed(&q.retrieval_phrasings().join(" \n ")),
            alpha,
            top_k,
            known_segment_ids: &known,
            user_tag: tq.tag,
            include_tag,
        };

        let (verdict, note, model_called) = match tq.tag {
            Some(UserTag::NotAsked) => {
                // Rule-based probe only: no tokens are spent unless the transcript
                // clearly covers the question.
                let contradicted = !turns.is_empty()
                    && matches!(
                        analyze_question(request(false), &HeuristicLlm),
                        Ok(v) if v.status == QuestionStatus::Answered
                    );
                if contradicted {
                    let v = analyze_question(request(true), client)?;
                    match discrepancy(UserTag::NotAsked, v.status) {
                        Some(n) => (v, Some(n), true),
                        None => (not_asked_verdict(), None, true),
                    }
                } else {
                    (not_asked_verdict(), None, false)
                }
            }
            Some(tag) => {
                if turns.is_empty() {
                    (
                        not_asked_verdict_without_text(),
                        discrepancy(tag, QuestionStatus::NotAsked),
                        false,
                    )
                } else {
                    let v = analyze_question(request(true), client)?;
                    let n = discrepancy(tag, v.status);
                    (v, n, true)
                }
            }
            None => {
                if turns.is_empty() {
                    (not_asked_verdict_without_text(), Some(UNTAGGED_NOTE.into()), false)
                } else {
                    let v = analyze_question(request(true), client)?;
                    (v, Some(UNTAGGED_NOTE.into()), true)
                }
            }
        };
        out.push(TaggedVerdict {
            question_id: q.id.clone(),
            tag: tq.tag,
            verdict,
            note,
            model_called,
        });
    }
    Ok(out)
}

/// Not asked because there is no transcript at all (nothing to search).
fn not_asked_verdict_without_text() -> Verdict {
    Verdict {
        extracted_answer: None,
        confidence: 0.0,
        ..not_asked_verdict()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::HashingEmbedder;
    use crate::heuristic::HeuristicLlm;
    use crate::model::QuestionStatus;

    fn turns() -> Vec<TranscriptTurn> {
        vec![
            TranscriptTurn {
                seq: 0,
                start_ms: 0,
                end_ms: 1000,
                channel: 1,
                speaker_label: Some("Dana".into()),
                text: "Thanks for joining the intake call.".into(),
            },
            TranscriptTurn {
                seq: 1,
                start_ms: 1000,
                end_ms: 2000,
                channel: 1,
                speaker_label: Some("Dana".into()),
                text: "The base salary is 50k and the start date is Q3.".into(),
            },
            TranscriptTurn {
                seq: 2,
                start_ms: 2000,
                end_ms: 3000,
                channel: 0,
                speaker_label: None,
                text: "Great, and are there relocation benefits?".into(),
            },
            TranscriptTurn {
                seq: 3,
                start_ms: 3000,
                end_ms: 4000,
                channel: 1,
                speaker_label: Some("Dana".into()),
                text: "We can talk about that later.".into(),
            },
        ]
    }

    fn questions() -> Vec<Question> {
        vec![
            Question {
                id: "q1".into(),
                sort_order: 0,
                main_question: "what is the salary and start date".into(),
                sample_questions: vec![],
                sample_answers: vec![],
            },
            Question {
                id: "q2".into(),
                sort_order: 1,
                main_question: "are there relocation benefits".into(),
                sample_questions: vec![],
                sample_answers: vec![],
            },
        ]
    }

    #[test]
    fn full_pipeline_runs_without_a_model() {
        let embedder = HashingEmbedder::new(256);
        let outcome = analyze_meeting(
            &questions(),
            &turns(),
            &embedder,
            0.5,
            5,
            &HeuristicLlm,
            true,
        )
        .unwrap();
        assert_eq!(outcome.verdicts.len(), 2);
        let q1 = &outcome.verdicts[0];
        assert_eq!(q1.verdict.status, QuestionStatus::Answered);
        // Every cited id must exist in the transcript.
        let known: Vec<String> = turns().iter().map(|t| format!("t{}", t.seq)).collect();
        for id in &q1.verdict.evidence_segment_ids {
            assert!(known.contains(id), "citation {id} not in transcript");
        }
        assert!(outcome.notes.is_some());
    }

    #[test]
    fn empty_input_is_handled() {
        let embedder = HashingEmbedder::new(64);
        let outcome =
            analyze_meeting(&[], &turns(), &embedder, 0.5, 5, &HeuristicLlm, false).unwrap();
        assert!(outcome.verdicts.is_empty());
        let outcome2 =
            analyze_meeting(&questions(), &[], &embedder, 0.5, 5, &HeuristicLlm, false).unwrap();
        assert!(outcome2.verdicts.is_empty());
    }

    #[test]
    fn deterministic_across_runs() {
        let embedder = HashingEmbedder::new(256);
        let a = analyze_meeting(
            &questions(),
            &turns(),
            &embedder,
            0.5,
            5,
            &HeuristicLlm,
            false,
        )
        .unwrap();
        let b = analyze_meeting(
            &questions(),
            &turns(),
            &embedder,
            0.5,
            5,
            &HeuristicLlm,
            false,
        )
        .unwrap();
        assert_eq!(a, b);
    }

    // ---- tag-aware analysis -------------------------------------------------

    use std::cell::RefCell;

    /// Wraps the rule-based client and records every model call, so tests can
    /// prove which questions consumed a model call and what the prompt said.
    struct RecordingLlm {
        prompts: RefCell<Vec<String>>,
    }
    impl RecordingLlm {
        fn new() -> Self {
            Self { prompts: RefCell::new(Vec::new()) }
        }
        fn called_for(&self, question: &str) -> Vec<String> {
            self.prompts
                .borrow()
                .iter()
                .filter(|p| p.contains(&format!("QUESTION (main phrasing): {question}\n")))
                .cloned()
                .collect()
        }
    }
    impl LlmClient for RecordingLlm {
        fn complete(&self, system: &str, user: &str, schema: &str) -> Result<String, String> {
            self.prompts.borrow_mut().push(user.to_string());
            HeuristicLlm.complete(system, user, schema)
        }
    }

    const SALARY: &str = "what is the salary and start date";
    const NOTICE: &str = "what is the notice period";

    fn tagged(tags: &[(&str, Option<UserTag>)]) -> Vec<TaggedQuestion> {
        tags.iter()
            .enumerate()
            .map(|(i, (q, tag))| TaggedQuestion {
                question: Question {
                    id: format!("q{i}"),
                    sort_order: i as i64,
                    main_question: (*q).into(),
                    sample_questions: vec![],
                    sample_answers: vec![],
                },
                tag: *tag,
            })
            .collect()
    }

    fn run(tags: &[(&str, Option<UserTag>)], llm: &RecordingLlm) -> Vec<TaggedVerdict> {
        analyze_tagged_meeting(&tagged(tags), &turns(), &HashingEmbedder::new(256), 0.5, 5, llm).unwrap()
    }

    #[test]
    fn not_asked_tag_skips_the_model_and_says_so() {
        let llm = RecordingLlm::new();
        let out = run(&[(NOTICE, Some(UserTag::NotAsked))], &llm);
        assert!(llm.prompts.borrow().is_empty(), "no model call for a Not asked question");
        assert!(!out[0].model_called);
        assert_eq!(out[0].verdict.status, QuestionStatus::NotAsked);
        assert_eq!(out[0].verdict.extracted_answer.as_deref(), Some(NOT_ASKED_ANSWER));
        assert!(out[0].verdict.evidence_segment_ids.is_empty());
        assert_eq!(out[0].note, None);
    }

    #[test]
    fn not_asked_tag_contradicted_by_the_transcript_is_surfaced_with_a_note() {
        let llm = RecordingLlm::new();
        let out = run(&[(SALARY, Some(UserTag::NotAsked))], &llm);
        assert!(out[0].model_called, "clear contradiction escalates to the model");
        assert_ne!(out[0].verdict.status, QuestionStatus::NotAsked, "answer is surfaced");
        assert!(!out[0].verdict.evidence_segment_ids.is_empty(), "and cites evidence");
        assert!(out[0].note.as_deref().unwrap().starts_with("Tagged Not asked"));
    }

    #[test]
    fn asked_and_answered_tags_reach_the_prompt() {
        let llm = RecordingLlm::new();
        let out = run(&[(SALARY, Some(UserTag::Answered)), (NOTICE, Some(UserTag::Asked))], &llm);
        assert!(llm.called_for(SALARY)[0].contains("INTERVIEWER TAG: Answered"));
        assert!(llm.called_for(NOTICE)[0].contains("INTERVIEWER TAG: Asked"));
        assert_eq!(out[0].verdict.status, QuestionStatus::Answered);
        assert_eq!(out[0].note, None, "tag and transcript agree");
        assert!(out[0].verdict.extracted_answer.as_deref().unwrap().starts_with("Answered."));
        // Tagged Asked, but nothing in the transcript: no invented answer, a note instead.
        assert_eq!(out[1].verdict.status, QuestionStatus::NotAsked);
        assert_eq!(out[1].verdict.extracted_answer, None);
        assert!(out[1].note.as_deref().unwrap().starts_with("Tagged Asked, but no supporting lines"));
    }

    #[test]
    fn untagged_questions_get_best_effort_search_and_a_flag() {
        let llm = RecordingLlm::new();
        let out = run(&[(SALARY, None)], &llm);
        assert!(out[0].model_called);
        assert!(llm.called_for(SALARY)[0].contains("did not tag"));
        assert_eq!(out[0].note.as_deref(), Some(UNTAGGED_NOTE));
        assert_eq!(out[0].verdict.status, QuestionStatus::Answered);
    }

    #[test]
    fn tags_change_the_amount_of_model_work() {
        let all_untagged = RecordingLlm::new();
        run(&[(SALARY, None), (NOTICE, None)], &all_untagged);
        let tagged = RecordingLlm::new();
        run(&[(SALARY, Some(UserTag::Answered)), (NOTICE, Some(UserTag::NotAsked))], &tagged);
        assert_eq!(all_untagged.prompts.borrow().len(), 2);
        assert_eq!(tagged.prompts.borrow().len(), 1, "the Not asked question cost nothing");
    }

    #[test]
    fn empty_transcript_never_calls_the_model() {
        let llm = RecordingLlm::new();
        let out = analyze_tagged_meeting(
            &tagged(&[(SALARY, Some(UserTag::Answered)), (NOTICE, None)]),
            &[],
            &HashingEmbedder::new(64),
            0.5,
            5,
            &llm,
        )
        .unwrap();
        assert!(llm.prompts.borrow().is_empty());
        assert!(out[0].note.as_deref().unwrap().starts_with("Tagged Answered"));
        assert_eq!(out[1].note.as_deref(), Some(UNTAGGED_NOTE));
    }
}

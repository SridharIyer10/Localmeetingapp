//! Data models for Intake Co-Pilot (Chunk 1).
//!
//! Multi-value text fields are stored as JSON arrays and parsed on read. They are
//! never split on commas.

use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TranscriptSegmentModel {
    pub id: String,
    pub meeting_id: String,
    pub seq: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    pub channel: i32,
    pub speaker_label: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MeetingQuestionModel {
    pub id: String,
    pub meeting_id: String,
    pub sort_order: i64,
    pub main_question: String,
    pub sample_questions_json: String,
    pub sample_answers_json: String,
    pub status: String,
    pub rating: Option<String>,
    pub extracted_answer: Option<String>,
    pub missing_details: Option<String>,
    pub confidence: Option<f64>,
    pub evidence_segment_ids: String,
    pub asked_timestamp_ms: Option<i64>,
    pub user_ticked: i64,
    pub user_notes: Option<String>,
    pub manually_overridden: i64,
    /// Interviewer's live tag: asked | not_asked | answered | NULL (user input).
    pub user_tag: Option<String>,
    /// Analysis note: untagged flag or tag/transcript discrepancy.
    pub tag_note: Option<String>,
}

impl MeetingQuestionModel {
    pub fn sample_questions(&self) -> Vec<String> {
        json_string_array(&self.sample_questions_json)
    }

    pub fn sample_answers(&self) -> Vec<String> {
        json_string_array(&self.sample_answers_json)
    }

    pub fn evidence_segment_ids(&self) -> Vec<String> {
        json_string_array(&self.evidence_segment_ids)
    }
}

/// Wire shape of a meeting question as the frontend (`CopilotQuestion` in
/// `src/lib/copilot/types.ts`) consumes it: JSON arrays are real arrays and the
/// SQLite 0/1 flags are booleans. Returning the raw DB model instead leaves
/// `sample_questions`/`evidence_segment_ids` as JSON strings and the flags as
/// numbers, which crashes the question panel.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MeetingQuestionView {
    pub id: String,
    pub meeting_id: String,
    pub sort_order: i64,
    pub main_question: String,
    pub sample_questions: Vec<String>,
    pub sample_answers: Vec<String>,
    pub status: String,
    pub rating: Option<String>,
    pub extracted_answer: Option<String>,
    pub missing_details: Option<String>,
    pub confidence: Option<f64>,
    pub evidence_segment_ids: Vec<String>,
    pub asked_timestamp_ms: Option<i64>,
    pub user_notes: Option<String>,
    pub manually_overridden: bool,
    pub user_tag: Option<String>,
    pub tag_note: Option<String>,
}

impl From<MeetingQuestionModel> for MeetingQuestionView {
    fn from(m: MeetingQuestionModel) -> Self {
        Self {
            sample_questions: m.sample_questions(),
            sample_answers: m.sample_answers(),
            evidence_segment_ids: m.evidence_segment_ids(),
            manually_overridden: m.manually_overridden != 0,
            id: m.id,
            meeting_id: m.meeting_id,
            sort_order: m.sort_order,
            main_question: m.main_question,
            status: m.status,
            rating: m.rating,
            extracted_answer: m.extracted_answer,
            missing_details: m.missing_details,
            confidence: m.confidence,
            asked_timestamp_ms: m.asked_timestamp_ms,
            user_notes: m.user_notes,
            user_tag: m.user_tag,
            tag_note: m.tag_note,
        }
    }
}

/// A named question bank ("header") that groups reusable questions.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct QuestionBankGroupModel {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct BankQuestionModel {
    pub id: String,
    /// The named question bank (`question_bank_groups.id`) this question belongs to.
    pub bank_id: Option<String>,
    pub question: String,
    pub sample_questions_json: String,
    pub sample_answers_json: String,
    pub created_at: String,
    pub updated_at: String,
}

impl BankQuestionModel {
    pub fn sample_questions(&self) -> Vec<String> {
        json_string_array(&self.sample_questions_json)
    }
    pub fn sample_answers(&self) -> Vec<String> {
        json_string_array(&self.sample_answers_json)
    }
}

/// General summary + notes for a meeting (always available after analysis).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeetingNotes {
    pub summary: String,
    pub action_points: Vec<String>,
    pub next_steps: Vec<String>,
    pub actions_discussed: Vec<String>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AiSettingsModel {
    pub id: i64,
    pub provider_mode: String,
    pub embedded_llm_model: Option<String>,
    pub embedded_stt_model: Option<String>,
    pub local_base_url: Option<String>,
    pub local_model_name: Option<String>,
    pub cloud_provider: Option<String>,
    pub cloud_model_name: Option<String>,
}

pub use copilot_core::json::{json_string_array, to_json_array};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn question_view_matches_the_frontend_contract() {
        let m = MeetingQuestionModel {
            id: "q1".into(),
            meeting_id: "m1".into(),
            sort_order: 0,
            main_question: "Salary?".into(),
            sample_questions_json: r#"["Pay?","Comp, total?"]"#.into(),
            sample_answers_json: "[]".into(),
            status: "not_asked".into(),
            rating: None,
            extracted_answer: None,
            missing_details: None,
            confidence: None,
            evidence_segment_ids: r#"["t1"]"#.into(),
            asked_timestamp_ms: None,
            user_ticked: 1,
            user_notes: None,
            manually_overridden: 0,
            user_tag: Some("answered".into()),
            tag_note: None,
        };
        let v = serde_json::to_value(MeetingQuestionView::from(m)).unwrap();
        // A value containing a comma must survive (never split on ",").
        assert_eq!(v["sample_questions"], serde_json::json!(["Pay?", "Comp, total?"]));
        assert_eq!(v["sample_answers"], serde_json::json!([]));
        assert_eq!(v["evidence_segment_ids"], serde_json::json!(["t1"]));
        assert_eq!(v["user_tag"], serde_json::json!("answered"));
        assert!(v.get("user_ticked").is_none(), "the old tick flag is not part of the contract");
        assert_eq!(v["manually_overridden"], serde_json::json!(false));
        assert!(v.get("sample_questions_json").is_none());
    }
}

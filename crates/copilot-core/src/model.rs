//! Core domain types for Intake Co-Pilot analysis.
//!
//! Coverage (`QuestionStatus`) and quality (`AnswerRating`) are deliberately
//! orthogonal. A rating is only ever set when `status == Answered`.

use serde::{Deserialize, Serialize};

/// Coverage of a question by the conversation. Three values only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    NotAsked,
    AskedNotAnswered,
    Answered,
}

impl QuestionStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            QuestionStatus::NotAsked => "not_asked",
            QuestionStatus::AskedNotAnswered => "asked_not_answered",
            QuestionStatus::Answered => "answered",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "not_asked" => Some(QuestionStatus::NotAsked),
            "asked_not_answered" => Some(QuestionStatus::AskedNotAnswered),
            "answered" | "asked_and_answered" => Some(QuestionStatus::Answered),
            _ => None,
        }
    }

    pub fn as_schema_str(self) -> &'static str {
        match self {
            QuestionStatus::NotAsked => "not_asked",
            QuestionStatus::AskedNotAnswered => "asked_not_answered",
            QuestionStatus::Answered => "asked_and_answered",
        }
    }
}

/// What the interviewer tapped for a question during the call. User input only:
/// nothing computes it, and it is never a verdict. The post-call analysis reads it
/// as a hint about where to look (and where not to spend a model call).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserTag {
    Asked,
    NotAsked,
    Answered,
}

impl UserTag {
    pub fn as_str(self) -> &'static str {
        match self {
            UserTag::Asked => "asked",
            UserTag::NotAsked => "not_asked",
            UserTag::Answered => "answered",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "asked" => Some(UserTag::Asked),
            "not_asked" => Some(UserTag::NotAsked),
            "answered" => Some(UserTag::Answered),
            _ => None,
        }
    }

    /// Label shown to the user and written into prompts.
    pub fn label(self) -> &'static str {
        match self {
            UserTag::Asked => "Asked",
            UserTag::NotAsked => "Not asked",
            UserTag::Answered => "Answered",
        }
    }
}

/// Quality of an answer. Only meaningful when the question was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum AnswerRating {
    Poor,
    Partial,
    Strong,
}

impl AnswerRating {
    pub fn as_str(self) -> &'static str {
        match self {
            AnswerRating::Poor => "Poor",
            AnswerRating::Partial => "Partial",
            AnswerRating::Strong => "Strong",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "Poor" | "poor" => Some(AnswerRating::Poor),
            "Partial" | "partial" => Some(AnswerRating::Partial),
            "Strong" | "strong" => Some(AnswerRating::Strong),
            _ => None,
        }
    }
}

/// A question loaded before the call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Question {
    pub id: String,
    pub sort_order: i64,
    pub main_question: String,
    #[serde(default)]
    pub sample_questions: Vec<String>,
    #[serde(default)]
    pub sample_answers: Vec<String>,
}

impl Question {
    /// All phrasings used to retrieve evidence: the main question plus samples.
    pub fn retrieval_phrasings(&self) -> Vec<String> {
        let mut out = Vec::with_capacity(1 + self.sample_questions.len());
        out.push(self.main_question.clone());
        out.extend(self.sample_questions.iter().cloned());
        out
    }
}

/// A transcript turn (one segment).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TranscriptTurn {
    pub seq: i64,
    pub start_ms: i64,
    pub end_ms: i64,
    /// 0 = mic/user, 1 = loopback/participant
    pub channel: i32,
    pub speaker_label: Option<String>,
    pub text: String,
}

/// A retrieved evidence window or segment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Evidence {
    pub segment_ids: Vec<String>,
    pub text: String,
    pub score: f32,
}

/// A raw model judgement before validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawVerdict {
    /// "asked_and_answered" | "asked_not_answered" | "not_asked"
    pub status: String,
    /// "Poor" | "Partial" | "Strong" | null
    #[serde(default)]
    pub rating: Option<String>,
    #[serde(default)]
    pub extracted_answer: Option<String>,
    #[serde(default)]
    pub missing_details: Option<String>,
    #[serde(default)]
    pub criteria_addressed: Vec<String>,
    #[serde(default)]
    pub criteria_total: Vec<String>,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub evidence_segment_ids: Vec<String>,
}

/// A validated verdict written back to the database.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Verdict {
    pub status: QuestionStatus,
    pub rating: Option<AnswerRating>,
    pub extracted_answer: Option<String>,
    pub missing_details: Option<String>,
    pub confidence: f32,
    pub evidence_segment_ids: Vec<String>,
}

/// Provider selection for analysis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMode {
    EmbeddedLocal,
    LocalServer,
    Cloud,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_round_trips() {
        for s in [
            QuestionStatus::NotAsked,
            QuestionStatus::AskedNotAnswered,
            QuestionStatus::Answered,
        ] {
            assert_eq!(QuestionStatus::from_str(s.as_str()), Some(s));
        }
    }

    #[test]
    fn user_tag_round_trips() {
        for t in [UserTag::Asked, UserTag::NotAsked, UserTag::Answered] {
            assert_eq!(UserTag::from_str(t.as_str()), Some(t));
        }
        assert_eq!(UserTag::from_str("Asked"), None, "stored values are exact");
    }

    #[test]
    fn rating_round_trips_case_insensitively() {
        assert_eq!(AnswerRating::from_str("strong"), Some(AnswerRating::Strong));
        assert_eq!(AnswerRating::from_str("Poor"), Some(AnswerRating::Poor));
        assert_eq!(AnswerRating::from_str("nonsense"), None);
    }

    #[test]
    fn retrieval_phrasings_include_main_then_samples() {
        let q = Question {
            id: "q1".into(),
            sort_order: 0,
            main_question: "main".into(),
            sample_questions: vec!["a".into(), "b".into()],
            sample_answers: vec![],
        };
        assert_eq!(q.retrieval_phrasings(), vec!["main", "a", "b"]);
    }
}

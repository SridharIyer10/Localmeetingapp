//! JSON schema for constrained decoding and strict parsing.
//!
//! The schema is what `llama-server`'s JSON-schema/GBNF constrained decoding is
//! given, which makes structurally invalid output impossible. Parsing is still
//! strict: unknown fields and invalid types are rejected rather than retried.

use crate::model::RawVerdict;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum SchemaError {
    #[error("invalid JSON: {0}")]
    Json(String),
    #[error("missing required field: {0}")]
    MissingField(String),
    #[error("unknown field not allowed: {0}")]
    UnknownField(String),
    #[error("wrong type for field: {0}")]
    WrongType(String),
}

pub const VERDICT_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["status", "confidence", "evidence_segment_ids"],
  "properties": {
    "status": { "type": "string", "enum": ["asked_and_answered", "asked_not_answered", "not_asked"] },
    "rating": { "type": ["string", "null"], "enum": ["Poor", "Partial", "Strong", null] },
    "extracted_answer": { "type": ["string", "null"] },
    "missing_details": { "type": ["string", "null"] },
    "criteria_addressed": { "type": "array", "items": { "type": "string" } },
    "criteria_total": { "type": "array", "items": { "type": "string" } },
    "confidence": { "type": "number", "minimum": 0, "maximum": 1 },
    "evidence_segment_ids": { "type": "array", "items": { "type": "string" } }
  }
}"#;

pub const UNIFIED_SUMMARY_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["questions", "meeting_summary"],
  "properties": {
    "questions": {
      "type": "array",
      "items": {
        "type": "object",
        "additionalProperties": false,
        "required": ["question", "status", "answer"],
        "properties": {
          "question": { "type": "string" },
          "status": {
            "type": "string",
            "enum": ["asked_and_answered", "asked_not_answered", "not_asked"]
          },
          "answer": { "type": "string" }
        }
      }
    },
    "meeting_summary": {
      "type": "object",
      "additionalProperties": false,
      "required": ["overview", "key_points", "decisions", "action_items", "next_steps"],
      "properties": {
        "overview": { "type": "string" },
        "key_points": { "type": "array", "items": { "type": "string" } },
        "decisions": { "type": "array", "items": { "type": "string" } },
        "action_items": { "type": "array", "items": { "type": "string" } },
        "next_steps": { "type": "array", "items": { "type": "string" } }
      }
    }
  }
}"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnifiedQuestionItem {
    pub question: String,
    /// "asked_and_answered" | "asked_not_answered" | "not_asked"
    pub status: String,
    pub answer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UnifiedMeetingSummary {
    pub overview: String,
    pub key_points: Vec<String>,
    pub decisions: Vec<String>,
    pub action_items: Vec<String>,
    pub next_steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UnifiedSummaryOutput {
    pub questions: Vec<UnifiedQuestionItem>,
    pub meeting_summary: UnifiedMeetingSummary,
}

pub const NOTES_SCHEMA: &str = r#"{
  "type": "object",
  "additionalProperties": false,
  "required": ["summary", "action_points", "next_steps", "actions_discussed"],
  "properties": {
    "summary": { "type": "string" },
    "action_points": { "type": "array", "items": { "type": "string" } },
    "next_steps": { "type": "array", "items": { "type": "string" } },
    "actions_discussed": { "type": "array", "items": { "type": "string" } }
  }
}"#;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Notes {
    /// General summary of the meeting. Always produced, independent of questions.
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub action_points: Vec<String>,
    #[serde(default)]
    pub next_steps: Vec<String>,
    #[serde(default)]
    pub actions_discussed: Vec<String>,
}

const VERDICT_ALLOWED: &[&str] = &[
    "status",
    "rating",
    "extracted_answer",
    "missing_details",
    "criteria_addressed",
    "criteria_total",
    "confidence",
    "evidence_segment_ids",
];

const VERDICT_REQUIRED: &[&str] = &["status", "confidence", "evidence_segment_ids"];

const VALID_UNIFIED_STATUSES: &[&str] = &["asked_and_answered", "asked_not_answered", "not_asked"];

/// Strictly parse and validate a unified summary output against `UNIFIED_SUMMARY_SCHEMA`.
pub fn parse_unified_summary(json: &str) -> Result<UnifiedSummaryOutput, SchemaError> {
    let mut value: Value = serde_json::from_str(json).map_err(|e| SchemaError::Json(e.to_string()))?;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| SchemaError::WrongType("root".into()))?;

    for key in obj.keys() {
        if !["questions", "meeting_summary"].contains(&key.as_str()) {
            return Err(SchemaError::UnknownField(key.clone()));
        }
    }
    for req in ["questions", "meeting_summary"] {
        if !obj.contains_key(req) {
            return Err(SchemaError::MissingField(req.to_string()));
        }
    }

    let qs = obj["questions"]
        .as_array()
        .ok_or_else(|| SchemaError::WrongType("questions".into()))?;
    for (i, item) in qs.iter().enumerate() {
        let item_obj = item
            .as_object()
            .ok_or_else(|| SchemaError::WrongType(format!("questions[{i}]")))?;
        for key in item_obj.keys() {
            if !["question", "status", "answer"].contains(&key.as_str()) {
                return Err(SchemaError::UnknownField(format!("questions[{i}].{key}")));
            }
        }
        for req in ["question", "status", "answer"] {
            if !item_obj.contains_key(req) {
                return Err(SchemaError::MissingField(format!("questions[{i}].{req}")));
            }
        }
        let status = item_obj["status"]
            .as_str()
            .ok_or_else(|| SchemaError::WrongType(format!("questions[{i}].status")))?;
        if !VALID_UNIFIED_STATUSES.contains(&status) {
            return Err(SchemaError::WrongType(format!(
                "questions[{i}].status must be one of asked_and_answered | asked_not_answered | not_asked, got '{status}'"
            )));
        }
    }

    let ms = obj
        .get_mut("meeting_summary")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| SchemaError::WrongType("meeting_summary".into()))?;
    for key in ms.keys() {
        if !["overview", "key_points", "decisions", "action_items", "next_steps"]
            .contains(&key.as_str())
        {
            return Err(SchemaError::UnknownField(format!("meeting_summary.{key}")));
        }
    }
    for req in ["overview", "key_points", "decisions", "action_items", "next_steps"] {
        if !ms.contains_key(req) {
            return Err(SchemaError::MissingField(format!("meeting_summary.{req}")));
        }
    }
    for list_field in ["key_points", "decisions", "action_items", "next_steps"] {
        if let Some(Value::String(s)) = ms.get(list_field).cloned() {
            let items = if s.trim().is_empty() {
                Vec::new()
            } else {
                vec![Value::String(s.trim().to_string())]
            };
            ms.insert(list_field.to_string(), Value::Array(items));
        }
    }

    let parsed: UnifiedSummaryOutput =
        serde_json::from_value(value).map_err(|e| SchemaError::WrongType(e.to_string()))?;
    Ok(parsed)
}

/// Strictly parse a model verdict. Rejects unknown fields and type mismatches.
pub fn parse_raw_verdict(json: &str) -> Result<RawVerdict, SchemaError> {
    let value: Value = serde_json::from_str(json).map_err(|e| SchemaError::Json(e.to_string()))?;
    let obj = value
        .as_object()
        .ok_or_else(|| SchemaError::WrongType("root".into()))?;

    for key in obj.keys() {
        if !VERDICT_ALLOWED.contains(&key.as_str()) {
            return Err(SchemaError::UnknownField(key.clone()));
        }
    }
    for key in VERDICT_REQUIRED {
        if !obj.contains_key(*key) {
            return Err(SchemaError::MissingField((*key).to_string()));
        }
    }

    let verdict: RawVerdict =
        serde_json::from_value(value).map_err(|e| SchemaError::WrongType(e.to_string()))?;
    Ok(verdict)
}

/// Strictly parse a notes extraction.
pub fn parse_notes(json: &str) -> Result<Notes, SchemaError> {
    let value: Value = serde_json::from_str(json).map_err(|e| SchemaError::Json(e.to_string()))?;
    let obj = value
        .as_object()
        .ok_or_else(|| SchemaError::WrongType("root".into()))?;
    for key in obj.keys() {
        if !["summary", "action_points", "next_steps", "actions_discussed"].contains(&key.as_str())
        {
            return Err(SchemaError::UnknownField(key.clone()));
        }
    }
    let notes: Notes =
        serde_json::from_value(value).map_err(|e| SchemaError::WrongType(e.to_string()))?;
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_is_valid_json() {
        let v: Value = serde_json::from_str(VERDICT_SCHEMA).unwrap();
        assert_eq!(v["type"], "object");
        let n: Value = serde_json::from_str(NOTES_SCHEMA).unwrap();
        assert_eq!(n["type"], "object");
        let u: Value = serde_json::from_str(UNIFIED_SUMMARY_SCHEMA).unwrap();
        assert_eq!(u["type"], "object");
    }

    #[test]
    fn parses_valid_verdict() {
        let json = r#"{"status":"asked_and_answered","rating":"Strong","confidence":0.9,"evidence_segment_ids":["t3"]}"#;
        let v = parse_raw_verdict(json).unwrap();
        assert_eq!(v.status, "asked_and_answered");
        assert_eq!(v.rating.as_deref(), Some("Strong"));
    }

    #[test]
    fn rating_null_is_allowed() {
        let json = r#"{"status":"asked_not_answered","rating":null,"confidence":0.5,"evidence_segment_ids":["t1"]}"#;
        let v = parse_raw_verdict(json).unwrap();
        assert!(v.rating.is_none());
    }

    #[test]
    fn missing_required_field_rejected() {
        let json = r#"{"status":"asked_and_answered","rating":"Poor"}"#;
        assert_eq!(
            parse_raw_verdict(json),
            Err(SchemaError::MissingField("confidence".into()))
        );
    }

    #[test]
    fn unknown_field_rejected() {
        let json = r#"{"status":"asked_and_answered","confidence":0.5,"evidence_segment_ids":[],"extra":1}"#;
        assert_eq!(
            parse_raw_verdict(json),
            Err(SchemaError::UnknownField("extra".into()))
        );
    }

    #[test]
    fn wrong_type_rejected() {
        let json = r#"{"status":123,"confidence":0.5,"evidence_segment_ids":[]}"#;
        assert!(matches!(
            parse_raw_verdict(json),
            Err(SchemaError::WrongType(_))
        ));
    }

    #[test]
    fn parses_notes() {
        let json = r#"{"action_points":["a"],"next_steps":["b"],"actions_discussed":[]}"#;
        let notes = parse_notes(json).unwrap();
        assert_eq!(notes.action_points, vec!["a"]);
    }

    #[test]
    fn parses_and_validates_unified_summary() {
        let json = r#"{
          "questions": [
            { "question": "Salary?", "status": "asked_and_answered", "answer": "The range is 1-4 lakhs." },
            { "question": "Notice?", "status": "not_asked", "answer": "Not discussed in this call." }
          ],
          "meeting_summary": {
            "overview": "Intake call.",
            "key_points": ["Point 1"],
            "decisions": ["Decision 1"],
            "action_items": ["Action 1"],
            "next_steps": ["Next 1"]
          }
        }"#;
        let parsed = parse_unified_summary(json).unwrap();
        assert_eq!(parsed.questions.len(), 2);
        assert_eq!(parsed.questions[0].status, "asked_and_answered");
        assert_eq!(parsed.meeting_summary.overview, "Intake call.");

        let bad_status = json.replace("asked_and_answered", "answered_only");
        assert!(matches!(
            parse_unified_summary(&bad_status),
            Err(SchemaError::WrongType(_))
        ));
    }
}

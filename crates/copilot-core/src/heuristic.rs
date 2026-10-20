//! Deterministic heuristic analyser (Chunk 6).
//!
//! A rule-based `LlmClient` used for tests, the browser/demo path and machines
//! with no analysis model available. It is deliberately simple: it scores token
//! overlap between the question and the retrieved evidence. It exists so the
//! whole pipeline (window → retrieve → prompt → parse → validate → persist) can
//! run end to end without a GGUF model, and so the eval harness has a floor.

use crate::model::RawVerdict;
use crate::prompt::{DATA_CLOSE, DATA_OPEN};
use crate::retrieval::tokenize;
use crate::schema::{NOTES_SCHEMA, VERDICT_SCHEMA};
use crate::LlmClient;

pub struct HeuristicLlm;

fn between<'a>(s: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let i = s.find(start)? + start.len();
    let rest = &s[i..];
    match rest.find(end) {
        Some(j) => Some(&rest[..j]),
        None => Some(rest),
    }
}

/// Question words and fillers that appear in almost every question; matching on
/// them says nothing about whether *this* question was asked.
const STOPWORDS: &[&str] = &[
    "what", "when", "where", "which", "while", "whom", "whose", "would", "could", "should", "does",
    "did", "have", "has", "had", "your", "you", "this", "that", "these", "those", "there", "their",
    "they", "them", "with", "about", "into", "from", "for", "will", "can", "are", "was", "were",
    "been", "being", "tell", "give", "please", "any", "some", "much", "many", "very", "also",
    "just", "like", "need", "want", "know", "how",
];

fn is_content_token(t: &str) -> bool {
    if STOPWORDS.contains(&t) {
        return false;
    }
    t.len() >= 4 || t.chars().any(|c| c.is_ascii_digit())
}

impl HeuristicLlm {
    fn summary(&self, text: &str) -> String {
        let mut parts: Vec<String> = Vec::new();
        for line in text.lines().map(|l| l.trim()).filter(|l| !l.is_empty()) {
            parts.push(line.to_string());
            if parts.len() >= 3 {
                break;
            }
        }
        let mut s = parts.join(" ");
        if s.len() > 320 {
            s.truncate(320);
        }
        if s.is_empty() {
            "No transcript content to summarise.".to_string()
        } else {
            s
        }
    }

    fn verdict(&self, user: &str) -> RawVerdict {
        let question = between(user, "QUESTION (main phrasing): ", "\n").unwrap_or_default();
        let evidence = between(user, DATA_OPEN, DATA_CLOSE).unwrap_or_default();

        let mut lines: Vec<(String, String)> = Vec::new();
        for line in evidence.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix('[') {
                if let Some(close) = rest.find(']') {
                    lines.push((
                        rest[..close].to_string(),
                        rest[close + 1..].trim().to_string(),
                    ));
                }
            }
        }

        let q_tokens: Vec<String> = tokenize(question)
            .into_iter()
            .filter(|t| is_content_token(t))
            .collect();
        let ev_lower = evidence.to_lowercase();
        let matched: Vec<&String> = q_tokens
            .iter()
            .filter(|t| ev_lower.contains(t.as_str()))
            .collect();
        let overlap = matched.len();
        let total = q_tokens.len().max(1);

        let asked = overlap >= 1;
        let answered = asked && overlap * 2 >= total;
        let rating = if !answered {
            None
        } else if overlap * 10 >= total * 8 {
            Some("Strong".to_string())
        } else {
            Some("Partial".to_string())
        };

        let mut cited: Vec<String> = if asked {
            let mut c: Vec<String> = lines
                .iter()
                .filter(|(_, text)| {
                    let tl = text.to_lowercase();
                    matched.iter().any(|t| tl.contains(t.as_str()))
                })
                .map(|(id, _)| id.clone())
                .collect();
            if c.is_empty() {
                c = lines.iter().take(3).map(|(id, _)| id.clone()).collect();
            }
            c.truncate(3);
            c.sort();
            c.dedup();
            c
        } else {
            Vec::new()
        };
        if !asked {
            cited.clear();
        }

        // Rules cannot interpret, so the fallback quotes the best supporting line
        // (one that is not itself a question) and says so. Nothing is invented.
        let extracted_answer = if answered {
            lines
                .iter()
                .filter(|(id, _)| cited.contains(id))
                .map(|(_, text)| text.as_str())
                .find(|text| !text.trim_end().ends_with('?'))
                .map(|text| format!("Answered. The transcript says: \"{}\"", text.trim()))
        } else if asked {
            Some("Asked, but no answer was found in the transcript.".to_string())
        } else {
            None
        };

        let status = if !asked {
            "not_asked"
        } else if answered {
            "asked_and_answered"
        } else {
            "asked_not_answered"
        };

        RawVerdict {
            status: status.to_string(),
            rating,
            extracted_answer,
            missing_details: None,
            criteria_addressed: vec![],
            criteria_total: vec![],
            confidence: Some((overlap as f32 / total as f32).min(1.0)),
            evidence_segment_ids: cited,
        }
    }
}

impl LlmClient for HeuristicLlm {
    fn complete(&self, _system: &str, user: &str, schema: &str) -> Result<String, String> {
        if schema == NOTES_SCHEMA {
            let text = between(user, DATA_OPEN, DATA_CLOSE).unwrap_or("");
            let summary = self.summary(text);
            return Ok(serde_json::json!({
                "summary": summary,
                "action_points": [],
                "next_steps": [],
                "actions_discussed": [],
            })
            .to_string());
        }
        let _ = VERDICT_SCHEMA;
        let verdict = self.verdict(user);
        serde_json::to_string(&verdict).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::parse_raw_verdict;

    fn user_with(question: &str, evidence: &str) -> String {
        format!(
            "QUESTION (main phrasing): {question}\n\nSample phrasings:\n(none)\n\nNo sample answers supplied.\n\nDATA:\n{DATA_OPEN}\n{evidence}\n{DATA_CLOSE}\n"
        )
    }

    #[test]
    fn detects_answered_question_and_cites_segment() {
        let user = user_with(
            "what is the salary and start date",
            "[t1] Dana: hello\n[t2] Dana: the base salary is 50k and the start date is Q3",
        );
        let json = HeuristicLlm.complete("", &user, VERDICT_SCHEMA).unwrap();
        let raw = parse_raw_verdict(&json).unwrap();
        assert_eq!(raw.status, "asked_and_answered");
        assert_eq!(raw.evidence_segment_ids, vec!["t2".to_string()]);
        assert!(raw.rating.is_some());
    }

    #[test]
    fn unrelated_evidence_is_not_asked() {
        let user = user_with(
            "are there relocation benefits",
            "[t1] Dana: we should grab lunch and talk coffee",
        );
        let json = HeuristicLlm.complete("", &user, VERDICT_SCHEMA).unwrap();
        let raw = parse_raw_verdict(&json).unwrap();
        assert_eq!(raw.status, "not_asked");
        assert!(raw.evidence_segment_ids.is_empty());
        assert!(raw.rating.is_none());
    }

    #[test]
    fn notes_schema_returns_a_general_summary() {
        let user = format!("Summarise.\n{DATA_OPEN}\nUser: hello there\nDana: the role pays 50k\n{DATA_CLOSE}\n");
        let json = HeuristicLlm.complete("", &user, NOTES_SCHEMA).unwrap();
        let notes = crate::schema::parse_notes(&json).unwrap();
        assert!(!notes.summary.is_empty(), "general summary must be present");
        assert!(notes.action_points.is_empty());
    }
}

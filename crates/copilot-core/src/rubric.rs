//! Rating rubric and verdict validation.
//!
//! The rubric is explicit and is applied *after* the model responds, as a
//! post-hoc check. This makes the rating reproducible and auditable even when
//! the model is unreliable.

use crate::model::{AnswerRating, QuestionStatus, RawVerdict, Verdict};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum VerdictError {
    #[error("rating must be NULL unless status == answered")]
    RatingWithoutAnswer,
    #[error("status == answered requires a rating")]
    MissingRatingForAnswer,
    #[error("every asked/answered verdict must cite at least one evidence segment id")]
    MissingEvidence,
    #[error("status string is not valid: {0}")]
    InvalidStatus(String),
    #[error("rating string is not valid: {0}")]
    InvalidRating(String),
    #[error("confidence out of range: {0}")]
    InvalidConfidence(f32),
    #[error("asked=false but answered=true is contradictory")]
    AnsweredWithoutAsked,
}

/// Parse the single status enum string onto `QuestionStatus`.
pub fn parse_status(status: &str) -> Result<QuestionStatus, VerdictError> {
    match status {
        "not_asked" => Ok(QuestionStatus::NotAsked),
        "asked_not_answered" => Ok(QuestionStatus::AskedNotAnswered),
        "asked_and_answered" | "answered" => Ok(QuestionStatus::Answered),
        other => Err(VerdictError::InvalidStatus(other.to_string())),
    }
}

/// Map `asked`/`answered` booleans onto the orthogonal status enum (retained for eval fixtures).
pub fn derive_status(asked: bool, answered: bool) -> Result<QuestionStatus, VerdictError> {
    match (asked, answered) {
        (false, true) => Err(VerdictError::AnsweredWithoutAsked),
        (false, false) => Ok(QuestionStatus::NotAsked),
        (true, false) => Ok(QuestionStatus::AskedNotAnswered),
        (true, true) => Ok(QuestionStatus::Answered),
    }
}

/// Grade a question that has `sample_answers` (an explicit rubric).
///
/// - `criteria_total` is the set of criteria derived from the sample answers.
/// - `criteria_addressed` is the subset the answer covers.
pub fn grade_with_criteria(
    criteria_addressed: &[String],
    criteria_total: &[String],
) -> AnswerRating {
    if criteria_total.is_empty() {
        // No criteria configured: cannot grade by coverage.
        return AnswerRating::Poor;
    }
    let total = criteria_total.len() as f32;
    let addressed = criteria_addressed
        .iter()
        .filter(|c| criteria_total.iter().any(|t| same_criterion(c, t)))
        .count() as f32;
    let ratio = addressed / total;
    if ratio >= 0.8 {
        AnswerRating::Strong
    } else if addressed >= 1.0 {
        AnswerRating::Partial
    } else {
        AnswerRating::Poor
    }
}

fn normalize_criterion(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn same_criterion(a: &str, b: &str) -> bool {
    normalize_criterion(a) == normalize_criterion(b)
}

/// Grade a question with no sample answers, from the model's completeness call.
pub fn grade_without_criteria(complete: bool, partial: bool) -> AnswerRating {
    if complete {
        AnswerRating::Strong
    } else if partial {
        AnswerRating::Partial
    } else {
        AnswerRating::Poor
    }
}

/// Validate a raw verdict and produce a canonical `Verdict`.
///
/// Enforces:
/// - status/rating orthogonality;
/// - evidence required for any asked/answered verdict;
/// - confidence clamped to 0..1;
/// - explicit rubric application when the model returned criteria counts.
pub fn validate(
    raw: &RawVerdict,
    criteria_total: &[String],
    has_sample_answers: bool,
) -> Result<Verdict, VerdictError> {
    let status = parse_status(&raw.status)?;

    let rating = match status {
        QuestionStatus::Answered => {
            if has_sample_answers && !criteria_total.is_empty() {
                // Explicit rubric: coverage decides the rating, not the model.
                Some(grade_with_criteria(&raw.criteria_addressed, criteria_total))
            } else {
                // No usable rubric: trust the model's rating but require it to be valid.
                match raw.rating.as_deref().and_then(AnswerRating::from_str) {
                    Some(r) => Some(r),
                    None => return Err(VerdictError::MissingRatingForAnswer),
                }
            }
        }
        _ => {
            if raw.rating.is_some() {
                return Err(VerdictError::RatingWithoutAnswer);
            }
            None
        }
    };

    let confidence = raw.confidence.unwrap_or(0.0);
    if !(0.0..=1.0).contains(&confidence) {
        return Err(VerdictError::InvalidConfidence(confidence));
    }

    if status != QuestionStatus::NotAsked && raw.evidence_segment_ids.is_empty() {
        return Err(VerdictError::MissingEvidence);
    }

    Ok(Verdict {
        status,
        rating,
        extracted_answer: raw.extracted_answer.clone(),
        missing_details: raw.missing_details.clone(),
        confidence,
        evidence_segment_ids: raw.evidence_segment_ids.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raw(status: &str) -> RawVerdict {
        RawVerdict {
            status: status.into(),
            rating: None,
            extracted_answer: None,
            missing_details: None,
            criteria_addressed: vec![],
            criteria_total: vec![],
            confidence: Some(0.7),
            evidence_segment_ids: vec!["t1".into()],
        }
    }

    #[test]
    fn rating_is_null_unless_answered() {
        let v = validate(&raw("asked_not_answered"), &[], false).unwrap();
        assert_eq!(v.status, QuestionStatus::AskedNotAnswered);
        assert_eq!(v.rating, None);
    }

    #[test]
    fn rating_present_when_answered_sets_rating() {
        let mut r = raw("asked_and_answered");
        r.rating = Some("Strong".into());
        let v = validate(&r, &[], false).unwrap();
        assert_eq!(v.status, QuestionStatus::Answered);
        assert_eq!(v.rating, Some(AnswerRating::Strong));
    }

    #[test]
    fn answered_without_rating_is_rejected() {
        assert_eq!(
            validate(&raw("asked_and_answered"), &[], false),
            Err(VerdictError::MissingRatingForAnswer)
        );
    }

    #[test]
    fn rating_without_answer_is_rejected() {
        let mut r = raw("asked_not_answered");
        r.rating = Some("Strong".into());
        assert_eq!(
            validate(&r, &[], false),
            Err(VerdictError::RatingWithoutAnswer)
        );
    }

    #[test]
    fn asked_verdict_requires_evidence() {
        let mut r = raw("asked_not_answered");
        r.evidence_segment_ids.clear();
        assert_eq!(validate(&r, &[], false), Err(VerdictError::MissingEvidence));
    }

    #[test]
    fn not_asked_needs_no_evidence() {
        let mut r = raw("not_asked");
        r.evidence_segment_ids.clear();
        r.rating = None;
        let v = validate(&r, &[], false).unwrap();
        assert_eq!(v.status, QuestionStatus::NotAsked);
    }

    #[test]
    fn criteria_rubric_boundaries() {
        let total = vec![
            "a".to_string(),
            "b".to_string(),
            "c".to_string(),
            "d".to_string(),
            "e".to_string(),
        ];
        assert_eq!(
            grade_with_criteria(
                &[
                    total[0].clone(),
                    total[1].clone(),
                    total[2].clone(),
                    total[3].clone()
                ],
                &total
            ),
            AnswerRating::Strong
        );
        assert_eq!(
            grade_with_criteria(&[total[0].clone()], &total),
            AnswerRating::Partial
        );
        assert_eq!(grade_with_criteria(&[], &total), AnswerRating::Poor);
    }

    #[test]
    fn criteria_used_when_sample_answers_present() {
        let mut r = raw("asked_and_answered");
        // Model claims Strong but addressed zero criteria -> Poor is enforced.
        r.rating = Some("Strong".into());
        let total = vec!["salary".to_string(), "start date".to_string()];
        let v = validate(&r, &total, true).unwrap();
        assert_eq!(v.rating, Some(AnswerRating::Poor));
    }

    #[test]
    fn confidence_out_of_range_rejected() {
        let mut r = raw("asked_and_answered");
        r.rating = Some("Poor".into());
        r.confidence = Some(1.5);
        assert_eq!(
            validate(&r, &[], false),
            Err(VerdictError::InvalidConfidence(1.5))
        );
    }

    #[test]
    fn invalid_status_enum_is_rejected() {
        assert_eq!(
            validate(&raw("answered_without_asked"), &[], false),
            Err(VerdictError::InvalidStatus("answered_without_asked".into()))
        );
    }
}

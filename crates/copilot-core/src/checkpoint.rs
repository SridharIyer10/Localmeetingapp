//! Analysis stage checkpointing and resumability.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessingStage {
    NotStarted,
    Transcribing,
    Indexing,
    EvaluatingQuestions,
    ExtractingNotes,
    Complete,
}

impl ProcessingStage {
    pub fn as_str(self) -> &'static str {
        match self {
            ProcessingStage::NotStarted => "not_started",
            ProcessingStage::Transcribing => "transcribing",
            ProcessingStage::Indexing => "indexing",
            ProcessingStage::EvaluatingQuestions => "evaluating_questions",
            ProcessingStage::ExtractingNotes => "extracting_notes",
            ProcessingStage::Complete => "complete",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "not_started" => Some(ProcessingStage::NotStarted),
            "transcribing" => Some(ProcessingStage::Transcribing),
            "indexing" => Some(ProcessingStage::Indexing),
            "evaluating_questions" => Some(ProcessingStage::EvaluatingQuestions),
            "extracting_notes" => Some(ProcessingStage::ExtractingNotes),
            "complete" => Some(ProcessingStage::Complete),
            _ => None,
        }
    }

    /// The stage that should run after this one.
    pub fn next(self) -> Option<ProcessingStage> {
        match self {
            ProcessingStage::NotStarted => Some(ProcessingStage::Indexing),
            ProcessingStage::Transcribing => Some(ProcessingStage::Indexing),
            ProcessingStage::Indexing => Some(ProcessingStage::EvaluatingQuestions),
            ProcessingStage::EvaluatingQuestions => Some(ProcessingStage::ExtractingNotes),
            ProcessingStage::ExtractingNotes => Some(ProcessingStage::Complete),
            ProcessingStage::Complete => None,
        }
    }

    /// Whether a checkpoint at this stage should skip work that already ran.
    pub fn is_resumable_from(self) -> bool {
        !matches!(
            self,
            ProcessingStage::Complete | ProcessingStage::NotStarted
        )
    }
}

/// Given a checkpoint, which stages still need to run (exclusive of the
/// checkpointed stage's own completion).
pub fn remaining_stages(from: ProcessingStage) -> Vec<ProcessingStage> {
    let mut out = Vec::new();
    let mut cur = from.next();
    while let Some(stage) = cur {
        out.push(stage);
        cur = stage.next();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_round_trips() {
        for s in [
            ProcessingStage::NotStarted,
            ProcessingStage::Transcribing,
            ProcessingStage::Indexing,
            ProcessingStage::EvaluatingQuestions,
            ProcessingStage::ExtractingNotes,
            ProcessingStage::Complete,
        ] {
            assert_eq!(ProcessingStage::from_str(s.as_str()), Some(s));
        }
    }

    #[test]
    fn resume_from_indexing_skips_indexing_itself() {
        let remaining = remaining_stages(ProcessingStage::Indexing);
        assert_eq!(
            remaining,
            vec![
                ProcessingStage::EvaluatingQuestions,
                ProcessingStage::ExtractingNotes,
                ProcessingStage::Complete
            ]
        );
    }

    #[test]
    fn complete_has_no_remaining_work() {
        assert!(remaining_stages(ProcessingStage::Complete).is_empty());
    }

    #[test]
    fn checkpoint_resumability() {
        assert!(!ProcessingStage::NotStarted.is_resumable_from());
        assert!(!ProcessingStage::Complete.is_resumable_from());
        assert!(ProcessingStage::EvaluatingQuestions.is_resumable_from());
    }
}

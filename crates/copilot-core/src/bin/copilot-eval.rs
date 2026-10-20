//! Evaluation harness for Intake Co-Pilot post-processing.
//!
//! Runs a fixture corpus of questions with human labels and emitted verdicts,
//! emits a machine-readable report, and optionally enforces release gates.
//!
//! Usage:
//!   copilot-eval <fixtures.json> [--degrade] [--gate]
//!
//! `--degrade` simulates k=1 retrieval by discarding every prediction whose
//! recorded `retrieval_rank` > 1. This proves the harness measures retrieval
//! quality rather than merely echoing labels.

use copilot_core::metrics::{cohens_kappa, detection, DetectionMetrics};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::process::ExitCode;

#[derive(Debug, Deserialize)]
struct Fixture {
    #[serde(default)]
    reference_hardware: String,
    #[serde(default)]
    models: BTreeMap<String, String>,
    cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct Case {
    id: String,
    #[serde(default)]
    question: String,
    expected: Labels,
    predicted: Predicted,
    #[serde(default)]
    evidence: EvidenceLabels,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct Labels {
    asked: bool,
    answered: bool,
    #[serde(default)]
    rating: Option<String>,
}

#[derive(Debug, Deserialize)]
struct Predicted {
    asked: bool,
    answered: bool,
    #[serde(default)]
    rating: Option<String>,
    #[serde(default)]
    retrieval_rank: u32,
}

#[derive(Debug, Deserialize, Default)]
struct EvidenceLabels {
    #[serde(default)]
    valid_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct Report {
    cases: usize,
    reference_hardware: String,
    models: BTreeMap<String, String>,
    asked_detection: DetectionMetrics,
    answered_detection: DetectionMetrics,
    status_accuracy: f32,
    rating_kappa: f32,
    rating_samples: usize,
    evidence_validity: f32,
    degraded: bool,
    failed_gates: Vec<String>,
}

fn status_of(asked: bool, answered: bool) -> &'static str {
    match (asked, answered) {
        (false, _) => "not_asked",
        (true, false) => "asked_not_answered",
        (true, true) => "answered",
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        eprintln!("usage: copilot-eval <fixtures.json> [--degrade] [--gate]");
        return ExitCode::from(2);
    }
    let path = &args[1];
    let degrade = args.iter().any(|a| a == "--degrade");
    let gate = args.iter().any(|a| a == "--gate");

    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("failed to read {path}: {e}");
            return ExitCode::from(2);
        }
    };
    let fixture: Fixture = match serde_json::from_str(&raw) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("failed to parse fixtures: {e}");
            return ExitCode::from(2);
        }
    };

    let mut asked = DetectionMetrics::default();
    let mut answered = DetectionMetrics::default();
    let mut status_expected: Vec<String> = Vec::new();
    let mut status_predicted: Vec<String> = Vec::new();
    let mut rating_expected: Vec<String> = Vec::new();
    let mut rating_predicted: Vec<String> = Vec::new();
    let mut evidence_total = 0usize;
    let mut evidence_valid = 0usize;

    for case in &fixture.cases {
        let predicted = if degrade && case.predicted.retrieval_rank > 1 {
            Labels {
                asked: false,
                answered: false,
                rating: None,
            }
        } else {
            Labels {
                asked: case.predicted.asked,
                answered: case.predicted.answered,
                rating: case.predicted.rating.clone(),
            }
        };

        detection(case.expected.asked, predicted.asked, &mut asked);
        detection(case.expected.answered, predicted.answered, &mut answered);

        status_expected.push(status_of(case.expected.asked, case.expected.answered).to_string());
        status_predicted.push(status_of(predicted.asked, predicted.answered).to_string());

        if case.expected.answered && predicted.answered {
            if let (Some(e), Some(p)) = (case.expected.rating.clone(), predicted.rating.clone()) {
                rating_expected.push(e);
                rating_predicted.push(p);
            }
        }

        if predicted.asked || predicted.answered {
            evidence_total += 1;
            if !case.evidence.valid_ids.is_empty() {
                evidence_valid += 1;
            }
        }
    }

    let status_correct = status_expected
        .iter()
        .zip(status_predicted.iter())
        .filter(|(a, b)| a == b)
        .count();
    let status_accuracy = if status_expected.is_empty() {
        1.0
    } else {
        status_correct as f32 / status_expected.len() as f32
    };

    let rating_kappa = cohens_kappa(&rating_expected, &rating_predicted);
    let evidence_validity = if evidence_total == 0 {
        1.0
    } else {
        evidence_valid as f32 / evidence_total as f32
    };

    let mut failed = Vec::new();
    check(&mut failed, "asked_recall", asked.recall(), 0.90);
    check(&mut failed, "asked_precision", asked.precision(), 0.85);
    check(&mut failed, "status_accuracy", status_accuracy, 0.85);
    check(&mut failed, "rating_kappa", rating_kappa, 0.60);
    check(&mut failed, "evidence_validity", evidence_validity, 1.0);

    let report = Report {
        cases: fixture.cases.len(),
        reference_hardware: fixture.reference_hardware.clone(),
        models: fixture.models.clone(),
        asked_detection: asked,
        answered_detection: answered,
        status_accuracy,
        rating_kappa,
        rating_samples: rating_expected.len(),
        evidence_validity,
        degraded: degrade,
        failed_gates: failed.clone(),
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());

    if gate && !failed.is_empty() {
        eprintln!("RELEASE GATE FAILED: {}", failed.join(", "));
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn check(failed: &mut Vec<String>, name: &str, value: f32, threshold: f32) {
    if value < threshold {
        failed.push(format!("{name}={value:.3} < {threshold:.2}"));
    }
}

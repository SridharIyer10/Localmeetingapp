//! Evaluation metrics for the post-processing harness.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct DetectionMetrics {
    pub true_positive: u32,
    pub false_positive: u32,
    pub true_negative: u32,
    pub false_negative: u32,
}

impl DetectionMetrics {
    pub fn precision(&self) -> f32 {
        let denom = self.true_positive + self.false_positive;
        if denom == 0 {
            1.0
        } else {
            self.true_positive as f32 / denom as f32
        }
    }

    pub fn recall(&self) -> f32 {
        let denom = self.true_positive + self.false_negative;
        if denom == 0 {
            1.0
        } else {
            self.true_positive as f32 / denom as f32
        }
    }

    pub fn f1(&self) -> f32 {
        let p = self.precision();
        let r = self.recall();
        if p + r == 0.0 {
            0.0
        } else {
            2.0 * p * r / (p + r)
        }
    }
}

pub fn detection(expected: bool, predicted: bool, m: &mut DetectionMetrics) {
    match (expected, predicted) {
        (true, true) => m.true_positive += 1,
        (true, false) => m.false_negative += 1,
        (false, true) => m.false_positive += 1,
        (false, false) => m.true_negative += 1,
    }
}

/// Cohen's kappa for two sets of categorical labels paired by index.
/// Returns 0.0 when there is no variation or no labels.
pub fn cohens_kappa(expected: &[String], predicted: &[String]) -> f32 {
    assert_eq!(
        expected.len(),
        predicted.len(),
        "label vectors must be aligned"
    );
    let n = expected.len();
    if n == 0 {
        return 0.0;
    }
    let mut observed = 0usize;
    let mut exp_counts: HashMap<&str, usize> = HashMap::new();
    let mut pred_counts: HashMap<&str, usize> = HashMap::new();
    for (e, p) in expected.iter().zip(predicted.iter()) {
        if e == p {
            observed += 1;
        }
        *exp_counts.entry(e.as_str()).or_insert(0) += 1;
        *pred_counts.entry(p.as_str()).or_insert(0) += 1;
    }
    let po = observed as f32 / n as f32;
    let pe: f32 = exp_counts
        .iter()
        .map(|(label, &c)| {
            let p = *pred_counts.get(label).unwrap_or(&0);
            (c as f32 / n as f32) * (p as f32 / n as f32)
        })
        .sum();
    if (1.0 - pe).abs() < 1e-9 {
        0.0
    } else {
        (po - pe) / (1.0 - pe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perfect_detection_metrics() {
        let mut m = DetectionMetrics::default();
        detection(true, true, &mut m);
        detection(false, false, &mut m);
        assert_eq!(m.precision(), 1.0);
        assert_eq!(m.recall(), 1.0);
        assert_eq!(m.f1(), 1.0);
    }

    #[test]
    fn detection_counts_correctly() {
        let mut m = DetectionMetrics::default();
        detection(true, false, &mut m); // FN
        detection(false, true, &mut m); // FP
        assert_eq!(m.false_negative, 1);
        assert_eq!(m.false_positive, 1);
        assert_eq!(m.recall(), 0.0);
        assert_eq!(m.precision(), 0.0);
    }

    #[test]
    fn kappa_perfect_agreement() {
        let labels = vec!["a".to_string(), "b".to_string(), "a".to_string()];
        assert!((cohens_kappa(&labels, &labels) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn kappa_chance_level_agreement_is_near_zero() {
        let expected = vec![
            "a".to_string(),
            "b".to_string(),
            "a".to_string(),
            "b".to_string(),
        ];
        let predicted = vec![
            "a".to_string(),
            "a".to_string(),
            "b".to_string(),
            "b".to_string(),
        ];
        let k = cohens_kappa(&expected, &predicted);
        assert!(k.abs() < 0.5);
    }

    #[test]
    fn kappa_no_variation_is_zero() {
        let expected = vec!["a".to_string(), "a".to_string()];
        let predicted = vec!["a".to_string(), "a".to_string()];
        assert_eq!(cohens_kappa(&expected, &predicted), 0.0);
    }
}

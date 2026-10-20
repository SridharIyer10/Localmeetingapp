//! Hybrid retrieval: dense cosine + lexical BM25.
//!
//! Dense vectors blur exactly the tokens that disambiguate intake questions
//! ("Q3", "$50k", "Workday", "P2"). We therefore blend cosine with BM25 at the
//! window level. The corpus is small (a few hundred windows), so brute-force
//! scoring is correct; no vector database is used.

use std::collections::HashMap;

const BM25_K1: f32 = 1.2;
const BM25_B: f32 = 0.75;

/// Lowercase alphanumeric tokens, preserving internal digits and punctuation
/// inside tokens (e.g. "q3", "$50k", "workday", "p2").
pub fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| {
            !(c.is_alphanumeric()
                || c == '$'
                || c == '%'
                || c == '#'
                || c == '-'
                || c == '_'
                || c == '+'
                || c == '.')
        })
        .filter(|t| !t.is_empty() && t.chars().any(|c| c.is_alphanumeric()))
        .map(|t| {
            t.trim_matches(|c| c == '$' || c == '%' || c == '.')
                .to_string()
        })
        .filter(|t| !t.is_empty())
        .collect()
}

/// A BM25 index over a fixed corpus of documents.
pub struct Bm25Index {
    doc_len: Vec<usize>,
    avgdl: f32,
    term_freqs: Vec<HashMap<String, usize>>,
    doc_freq: HashMap<String, usize>,
    n_docs: usize,
}

impl Bm25Index {
    pub fn new(docs: &[String]) -> Self {
        let doc_len: Vec<usize> = docs.iter().map(|d| tokenize(d).len()).collect();
        let avgdl = if doc_len.is_empty() {
            0.0
        } else {
            doc_len.iter().sum::<usize>() as f32 / doc_len.len() as f32
        };
        let term_freqs: Vec<HashMap<String, usize>> = docs
            .iter()
            .map(|d| {
                let mut tf = HashMap::new();
                for tok in tokenize(d) {
                    *tf.entry(tok).or_insert(0) += 1;
                }
                tf
            })
            .collect();
        let mut doc_freq: HashMap<String, usize> = HashMap::new();
        for tf in &term_freqs {
            for term in tf.keys() {
                *doc_freq.entry(term.clone()).or_insert(0) += 1;
            }
        }
        Self {
            doc_len,
            avgdl,
            term_freqs,
            doc_freq,
            n_docs: docs.len(),
        }
    }

    pub fn len(&self) -> usize {
        self.n_docs
    }

    pub fn is_empty(&self) -> bool {
        self.n_docs == 0
    }

    fn idf(&self, term: &str) -> f32 {
        let df = *self.doc_freq.get(term).unwrap_or(&0) as f32;
        let n = self.n_docs as f32;
        // BM25+ style idf, always non-negative.
        ((n - df + 0.5) / (df + 0.5) + 1.0).ln()
    }

    /// Score every document against a query. Returns scores in document order.
    pub fn score(&self, query: &str) -> Vec<f32> {
        let query_terms = tokenize(query);
        let mut scores = vec![0.0f32; self.n_docs];
        if self.n_docs == 0 || query_terms.is_empty() {
            return scores;
        }
        for term in &query_terms {
            let idf = self.idf(term);
            for (doc_idx, tf) in self.term_freqs.iter().enumerate() {
                let f = *tf.get(term).unwrap_or(&0) as f32;
                if f == 0.0 {
                    continue;
                }
                let dl = self.doc_len[doc_idx] as f32;
                let denom = f + BM25_K1 * (1.0 - BM25_B + BM25_B * dl / self.avgdl.max(1.0));
                scores[doc_idx] += idf * (f * (BM25_K1 + 1.0)) / denom.max(1e-6);
            }
        }
        scores
    }
}

/// Cosine similarity between two vectors. Zero-norm vectors score 0.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    (dot / (na.sqrt() * nb.sqrt())).clamp(-1.0, 1.0)
}

/// Normalize a vector to unit length (no-op for zero vectors).
pub fn normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

fn min_max_normalize(scores: &[f32]) -> Vec<f32> {
    if scores.is_empty() {
        return Vec::new();
    }
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;
    for &s in scores {
        min = min.min(s);
        max = max.max(s);
    }
    let range = max - min;
    if range <= 1e-9 {
        return vec![0.0; scores.len()];
    }
    scores.iter().map(|&s| (s - min) / range).collect()
}

/// Retrieve the top-k document indices for a query.
///
/// `alpha` weights the dense score (1.0 = pure cosine, 0.0 = pure BM25).
/// Scores are min-max normalized before blending so the scales are comparable.
pub fn hybrid_retrieve(
    query: &str,
    query_embedding: &[f32],
    doc_embeddings: &[Vec<f32>],
    bm25: &Bm25Index,
    alpha: f32,
    top_k: usize,
) -> Vec<(usize, f32)> {
    if doc_embeddings.is_empty() {
        return Vec::new();
    }
    let dense: Vec<f32> = doc_embeddings
        .iter()
        .map(|d| cosine(query_embedding, d))
        .collect();
    let lexical = bm25.score(query);
    let dense_n = min_max_normalize(&dense);
    let lexical_n = min_max_normalize(&lexical);

    let alpha = alpha.clamp(0.0, 1.0);
    let mut blended: Vec<(usize, f32)> = (0..doc_embeddings.len())
        .map(|i| {
            let s = alpha * dense_n.get(i).copied().unwrap_or(0.0)
                + (1.0 - alpha) * lexical_n.get(i).copied().unwrap_or(0.0);
            (i, s)
        })
        .collect();

    blended.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    blended.truncate(top_k);
    blended
}

#[cfg(test)]
mod tests {
    use super::*;

    fn emb(v: &[f32]) -> Vec<f32> {
        v.to_vec()
    }

    #[test]
    fn tokenize_preserves_disambiguating_tokens() {
        let toks = tokenize("Approved by Q3, pending CFO for $50k on Workday P2.");
        for expected in ["q3", "cfo", "50k", "workday", "p2"] {
            assert!(
                toks.contains(&expected.to_string()),
                "missing {expected} in {toks:?}"
            );
        }
    }

    #[test]
    fn bm25_ranks_exact_term_matches_above_others() {
        let docs = vec![
            "we discussed the weather and lunch".to_string(),
            "the salary band is 50k for this role".to_string(),
            "q3 headcount plan was approved".to_string(),
        ];
        let index = Bm25Index::new(&docs);
        let scores = index.score("q3 headcount");
        assert!(scores[2] > scores[0]);
        assert!(scores[2] > scores[1]);
    }

    #[test]
    fn cosine_handles_zero_vectors_and_mismatched_lengths() {
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
        assert_eq!(cosine(&[1.0], &[1.0, 2.0]), 0.0);
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hybrid_retrieval_finds_token_specific_window() {
        let docs = vec![
            "Participant: we talked about onboarding".to_string(),
            "Participant: the offer is $50k and starts in Q3".to_string(),
            "Participant: coffee preferences".to_string(),
        ];
        // Dense embeddings intentionally unhelpful (all similar).
        let doc_emb = vec![emb(&[1.0, 0.0]), emb(&[1.0, 0.0]), emb(&[1.0, 0.0])];
        let bm25 = Bm25Index::new(&docs);
        let results = hybrid_retrieve(
            "what is the salary and start date",
            &emb(&[1.0, 0.0]),
            &doc_emb,
            &bm25,
            0.3,
            2,
        );
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn hybrid_retrieval_empty_corpus_is_empty() {
        let bm25 = Bm25Index::new(&[]);
        assert!(hybrid_retrieve("q", &[], &[], &bm25, 0.5, 5).is_empty());
    }
}

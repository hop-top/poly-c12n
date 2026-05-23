use std::collections::HashMap;

use async_trait::async_trait;
use regex::Regex;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum MatchOperator {
    And,
    Or,
    Nor,
}

#[derive(Debug, Clone)]
pub enum MatchStrategy {
    Regex,
    Bm25,
    Trigram,
    Fuzzy(usize),
}

// ---------------------------------------------------------------------------
// KeywordRule
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct KeywordRule {
    pub label: String,
    pub patterns: Vec<String>,
    pub operator: MatchOperator,
    pub strategy: MatchStrategy,
    pub threshold: f64,
}

// ---------------------------------------------------------------------------
// BM25 config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Bm25Config {
    pub k1: f64,
    pub b: f64,
    pub corpus: Vec<String>,
}

impl Default for Bm25Config {
    fn default() -> Self {
        Self {
            k1: 1.2,
            b: 0.75,
            corpus: Vec::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// KeywordSignal
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct KeywordSignal {
    name: String,
    rules: Vec<KeywordRule>,
    bm25: Bm25Config,
}

impl KeywordSignal {
    pub fn new(name: impl Into<String>, rules: Vec<KeywordRule>) -> Self {
        Self {
            name: name.into(),
            rules,
            bm25: Bm25Config::default(),
        }
    }

    pub fn with_bm25_config(mut self, cfg: Bm25Config) -> Self {
        self.bm25 = cfg;
        self
    }

    // -- strategy dispatchers -----------------------------------------------

    fn score_rule(&self, text: &str, rule: &KeywordRule) -> Result<f64, SignalError> {
        let scores: Vec<f64> = rule
            .patterns
            .iter()
            .map(|p| self.score_pattern(text, p, &rule.strategy))
            .collect::<Result<_, _>>()?;

        let combined = match rule.operator {
            MatchOperator::Or => scores.iter().cloned().fold(0.0_f64, f64::max),
            MatchOperator::And => {
                if scores.iter().all(|&s| s >= rule.threshold) {
                    scores.iter().cloned().fold(f64::MAX, f64::min)
                } else {
                    0.0
                }
            }
            MatchOperator::Nor => {
                if scores.iter().all(|&s| s < rule.threshold) {
                    1.0
                } else {
                    0.0
                }
            }
        };

        Ok(combined)
    }

    fn score_pattern(
        &self,
        text: &str,
        pattern: &str,
        strategy: &MatchStrategy,
    ) -> Result<f64, SignalError> {
        match strategy {
            MatchStrategy::Regex => Self::score_regex(text, pattern),
            MatchStrategy::Bm25 => self.score_bm25(text, pattern),
            MatchStrategy::Trigram => Ok(Self::score_trigram(text, pattern)),
            MatchStrategy::Fuzzy(max_dist) => Ok(Self::score_fuzzy(text, pattern, *max_dist)),
        }
    }

    // -- Regex --------------------------------------------------------------

    fn score_regex(text: &str, pattern: &str) -> Result<f64, SignalError> {
        let re = Regex::new(pattern).map_err(|e| {
            SignalError::Configuration(format!("invalid regex '{}': {}", pattern, e))
        })?;
        Ok(if re.is_match(text) { 1.0 } else { 0.0 })
    }

    // -- BM25 TF-IDF -------------------------------------------------------

    fn score_bm25(&self, text: &str, term: &str) -> Result<f64, SignalError> {
        let term_lower = term.to_lowercase();
        let words = Self::tokenize(text);
        let doc_len = words.len() as f64;

        if doc_len == 0.0 {
            return Ok(0.0);
        }

        let tf = words.iter().filter(|w| *w == &term_lower).count() as f64;
        if tf == 0.0 {
            return Ok(0.0);
        }

        let n = self.bm25.corpus.len().max(1) as f64;
        let df = self
            .bm25
            .corpus
            .iter()
            .filter(|doc| Self::tokenize(doc).iter().any(|w| w == &term_lower))
            .count() as f64;

        let avg_dl = if self.bm25.corpus.is_empty() {
            doc_len
        } else {
            self.bm25
                .corpus
                .iter()
                .map(|d| Self::tokenize(d).len() as f64)
                .sum::<f64>()
                / n
        };

        let idf = ((n - df + 0.5) / (df + 0.5) + 1.0).ln();
        let k1 = self.bm25.k1;
        let b = self.bm25.b;
        let tf_norm = (tf * (k1 + 1.0)) / (tf + k1 * (1.0 - b + b * doc_len / avg_dl));

        let score = idf * tf_norm;
        // Normalise to 0..1 with a sigmoid-style clamp.
        Ok(score.max(0.0).min(1.0))
    }

    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect()
    }

    // -- Trigram ------------------------------------------------------------

    fn score_trigram(text: &str, pattern: &str) -> f64 {
        let text_tris = Self::trigrams(&text.to_lowercase());
        let pat_tris = Self::trigrams(&pattern.to_lowercase());

        if pat_tris.is_empty() {
            return 0.0;
        }

        let intersection = pat_tris.iter().filter(|t| text_tris.contains(t)).count() as f64;
        let union = text_tris.len().max(pat_tris.len()) as f64;

        if union == 0.0 {
            0.0
        } else {
            intersection / union
        }
    }

    fn trigrams(s: &str) -> Vec<String> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() < 3 {
            return vec![];
        }
        chars.windows(3).map(|w| w.iter().collect()).collect()
    }

    // -- Fuzzy (Levenshtein) ------------------------------------------------

    fn score_fuzzy(text: &str, pattern: &str, max_dist: usize) -> f64 {
        let text_lower = text.to_lowercase();
        let pat_lower = pattern.to_lowercase();
        let pat_words: Vec<&str> = pat_lower.split_whitespace().collect();
        let text_words: Vec<&str> = text_lower.split_whitespace().collect();

        // For each pattern word, find best (lowest) edit distance in text.
        let best: Option<usize> = pat_words
            .iter()
            .map(|pw| {
                text_words
                    .iter()
                    .map(|tw| Self::levenshtein(pw, tw))
                    .min()
                    .unwrap_or(usize::MAX)
            })
            .max(); // worst match across pattern words

        match best {
            Some(d) if d <= max_dist => {
                if max_dist == 0 {
                    1.0
                } else {
                    1.0 - (d as f64 / (max_dist as f64 + 1.0))
                }
            }
            _ => 0.0,
        }
    }

    fn levenshtein(a: &str, b: &str) -> usize {
        let a_chars: Vec<char> = a.chars().collect();
        let b_chars: Vec<char> = b.chars().collect();
        let (m, n) = (a_chars.len(), b_chars.len());

        let mut prev: Vec<usize> = (0..=n).collect();
        let mut curr = vec![0; n + 1];

        for i in 1..=m {
            curr[0] = i;
            for j in 1..=n {
                let cost = if a_chars[i - 1] == b_chars[j - 1] {
                    0
                } else {
                    1
                };
                curr[j] = (prev[j] + 1)
                    .min(curr[j - 1] + 1)
                    .min(prev[j - 1] + cost);
            }
            std::mem::swap(&mut prev, &mut curr);
        }
        prev[n]
    }
}

// ---------------------------------------------------------------------------
// Signal trait
// ---------------------------------------------------------------------------

#[async_trait]
impl Signal for KeywordSignal {
    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Keyword
    }

    async fn evaluate(
        &self,
        ctx: &ClassificationContext,
    ) -> Result<SignalResult, SignalError> {
        for rule in &self.rules {
            let score = self.score_rule(&ctx.text, rule)?;
            if score >= rule.threshold {
                return Ok(SignalResult {
                    name: self.name.clone(),
                    signal_type: SignalType::Keyword,
                    confidence: score,
                    labels: vec![rule.label.clone()],
                    metadata: HashMap::new(),
                });
            }
        }

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: SignalType::Keyword,
            confidence: 0.0,
            labels: vec![],
            metadata: HashMap::new(),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx(text: &str) -> ClassificationContext {
        ClassificationContext {
            text: text.to_string(),
            history: vec![],
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    // -- Regex --

    #[tokio::test]
    async fn regex_match_found() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "greeting".into(),
                patterns: vec!["hello".into()],
                operator: MatchOperator::Or,
                strategy: MatchStrategy::Regex,
                threshold: 0.5,
            }],
        );
        let r = sig.evaluate(&ctx("say hello world")).await.unwrap();
        assert_eq!(r.confidence, 1.0);
        assert_eq!(r.labels, vec!["greeting"]);
    }

    #[tokio::test]
    async fn regex_no_match() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "greeting".into(),
                patterns: vec!["hello".into()],
                operator: MatchOperator::Or,
                strategy: MatchStrategy::Regex,
                threshold: 0.5,
            }],
        );
        let r = sig.evaluate(&ctx("goodbye world")).await.unwrap();
        assert_eq!(r.confidence, 0.0);
        assert!(r.labels.is_empty());
    }

    // -- BM25 --

    #[tokio::test]
    async fn bm25_scores_present_term() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "topic".into(),
                patterns: vec!["rust".into()],
                operator: MatchOperator::Or,
                strategy: MatchStrategy::Bm25,
                threshold: 0.1,
            }],
        )
        .with_bm25_config(Bm25Config {
            k1: 1.2,
            b: 0.75,
            corpus: vec![
                "rust is great for systems programming".into(),
                "python is popular for scripting".into(),
                "java runs everywhere".into(),
            ],
        });
        let r = sig
            .evaluate(&ctx("rust is a systems language rust"))
            .await
            .unwrap();
        assert!(r.confidence > 0.0, "confidence should be > 0");
        assert_eq!(r.labels, vec!["topic"]);
    }

    #[tokio::test]
    async fn bm25_zero_for_absent_term() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "topic".into(),
                patterns: vec!["haskell".into()],
                operator: MatchOperator::Or,
                strategy: MatchStrategy::Bm25,
                threshold: 0.1,
            }],
        )
        .with_bm25_config(Bm25Config::default());
        let r = sig.evaluate(&ctx("rust is great")).await.unwrap();
        assert_eq!(r.confidence, 0.0);
    }

    // -- Trigram --

    #[tokio::test]
    async fn trigram_similar_strings() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "sim".into(),
                patterns: vec!["classification".into()],
                operator: MatchOperator::Or,
                strategy: MatchStrategy::Trigram,
                threshold: 0.3,
            }],
        );
        let r = sig
            .evaluate(&ctx("the classification system"))
            .await
            .unwrap();
        assert!(
            r.confidence >= 0.3,
            "trigram confidence {} should be >= 0.3",
            r.confidence
        );
    }

    #[tokio::test]
    async fn trigram_dissimilar_strings() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "sim".into(),
                patterns: vec!["xyz".into()],
                operator: MatchOperator::Or,
                strategy: MatchStrategy::Trigram,
                threshold: 0.5,
            }],
        );
        let r = sig.evaluate(&ctx("abcdef")).await.unwrap();
        assert!(
            r.confidence < 0.5,
            "trigram confidence {} should be < 0.5",
            r.confidence
        );
    }

    // -- Fuzzy --

    #[tokio::test]
    async fn fuzzy_close_match() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "typo".into(),
                patterns: vec!["hello".into()],
                operator: MatchOperator::Or,
                strategy: MatchStrategy::Fuzzy(2),
                threshold: 0.3,
            }],
        );
        let r = sig.evaluate(&ctx("helo world")).await.unwrap();
        assert!(
            r.confidence >= 0.3,
            "fuzzy confidence {} should be >= 0.3",
            r.confidence
        );
    }

    #[tokio::test]
    async fn fuzzy_too_far() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "typo".into(),
                patterns: vec!["hello".into()],
                operator: MatchOperator::Or,
                strategy: MatchStrategy::Fuzzy(1),
                threshold: 0.3,
            }],
        );
        let r = sig.evaluate(&ctx("xyzzz world")).await.unwrap();
        assert_eq!(r.confidence, 0.0);
    }

    // -- Operators --

    #[tokio::test]
    async fn and_operator_all_match() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "both".into(),
                patterns: vec!["hello".into(), "world".into()],
                operator: MatchOperator::And,
                strategy: MatchStrategy::Regex,
                threshold: 0.5,
            }],
        );
        let r = sig.evaluate(&ctx("hello world")).await.unwrap();
        assert_eq!(r.confidence, 1.0);
        assert_eq!(r.labels, vec!["both"]);
    }

    #[tokio::test]
    async fn and_operator_partial_match() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "both".into(),
                patterns: vec!["hello".into(), "missing".into()],
                operator: MatchOperator::And,
                strategy: MatchStrategy::Regex,
                threshold: 0.5,
            }],
        );
        let r = sig.evaluate(&ctx("hello world")).await.unwrap();
        assert_eq!(r.confidence, 0.0);
    }

    #[tokio::test]
    async fn nor_operator_none_present() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "safe".into(),
                patterns: vec!["bad".into(), "evil".into()],
                operator: MatchOperator::Nor,
                strategy: MatchStrategy::Regex,
                threshold: 0.5,
            }],
        );
        let r = sig.evaluate(&ctx("good content")).await.unwrap();
        assert_eq!(r.confidence, 1.0);
        assert_eq!(r.labels, vec!["safe"]);
    }

    #[tokio::test]
    async fn nor_operator_one_present() {
        let sig = KeywordSignal::new(
            "test",
            vec![KeywordRule {
                label: "safe".into(),
                patterns: vec!["bad".into(), "evil".into()],
                operator: MatchOperator::Nor,
                strategy: MatchStrategy::Regex,
                threshold: 0.5,
            }],
        );
        let r = sig.evaluate(&ctx("this is bad")).await.unwrap();
        assert_eq!(r.confidence, 0.0);
    }

    // -- First-match semantics --

    #[tokio::test]
    async fn first_matching_rule_wins() {
        let sig = KeywordSignal::new(
            "test",
            vec![
                KeywordRule {
                    label: "first".into(),
                    patterns: vec!["hello".into()],
                    operator: MatchOperator::Or,
                    strategy: MatchStrategy::Regex,
                    threshold: 0.5,
                },
                KeywordRule {
                    label: "second".into(),
                    patterns: vec!["hello".into()],
                    operator: MatchOperator::Or,
                    strategy: MatchStrategy::Regex,
                    threshold: 0.5,
                },
            ],
        );
        let r = sig.evaluate(&ctx("hello")).await.unwrap();
        assert_eq!(r.labels, vec!["first"]);
    }
}

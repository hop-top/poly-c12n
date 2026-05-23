use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

// ---------------------------------------------------------------------------
// MMLU-Pro categories
// ---------------------------------------------------------------------------

pub const CATEGORIES: &[&str] = &[
    "Math",
    "Physics",
    "Chemistry",
    "Biology",
    "Computer Science",
    "Engineering",
    "Medicine",
    "Law",
    "Economics",
    "Psychology",
    "Philosophy",
    "History",
    "Business",
    "Health",
    "Other",
];

// ---------------------------------------------------------------------------
// CategoryClassifier trait
// ---------------------------------------------------------------------------

/// Abstraction over the underlying classification model.
/// Implementations may range from keyword heuristics to BERT inference.
#[async_trait]
pub trait CategoryClassifier: Send + Sync {
    /// Returns `(category_name, probability)` pairs sorted descending by
    /// probability. Probabilities should sum to ~1.0.
    async fn classify(
        &self,
        text: &str,
    ) -> Result<Vec<(String, f64)>, SignalError>;
}

// ---------------------------------------------------------------------------
// DomainSignal
// ---------------------------------------------------------------------------

pub struct DomainSignal {
    name: String,
    classifier: Arc<dyn CategoryClassifier>,
    /// Shannon entropy below this threshold => single confident label.
    entropy_threshold: f64,
    /// Ignore categories with probability below this value.
    min_probability: f64,
}

impl DomainSignal {
    pub fn new(
        name: impl Into<String>,
        classifier: Arc<dyn CategoryClassifier>,
        entropy_threshold: f64,
        min_probability: f64,
    ) -> Self {
        Self {
            name: name.into(),
            classifier,
            entropy_threshold,
            min_probability,
        }
    }

    /// Shannon entropy: -sum(p * ln(p)) over non-zero probabilities.
    fn shannon_entropy(probs: &[(String, f64)]) -> f64 {
        let mut h = 0.0_f64;
        for (_, p) in probs {
            if *p > 0.0 {
                h -= p * p.ln();
            }
        }
        h
    }

    /// Maximum possible entropy for `n` categories (uniform distribution).
    fn max_entropy(n: usize) -> f64 {
        if n <= 1 {
            return 0.0;
        }
        (n as f64).ln()
    }
}

#[async_trait]
impl Signal for DomainSignal {
    async fn evaluate(
        &self,
        ctx: &ClassificationContext,
    ) -> Result<SignalResult, SignalError> {
        if ctx.text.is_empty() {
            return Err(SignalError::InvalidInput(
                "empty text".into(),
            ));
        }

        let distribution = self.classifier.classify(&ctx.text).await?;
        if distribution.is_empty() {
            return Err(SignalError::Inference(
                "classifier returned empty distribution".into(),
            ));
        }

        let entropy = Self::shannon_entropy(&distribution);
        let max_ent = Self::max_entropy(distribution.len());
        let normalized_entropy = if max_ent > 0.0 {
            entropy / max_ent
        } else {
            0.0
        };

        let (labels, confidence) = if entropy < self.entropy_threshold {
            // Confident single category: use top-1.
            let top = &distribution[0];
            (vec![top.0.clone()], top.1)
        } else {
            // Multi-label: all categories above min_probability.
            let above: Vec<String> = distribution
                .iter()
                .filter(|(_, p)| *p >= self.min_probability)
                .map(|(c, _)| c.clone())
                .collect();
            let labels = if above.is_empty() {
                vec![distribution[0].0.clone()]
            } else {
                above
            };
            (labels, 1.0 - normalized_entropy)
        };

        // Metadata
        let mut metadata = HashMap::new();
        metadata.insert(
            "entropy".into(),
            serde_json::json!(entropy),
        );
        metadata.insert(
            "normalized_entropy".into(),
            serde_json::json!(normalized_entropy),
        );

        let dist_map: HashMap<String, f64> =
            distribution.into_iter().collect();
        metadata.insert(
            "distribution".into(),
            serde_json::to_value(dist_map)
                .unwrap_or(serde_json::Value::Null),
        );

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: SignalType::Domain,
            confidence,
            labels,
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Domain
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Mock classifier --------------------------------------------------

    struct MockClassifier {
        /// Pre-canned distribution returned for any input.
        distribution: Vec<(String, f64)>,
    }

    impl MockClassifier {
        fn confident() -> Self {
            Self {
                distribution: vec![
                    ("Math".into(), 0.85),
                    ("Physics".into(), 0.10),
                    ("Other".into(), 0.05),
                ],
            }
        }

        fn ambiguous() -> Self {
            Self {
                distribution: vec![
                    ("Biology".into(), 0.30),
                    ("Chemistry".into(), 0.28),
                    ("Medicine".into(), 0.22),
                    ("Health".into(), 0.20),
                ],
            }
        }

        fn uniform(n: usize) -> Self {
            let p = 1.0 / n as f64;
            let distribution: Vec<(String, f64)> = CATEGORIES
                .iter()
                .take(n)
                .map(|c| (c.to_string(), p))
                .collect();
            Self { distribution }
        }

        fn empty() -> Self {
            Self {
                distribution: vec![],
            }
        }

        fn single() -> Self {
            Self {
                distribution: vec![("Law".into(), 1.0)],
            }
        }
    }

    #[async_trait]
    impl CategoryClassifier for MockClassifier {
        async fn classify(
            &self,
            _text: &str,
        ) -> Result<Vec<(String, f64)>, SignalError> {
            Ok(self.distribution.clone())
        }
    }

    fn make_ctx(text: &str) -> ClassificationContext {
        ClassificationContext {
            text: text.into(),
            history: vec![],
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    // -- Tests ------------------------------------------------------------

    #[tokio::test]
    async fn confident_single_label() {
        let sig = DomainSignal::new(
            "domain",
            Arc::new(MockClassifier::confident()),
            1.0,
            0.10,
        );
        let result = sig
            .evaluate(&make_ctx("solve x^2 + 1 = 0"))
            .await
            .unwrap();

        assert_eq!(result.labels, vec!["Math"]);
        assert!((result.confidence - 0.85).abs() < 1e-9);
        assert_eq!(result.signal_type, SignalType::Domain);
        assert!(result.metadata.contains_key("entropy"));
        assert!(result.metadata.contains_key("distribution"));
    }

    #[tokio::test]
    async fn ambiguous_multi_label() {
        let sig = DomainSignal::new(
            "domain",
            Arc::new(MockClassifier::ambiguous()),
            0.1, // low threshold: most distributions exceed it
            0.10,
        );
        let result = sig
            .evaluate(&make_ctx("cell metabolism and drug interactions"))
            .await
            .unwrap();

        assert!(result.labels.len() > 1);
        assert!(result.labels.contains(&"Biology".into()));
        assert!(result.labels.contains(&"Chemistry".into()));
        assert!(result.confidence > 0.0);
        assert!(result.confidence < 1.0);
    }

    #[tokio::test]
    async fn min_probability_filters() {
        let sig = DomainSignal::new(
            "domain",
            Arc::new(MockClassifier::ambiguous()),
            0.1,  // force multi-label path
            0.25, // only Bio + Chem pass
        );
        let result = sig
            .evaluate(&make_ctx("enzyme kinetics in organic chemistry"))
            .await
            .unwrap();

        assert_eq!(result.labels.len(), 2);
        assert!(result.labels.contains(&"Biology".into()));
        assert!(result.labels.contains(&"Chemistry".into()));
    }

    #[tokio::test]
    async fn single_category_distribution() {
        let sig = DomainSignal::new(
            "domain",
            Arc::new(MockClassifier::single()),
            1.0,
            0.10,
        );
        let result = sig
            .evaluate(&make_ctx("legal precedent"))
            .await
            .unwrap();

        assert_eq!(result.labels, vec!["Law"]);
        assert!((result.confidence - 1.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn uniform_distribution_high_entropy() {
        let sig = DomainSignal::new(
            "domain",
            Arc::new(MockClassifier::uniform(5)),
            0.1, // low: multi-label
            0.10,
        );
        let result = sig
            .evaluate(&make_ctx("random noise"))
            .await
            .unwrap();

        assert_eq!(result.labels.len(), 5);
        // Confidence should be near zero for max entropy.
        assert!(result.confidence < 0.1);
    }

    #[tokio::test]
    async fn empty_distribution_errors() {
        let sig = DomainSignal::new(
            "domain",
            Arc::new(MockClassifier::empty()),
            1.0,
            0.10,
        );
        let err = sig
            .evaluate(&make_ctx("anything"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("empty distribution"));
    }

    #[tokio::test]
    async fn empty_text_errors() {
        let sig = DomainSignal::new(
            "domain",
            Arc::new(MockClassifier::confident()),
            1.0,
            0.10,
        );
        let err = sig.evaluate(&make_ctx("")).await.unwrap_err();
        assert!(err.to_string().contains("empty text"));
    }

    #[tokio::test]
    async fn metadata_contains_entropy() {
        let sig = DomainSignal::new(
            "domain",
            Arc::new(MockClassifier::confident()),
            1.0,
            0.10,
        );
        let result = sig
            .evaluate(&make_ctx("calculus"))
            .await
            .unwrap();

        let entropy = result.metadata["entropy"].as_f64().unwrap();
        assert!(entropy >= 0.0);

        let norm = result.metadata["normalized_entropy"]
            .as_f64()
            .unwrap();
        assert!(norm >= 0.0);
        assert!(norm <= 1.0);
    }

    #[test]
    fn shannon_entropy_deterministic() {
        let probs =
            vec![("A".into(), 0.5), ("B".into(), 0.5)];
        let h = DomainSignal::shannon_entropy(&probs);
        let expected = (2.0_f64).ln(); // ln(2)
        assert!((h - expected).abs() < 1e-9);
    }

    #[test]
    fn shannon_entropy_zero_for_certain() {
        let probs = vec![("A".into(), 1.0)];
        let h = DomainSignal::shannon_entropy(&probs);
        assert!(h.abs() < 1e-12);
    }

    #[test]
    fn max_entropy_edge_cases() {
        assert_eq!(DomainSignal::max_entropy(0), 0.0);
        assert_eq!(DomainSignal::max_entropy(1), 0.0);
        assert!(
            (DomainSignal::max_entropy(2) - (2.0_f64).ln()).abs()
                < 1e-9
        );
    }

    #[test]
    fn categories_has_at_least_15() {
        assert!(CATEGORIES.len() >= 15);
    }

    #[test]
    fn trait_object_name_and_type() {
        let sig = DomainSignal::new(
            "test-domain",
            Arc::new(MockClassifier::confident()),
            1.0,
            0.10,
        );
        assert_eq!(sig.name(), "test-domain");
        assert_eq!(sig.signal_type(), SignalType::Domain);
    }
}

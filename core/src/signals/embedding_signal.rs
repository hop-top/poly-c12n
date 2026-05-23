use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::embedding::EmbeddingEngine;
use crate::prototype::PrototypeBank;
use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

pub struct EmbeddingRule {
    pub label: String,
    pub bank: PrototypeBank,
    pub threshold: f64,
    pub top_k: usize,
}

pub struct EmbeddingSignalConfig {
    pub name: String,
    pub rules: Vec<EmbeddingRule>,
}

pub struct EmbeddingSignal {
    config: EmbeddingSignalConfig,
    engine: Arc<dyn EmbeddingEngine>,
}

impl EmbeddingSignal {
    pub fn new(config: EmbeddingSignalConfig, engine: Arc<dyn EmbeddingEngine>) -> Self {
        Self { config, engine }
    }
}

#[async_trait]
impl Signal for EmbeddingSignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        if ctx.text.is_empty() {
            return Err(SignalError::InvalidInput("empty input text".to_string()));
        }

        let embedding = self
            .engine
            .embed(&ctx.text)
            .await
            .map_err(|e| SignalError::Inference(e.to_string()))?;

        // Score each rule against the query embedding.
        let mut scored: Vec<(usize, f64)> = Vec::with_capacity(self.config.rules.len());
        for (i, rule) in self.config.rules.iter().enumerate() {
            let score = rule
                .bank
                .score(&embedding)
                .map_err(|e| SignalError::Internal(e.to_string()))?;

            // Hard match: first rule exceeding threshold wins immediately.
            if score >= rule.threshold {
                return Ok(SignalResult {
                    name: self.config.name.clone(),
                    signal_type: SignalType::Embedding,
                    confidence: score,
                    labels: vec![rule.label.clone()],
                    metadata: HashMap::new(),
                });
            }

            scored.push((i, score));
        }

        // Soft match: collect top-K rules by score.
        scored.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Determine the global top_k limit (max across all rules, capped by count).
        let top_k = self
            .config
            .rules
            .iter()
            .map(|r| r.top_k)
            .max()
            .unwrap_or(1)
            .min(scored.len());

        let labels: Vec<String> = scored[..top_k]
            .iter()
            .map(|(i, _)| self.config.rules[*i].label.clone())
            .collect();

        let best_score = scored.first().map(|(_, s)| *s).unwrap_or(0.0);

        Ok(SignalResult {
            name: self.config.name.clone(),
            signal_type: SignalType::Embedding,
            confidence: best_score,
            labels,
            metadata: HashMap::new(),
        })
    }

    fn name(&self) -> &str {
        &self.config.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Embedding
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::EmbeddingError;

    /// Mock engine that returns a fixed embedding for any input.
    struct FixedEngine {
        vector: Vec<f32>,
    }

    #[async_trait]
    impl EmbeddingEngine for FixedEngine {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(self.vector.clone())
        }

        async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            Ok(texts.iter().map(|_| self.vector.clone()).collect())
        }

        fn dimension(&self) -> usize {
            self.vector.len()
        }
    }

    fn make_ctx(text: &str) -> ClassificationContext {
        ClassificationContext {
            text: text.to_string(),
            history: vec![],
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    fn make_bank(proto: Vec<f32>) -> PrototypeBank {
        let dim = proto.len();
        PrototypeBank::new(vec![proto], vec![1.0], 1.0, 1)
            .unwrap_or_else(|e| panic!("failed to create bank (dim={dim}): {e}"))
    }

    #[tokio::test]
    async fn hard_match_returns_first_exceeding_threshold() {
        let engine = Arc::new(FixedEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
        });

        let config = EmbeddingSignalConfig {
            name: "test".to_string(),
            rules: vec![
                EmbeddingRule {
                    label: "coding".to_string(),
                    bank: make_bank(vec![1.0, 0.0, 0.0, 0.0]),
                    threshold: 0.9,
                    top_k: 1,
                },
                EmbeddingRule {
                    label: "math".to_string(),
                    bank: make_bank(vec![0.0, 1.0, 0.0, 0.0]),
                    threshold: 0.9,
                    top_k: 1,
                },
            ],
        };

        let signal = EmbeddingSignal::new(config, engine);
        let result = signal.evaluate(&make_ctx("hello")).await.unwrap();

        assert_eq!(result.labels, vec!["coding"]);
        assert!(result.confidence >= 0.9);
    }

    #[tokio::test]
    async fn soft_match_returns_top_k_when_no_hard_match() {
        let engine = Arc::new(FixedEngine {
            vector: vec![0.7, 0.7, 0.0, 0.0],
        });

        let config = EmbeddingSignalConfig {
            name: "test".to_string(),
            rules: vec![
                EmbeddingRule {
                    label: "coding".to_string(),
                    bank: make_bank(vec![1.0, 0.0, 0.0, 0.0]),
                    threshold: 0.99,
                    top_k: 2,
                },
                EmbeddingRule {
                    label: "math".to_string(),
                    bank: make_bank(vec![0.0, 1.0, 0.0, 0.0]),
                    threshold: 0.99,
                    top_k: 2,
                },
                EmbeddingRule {
                    label: "art".to_string(),
                    bank: make_bank(vec![0.0, 0.0, 1.0, 0.0]),
                    threshold: 0.99,
                    top_k: 2,
                },
            ],
        };

        let signal = EmbeddingSignal::new(config, engine);
        let result = signal.evaluate(&make_ctx("hello")).await.unwrap();

        assert_eq!(result.labels.len(), 2);
        assert!(result.labels.contains(&"coding".to_string()));
        assert!(result.labels.contains(&"math".to_string()));
        assert!(result.confidence > 0.0);
    }

    #[tokio::test]
    async fn empty_input_returns_error() {
        let engine = Arc::new(FixedEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
        });

        let config = EmbeddingSignalConfig {
            name: "test".to_string(),
            rules: vec![],
        };

        let signal = EmbeddingSignal::new(config, engine);
        let err = signal.evaluate(&make_ctx("")).await;
        assert!(err.is_err());
    }

    #[tokio::test]
    async fn name_and_signal_type() {
        let engine = Arc::new(FixedEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
        });

        let config = EmbeddingSignalConfig {
            name: "embed_sig".to_string(),
            rules: vec![],
        };

        let signal = EmbeddingSignal::new(config, engine);
        assert_eq!(signal.name(), "embed_sig");
        assert_eq!(signal.signal_type(), SignalType::Embedding);
    }

    #[tokio::test]
    async fn single_rule_below_threshold_returns_soft() {
        let engine = Arc::new(FixedEngine {
            vector: vec![0.6, 0.8, 0.0, 0.0],
        });

        let config = EmbeddingSignalConfig {
            name: "test".to_string(),
            rules: vec![EmbeddingRule {
                label: "only".to_string(),
                bank: make_bank(vec![1.0, 0.0, 0.0, 0.0]),
                threshold: 0.99,
                top_k: 1,
            }],
        };

        let signal = EmbeddingSignal::new(config, engine);
        let result = signal.evaluate(&make_ctx("hi")).await.unwrap();

        assert_eq!(result.labels, vec!["only"]);
        assert!(result.confidence < 0.99);
    }
}

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::embedding::EmbeddingEngine;
use crate::prototype::PrototypeBank;
use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

pub struct ComplexitySignal {
    name: String,
    hard_bank: PrototypeBank,
    easy_bank: PrototypeBank,
    engine: Arc<dyn EmbeddingEngine>,
    margin: f64,
}

impl ComplexitySignal {
    pub fn new(
        name: impl Into<String>,
        hard_bank: PrototypeBank,
        easy_bank: PrototypeBank,
        engine: Arc<dyn EmbeddingEngine>,
        margin: f64,
    ) -> Self {
        Self {
            name: name.into(),
            hard_bank,
            easy_bank,
            engine,
            margin,
        }
    }
}

#[async_trait]
impl Signal for ComplexitySignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        let embedding = self
            .engine
            .embed(&ctx.text)
            .await
            .map_err(|e| SignalError::Inference(e.to_string()))?;

        let hard_score = self
            .hard_bank
            .score(&embedding)
            .map_err(|e| SignalError::Inference(e.to_string()))?;

        let easy_score = self
            .easy_bank
            .score(&embedding)
            .map_err(|e| SignalError::Inference(e.to_string()))?;

        let (label, confidence) = if hard_score - easy_score > self.margin {
            ("complex", hard_score)
        } else if easy_score - hard_score > self.margin {
            ("simple", easy_score)
        } else {
            ("moderate", 0.5)
        };

        let mut metadata = HashMap::new();
        metadata.insert("hard_score".into(), serde_json::Value::from(hard_score));
        metadata.insert("easy_score".into(), serde_json::Value::from(easy_score));
        metadata.insert("margin".into(), serde_json::Value::from(self.margin));

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: SignalType::Complexity,
            confidence,
            labels: vec![label.to_string()],
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Complexity
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{EmbeddingEngine, EmbeddingError};

    struct MockEngine {
        vector: Vec<f32>,
    }

    #[async_trait]
    impl EmbeddingEngine for MockEngine {
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

    fn make_bank(vecs: Vec<Vec<f32>>) -> PrototypeBank {
        let n = vecs.len();
        PrototypeBank::new(vecs, vec![1.0; n], 0.5, 1).unwrap()
    }

    fn ctx(text: &str) -> ClassificationContext {
        ClassificationContext {
            text: text.to_string(),
            history: vec![],
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn labels_complex_when_hard_dominates() {
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
        });
        let hard = make_bank(vec![vec![1.0, 0.0, 0.0, 0.0]]);
        let easy = make_bank(vec![vec![0.0, 1.0, 0.0, 0.0]]);
        let signal = ComplexitySignal::new("test", hard, easy, engine, 0.1);

        let result = signal.evaluate(&ctx("anything")).await.unwrap();
        assert_eq!(result.labels, vec!["complex"]);
        assert!(result.confidence > 0.5);
    }

    #[tokio::test]
    async fn labels_simple_when_easy_dominates() {
        let engine = Arc::new(MockEngine {
            vector: vec![0.0, 1.0, 0.0, 0.0],
        });
        let hard = make_bank(vec![vec![1.0, 0.0, 0.0, 0.0]]);
        let easy = make_bank(vec![vec![0.0, 1.0, 0.0, 0.0]]);
        let signal = ComplexitySignal::new("test", hard, easy, engine, 0.1);

        let result = signal.evaluate(&ctx("anything")).await.unwrap();
        assert_eq!(result.labels, vec!["simple"]);
        assert!(result.confidence > 0.5);
    }

    #[tokio::test]
    async fn labels_moderate_when_within_margin() {
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 1.0, 0.0, 0.0],
        });
        let hard = make_bank(vec![vec![1.0, 0.0, 0.0, 0.0]]);
        let easy = make_bank(vec![vec![0.0, 1.0, 0.0, 0.0]]);
        let signal = ComplexitySignal::new("test", hard, easy, engine, 0.5);

        let result = signal.evaluate(&ctx("anything")).await.unwrap();
        assert_eq!(result.labels, vec!["moderate"]);
        assert!((result.confidence - 0.5).abs() < 1e-6);
    }

    #[tokio::test]
    async fn metadata_contains_scores() {
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
        });
        let hard = make_bank(vec![vec![1.0, 0.0, 0.0, 0.0]]);
        let easy = make_bank(vec![vec![0.0, 1.0, 0.0, 0.0]]);
        let signal = ComplexitySignal::new("test", hard, easy, engine, 0.1);

        let result = signal.evaluate(&ctx("anything")).await.unwrap();
        assert!(result.metadata.contains_key("hard_score"));
        assert!(result.metadata.contains_key("easy_score"));
        assert!(result.metadata.contains_key("margin"));
    }

    #[test]
    fn signal_type_is_complexity() {
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
        });
        let hard = make_bank(vec![vec![1.0, 0.0, 0.0, 0.0]]);
        let easy = make_bank(vec![vec![0.0, 1.0, 0.0, 0.0]]);
        let signal = ComplexitySignal::new("cplx", hard, easy, engine, 0.1);

        assert_eq!(signal.signal_type(), SignalType::Complexity);
        assert_eq!(signal.name(), "cplx");
    }
}

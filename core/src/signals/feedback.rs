use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::embedding::{cosine_similarity, EmbeddingEngine};
use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

#[async_trait]
pub trait SatisfactionDetector: Send + Sync {
    /// Returns satisfaction score 0.0 (dissatisfied) to 1.0 (satisfied).
    async fn score(&self, text: &str) -> Result<f64, SignalError>;
}

pub struct FeedbackSignal {
    name: String,
    detector: Arc<dyn SatisfactionDetector>,
    engine: Arc<dyn EmbeddingEngine>,
    reask_threshold: f64,
}

impl FeedbackSignal {
    pub fn new(
        name: impl Into<String>,
        detector: Arc<dyn SatisfactionDetector>,
        engine: Arc<dyn EmbeddingEngine>,
        reask_threshold: f64,
    ) -> Self {
        Self {
            name: name.into(),
            detector,
            engine,
            reask_threshold,
        }
    }

    fn satisfaction_label(score: f64) -> &'static str {
        if score >= 0.7 {
            "satisfied"
        } else if score >= 0.4 {
            "neutral"
        } else {
            "dissatisfied"
        }
    }
}

#[async_trait]
impl Signal for FeedbackSignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        let satisfaction_score = self.detector.score(&ctx.text).await?;

        let mut reask_similarity: f64 = 0.0;
        let mut is_reask = false;

        if let Some(last) = ctx.history.last() {
            let embeddings = self
                .engine
                .embed_batch(&[ctx.text.as_str(), last.as_str()])
                .await
                .map_err(|e| SignalError::Inference(e.to_string()))?;

            let sim = cosine_similarity(&embeddings[0], &embeddings[1]);
            reask_similarity = sim as f64;
            is_reask = reask_similarity > self.reask_threshold;
        }

        let mut labels = vec![Self::satisfaction_label(satisfaction_score).to_string()];
        if is_reask {
            labels.push("reask".to_string());
        }

        // Confidence: use satisfaction score distance from neutral
        let confidence = if is_reask {
            reask_similarity
        } else {
            (satisfaction_score - 0.5).abs() * 2.0
        };

        let mut metadata = HashMap::new();
        metadata.insert(
            "satisfaction_score".into(),
            serde_json::Value::from(satisfaction_score),
        );
        metadata.insert(
            "reask_similarity".into(),
            serde_json::Value::from(reask_similarity),
        );

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: SignalType::Feedback,
            confidence,
            labels,
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Feedback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{EmbeddingEngine, EmbeddingError};

    struct MockDetector {
        value: f64,
    }

    #[async_trait]
    impl SatisfactionDetector for MockDetector {
        async fn score(&self, _text: &str) -> Result<f64, SignalError> {
            Ok(self.value)
        }
    }

    struct MockEngine {
        /// Returns same vector for all texts — sim = 1.0
        vector: Vec<f32>,
        /// If set, return different vectors per call index
        varied: bool,
    }

    #[async_trait]
    impl EmbeddingEngine for MockEngine {
        async fn embed(&self, _text: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(self.vector.clone())
        }

        async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            if self.varied {
                // First text gets [1,0,0,0], second gets [0,1,0,0]
                Ok(texts
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        let mut v = vec![0.0f32; 4];
                        v[i % 4] = 1.0;
                        v
                    })
                    .collect())
            } else {
                Ok(texts.iter().map(|_| self.vector.clone()).collect())
            }
        }

        fn dimension(&self) -> usize {
            self.vector.len()
        }
    }

    fn ctx_with_history(text: &str, history: Vec<&str>) -> ClassificationContext {
        ClassificationContext {
            text: text.to_string(),
            history: history.into_iter().map(String::from).collect(),
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    fn ctx(text: &str) -> ClassificationContext {
        ctx_with_history(text, vec![])
    }

    #[tokio::test]
    async fn labels_satisfied_high_score() {
        let detector = Arc::new(MockDetector { value: 0.9 });
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
            varied: false,
        });
        let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

        let result = signal.evaluate(&ctx("great")).await.unwrap();
        assert!(result.labels.contains(&"satisfied".to_string()));
    }

    #[tokio::test]
    async fn labels_neutral_mid_score() {
        let detector = Arc::new(MockDetector { value: 0.5 });
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
            varied: false,
        });
        let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

        let result = signal.evaluate(&ctx("ok")).await.unwrap();
        assert!(result.labels.contains(&"neutral".to_string()));
    }

    #[tokio::test]
    async fn labels_dissatisfied_low_score() {
        let detector = Arc::new(MockDetector { value: 0.1 });
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
            varied: false,
        });
        let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

        let result = signal.evaluate(&ctx("bad")).await.unwrap();
        assert!(result.labels.contains(&"dissatisfied".to_string()));
    }

    #[tokio::test]
    async fn detects_reask_with_similar_history() {
        let detector = Arc::new(MockDetector { value: 0.3 });
        // Same vector for all => cosine sim = 1.0
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
            varied: false,
        });
        let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

        let result = signal
            .evaluate(&ctx_with_history("same q", vec!["same q"]))
            .await
            .unwrap();
        assert!(result.labels.contains(&"reask".to_string()));
    }

    #[tokio::test]
    async fn no_reask_with_different_history() {
        let detector = Arc::new(MockDetector { value: 0.5 });
        // Different vectors => cosine sim = 0.0
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
            varied: true,
        });
        let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

        let result = signal
            .evaluate(&ctx_with_history("new q", vec!["old q"]))
            .await
            .unwrap();
        assert!(!result.labels.contains(&"reask".to_string()));
    }

    #[tokio::test]
    async fn no_reask_without_history() {
        let detector = Arc::new(MockDetector { value: 0.5 });
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
            varied: false,
        });
        let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

        let result = signal.evaluate(&ctx("hello")).await.unwrap();
        assert!(!result.labels.contains(&"reask".to_string()));
    }

    #[tokio::test]
    async fn metadata_contains_scores() {
        let detector = Arc::new(MockDetector { value: 0.7 });
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
            varied: false,
        });
        let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

        let result = signal.evaluate(&ctx("test")).await.unwrap();
        assert!(result.metadata.contains_key("satisfaction_score"));
        assert!(result.metadata.contains_key("reask_similarity"));
    }

    #[test]
    fn signal_type_is_feedback() {
        let detector = Arc::new(MockDetector { value: 0.5 });
        let engine = Arc::new(MockEngine {
            vector: vec![1.0, 0.0, 0.0, 0.0],
            varied: false,
        });
        let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

        assert_eq!(signal.signal_type(), SignalType::Feedback);
        assert_eq!(signal.name(), "fb");
    }
}

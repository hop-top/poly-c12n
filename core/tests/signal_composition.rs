mod common;

use std::sync::Arc;

use c12n_core::embedding::EmbeddingEngine;
use c12n_core::prototype::PrototypeBank;
use c12n_core::signal::Signal;
use c12n_core::signals::complexity::ComplexitySignal;
use c12n_core::signals::embedding_signal::{
    EmbeddingRule, EmbeddingSignal, EmbeddingSignalConfig,
};
use c12n_core::signals::feedback::{FeedbackSignal, SatisfactionDetector};
use c12n_core::types::{SignalError, SignalType};

use common::{make_ctx, make_ctx_with_history, MockEmbeddingEngine};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_bank(proto: Vec<f32>) -> PrototypeBank {
    PrototypeBank::new(vec![proto], vec![1.0], 1.0, 1).unwrap()
}

fn make_multi_bank(vecs: Vec<Vec<f32>>) -> PrototypeBank {
    let n = vecs.len();
    PrototypeBank::new(vecs, vec![1.0; n], 0.5, 1).unwrap()
}

struct FixedSatisfaction(f64);

#[async_trait::async_trait]
impl SatisfactionDetector for FixedSatisfaction {
    async fn score(
        &self,
        _text: &str,
    ) -> Result<f64, SignalError> {
        Ok(self.0)
    }
}

// ---------------------------------------------------------------------------
// 1. Embedding + PrototypeBank
// ---------------------------------------------------------------------------

#[tokio::test]
async fn embedding_signal_with_prototype_bank() {
    let engine: Arc<dyn EmbeddingEngine> =
        Arc::new(MockEmbeddingEngine);

    // MockEmbeddingEngine returns [0.5, 0.5, 0.5, 0.5] for generic text.
    // Build a bank with prototype [0.5, 0.5, 0.5, 0.5] => cosine = 1.0
    let config = EmbeddingSignalConfig {
        name: "embed_test".to_string(),
        rules: vec![
            EmbeddingRule {
                label: "exact_match".to_string(),
                bank: make_bank(vec![0.5, 0.5, 0.5, 0.5]),
                threshold: 0.9,
                top_k: 1,
            },
            EmbeddingRule {
                label: "orthogonal".to_string(),
                bank: make_bank(vec![1.0, 0.0, 0.0, 0.0]),
                threshold: 0.9,
                top_k: 1,
            },
        ],
    };

    let signal = EmbeddingSignal::new(config, engine);
    let result = signal
        .evaluate(&make_ctx("some generic text"))
        .await
        .unwrap();

    // Should hard-match "exact_match" (cosine = 1.0 >= 0.9 threshold)
    assert_eq!(result.signal_type, SignalType::Embedding);
    assert!(
        result.labels.contains(&"exact_match".to_string()),
        "should match exact prototype, got: {:?}",
        result.labels,
    );
    assert!(
        result.confidence >= 0.9,
        "confidence should exceed threshold",
    );
}

#[tokio::test]
async fn embedding_signal_soft_match() {
    let engine: Arc<dyn EmbeddingEngine> =
        Arc::new(MockEmbeddingEngine);

    // Both rules set very high threshold so no hard match occurs
    let config = EmbeddingSignalConfig {
        name: "embed_soft".to_string(),
        rules: vec![
            EmbeddingRule {
                label: "rule_a".to_string(),
                bank: make_bank(vec![1.0, 0.0, 0.0, 0.0]),
                threshold: 1.1, // impossible to hard match
                top_k: 2,
            },
            EmbeddingRule {
                label: "rule_b".to_string(),
                bank: make_bank(vec![0.0, 1.0, 0.0, 0.0]),
                threshold: 1.1,
                top_k: 2,
            },
        ],
    };

    let signal = EmbeddingSignal::new(config, engine);
    let result = signal
        .evaluate(&make_ctx("generic text"))
        .await
        .unwrap();

    // Soft match returns top-k labels
    assert_eq!(result.labels.len(), 2);
    assert!(result.confidence > 0.0);
    assert!(result.confidence < 1.1);
}

// ---------------------------------------------------------------------------
// 2. Complexity with dual banks
// ---------------------------------------------------------------------------

#[tokio::test]
async fn complexity_hard_dominates() {
    let engine: Arc<dyn EmbeddingEngine> =
        Arc::new(MockEmbeddingEngine);

    // "hard" text => [1,0,0,0]; hard_bank aligned with [1,0,0,0]
    let hard_bank = make_multi_bank(vec![vec![1.0, 0.0, 0.0, 0.0]]);
    let easy_bank = make_multi_bank(vec![vec![0.0, 1.0, 0.0, 0.0]]);

    let signal = ComplexitySignal::new(
        "cplx", hard_bank, easy_bank, engine, 0.1,
    );

    let result = signal
        .evaluate(&make_ctx("this is hard"))
        .await
        .unwrap();

    assert_eq!(result.signal_type, SignalType::Complexity);
    assert_eq!(result.labels, vec!["complex"]);
    assert!(result.confidence > 0.5);
    assert!(result.metadata.contains_key("hard_score"));
    assert!(result.metadata.contains_key("easy_score"));
}

#[tokio::test]
async fn complexity_easy_dominates() {
    let engine: Arc<dyn EmbeddingEngine> =
        Arc::new(MockEmbeddingEngine);

    // "easy" text => [0,1,0,0]; easy_bank aligned with [0,1,0,0]
    let hard_bank = make_multi_bank(vec![vec![1.0, 0.0, 0.0, 0.0]]);
    let easy_bank = make_multi_bank(vec![vec![0.0, 1.0, 0.0, 0.0]]);

    let signal = ComplexitySignal::new(
        "cplx", hard_bank, easy_bank, engine, 0.1,
    );

    let result = signal
        .evaluate(&make_ctx("this is easy"))
        .await
        .unwrap();

    assert_eq!(result.labels, vec!["simple"]);
    assert!(result.confidence > 0.5);
}

#[tokio::test]
async fn complexity_moderate_within_margin() {
    let engine: Arc<dyn EmbeddingEngine> =
        Arc::new(MockEmbeddingEngine);

    // Generic text => [0.5,0.5,0.5,0.5]; both banks score similarly
    let hard_bank = make_multi_bank(vec![vec![1.0, 0.0, 0.0, 0.0]]);
    let easy_bank = make_multi_bank(vec![vec![0.0, 1.0, 0.0, 0.0]]);

    let signal = ComplexitySignal::new(
        "cplx", hard_bank, easy_bank, engine, 0.5,
    );

    let result = signal
        .evaluate(&make_ctx("generic text here"))
        .await
        .unwrap();

    assert_eq!(result.labels, vec!["moderate"]);
    assert!((result.confidence - 0.5).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// 3. Feedback with history (reask detection)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn feedback_detects_reask_with_similar_history() {
    let detector = Arc::new(FixedSatisfaction(0.3));
    let engine: Arc<dyn EmbeddingEngine> =
        Arc::new(MockEmbeddingEngine);

    // MockEmbeddingEngine returns same vector for same-ish text
    // => cosine similarity = 1.0 => reask detected
    let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

    let ctx = make_ctx_with_history(
        "tell me about X",
        vec!["tell me about X"],
    );
    let result = signal.evaluate(&ctx).await.unwrap();

    assert!(
        result.labels.contains(&"reask".to_string()),
        "should detect reask, got: {:?}",
        result.labels,
    );
    assert!(
        result.labels.contains(&"dissatisfied".to_string()),
        "low satisfaction => dissatisfied",
    );
}

#[tokio::test]
async fn feedback_no_reask_different_content() {
    let detector = Arc::new(FixedSatisfaction(0.5));

    // Use an engine where embed_batch returns different vectors
    // to force cosine_similarity < threshold
    struct DiffEngine;

    #[async_trait::async_trait]
    impl EmbeddingEngine for DiffEngine {
        async fn embed(
            &self,
            _text: &str,
        ) -> Result<Vec<f32>, c12n_core::embedding::EmbeddingError> {
            Ok(vec![1.0, 0.0, 0.0, 0.0])
        }

        async fn embed_batch(
            &self,
            texts: &[&str],
        ) -> Result<Vec<Vec<f32>>, c12n_core::embedding::EmbeddingError>
        {
            // Return orthogonal vectors for different texts
            Ok(texts
                .iter()
                .enumerate()
                .map(|(i, _)| {
                    let mut v = vec![0.0f32; 4];
                    v[i % 4] = 1.0;
                    v
                })
                .collect())
        }

        fn dimension(&self) -> usize {
            4
        }
    }

    let engine: Arc<dyn EmbeddingEngine> = Arc::new(DiffEngine);
    let signal = FeedbackSignal::new("fb", detector, engine, 0.8);

    let ctx = make_ctx_with_history(
        "new question",
        vec!["old question"],
    );
    let result = signal.evaluate(&ctx).await.unwrap();

    assert!(
        !result.labels.contains(&"reask".to_string()),
        "should NOT detect reask for different content",
    );
}

#[tokio::test]
async fn feedback_no_reask_without_history() {
    let detector = Arc::new(FixedSatisfaction(0.9));
    let engine: Arc<dyn EmbeddingEngine> =
        Arc::new(MockEmbeddingEngine);

    let signal = FeedbackSignal::new("fb", detector, engine, 0.8);
    let result = signal
        .evaluate(&make_ctx("first message"))
        .await
        .unwrap();

    assert!(
        !result.labels.contains(&"reask".to_string()),
        "no history => no reask",
    );
    assert!(
        result.labels.contains(&"satisfied".to_string()),
        "high satisfaction => satisfied",
    );
    assert!(result.metadata.contains_key("satisfaction_score"));
    assert!(result.metadata.contains_key("reask_similarity"));
}

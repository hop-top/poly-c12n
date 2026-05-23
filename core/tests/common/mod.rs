#![allow(dead_code)]
//! Shared helpers across integration test binaries. Each binary uses a
//! subset, so blanket-allow dead_code at the module level.

use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use c12n_core::embedding::{EmbeddingEngine, EmbeddingError};
use c12n_core::signal::Signal;
use c12n_core::types::{ClassificationContext, SignalError, SignalResult, SignalType};

// ---------------------------------------------------------------------------
// MockEmbeddingEngine
// ---------------------------------------------------------------------------

/// Returns fixed vectors based on text content.
/// - Contains "hard" => [1, 0, 0, 0]
/// - Contains "easy" => [0, 1, 0, 0]
/// - Otherwise        => [0.5, 0.5, 0.5, 0.5]
pub struct MockEmbeddingEngine;

#[async_trait]
impl EmbeddingEngine for MockEmbeddingEngine {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Ok(vector_for(text))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Ok(texts.iter().map(|t| vector_for(t)).collect())
    }

    fn dimension(&self) -> usize {
        4
    }
}

fn vector_for(text: &str) -> Vec<f32> {
    let lower = text.to_lowercase();
    if lower.contains("hard") {
        vec![1.0, 0.0, 0.0, 0.0]
    } else if lower.contains("easy") {
        vec![0.0, 1.0, 0.0, 0.0]
    } else {
        vec![0.5, 0.5, 0.5, 0.5]
    }
}

// ---------------------------------------------------------------------------
// MockSignal
// ---------------------------------------------------------------------------

/// Configurable signal for pipeline tests.
pub struct MockSignal {
    pub label: String,
    pub signal_type: SignalType,
    pub confidence: f64,
    pub delay: Duration,
}

#[async_trait]
impl Signal for MockSignal {
    async fn evaluate(&self, _ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        Ok(SignalResult {
            name: self.label.clone(),
            signal_type: self.signal_type,
            confidence: self.confidence,
            labels: vec![self.label.clone()],
            metadata: HashMap::new(),
        })
    }

    fn name(&self) -> &str {
        &self.label
    }

    fn signal_type(&self) -> SignalType {
        self.signal_type
    }
}

// ---------------------------------------------------------------------------
// FailingSignal
// ---------------------------------------------------------------------------

/// Always returns an error.
pub struct FailingSignal {
    pub label: String,
}

#[async_trait]
impl Signal for FailingSignal {
    async fn evaluate(&self, _ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        Err(SignalError::Internal(format!(
            "{} always fails",
            self.label
        )))
    }

    fn name(&self) -> &str {
        &self.label
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Custom
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn make_ctx(text: &str) -> ClassificationContext {
    ClassificationContext {
        text: text.to_string(),
        history: vec![],
        headers: HashMap::new(),
        image_url: None,
        config: HashMap::new(),
    }
}

pub fn make_ctx_with_history(text: &str, history: Vec<&str>) -> ClassificationContext {
    ClassificationContext {
        text: text.to_string(),
        history: history.into_iter().map(String::from).collect(),
        headers: HashMap::new(),
        image_url: None,
        config: HashMap::new(),
    }
}

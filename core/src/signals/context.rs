use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

#[async_trait]
pub trait Tokenizer: Send + Sync {
    fn count_tokens(&self, text: &str) -> usize;
    fn model_name(&self) -> &str;
}

pub struct ModelPricing {
    pub model: String,
    pub input_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
}

pub struct ContextSignal {
    name: String,
    tokenizer: Arc<dyn Tokenizer>,
    output_ratio: f64,
    pricing: Vec<ModelPricing>,
}

impl ContextSignal {
    pub fn new(
        name: impl Into<String>,
        tokenizer: Arc<dyn Tokenizer>,
        output_ratio: f64,
        pricing: Vec<ModelPricing>,
    ) -> Self {
        Self {
            name: name.into(),
            tokenizer,
            output_ratio,
            pricing,
        }
    }

    fn token_label(count: usize) -> &'static str {
        match count {
            0..=99 => "short",
            100..=999 => "medium",
            1000..=4999 => "long",
            _ => "very_long",
        }
    }
}

#[async_trait]
impl Signal for ContextSignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        let input_tokens = self.tokenizer.count_tokens(&ctx.text);
        let estimated_output = (input_tokens as f64 * self.output_ratio).ceil() as usize;

        let label = Self::token_label(input_tokens);

        let mut costs = HashMap::new();
        for p in &self.pricing {
            let input_cost = (input_tokens as f64 / 1000.0) * p.input_cost_per_1k;
            let output_cost = (estimated_output as f64 / 1000.0) * p.output_cost_per_1k;
            costs.insert(
                p.model.clone(),
                serde_json::json!({
                    "input_cost": input_cost,
                    "output_cost": output_cost,
                    "total_cost": input_cost + output_cost,
                }),
            );
        }

        let mut metadata = HashMap::new();
        metadata.insert("input_tokens".into(), serde_json::json!(input_tokens));
        metadata.insert(
            "estimated_output_tokens".into(),
            serde_json::json!(estimated_output),
        );
        metadata.insert(
            "tokenizer_model".into(),
            serde_json::json!(self.tokenizer.model_name()),
        );
        metadata.insert("costs".into(), serde_json::json!(costs));

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: SignalType::Context,
            confidence: 1.0,
            labels: vec![label.to_string()],
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockTokenizer;

    #[async_trait]
    impl Tokenizer for MockTokenizer {
        fn count_tokens(&self, text: &str) -> usize {
            text.len() / 4
        }

        fn model_name(&self) -> &str {
            "mock-v1"
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

    #[tokio::test]
    async fn short_text_label() {
        let signal = ContextSignal::new("ctx", Arc::new(MockTokenizer), 0.5, vec![]);
        let result = signal.evaluate(&make_ctx("hello")).await.unwrap();
        assert_eq!(result.labels, vec!["short"]);
        assert_eq!(result.signal_type, SignalType::Context);
        assert_eq!(result.confidence, 1.0);
    }

    #[tokio::test]
    async fn medium_text_label() {
        let text = "a".repeat(500);
        let signal = ContextSignal::new("ctx", Arc::new(MockTokenizer), 0.5, vec![]);
        let result = signal.evaluate(&make_ctx(&text)).await.unwrap();
        assert_eq!(result.labels, vec!["medium"]);
    }

    #[tokio::test]
    async fn long_text_label() {
        let text = "a".repeat(8000);
        let signal = ContextSignal::new("ctx", Arc::new(MockTokenizer), 0.5, vec![]);
        let result = signal.evaluate(&make_ctx(&text)).await.unwrap();
        assert_eq!(result.labels, vec!["long"]);
    }

    #[tokio::test]
    async fn very_long_text_label() {
        let text = "a".repeat(40000);
        let signal = ContextSignal::new("ctx", Arc::new(MockTokenizer), 0.5, vec![]);
        let result = signal.evaluate(&make_ctx(&text)).await.unwrap();
        assert_eq!(result.labels, vec!["very_long"]);
    }

    #[tokio::test]
    async fn cost_calculation() {
        let pricing = vec![ModelPricing {
            model: "gpt-4".into(),
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
        }];
        let signal = ContextSignal::new("ctx", Arc::new(MockTokenizer), 1.0, pricing);
        let text = "a".repeat(4000);
        let result = signal.evaluate(&make_ctx(&text)).await.unwrap();

        let input_tokens = result.metadata["input_tokens"].as_u64().unwrap();
        assert_eq!(input_tokens, 1000);

        let est_output = result.metadata["estimated_output_tokens"].as_u64().unwrap();
        assert_eq!(est_output, 1000);

        let costs = result.metadata["costs"].as_object().unwrap();
        let gpt4 = costs["gpt-4"].as_object().unwrap();
        let total = gpt4["total_cost"].as_f64().unwrap();
        assert!((total - 0.09).abs() < 1e-9);
    }
}

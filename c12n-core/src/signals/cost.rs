use std::collections::HashMap;

use async_trait::async_trait;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

// ---------------------------------------------------------------------------
// ModelCost
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct ModelCost {
    pub model: String,
    pub input_cost_per_1k: f64,
    pub output_cost_per_1k: f64,
}

// ---------------------------------------------------------------------------
// CostEstimateSignal
// ---------------------------------------------------------------------------

pub struct CostEstimateSignal {
    name: String,
    models: Vec<ModelCost>,
    output_ratio: f64,
}

impl CostEstimateSignal {
    pub fn new(
        name: impl Into<String>,
        models: Vec<ModelCost>,
        output_ratio: f64,
    ) -> Self {
        Self {
            name: name.into(),
            models,
            output_ratio,
        }
    }

    /// Defaults with common model pricing (approximate, 2024-era).
    pub fn with_defaults(name: impl Into<String>) -> Self {
        let models = vec![
            ModelCost {
                model: "gpt-4o".to_string(),
                input_cost_per_1k: 0.005,
                output_cost_per_1k: 0.015,
            },
            ModelCost {
                model: "gpt-4o-mini".to_string(),
                input_cost_per_1k: 0.00015,
                output_cost_per_1k: 0.0006,
            },
            ModelCost {
                model: "claude-3.5-sonnet".to_string(),
                input_cost_per_1k: 0.003,
                output_cost_per_1k: 0.015,
            },
            ModelCost {
                model: "claude-3-haiku".to_string(),
                input_cost_per_1k: 0.00025,
                output_cost_per_1k: 0.00125,
            },
            ModelCost {
                model: "gemini-1.5-pro".to_string(),
                input_cost_per_1k: 0.00125,
                output_cost_per_1k: 0.005,
            },
        ];

        Self::new(name, models, 1.5)
    }

    /// Rough token estimate: word_count * 1.3
    fn estimate_tokens(text: &str) -> f64 {
        let word_count = text.split_whitespace().count() as f64;
        word_count * 1.3
    }

    fn cost_tier(total_cost: f64) -> &'static str {
        if total_cost < 0.001 {
            "micro"
        } else if total_cost < 0.01 {
            "small"
        } else if total_cost < 0.10 {
            "medium"
        } else {
            "large"
        }
    }
}

#[async_trait]
impl Signal for CostEstimateSignal {
    async fn evaluate(
        &self,
        ctx: &ClassificationContext,
    ) -> Result<SignalResult, SignalError> {
        let input_tokens = Self::estimate_tokens(&ctx.text);
        let output_tokens = input_tokens * self.output_ratio;

        let mut per_model = HashMap::new();
        let mut labels = Vec::new();
        let mut min_cost = f64::MAX;

        for mc in &self.models {
            let input_cost =
                (input_tokens / 1000.0) * mc.input_cost_per_1k;
            let output_cost =
                (output_tokens / 1000.0) * mc.output_cost_per_1k;
            let total = input_cost + output_cost;

            let tier = Self::cost_tier(total);
            let tier_label = format!("{}:{}", mc.model, tier);
            if !labels.contains(&tier_label) {
                labels.push(tier_label);
            }

            let model_detail = serde_json::json!({
                "input_tokens": input_tokens,
                "output_tokens": output_tokens,
                "input_cost": input_cost,
                "output_cost": output_cost,
                "total_cost": total,
                "tier": tier,
            });
            per_model.insert(mc.model.clone(), model_detail);

            if total < min_cost {
                min_cost = total;
            }
        }

        // Overall tier = cheapest model tier
        let overall_tier = Self::cost_tier(min_cost);
        labels.insert(0, overall_tier.to_string());

        let mut metadata = HashMap::new();
        metadata.insert(
            "per_model".to_string(),
            serde_json::to_value(&per_model).unwrap(),
        );
        metadata.insert(
            "input_tokens".to_string(),
            serde_json::json!(input_tokens),
        );
        metadata.insert(
            "output_tokens".to_string(),
            serde_json::json!(output_tokens),
        );
        metadata.insert(
            "output_ratio".to_string(),
            serde_json::json!(self.output_ratio),
        );

        // Confidence is always 1.0 for cost (deterministic)
        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: self.signal_type(),
            confidence: 1.0,
            labels,
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::CostEstimate
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

    #[tokio::test]
    async fn short_prompt_micro_tier() {
        let s = CostEstimateSignal::with_defaults("cost");
        let r = s.evaluate(&ctx("Hello")).await.unwrap();
        assert_eq!(r.confidence, 1.0);
        assert_eq!(r.labels[0], "micro");
    }

    #[tokio::test]
    async fn has_per_model_breakdown() {
        let s = CostEstimateSignal::with_defaults("cost");
        let r = s.evaluate(&ctx("Hello world")).await.unwrap();
        let per_model = r.metadata.get("per_model").unwrap();
        assert!(per_model.get("gpt-4o").is_some());
        assert!(per_model.get("claude-3.5-sonnet").is_some());
    }

    #[tokio::test]
    async fn token_estimate_scales() {
        let short = CostEstimateSignal::estimate_tokens("hi");
        let long = CostEstimateSignal::estimate_tokens(
            "This is a much longer prompt with many words",
        );
        assert!(long > short);
    }

    #[tokio::test]
    async fn custom_models() {
        let s = CostEstimateSignal::new(
            "cost",
            vec![ModelCost {
                model: "test-model".to_string(),
                input_cost_per_1k: 10.0,
                output_cost_per_1k: 10.0,
            }],
            2.0,
        );
        // Even "hello" at $10/1k tokens should be non-zero
        let r = s.evaluate(&ctx("hello world")).await.unwrap();
        assert!(r.labels.iter().any(|l| l.starts_with("test-model:")));
    }

    #[tokio::test]
    async fn cost_tiers() {
        assert_eq!(CostEstimateSignal::cost_tier(0.0001), "micro");
        assert_eq!(CostEstimateSignal::cost_tier(0.005), "small");
        assert_eq!(CostEstimateSignal::cost_tier(0.05), "medium");
        assert_eq!(CostEstimateSignal::cost_tier(0.5), "large");
    }

    #[tokio::test]
    async fn longer_prompt_higher_cost() {
        let s = CostEstimateSignal::with_defaults("cost");
        let r1 = s.evaluate(&ctx("hi")).await.unwrap();
        let long_text = "word ".repeat(500);
        let r2 = s.evaluate(&ctx(&long_text)).await.unwrap();

        let t1: f64 = r1.metadata["input_tokens"].as_f64().unwrap();
        let t2: f64 = r2.metadata["input_tokens"].as_f64().unwrap();
        assert!(t2 > t1);
    }
}

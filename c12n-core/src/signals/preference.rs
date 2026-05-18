use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

#[async_trait]
pub trait PreferenceLlm: Send + Sync {
    async fn query(
        &self,
        prompt: &str,
        system: &str,
    ) -> Result<String, SignalError>;
}

pub struct PreferenceSignal {
    name: String,
    llm: Arc<dyn PreferenceLlm>,
    system_prompt: String,
    timeout: Duration,
    labels: Vec<String>,
}

impl PreferenceSignal {
    pub fn new(
        name: impl Into<String>,
        llm: Arc<dyn PreferenceLlm>,
        system_prompt: impl Into<String>,
        timeout: Duration,
        labels: Vec<String>,
    ) -> Self {
        Self {
            name: name.into(),
            llm,
            system_prompt: system_prompt.into(),
            timeout,
            labels,
        }
    }

    fn find_label(&self, response: &str) -> (String, f64) {
        let lower = response.to_lowercase();
        let mut matches: Vec<&String> = self
            .labels
            .iter()
            .filter(|l| lower.contains(&l.to_lowercase()))
            .collect();

        if matches.len() == 1 {
            (matches[0].clone(), 1.0)
        } else if matches.is_empty() {
            // No label found — return first label as fallback
            (
                self.labels
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "unknown".into()),
                0.5,
            )
        } else {
            // Multiple matches — ambiguous; pick first match
            matches.sort();
            (matches[0].clone(), 0.5)
        }
    }
}

#[async_trait]
impl Signal for PreferenceSignal {
    async fn evaluate(
        &self,
        ctx: &ClassificationContext,
    ) -> Result<SignalResult, SignalError> {
        let response = tokio::time::timeout(
            self.timeout,
            self.llm.query(&ctx.text, &self.system_prompt),
        )
        .await
        .map_err(|_| SignalError::Timeout)??;

        let (label, confidence) = self.find_label(&response);

        let mut metadata = HashMap::new();
        metadata.insert(
            "raw_response".into(),
            serde_json::Value::from(response),
        );

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: SignalType::Preference,
            confidence,
            labels: vec![label],
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Preference
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLlm {
        response: Result<String, SignalError>,
    }

    #[async_trait]
    impl PreferenceLlm for MockLlm {
        async fn query(
            &self,
            _prompt: &str,
            _system: &str,
        ) -> Result<String, SignalError> {
            match &self.response {
                Ok(s) => Ok(s.clone()),
                Err(_) => Err(SignalError::Inference("mock error".into())),
            }
        }
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

    fn labels() -> Vec<String> {
        vec![
            "model-a".into(),
            "model-b".into(),
            "model-c".into(),
        ]
    }

    #[tokio::test]
    async fn clear_match_returns_full_confidence() {
        let llm = Arc::new(MockLlm {
            response: Ok("I prefer model-b for this task".into()),
        });
        let signal = PreferenceSignal::new(
            "pref",
            llm,
            "pick one",
            Duration::from_secs(5),
            labels(),
        );

        let result = signal.evaluate(&ctx("test")).await.unwrap();
        assert_eq!(result.labels, vec!["model-b"]);
        assert!((result.confidence - 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn ambiguous_match_returns_half_confidence() {
        let llm = Arc::new(MockLlm {
            response: Ok("model-a and model-b are both good".into()),
        });
        let signal = PreferenceSignal::new(
            "pref",
            llm,
            "pick one",
            Duration::from_secs(5),
            labels(),
        );

        let result = signal.evaluate(&ctx("test")).await.unwrap();
        assert!((result.confidence - 0.5).abs() < 1e-6);
    }

    #[tokio::test]
    async fn no_match_returns_fallback() {
        let llm = Arc::new(MockLlm {
            response: Ok("none of the above".into()),
        });
        let signal = PreferenceSignal::new(
            "pref",
            llm,
            "pick one",
            Duration::from_secs(5),
            labels(),
        );

        let result = signal.evaluate(&ctx("test")).await.unwrap();
        assert_eq!(result.labels, vec!["model-a"]);
        assert!((result.confidence - 0.5).abs() < 1e-6);
    }

    #[tokio::test]
    async fn timeout_returns_error() {
        struct SlowLlm;

        #[async_trait]
        impl PreferenceLlm for SlowLlm {
            async fn query(
                &self,
                _prompt: &str,
                _system: &str,
            ) -> Result<String, SignalError> {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok("model-a".into())
            }
        }

        let signal = PreferenceSignal::new(
            "pref",
            Arc::new(SlowLlm),
            "pick one",
            Duration::from_millis(10),
            labels(),
        );

        let result = signal.evaluate(&ctx("test")).await;
        assert!(matches!(result, Err(SignalError::Timeout)));
    }

    #[tokio::test]
    async fn llm_error_propagates() {
        let llm = Arc::new(MockLlm {
            response: Err(SignalError::Inference("fail".into())),
        });
        let signal = PreferenceSignal::new(
            "pref",
            llm,
            "pick one",
            Duration::from_secs(5),
            labels(),
        );

        let result = signal.evaluate(&ctx("test")).await;
        assert!(result.is_err());
    }

    #[test]
    fn signal_type_is_preference() {
        let llm = Arc::new(MockLlm {
            response: Ok("ok".into()),
        });
        let signal = PreferenceSignal::new(
            "pref",
            llm,
            "sys",
            Duration::from_secs(5),
            labels(),
        );

        assert_eq!(signal.signal_type(), SignalType::Preference);
        assert_eq!(signal.name(), "pref");
    }
}

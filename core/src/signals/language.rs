use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

#[derive(Debug, Clone)]
pub struct DetectedLanguage {
    pub code: String,
    pub name: String,
    pub confidence: f64,
}

#[async_trait]
pub trait LanguageDetector: Send + Sync {
    fn detect(&self, text: &str) -> Option<DetectedLanguage>;
    fn detect_multiple(&self, text: &str) -> Vec<DetectedLanguage>;
}

pub struct LanguageSignal {
    name: String,
    detector: Arc<dyn LanguageDetector>,
}

impl LanguageSignal {
    pub fn new(name: impl Into<String>, detector: Arc<dyn LanguageDetector>) -> Self {
        Self {
            name: name.into(),
            detector,
        }
    }
}

#[async_trait]
impl Signal for LanguageSignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        let primary = self.detector.detect(&ctx.text);
        let all = self.detector.detect_multiple(&ctx.text);

        let (labels, confidence) = match &primary {
            Some(lang) => (vec![lang.code.clone()], lang.confidence),
            None => (vec![], 0.0),
        };

        let mut metadata = HashMap::new();

        if let Some(ref lang) = primary {
            metadata.insert(
                "primary_language".into(),
                serde_json::json!({
                    "code": lang.code,
                    "name": lang.name,
                    "confidence": lang.confidence,
                }),
            );
        }

        let detected: Vec<_> = all
            .iter()
            .map(|l| {
                serde_json::json!({
                    "code": l.code,
                    "name": l.name,
                    "confidence": l.confidence,
                })
            })
            .collect();
        metadata.insert("detected_languages".into(), serde_json::json!(detected));
        metadata.insert("language_count".into(), serde_json::json!(all.len()));

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: SignalType::Language,
            confidence,
            labels,
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Language
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockDetector;

    #[async_trait]
    impl LanguageDetector for MockDetector {
        fn detect(&self, text: &str) -> Option<DetectedLanguage> {
            if text.is_empty() {
                return None;
            }
            // Simple heuristic: check for common words
            if text.contains("bonjour") || text.contains("merci") {
                Some(DetectedLanguage {
                    code: "fr".into(),
                    name: "French".into(),
                    confidence: 0.9,
                })
            } else {
                Some(DetectedLanguage {
                    code: "en".into(),
                    name: "English".into(),
                    confidence: 0.95,
                })
            }
        }

        fn detect_multiple(&self, text: &str) -> Vec<DetectedLanguage> {
            let mut results = Vec::new();
            if let Some(primary) = self.detect(text) {
                results.push(primary);
            }
            if text.contains("bonjour") && text.contains("hello") {
                results.push(DetectedLanguage {
                    code: "en".into(),
                    name: "English".into(),
                    confidence: 0.4,
                });
            }
            results
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
    async fn detects_english() {
        let signal = LanguageSignal::new("lang", Arc::new(MockDetector));
        let result = signal.evaluate(&make_ctx("hello world")).await.unwrap();
        assert_eq!(result.labels, vec!["en"]);
        assert_eq!(result.confidence, 0.95);
        assert_eq!(result.signal_type, SignalType::Language);
    }

    #[tokio::test]
    async fn detects_french() {
        let signal = LanguageSignal::new("lang", Arc::new(MockDetector));
        let result = signal
            .evaluate(&make_ctx("bonjour le monde"))
            .await
            .unwrap();
        assert_eq!(result.labels, vec!["fr"]);
        assert_eq!(result.confidence, 0.9);
    }

    #[tokio::test]
    async fn empty_text_no_detection() {
        let signal = LanguageSignal::new("lang", Arc::new(MockDetector));
        let result = signal.evaluate(&make_ctx("")).await.unwrap();
        assert!(result.labels.is_empty());
        assert_eq!(result.confidence, 0.0);
    }

    #[tokio::test]
    async fn multiple_languages_in_metadata() {
        let signal = LanguageSignal::new("lang", Arc::new(MockDetector));
        let result = signal.evaluate(&make_ctx("hello bonjour")).await.unwrap();
        // Primary is French (bonjour triggers first)
        assert_eq!(result.labels, vec!["fr"]);

        let langs = result.metadata["detected_languages"].as_array().unwrap();
        assert_eq!(langs.len(), 2);

        let count = result.metadata["language_count"].as_u64().unwrap();
        assert_eq!(count, 2);
    }

    #[tokio::test]
    async fn metadata_includes_primary() {
        let signal = LanguageSignal::new("lang", Arc::new(MockDetector));
        let result = signal.evaluate(&make_ctx("hello")).await.unwrap();
        let primary = result.metadata["primary_language"].as_object().unwrap();
        assert_eq!(primary["code"].as_str().unwrap(), "en");
        assert_eq!(primary["name"].as_str().unwrap(), "English");
    }
}

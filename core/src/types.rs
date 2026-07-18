use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SignalType {
    Keyword,
    Embedding,
    Domain,
    Jailbreak,
    PII,
    Toxicity,
    Context,
    Structure,
    Language,
    Complexity,
    Preference,
    Feedback,
    OutputFormat,
    CodeContent,
    ToolCalling,
    CostEstimate,
    Sentiment,
    Intent,
    Topic,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignalResult {
    pub name: String,
    pub signal_type: SignalType,
    pub confidence: f64,
    pub labels: Vec<String>,
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationContext {
    pub text: String,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub history: Vec<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub headers: HashMap<String, String>,
    #[serde(default)]
    pub image_url: Option<String>,
    #[serde(default, deserialize_with = "deserialize_null_default")]
    pub config: HashMap<String, serde_json::Value>,
}

// Deserializer that treats explicit JSON `null` for sequence/map fields
// as the type's Default rather than failing. Lets Go callers that send
// nil slices/maps (which marshal to `null`) interop with this struct
// without populating empty placeholders on the wire.
fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Default + serde::Deserialize<'de>,
{
    let opt = Option::<T>::deserialize(deserializer)?;
    Ok(opt.unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_deserialize_tolerates_missing_fields() {
        let json = r#"{"text":"hello"}"#;
        let ctx: ClassificationContext = serde_json::from_str(json).unwrap();
        assert_eq!(ctx.text, "hello");
        assert!(ctx.history.is_empty());
        assert!(ctx.headers.is_empty());
        assert!(ctx.config.is_empty());
        assert!(ctx.image_url.is_none());
    }

    #[test]
    fn context_deserialize_tolerates_null_collections() {
        let json = r#"{"text":"hello","history":null,"headers":null,"config":null}"#;
        let ctx: ClassificationContext = serde_json::from_str(json).unwrap();
        assert_eq!(ctx.text, "hello");
        assert!(ctx.history.is_empty());
        assert!(ctx.headers.is_empty());
        assert!(ctx.config.is_empty());
    }
}

#[derive(Debug, Error)]
pub enum SignalError {
    #[error("signal evaluation timed out")]
    Timeout,
    #[error("failed to load model: {0}")]
    ModelLoad(String),
    #[error("inference failed: {0}")]
    Inference(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("internal error: {0}")]
    Internal(String),
}

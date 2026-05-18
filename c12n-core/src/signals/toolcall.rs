use std::collections::HashMap;

use async_trait::async_trait;
use regex::Regex;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

// ---------------------------------------------------------------------------
// ToolCallingSignal
// ---------------------------------------------------------------------------

pub struct ToolCallingSignal {
    name: String,
    action_verbs: Vec<(Regex, String)>,
    temporal_re: Regex,
    realworld_patterns: Vec<Regex>,
}

impl ToolCallingSignal {
    pub fn new(name: impl Into<String>) -> Self {
        let verbs = &[
            "search", "lookup", "look up", "find", "calculate",
            "fetch", "send", "create", "delete", "update", "get",
            "check", "book", "order", "schedule", "download",
            "upload", "query", "list", "subscribe",
        ];

        let action_verbs: Vec<(Regex, String)> = verbs
            .iter()
            .map(|v| {
                let pat = format!(r"(?i)\b{}\b", v.replace(' ', r"\s+"));
                (Regex::new(&pat).unwrap(), v.to_string())
            })
            .collect();

        let realworld_patterns = vec![
            r"(?i)\bwhat('s| is) the weather\b",
            r"(?i)\bstock price\b",
            r"(?i)\bwhat time\b",
            r"(?i)\bexchange rate\b",
            r"(?i)\bcurrent (price|temperature|status|score)\b",
            r"(?i)\bhow much does .+ cost\b",
            r"(?i)\btrack(ing)?\s+(my\s+)?(order|package|shipment)\b",
            r"(?i)\bflight\s+status\b",
            r"(?i)\bsend\s+(an?\s+)?email\b",
            r"(?i)\bset\s+(an?\s+)?(alarm|reminder|timer)\b",
        ];

        Self {
            name: name.into(),
            action_verbs,
            temporal_re: Regex::new(
                r"(?i)\b(today|now|current(ly)?|latest|recent(ly)?|tomorrow|tonight|this\s+(morning|afternoon|evening|week|month))\b",
            )
            .unwrap(),
            realworld_patterns: realworld_patterns
                .into_iter()
                .map(|p| Regex::new(p).unwrap())
                .collect(),
        }
    }
}

#[async_trait]
impl Signal for ToolCallingSignal {
    async fn evaluate(
        &self,
        ctx: &ClassificationContext,
    ) -> Result<SignalResult, SignalError> {
        let text = &ctx.text;
        let mut labels = Vec::new();
        let mut signals = 0u32;

        // Action verb detection
        let mut matched_verbs = Vec::new();
        for (re, verb) in &self.action_verbs {
            if re.is_match(text) {
                matched_verbs.push(verb.clone());
                let label = format!("action:{}", verb);
                if !labels.contains(&label) {
                    labels.push(label);
                }
                signals += 1;
            }
        }

        // Temporal references
        let has_temporal = self.temporal_re.is_match(text);
        if has_temporal {
            signals += 1;
            if !labels.contains(&"temporal".to_string()) {
                labels.push("temporal".to_string());
            }
        }

        // Real-world state questions
        let mut has_realworld = false;
        for re in &self.realworld_patterns {
            if re.is_match(text) {
                has_realworld = true;
                signals += 1;
                if !labels.contains(&"realworld_state".to_string()) {
                    labels.push("realworld_state".to_string());
                }
                break;
            }
        }

        // Confidence: scale with number of indicators
        let confidence = if signals == 0 {
            0.0
        } else {
            let base: f64 = match signals {
                1 => 0.5,
                2 => 0.75,
                _ => 0.9,
            };
            // Realworld bumps confidence
            if has_realworld { base.max(0.8) } else { base }
        };

        // Top-level label
        if signals > 0 && !labels.contains(&"tool_calling".to_string()) {
            labels.insert(0, "tool_calling".to_string());
        }

        let mut metadata = HashMap::new();
        metadata.insert(
            "action_verbs".to_string(),
            serde_json::to_value(&matched_verbs).unwrap(),
        );
        metadata.insert(
            "has_temporal".to_string(),
            serde_json::Value::Bool(has_temporal),
        );
        metadata.insert(
            "has_realworld_state".to_string(),
            serde_json::Value::Bool(has_realworld),
        );
        metadata.insert(
            "signal_count".to_string(),
            serde_json::Value::Number(signals.into()),
        );

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: self.signal_type(),
            confidence,
            labels,
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::ToolCalling
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
    async fn detects_action_verb() {
        let s = ToolCallingSignal::new("tool");
        let r = s.evaluate(&ctx("Search for flights to NYC")).await.unwrap();
        assert!(r.confidence >= 0.5);
        assert!(r.labels.contains(&"tool_calling".to_string()));
        assert!(r.labels.contains(&"action:search".to_string()));
    }

    #[tokio::test]
    async fn detects_temporal() {
        let s = ToolCallingSignal::new("tool");
        let r = s
            .evaluate(&ctx("Find the latest news today"))
            .await
            .unwrap();
        assert!(r.confidence >= 0.75);
        assert!(r.labels.contains(&"temporal".to_string()));
    }

    #[tokio::test]
    async fn detects_realworld_state() {
        let s = ToolCallingSignal::new("tool");
        let r = s
            .evaluate(&ctx("What's the weather in London?"))
            .await
            .unwrap();
        assert!(r.confidence >= 0.8);
        assert!(r.labels.contains(&"realworld_state".to_string()));
    }

    #[tokio::test]
    async fn multiple_signals_high_confidence() {
        let s = ToolCallingSignal::new("tool");
        let r = s
            .evaluate(&ctx(
                "Check the current stock price of AAPL",
            ))
            .await
            .unwrap();
        // "check" verb + "current" temporal + "stock price" realworld
        assert!(r.confidence >= 0.9);
    }

    #[tokio::test]
    async fn no_tool_calling() {
        let s = ToolCallingSignal::new("tool");
        let r = s
            .evaluate(&ctx("Explain the theory of relativity"))
            .await
            .unwrap();
        assert_eq!(r.confidence, 0.0);
        assert!(r.labels.is_empty());
    }

    #[tokio::test]
    async fn send_email_realworld() {
        let s = ToolCallingSignal::new("tool");
        let r = s
            .evaluate(&ctx("Send an email to Bob"))
            .await
            .unwrap();
        assert!(r.confidence >= 0.8);
        assert!(r.labels.contains(&"realworld_state".to_string()));
    }

    #[tokio::test]
    async fn schedule_action() {
        let s = ToolCallingSignal::new("tool");
        let r = s
            .evaluate(&ctx("Schedule a meeting for tomorrow"))
            .await
            .unwrap();
        assert!(r.confidence >= 0.75);
        assert!(r.labels.contains(&"action:schedule".to_string()));
        assert!(r.labels.contains(&"temporal".to_string()));
    }
}

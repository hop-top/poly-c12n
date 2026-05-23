use std::cmp::Ordering;
use std::collections::HashMap;

use async_trait::async_trait;
use regex::Regex;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

pub struct StructureRule {
    pub name: String,
    pub pattern: String,
    pub predicate: StructurePredicate,
}

pub enum StructurePredicate {
    Exists,
    Count(Ordering, usize),
    Density(Ordering, f64),
    Sequence(Vec<String>),
}

pub struct StructureSignal {
    name: String,
    rules: Vec<StructureRule>,
}

impl StructureSignal {
    pub fn new(name: impl Into<String>, rules: Vec<StructureRule>) -> Self {
        Self {
            name: name.into(),
            rules,
        }
    }

    fn check_rule(rule: &StructureRule, text: &str) -> Result<bool, SignalError> {
        match &rule.predicate {
            StructurePredicate::Exists => {
                let re = Self::compile(&rule.pattern)?;
                Ok(re.is_match(text))
            }
            StructurePredicate::Count(ord, threshold) => {
                let re = Self::compile(&rule.pattern)?;
                let count = re.find_iter(text).count();
                Ok(count.cmp(threshold) == *ord)
            }
            StructurePredicate::Density(ord, threshold) => {
                let re = Self::compile(&rule.pattern)?;
                let count = re.find_iter(text).count();
                let density = if text.is_empty() {
                    0.0
                } else {
                    (count as f64 / text.len() as f64) * 100.0
                };
                Ok(density.partial_cmp(threshold).unwrap_or(Ordering::Equal) == *ord)
            }
            StructurePredicate::Sequence(patterns) => {
                let mut search_start = 0;
                for pat in patterns {
                    let re = Self::compile(pat)?;
                    match re.find(&text[search_start..]) {
                        Some(m) => {
                            search_start += m.end();
                        }
                        None => return Ok(false),
                    }
                }
                Ok(true)
            }
        }
    }

    fn compile(pattern: &str) -> Result<Regex, SignalError> {
        Regex::new(pattern)
            .map_err(|e| SignalError::Configuration(format!("invalid regex '{}': {}", pattern, e)))
    }
}

#[async_trait]
impl Signal for StructureSignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        if self.rules.is_empty() {
            return Ok(SignalResult {
                name: self.name.clone(),
                signal_type: SignalType::Structure,
                confidence: 0.0,
                labels: vec![],
                metadata: HashMap::new(),
            });
        }

        let mut matched = Vec::new();
        let mut rule_results = HashMap::new();

        for rule in &self.rules {
            let passes = Self::check_rule(rule, &ctx.text)?;
            rule_results.insert(rule.name.clone(), serde_json::json!(passes));
            if passes {
                matched.push(rule.name.clone());
            }
        }

        let confidence = matched.len() as f64 / self.rules.len() as f64;

        let mut metadata = HashMap::new();
        metadata.insert("rule_results".into(), serde_json::json!(rule_results));
        metadata.insert("matched_count".into(), serde_json::json!(matched.len()));
        metadata.insert("total_rules".into(), serde_json::json!(self.rules.len()));

        Ok(SignalResult {
            name: self.name.clone(),
            signal_type: SignalType::Structure,
            confidence,
            labels: matched,
            metadata,
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Structure
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    async fn exists_predicate() {
        let signal = StructureSignal::new(
            "struct",
            vec![StructureRule {
                name: "has_url".into(),
                pattern: r"https?://\S+".into(),
                predicate: StructurePredicate::Exists,
            }],
        );
        let result = signal
            .evaluate(&make_ctx("visit https://example.com"))
            .await
            .unwrap();
        assert_eq!(result.labels, vec!["has_url"]);
        assert_eq!(result.confidence, 1.0);

        let result = signal.evaluate(&make_ctx("no links here")).await.unwrap();
        assert!(result.labels.is_empty());
        assert_eq!(result.confidence, 0.0);
    }

    #[tokio::test]
    async fn count_predicate() {
        let signal = StructureSignal::new(
            "struct",
            vec![StructureRule {
                name: "many_sentences".into(),
                pattern: r"\.".into(),
                predicate: StructurePredicate::Count(Ordering::Greater, 2),
            }],
        );
        let result = signal
            .evaluate(&make_ctx("One. Two. Three. Four."))
            .await
            .unwrap();
        assert_eq!(result.labels, vec!["many_sentences"]);

        let result = signal.evaluate(&make_ctx("One. Two.")).await.unwrap();
        assert!(result.labels.is_empty());
    }

    #[tokio::test]
    async fn density_predicate() {
        let signal = StructureSignal::new(
            "struct",
            vec![StructureRule {
                name: "high_digit_density".into(),
                pattern: r"\d".into(),
                predicate: StructurePredicate::Density(Ordering::Greater, 10.0),
            }],
        );
        // 10 digits in 20 chars = 50%
        let result = signal
            .evaluate(&make_ctx("1234567890abcdefghij"))
            .await
            .unwrap();
        assert_eq!(result.labels, vec!["high_digit_density"]);
    }

    #[tokio::test]
    async fn sequence_predicate() {
        let signal = StructureSignal::new(
            "struct",
            vec![StructureRule {
                name: "greeting_then_question".into(),
                pattern: String::new(),
                predicate: StructurePredicate::Sequence(vec![r"(?i)hello".into(), r"\?".into()]),
            }],
        );
        let result = signal
            .evaluate(&make_ctx("Hello there! How are you?"))
            .await
            .unwrap();
        assert_eq!(result.labels, vec!["greeting_then_question"]);

        let result = signal
            .evaluate(&make_ctx("How are you? Hello!"))
            .await
            .unwrap();
        assert!(result.labels.is_empty());
    }

    #[tokio::test]
    async fn multiple_rules_confidence() {
        let signal = StructureSignal::new(
            "struct",
            vec![
                StructureRule {
                    name: "has_code".into(),
                    pattern: r"```".into(),
                    predicate: StructurePredicate::Exists,
                },
                StructureRule {
                    name: "has_heading".into(),
                    pattern: r"^#\s".into(),
                    predicate: StructurePredicate::Exists,
                },
            ],
        );
        let result = signal
            .evaluate(&make_ctx("# Title\n```code```"))
            .await
            .unwrap();
        assert_eq!(result.confidence, 1.0);
        assert_eq!(result.labels.len(), 2);

        let result = signal.evaluate(&make_ctx("```code```")).await.unwrap();
        assert_eq!(result.confidence, 0.5);
        assert_eq!(result.labels, vec!["has_code"]);
    }

    #[tokio::test]
    async fn empty_rules() {
        let signal = StructureSignal::new("struct", vec![]);
        let result = signal.evaluate(&make_ctx("anything")).await.unwrap();
        assert_eq!(result.confidence, 0.0);
        assert!(result.labels.is_empty());
    }

    #[tokio::test]
    async fn invalid_regex_returns_error() {
        let signal = StructureSignal::new(
            "struct",
            vec![StructureRule {
                name: "bad".into(),
                pattern: r"[invalid".into(),
                predicate: StructurePredicate::Exists,
            }],
        );
        let result = signal.evaluate(&make_ctx("test")).await;
        assert!(result.is_err());
    }
}

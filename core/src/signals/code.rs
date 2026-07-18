use std::collections::HashMap;

use async_trait::async_trait;
use regex::Regex;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

// ---------------------------------------------------------------------------
// CodeIntent
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeIntent {
    Generate,
    Review,
    Debug,
    Explain,
    Refactor,
    Test,
    Document,
}

impl CodeIntent {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Generate => "generate",
            Self::Review => "review",
            Self::Debug => "debug",
            Self::Explain => "explain",
            Self::Refactor => "refactor",
            Self::Test => "test",
            Self::Document => "document",
        }
    }
}

// ---------------------------------------------------------------------------
// Language list
// ---------------------------------------------------------------------------

const LANGUAGES: &[&str] = &[
    "rust",
    "python",
    "javascript",
    "typescript",
    "java",
    "kotlin",
    "swift",
    "go",
    "golang",
    "ruby",
    "php",
    "csharp",
    "c#",
    "cpp",
    "c\\+\\+",
    "scala",
    "haskell",
    "elixir",
    "erlang",
    "clojure",
    "lua",
    "perl",
    "r",
    "matlab",
    "julia",
    "dart",
    "zig",
    "nim",
    "ocaml",
    "fortran",
    "sql",
    "bash",
    "shell",
    "powershell",
    "objective-c",
    "assembly",
    "html",
    "css",
];

// ---------------------------------------------------------------------------
// CodeContentSignal
// ---------------------------------------------------------------------------

pub struct CodeContentSignal {
    name: String,
    code_fence_re: Regex,
    lang_patterns: Vec<(Regex, String)>,
    intent_patterns: Vec<(Regex, CodeIntent)>,
    keyword_re: Regex,
}

impl CodeContentSignal {
    pub fn new(name: impl Into<String>) -> Self {
        let lang_patterns: Vec<(Regex, String)> = LANGUAGES
            .iter()
            .map(|lang| {
                let display = lang.replace("\\+", "+").replace("\\", "");
                let is_word_only = display
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '_');
                let pat = if is_word_only {
                    format!(r"(?i)\b{}\b", lang)
                } else {
                    format!(r"(?i)(^|[^[:alnum:]_])(?:{})($|[^[:alnum:]_])", lang)
                };
                (Regex::new(&pat).unwrap(), display)
            })
            .collect();

        let intent_patterns = vec![
            (
                r"(?i)\b(write|create|implement|build|generate|make)\s+(a\s+)?(function|class|method|program|script|module|api|code)\b",
                CodeIntent::Generate,
            ),
            (
                r"(?i)\b(review|check|audit|inspect|analyze)\s+(this\s+|my\s+|the\s+)?(code|implementation|function|class)\b",
                CodeIntent::Review,
            ),
            (
                r"(?i)\b(fix|debug|troubleshoot|solve|resolve)\s+(this\s+|my\s+|the\s+)?(bug|error|issue|problem|code)\b",
                CodeIntent::Debug,
            ),
            (
                r"(?i)\bwhat\s+does\s+(this|the)\s+(code|function|method|class)\s+do\b",
                CodeIntent::Explain,
            ),
            (
                r"(?i)\b(explain|understand|walk\s+through|describe)\s+(this\s+|the\s+)?(code|function|method|class|snippet)\b",
                CodeIntent::Explain,
            ),
            (
                r"(?i)\b(refactor|improve|optimize|clean\s+up|simplify)\s+(this\s+|my\s+|the\s+)?(code|function|method|class|implementation)\b",
                CodeIntent::Refactor,
            ),
            (
                r"(?i)\b(write|create|add|generate)\s+(a\s+)?(unit\s+)?test",
                CodeIntent::Test,
            ),
            (
                r"(?i)\b(document|add\s+docs|write\s+docs|add\s+comments|docstring)\b",
                CodeIntent::Document,
            ),
        ];

        Self {
            name: name.into(),
            code_fence_re: Regex::new(r"```[\w]*").unwrap(),
            lang_patterns,
            intent_patterns: intent_patterns
                .into_iter()
                .map(|(p, i)| (Regex::new(p).unwrap(), i))
                .collect(),
            keyword_re: Regex::new(
                r"(?i)\b(function|class|struct|enum|interface|trait|impl|def|fn|var|let|const|import|require|module|package|async|await|return|throw|catch|try)\b"
            ).unwrap(),
        }
    }
}

#[async_trait]
impl Signal for CodeContentSignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        let text = &ctx.text;
        let mut labels = Vec::new();
        let mut confidence = 0.0_f64;

        // Detect code fences
        let has_code_fence = self.code_fence_re.is_match(text);
        if has_code_fence {
            confidence = confidence.max(0.9);
        }

        // Detect languages
        let mut languages = Vec::new();
        for (re, lang) in &self.lang_patterns {
            if re.is_match(text) && !languages.contains(lang) {
                languages.push(lang.clone());
                let label = format!("lang:{}", lang);
                if !labels.contains(&label) {
                    labels.push(label);
                }
                confidence = confidence.max(0.8);
            }
        }

        // Detect intent
        let mut intent: Option<CodeIntent> = None;
        for (re, i) in &self.intent_patterns {
            if re.is_match(text) {
                intent = Some(*i);
                let label = format!("intent:{}", i.as_str());
                if !labels.contains(&label) {
                    labels.push(label);
                }
                confidence = confidence.max(0.85);
                break;
            }
        }

        // Programming keywords as weak signal
        if labels.is_empty() && self.keyword_re.is_match(text) {
            confidence = confidence.max(0.5);
        }

        // Build metadata
        let mut metadata = HashMap::new();
        metadata.insert(
            "languages".to_string(),
            serde_json::to_value(&languages).unwrap(),
        );
        metadata.insert(
            "intent".to_string(),
            serde_json::to_value(intent.map(|i| i.as_str().to_string())).unwrap(),
        );
        metadata.insert(
            "has_code_fence".to_string(),
            serde_json::Value::Bool(has_code_fence),
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
        SignalType::CodeContent
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
    async fn detects_code_fence() {
        let s = CodeContentSignal::new("code");
        let r = s
            .evaluate(&ctx("```rust\nfn main() {}\n```"))
            .await
            .unwrap();
        assert!(r.confidence >= 0.9);
        assert!(r.labels.contains(&"lang:rust".to_string()));
        assert_eq!(r.metadata["has_code_fence"], serde_json::Value::Bool(true),);
    }

    #[tokio::test]
    async fn detects_generate_intent() {
        let s = CodeContentSignal::new("code");
        let r = s
            .evaluate(&ctx("Write a function in Python to sort a list"))
            .await
            .unwrap();
        assert!(r.labels.contains(&"intent:generate".to_string()));
        assert!(r.labels.contains(&"lang:python".to_string()));
    }

    #[tokio::test]
    async fn detects_debug_intent() {
        let s = CodeContentSignal::new("code");
        let r = s.evaluate(&ctx("Fix this bug in my code")).await.unwrap();
        assert!(r.labels.contains(&"intent:debug".to_string()));
    }

    #[tokio::test]
    async fn detects_explain_intent() {
        let s = CodeContentSignal::new("code");
        let r = s.evaluate(&ctx("What does this code do?")).await.unwrap();
        assert!(r.labels.contains(&"intent:explain".to_string()));
    }

    #[tokio::test]
    async fn detects_review_intent() {
        let s = CodeContentSignal::new("code");
        let r = s
            .evaluate(&ctx("Review this code for issues"))
            .await
            .unwrap();
        assert!(r.labels.contains(&"intent:review".to_string()));
    }

    #[tokio::test]
    async fn detects_refactor_intent() {
        let s = CodeContentSignal::new("code");
        let r = s
            .evaluate(&ctx("Refactor this code to be cleaner"))
            .await
            .unwrap();
        assert!(r.labels.contains(&"intent:refactor".to_string()));
    }

    #[tokio::test]
    async fn detects_multiple_languages() {
        let s = CodeContentSignal::new("code");
        let r = s
            .evaluate(&ctx("Convert this Python script to JavaScript"))
            .await
            .unwrap();
        assert!(r.labels.contains(&"lang:python".to_string()));
        assert!(r.labels.contains(&"lang:javascript".to_string()));
    }

    #[tokio::test]
    async fn weak_keyword_signal() {
        let s = CodeContentSignal::new("code");
        let r = s
            .evaluate(&ctx("The function returns a value"))
            .await
            .unwrap();
        assert!(r.confidence >= 0.5);
        // No labels from keywords alone
    }

    #[tokio::test]
    async fn no_code_detected() {
        let s = CodeContentSignal::new("code");
        let r = s
            .evaluate(&ctx("Tell me about the weather today"))
            .await
            .unwrap();
        assert_eq!(r.confidence, 0.0);
    }
}

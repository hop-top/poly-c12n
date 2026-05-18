use std::collections::HashMap;

use async_trait::async_trait;
use regex::Regex;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

// ---------------------------------------------------------------------------
// OutputFormat
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Xml,
    Markdown,
    Code,
    Table,
    Csv,
    Yaml,
    PlainText,
}

impl OutputFormat {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Xml => "xml",
            Self::Markdown => "markdown",
            Self::Code => "code",
            Self::Table => "table",
            Self::Csv => "csv",
            Self::Yaml => "yaml",
            Self::PlainText => "plain_text",
        }
    }
}

// ---------------------------------------------------------------------------
// OutputFormatSignal
// ---------------------------------------------------------------------------

pub struct OutputFormatSignal {
    name: String,
    explicit_patterns: Vec<(Regex, OutputFormat)>,
    inferred_patterns: Vec<(Regex, OutputFormat)>,
}

impl OutputFormatSignal {
    pub fn new(name: impl Into<String>) -> Self {
        let explicit = vec![
            (r"(?i)\b(respond|reply|answer|output|format|give|return|provide)\b.{0,20}\bjson\b", OutputFormat::Json),
            (r"(?i)\bas\s+json\b", OutputFormat::Json),
            (r"(?i)\bin\s+json\b", OutputFormat::Json),
            (r"(?i)\bjson\s+(format|output|response)\b", OutputFormat::Json),
            (r"(?i)\b(respond|reply|answer|output|format|give|return|provide)\b.{0,20}\bxml\b", OutputFormat::Xml),
            (r"(?i)\bas\s+xml\b", OutputFormat::Xml),
            (r"(?i)\bin\s+xml\b", OutputFormat::Xml),
            (r"(?i)\b(respond|reply|answer|output|format|give|return|provide)\b.{0,20}\bmarkdown\b", OutputFormat::Markdown),
            (r"(?i)\bas\s+markdown\b", OutputFormat::Markdown),
            (r"(?i)\bin\s+markdown\b", OutputFormat::Markdown),
            (r"(?i)\b(respond|reply|answer|output|format|give|return|provide)\b.{0,20}\btable\b", OutputFormat::Table),
            (r"(?i)\bas\s+a?\s*table\b", OutputFormat::Table),
            (r"(?i)\bformat\s+as\s+table\b", OutputFormat::Table),
            (r"(?i)\btabular\s+format\b", OutputFormat::Table),
            (r"(?i)\b(respond|reply|answer|output|format|give|return|provide)\b.{0,20}\bcsv\b", OutputFormat::Csv),
            (r"(?i)\bas\s+csv\b", OutputFormat::Csv),
            (r"(?i)\bin\s+csv\b", OutputFormat::Csv),
            (r"(?i)\b(respond|reply|answer|output|format|give|return|provide)\b.{0,20}\byaml\b", OutputFormat::Yaml),
            (r"(?i)\bas\s+yaml\b", OutputFormat::Yaml),
            (r"(?i)\bin\s+yaml\b", OutputFormat::Yaml),
            (r"(?i)\b(respond|reply|answer|output|format|give|return|provide)\b.{0,20}\bcode\b", OutputFormat::Code),
        ];

        let inferred = vec![
            (r"(?i)\bjson\b", OutputFormat::Json),
            (r"(?i)\bxml\b", OutputFormat::Xml),
            (r"```", OutputFormat::Code),
            (r"(?i)\bcode\s*(block|snippet|example)\b", OutputFormat::Code),
            (r"(?i)\byaml\b", OutputFormat::Yaml),
            (r"(?i)\bcsv\b", OutputFormat::Csv),
            (r"(?i)\btable\b", OutputFormat::Table),
            (r"(?i)\bmarkdown\b", OutputFormat::Markdown),
        ];

        Self {
            name: name.into(),
            explicit_patterns: explicit
                .into_iter()
                .map(|(p, f)| (Regex::new(p).unwrap(), f))
                .collect(),
            inferred_patterns: inferred
                .into_iter()
                .map(|(p, f)| (Regex::new(p).unwrap(), f))
                .collect(),
        }
    }
}

#[async_trait]
impl Signal for OutputFormatSignal {
    async fn evaluate(
        &self,
        ctx: &ClassificationContext,
    ) -> Result<SignalResult, SignalError> {
        let text = &ctx.text;
        let mut labels = Vec::new();
        let mut confidence = 0.0_f64;

        for (re, fmt) in &self.explicit_patterns {
            if re.is_match(text) {
                let label = fmt.as_str().to_string();
                if !labels.contains(&label) {
                    labels.push(label);
                }
                confidence = 1.0;
            }
        }

        if labels.is_empty() {
            for (re, fmt) in &self.inferred_patterns {
                if re.is_match(text) {
                    let label = fmt.as_str().to_string();
                    if !labels.contains(&label) {
                        labels.push(label);
                    }
                    confidence = 0.6;
                }
            }
        }

        let mut metadata = HashMap::new();
        metadata.insert(
            "formats".to_string(),
            serde_json::to_value(&labels).unwrap(),
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
        SignalType::OutputFormat
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
    async fn explicit_json() {
        let s = OutputFormatSignal::new("fmt");
        let r = s.evaluate(&ctx("Respond in JSON")).await.unwrap();
        assert_eq!(r.confidence, 1.0);
        assert!(r.labels.contains(&"json".to_string()));
    }

    #[tokio::test]
    async fn explicit_table() {
        let s = OutputFormatSignal::new("fmt");
        let r = s.evaluate(&ctx("Format as table please")).await.unwrap();
        assert_eq!(r.confidence, 1.0);
        assert!(r.labels.contains(&"table".to_string()));
    }

    #[tokio::test]
    async fn explicit_xml() {
        let s = OutputFormatSignal::new("fmt");
        let r = s.evaluate(&ctx("Give me XML output")).await.unwrap();
        assert_eq!(r.confidence, 1.0);
        assert!(r.labels.contains(&"xml".to_string()));
    }

    #[tokio::test]
    async fn inferred_code_fence() {
        let s = OutputFormatSignal::new("fmt");
        let r = s.evaluate(&ctx("Here is ```some code```")).await.unwrap();
        assert_eq!(r.confidence, 0.6);
        assert!(r.labels.contains(&"code".to_string()));
    }

    #[tokio::test]
    async fn inferred_yaml() {
        let s = OutputFormatSignal::new("fmt");
        let r = s.evaluate(&ctx("I have a yaml config")).await.unwrap();
        assert_eq!(r.confidence, 0.6);
        assert!(r.labels.contains(&"yaml".to_string()));
    }

    #[tokio::test]
    async fn no_format_detected() {
        let s = OutputFormatSignal::new("fmt");
        let r = s.evaluate(&ctx("Tell me a joke")).await.unwrap();
        assert_eq!(r.confidence, 0.0);
        assert!(r.labels.is_empty());
    }

    #[tokio::test]
    async fn explicit_csv() {
        let s = OutputFormatSignal::new("fmt");
        let r = s.evaluate(&ctx("Return the data as csv")).await.unwrap();
        assert_eq!(r.confidence, 1.0);
        assert!(r.labels.contains(&"csv".to_string()));
    }
}

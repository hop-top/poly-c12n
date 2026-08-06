//! Integration coverage for [`SdkPipeline`] — construction and async
//! evaluation against real engine signals.
//!
//! The C ABI in `c12n-core` hardcodes an empty signal vector, so these
//! paths are only reachable from the Rust SDK. Everything here goes
//! through the public `hop_top_c12n` surface only (no `c12n_core`
//! dependency), which is also what keeps the crate's re-export promise
//! honest.

use std::time::Duration;

use hop_top_c12n::signals::keyword::{KeywordRule, KeywordSignal, MatchOperator, MatchStrategy};
use hop_top_c12n::{
    ClassificationContext, PipelineConfig, SdkPipeline, Signal, SignalError, SignalResult,
    SignalType,
};

fn keyword_signal(name: &str, label: &str, pattern: &str) -> KeywordSignal {
    KeywordSignal::new(
        name,
        vec![KeywordRule {
            label: label.to_string(),
            patterns: vec![pattern.to_string()],
            operator: MatchOperator::Or,
            strategy: MatchStrategy::Regex,
            threshold: 0.5,
        }],
    )
}

fn config(max_concurrency: usize) -> PipelineConfig {
    PipelineConfig::builder()
        .max_concurrency(max_concurrency)
        .timeout(Duration::from_secs(5))
        .build()
}

#[test]
fn new_reports_signal_count() {
    let pipeline = SdkPipeline::new(
        vec![
            Box::new(keyword_signal("kw-a", "python", "(?i)python")),
            Box::new(keyword_signal("kw-b", "rust", "(?i)rust")),
        ],
        config(4),
    );
    assert_eq!(pipeline.signal_count(), 2);
}

#[test]
fn new_accepts_empty_signal_set() {
    let pipeline = SdkPipeline::new(vec![], PipelineConfig::default());
    assert_eq!(pipeline.signal_count(), 0);
}

#[tokio::test]
async fn evaluate_with_no_signals_reports_diagnostic() {
    let pipeline = SdkPipeline::new(vec![], config(2));
    let ctx = ClassificationContext {
        text: "anything".to_string(),
        ..Default::default()
    };

    let result = pipeline.evaluate(&ctx).await;

    // A pipeline with no registered signals reports that loudly rather than
    // returning an empty envelope indistinguishable from a no-match outcome.
    assert!(result.results.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(result.errors[0]
        .to_string()
        .contains("no registered signals"));
}

#[tokio::test]
async fn evaluate_runs_a_real_signal_and_matches() {
    let pipeline = SdkPipeline::new(
        vec![Box::new(keyword_signal("language", "python", "(?i)python"))],
        config(4),
    );
    let ctx = ClassificationContext {
        text: "Write a Python function that sorts a list".to_string(),
        ..Default::default()
    };

    let result = pipeline.evaluate(&ctx).await;

    assert!(result.errors.is_empty(), "unexpected errors: {result:?}");
    assert_eq!(result.results.len(), 1);

    let signal_result = &result.results[0];
    assert_eq!(signal_result.name, "language");
    assert_eq!(signal_result.signal_type, SignalType::Keyword);
    assert!(
        signal_result.labels.contains(&"python".to_string()),
        "expected 'python' label, got {:?}",
        signal_result.labels
    );
    assert!(signal_result.confidence > 0.0);
}

#[tokio::test]
async fn evaluate_non_matching_text_reports_no_labels() {
    let pipeline = SdkPipeline::new(
        vec![Box::new(keyword_signal("language", "python", "(?i)python"))],
        config(4),
    );
    let ctx = ClassificationContext {
        text: "What is the capital of France?".to_string(),
        ..Default::default()
    };

    let result = pipeline.evaluate(&ctx).await;

    assert!(result.errors.is_empty());
    assert_eq!(result.results.len(), 1);
    assert!(result.results[0].labels.is_empty());
}

#[tokio::test]
async fn evaluate_fans_out_across_multiple_signals() {
    let pipeline = SdkPipeline::new(
        vec![
            Box::new(keyword_signal("lang-python", "python", "(?i)python")),
            Box::new(keyword_signal("lang-rust", "rust", "(?i)rust")),
        ],
        config(2),
    );
    let ctx = ClassificationContext {
        text: "Port this Python module to Rust".to_string(),
        ..Default::default()
    };

    let result = pipeline.evaluate(&ctx).await;

    assert!(result.errors.is_empty());
    assert_eq!(result.results.len(), 2);

    let mut names: Vec<&str> = result.results.iter().map(|r| r.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(names, vec!["lang-python", "lang-rust"]);
}

// -- custom signal implemented purely against the SDK's re-exports -----------

struct FailingSignal;

#[hop_top_c12n::async_trait]
impl Signal for FailingSignal {
    async fn evaluate(&self, _ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        Err(SignalError::Inference("boom".to_string()))
    }

    fn name(&self) -> &str {
        "failing"
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Custom
    }
}

#[tokio::test]
async fn evaluate_surfaces_signal_errors() {
    let pipeline = SdkPipeline::new(vec![Box::new(FailingSignal)], config(1));
    let ctx = ClassificationContext::default();

    let result = pipeline.evaluate(&ctx).await;

    assert!(result.results.is_empty());
    assert_eq!(result.errors.len(), 1);
    assert!(
        result.errors[0].to_string().contains("failing"),
        "unexpected error text: {}",
        result.errors[0]
    );
}

#[tokio::test]
async fn evaluate_mixes_successes_and_failures() {
    let pipeline = SdkPipeline::new(
        vec![
            Box::new(keyword_signal("language", "python", "(?i)python")),
            Box::new(FailingSignal),
        ],
        config(4),
    );
    let ctx = ClassificationContext {
        text: "Write a Python function".to_string(),
        ..Default::default()
    };

    let result = pipeline.evaluate(&ctx).await;

    assert_eq!(result.results.len(), 1);
    assert_eq!(result.errors.len(), 1);
    assert_eq!(result.results[0].name, "language");
}

mod common;

use std::time::Duration;

use c12n_core::types::SignalType;
use c12n_core::Pipeline;
use c12n_core::signals::code::CodeContentSignal;
use c12n_core::signals::cost::CostEstimateSignal;
use c12n_core::signals::format::OutputFormatSignal;
use c12n_core::signals::keyword::{
    KeywordRule, KeywordSignal, MatchOperator, MatchStrategy,
};
use c12n_core::signals::toolcall::ToolCallingSignal;

use common::{make_ctx, FailingSignal, MockSignal};

// ---------------------------------------------------------------------------
// 1. Multi-signal pipeline
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multi_signal_pipeline() {
    // Prompt chosen to trigger keyword, format, code signals but NOT
    // tool-calling (no action verbs from the tool-calling verb set).
    // Note: "Write a function" triggers Generate intent (regex
    // requires verb + optional "a " + noun with no words between).
    let prompt =
        "Write a function in Python that sorts numbers and respond in JSON";

    let keyword = KeywordSignal::new(
        "kw",
        vec![KeywordRule {
            label: "python_sort".to_string(),
            patterns: vec![r"(?i)\bpython\b".to_string()],
            operator: MatchOperator::Or,
            strategy: MatchStrategy::Regex,
            threshold: 0.5,
        }],
    );

    let format = OutputFormatSignal::new("fmt");
    let code = CodeContentSignal::new("code");
    let tool = ToolCallingSignal::new("tool");
    let cost = CostEstimateSignal::with_defaults("cost");

    let pipeline = Pipeline::new(
        vec![
            Box::new(keyword),
            Box::new(format),
            Box::new(code),
            Box::new(tool),
            Box::new(cost),
        ],
        8,
        Duration::from_secs(5),
    );

    let result = pipeline.evaluate(&make_ctx(prompt)).await;

    // All 5 signals produced results, no errors
    assert_eq!(
        result.results.len(),
        5,
        "expected 5 results, got {} (errors: {:?})",
        result.results.len(),
        result.errors,
    );
    assert!(
        result.errors.is_empty(),
        "unexpected errors: {:?}",
        result.errors,
    );

    // Helper: find result by signal type
    let find = |st: SignalType| -> &c12n_core::SignalResult {
        result
            .results
            .iter()
            .find(|r| r.signal_type == st)
            .unwrap_or_else(|| panic!("missing signal type {:?}", st))
    };

    // Keyword matched "python"
    let kw = find(SignalType::Keyword);
    assert!(kw.confidence > 0.0, "keyword should match");
    assert!(
        kw.labels.contains(&"python_sort".to_string()),
        "keyword label missing",
    );

    // Format detected JSON (explicit "respond in JSON")
    let fmt = find(SignalType::OutputFormat);
    assert!(
        fmt.labels.contains(&"json".to_string()),
        "format should detect json, got: {:?}",
        fmt.labels,
    );

    // Code detected Python + Generate intent
    let code_r = find(SignalType::CodeContent);
    assert!(
        code_r.labels.contains(&"lang:python".to_string()),
        "code should detect python",
    );
    assert!(
        code_r.labels.contains(&"intent:generate".to_string()),
        "code should detect generate intent, got: {:?}",
        code_r.labels,
    );

    // ToolCalling should NOT fire (no action verbs from tool set)
    let tool_r = find(SignalType::ToolCalling);
    assert_eq!(
        tool_r.confidence, 0.0,
        "tool_calling should not fire on coding prompt, got labels: {:?}",
        tool_r.labels,
    );

    // Cost produced a tier label (first label is overall tier)
    let cost_r = find(SignalType::CostEstimate);
    assert!(
        !cost_r.labels.is_empty(),
        "cost should produce at least one label",
    );
    let tier = &cost_r.labels[0];
    assert!(
        ["micro", "small", "medium", "large"].contains(&tier.as_str()),
        "unexpected cost tier: {}",
        tier,
    );
}

// ---------------------------------------------------------------------------
// 2. Pipeline error handling
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pipeline_error_handling() {
    let ok = MockSignal {
        label: "ok_signal".to_string(),
        signal_type: SignalType::Custom,
        confidence: 0.8,
        delay: Duration::ZERO,
    };
    let bad = FailingSignal {
        label: "bad_signal".to_string(),
    };

    let pipeline = Pipeline::new(
        vec![Box::new(ok), Box::new(bad)],
        4,
        Duration::from_secs(1),
    );

    let result = pipeline.evaluate(&make_ctx("test")).await;

    assert_eq!(result.results.len(), 1, "one signal should succeed");
    assert_eq!(result.errors.len(), 1, "one signal should fail");
    assert_eq!(result.results[0].name, "ok_signal");

    let err_str = format!("{}", result.errors[0]);
    assert!(
        err_str.contains("bad_signal"),
        "error should name the failing signal: {}",
        err_str,
    );
}

// ---------------------------------------------------------------------------
// 3. Pipeline concurrency
// ---------------------------------------------------------------------------

#[tokio::test]
async fn pipeline_concurrency() {
    let signals: Vec<Box<dyn c12n_core::Signal>> = (0..5)
        .map(|i| -> Box<dyn c12n_core::Signal> {
            Box::new(MockSignal {
                label: format!("s{i}"),
                signal_type: SignalType::Custom,
                confidence: 0.7,
                delay: Duration::from_millis(20),
            })
        })
        .collect();

    // max_concurrency=2, 5 signals each 20ms => at least ~60ms serial
    let pipeline = Pipeline::new(signals, 2, Duration::from_secs(5));
    let result = pipeline.evaluate(&make_ctx("concurrent")).await;

    assert_eq!(
        result.results.len(),
        5,
        "all 5 signals should complete",
    );
    assert!(result.errors.is_empty());
    // Concurrency=2 means ceil(5/2)=3 batches * 20ms = 60ms minimum
    assert!(
        result.duration >= Duration::from_millis(50),
        "duration too short for bounded concurrency: {:?}",
        result.duration,
    );
}

// ---------------------------------------------------------------------------
// 4. Empty pipeline
// ---------------------------------------------------------------------------

#[tokio::test]
async fn empty_pipeline() {
    let pipeline = Pipeline::new(vec![], 4, Duration::from_secs(1));
    let result = pipeline.evaluate(&make_ctx("nothing")).await;

    assert!(result.results.is_empty());
    assert!(result.errors.is_empty());
}

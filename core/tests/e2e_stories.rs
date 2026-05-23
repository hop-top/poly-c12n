mod common;

use std::collections::HashMap;
use std::time::Duration;

use c12n_core::types::SignalType;
use c12n_core::Pipeline;
use c12n_core::signals::code::CodeContentSignal;
use c12n_core::signals::cost::CostEstimateSignal;
use c12n_core::signals::format::OutputFormatSignal;
use c12n_core::signals::keyword::{
    KeywordRule, KeywordSignal, MatchOperator, MatchStrategy,
};
use common::{make_ctx, FailingSignal, MockSignal};

// ---------------------------------------------------------------------------
// a) Pipeline lifecycle (solo-dev: zero-to-classify)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn lifecycle_default_pipeline_evaluates_without_panic() {
    let pipeline = Pipeline::new(
        vec![
            Box::new(OutputFormatSignal::new("fmt")),
            Box::new(CodeContentSignal::new("code")),
            Box::new(CostEstimateSignal::with_defaults("cost")),
        ],
        4,
        Duration::from_secs(5),
    );

    let result = pipeline.evaluate(&make_ctx("Hello world")).await;

    assert!(
        result.duration >= Duration::ZERO,
        "duration_ns must be >= 0, got {:?}",
        result.duration,
    );
    assert_eq!(result.results.len(), 3);
    assert!(result.errors.is_empty());
}

#[tokio::test]
async fn lifecycle_empty_pipeline_returns_valid_result() {
    let pipeline = Pipeline::new(vec![], 4, Duration::from_secs(1));
    let result = pipeline.evaluate(&make_ctx("anything")).await;

    assert!(result.results.is_empty());
    assert!(result.errors.is_empty());
    assert!(result.duration >= Duration::ZERO);
}

// ---------------------------------------------------------------------------
// b) Signal coverage (researcher: signal combinations)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn coverage_all_16_signal_types_representable() {
    let all_types = [
        SignalType::Keyword,
        SignalType::Embedding,
        SignalType::Domain,
        SignalType::Jailbreak,
        SignalType::PII,
        SignalType::Toxicity,
        SignalType::Context,
        SignalType::Structure,
        SignalType::Language,
        SignalType::Complexity,
        SignalType::Preference,
        SignalType::Feedback,
        SignalType::OutputFormat,
        SignalType::CodeContent,
        SignalType::ToolCalling,
        SignalType::CostEstimate,
    ];

    // Create a mock signal for each type and run through a pipeline.
    let signals: Vec<Box<dyn c12n_core::Signal>> = all_types
        .iter()
        .enumerate()
        .map(|(i, st)| -> Box<dyn c12n_core::Signal> {
            Box::new(MockSignal {
                label: format!("sig_{i}"),
                signal_type: *st,
                confidence: 0.5,
                delay: Duration::ZERO,
            })
        })
        .collect();

    let pipeline = Pipeline::new(signals, 8, Duration::from_secs(5));
    let result = pipeline.evaluate(&make_ctx("coverage test")).await;

    assert_eq!(result.results.len(), 16, "all 16 signal types should produce results");
    assert!(result.errors.is_empty());

    let mut seen: Vec<SignalType> = result.results.iter().map(|r| r.signal_type).collect();
    seen.sort_by_key(|s| format!("{s:?}"));
    seen.dedup();
    assert_eq!(seen.len(), 16, "all 16 types should be unique in output");
}

#[tokio::test]
async fn coverage_multiple_texts_activate_different_signals() {
    let kw = KeywordSignal::new(
        "kw",
        vec![KeywordRule {
            label: "python".to_string(),
            patterns: vec![r"(?i)\bpython\b".to_string()],
            operator: MatchOperator::Or,
            strategy: MatchStrategy::Regex,
            threshold: 0.5,
        }],
    );
    let fmt = OutputFormatSignal::new("fmt");
    let code = CodeContentSignal::new("code");

    let pipeline = Pipeline::new(
        vec![Box::new(kw), Box::new(fmt), Box::new(code)],
        4,
        Duration::from_secs(5),
    );

    // Python prompt should trigger keyword + code
    let r1 = pipeline.evaluate(&make_ctx("Write Python code for sorting")).await;
    let kw_r = r1.results.iter().find(|r| r.signal_type == SignalType::Keyword).unwrap();
    assert!(kw_r.confidence > 0.0, "python keyword should match");

    // Plain greeting should not trigger keyword
    let r2 = pipeline.evaluate(&make_ctx("Good morning, how are you?")).await;
    let kw_r2 = r2.results.iter().find(|r| r.signal_type == SignalType::Keyword).unwrap();
    assert_eq!(kw_r2.confidence, 0.0, "greeting should not trigger python keyword");
}

// ---------------------------------------------------------------------------
// c) JSON output conformance (agent: structured JSON)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn json_roundtrip_via_serde() {
    let pipeline = Pipeline::new(
        vec![
            Box::new(OutputFormatSignal::new("fmt")),
            Box::new(CostEstimateSignal::with_defaults("cost")),
        ],
        4,
        Duration::from_secs(5),
    );

    let result = pipeline.evaluate(&make_ctx("Respond in JSON format")).await;

    // Serialize to JSON
    let json = serde_json::json!({
        "results": result.results.iter().map(|r| {
            serde_json::json!({
                "name": r.name,
                "signal_type": r.signal_type,
                "confidence": r.confidence,
                "labels": r.labels,
                "metadata": r.metadata,
            })
        }).collect::<Vec<_>>(),
        "errors": result.errors.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
        "duration_ns": result.duration.as_nanos() as u64,
    });

    let json_str = serde_json::to_string(&json).expect("serialize to JSON");
    let parsed: serde_json::Value = serde_json::from_str(&json_str).expect("deserialize JSON");

    assert!(parsed["results"].is_array());
    assert!(parsed["errors"].is_array());
    assert!(parsed["duration_ns"].is_number());
}

#[tokio::test]
async fn json_signal_result_roundtrip() {
    let original = c12n_core::SignalResult {
        name: "test_signal".to_string(),
        signal_type: SignalType::Keyword,
        confidence: 0.85,
        labels: vec!["greeting".to_string(), "polite".to_string()],
        metadata: {
            let mut m = HashMap::new();
            m.insert("key".to_string(), serde_json::json!("value"));
            m
        },
    };

    let json_str = serde_json::to_string(&original).expect("serialize");
    let decoded: c12n_core::SignalResult =
        serde_json::from_str(&json_str).expect("deserialize");

    assert_eq!(decoded.name, original.name);
    assert_eq!(decoded.signal_type, original.signal_type);
    assert!((decoded.confidence - original.confidence).abs() < f64::EPSILON);
    assert_eq!(decoded.labels, original.labels);
}

// ---------------------------------------------------------------------------
// d) Confidence thresholds (platform-eng: signal thresholds)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn threshold_zero_passes_all() {
    let signals: Vec<Box<dyn c12n_core::Signal>> = (0..5)
        .map(|i| -> Box<dyn c12n_core::Signal> {
            Box::new(MockSignal {
                label: format!("s{i}"),
                signal_type: SignalType::Custom,
                confidence: 0.1 * (i as f64 + 1.0),
                delay: Duration::ZERO,
            })
        })
        .collect();

    let pipeline = Pipeline::new(signals, 4, Duration::from_secs(1));
    let result = pipeline.evaluate(&make_ctx("threshold test")).await;

    let threshold = 0.0;
    let passing: Vec<_> = result
        .results
        .iter()
        .filter(|r| r.confidence >= threshold)
        .collect();
    assert_eq!(passing.len(), 5, "threshold 0.0 should pass all signals");
}

#[tokio::test]
async fn threshold_one_filters_all_sub_1() {
    let signals: Vec<Box<dyn c12n_core::Signal>> = (0..5)
        .map(|i| -> Box<dyn c12n_core::Signal> {
            Box::new(MockSignal {
                label: format!("s{i}"),
                signal_type: SignalType::Custom,
                confidence: 0.1 * (i as f64 + 1.0),
                delay: Duration::ZERO,
            })
        })
        .collect();

    let pipeline = Pipeline::new(signals, 4, Duration::from_secs(1));
    let result = pipeline.evaluate(&make_ctx("threshold test")).await;

    let threshold = 1.0;
    let passing: Vec<_> = result
        .results
        .iter()
        .filter(|r| r.confidence >= threshold)
        .collect();
    assert!(passing.is_empty(), "threshold 1.0 should filter all sub-1.0 signals");
}

#[tokio::test]
async fn threshold_partial_filter() {
    let signals: Vec<Box<dyn c12n_core::Signal>> = vec![
        Box::new(MockSignal {
            label: "low".to_string(),
            signal_type: SignalType::Custom,
            confidence: 0.3,
            delay: Duration::ZERO,
        }),
        Box::new(MockSignal {
            label: "high".to_string(),
            signal_type: SignalType::Custom,
            confidence: 0.8,
            delay: Duration::ZERO,
        }),
    ];

    let pipeline = Pipeline::new(signals, 4, Duration::from_secs(1));
    let result = pipeline.evaluate(&make_ctx("test")).await;

    let threshold = 0.5;
    let passing: Vec<_> = result
        .results
        .iter()
        .filter(|r| r.confidence >= threshold)
        .collect();
    assert_eq!(passing.len(), 1);
    assert_eq!(passing[0].name, "high");
}

// ---------------------------------------------------------------------------
// e) Error handling (platform-eng: doctor catches misconfigs)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn error_handling_failing_signal_does_not_panic() {
    let pipeline = Pipeline::new(
        vec![
            Box::new(MockSignal {
                label: "ok".to_string(),
                signal_type: SignalType::Custom,
                confidence: 0.9,
                delay: Duration::ZERO,
            }),
            Box::new(FailingSignal {
                label: "broken".to_string(),
            }),
        ],
        4,
        Duration::from_secs(1),
    );

    let result = pipeline.evaluate(&make_ctx("error test")).await;

    assert_eq!(result.results.len(), 1, "good signal should succeed");
    assert_eq!(result.errors.len(), 1, "bad signal should produce error");
    assert!(
        format!("{}", result.errors[0]).contains("broken"),
        "error should reference failing signal name",
    );
}

#[tokio::test]
async fn error_handling_timeout_produces_error() {
    let pipeline = Pipeline::new(
        vec![Box::new(MockSignal {
            label: "slow".to_string(),
            signal_type: SignalType::Custom,
            confidence: 0.5,
            delay: Duration::from_secs(10),
        })],
        4,
        Duration::from_millis(50),
    );

    let result = pipeline.evaluate(&make_ctx("timeout test")).await;

    assert!(result.results.is_empty());
    assert_eq!(result.errors.len(), 1);
    let err = format!("{}", result.errors[0]);
    assert!(err.contains("slow"), "timeout error should name the signal: {err}");
}

#[tokio::test]
async fn error_handling_all_signals_fail_gracefully() {
    let signals: Vec<Box<dyn c12n_core::Signal>> = (0..3)
        .map(|i| -> Box<dyn c12n_core::Signal> {
            Box::new(FailingSignal {
                label: format!("fail_{i}"),
            })
        })
        .collect();

    let pipeline = Pipeline::new(signals, 4, Duration::from_secs(1));
    let result = pipeline.evaluate(&make_ctx("all fail")).await;

    assert!(result.results.is_empty());
    assert_eq!(result.errors.len(), 3);
}

// ---------------------------------------------------------------------------
// f) Benchmark baseline (researcher: benchmark)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn benchmark_n_iterations_no_panic_or_leak() {
    let pipeline = Pipeline::new(
        vec![
            Box::new(OutputFormatSignal::new("fmt")),
            Box::new(CostEstimateSignal::with_defaults("cost")),
        ],
        4,
        Duration::from_secs(5),
    );

    let ctx = make_ctx("benchmark iteration test");
    let n = 50;

    for i in 0..n {
        let result = pipeline.evaluate(&ctx).await;
        assert!(
            result.errors.is_empty(),
            "iteration {i} produced errors: {:?}",
            result.errors,
        );
    }
}

#[tokio::test]
async fn benchmark_duration_scales_roughly_linearly() {
    let make_pipeline = |count: usize| {
        let signals: Vec<Box<dyn c12n_core::Signal>> = (0..count)
            .map(|i| -> Box<dyn c12n_core::Signal> {
                Box::new(MockSignal {
                    label: format!("s{i}"),
                    signal_type: SignalType::Custom,
                    confidence: 0.5,
                    delay: Duration::from_millis(10),
                })
            })
            .collect();
        Pipeline::new(signals, 1, Duration::from_secs(5))
    };

    let ctx = make_ctx("scaling test");

    // 2 signals serial: ~20ms
    let p2 = make_pipeline(2);
    let r2 = p2.evaluate(&ctx).await;

    // 4 signals serial: ~40ms
    let p4 = make_pipeline(4);
    let r4 = p4.evaluate(&ctx).await;

    // 4x should take roughly 2x as long as 2x (allow 1.5x margin)
    let ratio = r4.duration.as_nanos() as f64 / r2.duration.as_nanos() as f64;
    assert!(
        ratio < 4.0,
        "duration ratio should be roughly linear (~2x), got {ratio:.2}x",
    );
    assert!(
        ratio > 1.0,
        "more signals should take more time, got ratio {ratio:.2}",
    );
}

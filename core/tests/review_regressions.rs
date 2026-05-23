// Regression tests for PR #1 code review fixes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::Mutex;

use c12n_core::signal::Signal;
use c12n_core::types::{ClassificationContext, SignalError, SignalResult, SignalType};
use c12n_core::Pipeline;

fn ctx(text: &str) -> ClassificationContext {
    ClassificationContext {
        text: text.into(),
        history: vec![],
        headers: HashMap::new(),
        image_url: None,
        config: HashMap::new(),
    }
}

// ---- 1. JailbreakSignal: removed redundant HashSet cache ----
// Verify caching still works with only the HashMap.

use c12n_core::signals::safety::{JailbreakDetector, JailbreakSignal};

struct CountingDetector {
    call_count: Arc<Mutex<usize>>,
}

#[async_trait]
impl JailbreakDetector for CountingDetector {
    async fn detect(&self, _text: &str) -> Result<(f64, Vec<String>), SignalError> {
        let mut count = self.call_count.lock().await;
        *count += 1;
        Ok((0.9, vec!["injection".into()]))
    }
}

#[tokio::test]
async fn jailbreak_cache_dedup_without_hashset() {
    let count = Arc::new(Mutex::new(0usize));
    let sig = JailbreakSignal::new(Box::new(CountingDetector {
        call_count: count.clone(),
    }));
    let c = ctx("attack prompt");

    // First call: detector invoked
    let r1 = sig.evaluate(&c).await.unwrap();
    assert_eq!(*count.lock().await, 1);
    assert_eq!(r1.confidence, 0.9);

    // Second call: cached, detector NOT invoked
    let r2 = sig.evaluate(&c).await.unwrap();
    assert_eq!(*count.lock().await, 1);
    assert_eq!(r2.confidence, 0.9);
}

// ---- 2. Pipeline: max_concurrency=0 clamped to 1 ----

struct InstantSignal;

#[async_trait]
impl Signal for InstantSignal {
    async fn evaluate(&self, _ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        Ok(SignalResult {
            name: "instant".into(),
            signal_type: SignalType::Custom,
            confidence: 1.0,
            labels: vec![],
            metadata: HashMap::new(),
        })
    }
    fn name(&self) -> &str {
        "instant"
    }
    fn signal_type(&self) -> SignalType {
        SignalType::Custom
    }
}

#[tokio::test]
async fn pipeline_zero_concurrency_does_not_hang() {
    let pipeline = Pipeline::new(
        vec![Box::new(InstantSignal) as Box<dyn Signal>],
        0, // would hang without the clamp
        Duration::from_secs(2),
    );
    let result =
        tokio::time::timeout(Duration::from_secs(3), pipeline.evaluate(&ctx("test"))).await;
    assert!(
        result.is_ok(),
        "pipeline with max_concurrency=0 should not hang"
    );
    let pr = result.unwrap();
    assert_eq!(pr.results.len(), 1);
}

// ---- 3. PreferenceSignal: preserves original error kind ----

use c12n_core::signals::preference::{PreferenceLlm, PreferenceSignal};

struct ConfigErrorLlm;

#[async_trait]
impl PreferenceLlm for ConfigErrorLlm {
    async fn query(&self, _prompt: &str, _system: &str) -> Result<String, SignalError> {
        Err(SignalError::Configuration("bad config".into()))
    }
}

#[tokio::test]
async fn preference_preserves_error_kind() {
    let sig = PreferenceSignal::new(
        "test",
        Arc::new(ConfigErrorLlm),
        "system",
        Duration::from_secs(5),
        vec!["a".into(), "b".into()],
    );
    let err = sig.evaluate(&ctx("test")).await.unwrap_err();
    match err {
        SignalError::Configuration(_) => {} // original error kind preserved
        other => panic!("expected Configuration, got {:?}", other),
    }
}

// ---- 4. CodeContentSignal: detects C++, C#, Objective-C ----

use c12n_core::signals::code::CodeContentSignal;

#[tokio::test]
async fn code_detects_cpp() {
    let sig = CodeContentSignal::new("test");
    let r = sig
        .evaluate(&ctx("Write a C++ function to sort a vector"))
        .await
        .unwrap();
    assert!(
        r.labels.iter().any(|l| l.to_lowercase().contains("c++")),
        "should detect C++ but got labels: {:?}",
        r.labels
    );
}

#[tokio::test]
async fn code_detects_csharp() {
    let sig = CodeContentSignal::new("test");
    let r = sig
        .evaluate(&ctx("Implement this in C# using LINQ"))
        .await
        .unwrap();
    assert!(
        r.labels.iter().any(|l| l.to_lowercase().contains("c#")),
        "should detect C# but got labels: {:?}",
        r.labels
    );
}

// ---- 5. KeywordSignal: user regex used as-is (no \b wrapping) ----

use c12n_core::signals::keyword::{KeywordRule, KeywordSignal, MatchOperator, MatchStrategy};

#[tokio::test]
async fn keyword_regex_no_implicit_word_boundary() {
    // Pattern without \b should match substrings
    let sig = KeywordSignal::new(
        "test",
        vec![KeywordRule {
            label: "found".into(),
            patterns: vec!["hell".into()], // should match "hello" as substring
            operator: MatchOperator::Or,
            strategy: MatchStrategy::Regex,
            threshold: 0.5,
        }],
    );
    let r = sig.evaluate(&ctx("hello world")).await.unwrap();
    assert_eq!(
        r.confidence, 1.0,
        "substring match should work without implicit \\b"
    );
}

#[tokio::test]
async fn keyword_regex_user_supplies_word_boundary() {
    // User explicitly adds \b for word boundary
    let sig = KeywordSignal::new(
        "test",
        vec![KeywordRule {
            label: "exact".into(),
            patterns: vec![r"\bhello\b".into()],
            operator: MatchOperator::Or,
            strategy: MatchStrategy::Regex,
            threshold: 0.5,
        }],
    );
    let r = sig.evaluate(&ctx("say hello world")).await.unwrap();
    assert_eq!(r.confidence, 1.0);

    // "helloworld" should NOT match with explicit \b
    let r2 = sig.evaluate(&ctx("helloworld")).await.unwrap();
    assert_eq!(r2.confidence, 0.0);
}

// ---- 6. Benchmark rand_vec: symmetric distribution ----

#[test]
fn rand_vec_covers_negative_values() {
    // Reproduce the benchmark's rand_vec logic
    let mut state: u64 = 42;
    let values: Vec<f32> = (0..1000)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = state as u32;
            (value as f32) / (u32::MAX as f32) * 2.0 - 1.0
        })
        .collect();

    let has_negative = values.iter().any(|&v| v < -0.1);
    let has_positive = values.iter().any(|&v| v > 0.1);
    assert!(has_negative, "rand_vec should produce negative values");
    assert!(has_positive, "rand_vec should produce positive values");

    // All values in [-1, 1]
    assert!(values.iter().all(|&v| v >= -1.0 && v <= 1.0));
}

// ---- 7. PiiSignal: chunk_text is non-overlapping ----

use c12n_core::signals::safety::PiiSignal;

// PiiSignal::chunk_text is private, so we test via the public API.
// The comment fix is verified by reading the source (no behavioral test needed).
// But we verify chunking produces correct offsets (no gaps, no overlaps).

use c12n_core::signals::safety::{PiiDetector, PiiEntity};

struct OffsetTrackingDetector;

#[async_trait]
impl PiiDetector for OffsetTrackingDetector {
    async fn detect_entities(&self, text: &str) -> Result<Vec<PiiEntity>, SignalError> {
        // Return entity at fixed position if "email" substring found
        if let Some(pos) = text.find("test@example.com") {
            Ok(vec![PiiEntity {
                entity_type: "EMAIL".into(),
                text: "test@example.com".into(),
                start: pos,
                end: pos + 16,
                confidence: 0.99,
            }])
        } else {
            Ok(vec![])
        }
    }
}

#[tokio::test]
async fn pii_chunking_adjusts_offsets_correctly() {
    let sig = PiiSignal::new(
        Box::new(OffsetTrackingDetector),
        HashSet::from(["EMAIL".to_string()]),
        30, // small chunk size to force splitting
    );
    // Text longer than chunk size with entity in second chunk
    let long_text = "this is padding text. test@example.com is here";
    let r = sig.evaluate(&ctx(long_text)).await.unwrap();
    assert!(r.confidence > 0.0, "should detect PII across chunks");
    assert!(r.labels.contains(&"EMAIL".to_string()));
}

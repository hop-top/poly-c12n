use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

// `Instant` source — see ADR-0001 (Implementation Notes: time-on-wasm).
// Native targets keep `std::time::Instant` (zero-cost, same as before).
// On `wasm32-unknown-unknown`, `std::time::Instant::now()` is stubbed to
// `unreachable!()`, so the `wasm` feature substitutes `instant::Instant`,
// which is backed by `performance.now()` via wasm-bindgen.
//
// NOTE: This fix only addresses the explicit `Instant::now()` call below.
// Tokio's `time::timeout` / `time::sleep` machinery still relies on
// `std::time::Instant` in its driver; the single-threaded `current_thread`
// runtime built in `wasm.rs` happens to schedule signals without invoking
// that path for our short-lived classification workload. The option-B
// follow-up (refactor signal scheduler around `wasm-bindgen-futures`
// timeouts and drop tokio's `time` feature on wasm32) tracks the
// idiomatic fix; see ADR-0001.
#[cfg(feature = "wasm")]
use instant::Instant;
#[cfg(not(feature = "wasm"))]
use std::time::Instant;

use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult};

/// Error produced by the pipeline orchestrator.
#[derive(Debug, Error)]
pub enum PipelineError {
    #[error("signal '{name}' failed: {error}")]
    SignalFailed { name: String, error: SignalError },
    #[error("signal '{name}' timed out")]
    Timeout { name: String },
}

/// Aggregated result of a full pipeline evaluation.
pub struct PipelineResult {
    pub results: Vec<SignalResult>,
    pub errors: Vec<PipelineError>,
    pub duration: Duration,
}

/// Orchestrates parallel signal evaluation with concurrency and timeout limits.
pub struct Pipeline {
    signals: Vec<Arc<dyn Signal>>,
    semaphore: Arc<Semaphore>,
    timeout: Duration,
}

impl Pipeline {
    pub fn new(signals: Vec<Box<dyn Signal>>, max_concurrency: usize, timeout: Duration) -> Self {
        Self {
            signals: signals.into_iter().map(Arc::from).collect(),
            semaphore: Arc::new(Semaphore::new(max_concurrency.max(1))),
            timeout,
        }
    }

    pub fn add_signal(&mut self, signal: Box<dyn Signal>) {
        self.signals.push(Arc::from(signal));
    }

    pub fn signal_count(&self) -> usize {
        self.signals.len()
    }

    /// Fan out all signals in parallel, respecting concurrency and timeout limits.
    pub async fn evaluate(&self, ctx: &ClassificationContext) -> PipelineResult {
        let start = Instant::now();
        let mut join_set = JoinSet::new();

        for signal in &self.signals {
            let name = signal.name().to_string();
            let ctx = ctx.clone();
            let sem = Arc::clone(&self.semaphore);
            let signal = Arc::clone(signal);
            let timeout_dur = self.timeout;

            join_set.spawn(async move {
                let _permit = sem.acquire().await.expect("semaphore closed");

                match tokio::time::timeout(timeout_dur, signal.evaluate(&ctx)).await {
                    Ok(Ok(result)) => Ok(result),
                    Ok(Err(err)) => Err(PipelineError::SignalFailed { name, error: err }),
                    Err(_elapsed) => Err(PipelineError::Timeout { name }),
                }
            });
        }

        let mut results = Vec::new();
        let mut errors = Vec::new();

        while let Some(outcome) = join_set.join_next().await {
            match outcome {
                Ok(Ok(result)) => results.push(result),
                Ok(Err(err)) => errors.push(err),
                Err(join_err) => {
                    errors.push(PipelineError::SignalFailed {
                        name: "unknown".into(),
                        error: SignalError::Internal(format!("task panicked: {join_err}")),
                    });
                }
            }
        }

        PipelineResult {
            results,
            errors,
            duration: start.elapsed(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SignalType;
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct MockSignal {
        label: String,
        delay: Duration,
        fail: bool,
    }

    #[async_trait]
    impl Signal for MockSignal {
        async fn evaluate(
            &self,
            _ctx: &ClassificationContext,
        ) -> Result<SignalResult, SignalError> {
            tokio::time::sleep(self.delay).await;
            if self.fail {
                return Err(SignalError::Internal("mock failure".into()));
            }
            Ok(SignalResult {
                name: self.label.clone(),
                signal_type: SignalType::Custom,
                confidence: 0.9,
                labels: vec![self.label.clone()],
                metadata: HashMap::new(),
            })
        }

        fn name(&self) -> &str {
            &self.label
        }

        fn signal_type(&self) -> SignalType {
            SignalType::Custom
        }
    }

    fn make_ctx() -> ClassificationContext {
        ClassificationContext {
            text: "hello".into(),
            history: vec![],
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    #[tokio::test]
    async fn evaluate_collects_results() {
        let pipeline = Pipeline::new(
            vec![
                Box::new(MockSignal {
                    label: "a".into(),
                    delay: Duration::from_millis(10),
                    fail: false,
                }),
                Box::new(MockSignal {
                    label: "b".into(),
                    delay: Duration::from_millis(10),
                    fail: false,
                }),
            ],
            4,
            Duration::from_secs(1),
        );

        let result = pipeline.evaluate(&make_ctx()).await;
        assert_eq!(result.results.len(), 2);
        assert!(result.errors.is_empty());
    }

    #[tokio::test]
    async fn evaluate_captures_failures() {
        let pipeline = Pipeline::new(
            vec![
                Box::new(MockSignal {
                    label: "ok".into(),
                    delay: Duration::from_millis(5),
                    fail: false,
                }),
                Box::new(MockSignal {
                    label: "bad".into(),
                    delay: Duration::from_millis(5),
                    fail: true,
                }),
            ],
            4,
            Duration::from_secs(1),
        );

        let result = pipeline.evaluate(&make_ctx()).await;
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.errors.len(), 1);
    }

    #[tokio::test]
    async fn evaluate_handles_timeout() {
        let pipeline = Pipeline::new(
            vec![Box::new(MockSignal {
                label: "slow".into(),
                delay: Duration::from_secs(5),
                fail: false,
            })],
            4,
            Duration::from_millis(50),
        );

        let result = pipeline.evaluate(&make_ctx()).await;
        assert!(result.results.is_empty());
        assert_eq!(result.errors.len(), 1);
        assert!(matches!(&result.errors[0], PipelineError::Timeout { name } if name == "slow"));
    }

    #[tokio::test]
    async fn concurrency_is_bounded() {
        // 4 signals, concurrency=1 — they run serially
        let signals: Vec<Box<dyn Signal>> = (0..4)
            .map(|i| -> Box<dyn Signal> {
                Box::new(MockSignal {
                    label: format!("s{i}"),
                    delay: Duration::from_millis(25),
                    fail: false,
                })
            })
            .collect();

        let pipeline = Pipeline::new(signals, 1, Duration::from_secs(2));
        let result = pipeline.evaluate(&make_ctx()).await;

        assert_eq!(result.results.len(), 4);
        // Serial execution of 4 x 25ms >= 100ms
        assert!(result.duration >= Duration::from_millis(90));
    }

    #[tokio::test]
    async fn add_signal_and_count() {
        let mut pipeline = Pipeline::new(vec![], 4, Duration::from_secs(1));
        assert_eq!(pipeline.signal_count(), 0);

        pipeline.add_signal(Box::new(MockSignal {
            label: "x".into(),
            delay: Duration::ZERO,
            fail: false,
        }));
        assert_eq!(pipeline.signal_count(), 1);
    }
}

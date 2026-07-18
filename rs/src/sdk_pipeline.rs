use c12n_core::{ClassificationContext, Pipeline, PipelineResult, Signal};
use tracing::{error, info};

use crate::PipelineConfig;

/// Wraps the engine's [`c12n_core::Pipeline`] with SDK ergonomics:
/// fluent config + structured lifecycle events.
///
/// At v0.1.0-alpha.0 this is a thin wrapper — signals are passed at
/// construction time, same as the engine. Future versions may add a
/// signal-builder DSL gated on `hop-top-kit` integration.
pub struct SdkPipeline {
    inner: Pipeline,
}

impl SdkPipeline {
    pub fn new(signals: Vec<Box<dyn Signal>>, config: PipelineConfig) -> Self {
        info!(
            target: "c12n.pipeline.init.ok",
            signal_count = signals.len(),
            max_concurrency = config.max_concurrency,
            timeout_ms = config.timeout.as_millis() as u64,
            "pipeline initialized"
        );
        Self {
            inner: Pipeline::new(signals, config.max_concurrency, config.timeout),
        }
    }

    pub async fn evaluate(&self, ctx: &ClassificationContext) -> PipelineResult {
        info!(target: "c12n.pipeline.evaluate.start", "evaluate begin");
        let result = self.inner.evaluate(ctx).await;
        if result.errors.is_empty() {
            info!(
                target: "c12n.pipeline.evaluate.ok",
                result_count = result.results.len(),
                duration_ms = result.duration.as_millis() as u64,
                "evaluate complete"
            );
        } else {
            error!(
                target: "c12n.pipeline.evaluate.failed",
                error_count = result.errors.len(),
                "evaluate finished with errors"
            );
        }
        result
    }

    pub fn signal_count(&self) -> usize {
        self.inner.signal_count()
    }
}

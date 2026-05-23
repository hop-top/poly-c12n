//! hop-top-c12n — idiomatic Rust SDK over the c12n classification engine.
//!
//! Re-exports the engine's public types and adds ergonomic conveniences:
//!
//! - [`PipelineConfig`] + [`PipelineConfigBuilder`] for fluent config
//!   construction (the engine itself takes raw `(signals, concurrency,
//!   timeout)` — this SDK wraps that with a typed config struct mirroring
//!   the Go binding's `PipelineConfig`).
//! - Structured lifecycle events via [`tracing`].
//!
//! For the raw FFI surface (cgo / PHP / TS consumers) and the
//! classification algorithms themselves, see the `c12n-core` crate
//! directly.

// Re-exports from the engine.
pub use c12n_core::{
    ClassificationContext, Pipeline, PipelineError, PipelineResult, Signal, SignalError,
    SignalResult, SignalType,
};

mod builder;
pub use builder::{PipelineConfig, PipelineConfigBuilder};

mod sdk_pipeline;
pub use sdk_pipeline::SdkPipeline;

/// Reserved for future kit-rs integration once the kit-rs surface ships
/// logging / output / cli equivalents to kit-go. Tracked in the
/// `kit-rs-surface-followup` track.
pub mod kit {
    // Intentionally empty at v0.1.0-alpha.0.
}

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
//!
//! ---
//!
//! The README is included verbatim below, so its quickstart is compiled
//! and executed by `cargo test` as a doctest — the published example can
//! never drift from the real API.
//!
#![doc = include_str!("../README.md")]

// Re-exports from the engine.
pub use c12n_core::{
    ClassificationContext, Pipeline, PipelineError, PipelineResult, Signal, SignalError,
    SignalResult, SignalType,
};

/// The engine's built-in signal implementations, re-exported so consumers
/// can construct a real pipeline without also depending on `c12n-core`.
///
/// ```
/// use hop_top_c12n::signals::keyword::{
///     KeywordRule, KeywordSignal, MatchOperator, MatchStrategy,
/// };
///
/// let signal = KeywordSignal::new(
///     "kw",
///     vec![KeywordRule {
///         label: "code".to_string(),
///         patterns: vec!["(?i)python".to_string()],
///         operator: MatchOperator::Or,
///         strategy: MatchStrategy::Regex,
///         threshold: 0.5,
///     }],
/// );
/// # let _ = signal;
/// ```
pub use c12n_core::signals;

/// Re-exported so consumers can implement the [`Signal`] trait without
/// taking their own `async-trait` dependency (and risking a version
/// skew with the engine's).
pub use async_trait::async_trait;

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

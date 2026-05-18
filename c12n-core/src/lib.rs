pub mod embedding;
pub mod ffi;
pub mod pipeline;
pub mod prototype;
pub mod signal;
pub mod signals;
pub mod types;

pub use pipeline::{Pipeline, PipelineError, PipelineResult};
pub use signal::Signal;
pub use types::{ClassificationContext, SignalError, SignalResult, SignalType};

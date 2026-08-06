pub mod chain;
pub mod embedding;
// The C ABI in `ffi.rs` targets cgo + PyO3. It is not compiled for
// `wasm32` because its multi-threaded `tokio::runtime::Runtime::new()`
// and C string plumbing are not meaningful in that environment. The
// parallel `wasm` module below exposes a `#[wasm_bindgen]` surface
// using a single-threaded executor instead. See ADR-0001.
#[cfg(not(target_arch = "wasm32"))]
pub mod ffi;
pub mod pipeline;
pub mod prototype;
pub mod registry;
pub mod rt;
pub mod signal;
pub mod signals;
pub mod types;

#[cfg(all(feature = "wasm", target_arch = "wasm32"))]
pub mod wasm;

pub use chain::{
    Chain, ChainOutcome, ChainProvenance, ChainStrategy, ChainableTier, DEFAULT_ESCALATE_THRESHOLD,
    PROVENANCE_METADATA_KEY,
};
pub use pipeline::{Pipeline, PipelineError, PipelineResult};
pub use signal::Signal;
pub use types::{ClassificationContext, SignalError, SignalResult, SignalType};

//! WebAssembly bindings for c12n-core (consumed by `c12n-ts`).
//!
//! Parallel surface to `ffi.rs`. Where `ffi.rs` exposes the pipeline over the
//! C ABI for cgo + PyO3 callers (multi-threaded tokio runtime, JSON-as-CString
//! in/out), this module exposes `#[wasm_bindgen]` types that JS/TS callers
//! drive directly via `serde-wasm-bindgen`. The tokio runtime is single-threaded
//! because wasm32 lacks thread primitives.
//!
//! See ADR-0001 (`docs/adr/0001-c12n-ts-wasm-binding.md`) for full context.
//!
//! # JS usage shape
//!
//! ```js
//! import init, { Pipeline } from "@hop-top/c12n";
//! await init();
//! const p = new Pipeline({ max_concurrency: 8, timeout_ms: 5000 });
//! const out = p.evaluate({ text: "hello", history: [], headers: {}, config: {} });
//! // out is a JSON string with { results, errors, duration_ms }
//! ```

use std::time::Duration;

use serde::{Deserialize, Serialize};
use wasm_bindgen::prelude::*;

use crate::pipeline::Pipeline as InnerPipeline;
use crate::types::ClassificationContext;

/// Mirror of `ffi.rs`'s PipelineConfig for the wasm surface.
#[derive(Deserialize)]
struct WasmPipelineConfig {
    #[serde(default = "default_concurrency")]
    max_concurrency: usize,
    #[serde(default = "default_timeout_ms")]
    timeout_ms: u64,
}

fn default_concurrency() -> usize {
    8
}

fn default_timeout_ms() -> u64 {
    5000
}

/// JSON-serializable result mirroring `FfiResult` so JS callers see the same
/// shape regardless of which binding they go through.
#[derive(Serialize)]
struct WasmResult {
    results: Vec<WasmSignalResult>,
    errors: Vec<String>,
    duration_ms: u64,
}

#[derive(Serialize)]
struct WasmSignalResult {
    name: String,
    signal_type: crate::types::SignalType,
    confidence: f64,
    labels: Vec<String>,
    metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// Install `console_error_panic_hook` once. JS callers may invoke this for
/// nicer Rust panic surfacing in browser/Node consoles.
#[wasm_bindgen(js_name = setPanicHook)]
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

/// `#[wasm_bindgen]` Pipeline wrapper.
///
/// Holds the inner `Pipeline` plus a single-threaded tokio runtime. The runtime
/// is built with `Builder::new_current_thread().enable_all()` because wasm32
/// has no thread primitives; classification signals run sequentially within
/// one logical context.
#[wasm_bindgen]
pub struct Pipeline {
    inner: InnerPipeline,
    runtime: tokio::runtime::Runtime,
}

#[wasm_bindgen]
impl Pipeline {
    /// Construct a pipeline from a JS config object.
    ///
    /// `config` is a JS object like `{ max_concurrency: number, timeout_ms: number }`.
    /// Both fields are optional; defaults match `ffi.rs` (8 / 5000ms).
    #[wasm_bindgen(constructor)]
    pub fn new(config: JsValue) -> Result<Pipeline, JsValue> {
        let cfg: WasmPipelineConfig = if config.is_undefined() || config.is_null() {
            WasmPipelineConfig {
                max_concurrency: default_concurrency(),
                timeout_ms: default_timeout_ms(),
            }
        } else {
            serde_wasm_bindgen::from_value(config)
                .map_err(|e| JsValue::from_str(&format!("invalid config: {e}")))?
        };

        let runtime = build_runtime()
            .map_err(|e| JsValue::from_str(&format!("failed to build runtime: {e}")))?;

        let inner = InnerPipeline::new(
            vec![],
            cfg.max_concurrency,
            Duration::from_millis(cfg.timeout_ms),
        );

        Ok(Pipeline { inner, runtime })
    }

    /// Evaluate a classification context. Accepts a JS object matching
    /// `ClassificationContext` (`text`, `history`, `headers`, `image_url`,
    /// `config`). Returns a JSON string with `{ results, errors, duration_ms }`
    /// matching the C ABI's `FfiResult` shape.
    pub fn evaluate(&self, ctx: JsValue) -> Result<String, JsValue> {
        let context: ClassificationContext = serde_wasm_bindgen::from_value(ctx)
            .map_err(|e| JsValue::from_str(&format!("invalid context: {e}")))?;

        let result = self.runtime.block_on(self.inner.evaluate(&context));

        let out = WasmResult {
            results: result
                .results
                .into_iter()
                .map(|r| WasmSignalResult {
                    name: r.name,
                    signal_type: r.signal_type,
                    confidence: r.confidence,
                    labels: r.labels,
                    metadata: r.metadata,
                })
                .collect(),
            errors: result.errors.iter().map(|e| e.to_string()).collect(),
            duration_ms: result.duration.as_millis() as u64,
        };

        serde_json::to_string(&out)
            .map_err(|e| JsValue::from_str(&format!("failed to serialize result: {e}")))
    }

    /// Number of signals currently registered on the pipeline.
    #[wasm_bindgen(js_name = signalCount)]
    pub fn signal_count(&self) -> usize {
        self.inner.signal_count()
    }
}

/// Build a tokio runtime appropriate for the current build configuration.
///
/// Under `feature = "wasm"`, this uses `Builder::new_current_thread()` because
/// wasm32 has no thread primitives. Non-wasm callers should keep using
/// `tokio::runtime::Runtime::new()` directly (see `ffi.rs`).
fn build_runtime() -> std::io::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
}

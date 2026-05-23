//! C FFI layer for c12n-core pipeline.
//!
//! Exposes the classification pipeline via C ABI with JSON in/out.
//! All functions use `extern "C"` + `#[no_mangle]`.
//!
//! # Lifecycle
//!
//! ```c
//! // Create pipeline (empty, no signals)
//! void *p = c12n_pipeline_new("{\"max_concurrency\":8,\"timeout_ms\":5000}");
//!
//! // Evaluate context
//! char *json = c12n_pipeline_evaluate(p, "{\"text\":\"hello\",\"history\":[],\"headers\":{}}");
//!
//! // Free result string
//! c12n_result_free(json);
//!
//! // Free pipeline
//! c12n_pipeline_free(p);
//! ```

use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::pipeline::Pipeline;
use crate::types::ClassificationContext;

/// Wrapper holding the pipeline and its tokio runtime.
struct FfiPipeline {
    pipeline: Pipeline,
    runtime: tokio::runtime::Runtime,
}

/// Config accepted by `c12n_pipeline_new`.
#[derive(Deserialize)]
struct PipelineConfig {
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

/// JSON-serializable result returned by `c12n_pipeline_evaluate`.
#[derive(Serialize)]
struct FfiResult {
    results: Vec<FfiSignalResult>,
    errors: Vec<String>,
    duration_ms: u64,
}

#[derive(Serialize)]
struct FfiSignalResult {
    name: String,
    signal_type: crate::types::SignalType,
    confidence: f64,
    labels: Vec<String>,
    metadata: std::collections::HashMap<String, serde_json::Value>,
}

/// JSON error envelope.
#[derive(Serialize)]
struct FfiError {
    error: String,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert a raw C string to `&str`. Returns `None` on null or invalid UTF-8.
unsafe fn cstr_to_str<'a>(ptr: *const c_char) -> Option<&'a str> {
    if ptr.is_null() {
        return None;
    }
    unsafe { CStr::from_ptr(ptr) }.to_str().ok()
}

/// Serialize `val` to a heap-allocated C string. Caller must free via
/// `c12n_result_free`.
fn to_c_json<T: Serialize>(val: &T) -> *mut c_char {
    match serde_json::to_string(val) {
        Ok(s) => CString::new(s)
            .map(CString::into_raw)
            .unwrap_or(ptr::null_mut()),
        Err(_) => ptr::null_mut(),
    }
}

/// Return a JSON error string.
fn error_json(msg: &str) -> *mut c_char {
    to_c_json(&FfiError {
        error: msg.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Public C API
// ---------------------------------------------------------------------------

/// Create a new pipeline. Returns opaque pointer.
///
/// `config_json`: JSON string with `max_concurrency` and `timeout_ms`.
/// Returns null on error.
#[no_mangle]
pub extern "C" fn c12n_pipeline_new(config_json: *const c_char) -> *mut c_void {
    let json = match unsafe { cstr_to_str(config_json) } {
        Some(s) => s,
        None => return ptr::null_mut(),
    };

    let cfg: PipelineConfig = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(_) => return ptr::null_mut(),
    };

    let runtime = match tokio::runtime::Runtime::new() {
        Ok(rt) => rt,
        Err(_) => return ptr::null_mut(),
    };

    let pipeline = Pipeline::new(
        vec![],
        cfg.max_concurrency,
        Duration::from_millis(cfg.timeout_ms),
    );

    let boxed = Box::new(FfiPipeline { pipeline, runtime });
    Box::into_raw(boxed) as *mut c_void
}

/// Evaluate a context through the pipeline.
///
/// `pipeline`: pointer from `c12n_pipeline_new`.
/// `context_json`: JSON string matching `ClassificationContext`.
///
/// Returns a heap-allocated JSON string. Caller must free with
/// `c12n_result_free`. Returns a JSON error object on failure.
#[no_mangle]
pub extern "C" fn c12n_pipeline_evaluate(
    pipeline: *const c_void,
    context_json: *const c_char,
) -> *mut c_char {
    if pipeline.is_null() {
        return error_json("null pipeline pointer");
    }

    let json = match unsafe { cstr_to_str(context_json) } {
        Some(s) => s,
        None => return error_json("null or invalid context_json"),
    };

    let ctx: ClassificationContext = match serde_json::from_str(json) {
        Ok(c) => c,
        Err(e) => return error_json(&format!("invalid context JSON: {e}")),
    };

    let ffi = unsafe { &*(pipeline as *const FfiPipeline) };

    let result = ffi.runtime.block_on(ffi.pipeline.evaluate(&ctx));

    let out = FfiResult {
        results: result
            .results
            .into_iter()
            .map(|r| FfiSignalResult {
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

    to_c_json(&out)
}

/// Free a pipeline created by `c12n_pipeline_new`.
#[no_mangle]
pub extern "C" fn c12n_pipeline_free(pipeline: *mut c_void) {
    if pipeline.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(pipeline as *mut FfiPipeline));
    }
}

/// Free a result string returned by `c12n_pipeline_evaluate`.
#[no_mangle]
pub extern "C" fn c12n_result_free(result: *mut c_char) {
    if result.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(result));
    }
}

/// Return the JSON string as-is (identity; included for API completeness).
#[no_mangle]
pub extern "C" fn c12n_result_json(result: *const c_char) -> *const c_char {
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(s: &str) -> CString {
        CString::new(s).unwrap()
    }

    #[test]
    fn roundtrip_create_evaluate_free() {
        let cfg = c(r#"{"max_concurrency":2,"timeout_ms":1000}"#);
        let p = c12n_pipeline_new(cfg.as_ptr());
        assert!(!p.is_null());

        let ctx = c(r#"{"text":"hi","history":[],"headers":{},"config":{}}"#);
        let res = c12n_pipeline_evaluate(p, ctx.as_ptr());
        assert!(!res.is_null());

        let json_str = unsafe { CStr::from_ptr(res) }.to_str().unwrap();
        let val: serde_json::Value = serde_json::from_str(json_str).unwrap();
        assert!(val["results"].is_array());
        assert!(val["errors"].is_array());
        assert!(val["duration_ms"].is_number());

        c12n_result_free(res);
        c12n_pipeline_free(p);
    }

    #[test]
    fn null_pipeline_returns_error() {
        let ctx = c(r#"{"text":"x","history":[],"headers":{},"config":{}}"#);
        let res = c12n_pipeline_evaluate(ptr::null() as *const c_void, ctx.as_ptr());
        assert!(!res.is_null());

        let json_str = unsafe { CStr::from_ptr(res) }.to_str().unwrap();
        assert!(json_str.contains("null pipeline pointer"));

        c12n_result_free(res);
    }

    #[test]
    fn null_config_returns_null() {
        let p = c12n_pipeline_new(ptr::null());
        assert!(p.is_null());
    }

    #[test]
    fn invalid_json_returns_null() {
        let bad = c("not json");
        let p = c12n_pipeline_new(bad.as_ptr());
        assert!(p.is_null());
    }

    #[test]
    fn free_null_is_noop() {
        c12n_pipeline_free(ptr::null_mut());
        c12n_result_free(ptr::null_mut());
    }
}

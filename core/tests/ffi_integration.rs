use std::ffi::{c_char, c_void, CStr, CString};
use std::ptr;

use c12n_core::ffi::{
    c12n_pipeline_evaluate, c12n_pipeline_free, c12n_pipeline_new,
    c12n_result_free,
};

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

// ---------------------------------------------------------------------------
// 1. Round-trip: create -> evaluate -> verify -> free
// ---------------------------------------------------------------------------

#[test]
fn roundtrip_create_evaluate_free() {
    let cfg = c(r#"{"max_concurrency":4,"timeout_ms":2000}"#);
    let p = c12n_pipeline_new(cfg.as_ptr());
    assert!(!p.is_null(), "pipeline creation should succeed");

    let ctx = c(r#"{"text":"Hello world","history":[],"headers":{},"config":{}}"#);
    let res = c12n_pipeline_evaluate(p, ctx.as_ptr());
    assert!(!res.is_null(), "evaluate should return non-null");

    // Parse the JSON result
    let json_str = unsafe { CStr::from_ptr(res) }.to_str().unwrap();
    let val: serde_json::Value =
        serde_json::from_str(json_str).expect("result must be valid JSON");

    assert!(val["results"].is_array(), "results must be array");
    assert!(val["errors"].is_array(), "errors must be array");
    assert!(val["duration_ms"].is_number(), "duration_ms must be number");

    c12n_result_free(res);
    c12n_pipeline_free(p);
}

// ---------------------------------------------------------------------------
// 2. Null safety
// ---------------------------------------------------------------------------

#[test]
fn null_pipeline_pointer_returns_error_json() {
    let ctx = c(r#"{"text":"x","history":[],"headers":{},"config":{}}"#);
    let res = c12n_pipeline_evaluate(
        ptr::null() as *const c_void,
        ctx.as_ptr(),
    );
    assert!(!res.is_null(), "should return error JSON, not null");

    let json_str = unsafe { CStr::from_ptr(res) }.to_str().unwrap();
    assert!(
        json_str.contains("null pipeline pointer"),
        "error should mention null pointer: {}",
        json_str,
    );

    c12n_result_free(res);
}

#[test]
fn null_config_returns_null() {
    let p = c12n_pipeline_new(ptr::null());
    assert!(p.is_null(), "null config should yield null pipeline");
}

#[test]
fn null_context_returns_error_json() {
    let cfg = c(r#"{"max_concurrency":2,"timeout_ms":1000}"#);
    let p = c12n_pipeline_new(cfg.as_ptr());
    assert!(!p.is_null());

    let res = c12n_pipeline_evaluate(p, ptr::null() as *const c_char);
    assert!(!res.is_null(), "should return error JSON");

    let json_str = unsafe { CStr::from_ptr(res) }.to_str().unwrap();
    assert!(
        json_str.contains("null") || json_str.contains("invalid"),
        "error should mention null/invalid context: {}",
        json_str,
    );

    c12n_result_free(res);
    c12n_pipeline_free(p);
}

// ---------------------------------------------------------------------------
// 3. Invalid JSON
// ---------------------------------------------------------------------------

#[test]
fn invalid_config_json_returns_null() {
    let bad = c("not valid json");
    let p = c12n_pipeline_new(bad.as_ptr());
    assert!(p.is_null(), "bad config JSON should yield null");
}

#[test]
fn invalid_context_json_returns_error() {
    let cfg = c(r#"{"max_concurrency":2,"timeout_ms":1000}"#);
    let p = c12n_pipeline_new(cfg.as_ptr());
    assert!(!p.is_null());

    let bad_ctx = c("{broken json");
    let res = c12n_pipeline_evaluate(p, bad_ctx.as_ptr());
    assert!(!res.is_null(), "should return error JSON");

    let json_str = unsafe { CStr::from_ptr(res) }.to_str().unwrap();
    let val: serde_json::Value =
        serde_json::from_str(json_str).expect("error must be valid JSON");
    assert!(
        val["error"].is_string(),
        "error envelope should have 'error' field",
    );

    c12n_result_free(res);
    c12n_pipeline_free(p);
}

// ---------------------------------------------------------------------------
// 4. Free safety — null free is a no-op
// ---------------------------------------------------------------------------

#[test]
fn free_null_pipeline_is_noop() {
    c12n_pipeline_free(ptr::null_mut());
    // No crash = pass
}

#[test]
fn free_null_result_is_noop() {
    c12n_result_free(ptr::null_mut());
    // No crash = pass
}

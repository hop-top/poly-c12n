---
status: partial
bindings:
  go: partial
  python: shipped
  typescript: shipped
  php: shipped
personas: [cost-control-startup, middleware-developer]
priority: P1
---

# US-0007: Parse JSON from FFI without panic

As a tool author embedding c12n in a long-running process, I want JSON
parsing of FFI responses to fail gracefully on bad input — never panic,
never bring down my service.

> **Status: partial.** The no-panic guarantee holds. The "round-trip
> preserves all fields" guarantee does not: since PR #42, Go cannot
> parse native FFI output at all, because the two sides disagree on the
> shape of `errors`. Two integration tests in this story's own file
> fail. See [Native round-trip is
> broken](#native-round-trip-is-broken).

## Use this when

- Middleware sees a malformed classification result.
- FFI boundary returns truncated / corrupted JSON.
- Test fixtures contain edge-case JSON shapes.

## Result

`ParseResult(raw string) (*PipelineResult, error)`
([`go/result.go:17`](../../go/result.go)) — note it takes a `string`,
not `[]byte`, and returns a **pointer**. Invalid JSON returns the
underlying `encoding/json` error. It never panics.

There is no `c12n.ErrInvalidJSON` sentinel. `errors.Is` against it does
not compile. Check `err != nil` and inspect with
`errors.As(&json.SyntaxError{})` / `*json.UnmarshalTypeError` if you
need to distinguish causes.

## Steps

```go
ctx := c12n.ClassificationContext{
    Text:    "...",
    History: []string{"previous turn"},
    Headers: map[string]string{"X-Tenant": "acme"},
    Config:  map[string]any{"mode": "fast"},
}

raw, err := pipeline.Evaluate(ctx)
if err != nil {
    return err
}

result, err := c12n.ParseResult(raw)
if err != nil {
    // Malformed or shape-mismatched payload — degrade, don't crash.
    return fallbackRoute()
}
```

`ClassificationContext` fields are `Text`, `History`, `Headers`,
`ImageURL`, `Config` ([`go/c12n.go:20-26`](../../go/c12n.go)). There
are no `Domain` or `Metadata` fields — arbitrary per-request data goes
in `Config`.

## Verify

Stub mode — parsing without FFI. All three pass:

```bash
cd go && CGO_ENABLED=0 go test -run TestE2E_ParseResult_InvalidJSON_Error ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_ClassificationContext_FullRoundTrip ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_ClassificationContext_MinimalFields ./...
```

cgo mode — the crate is `hop-top-c12n-core`, and the integration tests
need both build tags plus the library path:

```bash
cargo build -p hop-top-c12n-core
cd go && CGO_ENABLED=1 \
  CGO_LDFLAGS="-L$(cd .. && pwd)/target/debug" \
  DYLD_LIBRARY_PATH="$(cd .. && pwd)/target/debug" \
  go test -tags "c12n_native integration" \
  -run TestIntegration_JSONRoundTripThroughFFI ./...
```

This command **fails today**. That is the honest current state, not a
setup error on your part.

## Native round-trip is broken

Go decodes `errors` as structs; the FFI encodes it as strings.

```go
// go/result.go:10
Errors []PipelineError `json:"errors"`
```

```rust
// core/src/ffi.rs:57
errors: Vec<String>,
// core/src/ffi.rs:198
errors: result.errors.iter().map(|e| e.to_string()).collect(),
```

Before PR #42 an unconfigured pipeline returned `errors: []`, which
unmarshals into `[]PipelineError` happily — so the mismatch was latent
and no test caught it. PR #42 added `PipelineError::NoSignals`
([`core/src/pipeline.rs:24-33`](../../core/src/pipeline.rs)), so every
native evaluation now returns exactly one error string and parsing
fails:

```
--- FAIL: TestIntegration_PipelineEmptyResult (0.00s)
    integration_test.go:47: ParseResult: json: cannot unmarshal string
    into Go struct field PipelineResult.errors of type c12n.PipelineError
--- FAIL: TestIntegration_JSONRoundTripThroughFFI (0.00s)
    integration_test.go:108: ParseResult: json: cannot unmarshal string
    into Go struct field PipelineResult.errors of type c12n.PipelineError
FAIL
```

Python, TypeScript and PHP treat `errors` as strings and were updated
alongside #42
([`ts/test/pipeline.integration.test.ts:92`](../../ts/test/pipeline.integration.test.ts),
[`php/tests/PipelineFfiIntegrationTest.php:119`](../../php/tests/PipelineFfiIntegrationTest.php)).
Go is the only binding left behind.

Separately, `TestIntegration_JSONRoundTripThroughFFI` would not have
substantiated the "preserves all fields" claim even when it passed: it
re-marshals the *result* and asserts `DurationMs >= 0`. It never
compares the outbound context against anything, because the FFI does
not echo the context back. Field preservation is covered only by the
stub-mode `TestE2E_ClassificationContext_FullRoundTrip`, which
marshals and unmarshals in Go without crossing the C ABI.

## How it works

The FFI boundary is JSON-only — no struct sharing across the C ABI. Go
marshals `ClassificationContext` to JSON, `normalizeContext`
([`go/c12n.go:32`](../../go/c12n.go)) replaces nil slices/maps with
empty values so the wire never carries `null` for collections, and the
bytes go to `c12n_pipeline_evaluate`. The returned C string is copied
into Go and unmarshalled.

Failures on the Rust side come back as a JSON error envelope
(`{"error": "..."}`), not as a null pointer
([`core/src/ffi.rs:104`](../../core/src/ffi.rs)). Go does not currently
detect that envelope — it unmarshals into `PipelineResult`, yielding a
zero-valued result rather than an error.

## What this story needs to reach `shipped`

1. Agree one wire shape for `errors` and align Go with it.
2. Detect the `{"error": ...}` envelope in `Evaluate` instead of
   silently producing an empty result.
3. A native test asserting a non-empty `errors` array parses.

## Tests

- [`go/e2e_test.go:TestE2E_ParseResult_InvalidJSON_Error`](../../go/e2e_test.go)
  — empty / garbage / truncated / wrong-type. Passes.
- [`go/e2e_test.go:TestE2E_ClassificationContext_FullRoundTrip`](../../go/e2e_test.go)
  — Go-only marshal/unmarshal; does not cross the C ABI. Passes.
- [`go/e2e_test.go:TestE2E_ClassificationContext_MinimalFields`](../../go/e2e_test.go) — passes.
- [`go/integration_test.go:TestIntegration_JSONRoundTripThroughFFI`](../../go/integration_test.go)
  — **fails** under `-tags "c12n_native integration"`.
- [`go/integration_test.go:TestIntegration_PipelineEmptyResult`](../../go/integration_test.go)
  — **fails**, same cause.
- [`go/xrr_adapter_test.go`](../../go/xrr_adapter_test.go) — cassette
  record/replay; operates on fixture strings, not the FFI.

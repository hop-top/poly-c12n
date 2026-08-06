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

> **Status: partial.** The no-panic guarantee holds, and the native
> round-trip is fixed: both integration tests in this story's file now
> pass. One gap remains — Go still does not detect the Rust side's
> `{"error": ...}` envelope, so a failed evaluation silently decodes
> into a zero-valued result. See [Error envelopes decode
> silently](#error-envelopes-decode-silently).

## Use this when

- Middleware sees a malformed classification result.
- FFI boundary returns truncated / corrupted JSON.
- Test fixtures contain edge-case JSON shapes.

## Result

`ParseResult(raw string) (*PipelineResult, error)`
([`go/result.go`](../../go/result.go)) — note it takes a `string`,
not `[]byte`, and returns a **pointer**. Invalid JSON returns the
underlying `encoding/json` error. It never panics.

There is no `c12n.ErrInvalidJSON` sentinel. `errors.Is` against it does
not compile. Check `err != nil` and inspect with
`errors.As(&json.SyntaxError{})` / `*json.UnmarshalTypeError` if you
need to distinguish causes.

## The native round-trip works

Go's `errors` field is now `[]PipelineError` where
`type PipelineError string` ([`go/types.go`](../../go/types.go)),
matching `FfiResult.errors: Vec<String>`
([`core/src/ffi.rs`](../../core/src/ffi.rs)) and the PHP / TypeScript
bindings. The shape disagreement that broke every native evaluation
after PR #42 is resolved.

```console
$ cargo build -p hop-top-c12n-core --release
$ cd go && CGO_ENABLED=1 \
    CGO_LDFLAGS="-L$(cd .. && pwd)/target/release" \
    DYLD_LIBRARY_PATH="$(cd .. && pwd)/target/release" \
    go test -tags "c12n_native integration" -count=1 ./...
ok  	hop.top/c12n	0.387s
ok  	hop.top/c12n/cmd/c12n	0.365s
```

CI runs this combination too, so the mismatch cannot silently return.

One caveat carried forward from the previous revision:
`TestIntegration_JSONRoundTripThroughFFI` does not substantiate a
"preserves all fields" claim. It re-marshals the *result* and asserts
`DurationMs >= 0`; it never compares the outbound context against
anything, because the FFI does not echo the context back. Field
preservation is covered only by the stub-mode
`TestE2E_ClassificationContext_FullRoundTrip`, which marshals and
unmarshals in Go without crossing the C ABI. This story therefore
claims no-panic parsing, not field preservation.

## Error envelopes decode silently

Failures on the Rust side come back as a JSON error envelope
(`{"error": "..."}`), not as a null pointer
([`core/src/ffi.rs`](../../core/src/ffi.rs)). Go does not detect that
envelope — there is no `"error"` key check anywhere in
[`go/`](../../go/). `ParseResult` unmarshals it into `PipelineResult`,
where no field matches, yielding a zero-valued result:
empty `Results`, empty `Errors`, `DurationMs` 0.

A caller then sees `HasErrors() == false` on what was actually a
failure. That is the opposite of the degrade-don't-crash contract this
story promises, and it is the reason the story is not `shipped`.

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
    // Malformed payload — degrade, don't crash.
    return fallbackRoute()
}

// Guard against the silent-envelope case until it is handled upstream.
if len(result.Results) == 0 && !result.HasErrors() {
    return fallbackRoute()
}
```

`ClassificationContext` fields are `Text`, `History`, `Headers`,
`ImageURL`, `Config` ([`go/c12n.go`](../../go/c12n.go)). There
are no `Domain` or `Metadata` fields — arbitrary per-request data goes
in `Config`.

## Verify

Stub mode — parsing without FFI:

```bash
cd go && CGO_ENABLED=0 go test -run TestE2E_ParseResult_InvalidJSON_Error ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_ClassificationContext_FullRoundTrip ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_ClassificationContext_MinimalFields ./...
```

cgo mode — the crate is `hop-top-c12n-core`, and the integration tests
need both build tags plus the library path:

```bash
cargo build -p hop-top-c12n-core --release
cd go && CGO_ENABLED=1 \
  CGO_LDFLAGS="-L$(cd .. && pwd)/target/release" \
  DYLD_LIBRARY_PATH="$(cd .. && pwd)/target/release" \
  go test -tags "c12n_native integration" \
  -run TestIntegration_JSONRoundTripThroughFFI ./...
```

All pass, including the cgo command — which failed in the previous
revision of this story.

## How it works

The FFI boundary is JSON-only — no struct sharing across the C ABI. Go
marshals `ClassificationContext` to JSON, `normalizeContext`
([`go/c12n.go`](../../go/c12n.go)) replaces nil slices/maps with
empty values so the wire never carries `null` for collections, and the
bytes go to `c12n_pipeline_evaluate`. The returned C string is copied
into Go and unmarshalled.

## What this story needs to reach `shipped`

1. Detect the `{"error": ...}` envelope in `Evaluate` and return it as
   a Go error instead of an empty result.
2. A test feeding that envelope through `Evaluate` and asserting the
   error surfaces.

## Tests

- [`go/e2e_test.go:TestE2E_ParseResult_InvalidJSON_Error`](../../go/e2e_test.go)
  — empty / garbage / truncated / wrong-type. Passes.
- [`go/e2e_test.go:TestE2E_ClassificationContext_FullRoundTrip`](../../go/e2e_test.go)
  — Go-only marshal/unmarshal; does not cross the C ABI. Passes.
- [`go/e2e_test.go:TestE2E_ClassificationContext_MinimalFields`](../../go/e2e_test.go) — passes.
- [`go/integration_test.go:TestIntegration_JSONRoundTripThroughFFI`](../../go/integration_test.go)
  — passes under `-tags "c12n_native integration"`.
- [`go/integration_test.go:TestIntegration_PipelineEmptyResult`](../../go/integration_test.go)
  — passes; asserts on the decoded `errors` array.
- [`go/xrr_adapter_test.go`](../../go/xrr_adapter_test.go) — cassette
  record/replay; operates on fixture strings, not the FFI.

No test covers the `{"error": ...}` envelope path.

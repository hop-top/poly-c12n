---
status: shipped
personas: [cost-control-startup, middleware-developer]
priority: P1
---

# US-0007: Parse JSON from FFI without panic

As a tool author embedding c12n in a long-running process, I want
JSON parsing of FFI responses to fail gracefully on bad input —
never panic, never bring down my service.

## Use this when

- Middleware sees a malformed classification result.
- FFI boundary returns truncated / corrupted JSON.
- Test fixtures contain edge-case JSON shapes.

## Result

`ParseResult(jsonBytes)` returns `(PipelineResult, error)`. Invalid
JSON returns a typed error; never panics. Round-tripping a valid
`ClassificationContext` through marshal → FFI → unmarshal preserves
all fields.

## Steps

```go
ctx := c12n.ClassificationContext{
    Text:     "...",
    Domain:   "code",
    Metadata: map[string]any{"tenant": "acme"},
}

// Outbound: ctx → JSON → Rust core
// Return:   Rust core → JSON → PipelineResult

result, err := pipeline.Evaluate(ctx)
if err != nil {
    if errors.Is(err, c12n.ErrInvalidJSON) {
        // log + degrade gracefully
        return fallbackRoute()
    }
    return err
}
```

## Verify

```bash
# Stub mode — parsing without FFI
CGO_ENABLED=0 go test -run TestE2E_ParseResult_InvalidJSON_Error ./...
CGO_ENABLED=0 go test -run TestE2E_ClassificationContext_FullRoundTrip ./...
CGO_ENABLED=0 go test -run TestE2E_ClassificationContext_MinimalFields ./...

# cgo mode — full FFI roundtrip via Rust core
cargo build -p c12n-core
CGO_ENABLED=1 CGO_LDFLAGS="-L$(pwd)/target/debug" \
  DYLD_LIBRARY_PATH="$(pwd)/target/debug" \
  go test -run TestIntegration_JSONRoundTripThroughFFI ./...
```

## How it works

c12n's FFI boundary is JSON-only — no struct sharing across the C
ABI. The Go side marshals `ClassificationContext` to JSON, hands
the bytes to the Rust core (in cgo builds), and unmarshals the
returned bytes into `PipelineResult`. All marshal errors return
typed errors via `PipelineError`.

## Tests

- [`e2e_test.go:TestE2E_ParseResult_InvalidJSON_Error`](../../e2e_test.go)
- [`e2e_test.go:TestE2E_ClassificationContext_FullRoundTrip`](../../e2e_test.go)
- [`e2e_test.go:TestE2E_ClassificationContext_MinimalFields`](../../e2e_test.go)
- [`integration_test.go:TestIntegration_JSONRoundTripThroughFFI`](../../integration_test.go)
- [`xrr_adapter_test.go`](../../xrr_adapter_test.go) — cassette
  integration

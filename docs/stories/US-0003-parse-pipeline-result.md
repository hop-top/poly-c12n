---
status: shipped
personas: [llm-routing-saas, framework-author, middleware-developer]
priority: P0
---

# US-0003: Parse PipelineResult into typed scores

As a tool author, I want typed accessors on `PipelineResult` so my
routing logic doesn't `map[string]any`-walk.

## Use this when

- Routing layer reads per-signal scores to make the model decision.
- Logging / metrics ingestion needs typed extraction.
- Tests want to assert specific signal scores.

## Result

`PipelineResult.Signal(SignalType)` returns the `SignalResult` for
that type, or `nil`. `Confidence()` returns overall confidence.
`HasErrors()` / `Errors()` surface per-signal failures.

## Steps

```go
result, err := pipeline.Evaluate(ctx)
if err != nil {
    return err
}

if score := result.Signal(c12n.SignalCodeContent); score != nil {
    if score.Score > 0.8 {
        // route to code-specialized model
    }
}

if result.Confidence() < 0.3 {
    // escalate to powerful model
}
```

## Verify

```bash
CGO_ENABLED=0 go test -run TestE2E_ParseResult_Accessors ./...
CGO_ENABLED=0 go test -run TestE2E_PipelineResult_Signal ./...
CGO_ENABLED=0 go test -run TestE2E_PipelineResult_HasSignal ./...
```

## How it works

`PipelineResult` wraps the JSON returned from the Rust core. Typed
accessors (`Signal`, `Confidence`, `HasErrors`) parse the JSON
lazily on demand. Invalid JSON returns a typed error from
`ParseResult` rather than panicking.

In stub mode, `PipelineResult` construction + parsing still works
on hand-built JSON — useful for tests.

## Tests

- [`e2e_test.go:TestE2E_ParseResult_Accessors`](../../e2e_test.go)
- [`e2e_test.go:TestE2E_PipelineResult_Signal`](../../e2e_test.go)
- [`e2e_test.go:TestE2E_PipelineResult_HasSignal`](../../e2e_test.go)
- [`e2e_test.go:TestE2E_ParseResult_HasErrors`](../../e2e_test.go)
- [`e2e_test.go:TestE2E_ParseResult_InvalidJSON_Error`](../../e2e_test.go)
- [`result_test.go`](../../result_test.go) — unit-level

---
status: shipped
personas: [llm-routing-saas, middleware-developer]
priority: P1
---

# US-0005: Detect low-confidence classifications

As a tool author, I want `Confidence()` on `PipelineResult` so I
can escalate ambiguous prompts to a more capable model.

## Use this when

- Routing logic falls back to a powerful model when the cheap
  classifier isn't sure.
- Logging / alerting on classification quality.
- A/B test gates: only route confident classifications to the
  cheap path.

## Result

`PipelineResult.Confidence()` returns a `float64` in `[0.0, 1.0]`.
Higher = more certain. Implementations must clamp to the range.

## Steps

```go
result, _ := pipeline.Evaluate(ctx)
conf := result.Confidence()

switch {
case conf >= 0.85:
    // route to cheap, fast model
case conf >= 0.5:
    // route to mid-tier model
default:
    // escalate to powerful model
}
```

## Verify

```bash
CGO_ENABLED=0 go test -run TestE2E_PipelineResult_Confidence_Range ./...
CGO_ENABLED=0 go test -run TestE2E_PipelineResult_Confidence_Accessor ./...
```

## How it works

Confidence aggregates per-signal scores via the configured weighting
scheme. The default is a weighted average; alternative schemes
(max, min, custom) can be configured via `kit/config`. The accessor
parses JSON lazily and validates range.

## Tests

- [`e2e_test.go:TestE2E_PipelineResult_Confidence_Range`](../../e2e_test.go)
- [`e2e_test.go:TestE2E_PipelineResult_Confidence_Accessor`](../../e2e_test.go)

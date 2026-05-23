---
status: shipped
personas: [llm-routing-saas, framework-author, middleware-developer]
priority: P0
---

# US-0001: Configure pipeline via PipelineConfig

As a tool author, I want to construct a c12n pipeline with my own
fan-out concurrency + timeout, so middleware can match its latency
budget.

## Use this when

- Embedding c12n in middleware / a framework / a CLI.
- Need to bound classification latency at a known SLO.
- Want predictable resource use under load.

## Result

`c12n.NewPipeline(PipelineConfig{...})` returns a `*Pipeline` ready
to `Evaluate`. `Close()` releases resources and is idempotent.

## Steps

```go
pipeline, err := c12n.NewPipeline(c12n.PipelineConfig{
    MaxConcurrency: 8,
    Timeout:        5 * time.Second,
})
if err != nil {
    return err
}
defer pipeline.Close()
```

In CGO_ENABLED=0 builds, `err` will be `errNoCgo` — that's expected
in v0.1.0-alpha.0. Code paths that depend only on construction +
`Close()` (config validation, lifecycle plumbing) work either way.

## Verify

```bash
CGO_ENABLED=0 go test -run TestE2E_DefaultConfigToPipeline ./...
CGO_ENABLED=0 go test -run TestIntegration_PipelineLifecycle ./...
```

## How it works

`PipelineConfig` carries:

- `MaxConcurrency` — fan-out width for the Rust pipeline.
- `Timeout` — overall budget; per-signal timeouts can be tighter.
- Signal-specific config (thresholds, weights) loaded via
  `kit/config`.

`NewPipeline` returns the right impl per build tag (`c12n_cgo.go`
for cgo, `c12n_stub.go` otherwise).

## Tests

- [`e2e_test.go:TestE2E_DefaultConfigToPipeline`](../../e2e_test.go)
- [`integration_test.go:TestIntegration_PipelineLifecycle`](../../integration_test.go)
- [`integration_test.go:TestIntegration_PipelineCloseIdempotent`](../../integration_test.go)

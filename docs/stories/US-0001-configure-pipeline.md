---
status: partial
bindings:
  rust: shipped
  go: partial
  python: partial
  typescript: partial
  php: partial
personas: [llm-routing-saas, framework-author, middleware-developer]
priority: P0
---

# US-0001: Configure pipeline via PipelineConfig

As a tool author, I want to construct a c12n pipeline with my own
fan-out concurrency + timeout, so middleware can match its latency
budget.

> **Status: partial.** Concurrency and timeout genuinely reach the
> engine. *Nothing else does.* `PipelineConfig` carries exactly two
> fields, and the ~25-field `Config` collapses to those same two on the
> way in — see [What config actually reaches the
> engine](#what-config-actually-reaches-the-engine).

## Use this when

- Embedding c12n in middleware / a framework / a CLI.
- Need to bound classification latency at a known SLO.
- Want predictable resource use under load.

## Result

`c12n.NewPipeline(PipelineConfig{...})` returns a `*Pipeline` ready to
`Evaluate`. `Close()` releases resources and is idempotent.

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

`PipelineConfig` has these two fields and no others
([`go/c12n.go:8-11`](../../go/c12n.go)).

In default (stub) builds `err` is `errNativeDisabled` — expected at
v0.1.0-alpha.0. Code paths that depend only on construction + `Close()`
(config validation, lifecycle plumbing) work either way.

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestE2E_DefaultConfigToPipeline ./...
```

The native lifecycle test needs the cdylib on the linker path:

```bash
cargo build -p hop-top-c12n-core
cd go && CGO_ENABLED=1 \
  CGO_LDFLAGS="-L$(cd .. && pwd)/target/debug" \
  DYLD_LIBRARY_PATH="$(cd .. && pwd)/target/debug" \
  go test -tags "c12n_native integration" \
  -run 'TestIntegration_Pipeline(Lifecycle|CloseIdempotent)' ./...
```

Both named tests pass. Two of their siblings in the same file do **not**
— see [US-0007](US-0007-json-ffi-roundtrip.md).

## What config actually reaches the engine

`Config` declares ~25 fields — per-signal enables, thresholds, model
paths ([`go/config.go:14-66`](../../go/config.go)). `ToPipelineConfig()`
passes through two of them:

```go
// go/config.go:110
func (c *Config) ToPipelineConfig() PipelineConfig {
    return PipelineConfig{
        MaxConcurrency: c.MaxConcurrency,
        Timeout:        time.Duration(c.TimeoutMs) * time.Millisecond,
    }
}
```

The remaining fields are inert with respect to classification.
`EnabledSignals()` ([`go/config.go:118`](../../go/config.go)) is
consumed only for *reporting* — a debug log line
([`go/cmd/c12n/root.go:87`](../../go/cmd/c12n/root.go)) and the
`c12n signals` table
([`go/cmd/c12n/signals.go`](../../go/cmd/c12n/signals.go)). It never
constructs a pipeline. `c12n signals` can therefore print
`Enabled: true` for a signal that will not run.

Downstream of that, the FFI constructs `Pipeline::new(vec![], ..)`
([`core/src/ffi.rs:134`](../../core/src/ffi.rs)) regardless of config,
so a Go-built pipeline has zero signals. Since PR #42 `evaluate`
reports this as `PipelineError::NoSignals` rather than returning a
silent empty envelope.

## How it works

`NewPipeline` selects an implementation by build tag:
[`go/c12n_cgo.go`](../../go/c12n_cgo.go) with `-tags c12n_native`,
[`go/c12n_stub.go`](../../go/c12n_stub.go) otherwise. The cgo path
marshals `{max_concurrency, timeout_ms}` to JSON and hands it to
`c12n_pipeline_new`.

## What this story needs to reach `shipped`

Config-schema plumbing that lets `ToPipelineConfig` (or a successor)
express a signal set the FFI can register. Until then the per-signal
half of `Config` is documentation of intent, not behaviour.

## Tests

- [`go/e2e_test.go:TestE2E_DefaultConfigToPipeline`](../../go/e2e_test.go)
  — asserts the two-field conversion; the pipeline it builds is a stub.
- [`go/integration_test.go:TestIntegration_PipelineLifecycle`](../../go/integration_test.go)
- [`go/integration_test.go:TestIntegration_PipelineCloseIdempotent`](../../go/integration_test.go)
- [`go/config_test.go:TestToPipelineConfig`](../../go/config_test.go) — unit-level.

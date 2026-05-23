# Middleware developer

Writing HTTP middleware that classifies-then-routes LLM requests.

## Use c12n when

- Middleware sits between client app and LLM provider.
- Want classification + routing decision in <5ms.
- Don't want middleware code to depend on Rust toolchain in
  production builds.

## Constraints

- Latency budget < 5ms per request including JSON parse.
- Stateless middleware: pipeline must be safe to share across
  requests OR cheap to recreate.
- Production deploy is a single Go binary.

## What they get from c12n

- `c12n.NewPipeline(PipelineConfig{...})` + `pipeline.Evaluate(ctx)` —
  hot-path API.
- `MaxConcurrency` knob for tuning fan-out.
- Timeout enforcement via `ClassificationContext.Timeout`.
- `Close()` is idempotent — safe in defer.

## Stories

- [US-0001 Configure pipeline via PipelineConfig](../../stories/US-0001-configure-pipeline.md)
- [US-0003 Parse PipelineResult into typed scores](../../stories/US-0003-parse-pipeline-result.md)
- [US-0005 Detect low-confidence classifications](../../stories/US-0005-low-confidence-detection.md)
- [US-0007 Parse JSON from FFI without panic](../../stories/US-0007-json-ffi-roundtrip.md)

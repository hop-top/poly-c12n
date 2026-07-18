# Framework author

Building a higher-level orchestration framework that delegates
classification to c12n.

## Use c12n when

- Your framework needs a swappable classifier; want c12n's
  `Pipeline` interface as a plug-in.
- Want classification + routing in the same hot path.
- Framework users expect zero-config defaults.

## Constraints

- API stability for downstream consumers.
- Documentation overhead: must explain c12n concepts in framework's
  own vocabulary.
- Must work in CGO_ENABLED=0 mode for framework users who don't
  want native deps.

## What they get from c12n

- `c12n_stub.go` lets the framework compile without Rust core —
  classifier returns `errNoCgo` until the user wires it.
- `Pipeline` interface is small: `Evaluate(ctx) (PipelineResult,
  error)` + `Close()`.
- `ClassificationContext` is a struct, not a builder — clean for
  framework users to construct.

## Stories

- [US-0001 Configure pipeline via PipelineConfig](../../stories/US-0001-configure-pipeline.md)
- [US-0003 Parse PipelineResult into typed scores](../../stories/US-0003-parse-pipeline-result.md)
- [US-0008 Configure pipeline scope (system/user/project)](../../stories/US-0008-config-scope.md)

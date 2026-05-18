# c12n stories

Tool-author user stories. Each story is one page, intent-driven shape:
**Use this when / Result / Steps / Verify / How it works / Tests.**

| ID  | Title | Tests |
|-----|-------|-------|
| [US-0001](US-0001-configure-pipeline.md) | Configure pipeline via PipelineConfig | `e2e_test.go:TestE2E_DefaultConfigToPipeline`, `integration_test.go:TestIntegration_PipelineLifecycle` |
| [US-0002](US-0002-classify-cli.md) | Evaluate a prompt via CLI | `cmd/c12n/e2e_test.go:TestE2EClassifyFlagsComplete`, `TestE2EClassifyStdinFlag`, `TestE2EClassifyFormatFlag` |
| [US-0003](US-0003-parse-pipeline-result.md) | Parse PipelineResult into typed scores | `e2e_test.go:TestE2E_ParseResult_Accessors`, `TestE2E_PipelineResult_Signal`, `TestE2E_ParseResult_InvalidJSON_Error` |
| [US-0004](US-0004-bench-overhead.md) | Benchmark classification overhead | `cmd/c12n/bench_regressions_test.go:TestBenchPercentile_*`, `cmd/c12n/e2e_test.go:TestE2EBenchIterationsFlag` |
| [US-0005](US-0005-low-confidence-detection.md) | Detect low-confidence classifications | `e2e_test.go:TestE2E_PipelineResult_Confidence_Range`, `TestE2E_PipelineResult_Confidence_Accessor` |
| [US-0006](US-0006-toolspec-discovery.md) | Emit toolspec JSON for AI-agent discovery | `cmd/c12n/e2e_test.go:TestE2EToolspecValidJSON`, `TestE2EToolspecContainsAllCommands`, `TestE2EToolspecHasErrorPatterns` |
| [US-0007](US-0007-json-ffi-roundtrip.md) | Parse JSON from FFI without panic | `integration_test.go:TestIntegration_JSONRoundTripThroughFFI`, `e2e_test.go:TestE2E_ClassificationContext_FullRoundTrip` |
| [US-0008](US-0008-config-scope.md) | Configure pipeline scope (system/user/project) | `cmd/c12n/e2e_test.go:TestE2EConfigSetScopeFlag`, `cmd/c12n/doctor_regressions_test.go:TestDoctorConfigCheck_*` |

UCP: tool authors. See [personas/](../personas/README.md) for the five
roles these stories serve.

## CGO note

c12n ships two Go build modes:

- **`CGO_ENABLED=0`** (stub): pipeline + config + parsing + CLI all
  work; `Pipeline.Evaluate` returns `errNoCgo`. Useful for tooling
  that consumes c12n types without needing the engine.
- **`CGO_ENABLED=1`** (real): links `libc12n_core.{so,dylib}` from
  the Rust core (`c12n-core/`). Real classification.

Both modes are exercised in CI. Stories below note the mode where
relevant; otherwise they run in both.

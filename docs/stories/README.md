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
| [US-0020](US-0020-php-install-load.md) | Install and load the PHP binding | `php/tests/FfiTest.php`, `php/tests/InstallerTest.php`, `php/tests/PipelineFfiIntegrationTest.php` |
| [US-0021](US-0021-php-evaluate-parse.md) | Evaluate a context and parse the result in PHP | `php/tests/PipelineFfiIntegrationTest.php:testEvaluateReturnsValidEnvelopeForDefaultConfig`, `testPipelineResultParsesRoundtripJson`, `php/tests/PipelineTest.php` |
| [US-0022](US-0022-php-no-signals.md) | Handle errors and the NoSignals diagnostic in PHP | `php/tests/PipelineFfiIntegrationTest.php:testEmptyPipelineJsonShapeMatchesCanonicalParity`, `testEvaluateReturnsErrorEnvelopeForMalformedContext` |
| [US-0023](US-0023-ts-install-entrypoint.md) | Install `@hop-top/c12n` and pick the right entrypoint | `ts/test/pipeline.integration.test.ts`, `ts/test/setup.ts`, `ts/test/bundler-smoke.test.ts` |
| [US-0024](US-0024-ts-evaluate-parse.md) | Evaluate a context and parse the result in TypeScript | `ts/test/pipeline.integration.test.ts`, `ts/test/pipeline.test.ts` |
| [US-0025](US-0025-ts-no-signals.md) | Handle errors and the NoSignals diagnostic in TypeScript | `ts/test/pipeline.integration.test.ts`, `ts/test/pipeline.test.ts`, `ts/test/bundler-smoke.test.ts` |

UCP: tool authors. See [personas/](../personas/README.md) for the five
roles these stories serve.

## Binding coverage

US-0001..US-0008 were written Go-first and cite Go test paths
throughout; several describe surfaces (CLI, build tags) that exist
only on Go. US-0020..US-0025 cover PHP and TypeScript.

**Detector configuration is Rust-only today.** Detectors and tiered
chains (`core/src/registry.rs`, `core/src/chain.rs`, ADR-0003) are
not plumbed through `c12n_pipeline_new` (`core/src/ffi.rs`) or the
wasm constructor (`core/src/wasm.rs`), both of which build the
pipeline with a hardcoded empty signal vector. PHP and TS callers
therefore get a working pipeline that returns zero results plus a
`NoSignals` diagnostic. US-0022 and US-0025 document this; they are
marked `status: partial` for that reason.

## Build-mode note

c12n ships two Go build modes, selected by the `c12n_native` build tag:

- **default** (stub, regardless of `CGO_ENABLED`): pipeline + config +
  parsing + CLI all work; `Pipeline.Evaluate` returns
  `errNativeDisabled`. Useful for tooling that consumes c12n types
  without needing the engine.
- **`-tags c12n_native`** (real, requires cgo): links
  `libc12n_core.{so,dylib}` from the Rust core (`c12n-core/`). Real
  classification.

Both modes are exercised in CI. Stories below note the mode where
relevant; otherwise they run in both.

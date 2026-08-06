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
| [US-0009](US-0009-regex-pii-baseline.md) | Catch structured PII with the regex baseline | `core/src/signals/detectors/pii_regex.rs:email_true_positives`, `luhn_accepts_known_test_cards`, `wired_into_pii_signal` |
| [US-0010](US-0010-heuristic-jailbreak-baseline.md) | Flag obvious jailbreak attempts with the heuristic baseline | `core/src/signals/detectors/jailbreak_heuristic.rs:injection_true_positives`, `confidence_saturates_and_never_reaches_one` |
| [US-0011](US-0011-stopword-language-baseline.md) | Identify Western European languages by stopword frequency | `core/src/signals/detectors/language_stopword.rs:detects_each_supported_language`, `unsupported_scripts_yield_none_not_a_wrong_guess` |
| [US-0012](US-0012-approx-tokenizer-estimate.md) | Estimate token counts without a vocabulary | `core/src/signals/detectors/tokenizer_approx.rs:model_name_advertises_approximation`, `english_prose_lands_near_four_chars_per_token` |
| [US-0013](US-0013-tiered-detector-chains.md) | Trade cost against accuracy with tiered detector chains | `core/src/chain.rs:parses_strategy_specs`, `scalar_trait_rejects_confidence_strategies`, `provenance_lands_in_metadata` |
| [US-0014](US-0014-detector-registry-by-name.md) | Build detector chains from configuration names | `core/src/registry.rs:builds_registered_pii_detector`, `unknown_name_fails_loudly_and_names_alternatives` |
| [US-0015](US-0015-no-signals-diagnostic.md) | Learn that a pipeline has no signals registered | `core/src/pipeline.rs` (NoSignals), `ts/test/pipeline.integration.test.ts`, `php/tests/PipelineFfiIntegrationTest.php` |
| [US-0020](US-0020-php-install-load.md) | Install and load the PHP binding | `php/tests/FfiTest.php`, `php/tests/InstallerTest.php`, `php/tests/PipelineFfiIntegrationTest.php` |
| [US-0021](US-0021-php-evaluate-parse.md) | Evaluate a context and parse the result in PHP | `php/tests/PipelineFfiIntegrationTest.php:testEvaluateReturnsValidEnvelopeForDefaultConfig`, `php/tests/PipelineTest.php` |
| [US-0022](US-0022-php-no-signals.md) | Handle errors and the NoSignals diagnostic in PHP | `php/tests/PipelineFfiIntegrationTest.php:testEmptyPipelineJsonShapeMatchesCanonicalParity` |
| [US-0023](US-0023-ts-install-entrypoint.md) | Install `@hop-top/c12n` and pick the right entrypoint | `ts/test/nodejs-subpath.test.ts`, `ts/test/bundler-smoke.test.ts` |
| [US-0024](US-0024-ts-evaluate-parse.md) | Evaluate a context and parse the result in TypeScript | `ts/test/pipeline.integration.test.ts`, `ts/test/pipeline.test.ts` |
| [US-0025](US-0025-ts-no-signals.md) | Handle errors and the NoSignals diagnostic in TypeScript | `ts/test/pipeline.integration.test.ts`, `ts/test/bundler-smoke.test.ts` |

## Status values

`shipped` — usable today from every binding the story names.
`partial` — implemented in the Rust core, but **not yet selectable
from Go / Python / PHP / TypeScript**, because the config-schema
plumbing has not landed. US-0009..US-0014 are `partial` for exactly
this reason; each states its boundary inline.

## Quality limits are documented, not implied

The tier-1 detectors (US-0009..US-0012) are deliberately simple
baselines. Each story carries the detector's real limitations in
prose — what it misses, where it is bypassed, how wrong the number
can be — because a caller who never reads Rust doc comments would
otherwise assume coverage that does not exist. Believing PII is being
caught when it is not is worse than shipping nothing.

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

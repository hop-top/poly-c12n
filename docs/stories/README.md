# c12n stories

Tool-author user stories. Each story is one page, intent-driven shape:
**Use this when / Result / Steps / Verify / How it works / Tests.**

UCP: tool authors. See [personas/](../personas/README.md) for the five
roles these stories serve.

## Status scheme

Stories carry a status in frontmatter. Until PR #42 every story here was
marked `shipped`, including features that had never worked through any
binding — so `shipped` had stopped carrying information. It now means
something narrow and checkable.

| Status | Means |
|--------|-------|
| `shipped` | Every documented claim is true today, and a test exercises the real code path — not a hand-written fixture standing in for it. |
| `partial` | The core works, but a documented sub-claim does not. The story must say which, and why. |
| `planned` | The described capability cannot be used as written. The story stays for design intent and carries a blocker section. |

Because c12n publishes five artifacts from one core, a single status is
usually too coarse: a feature can be complete in Rust and unreachable
from Go. Stories therefore also carry a per-binding map:

```yaml
status: partial          # worst-case across bindings
bindings:
  rust: shipped
  go: partial
  python: planned
  typescript: planned
  php: planned
```

Top-level `status` is the **worst case** across bindings, so a reader
who checks only that field is never over-promised. Stories with a
single relevant binding (the CLI stories) list just that one.

## Index

| ID | Title | Status | Blocked on |
|----|-------|--------|-----------|
| [US-0001](US-0001-configure-pipeline.md) | Configure pipeline via PipelineConfig | `partial` | only 2 of ~25 config fields reach the engine |
| [US-0002](US-0002-classify-cli.md) | Evaluate a prompt via CLI | `partial` | CLI runs; no binding can register signals, so `results` is always empty |
| [US-0003](US-0003-parse-pipeline-result.md) | Parse PipelineResult into typed scores | `partial` | score accessors never see engine output; fixture-only |
| [US-0004](US-0004-bench-overhead.md) | Benchmark classification overhead | `partial` | benchmarks a signal-less pipeline; measures FFI cost only |
| [US-0005](US-0005-low-confidence-detection.md) | Detect low-confidence classifications | `planned` | no aggregate confidence; no signals registered |
| [US-0006](US-0006-toolspec-discovery.md) | Emit toolspec JSON for AI-agent discovery | `partial` | spec hand-authored and already stale — omits `status` and `toolspec` |
| [US-0007](US-0007-json-ffi-roundtrip.md) | Parse JSON from FFI without panic | `partial` | `{"error": ...}` envelope decodes silently to an empty result |
| [US-0008](US-0008-config-scope.md) | Configure pipeline scope (system/user/project) | `partial` | `config set` quotes numeric values, breaking later loads; env layer inert |
| [US-0009](US-0009-regex-pii-baseline.md) | Catch structured PII with the regex baseline | `partial` | Rust-constructible only; no config plumbing |
| [US-0010](US-0010-heuristic-jailbreak-baseline.md) | Flag obvious jailbreak attempts with the heuristic baseline | `partial` | same |
| [US-0011](US-0011-stopword-language-baseline.md) | Identify Western European languages by stopword frequency | `partial` | same |
| [US-0012](US-0012-approx-tokenizer-estimate.md) | Estimate token counts without a vocabulary | `partial` | same |
| [US-0013](US-0013-tiered-detector-chains.md) | Trade cost against accuracy with tiered detector chains | `partial` | same |
| [US-0014](US-0014-detector-registry-by-name.md) | Build detector chains from configuration names | `partial` | same |
| [US-0015](US-0015-no-signals-diagnostic.md) | Learn that a pipeline has no signals registered | `shipped` | — |
| [US-0020](US-0020-php-install-load.md) | Install and load the PHP binding | `shipped` | — |
| [US-0021](US-0021-php-evaluate-parse.md) | Evaluate a context and parse the result in PHP | `shipped` | — |
| [US-0022](US-0022-php-no-signals.md) | Handle errors and the NoSignals diagnostic in PHP | `partial` | no option makes a signal fire from PHP |
| [US-0023](US-0023-ts-install-entrypoint.md) | Install `@hop-top/c12n` and pick the right entrypoint | `shipped` | — |
| [US-0024](US-0024-ts-evaluate-parse.md) | Evaluate a context and parse the result in TypeScript | `shipped` | — |
| [US-0025](US-0025-ts-no-signals.md) | Handle errors and the NoSignals diagnostic in TypeScript | `partial` | no option makes a signal fire from TS |

The common root cause behind most of these: `evaluate` only scores
signals registered at construction, and four of the five bindings
construct with `vec![]`. PR #42 landed real detectors
([`core/src/signals/detectors/`](../../core/src/signals/detectors/)), a
name-based registry ([`core/src/registry.rs`](../../core/src/registry.rs))
and tiered chains ([`core/src/chain.rs`](../../core/src/chain.rs)) — but
**not** the config-schema plumbing that would let a Go / PHP / Python /
TypeScript caller name a detector. Chains remain Rust-constructible
only. Treat any claim that a non-Rust binding can select detectors as
false until that plumbing lands.

That plumbing is still missing. `c12n_pipeline_new`
([`core/src/ffi.rs`](../../core/src/ffi.rs)),
[`core/src/wasm.rs`](../../core/src/wasm.rs) and
[`py/src/lib.rs`](../../py/src/lib.rs) all still construct with a
hardcoded empty signal vector, and `go/config.go`'s
`ToPipelineConfig()` forwards only `MaxConcurrency` and `Timeout` out
of ~25 fields — `EnabledSignals()` remains report-only. So the CLI
stories below describe commands that **run** and return a well-formed
envelope containing zero results plus the `NoSignals` diagnostic. A
running command is not a working classifier; none of them is
`shipped` on that basis.

What the CLI-fixing PRs did change: the startup panics are gone and
the binary is runnable, `--scope system` resolves a path, Go parses
the native `errors` payload, and `doctor` / `status` / `toolspec` /
`signals` work in stub builds because pipeline construction is no
longer a precondition for every command.

## A note on test evidence

Several stories cite tests that pass while proving less than the story
claimed. The recurring pattern: a test parses a hand-written JSON
fixture and asserts the accessor reads it back. That validates the
accessor, not the engine — and it stays green no matter what the engine
produces.

When citing a test as evidence, state what it actually exercises.
Stories here now label fixture-only tests as such.

## Test coverage is Go-first

This index used to list Go test paths for all eight stories, which read
as though Go were the only tested binding. It is not — it is the only
one these stories were written against:

| Binding | Tests |
|---------|-------|
| Rust | [`core/tests/`](../../core/tests/) + in-crate; 296 pass via `cargo test -p hop-top-c12n-core --all-targets` |
| Go | [`go/`](../../go/) `*_test.go` — the stories below |
| Python | [`py/tests/`](../../py/tests/) — 8 modules incl. `test_e2e_stories.py` |
| TypeScript | [`ts/test/`](../../ts/test/) — unit, integration, bundler smoke |
| PHP | [`php/tests/`](../../php/tests/) — unit + FFI integration |

PHP and TypeScript were ahead of Go for a while: both were updated for
#42's `NoSignals` diagnostic while Go was not, which is how
[US-0007](US-0007-json-ffi-roundtrip.md)'s breakage went unnoticed. Go
has since caught up — `PipelineError` matches the wire format and CI
runs the suite under `-tags "c12n_native integration"`, so the same
class of drift now fails the build.
Story coverage for those bindings is being written separately.

Test paths in each story are repo-root-relative (`go/e2e_test.go`, not
`e2e_test.go`). Earlier revisions linked one directory too high and
every link 404'd.
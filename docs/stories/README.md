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
| [US-0002](US-0002-classify-cli.md) | Evaluate a prompt via CLI | `planned` | binary panics at startup (duplicate `--config`) |
| [US-0003](US-0003-parse-pipeline-result.md) | Parse PipelineResult into typed scores | `partial` | Go cannot parse native `errors` payload |
| [US-0004](US-0004-bench-overhead.md) | Benchmark classification overhead | `planned` | startup panic; benchmarks a signal-less pipeline |
| [US-0005](US-0005-low-confidence-detection.md) | Detect low-confidence classifications | `planned` | no aggregate confidence; no signals registered |
| [US-0006](US-0006-toolspec-discovery.md) | Emit toolspec JSON for AI-agent discovery | `partial` | startup panic; spec hand-authored, can drift |
| [US-0007](US-0007-json-ffi-roundtrip.md) | Parse JSON from FFI without panic | `partial` | native round-trip fails to parse |
| [US-0008](US-0008-config-scope.md) | Configure pipeline scope (system/user/project) | `partial` | `--scope system` unusable; no env layer |
| [US-0009](US-0009-regex-pii-baseline.md) | Catch structured PII with the regex baseline | `partial` | Rust-constructible only; no config plumbing |
| [US-0010](US-0010-heuristic-jailbreak-baseline.md) | Flag obvious jailbreak attempts with the heuristic baseline | `partial` | same |
| [US-0011](US-0011-stopword-language-baseline.md) | Identify Western European languages by stopword frequency | `partial` | same |
| [US-0012](US-0012-approx-tokenizer-estimate.md) | Estimate token counts without a vocabulary | `partial` | same |
| [US-0013](US-0013-tiered-detector-chains.md) | Trade cost against accuracy with tiered detector chains | `partial` | same |
| [US-0014](US-0014-detector-registry-by-name.md) | Build detector chains from configuration names | `partial` | same |
| [US-0015](US-0015-no-signals-diagnostic.md) | Learn that a pipeline has no signals registered | `shipped` | — |

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

PHP and TypeScript have real suites but no stories. Ironically their
integration tests are *ahead* of Go's: both were updated for #42's
`NoSignals` diagnostic, while Go's were not — which is how
[US-0007](US-0007-json-ffi-roundtrip.md)'s breakage went unnoticed.
Story coverage for those bindings is being written separately.

Test paths in each story are repo-root-relative (`go/e2e_test.go`, not
`e2e_test.go`). Earlier revisions linked one directory too high and
every link 404'd.

## Build-mode note

c12n ships two Go build modes, selected by the `c12n_native` build tag:

- **default** (stub, regardless of `CGO_ENABLED`): config + parsing +
  the command tree work; `NewPipeline` and `Pipeline.Evaluate` return
  `errNativeDisabled`. Useful for tooling that consumes c12n types
  without needing the engine.
- **`-tags c12n_native`** (real, requires cgo): links
  `libc12n_core.{so,dylib}` built from [`core/`](../../core/). Real
  classification — subject to the signal-registration gap above.

The native integration tests need `-tags "c12n_native integration"` and
the cdylib on the library path:

```bash
cargo build -p hop-top-c12n-core
cd go && CGO_ENABLED=1 \
  CGO_LDFLAGS="-L$(cd .. && pwd)/target/debug" \
  DYLD_LIBRARY_PATH="$(cd .. && pwd)/target/debug" \
  go test -tags "c12n_native integration" ./...
```

The crate is `hop-top-c12n-core`; `cargo build -p c12n-core` fails.
Two tests in that run currently fail — see
[US-0007](US-0007-json-ffi-roundtrip.md).

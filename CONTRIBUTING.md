# Contributing to poly-c12n

c12n is a Rust core engine wrapped by five language bindings (Go, Python, TypeScript, PHP, and an idiomatic Rust SDK). All five share one source tree and one CI workflow.

## Use this when

- You want to fix a bug, add a signal type, or improve a binding.
- You want to add a new language binding.
- You want to propose an architectural change.

## Before you begin

Required tools (managed via [`mise`](https://mise.jdx.dev/)):

- **Rust** (stable, with `rustfmt` + `clippy`)
- **Go** 1.24+
- **Python** 3.11+
- **Node** 20+ and **pnpm** 9
- **PHP** 8.4 + `composer`
- **wasm-pack** (for TypeScript bindings)
- **cbindgen** (auto-installed by `core/build.rs`)

```bash
mise install
```

## Quick path

```bash
git clone git@github.com:hop-top/poly-c12n.git
cd poly-c12n

# Build the Rust core + cdylib (every other binding needs this)
cargo build -p hop-top-c12n-core

# Run a language's test suite:
cargo test --workspace          # rust core + rs SDK
( cd go && go test ./... )      # go (stub mode, no cgo)
( cd py && pytest -q )          # python (needs pytest pytest-asyncio pyyaml)
( cd ts && pnpm install && pnpm test )   # typescript
( cd php && composer install && vendor/bin/phpunit )   # php
```

The `cargo build -p hop-top-c12n-core` step produces `target/debug/libc12n_core.{so,dylib,dll}`. The Go cgo tests, PHP FFI tests, and Python PyO3 tests all load that file. Without it, you'll see "library not found" errors at link time.

## Steps

### 1. Branch + commit

Branch names: `<type>/c12n-T<NNNN>-<slug>` (e.g. `chore/c12n-T0200-license-sync`). Track numbers come from the local `tlc` task list (`.tlc/`).

Commits follow [Conventional Commits](https://conventionalcommits.org):

```
feat(scope): add new signal type
fix(core): wasm32 Pipeline panic on Instant::now
ci(c12n-php): bump PHP floor to 8.4
docs(adr): formalize manifest.json convention
```

Valid scopes: `core`, `rs`, `go`, `py`, `ts`, `php`, `ci`, `docs`, `release`. Anything else is fine if it's clear.

### 2. Architecture decisions: write an ADR

New signal type? New binding? Cross-cutting refactor? Drop an ADR under `docs/adr/<N>-<slug>.md` first. Existing examples: `0001-c12n-ts-wasm-binding.md`, `0002-c12n-php-ffi-binding.md`.

### 3. Open a PR

- Reference the tlc task ID in the PR title (`(T-NNNN)`).
- Target `main`.
- CI runs the polyglot matrix (rust × 3 OS + go × 3 OS × {stub,cgo} + python × 3 OS + ts × 3 OS + php × 2 OS).
- All required checks must pass.

## Common issues

| Symptom | Cause | Fix |
|---|---|---|
| `library 'c12n_core' not found` when running Go cgo tests | cdylib not built yet | `cargo build -p hop-top-c12n-core` from repo root |
| `composer install` fails resolving `hop-top/kit` | composer.json missing `"minimum-stability": "alpha"` | kit publishes on the alpha channel; see `php/composer.json` for the working config |
| `pnpm install --frozen-lockfile` fails locally but passes in CI | Outer `pnpm-workspace.yaml` at the labspace root shadowing `ts/` | Run `pnpm install --ignore-workspace` from `ts/` |
| `wasm-pack build` complains about `[package]` | Invoking from `ts/` — that's not a Cargo crate | Run from repo root: `wasm-pack build core --target bundler --features wasm --out-dir ../ts/pkg/bundler -- --no-default-features` |
| `rustfmt` diff in CI | Local rustfmt out of sync with stable | `cargo fmt --all` from repo root |
| `clippy -D warnings` failure on unrelated code | Newer clippy version surfacing new lints | Fix locally; we don't pin clippy version yet |

## How it works

**Polyglot monorepo**: the `core/` Rust crate is the engine; `core/src/ffi.rs` exports a C ABI (consumed by Go cgo + PHP FFI); `core/src/wasm.rs` exports a `wasm-bindgen` surface (consumed by the TypeScript binding). `cbindgen` regenerates `core/include/libc12n_core.h` on every build.

**Tests**: each language's suite runs against the same Rust core, ensuring wire compatibility. Adding a new signal in `core/src/signals/<name>.rs` means adding parity tests in every binding.

**Releases**: managed by `release-please` per-component (each language ships independently). See [RELEASING.md](RELEASING.md) for the flow.

## Options

| Workflow | When to use |
|---|---|
| `tlc track create` + plan.md | Multi-task work that spans bindings (anything > 1 PR) |
| Direct PR | Single-binding fix or doc change |
| Open issue first | When you want sign-off on an approach before coding |

## Code of conduct

This project follows the [Contributor Covenant 2.1](https://www.contributor-covenant.org/version/2/1/code_of_conduct/). Report violations to the email in [SECURITY.md](SECURITY.md).

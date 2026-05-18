# ADR-0001: c12n-ts binds c12n-core via WebAssembly (wasm-bindgen)

Status: accepted
Date: 2026-05-18
Track: `c12n-ts-bindings`

## Context

c12n already ships three language surfaces over the `c12n-core` Rust crate:

| Surface | Path | Binding mechanism |
|---------|------|-------------------|
| Rust    | `c12n-core/`         | native crate, tokio runtime in-process |
| Go      | `c12n_cgo.go`        | cgo over the C ABI in `c12n-core/src/ffi.rs` |
| Python  | `c12n-py/src/lib.rs` | PyO3, holds a tokio `Runtime` per pipeline |

The hop-top fleet pattern (see `kit/sdk/`) ships five language SDKs per tool. c12n
is missing TypeScript and PHP. This ADR decides how `c12n-ts` (the npm package
`@hop-top/c12n`) binds to `c12n-core`.

The existing C ABI (`c12n_pipeline_new` / `_evaluate` / `_free`, JSON in/out) is
the canonical surface for native bindings. It internally constructs a multi-threaded
`tokio::runtime::Runtime` per pipeline and `block_on`s `pipeline.evaluate` from
foreign callers.

Three binding strategies were on the table:

1. **N-API / napi-rs** — Node-native addon. Best perf, per-platform prebuilds.
2. **WASM / wasm-bindgen** — universal artifact. Single `.wasm` file.
3. **Pure-TS subprocess over the `c12n` Go CLI** — no native binding at all.

TS consumers target multiple runtimes today: Node services, browser tooling (agent
playgrounds, classification previews), Cloudflare Workers (edge classification),
Deno (occasional). A Node-only artifact would force a parallel browser/Workers
binding later. A subprocess approach forfeits the "native binding" framing
entirely and degrades latency to fork+exec per classification.

## Decision

`c12n-ts` binds `c12n-core` via **WebAssembly using `wasm-bindgen`**. A single
`.wasm` artifact ships inside `@hop-top/c12n` on npm and runs across Node,
browsers, Cloudflare Workers, and Deno without per-target builds.

The six locked sub-decisions:

1. **Binding strategy: WASM via wasm-bindgen.** Universal artifact. One `.wasm`
   file ships in the npm package. Accepts the tokio-on-wasm constraint:
   `c12n-core` grows a Cargo feature `wasm` that swaps the multi-threaded
   `tokio::runtime::Runtime::new()` call in `ffi.rs` for a single-threaded
   `tokio::runtime::Builder::new_current_thread()` executor (or
   `wasm-bindgen-futures` when async needs to surface to JS). The feature-flag
   work is in-scope for this track (T-0120).
2. **Package name: `@hop-top/c12n`.** Matches `@hop-top/kit` (kit-ts).
3. **Versioning: linked-versions** with `c12n`, `c12n-core`, `c12n-py`, and
   `c12n-php`. All bump together via `release-please-config.json`'s
   `linked-versions` group. The first c12n-ts tag is `c12n-ts/v0.1.0-alpha.0`,
   cut after `c12n/v…`'s initial release.
4. **Windows: shipped at v0.1.0-alpha.0.** WASM is OS-agnostic so Windows is free.
   CI matrix runs ubuntu + macOS + Windows for the ts build+test job (parity with
   Go + Python coverage).
5. **Kit-ts dependency: required.** `c12n-ts` depends on `@hop-top/kit` for the
   canonical hop-top observability story — log emission through kit-ts `bus` /
   `output` / `cli` surfaces, not ad-hoc `console.log`. Users plugging c12n-ts
   into a kit-ts-aware app get unified logging and event topics for free.
6. **No prebuild matrix.** WASM is one artifact across all JS runtimes and OSes.
   No `prebuild-*`, no `node-gyp` fallback, no postinstall compilation. `npm
   install @hop-top/c12n` retrieves bytes; nothing builds at install time.

### Build tool

`wasm-pack` produces the `.wasm` binary plus the JS glue (typed bindings,
`init()`, memory plumbing). Standard wasm-bindgen workflow. Target selection is
an open implementation question (see below) — the package likely ships a
`--target bundler` build for broad bundler compatibility, with a `--target
nodejs` fallback if direct Node imports without a bundler are required.

## Consequences

### Positive

- **One artifact, all JS runtimes.** Node, browser, Cloudflare Workers, Deno —
  same `.wasm`. No "we only support Node" caveat. Edge classification at
  Workers / Deno-Deploy gets the same signal evaluation as a Node CLI.
- **No prebuild matrix.** Skips the napi-rs maintenance burden of building +
  uploading prebuilds for every (OS × arch × node-version) tuple on each release.
- **No install-time compilation.** WASM bytes are content — no native toolchain
  required on the consumer side, no postinstall script, no opaque failure modes
  on uncommon platforms.
- **Aligns with kit-ts shape.** kit-ts (`@hop-top/kit`) is pure-TS; `c12n-ts`
  adds a `.wasm` asset but the package layout (`dist/` exports, ES2020 CJS,
  vitest, eslint) is identical. Fleet packaging stays uniform.
- **No C ABI marshalling on the TS side.** wasm-bindgen generates typed JS glue
  directly against Rust types; TS callers see `Pipeline`, `evaluate(ctx)`, and
  `Result` objects rather than raw pointers + `c12n_result_free()` calls.

### Negative

- **c12n-core grows a `wasm` Cargo feature.** Scope creep onto the core crate:
  it must offer a different async-runtime surface under `cfg(feature = "wasm")`.
  Today `ffi.rs` calls `tokio::runtime::Runtime::new()` unconditionally; that
  line goes behind a feature flag and the wasm build substitutes a
  single-threaded executor. Maintained in c12n-core, not c12n-ts.
- **WASM bundle is larger than a minimal N-API binary.** Hundreds of KB of
  `.wasm` vs tens of KB of native `.node`. Tradeoff documented and accepted —
  c12n classification is not bundle-size-critical; consumers integrating into
  edge bundles already budget for WASM payloads (e.g. SQLite WASM, image codecs).
- **No multi-threaded classification in the WASM build.** The single-threaded
  executor evaluates signals sequentially within a single context. For v0 this
  is acceptable: classification is fast (signal evaluation is short-lived) and
  one-context-at-a-time matches how every existing binding is called from a
  request handler. If a real benchmark surfaces a multi-thread need, revisit
  with `wasm-bindgen-rayon` or split-runtime experiments.
- **Native bindings (Go, Python) keep the multi-thread runtime.** c12n-ts is the
  only surface with the single-thread constraint. Documentation must call this
  out so users running parity tests across surfaces understand the throughput
  difference.

### Neutral

- **Build tool: `wasm-pack`.** Standard wasm-bindgen workflow; no exotic
  toolchain. `cargo install wasm-pack` is the only extra step on CI runners
  beyond what `c12n-core` already needs.
- **Async surface.** TS callers `await pipeline.evaluate(ctx)`. The JS glue
  resolves the promise once the single-threaded executor completes the
  evaluate call. No tokio types leak across the binding.
- **No N-API fallback for "Node power users".** Decision is WASM-only. If a
  user lands with a real Node-perf complaint backed by numbers, ADR-0002 can
  add a per-runtime selector — not a v0 concern.

## Alternatives considered

### A. N-API / napi-rs

A native Node addon compiled per (OS × arch × Node ABI). Best classification
throughput — direct memory access, no WASM linear-memory copies, full tokio
multi-thread.

Rejected — Node-only. Doesn't run in browsers, Cloudflare Workers, or Deno
(Deno's Node compat shim covers some N-API but not reliably across versions).
Adopters at edge runtimes would need a parallel binding strategy anyway; that
forks the c12n-ts surface into "Node build" vs "everywhere-else build" and
doubles release-engineering work. Prebuild matrix maintenance (uploading
`.node` artifacts for ubuntu/macos/windows × arm64/x64 × node 18/20/22) is a
recurring tax we'd rather not pay for v0.

### B. WASM via wasm-bindgen (chosen)

See **Decision** above.

### C. Pure-TS over `c12n` CLI subprocess

Skip native binding entirely. `c12n-ts` shells out to the Go CLI's `evaluate`
command with JSON over stdin/stdout.

Rejected — worst per-call latency (fork+exec per classification), no streaming
shape, requires the Go binary on the consumer's `PATH`. Breaks the "native
binding" framing — c12n-ts would be more like a kit-CLI wrapper than a real
SDK. Doesn't run in browsers / Workers / Deno at all (no subprocess primitive).

## Out of scope

- PHP bindings (separate track, fleet-parity follow-up).
- Browser-only signal variants (e.g. UI-driven classification signals). The
  WASM build runs the same signal set as native bindings; browser-specific
  signals would be a downstream concern.
- A Node-native fast path. Revisited only if a real adopter benchmark shows
  WASM throughput blocking a use case.
- Streaming evaluation. Current FFI is single-shot evaluate; streaming is a
  c12n-core concern, not a binding concern.

## References

- `c12n-core/src/ffi.rs` — C ABI surface c12n-ts must replace with wasm-bindgen
  equivalents.
- `c12n_cgo.go` — Go binding for parity reference (the surface TS mirrors).
- `c12n-py/src/lib.rs` — Python binding showing the per-pipeline tokio runtime
  pattern that c12n-ts must adapt for single-threaded WASM.
- `kit/sdk/ts/` — kit-ts packaging shape (`@hop-top/kit`) that c12n-ts mirrors
  for `package.json`, `tsconfig.json`, `eslint.config.mjs`, `src/`, `test/`.
- Track plan: `.tlc/tracks/c12n-ts-bindings/plan.md` — locked decisions section.

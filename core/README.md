# hop-top-c12n-core

The c12n classification **engine** — low-level Rust core with a C ABI for
cross-language FFI consumers (Go cgo, Python PyO3, PHP FFI, TS WASM).

> [!NOTE]
> **Read-only mirror.** This repo is subtree-pushed from
> [`hop-top/poly-c12n`](https://github.com/hop-top/poly-c12n). Open issues and
> PRs there, not here.

> [!WARNING]
> **Alpha — API, ABI, and tag history may break.** Pin to exact tags, not
> ranges.

## Engine, not SDK

`hop-top-c12n-core` is the raw engine: the signal trait, the fan-out/fan-in
pipeline, embedding + prototype scoring primitives, and the FFI/WASM surfaces
that every other language binding is built on.

Most Rust users do **not** want this crate directly. The ergonomic Rust SDK is
[`hop-top-c12n`](https://crates.io/crates/hop-top-c12n) — it wraps this engine
with a typed `PipelineConfig` builder, a `tracing`-instrumented pipeline, and
re-exports of every engine type. Reach for the SDK first.

Use `hop-top-c12n-core` directly only when you:

- consume the C ABI from another language (cgo / PyO3 / PHP FFI / WASM),
- need the raw primitives without the SDK's config + tracing layer, or
- are building a new language binding.

## Install

```toml
[dependencies]
# Use the latest published alpha tag — see:
# https://crates.io/crates/hop-top-c12n-core
hop-top-c12n-core = "<latest-alpha>"
```

Building as an FFI target produces `libc12n_core.{so,dylib,dll}` — the crate is
declared `crate-type = ["lib", "cdylib"]`.

## Rust surface

The core is small and composable:

- **`Signal`** (`signal.rs`) — the `async_trait` every classifier implements:
  `evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError>`,
  plus `name()` and `signal_type()`.
- **`Pipeline`** (`pipeline.rs`) — fan-out/fan-in orchestrator. Runs all signals
  in parallel under a concurrency semaphore and a per-signal timeout, then
  collects a `PipelineResult { results, errors, duration }`.
- **`EmbeddingEngine`** + `cosine_similarity` (`embedding.rs`) — embedding trait
  (`embed`, `embed_batch`, `dimension`) and a chunked cosine-similarity helper.
- **`PrototypeBank`** (`prototype.rs`) — weighted prototype scoring that blends
  the best match with the mean of the top-M matches.
- **`ClassificationContext`**, **`SignalResult`**, **`SignalType`**,
  **`SignalError`** (`types.rs`) — the shared data types on the wire.

### Signals

`SignalType` declares **20** categories; **16** ship with a concrete `Signal`
implementation in `signals/`:

`Keyword`, `Embedding`, `Domain`, `Context`, `Structure`, `Language`,
`Complexity`, `Preference`, `Feedback`, `OutputFormat`, `CodeContent`,
`ToolCalling`, `CostEstimate`, and three safety signals — `Jailbreak`, `PII`,
`Toxicity`.

The remaining declared-but-unimplemented categories (`Sentiment`, `Intent`,
`Topic`, `Custom`) round out the enum.

## FFI / C ABI surface

`ffi.rs` exposes the pipeline over a `extern "C"` + `#[no_mangle]` surface with
**JSON in / JSON out**. `cbindgen` emits the header at
[`include/libc12n_core.h`](include/libc12n_core.h), which the PHP FFI binding
(and any C consumer) loads. The functions:

| Function | Purpose |
|----------|---------|
| `c12n_pipeline_new(config_json)` | Build a pipeline from `{"max_concurrency","timeout_ms"}`; returns an opaque pointer (null on error). |
| `c12n_pipeline_evaluate(pipeline, context_json)` | Evaluate a `ClassificationContext`; returns a heap JSON string `{results, errors, duration_ms}`, or a JSON error object. |
| `c12n_pipeline_free(pipeline)` | Free a pipeline. |
| `c12n_result_free(result)` | Free a result string returned by `evaluate`. |
| `c12n_result_json(result)` | Identity pass-through (API completeness). |

```c
void *p = c12n_pipeline_new("{\"max_concurrency\":8,\"timeout_ms\":5000}");
char *json = c12n_pipeline_evaluate(p, "{\"text\":\"hello\",\"history\":[],\"headers\":{}}");
c12n_result_free(json);
c12n_pipeline_free(p);
```

The native FFI layer runs on a multi-threaded tokio runtime and is **not**
compiled for `wasm32`.

## WASM surface

Under the `wasm` feature, `wasm.rs` exposes a parallel `#[wasm_bindgen]`
surface consumed by the TypeScript binding (`c12n-ts`, published as
`@hop-top/c12n`). It swaps the multi-threaded runtime for a single-threaded
`current_thread` executor (wasm32 has no thread primitives) and produces the
same `{ results, errors, duration_ms }` shape as the C ABI.

```js
import init, { Pipeline } from "@hop-top/c12n";
await init();
const p = new Pipeline({ max_concurrency: 8, timeout_ms: 5000 });
const out = p.evaluate({ text: "hello", history: [], headers: {}, config: {} });
```

## License

MIT

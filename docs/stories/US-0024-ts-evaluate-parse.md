---
status: shipped
personas: [framework-author, middleware-developer, cost-control-startup]
priority: P0
---

# US-0024: Evaluate a context and parse the result in TypeScript

As a tool author on TypeScript, I want typed accessors over the
classification result so my routing code reads `result.signal('PII')`
rather than indexing a parsed JSON blob.

## Use this when

- Wiring c12n into a Node or Workers request path.
- Reading per-signal confidence for routing or telemetry.
- Writing vitest assertions against classification output.

## Result

`pipeline.evaluate(ctx)` returns the raw JSON envelope as a `string`.
`parseResult(raw)` returns a `PipelineResult` with `signal(t)`,
`hasSignal(t)`, `signals(t)`, `confidence(t?)`, `hasErrors()`, plus
the readonly `results`, `errors`, `duration_ms` fields.

**Today every TS pipeline returns zero signal results.** See
[US-0025](US-0025-ts-no-signals.md) before building on these
accessors.

## Steps

```ts
import { createNodePipeline } from '@hop-top/c12n/nodejs';
import { parseResult, normalizeContext } from '@hop-top/c12n';

const pipeline = await createNodePipeline({
  config: { max_concurrency: 8, timeout_ms: 5000 },
});

const ctx = normalizeContext({ text: 'Write a Python function to sort a list' });
const raw = pipeline.evaluate(ctx);
console.log('raw:', raw);

const result = parseResult(raw);
console.log('signalCount:', pipeline.signalCount());
console.log('results.length:', result.results.length);
console.log('confidence():', result.confidence());
console.log('hasErrors():', result.hasErrors());
console.log('errors:', result.errors);

pipeline.close();
```

Real output, run against wasm built from this tree:

```
raw: {"results":[],"errors":["pipeline has no registered signals; results will be empty"],"duration_ms":0}
signalCount: 0
results.length: 0
confidence(): 0
hasErrors(): true
errors: [ 'pipeline has no registered signals; results will be empty' ]
```

### `normalizeContext` saves you the boilerplate

`ClassificationContext` requires `history`, `headers` and `config` to
be present. `normalizeContext({ text })` fills them with empty
defaults and throws `TypeError` if `text` is missing or non-string.
Without it you write every field by hand:

```ts
const ctx = { text: 'hi', history: [], headers: {}, config: {} };
```

Callers always use **camelCase** (`imageUrl`); the snake_case wire
translation (`image_url`) happens internally in `toWireContext`.

### `confidence()` overloads

- `confidence()` — max confidence across all signals, `0` for an
  empty result set.
- `confidence(type)` — that signal's confidence, `0` if absent.

Both return `0` rather than `undefined`, so `0` is ambiguous between
"scored zero" and "never ran". Branch on `result.hasSignal(type)` when
the distinction matters.

### Lifecycle

`close()` is idempotent. After it, `evaluate()` throws and
`signalCount()` returns `0`:

```ts
pipeline.close();
pipeline.evaluate(ctx);
// after close: c12n: pipeline is closed
```

One pipeline evaluates sequentially — the wasm executor is
single-threaded (`new_current_thread` in `core/src/wasm.rs`), so
concurrent callers must serialise themselves.

### Optional structured logging

`Logger` is a structural interface (`info`/`warn`/`error`/`debug`).
Pass any duck-typed object; the package takes no logging dependency:

```ts
const pipeline = await createNodePipeline({ logger: console });
```

## Verify

```bash
cd ts
npx --yes pnpm@9 build:wasm:nodejs
npx --yes pnpm@9 vitest run --exclude test/bundler-smoke.test.ts
```

```
 ✓ test/pipeline.test.ts  (8 tests) 4ms
 ✓ test/pipeline.integration.test.ts  (13 tests) 24ms

 Test Files  2 passed (2)
      Tests  21 passed (21)
```

The snippet above was executed directly against `src/`; its output is
pasted verbatim.

## How it works

`parseResult` validates the shape before constructing: it throws
`TypeError` if `results` or `errors` is not an array or `duration_ms`
is not a number, and `SyntaxError` (from `JSON.parse`) on malformed
JSON. It does **not** validate individual `SignalResult` entries — the
`SignalType` union in `result.ts` is a compile-time claim about what
the core emits, not a runtime check.

Errors crossing the wasm boundary arrive as `JsValue` (string or
object). `wrapWasmError` converts them to native `Error` instances
prefixed with `c12n:`, so callers `try/catch (e: Error)` idiomatically
instead of handling raw `JsValue`.

`toWireContext` runs inside `evaluate`'s `try`, so a malformed context
surfaces through the same `evaluate.failed` log branch as a wasm-side
failure rather than escaping unlogged.

## Tests

- [`ts/test/pipeline.integration.test.ts`](../../ts/test/pipeline.integration.test.ts)
  — real-wasm roundtrip, `parseResult` accessors, close semantics,
  cross-surface JSON-shape parity with Go and Python
- [`ts/test/pipeline.test.ts`](../../ts/test/pipeline.test.ts) —
  `normalizeContext`, `parseResult` validation, logger plumbing

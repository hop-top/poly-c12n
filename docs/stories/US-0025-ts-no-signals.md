---
status: partial
personas: [framework-author, middleware-developer, llm-routing-saas]
priority: P0
---

# US-0025: Handle errors and the NoSignals diagnostic in TypeScript

As a tool author on TypeScript, I want to know which failures throw,
which arrive as diagnostics inside a successful result, and why my
pipeline returns zero signals today.

## Use this when

- A TS pipeline evaluates successfully but `results` is empty.
- Deciding whether to `try/catch` or to inspect `result.errors`.
- Assessing whether c12n on TS can meet a routing requirement yet.

## Result

Two distinct failure channels:

- **Thrown** — native `Error` instances. `c12n: pipeline is closed`,
  `c12n: failed to create pipeline: ...`, `c12n: evaluate failed: ...`
  from the wasm boundary; `TypeError` / `SyntaxError` from
  `parseResult` and `normalizeContext`.
- **Returned** — `result.errors: string[]` carries per-evaluation
  diagnostics on an otherwise successful call.

## The NoSignals diagnostic

An unconfigured pipeline now reports itself loudly instead of
returning a silently-empty envelope:

```json
{"results":[],"errors":["pipeline has no registered signals; results will be empty"],"duration_ms":0}
```

`result.hasErrors()` is therefore `true` for a default pipeline. This
is `PipelineError::NoSignals`, and it is **expected** on TS today, not
a misconfiguration on your side.

### Why TS gets zero signals

**TS cannot configure which detectors run.** `PipelineConfig` carries
only two numeric fields:

```ts
export interface PipelineConfig {
  max_concurrency?: number;  // default 8
  timeout_ms?: number;       // default 5000
}
```

Detectors and tiered chains exist in the Rust core
(`core/src/registry.rs`, `core/src/chain.rs`, ADR-0003), but the wasm
constructor in `core/src/wasm.rs` builds
`InnerPipeline::new(vec![], cfg.max_concurrency, ...)` — a hardcoded
empty signal vector — and neither `wasm.rs` nor `WasmPipelineConfig`
references the registry at all.

So the honest current state: **a TS caller gets a pipeline that
constructs, evaluates, and closes correctly, and returns zero results
plus the NoSignals diagnostic.** `pipeline.signalCount()` returns `0`
and there is no option that changes it. Do not ship routing logic that
depends on a signal firing; there is nothing to configure that would
make one fire.

Treat this story's status as `partial` until detector config is
plumbed through `core/src/wasm.rs`.

## Steps

```ts
import { createNodePipeline } from '@hop-top/c12n/nodejs';
import { parseResult, normalizeContext } from '@hop-top/c12n';

let pipeline;
try {
  pipeline = await createNodePipeline();
} catch (err) {
  // wasm module missing / failed to init.
  console.error('c12n unavailable:', (err as Error).message);
  process.exit(1);
}

try {
  const raw = pipeline.evaluate(normalizeContext({ text: 'classify me' }));
  const result = parseResult(raw);

  // Diagnostics ride along with a SUCCESSFUL evaluation.
  if (result.hasErrors()) {
    for (const diag of result.errors) {
      console.log('diagnostic:', diag);
    }
  }

  // Zero signals today — guard rather than assuming one fired.
  if (result.hasSignal('PII')) {
    // ...route on PII
  }
} catch (err) {
  console.error('evaluate failed:', (err as Error).message);
} finally {
  pipeline.close();
}
```

Real output:

```
diagnostic: pipeline has no registered signals; results will be empty
```

Thrown-channel example, executed:

```ts
pipeline.close();
pipeline.evaluate(ctx);
// after close: c12n: pipeline is closed
```

Distinguishing the two channels matters: a `NoSignals` diagnostic
should not page anyone, while a thrown `Error` means the call did not
happen.

## Verify

```bash
cd ts
npx --yes pnpm@9 build:wasm:nodejs
npx --yes pnpm@9 vitest run --exclude test/bundler-smoke.test.ts
```

```
 Test Files  2 passed (2)
      Tests  21 passed (21)
```

`pipeline.integration.test.ts` asserts `errors` has length 1 matching
`/no registered signals/`, and that `hasErrors()` is `true` for an
unconfigured pipeline. The cross-surface parity cases assert only
`Array.isArray(parsed.errors)` — contents depend on configuration,
not on which binding you use.

## How it works

`core/src/wasm.rs` stringifies errors before adding them to the result
(`errors: result.errors.iter().map(|e| e.to_string())`), so the TS
side receives `string[]`, not structured variants — hence
`export type ResultError = string`. If the core later serialises
structured errors, that type widens.

`close()` swallows and logs failures from wasm-bindgen's `free()`
rather than rethrowing, keeping it idempotent and safe inside
`finally` blocks. `signalCount()` short-circuits to `0` after close
instead of touching the freed object.

`parseResult` rejects a malformed envelope before you can read
accessors off it, so `result.errors` is always an array by the time
you loop it.

## Tests

- [`ts/test/pipeline.integration.test.ts`](../../ts/test/pipeline.integration.test.ts)
  — no-signals diagnostic, `hasErrors()` on an empty pipeline,
  `confidence()` on an empty result set, close semantics, parity
- [`ts/test/pipeline.test.ts`](../../ts/test/pipeline.test.ts) —
  `parseResult` `TypeError` paths, `normalizeContext` `TypeError`
- [`ts/test/bundler-smoke.test.ts`](../../ts/test/bundler-smoke.test.ts)
  — same diagnostic asserted through the bundler target
- [`docs/adr/0003-tiered-detector-chains.md`](../adr/0003-tiered-detector-chains.md)
  — the chain design TS cannot yet reach

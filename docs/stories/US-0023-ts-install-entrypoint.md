---
status: shipped
personas: [framework-author, middleware-developer, internal-tool-builder]
priority: P0
---

# US-0023: Install `@hop-top/c12n` and pick the right entrypoint

As a tool author on TypeScript, I want to import c12n from the
entrypoint that matches my runtime, so the wasm module actually
resolves instead of failing at import time.

## Use this when

- Adding c12n to a Node CLI, a Vite/webpack app, or a Cloudflare Worker.
- `Cannot find module '.../pkg/bundler/c12n_core.js'` at runtime.
- Deciding between `@hop-top/c12n` and `@hop-top/c12n/nodejs`.

## Result

Two subpath exports, each bound to a different wasm-pack target. Pick
by runtime, not by preference:

| Import | wasm target | Use for |
|---|---|---|
| `@hop-top/c12n` | `pkg/bundler/` | Vite, webpack, Rollup, esbuild, Cloudflare Workers (wrangler), browsers |
| `@hop-top/c12n/nodejs` | `pkg/nodejs/` | Plain Node — CLIs, scripts, vitest in default mode |

Requires Node >= 20.

## Steps

```bash
npm install @hop-top/c12n   # or: pnpm add @hop-top/c12n
```

**Bundler / browser / Workers** — the default subpath:

```ts
import { Pipeline, parseResult } from '@hop-top/c12n';

const pipeline = await Pipeline.create({ config: { max_concurrency: 8 } });
```

**Plain Node, no bundler** — the `/nodejs` subpath, and note the
different factory function:

```ts
import { createNodePipeline } from '@hop-top/c12n/nodejs';
import { parseResult, normalizeContext } from '@hop-top/c12n';

const pipeline = await createNodePipeline({
  config: { max_concurrency: 8, timeout_ms: 5000 },
});
```

### The gotcha: `Pipeline.create()` is bundler-only

`@hop-top/c12n/nodejs` re-exports the whole public API, so
`Pipeline.create` is importable from it — and it still resolves
`pkg/bundler/`. Under plain Node that throws:

```
bundler path FAILED under plain Node:
Cannot find module '.../ts/pkg/bundler/c12n_core.js' imported from .../ts/src/wasm-loader.js
```

Under Node use `createNodePipeline()`, or `new Pipeline({ wasm })`
with a module from `loadNodejs()`. `Pipeline.create()` and
`createNodePipeline()` take the same options and return the same
`Pipeline` — only the loader differs.

Auto-detection is deliberately not attempted: bundlers cannot
statically resolve a runtime `typeof process` branch, and conditional
`require()` inside ESM warns under Node and errors under bundlers.
Subpath exports are the resolution mechanism.

## Verify

```bash
cd ts
npx --yes pnpm@9 install --frozen-lockfile --ignore-workspace
npx --yes pnpm@9 build:wasm:nodejs      # requires wasm-pack + cargo
npx --yes pnpm@9 vitest run --exclude test/bundler-smoke.test.ts
```

```
 RUN  v1.6.1 .../ts

 ✓ test/pipeline.test.ts  (8 tests) 4ms
 ✓ test/pipeline.integration.test.ts  (13 tests) 24ms

 Test Files  2 passed (2)
      Tests  21 passed (21)
```

Without `build:wasm:nodejs` the 13 integration tests **skip** rather
than fail, so `pnpm test` stays green for consumers without
`wasm-pack`. Check the count — 8 passing means the wasm path never
ran.

`test/bundler-smoke.test.ts` is excluded above because it needs a real
browser; run it with `pnpm test:bundler`.

## How it works

`package.json#exports` maps `.` to `dist/index.{js,cjs}` and `./nodejs`
to `dist/nodejs.{js,cjs}`. `src/wasm-loader.ts:loadBundler()` dynamic-
imports `../pkg/bundler/c12n_core.js`; `src/nodejs.ts:loadNodejs()`
imports `../pkg/nodejs/c12n_core.js`.

The `--target bundler` glue may expose an async `init` the host must
call once. `Pipeline.create()` calls `wasm.default()` defensively — a
no-op when the bundler already initialised it. The `--target nodejs`
glue reads the `.wasm` synchronously via `fs`, so `createNodePipeline()`
skips that step; it only calls `setPanicHook()`.

Both paths are generated from the same `core/src/wasm.rs`, so the
`Pipeline` class shape and JSON envelope are identical.

## Tests

- [`ts/test/pipeline.integration.test.ts`](../../ts/test/pipeline.integration.test.ts)
  — real-wasm roundtrip via the nodejs target
- [`ts/test/pipeline.test.ts`](../../ts/test/pipeline.test.ts) —
  pure-TS surface, no wasm required
- [`ts/test/setup.ts`](../../ts/test/setup.ts) — `hasWasm()` /
  `wasmRuntimeOk()` gating
- [`ts/test/bundler-smoke.test.ts`](../../ts/test/bundler-smoke.test.ts)
  — bundler target in a real browser

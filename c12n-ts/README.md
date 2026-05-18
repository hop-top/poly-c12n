# `@hop-top/c12n`

TypeScript bindings for [`c12n-core`][core] — a classification pipeline
runtime — shipped as a single WebAssembly artifact that runs across
Node.js, browsers, Cloudflare Workers, and Deno.

See [ADR-0001][adr] for the locked binding decisions.

## Install

```sh
npm install @hop-top/c12n
# or
pnpm add @hop-top/c12n
# or
yarn add @hop-top/c12n
```

No prebuild matrix, no `node-gyp`, no postinstall compilation. The `.wasm`
artifact is shipped with the package.

## Quickstart

### Node / bundler (Vite, webpack, esbuild, wrangler)

```ts
import { Pipeline, parseResult } from '@hop-top/c12n';

const pipeline = await Pipeline.create({
  config: { max_concurrency: 8, timeout_ms: 5000 },
});

const raw = pipeline.evaluate({
  text: 'Tell me about NextGen Cluster Lab.',
  history: [],
  headers: {},
  config: {},
});

const result = parseResult(raw);
console.log('max confidence:', result.confidence());
console.log('PII confidence:', result.confidence('PII'));

pipeline.close();
```

### Node without a bundler

```ts
import { Pipeline, parseResult } from '@hop-top/c12n/nodejs';

const pipeline = await Pipeline.create();
// ... same as above
```

### Browser

The `.` subpath is bundler-friendly out of the box. Vite, webpack 5,
Rollup, and esbuild all resolve the `.wasm` asset automatically. No
extra config required for standard web bundles.

### Cloudflare Workers

The `.` subpath ships a `--target bundler` artifact; wrangler v3+
bundles the wasm asset natively.

```ts
import { Pipeline } from '@hop-top/c12n';

export default {
  async fetch(req: Request): Promise<Response> {
    const pipeline = await Pipeline.create();
    const raw = pipeline.evaluate({ text: await req.text(), history: [], headers: {}, config: {} });
    pipeline.close();
    return new Response(raw, { headers: { 'content-type': 'application/json' } });
  },
};
```

## API surface

| Export                 | Kind        | Purpose                                                  |
|------------------------|-------------|----------------------------------------------------------|
| `Pipeline`             | class       | Wraps the wasm-bindgen Pipeline. Construct + `evaluate`. |
| `Pipeline.create`      | static fn   | Lazy-load the wasm module + return a ready Pipeline.     |
| `PipelineConfig`       | type        | `{ max_concurrency?, timeout_ms? }`.                     |
| `PipelineOptions`      | type        | Constructor input for `new Pipeline(...)`.               |
| `Logger`               | type        | Structural logger interface (kit-ts compatible).         |
| `ClassificationContext`| type        | Input shape (`text`, `history`, `headers`, `config`, …). |
| `normalizeContext`     | fn          | Fill nil collections with empty defaults.                |
| `toWireContext`        | fn          | camelCase → snake_case wire shape (internal helper).     |
| `parseResult`          | fn          | Deserialize raw JSON result into a typed accessor.       |
| `PipelineResult`       | class       | Typed accessors: `signal()`, `confidence()`, etc.        |
| `SignalType`           | type        | Union of all signal types (`'PII' \| 'Toxicity' \| …`).  |
| `SignalResult`         | type        | Individual signal output shape.                          |
| `loadBundler`          | fn          | Direct wasm-bindgen module loader (advanced).            |

## Logging

`Pipeline` accepts an optional `Logger` matching [`@hop-top/kit`][kit]'s
`Logger` shape. Pass `createLogger()` from kit-ts to get structured
event topics under `c12n.pipeline.*`:

```ts
import { createLogger } from '@hop-top/kit/log'; // see kit-ts log surface
import { Pipeline } from '@hop-top/c12n';

const logger = createLogger();
const pipeline = await Pipeline.create({ logger });
```

Events emitted:
- `c12n.pipeline.init.ok` / `.init.failed`
- `c12n.pipeline.evaluate.start` / `.evaluate.ok` / `.evaluate.failed`
- `c12n.pipeline.close.ok` / `.close.failed`

## Development

```sh
# Build the wasm artifact (requires rustup + wasm32 target)
pnpm build:wasm:bundler
pnpm build:wasm:nodejs

# Build TS
pnpm build:ts

# Full build (wasm + TS)
pnpm build

# Test (vitest, no wasm required for smoke tests)
pnpm test

# Lint
pnpm lint
```

Prereqs for the wasm build:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-pack
```

The CI pipeline handles all of this on every release; local builds are
only needed for development.

## Constraints

Per [ADR-0001][adr], the wasm build uses a single-threaded tokio
executor (`new_current_thread`). Signals within a single `evaluate()`
call run sequentially. This matches how each existing binding (Go,
Python, Rust) is called from a request handler. If you need
multi-threaded classification, use the Go (`c12n_cgo.go`) or Python
(`c12n-py`) bindings.

## Related

- [`c12n-core`][core] — Rust classification engine (parent crate).
- [`c12n` Go binding][gobind] — native cgo binding (multi-threaded).
- [`c12n-py`][pybind] — Python binding (multi-threaded).
- [`@hop-top/kit`][kit] — shared CLI utilities (logger, output, config).
- [ADR-0001][adr] — locked decision: WASM via wasm-bindgen.

## License

MIT.

[core]: ../c12n-core
[adr]: ../docs/adr/0001-c12n-ts-wasm-binding.md
[kit]: https://www.npmjs.com/package/@hop-top/kit
[gobind]: ../c12n_cgo.go
[pybind]: ../c12n-py

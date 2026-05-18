/**
 * Environment-aware loader for the wasm-bindgen generated bindings.
 *
 * Per ADR-0001 we ship two wasm-pack targets in `pkg/`:
 *   - `pkg/bundler/` — `--target bundler` (Vite, webpack, Rollup, esbuild,
 *     Cloudflare Workers via wrangler, Deno bundlers). The default. The
 *     generated JS uses `import * as wasm from "...wasm"` which bundlers
 *     resolve as a binary asset.
 *   - `pkg/nodejs/`  — `--target nodejs` (`require()`-friendly, reads the
 *     `.wasm` synchronously from disk via `fs`).
 *
 * Resolution strategy:
 *   - Default subpath (`.`) re-exports from `pkg/bundler`. Bundlers and edge
 *     runtimes get the artifact they expect.
 *   - Explicit `./nodejs` subpath re-exports from `pkg/nodejs`. Node-without-
 *     bundler consumers (CLIs, scripts, vitest in default mode) import via
 *     `@hop-top/c12n/nodejs` to avoid the bundler glue.
 *
 * Auto-detection is intentionally NOT done here: bundlers can't statically
 * resolve a runtime `typeof process` branch, and conditional `require()` in
 * an ESM module triggers warnings under Node + bundler errors. Sub-path
 * exports (`package.json` `"exports"`) are the canonical solution and let
 * consumers + bundlers pick the right artifact at resolve time.
 *
 * This file is a thin re-export shim for the bundler target; see
 * `src/nodejs.ts` (built to `dist/nodejs.{js,cjs,d.ts}`) for the nodejs
 * counterpart.
 *
 * NOTE: This file imports from `../pkg/bundler` which only exists after
 * `pnpm build:wasm:bundler` has run. Until then, `tsc` will error — that is
 * intentional. The CI workflow runs the wasm build before the TS build.
 */

// The wasm-bindgen output exposes a `Pipeline` class plus an `init` default
// export (or a sync constructor under `--target nodejs`). We re-export the
// generated types; downstream files type-narrow as needed.
//
// Until `wasm-pack build` has run, `../pkg/bundler` does not exist on disk.
// We declare the module shape here so `tsc` against the source can resolve
// imports without the pkg/ directory present. The wasm-bindgen generated
// `.d.ts` overrides this shape once it lands in `pkg/bundler/`.
//
// Generated surface (per c12n-core/src/wasm.rs):
//   default export `init(input?): Promise<void>`
//   export class Pipeline { constructor(config: any); evaluate(ctx: any): string; signalCount(): number; free(): void; }
//   export function setPanicHook(): void;
export interface WasmPipelineCtor {
  new (config: unknown): WasmPipelineInstance;
}

export interface WasmPipelineInstance {
  evaluate(ctx: unknown): string;
  signalCount(): number;
  free(): void;
}

export interface WasmModule {
  Pipeline: WasmPipelineCtor;
  setPanicHook?: () => void;
  default?: (input?: unknown) => Promise<unknown>;
}

/**
 * Load the bundler-target wasm module.
 *
 * Bundlers (Vite, webpack, esbuild, wrangler) resolve `../pkg/bundler` at
 * build time and inline the `.wasm` asset. Cloudflare Workers consumers
 * should call the returned module's `default()` once on startup if the
 * generated glue requires it.
 */
export async function loadBundler(): Promise<WasmModule> {
  // Dynamic import keeps the top-level synchronous and lets the host
  // runtime (bundler) decide when to materialise the wasm bytes.
  const mod = (await import(/* @vite-ignore */ '../pkg/bundler/c12n_core.js')) as WasmModule;
  return mod;
}

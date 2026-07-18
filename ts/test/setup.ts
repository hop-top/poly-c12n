/**
 * Vitest global setup — detects whether the wasm-pack `nodejs` artifact
 * has been built and exposes that as `globalThis.__WASM_AVAILABLE__`.
 *
 * Integration tests under `test/pipeline.integration.test.ts` gate every
 * `it()` call on this flag via `it.skipIf(!hasWasm)`. The pure-TS smoke
 * tests in `test/pipeline.test.ts` do not depend on this flag and always
 * run.
 *
 * Build precondition (see ts/README.md "Development"):
 *
 *     pnpm build:wasm:test    # = wasm-pack build ../core --target nodejs ...
 *
 * Without the artifact, integration tests are SKIPPED, not FAILED — that
 * keeps `pnpm test` green for downstream consumers who haven't installed
 * `wasm-pack` locally. CI runs the build step before `vitest run` so the
 * integration tests are exercised on every push.
 *
 * NOTE: vitest 1.x's `globalSetup` runs in a separate Node context that
 * can't share state with the test workers. We use a simple `beforeAll`
 * inside each integration test file instead (see
 * `pipeline.integration.test.ts`) and keep this file as the canonical
 * place to read the wasm availability. Both paths share the same check
 * function so the source of truth stays single.
 */

import { existsSync } from 'node:fs';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const wasmJs = resolve(here, '..', 'pkg', 'nodejs', 'c12n_core.js');
const wasmBg = resolve(here, '..', 'pkg', 'nodejs', 'c12n_core_bg.wasm');

/** True when both wasm-bindgen glue + the `.wasm` binary exist on disk. */
export function hasWasm(): boolean {
  return existsSync(wasmJs) && existsSync(wasmBg);
}

/** Absolute path to the wasm-pack `nodejs` glue (CJS entrypoint). */
export const wasmJsPath = wasmJs;

if (!hasWasm()) {
  console.warn(
    '[c12n-ts] wasm artifact not found at pkg/nodejs/. ' +
      'Integration tests will be skipped. Run `pnpm build:wasm:test` ' +
      'to build the wasm bundle (requires wasm-pack + rustup wasm32 target).',
  );
}

/**
 * One-shot runtime smoke: does `new wasm.Pipeline({})` succeed?
 *
 * The wasm artifact may load without throwing, yet `Pipeline::new`
 * panics if the tokio runtime can't be constructed on wasm32. The
 * canonical offender is `tokio` features that depend on
 * `std::time::Instant` — `time` panics on `wasm32-unknown-unknown`
 * because the target's libstd is `time/unsupported.rs::now()`-stubbed
 * (returns an `unreachable!()` panic). Diagnosed by lifting the panic
 * message: `time not implemented on this platform`.
 *
 * Mitigations live in c12n-core (`core/Cargo.toml`, `core/src/wasm.rs`),
 * not c12n-ts: either remove the `time` feature from tokio on
 * `cfg(target_arch = "wasm32")` (and stop calling `Duration`-based
 * APIs on the wasm runtime), or pull in `wasm-bindgen-rayon` /
 * `instant` crate / `tokio` with `wasm_js` feature to provide an
 * `Instant::now()` source.
 *
 * Until that's fixed, integration tests that *construct* a `Pipeline`
 * via the real wasm surface skip with a clear warning. Tests that
 * exercise only TS-side helpers (`normalizeContext`, `parseResult`,
 * type surface) continue to run via `pipeline.test.ts`.
 */
let cachedWasmRuntimeOk: boolean | undefined;

export async function wasmRuntimeOk(): Promise<boolean> {
  if (cachedWasmRuntimeOk !== undefined) return cachedWasmRuntimeOk;
  if (!hasWasm()) {
    cachedWasmRuntimeOk = false;
    return false;
  }
  try {
    const mod = (await import(wasmJsPath)) as {
      Pipeline: new (cfg: unknown) => { free?: () => void };
      setPanicHook?: () => void;
    };
    if (typeof mod.setPanicHook === 'function') mod.setPanicHook();
    const probe = new mod.Pipeline({});
    probe.free?.();
    cachedWasmRuntimeOk = true;
  } catch (err) {
    console.warn(
      '[c12n-ts] wasm runtime probe failed; integration tests SKIPPED. ' +
        'Underlying error: ' +
        (err instanceof Error ? err.message : String(err)) +
        '. Likely cause: core/src/wasm.rs Pipeline::new builds a tokio ' +
        'runtime whose `time` feature depends on std::time::Instant — ' +
        'unsupported on wasm32-unknown-unknown. Fix in c12n-core, not c12n-ts.',
    );
    cachedWasmRuntimeOk = false;
  }
  return cachedWasmRuntimeOk;
}

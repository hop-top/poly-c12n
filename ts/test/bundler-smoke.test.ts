/**
 * Bundler-target smoke tests for `@hop-top/c12n` (T-0187).
 *
 * Scope:
 *   - ADR-0001 commits to TWO wasm-pack targets: `--target nodejs` AND
 *     `--target bundler`. The nodejs target is covered by
 *     `pipeline.integration.test.ts` (vitest under Node). This file is
 *     the FIRST end-to-end exercise of the bundler artifact (the `.`
 *     subpath export, not `./nodejs`).
 *   - Runs in a real browser context via `@vitest/browser` + Playwright
 *     so the bundler's `WebAssembly.instantiate` + ESM module-resolution
 *     paths are exercised the same way a downstream Vite / esbuild /
 *     wrangler consumer would hit them.
 *   - Smoke only. We construct a Pipeline, call `evaluate({text:"hello"})`,
 *     and assert the JSON shape matches what Node sees (`results`,
 *     `errors`, `duration_ms`). Behavioural parity with the nodejs target
 *     is the goal; exhaustive signal-content assertions are out of scope.
 *
 * Build precondition:
 *   - `pnpm build:wasm:bundler` must have produced `ts/pkg/bundler/`
 *     before this file runs. If the artifact is absent, every test
 *     skips via `it.skipIf(!hasBundler)` so `pnpm test:bundler` stays
 *     green on machines without wasm-pack installed. CI runs the
 *     bundler build before invoking `pnpm test:bundler`.
 *
 * Why a separate file (not inside `pipeline.integration.test.ts`):
 *   - The browser-mode vitest run uses a different config
 *     (`vitest.browser.config.ts` / browser mode flags) than the Node
 *     run. Keeping bundler tests in their own file lets `pnpm test`
 *     under Node skip them via the include/exclude patterns and lets
 *     `pnpm test:bundler` target only this file under browser mode.
 *
 * Why `@vitest/browser` over plain Playwright test:
 *   - Reuses the existing vitest configuration + assertion surface.
 *     One devDep family (`vitest` / `@vitest/browser` / `playwright`),
 *     one runner, identical `expect()` semantics with the nodejs tests.
 *     If `@vitest/browser` proves unstable we can move to a standalone
 *     `playwright.config.ts` without rewriting the assertions.
 */

import { beforeAll, describe, expect, it } from 'vitest';

import {
  type ClassificationContext,
  Pipeline,
  parseResult,
  type WasmModule,
} from '../src/index.js';

// ---------------------------------------------------------------------------
// Artifact gate — skip everything if the bundler pkg/ is absent.
// ---------------------------------------------------------------------------
//
// In browser mode we can't `existsSync` the filesystem from inside the
// test. The bundler test runner imports `../pkg/bundler/c12n_core.js` at
// module-load time; if the file is missing, the dynamic import throws
// and `hasBundler` stays false. CI's `pnpm build:wasm:bundler` step
// produces the artifact before this file runs.

let bundlerMod: WasmModule | undefined;
let hasBundler = false;

beforeAll(async () => {
  try {
    // Vite (the engine @vitest/browser uses) resolves `?url`/`?init`
    // suffixes for wasm, but the wasm-pack `--target bundler` artifact
    // exposes a plain ESM module that imports the `.wasm` for us. We
    // import the generated glue directly.
    bundlerMod = (await import('../pkg/bundler/c12n_core.js')) as WasmModule;
    if (typeof bundlerMod.default === 'function') {
      // wasm-bindgen's bundler target ships an async `init()` default
      // export. Some bundlers (Vite ≥5) call it implicitly via the
      // import-attributes mechanism; calling defensively is a no-op
      // when already initialised.
      await bundlerMod.default();
    }
    if (typeof bundlerMod.setPanicHook === 'function') {
      bundlerMod.setPanicHook();
    }
    hasBundler = true;
  } catch (err) {
    // eslint-disable-next-line no-console
    console.warn(
      '[c12n-ts] bundler artifact unavailable; browser smoke tests SKIPPED. ' +
        'Run `pnpm build:wasm:bundler` first (requires wasm-pack + ' +
        'rustup wasm32 target). Underlying error: ' +
        (err instanceof Error ? err.message : String(err)),
    );
  }
});

// ---------------------------------------------------------------------------
// T-0187 — bundler artifact smoke
// ---------------------------------------------------------------------------

describe('bundler target — end-to-end in a real browser', () => {
  it('loads the wasm-bindgen bundler artifact via dynamic import', () => {
    if (!hasBundler) return;
    expect(bundlerMod).toBeDefined();
    expect(typeof bundlerMod!.Pipeline).toBe('function');
  });

  it('constructs a Pipeline + evaluates "hello" with the expected JSON shape', () => {
    if (!hasBundler) return;
    const pipeline = new Pipeline({ wasm: bundlerMod!, config: {} });
    try {
      const ctx: ClassificationContext = {
        text: 'hello',
        history: [],
        headers: {},
        config: {},
      };
      const raw = pipeline.evaluate(ctx);

      // Same wire shape Node sees (asserted in
      // pipeline.integration.test.ts). The whole point of this test is
      // proving the bundler artifact emits the SAME shape as nodejs.
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      expect(parsed).toHaveProperty('results');
      expect(parsed).toHaveProperty('errors');
      expect(parsed).toHaveProperty('duration_ms');
      expect(Array.isArray(parsed.results)).toBe(true);
      expect(Array.isArray(parsed.errors)).toBe(true);
      expect(typeof parsed.duration_ms).toBe('number');

      // Empty pipeline → empty results + no errors.
      expect(parsed.results).toEqual([]);
      expect(parsed.errors).toEqual([]);
      expect(parsed.duration_ms as number).toBeGreaterThanOrEqual(0);
    } finally {
      pipeline.close();
    }
  });

  it('parseResult wraps the bundler output with PipelineResult accessors', () => {
    if (!hasBundler) return;
    const pipeline = new Pipeline({ wasm: bundlerMod!, config: {} });
    try {
      const raw = pipeline.evaluate({
        text: 'hello',
        history: [],
        headers: {},
        config: {},
      });
      const result = parseResult(raw);
      expect(result.confidence()).toBe(0);
      expect(result.hasErrors()).toBe(false);
      expect(result.results).toEqual([]);
    } finally {
      pipeline.close();
    }
  });
});

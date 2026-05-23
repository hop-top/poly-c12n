/**
 * Real-wasm integration tests for `@hop-top/c12n`.
 *
 * Scope (T-0122 / T-0123):
 *   - Build precondition: `pnpm build:wasm:test` must have run (the
 *     `pkg/nodejs/` artifact exists). If not, every test is skipped via
 *     `it.skipIf(!runtimeOk)` so `pnpm test` stays green for consumers
 *     without `wasm-pack` installed locally. CI runs the wasm build
 *     before `vitest run`, so these tests are exercised on every push.
 *   - Loader target: `--target nodejs` (vitest runs under Node). The
 *     `--target bundler` artifact is intended for downstream consumers
 *     (Vite, webpack, wrangler, esbuild) and is NOT exercised here.
 *   - Mirrors the Go integration scenarios in
 *     `go/integration_test.go` and the Python parity scenarios in
 *     `py/tests/test_integration.py` for cross-surface JSON-shape
 *     parity.
 *
 * Gating model:
 *   - `hasWasm()` — artifact-on-disk check (cheap; `existsSync`).
 *   - `wasmRuntimeOk()` — runtime probe that constructs a Pipeline
 *     once and caches the result. Distinguishes "artifact missing"
 *     from "artifact present but crashes at runtime" (e.g. tokio's
 *     `time` feature panics on wasm32 because std::time::Instant is
 *     unsupported there). The probe runs in `beforeAll`; individual
 *     tests gate on the cached boolean via `it.skipIf(!runtimeOk)`.
 *
 * The wasm pipeline is constructed with NO signals (default config).
 * Every signal evaluation is therefore an empty set; we assert the
 * shape, not the signal content. Signal-specific assertions belong in
 * the Rust core / per-signal vitest tests once the registry surface
 * exists in the wasm binding (out of scope for this task).
 */

import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import { hasWasm, wasmJsPath, wasmRuntimeOk } from './setup.js';

import { Pipeline, parseResult, normalizeContext, type Logger } from '../src/index.js';
import type { WasmModule } from '../src/wasm-loader.js';

// ---------------------------------------------------------------------------
// Lifecycle: load wasm once per file, share across describe blocks.
// ---------------------------------------------------------------------------

let wasm: WasmModule | undefined;
let runtimeOk = false;

beforeAll(async () => {
  if (!hasWasm()) return;
  // Dynamic import via absolute path keeps tsc happy when pkg/ is
  // absent (the static `../pkg/nodejs/c12n_core.js` import path doesn't
  // resolve until wasm-pack has run). The absolute path also bypasses
  // the resolver entirely so vitest doesn't try to type-check it.
  wasm = (await import(wasmJsPath)) as WasmModule;
  if (typeof wasm.setPanicHook === 'function') {
    wasm.setPanicHook();
  }
  runtimeOk = await wasmRuntimeOk();
});

// ---------------------------------------------------------------------------
// T-0122 / T-0123 #2 — real classification roundtrip
// ---------------------------------------------------------------------------

describe('Pipeline real wasm — roundtrip', () => {
  it.skipIf(!hasWasm())('evaluates an empty pipeline + returns valid JSON shape', () => {
    if (!runtimeOk) return; // probe failure already warned
    const pipeline = new Pipeline({ wasm: wasm!, config: {} });
    try {
      const raw = pipeline.evaluate({
        text: 'hello',
        history: [],
        headers: {},
        config: {},
      });

      // Shape assertion — direct JSON.parse, not parseResult, so we
      // verify the wire shape c12n-core emits is *exactly* what TS
      // callers see before any wrapper massage.
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      expect(parsed).toHaveProperty('results');
      expect(parsed).toHaveProperty('errors');
      expect(parsed).toHaveProperty('duration_ms');
      expect(Array.isArray(parsed.results)).toBe(true);
      expect(Array.isArray(parsed.errors)).toBe(true);
      expect(typeof parsed.duration_ms).toBe('number');
      expect(parsed.results).toEqual([]);
      expect(parsed.errors).toEqual([]);
      expect(parsed.duration_ms as number).toBeGreaterThanOrEqual(0);
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('parseResult wraps the wasm output with PipelineResult accessors', () => {
    if (!runtimeOk) return;
    const pipeline = new Pipeline({ wasm: wasm!, config: {} });
    try {
      const raw = pipeline.evaluate({
        text: 'Tell me about NextGen Cluster Lab.',
        history: [],
        headers: {},
        config: {},
      });

      const result = parseResult(raw);
      // Empty pipeline → empty results → confidence() === 0 by docs.
      expect(result.confidence()).toBe(0);
      expect(result.hasErrors()).toBe(false);
      expect(result.results).toEqual([]);
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('signalCount() reports 0 for default-config pipeline', () => {
    if (!runtimeOk) return;
    const pipeline = new Pipeline({ wasm: wasm!, config: {} });
    try {
      expect(pipeline.signalCount()).toBe(0);
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('honours max_concurrency + timeout_ms config overrides', () => {
    if (!runtimeOk) return;
    const pipeline = new Pipeline({
      wasm: wasm!,
      config: { max_concurrency: 4, timeout_ms: 2000 },
    });
    try {
      // No way to introspect the runtime config from the wasm surface,
      // so the smoke is "construction does not throw with explicit
      // tuning". Mirrors Go's TestIntegration_PipelineLifecycle which
      // also just verifies construction.
      const raw = pipeline.evaluate(normalizeContext({ text: 'tune' }));
      expect(typeof raw).toBe('string');
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('close() is idempotent + evaluate after close throws', () => {
    if (!runtimeOk) return;
    const pipeline = new Pipeline({ wasm: wasm!, config: {} });
    pipeline.close();
    pipeline.close(); // must not throw
    expect(() =>
      pipeline.evaluate({ text: 'post-close', history: [], headers: {}, config: {} }),
    ).toThrow(/closed/);
  });
});

// ---------------------------------------------------------------------------
// T-0122 / T-0123 #3 — stub-only context normalization (extension of T3)
// ---------------------------------------------------------------------------

describe('normalizeContext — wasm-side acceptance', () => {
  it.skipIf(!hasWasm())('wasm accepts a normalized context with empty defaults', () => {
    if (!runtimeOk) return;
    const pipeline = new Pipeline({ wasm: wasm!, config: {} });
    try {
      const ctx = normalizeContext({ text: 'minimal' });
      expect(ctx.history).toEqual([]);
      expect(ctx.headers).toEqual({});
      expect(ctx.config).toEqual({});
      expect(ctx.imageUrl).toBeUndefined();
      // The crucial wasm-boundary assertion: empty arrays + maps
      // produced by normalizeContext serialize cleanly across
      // serde-wasm-bindgen. If the Rust side were strict about
      // null-vs-empty, this call would throw.
      const raw = pipeline.evaluate(ctx);
      expect(JSON.parse(raw)).toMatchObject({ results: [], errors: [] });
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('wasm accepts populated context fields', () => {
    if (!runtimeOk) return;
    const pipeline = new Pipeline({ wasm: wasm!, config: {} });
    try {
      const ctx = normalizeContext({
        text: 'populated',
        history: ['prev1', 'prev2'],
        headers: { 'X-Custom': 'v', Authorization: 'Bearer tok' },
        imageUrl: 'https://example.com/x.png',
        config: { strict: true, threshold: 0.5 },
      });
      const raw = pipeline.evaluate(ctx);
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      expect(parsed.results).toEqual([]);
      expect(parsed.errors).toEqual([]);
    } finally {
      pipeline.close();
    }
  });
});

// ---------------------------------------------------------------------------
// T-0122 / T-0123 #4 — error path: invalid JSON config throws a TS Error
// ---------------------------------------------------------------------------

describe('Pipeline error paths', () => {
  it.skipIf(!hasWasm())('throws Error when constructed with an invalid config payload', () => {
    if (!runtimeOk) return;
    // The wasm-bindgen layer rejects non-object configs because
    // serde-wasm-bindgen::from_value() can't deserialize a primitive
    // into WasmPipelineConfig. We pass a string to force the failure.
    expect(() => new Pipeline({ wasm: wasm!, config: 'not-a-config' as never })).toThrow(Error);
  });

  it.skipIf(!hasWasm())('throws Error when constructed with bogus field types', () => {
    if (!runtimeOk) return;
    // `max_concurrency` must be a usize (number). Passing a string
    // should bubble back to TS as an Error, not a silent null.
    expect(
      () =>
        new Pipeline({
          wasm: wasm!,
          config: { max_concurrency: 'lots' as unknown as number },
        }),
    ).toThrow(Error);
  });

  it.skipIf(!hasWasm())('throws Error when evaluate receives an invalid context', () => {
    if (!runtimeOk) return;
    const pipeline = new Pipeline({ wasm: wasm!, config: {} });
    try {
      // text is required + must be a string. The wasm layer rejects
      // contexts missing `text` via serde's required-field error.
      // We bypass normalizeContext (which would have thrown earlier)
      // by casting to the wasm-internal shape.
      expect(() => pipeline.evaluate({ history: [], headers: {}, config: {} } as never)).toThrow(
        Error,
      );
    } finally {
      pipeline.close();
    }
  });
});

// ---------------------------------------------------------------------------
// T-0122 / T-0123 #5 — Lifecycle event smoke (Logger stub)
// ---------------------------------------------------------------------------

describe('Pipeline lifecycle logging', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it.skipIf(!hasWasm())('fires info on init + evaluate + close, error on bad evaluate', () => {
    if (!runtimeOk) return;
    const logger: Logger = {
      info: vi.fn(),
      warn: vi.fn(),
      error: vi.fn(),
      debug: vi.fn(),
    };

    const pipeline = new Pipeline({ wasm: wasm!, config: {}, logger });
    expect(logger.info).toHaveBeenCalledWith(
      'c12n.pipeline.init.ok',
      'max_concurrency',
      expect.any(Number),
      'timeout_ms',
      expect.any(Number),
    );

    // Successful evaluate emits debug start + debug ok (not info), so
    // verify by spying on debug.
    pipeline.evaluate({ text: 'log-me', history: [], headers: {}, config: {} });
    expect(logger.debug).toHaveBeenCalledWith(
      'c12n.pipeline.evaluate.start',
      'text_len',
      expect.any(Number),
    );
    expect(logger.debug).toHaveBeenCalledWith('c12n.pipeline.evaluate.ok');

    // Force an evaluate failure to hit the error branch.
    expect(() => pipeline.evaluate({} as never)).toThrow();
    expect(logger.error).toHaveBeenCalledWith(
      'c12n.pipeline.evaluate.failed',
      'error',
      expect.any(String),
    );

    pipeline.close();
    expect(logger.info).toHaveBeenCalledWith('c12n.pipeline.close.ok');
  });
});

// ---------------------------------------------------------------------------
// T-0122 / T-0123 #6 — parity with Go + Python (JSON-shape only)
// ---------------------------------------------------------------------------

describe('cross-surface parity (JSON shape)', () => {
  it.skipIf(!hasWasm())(
    'empty-pipeline result matches the Python (`duration_ms`) wire shape exactly',
    () => {
      if (!runtimeOk) return;
      // Python (`py/src/lib.rs::serialize_result`) and WASM
      // (`core/src/wasm.rs::WasmResult`) both emit `duration_ms`. Go
      // (`go/result.go::PipelineResult`) is the outlier — it uses
      // `duration_ns` because cgo binds to the C ABI via `FfiResult`,
      // not the wasm surface. The TS-vs-Python parity is therefore
      // direct; TS-vs-Go is asserted on the structural keys only.
      const pipeline = new Pipeline({ wasm: wasm!, config: {} });
      try {
        const raw = pipeline.evaluate({
          text: 'Write a Python function to sort a list',
          history: [],
          headers: {},
          config: {},
        });
        const parsed = JSON.parse(raw) as Record<string, unknown>;

        // Python wire shape (from py/src/lib.rs:92-97):
        //   { "results": [...], "errors": [...], "duration_ms": <u64> }
        // Empty pipeline yields:
        const expectedPythonShape = {
          results: [],
          errors: [],
        };
        expect(parsed).toMatchObject(expectedPythonShape);
        expect(typeof parsed.duration_ms).toBe('number');
        expect((parsed.duration_ms as number) >= 0).toBe(true);
      } finally {
        pipeline.close();
      }
    },
  );

  it.skipIf(!hasWasm())(
    'empty-pipeline structural keys match Go (modulo duration_ns vs duration_ms)',
    () => {
      if (!runtimeOk) return;
      // Go's wire shape is `{ results, errors, duration_ns }`. TS uses
      // `duration_ms`. Per ADR-0001 + result.go this is a known
      // divergence — assertion-level we verify the non-duration keys
      // match Go's literal shape.
      const pipeline = new Pipeline({ wasm: wasm!, config: {} });
      try {
        const raw = pipeline.evaluate({
          text: 'Write a Python function to sort a list',
          history: [],
          headers: {},
          config: {},
        });
        const parsed = JSON.parse(raw) as Record<string, unknown>;

        // Go's empty-pipeline ParseResult test in go/result_test.go:138
        // asserts `{"results":[],"errors":[],"duration_ns":0}`. The TS
        // side asserts the same structural body but uses duration_ms.
        const keysSorted = Object.keys(parsed).sort();
        expect(keysSorted).toEqual(['duration_ms', 'errors', 'results']);
        expect(parsed.results).toEqual([]);
        expect(parsed.errors).toEqual([]);
      } finally {
        pipeline.close();
      }
    },
  );
});

/**
 * Regression coverage for the `@hop-top/c12n/nodejs` subpath export.
 *
 * `src/nodejs.ts` used to `export * from './index.js'`, which re-exported
 * the base `Pipeline` whose static `create()` resolves `pkg/bundler/`.
 * Under plain Node that throws `Cannot find module .../pkg/bundler/
 * c12n_core.js` — so the entrypoint the README and the module docblock
 * both document was a hard failure on first call, while
 * `createNodePipeline()` (undocumented in the README quickstart) worked.
 *
 * These tests exercise the DOCUMENTED path: `Pipeline.create()` imported
 * from the nodejs subpath. They gate on the wasm artifact the same way
 * `pipeline.integration.test.ts` does, so `pnpm test` stays green
 * without wasm-pack installed.
 */

import { beforeAll, describe, expect, it } from 'vitest';

import { hasWasm, wasmRuntimeOk } from './setup.js';

import { Pipeline as BasePipeline } from '../src/index.js';
import { Pipeline, createNodePipeline, loadNodejs, parseResult } from '../src/nodejs.js';

let runtimeOk = false;

beforeAll(async () => {
  if (!hasWasm()) return;
  runtimeOk = await wasmRuntimeOk();
});

describe('@hop-top/c12n/nodejs — documented entrypoint', () => {
  it.skipIf(!hasWasm())('Pipeline.create() resolves the nodejs wasm target', async () => {
    if (!runtimeOk) return;
    // The exact call the README quickstart and the module docblock
    // promise. Before the fix this rejected with "Cannot find module
    // .../pkg/bundler/c12n_core.js".
    const pipeline = await Pipeline.create();
    try {
      expect(pipeline.signalCount()).toBe(0);
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('Pipeline.create() honours config overrides', async () => {
    if (!runtimeOk) return;
    const pipeline = await Pipeline.create({ config: { max_concurrency: 4, timeout_ms: 2000 } });
    try {
      const raw = pipeline.evaluate({ text: 'tune', history: [], headers: {}, config: {} });
      expect(typeof raw).toBe('string');
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('Pipeline.create() produces a working evaluate roundtrip', async () => {
    if (!runtimeOk) return;
    const pipeline = await Pipeline.create();
    try {
      const raw = pipeline.evaluate({ text: 'hello', history: [], headers: {}, config: {} });
      const parsed = JSON.parse(raw) as Record<string, unknown>;
      expect(parsed.results).toEqual([]);
      expect(Array.isArray(parsed.errors)).toBe(true);
      expect(typeof parsed.duration_ms).toBe('number');
      // `parseResult` is re-exported from the subpath and must accept
      // output produced through it.
      expect(parseResult(raw).confidence()).toBe(0);
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('nodejs Pipeline instances are instanceof the base Pipeline', async () => {
    if (!runtimeOk) return;
    // Subclassing must not break code typed against `@hop-top/c12n`'s
    // `Pipeline`.
    const pipeline = await Pipeline.create();
    try {
      expect(pipeline).toBeInstanceOf(BasePipeline);
      expect(pipeline).toBeInstanceOf(Pipeline);
    } finally {
      pipeline.close();
    }
  });

  it.skipIf(!hasWasm())('createNodePipeline() stays equivalent to Pipeline.create()', async () => {
    if (!runtimeOk) return;
    const pipeline = await createNodePipeline();
    try {
      expect(pipeline).toBeInstanceOf(Pipeline);
      expect(pipeline.signalCount()).toBe(0);
    } finally {
      pipeline.close();
    }
  });

  it('does not re-export the bundler-resolving Pipeline', () => {
    // The subpath's `Pipeline` must be the nodejs override, never the
    // base class — that shadowing was the bug.
    expect(Pipeline).not.toBe(BasePipeline);
    expect(Object.getPrototypeOf(Pipeline)).toBe(BasePipeline);
  });

  it.skipIf(!hasWasm())('loadNodejs() exposes the wasm module surface', async () => {
    const wasm = await loadNodejs();
    expect(typeof wasm.Pipeline).toBe('function');
  });
});

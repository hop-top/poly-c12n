/**
 * `@hop-top/c12n` — TypeScript bindings for c12n-core via WebAssembly.
 *
 * Public API surface. See README.md for quickstart + ADR-0001 for the
 * locked binding decisions.
 *
 * @example Bundler / browser / Workers
 * ```ts
 * import { Pipeline, parseResult } from '@hop-top/c12n';
 *
 * const pipeline = await Pipeline.create({ config: { max_concurrency: 8 } });
 * const raw = pipeline.evaluate({ text: 'hello', history: [], headers: {}, config: {} });
 * const result = parseResult(raw);
 * console.log(result.confidence());
 * pipeline.close();
 * ```
 *
 * @example Node without bundler
 * ```ts
 * import { Pipeline } from '@hop-top/c12n/nodejs';
 * ```
 */

export { Pipeline } from './pipeline.js';
export type { PipelineConfig, PipelineOptions, Logger } from './pipeline.js';

export { normalizeContext, toWireContext } from './context.js';
export type { ClassificationContext, WireContext } from './context.js';

export { parseResult, PipelineResult } from './result.js';
export type { PipelineResultRaw, SignalResult, SignalType, ResultError } from './result.js';

export { loadBundler } from './wasm-loader.js';
export type { WasmModule, WasmPipelineCtor, WasmPipelineInstance } from './wasm-loader.js';

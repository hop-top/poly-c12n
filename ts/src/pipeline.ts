/**
 * Pipeline — TS wrapper around the wasm-bindgen-generated Pipeline class.
 *
 * Mirrors the Go `c12n_cgo.go:Pipeline` API surface (`NewPipeline` →
 * `new Pipeline(...)`, `Evaluate` → `evaluate(...)`, `Close` →
 * `close()`). Errors from the wasm layer arrive as `JsValue` strings via
 * wasm-bindgen — we surface them as native `Error` instances.
 *
 * Per ADR-0001 the wasm executor is single-threaded (`new_current_thread`
 * in `c12n-core/src/wasm.rs`). One pipeline instance evaluates
 * sequentially; concurrent callers must serialise themselves (same
 * constraint as the Go `Pipeline.mu` lock).
 *
 * Optional kit-ts logging: pass a `Logger` in `PipelineOptions.logger` to
 * receive structured lifecycle events. The `Logger` interface matches
 * `@hop-top/kit`'s `log.ts` shape — `createLogger()` from kit-ts can be
 * passed straight in once kit-ts exposes the `./log` subpath (currently
 * only the source file exists; the exports table in kit-ts package.json
 * doesn't expose it yet — see kit-ts package.json `exports`). Callers can
 * also supply any duck-typed object matching the interface.
 */

import { toWireContext, type ClassificationContext } from './context.js';
import type { WasmModule, WasmPipelineInstance } from './wasm-loader.js';

/**
 * Configuration for `Pipeline.create` / `new Pipeline`. Field names match
 * the snake_case the wasm layer expects (see
 * `c12n-core/src/wasm.rs::WasmPipelineConfig`).
 */
export interface PipelineConfig {
  /** Max concurrent signal evaluations. Default: 8. */
  max_concurrency?: number;
  /** Timeout per signal in milliseconds. Default: 5000. */
  timeout_ms?: number;
}

/**
 * Minimal structural Logger interface compatible with `@hop-top/kit`'s
 * `Logger` (see `kit/sdk/ts/src/log.ts`). Pipeline accepts any object
 * matching this shape; kit-ts callers pass their `createLogger()` output
 * directly once the `./log` subpath ships in kit-ts.
 */
export interface Logger {
  info(msg: string, ...keyvals: unknown[]): void;
  warn(msg: string, ...keyvals: unknown[]): void;
  error(msg: string, ...keyvals: unknown[]): void;
  debug(msg: string, ...keyvals: unknown[]): void;
}

/** No-op logger used when no logger is supplied. */
const noopLogger: Logger = {
  info: () => {},
  warn: () => {},
  error: () => {},
  debug: () => {},
};

export interface PipelineOptions {
  /** Pre-loaded wasm module. Use `Pipeline.create` to load lazily. */
  wasm: WasmModule;
  /** Pipeline tuning. Falls back to wasm-side defaults if omitted. */
  config?: PipelineConfig;
  /** Optional structured logger. Defaults to no-op. */
  logger?: Logger;
}

/**
 * Classification pipeline.
 *
 * Construct via `Pipeline.create()` for the common async/lazy-load path,
 * or directly via `new Pipeline({ wasm })` if you already hold the loaded
 * wasm module (test harnesses, custom loaders).
 */
export class Pipeline {
  private readonly inner: WasmPipelineInstance;
  private readonly logger: Logger;
  private closed = false;

  constructor(opts: PipelineOptions) {
    this.logger = opts.logger ?? noopLogger;

    try {
      this.inner = new opts.wasm.Pipeline(opts.config ?? {});
    } catch (err) {
      this.logger.error('c12n.pipeline.init.failed', 'error', stringifyError(err));
      throw wrapWasmError('failed to create pipeline', err);
    }

    this.logger.info(
      'c12n.pipeline.init.ok',
      'max_concurrency',
      opts.config?.max_concurrency ?? 8,
      'timeout_ms',
      opts.config?.timeout_ms ?? 5000,
    );
  }

  /**
   * Lazy-load the wasm module and construct a pipeline.
   *
   * Resolves with a ready-to-use `Pipeline`. The wasm bytes are fetched
   * via the loader the bundler resolved (`pkg/bundler/` for browser /
   * Vite / wrangler / esbuild, `pkg/nodejs/` when importing from
   * `@hop-top/c12n/nodejs`).
   */
  static async create(opts: { config?: PipelineConfig; logger?: Logger } = {}): Promise<Pipeline> {
    const { loadBundler } = await import('./wasm-loader.js');
    const wasm = await loadBundler();
    if (typeof wasm.default === 'function') {
      // wasm-bindgen `--target bundler` may expose an async init the
      // host must call once before the module is usable. Some bundlers
      // (Vite/wrangler) trigger this implicitly; calling defensively is
      // a no-op when already initialised.
      await wasm.default();
    }
    if (typeof wasm.setPanicHook === 'function') {
      wasm.setPanicHook();
    }
    return new Pipeline({ wasm, config: opts.config, logger: opts.logger });
  }

  /**
   * Evaluate a classification context.
   *
   * Returns the raw JSON string emitted by `c12n-core` — pass to
   * `parseResult()` for a typed accessor object. Mirrors Go's
   * `Pipeline.Evaluate` which also returns `(string, error)`.
   */
  evaluate(ctx: ClassificationContext): string {
    if (this.closed) {
      throw new Error('c12n: pipeline is closed');
    }
    const wire = toWireContext(ctx);
    this.logger.debug('c12n.pipeline.evaluate.start', 'text_len', ctx.text.length);
    try {
      const raw = this.inner.evaluate(wire);
      this.logger.debug('c12n.pipeline.evaluate.ok');
      return raw;
    } catch (err) {
      this.logger.error('c12n.pipeline.evaluate.failed', 'error', stringifyError(err));
      throw wrapWasmError('evaluate failed', err);
    }
  }

  /** Number of signals currently registered on the pipeline. */
  signalCount(): number {
    if (this.closed) return 0;
    return this.inner.signalCount();
  }

  /**
   * Release the underlying wasm-bindgen object. Idempotent. After
   * `close()`, `evaluate()` throws.
   */
  close(): void {
    if (this.closed) return;
    this.closed = true;
    try {
      this.inner.free();
      this.logger.info('c12n.pipeline.close.ok');
    } catch (err) {
      // wasm-bindgen `free()` shouldn't throw under normal use; log and
      // swallow to keep `close()` idempotent + safe in finally blocks.
      this.logger.warn('c12n.pipeline.close.failed', 'error', stringifyError(err));
    }
  }
}

// ---------------------------------------------------------------------------
// Error adapters
// ---------------------------------------------------------------------------

/**
 * wasm-bindgen throws `JsValue` (string or object) across the boundary.
 * Translate to native `Error` so TS callers can `try/catch (e: Error)`
 * idiomatically.
 */
function wrapWasmError(prefix: string, err: unknown): Error {
  return new Error(`c12n: ${prefix}: ${stringifyError(err)}`);
}

function stringifyError(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === 'string') return err;
  try {
    return JSON.stringify(err);
  } catch {
    return String(err);
  }
}

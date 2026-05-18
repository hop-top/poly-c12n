/**
 * ClassificationContext — input shape for `Pipeline.evaluate`.
 *
 * Mirrors the Rust `c12n_core::ClassificationContext` struct + the Go
 * `ClassificationContext` (see ../../types.go). Collections are required
 * non-null; the factory `normalizeContext` fills in empty defaults so
 * callers can pass partial inputs without writing `headers: {}` etc by
 * hand. This matches the Go binding's `normalizeContext` helper in
 * `c12n.go`.
 *
 * `imageUrl` is optional (matches the Rust `Option<String>` and the Go
 * `*string` field with `omitempty`). The wasm-bindgen layer treats
 * `undefined` correctly via `#[serde(default)]` on the Rust side.
 */

/**
 * Classification input passed to `Pipeline.evaluate`.
 *
 * All fields except `imageUrl` are required AFTER normalization. Use
 * `normalizeContext({ text: '...' })` if you only have partial input.
 */
export interface ClassificationContext {
  /** Primary text to classify. Required. */
  text: string;
  /** Conversation history (chronological). Empty array if none. */
  history: string[];
  /** HTTP / framing headers attached to the input. Empty map if none. */
  headers: Record<string, string>;
  /** Optional image URL when a signal needs vision input. */
  imageUrl?: string;
  /** Per-call signal config overrides. Empty map if none. */
  config: Record<string, unknown>;
}

/**
 * Fills nil collections with empty defaults.
 *
 * Mirrors Go's `normalizeContext` in `c12n.go`. The Rust side tolerates
 * `null` via `#[serde(default)]` but explicitly normalising avoids
 * surprises across runtimes (Node serializes `undefined` ≠ `null` ≠
 * absent in some bundler glue paths).
 *
 * NOTE: serde-wasm-bindgen on the Rust side expects snake_case
 * (`image_url`). We translate `imageUrl` → `image_url` at the wasm
 * boundary inside `Pipeline.evaluate`; callers always work with camelCase
 * on the TS side.
 */
export function normalizeContext(input: Partial<ClassificationContext>): ClassificationContext {
  if (typeof input.text !== 'string') {
    throw new TypeError('normalizeContext: `text` is required and must be a string');
  }
  return {
    text: input.text,
    history: input.history ?? [],
    headers: input.headers ?? {},
    imageUrl: input.imageUrl,
    config: input.config ?? {},
  };
}

/**
 * Wire-shape (snake_case) sent across the wasm boundary. The Rust
 * `ClassificationContext` uses `#[serde]` field names matching its
 * struct fields (snake_case), so the JS object handed to
 * `wasmPipeline.evaluate(...)` must use those names. This adapter is
 * internal — callers see the camelCase `ClassificationContext` shape.
 *
 * @internal
 */
export interface WireContext {
  text: string;
  history: string[];
  headers: Record<string, string>;
  image_url?: string;
  config: Record<string, unknown>;
}

/** Convert the public camelCase shape to the snake_case wire shape. @internal */
export function toWireContext(ctx: ClassificationContext): WireContext {
  return {
    text: ctx.text,
    history: ctx.history,
    headers: ctx.headers,
    image_url: ctx.imageUrl,
    config: ctx.config,
  };
}

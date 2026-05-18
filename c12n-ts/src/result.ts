/**
 * PipelineResult — output shape from `Pipeline.evaluate`.
 *
 * Mirrors the Rust `WasmResult` from `c12n-core/src/wasm.rs` (which itself
 * mirrors the C-ABI `FfiResult` for parity with Go/Python). The wire shape
 * is JSON; this module deserializes + provides typed accessors that
 * parallel Go's `PipelineResult` methods in `result.go`.
 */

/**
 * All classification signal types. Mirrors `c12n_core::SignalType` and
 * Go's `SignalType` constants in `types.go`. New variants added in
 * `c12n-core` should be added here too.
 */
export type SignalType =
  | 'Keyword'
  | 'Embedding'
  | 'Domain'
  | 'Jailbreak'
  | 'PII'
  | 'Toxicity'
  | 'Context'
  | 'Structure'
  | 'Language'
  | 'Complexity'
  | 'Preference'
  | 'Feedback'
  | 'OutputFormat'
  | 'CodeContent'
  | 'ToolCalling'
  | 'CostEstimate'
  | 'Sentiment'
  | 'Intent'
  | 'Topic'
  | 'Custom';

/** A single signal's classification output. */
export interface SignalResult {
  name: string;
  signal_type: SignalType;
  confidence: number;
  labels: string[];
  metadata: Record<string, unknown>;
}

/**
 * Pipeline-level error variants. Mirrors `c12n_core::PipelineError` via
 * the JSON-serialised representation that the wasm layer emits as a
 * string in the `errors` array.
 *
 * The wasm surface in `c12n-core/src/wasm.rs` stringifies errors before
 * adding them to the result (`errors: result.errors.iter().map(|e|
 * e.to_string())`), so we receive `string[]` here, not structured
 * variants. If c12n-core later switches to serializing structured
 * errors, widen this type.
 */
export type ResultError = string;

/**
 * Deserialized pipeline result. Field names match the JSON the wasm
 * layer emits exactly.
 */
export interface PipelineResultRaw {
  results: SignalResult[];
  errors: ResultError[];
  duration_ms: number;
}

/**
 * Typed accessor wrapper around the raw result.
 *
 * Mirrors Go's `*PipelineResult` methods in `result.go`. `confidence()`
 * with no argument returns the top-level (max) confidence across all
 * signal results, or `0` if the result set is empty. `confidence(type)`
 * returns the matched signal's confidence, or `0` if no match. See
 * empty-result semantics in the class docs below.
 */
export class PipelineResult {
  readonly results: SignalResult[];
  readonly errors: ResultError[];
  readonly duration_ms: number;

  constructor(raw: PipelineResultRaw) {
    this.results = raw.results;
    this.errors = raw.errors;
    this.duration_ms = raw.duration_ms;
  }

  /** Returns the first signal matching `t`, or `undefined`. */
  signal(t: SignalType): SignalResult | undefined {
    return this.results.find((r) => r.signal_type === t);
  }

  /** Returns true if a signal of type `t` exists. */
  hasSignal(t: SignalType): boolean {
    return this.signal(t) !== undefined;
  }

  /**
   * Returns the confidence score.
   *
   * - With no argument: the maximum confidence across all signals, or
   *   `0` for an empty result set. Mirrors a "did anything fire?"
   *   convenience accessor — `0` is the explicit empty-pipeline value
   *   (not `undefined`) so callers can do `if (result.confidence() <
   *   threshold)` without null-checks.
   * - With a `SignalType`: the matched signal's confidence, or `0` if
   *   no match. Matches Go's `Confidence(t)` in `result.go`.
   */
  confidence(t?: SignalType): number {
    if (t === undefined) {
      if (this.results.length === 0) return 0;
      return Math.max(...this.results.map((r) => r.confidence));
    }
    const s = this.signal(t);
    return s ? s.confidence : 0;
  }

  /** Returns all signals matching `t`. */
  signals(t: SignalType): SignalResult[] {
    return this.results.filter((r) => r.signal_type === t);
  }

  /** True if any signal errors occurred. */
  hasErrors(): boolean {
    return this.errors.length > 0;
  }
}

/**
 * Parse the raw JSON string emitted by `Pipeline.evaluate`.
 *
 * @throws SyntaxError if `raw` is not valid JSON.
 * @throws TypeError if the parsed shape is missing required fields.
 */
export function parseResult(raw: string): PipelineResult {
  const parsed = JSON.parse(raw) as Partial<PipelineResultRaw>;
  if (!Array.isArray(parsed.results)) {
    throw new TypeError('parseResult: missing or invalid `results` array');
  }
  if (!Array.isArray(parsed.errors)) {
    throw new TypeError('parseResult: missing or invalid `errors` array');
  }
  if (typeof parsed.duration_ms !== 'number') {
    throw new TypeError('parseResult: missing or invalid `duration_ms` number');
  }
  return new PipelineResult({
    results: parsed.results,
    errors: parsed.errors,
    duration_ms: parsed.duration_ms,
  });
}

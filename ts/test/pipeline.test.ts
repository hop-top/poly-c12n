/**
 * Smoke tests for the public TS API surface.
 *
 * Scope (per T-0121): import + type-check the public surface, exercise
 * the pure-TS helpers that don't require the wasm artifact. Actual
 * classification flow (load wasm → evaluate → parse) is covered by the
 * downstream T9 vitest agent's integration test which depends on
 * `wasm-pack build` having run.
 */

import { describe, expect, it } from 'vitest';

import {
  normalizeContext,
  parseResult,
  type ClassificationContext,
  type PipelineConfig,
  type SignalType,
} from '../src/index.js';

describe('normalizeContext', () => {
  it('fills empty defaults for collections when only text is supplied', () => {
    const ctx = normalizeContext({ text: 'hello' });
    expect(ctx.text).toBe('hello');
    expect(ctx.history).toEqual([]);
    expect(ctx.headers).toEqual({});
    expect(ctx.config).toEqual({});
    expect(ctx.imageUrl).toBeUndefined();
  });

  it('preserves provided collections', () => {
    const ctx: ClassificationContext = normalizeContext({
      text: 'hi',
      history: ['prev'],
      headers: { 'x-test': '1' },
      config: { strict: true },
      imageUrl: 'https://example.com/x.png',
    });
    expect(ctx.history).toEqual(['prev']);
    expect(ctx.headers).toEqual({ 'x-test': '1' });
    expect(ctx.config).toEqual({ strict: true });
    expect(ctx.imageUrl).toBe('https://example.com/x.png');
  });

  it('rejects missing or non-string text', () => {
    expect(() => normalizeContext({} as Partial<ClassificationContext>)).toThrow(TypeError);
    expect(() =>
      normalizeContext({ text: 42 as unknown as string }),
    ).toThrow(TypeError);
  });
});

describe('parseResult', () => {
  it('parses an empty pipeline result and exposes 0 confidence', () => {
    const r = parseResult('{"results":[],"errors":[],"duration_ms":0}');
    expect(r.results).toEqual([]);
    expect(r.errors).toEqual([]);
    expect(r.duration_ms).toBe(0);
    // Documented semantics: empty pipeline → confidence() === 0 (NOT undefined).
    // See src/result.ts PipelineResult.confidence() docs.
    expect(r.confidence()).toBe(0);
    expect(r.hasErrors()).toBe(false);
  });

  it('returns max confidence across signals when no type specified', () => {
    const raw = JSON.stringify({
      results: [
        { name: 'kw', signal_type: 'Keyword', confidence: 0.3, labels: [], metadata: {} },
        { name: 'pi', signal_type: 'PII', confidence: 0.7, labels: [], metadata: {} },
      ],
      errors: [],
      duration_ms: 12,
    });
    const r = parseResult(raw);
    expect(r.confidence()).toBe(0.7);
    expect(r.confidence('Keyword' as SignalType)).toBe(0.3);
    expect(r.confidence('PII' as SignalType)).toBe(0.7);
    expect(r.confidence('Toxicity' as SignalType)).toBe(0);
  });

  it('looks up signals by type', () => {
    const raw = JSON.stringify({
      results: [
        { name: 'pi', signal_type: 'PII', confidence: 0.5, labels: ['email'], metadata: {} },
      ],
      errors: ['SignalFailed: toxicity'],
      duration_ms: 4,
    });
    const r = parseResult(raw);
    expect(r.hasSignal('PII')).toBe(true);
    expect(r.hasSignal('Toxicity')).toBe(false);
    expect(r.signal('PII')?.labels).toEqual(['email']);
    expect(r.signals('PII')).toHaveLength(1);
    expect(r.hasErrors()).toBe(true);
  });

  it('rejects malformed payloads', () => {
    expect(() => parseResult('not json')).toThrow(SyntaxError);
    expect(() => parseResult('{}')).toThrow(TypeError);
    expect(() => parseResult('{"results":[]}')).toThrow(TypeError);
  });
});

describe('type surface', () => {
  it('PipelineConfig fields accept snake_case names', () => {
    const cfg: PipelineConfig = { max_concurrency: 4, timeout_ms: 2000 };
    expect(cfg.max_concurrency).toBe(4);
    expect(cfg.timeout_ms).toBe(2000);
  });
});

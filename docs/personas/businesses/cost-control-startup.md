# Cost-control startup

Small team building an LLM-cost dashboard / proxy.

## Use c12n when

- You're proxying customer LLM calls and want to surface "which
  signals fired" per request for the dashboard.
- Cost-saving recommendations need signal-level data, not just
  request totals.
- Need a CLI to classify ad-hoc prompts during customer demos.

## Constraints

- Single Go binary preferred; no Rust toolchain in production.
- Latency: classification overhead must stay <5ms.
- Open-source-friendly licence (TBD).

## What they get from c12n

- `c12n classify` CLI for ad-hoc / demo use.
- `c12n bench` for measuring classification overhead on real
  customer traffic shapes.
- `Pipeline.Evaluate` returns 20 signal scores — granular enough
  for cost-savings narratives.

## Stories

- [US-0002 Evaluate a prompt via CLI](../../stories/US-0002-classify-cli.md)
- [US-0004 Benchmark classification overhead](../../stories/US-0004-bench-overhead.md)
- [US-0007 Parse JSON from FFI without panic](../../stories/US-0007-json-ffi-roundtrip.md)

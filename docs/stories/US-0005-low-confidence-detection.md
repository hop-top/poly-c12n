---
status: planned
bindings:
  rust: partial
  go: planned
  python: planned
  typescript: planned
  php: planned
personas: [llm-routing-saas, middleware-developer]
priority: P1
---

# US-0005: Detect low-confidence classifications

As a tool author, I want a per-signal confidence score on the
classification result so I can escalate ambiguous prompts to a more
capable model.

> **Status: planned — do not route production traffic on this yet.**
>
> There is no aggregate pipeline confidence. Every binding exposes
> *per-signal* confidence only. Worse, four of the five bindings
> construct a pipeline with **zero signals**, so there are no
> per-signal scores to read either — see [Binding
> reality](#binding-reality). Routing thresholds written against this
> story before that is fixed escalate 100% of traffic.

## Use this when

- Routing logic falls back to a powerful model when the cheap
  classifier isn't sure.
- Logging / alerting on classification quality.

Today only a Rust caller who constructs `Pipeline::new(signals, ..)`
with a non-empty signal list can do any of this.

## Result

Go: `PipelineResult.Confidence(SignalType) float64` returns the
confidence of the **named signal**, or `0` when that signal is absent
from the result. It takes an argument — there is no zero-argument
`Confidence()` accessor.

```go
// go/result.go:47
func (r *PipelineResult) Confidence(t SignalType) float64
```

`0` is therefore ambiguous: it means "signal genuinely scored 0" and
"signal did not run" alike. Callers must disambiguate with
`HasSignal(t)` before trusting the number.

## Steps

Per-signal, with an explicit presence check:

```go
result, err := c12n.ParseResult(raw)
if err != nil {
    return err
}

if !result.HasSignal(c12n.SignalJailbreak) {
    // Signal did not run. Not the same as "scored 0".
    return errSignalUnavailable
}

if result.Confidence(c12n.SignalJailbreak) >= 0.8 {
    // escalate
}
```

There is no supported way to collapse several signals into one number
inside c12n. If your routing layer wants an aggregate, compute it
yourself from `result.Results` and own the weighting decision.

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestE2E_PipelineResult_Confidence_Range ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_PipelineResult_Confidence_Accessor ./...
```

Both pass, but note what they cover: they parse a **hand-written JSON
fixture** (`validResultJSON`), not engine output. They prove the
accessor reads the field it is given. They prove nothing about whether
a real pipeline ever produces that field.

## Binding reality

`Pipeline::evaluate` only scores signals registered at construction.
Four of five bindings register none:

| Binding | Constructs with | Can register signals? |
|---------|-----------------|-----------------------|
| Rust (`rs/`) | `Pipeline::new(signals, ..)` — caller-supplied ([`rs/src/sdk_pipeline.rs:26`](../../rs/src/sdk_pipeline.rs)) | yes |
| Go (cgo) | `Pipeline::new(vec![], ..)` ([`core/src/ffi.rs:134`](../../core/src/ffi.rs)) | no |
| Python | `Pipeline::new(vec![], ..)` ([`py/src/lib.rs:25`](../../py/src/lib.rs)) | no |
| TypeScript (wasm) | `InnerPipeline::new(vec![], ..)` ([`core/src/wasm.rs:104`](../../core/src/wasm.rs)) | no |
| PHP (FFI) | via `ffi.rs` — same `vec![]` | no |

Since PR #42 this at least fails loudly. An unconfigured pipeline
returns `PipelineError::NoSignals`
([`core/src/pipeline.rs:24-33`](../../core/src/pipeline.rs)), so
`errors` carries `"pipeline has no registered signals; results will be
empty"` instead of a silent empty envelope.

Detectors and tiered chains landed in the same PR
([`core/src/registry.rs`](../../core/src/registry.rs),
[`core/src/chain.rs`](../../core/src/chain.rs)), but the
config-schema plumbing that would let a binding *name* them from YAML
does not exist yet. Chains remain Rust-constructible only.

## What this story needs before it ships

1. Config-schema plumbing so a binding can express which detectors to
   register (blocks Go / Python / TypeScript / PHP).
2. A decision on whether an aggregate confidence is part of the
   contract at all — and if so, a specified aggregation rule.
3. Tests that assert confidence values produced by a **real** pipeline,
   not by a fixture.

## Tests

- [`go/e2e_test.go:TestE2E_PipelineResult_Confidence_Range`](../../go/e2e_test.go)
  — fixture-only; asserts parsed values land in `[0,1]`.
- [`go/e2e_test.go:TestE2E_PipelineResult_Confidence_Accessor`](../../go/e2e_test.go)
  — fixture-only; asserts the missing-signal case returns `0`.
- [`go/result_test.go:TestConfidence`](../../go/result_test.go) — unit-level.

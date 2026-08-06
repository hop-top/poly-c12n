---
status: partial
bindings:
  go: partial
personas: [llm-routing-saas, framework-author, middleware-developer]
priority: P0
---

# US-0003: Parse PipelineResult into typed scores

As a tool author, I want typed accessors on `PipelineResult` so my
routing logic doesn't `map[string]any`-walk.

> **Status: partial.** The `errors` wire-shape break is fixed — native
> engine output parses, and a native test asserts on the decoded
> diagnostic. What is still unproven is the part this story is named
> for: no engine output has ever contained a `results` entry, so the
> per-signal accessors are exercised only against fixtures. See
> [The score accessors are still fixture-only](#the-score-accessors-are-still-fixture-only).

## Use this when

- Routing layer reads per-signal scores to make the model decision.
- Logging / metrics ingestion needs typed extraction.
- Tests want to assert specific signal scores.

## Result

On `*PipelineResult` ([`go/result.go`](../../go/result.go)):

| Accessor | Returns |
|----------|---------|
| `Signal(SignalType) *SignalResult` | first matching result, or `nil` |
| `Signals(SignalType) []SignalResult` | all matching results |
| `HasSignal(SignalType) bool` | presence check |
| `Confidence(SignalType) float64` | that signal's confidence, or `0` |
| `HasErrors() bool` | `len(r.Errors) > 0` |
| `Duration() time.Duration` | from `DurationMs` |

`Errors` is an exported **field** (`[]PipelineError`), not a method —
there is no `Errors()` accessor. `Confidence` requires a `SignalType`
argument; there is no aggregate confidence
([US-0005](US-0005-low-confidence-detection.md)).

Score lives on `SignalResult.Confidence`
([`go/types.go`](../../go/types.go)). There is no `Score` field.

## The errors wire shape is fixed

`PipelineError` is now a string type matching what the FFI actually
emits ([`go/types.go`](../../go/types.go)):

```go
// PipelineError is a diagnostic emitted by the pipeline, rendered by the
// core through its Display impl.
type PipelineError string

func (e PipelineError) Error() string { return string(e) }
```

That lines up with `FfiResult.errors: Vec<String>`
([`core/src/ffi.rs`](../../core/src/ffi.rs)) and with the PHP and
TypeScript bindings, which always treated `errors` as strings. The
previous struct shape — with `SignalFailed` / `Timeout` variants —
matched no payload the FFI has ever produced.

Native evaluation now parses:

```console
$ cd go && CGO_LDFLAGS="-L$(cd .. && pwd)/target/release" \
    DYLD_LIBRARY_PATH="$(cd .. && pwd)/target/release" \
    go test -tags "c12n_native integration" -count=1 ./...
ok  	hop.top/c12n	0.387s
ok  	hop.top/c12n/cmd/c12n	0.365s
```

`TestIntegration_PipelineEmptyResult` asserts on the decoded contents
of real engine output — not merely that `ParseResult` returned without
error — so a regression in the wire shape fails the suite.

## The score accessors are still fixture-only

`Signal`, `Signals`, `HasSignal` and `Confidence` all read the
`Results` slice. Every native evaluation returns `results: []`, because
no binding can register a detector
([US-0002](US-0002-classify-cli.md)). So these four accessors have
never run against engine-produced data — only against
`validResultJSON`, a hand-written fixture in
[`go/e2e_test.go`](../../go/e2e_test.go).

The fixture and the engine agree on field names, and the `errors`
half of the envelope is now natively covered. But "parse
`PipelineResult` into typed scores" is not proven end to end until the
engine emits a score.

## Steps

```go
raw, err := pipeline.Evaluate(ctx)
if err != nil {
    return err
}

result, err := c12n.ParseResult(raw)
if err != nil {
    return err
}

if result.HasErrors() {
    // Includes the NoSignals diagnostic on an unconfigured pipeline —
    // which is every pipeline today.
    log.Warn("classification errors", "errors", result.Errors)
}

if score := result.Signal(c12n.SignalCodeContent); score != nil {
    if score.Confidence > 0.8 {
        // route to code-specialized model
    }
}
```

The `Signal` branch is unreachable in practice right now: `Results` is
always empty.

## Verify

Fixture-based, stub mode:

```bash
cd go && CGO_ENABLED=0 go test -run TestE2E_ParseResult_Accessors ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_PipelineResult_Signal ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_PipelineResult_HasSignal ./...
cd go && CGO_ENABLED=0 go test -run TestPipelineError_WireFormat ./...
cd go && CGO_ENABLED=0 go test -run TestPipelineError_NoSignalsEnvelope ./...
```

Against the real engine:

```bash
cargo build -p hop-top-c12n-core --release
cd go && CGO_ENABLED=1 \
  CGO_LDFLAGS="-L$(cd .. && pwd)/target/release" \
  DYLD_LIBRARY_PATH="$(cd .. && pwd)/target/release" \
  go test -tags "c12n_native integration" -count=1 ./...
```

All pass. `TestPipelineError_WireFormat` pins the three exact strings
the core's `Display` impl emits; `TestIntegration_PipelineEmptyResult`
is the one that crosses the C ABI.

## How it works

`ParseResult` is a single eager `json.Unmarshal` into `PipelineResult`
([`go/result.go`](../../go/result.go)) — not lazy, despite what
earlier revisions of this story claimed. Accessors are plain slice
walks over the already-decoded `Results`. Malformed JSON returns the
`encoding/json` error; there is no c12n-specific error type, and
nothing panics.

## What this story needs to reach `shipped`

A native test that parses an engine result containing at least one
`results` entry and asserts `Signal` / `Confidence` read it back. That
needs detector selection plumbed through the FFI first
([US-0002](US-0002-classify-cli.md)).

## Tests

- [`go/integration_test.go:TestIntegration_PipelineEmptyResult`](../../go/integration_test.go)
  — native; asserts on the decoded `errors` array. Passes.
- [`go/integration_test.go:TestIntegration_JSONRoundTripThroughFFI`](../../go/integration_test.go)
  — native. Passes.
- [`go/c12n_test.go:TestPipelineError_WireFormat`](../../go/c12n_test.go)
  — pins the core's three `Display` strings.
- [`go/c12n_test.go:TestPipelineError_NoSignalsEnvelope`](../../go/c12n_test.go)
  — fixture of the envelope every unconfigured evaluation produces.
- [`go/e2e_test.go:TestE2E_ParseResult_Accessors`](../../go/e2e_test.go)
  — fixture; covers `HasErrors`, `Signals`, `Duration`.
- [`go/e2e_test.go:TestE2E_PipelineResult_Signal`](../../go/e2e_test.go) — fixture.
- [`go/e2e_test.go:TestE2E_PipelineResult_HasSignal`](../../go/e2e_test.go) — fixture.
- [`go/e2e_test.go:TestE2E_ParseResult_InvalidJSON_Error`](../../go/e2e_test.go)
  — empty / garbage / truncated / wrong-type inputs.
- [`go/result_test.go`](../../go/result_test.go) — unit-level.

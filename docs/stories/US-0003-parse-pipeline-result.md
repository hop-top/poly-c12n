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

> **Status: partial.** The accessors exist and behave as described *for
> JSON you hand them*. Parsing real native-engine output currently
> fails — the `errors` field shape does not match. See [Native output
> does not parse](#native-output-does-not-parse).

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
([`go/types.go:31`](../../go/types.go)). There is no `Score` field.

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
    // Includes the NoSignals diagnostic on an unconfigured pipeline.
    log.Warn("classification errors", "errors", result.Errors)
}

if score := result.Signal(c12n.SignalCodeContent); score != nil {
    if score.Confidence > 0.8 {
        // route to code-specialized model
    }
}
```

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestE2E_ParseResult_Accessors ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_PipelineResult_Signal ./...
cd go && CGO_ENABLED=0 go test -run TestE2E_PipelineResult_HasSignal ./...
```

All pass. All operate on `validResultJSON`, a hand-written fixture in
[`go/e2e_test.go`](../../go/e2e_test.go) — not on engine output.

## Native output does not parse

Go models `errors` as a slice of structs:

```go
// go/result.go:10
Errors []PipelineError `json:"errors"`
```

The FFI serializes it as a slice of **strings**:

```rust
// core/src/ffi.rs:198
errors: result.errors.iter().map(|e| e.to_string()).collect(),
```

`FfiResult.errors` is `Vec<String>`
([`core/src/ffi.rs:57`](../../core/src/ffi.rs)). Before PR #42 the
pipeline always returned an empty `errors` array, so `[]` unmarshalled
into `[]PipelineError` without complaint and the mismatch stayed
invisible. Now that an unconfigured pipeline emits `NoSignals`, every
native evaluation returns one string and `ParseResult` fails outright:

```
$ cargo build -p hop-top-c12n-core
$ cd go && CGO_ENABLED=1 \
    CGO_LDFLAGS="-L$(cd .. && pwd)/target/debug" \
    DYLD_LIBRARY_PATH="$(cd .. && pwd)/target/debug" \
    go test -tags "c12n_native integration" -run TestIntegration ./...
--- FAIL: TestIntegration_PipelineEmptyResult
    integration_test.go:47: ParseResult: json: cannot unmarshal string
    into Go struct field PipelineResult.errors of type c12n.PipelineError
--- FAIL: TestIntegration_JSONRoundTripThroughFFI
    integration_test.go:108: ParseResult: json: cannot unmarshal string
    into Go struct field PipelineResult.errors of type c12n.PipelineError
FAIL
```

The `PipelineError` struct's `SignalFailed` / `Timeout` variants
([`go/types.go:39-47`](../../go/types.go)) match no shape the FFI has
ever emitted. Go and PHP/TypeScript disagree here: the latter two treat
`errors` as strings and were updated for `NoSignals`
([`ts/test/pipeline.integration.test.ts:92`](../../ts/test/pipeline.integration.test.ts),
[`php/tests/PipelineFfiIntegrationTest.php:119`](../../php/tests/PipelineFfiIntegrationTest.php)).
Go was not.

## How it works

`ParseResult` is a single eager `json.Unmarshal` into `PipelineResult`
([`go/result.go:17`](../../go/result.go)) — not lazy, despite what
earlier revisions of this story claimed. Accessors are plain slice
walks over the already-decoded `Results`. Malformed JSON returns the
`encoding/json` error; there is no c12n-specific error type, and
nothing panics.

In stub mode, construction + parsing work on hand-built JSON.

## What this story needs to reach `shipped`

Reconcile the `errors` wire shape. Either Go decodes `[]string` (matching
PHP/TS and the FFI as built), or the FFI emits structured errors and
every binding is updated together. Pick one and cover it with a native
test that asserts on a non-empty `errors` array.

## Tests

- [`go/e2e_test.go:TestE2E_ParseResult_Accessors`](../../go/e2e_test.go)
  — fixture; covers `HasErrors`, `Signals`, `Duration`.
- [`go/e2e_test.go:TestE2E_PipelineResult_Signal`](../../go/e2e_test.go) — fixture.
- [`go/e2e_test.go:TestE2E_PipelineResult_HasSignal`](../../go/e2e_test.go) — fixture.
- [`go/e2e_test.go:TestE2E_ParseResult_HasErrors`](../../go/e2e_test.go)
  — fixture uses a `SignalFailed` shape the FFI does not emit.
- [`go/e2e_test.go:TestE2E_ParseResult_InvalidJSON_Error`](../../go/e2e_test.go)
  — empty / garbage / truncated / wrong-type inputs.
- [`go/result_test.go`](../../go/result_test.go) — unit-level.

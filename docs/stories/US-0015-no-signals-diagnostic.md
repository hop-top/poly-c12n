---
status: shipped
personas: [middleware-developer, internal-tool-builder, framework-author]
priority: P0
---

# US-0015: Learn that a pipeline has no signals registered

As a tool author, I want an unconfigured pipeline to say so loudly,
so I find the wiring bug during integration instead of shipping a
classifier that silently classifies nothing.

## Use this when

- First integration of any binding — the empty envelope used to look
  like a successful "nothing matched".
- A health check that must distinguish "no signals fired" from "no
  signals exist".
- Debugging a pipeline that returns plausible-looking empty results.

## Result

`Pipeline::evaluate` on a pipeline with zero registered signals
returns immediately with one error instead of an empty envelope:

```text
pipeline has no registered signals; results will be empty
```

In Rust it is `PipelineError::NoSignals`. Every binding surfaces the
`errors` array, so the diagnostic reaches all published artifacts
without a per-language change:

```json
{ "results": [], "errors": ["pipeline has no registered signals; results will be empty"], "duration_ms": 0 }
```

Previously this same situation produced
`{"results": [], "errors": []}` — indistinguishable from a healthy
pipeline that found nothing. That silence is the bug this replaces.

## This is the current state for every non-Rust binding

`c12n_pipeline_new` (`core/src/ffi.rs`), `core/src/wasm.rs` and
`py/src/lib.rs` all construct their pipeline with `vec![]` signals.
The config plumbing that would let a Go, PHP, Python or TypeScript
caller select detectors has not landed, so **every** non-Rust
pipeline hits this error today. That is intended: a loud, accurate
"nothing is configured" beats a silent empty result.

Rust callers register signals directly and do not see it — see
[US-0013](US-0013-tiered-detector-chains.md) and
[US-0014](US-0014-detector-registry-by-name.md).

## Steps

Rust:

```rust
use std::time::Duration;
use c12n_core::pipeline::{Pipeline, PipelineError};

let pipeline = Pipeline::new(vec![], 8, Duration::from_millis(5000));
let result = pipeline.evaluate(&ctx).await;

assert!(result.results.is_empty());
assert!(matches!(&result.errors[0], PipelineError::NoSignals));
```

Actual output:

```text
results=0 errors=1
error: pipeline has no registered signals; results will be empty
```

Register a signal and the error disappears:

```rust
let chain = registry::pii_chain(&["regex"], "escalate:0.8")?;
let signal = PiiSignal::with_chain(
    chain,
    HashSet::from(["EMAIL".to_string(), "CREDIT_CARD".to_string()]),
    4096,
);
let pipeline = Pipeline::new(vec![Box::new(signal)], 8, Duration::from_millis(5000));
// → errors=0, name=pii confidence=0.95 labels=["EMAIL", "CREDIT_CARD"]
```

Other bindings check the same string, since the variant does not
cross the FFI boundary:

```typescript
expect(parsed.errors).toHaveLength(1);
expect(String(parsed.errors[0])).toMatch(/no registered signals/);
```

```php
self::assertCount(1, $result->errors());
self::assertStringContainsString('no registered signals', $result->errors()[0]);
```

Note that `hasErrors()` is now `true` for a default-config pipeline in
every binding. Health checks that treated "no errors" as "healthy"
need updating.

## Verify

```bash
cargo test -p hop-top-c12n-core --lib pipeline::
```

## How it works

`evaluate` checks `self.signals.is_empty()` before spawning anything
and short-circuits with the single error, so an unconfigured pipeline
costs nothing beyond the check. `duration` is still populated.

An unconfigured pipeline is almost always a wiring bug in the caller
rather than a legitimate "nothing matched" outcome, so it is reported
as an error rather than an empty success. `signal_count()` remains
available for callers that want to check before evaluating.

## Tests

- [`core/src/pipeline.rs`](../../core/src/pipeline.rs) — asserts
  `PipelineError::NoSignals` for an empty pipeline
- [`ts/test/pipeline.integration.test.ts`](../../ts/test/pipeline.integration.test.ts) —
  wasm surface reports exactly one `NoSignals` error;
  `parseResult` sets `hasErrors()`
- [`php/tests/PipelineFfiIntegrationTest.php`](../../php/tests/PipelineFfiIntegrationTest.php) —
  `testPipelineResultParsesRoundtripJson`

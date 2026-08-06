---
status: partial
personas: [middleware-developer, internal-tool-builder, llm-routing-saas]
priority: P0
---

# US-0022: Handle errors and the NoSignals diagnostic in PHP

As a tool author on PHP, I want to know which failures throw, which
arrive as diagnostics inside a successful result, and why my pipeline
returns zero signals today.

## Use this when

- A PHP pipeline evaluates successfully but `results()` is empty.
- Deciding whether to `catch` or to inspect `errors()`.
- Assessing whether c12n on PHP can meet a routing requirement yet.

## Result

Two distinct failure channels:

- **Thrown** — `PipelineException` (extends `C12nException` extends
  `RuntimeException`) for lifecycle and transport failures: closed
  pipeline, null FFI pointer, unparseable JSON, FFI `{"error": ...}`
  envelopes.
- **Returned** — `PipelineResult::errors()` carries per-evaluation
  diagnostics on an otherwise successful call.

## The NoSignals diagnostic

An unconfigured pipeline now reports itself loudly instead of
returning a silently-empty envelope:

```
{"results":[],"errors":["pipeline has no registered signals; results will be empty"],"duration_ms":0}
```

`hasErrors()` is therefore `true` for a default pipeline. This is
`PipelineError::NoSignals`, and it is **expected** on PHP today, not a
misconfiguration on your side.

### Why PHP gets zero signals

**PHP cannot configure which detectors run.** `PipelineConfig`
carries only `maxConcurrency` and `timeoutMs`:

```php
// The entire PHP-side config surface.
new PipelineConfig(maxConcurrency: 8, timeoutMs: 5000);
// → {"max_concurrency":8,"timeout_ms":5000}
```

Detectors and tiered chains exist in the Rust core
(`core/src/registry.rs`, `core/src/chain.rs`, ADR-0003), but
`c12n_pipeline_new` in `core/src/ffi.rs` constructs
`InnerPipeline::new(vec![], ...)` — a hardcoded empty signal vector —
and neither `ffi.rs` nor the config struct it deserialises references
the registry at all.

So the honest current state: **a PHP caller gets a pipeline that
constructs, evaluates, and closes correctly, and returns zero results
plus the NoSignals diagnostic.** There is no PHP-side way to enable
PII, jailbreak, or language detection. Do not ship routing logic that
depends on a signal firing; there is nothing to configure that would
make one fire.

Treat this story's status as `partial` until detector config is
plumbed through `c12n_pipeline_new`.

## Steps

```php
use HopTop\C12n\ClassificationContext;
use HopTop\C12n\Exception\C12nException;
use HopTop\C12n\Exception\PipelineException;
use HopTop\C12n\Pipeline;
use HopTop\C12n\PipelineConfig;
use HopTop\C12n\PipelineResult;

try {
    $pipeline = new Pipeline(new PipelineConfig());
} catch (C12nException $e) {
    // Library missing / unloadable — see US-0020.
    fwrite(STDERR, 'c12n unavailable: ' . $e->getMessage() . PHP_EOL);
    exit(1);
}

try {
    $json = $pipeline->evaluate(new ClassificationContext(text: 'classify me'));
    $result = new PipelineResult($json);

    // Diagnostics ride along with a SUCCESSFUL evaluation.
    foreach ($result->errors() as $err) {
        printf("diagnostic: %s\n", $err);
    }
} catch (PipelineException $e) {
    fwrite(STDERR, 'evaluate failed: ' . $e->getMessage() . PHP_EOL);
} finally {
    $pipeline->close();
}
```

Real output:

```
diagnostic: pipeline has no registered signals; results will be empty
```

Thrown-channel examples, both executed:

```php
$pipeline->close();
$pipeline->evaluate(new ClassificationContext(text: 'after close'));
// after close: c12n: pipeline is closed

new PipelineResult('{ not json');
// bad json: c12n: failed to parse result JSON: Syntax error
```

Distinguishing the two channels matters: a `NoSignals` diagnostic
should not page anyone, while a `PipelineException` means the call
did not happen.

## Verify

```bash
cd php
C12N_CORE_LIB_PATH="$(cd .. && pwd)/target/release/libc12n_core.dylib" \
  ./vendor/bin/phpunit --no-coverage --filter PipelineFfiIntegrationTest
```

`testEvaluateReturnsValidEnvelopeForDefaultConfig` and
`testPipelineResultParsesRoundtripJson` both assert exactly one error
containing `no registered signals`, and the latter asserts
`hasErrors() === true`.

## How it works

`PipelineResult::__construct` inspects the decoded envelope before
populating accessors. A top-level `{"error": "..."}` key — the shape
the FFI returns for a context Rust could not deserialise — is
converted into a thrown `PipelineException`. A `errors` **array** is
kept as data and exposed through `errors()`. Same wire field family,
deliberately different handling: one means the evaluation failed, the
other means it succeeded with caveats.

`Pipeline::__construct` throws when `c12n_pipeline_new` returns null,
guarding both PHP's plain `null` and an `FFI\CData`-wrapped null
pointer, since PHP FFI returns the former for NULL `void*`.

## Tests

- [`php/tests/PipelineFfiIntegrationTest.php`](../../php/tests/PipelineFfiIntegrationTest.php)
  — `testEvaluateReturnsValidEnvelopeForDefaultConfig`,
  `testPipelineResultParsesRoundtripJson`,
  `testEmptyPipelineJsonShapeMatchesCanonicalParity`,
  `testConstructorThrowsOnInvalidConfigJson`,
  `testEvaluateReturnsErrorEnvelopeForMalformedContext`
- [`php/tests/PipelineTest.php`](../../php/tests/PipelineTest.php) —
  malformed-JSON parse errors
- [`docs/adr/0003-tiered-detector-chains.md`](../adr/0003-tiered-detector-chains.md)
  — the chain design PHP cannot yet reach

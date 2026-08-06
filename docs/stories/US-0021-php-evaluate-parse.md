---
status: shipped
personas: [middleware-developer, internal-tool-builder, cost-control-startup]
priority: P0
---

# US-0021: Evaluate a context and parse the result in PHP

As a tool author on PHP, I want to build a pipeline, evaluate a
classification context, and read signals off the result with typed
accessors instead of walking decoded arrays.

## Use this when

- Wiring c12n into a PHP request path that routes by prompt content.
- Extracting per-signal confidence for logging or metrics.
- Asserting classification behaviour in a PHPUnit suite.

## Result

`Pipeline::evaluate()` returns the raw JSON envelope as a `string`.
Wrapping it in `new PipelineResult($json)` gives `results()`,
`errors()`, `hasErrors()`, `durationMs()`, `signal($type)` and
`confidence($type)`.

**Today every PHP pipeline returns zero signal results.** See
[US-0022](US-0022-php-no-signals.md) — read it before you build
routing logic on these accessors.

## Steps

```php
use HopTop\C12n\ClassificationContext;
use HopTop\C12n\Pipeline;
use HopTop\C12n\PipelineConfig;
use HopTop\C12n\PipelineResult;

$pipeline = new Pipeline(new PipelineConfig(maxConcurrency: 8, timeoutMs: 5000));

try {
    $json = $pipeline->evaluate(new ClassificationContext(
        text: 'Write a Python function to sort a list',
        history: ['earlier turn'],
        headers: ['x-trace' => 'abc123'],
        config: ['mode' => 'strict'],
    ));

    $result = new PipelineResult($json);

    printf("results: %d\n", count($result->results()));
    printf("hasErrors: %s\n", $result->hasErrors() ? 'true' : 'false');
    printf("durationMs: %d\n", $result->durationMs());
    printf("confidence(PII): %.2f\n", $result->confidence('PII'));
    var_dump($result->signal('PII'));
} finally {
    $pipeline->close();
}
```

Real output, run against `libc12n_core` built from this tree:

```
results: 0
hasErrors: true
durationMs: 0
confidence(PII): 0.00
NULL
```

Note what the accessors do on an empty result set: `signal()` returns
`null`, `confidence()` returns `0.0`. Neither distinguishes "signal
ran and scored 0.0" from "no signal ran". Branch on
`$result->signal($type) !== null`, not on the confidence value.

### Lifecycle

`close()` is idempotent and `__destruct` calls it, so `try/finally`
is belt-and-braces rather than mandatory. Evaluating after close
throws:

```php
$pipeline->close();
$pipeline->evaluate(new ClassificationContext(text: 'after close'));
// PipelineException: c12n: pipeline is closed
```

`Pipeline` is **not** safe for concurrent use without external
synchronisation.

## Verify

```bash
cd php
C12N_CORE_LIB_PATH="$(cd .. && pwd)/target/release/libc12n_core.dylib" \
  ./vendor/bin/phpunit --no-coverage --filter PipelineFfiIntegrationTest
```

The snippet above was executed directly; its output is pasted verbatim.

## How it works

`ClassificationContext::toFfiJson()` encodes the context for the wire,
normalising empty PHP arrays in map positions (`headers`, `config`) to
`{}` rather than `[]` so Rust's serde `HashMap` deserialiser accepts
them — Go's `map[string]string` and Python's `dict` encode empty the
same way, keeping the wire shape identical across bindings.

`Pipeline::evaluate()` calls `c12n_pipeline_evaluate`, copies the
returned C string into PHP memory with `FFI::string()`, then frees the
FFI-owned buffer in a `finally`. The opaque pointer is nulled on
`close()` so post-close calls throw instead of dereferencing freed
memory.

`SignalResult::fromArray()` maps the wire's `signal_type` key onto the
PHP property `type` — the field names differ across the boundary.

> **Doc drift:** `php/src/PipelineResult.php:23-24` claims the FFI
> returns `duration_ms` "unlike the cgo Go binding which uses
> `duration_ns`". Go's `result.go:12` uses
> `DurationMs int64 \`json:"duration_ms"\`` — both sides agree, and
> the comment describes a difference that no longer exists.

## Tests

- [`php/tests/PipelineFfiIntegrationTest.php`](../../php/tests/PipelineFfiIntegrationTest.php)
  — `testEvaluateReturnsValidEnvelopeForDefaultConfig`,
  `testPipelineResultParsesRoundtripJson`,
  `testEvaluatePropagatesAllContextFields`,
  `testCloseIsIdempotent`, `testEvaluateAfterCloseThrows`,
  `testDestructorCleansUpUnclosedPipeline`
- [`php/tests/PipelineTest.php`](../../php/tests/PipelineTest.php) —
  config + context encoding without FFI

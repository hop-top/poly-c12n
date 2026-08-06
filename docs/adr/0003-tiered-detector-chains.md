# ADR-0003: Detector traits accept multiple implementations as configurable tiers

Status: accepted
Date: 2026-08-05
Track: `detector-chain`

## Context

`c12n-core` defines nine abstraction traits that signals depend on:

| Trait | Module | Returns |
|-------|--------|---------|
| `PiiDetector` | `signals/safety.rs` | `Result<Vec<PiiEntity>, _>` |
| `JailbreakDetector` | `signals/safety.rs` | `Result<(f64, Vec<String>), _>` |
| `ToxicityDetector` | `signals/safety.rs` | `Result<Vec<(String, f64)>, _>` |
| `LanguageDetector` | `signals/language.rs` | `Option<DetectedLanguage>` + `Vec<_>` |
| `CategoryClassifier` | `signals/domain.rs` | `Result<Vec<(String, f64)>, _>` |
| `SatisfactionDetector` | `signals/feedback.rs` | `Result<f64, _>` |
| `Tokenizer` | `signals/context.rs` | `usize` |
| `EmbeddingEngine` | `embedding.rs` | `Result<Vec<f32>, _>` |
| `PreferenceLlm` | `signals/preference.rs` | `Result<String, _>` |

Each signal held exactly one boxed implementation
(`PiiSignal { detector: Box<dyn PiiDetector> }`). That forces a build-time
choice between a cheap-and-approximate detector and an expensive-and-accurate
one, when what operators actually want is both: run the cheap one, and only pay
for the expensive one when the cheap one is unsure.

The canonical case is PII — regex catches the easy 90% for free, an NLP model
catches the rest, a local LLM is the last resort. The same shape recurs for
toxicity and domain classification. For the scalar traits the second
implementation is not a quality escalation but a *failover*: if the primary
embedding provider is down, use the secondary.

## Decision

Introduce `core/src/chain.rs`: a generic `Chain<T: ?Sized>` holding
`Vec<Arc<T>>` plus a `ChainStrategy`. Every one of the nine traits is
chainable, uniformly. Strategy is selected per signal.

### Strategy enum

```rust
pub enum ChainStrategy {
    Escalate { threshold: f64 },  // default, threshold 0.5
    MergeAll,
    FallbackOnError,
}
```

- **`Escalate`** — run tier 0; advance when the result is empty *or* its
  confidence is `< threshold`. First tier clearing the bar wins.
- **`MergeAll`** — run every tier, union the results, confidence = max.
- **`FallbackOnError`** — advance only on `Err`. No confidence-based
  escalation.

Selected from config via `ChainStrategy::parse`, which accepts `"escalate"`,
`"escalate:0.75"`, `"merge_all"`/`"merge"`, `"fallback_on_error"`/`"fallback"`.
This keeps strategy selection a per-signal string in existing config plumbing
rather than a new structured type every binding would have to marshal.

### Scalar traits: explicit config validation, not silent degradation

`Tokenizer -> usize`, `EmbeddingEngine -> Vec<f32>` and `PreferenceLlm ->
String` carry no confidence, so `Escalate` and `MergeAll` are meaningless for
them. This is encoded as an associated constant on a marker trait:

```rust
pub trait ChainableTier {
    const TIER_KIND: &'static str;
    const SUPPORTS_CONFIDENCE: bool;
}
```

`Chain::new` reads `SUPPORTS_CONFIDENCE` and returns
`SignalError::Configuration` naming both the trait and the remedy when the
combination is invalid.

**Why validation rather than a hard compile-time split.** A fully
compile-time-illegal encoding (separate `ConfidenceChain` / `ScalarChain`
types, or a sealed strategy type parameter) was rejected: strategy arrives as
a *runtime config string*, so an invalid combination is a config error that
must be reportable as one. Splitting the type would only move the failure to
the parse site while doubling the API surface, and every binding (Go/Py/PHP/TS)
would need to know which trait got which container. The associated constant
keeps the knowledge in one place, is visible at compile time to any code that
wants it, and yields a clear runtime diagnostic on the config path. The
constraint is expressed once and enforced at the only boundary that can
actually see the config value.

### Error policy

| Strategy | One tier errors | Every tier errors |
|----------|-----------------|-------------------|
| `Escalate` | record + skip, continue to next tier | `Err` |
| `MergeAll` | record + skip, merge the rest | `Err` |
| `FallbackOnError` | record + advance | `Err` |

Under `Escalate` a failing tier does **not** fail the chain. A broken cheap
tier must not deny the caller the answer an expensive tier can still produce —
that would make the cheap tier a single point of failure, which is the opposite
of what tiering is for. Under `MergeAll` a partial failure still merges the
surviving tiers, because a union of two of three sources is strictly better
than nothing. In all cases errors are recorded in provenance rather than
silently dropped.

If no tier clears the escalation threshold, `Escalate` returns the
highest-confidence result seen rather than erroring. Escalation is a quality
knob, not a hard gate.

**Single-tier transparency.** When a chain has exactly one tier, its error is
propagated with the original `SignalError` variant intact rather than wrapped.
Callers matching on the kind (`Configuration` vs `Inference` vs `Timeout`) keep
working exactly as they did before chains existed — this is load-bearing for
the existing `preference_preserves_error_kind` regression test. Only a genuine
multi-tier failure collapses into one `Inference` carrying the full tally,
because no single variant honestly represents N different failures.

### Dedup rules under `MergeAll`

- `PiiDetector` — key `(entity_type, start, end)`, keep max confidence.
- `Vec<(String, f64)>` traits (`ToxicityDetector`, `CategoryClassifier`) — key
  on the `String`, keep max score.
- `JailbreakDetector` — labels deduped by value, confidence = max.
- `LanguageDetector` — key on language code, keep max confidence.
- `SatisfactionDetector` — scalar score, max wins.

`CategoryClassifier` additionally renormalizes the merged distribution so the
downstream Shannon-entropy maths in `DomainSignal` still sees probabilities
summing to ~1.0. Merging two distributions without renormalizing would inflate
entropy and silently flip the signal into its multi-label branch.

### Tier provenance (hard requirement)

Every chain call returns `ChainOutcome<T> { value, provenance }`.
`ChainProvenance` records the strategy, tiers attempted in call order, the
winning tier index, and every swallowed tier error. Signals fold it into
`SignalResult::metadata` under the stable key `"chain"`:

```json
{ "strategy": "escalate", "tiers_attempted": [0, 1], "winning_tier": 1,
  "escalated": true, "tier_errors": [{"tier": 0, "error": "..."}] }
```

Without this an LLM tier that starts firing on every request is invisible until
the bill arrives. `winning_tier` is `null` under `MergeAll`, where several
tiers contribute by definition.

`PiiSignal` chunks long text and therefore calls its chain once per chunk; the
per-chunk records are folded into one, taking the union of tiers touched and
the deepest tier any chunk had to reach.

### Backward compatibility

Every existing constructor is preserved and delegates to `Chain::single`, a
one-element chain with `FallbackOnError` — behaviourally identical to a bare
boxed detector. A parallel `with_chain` / `with_chains` constructor takes a
chain. All 16 signals and their tests compile and pass unchanged.

The `Box<dyn X>` parameters of the legacy constructors are converted with
`Arc::from`, so no caller signature changed.

`SignalError` gains `From<EmbeddingError>`, mapping to
`SignalError::Inference` — exactly the mapping the signal bodies previously
applied by hand.

### Async

Execution is sequential for v1, including `MergeAll`. `Escalate` is inherently
sequential (each decision depends on the previous tier's confidence).
`MergeAll` could run tiers concurrently and should eventually; it is left
sequential here to keep v1 small, and because the merge cost is dominated by
the slowest tier either way when tiers are cheap. Concurrency is a contained
follow-up: only the `MergeAll` arms would change.

## Consequences

- Detector traits keep their exact signatures. Tier-1 implementations being
  written in parallel (`RegexPiiDetector`, `HeuristicJailbreakDetector`,
  `StopwordLanguageDetector`, `ApproxTokenizer`) need no knowledge of chaining
  — `Chain` wraps them.
- Operators can trade cost against accuracy per signal without a rebuild.
- Tier provenance makes cost regressions debuggable from the signal output.
- A misconfigured strategy for a scalar trait fails loudly at construction.
- `EmbeddingEngine` tiers must agree on dimensionality;
  `Chain::validate_dimensions` checks this, since a failover that silently
  changes vector width would corrupt every prototype-bank comparison.

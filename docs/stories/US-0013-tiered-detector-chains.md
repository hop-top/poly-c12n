---
status: partial
personas: [llm-routing-saas, cost-control-startup, framework-author]
priority: P0
---

# US-0013: Trade cost against accuracy with tiered detector chains

As a tool author, I want to order several detector implementations
into tiers so the cheap one handles the easy cases and the expensive
one only runs when it is actually needed.

## Use this when

- A regex catches the easy 90% for free and an NLP model or LLM
  should handle only the remainder.
- You want a failover: if the primary embedding provider is down,
  use the secondary.
- You need to answer "did this result cost me an LLM call?" from the
  signal output, without instrumenting the detectors.

## Scope today: Rust-only

`Chain` is constructible **from Rust only**. The config plumbing that
would let a Go, Python, PHP or TypeScript caller name tiers and a
strategy in YAML/PKL has **not** landed — see the scope note in
[US-0009](US-0009-regex-pii-baseline.md). Design rationale lives in
[ADR-0003](../adr/0003-tiered-detector-chains.md).

## Result

`Chain<T>` holds `Vec<Arc<T>>` plus a `ChainStrategy`. All nine
detector traits are chainable uniformly. Every call returns
`ChainOutcome { value, provenance }`.

### Strategies

| Strategy | Behaviour | Config strings |
|----------|-----------|----------------|
| `Escalate { threshold }` | Run tier 0; advance when it errors, returns empty, or scores below `threshold`. First tier clearing the bar wins. | `"escalate"`, `"escalate:0.8"` |
| `MergeAll` | Run every tier, union and dedup the results, confidence = max. | `"merge_all"`, `"merge"` |
| `FallbackOnError` | Return tier 0's result however unconfident; advance **only** on `Err`. | `"fallback_on_error"`, `"fallback"` |

The default is `Escalate { threshold: 0.5 }`.

If no tier clears the threshold, `Escalate` returns the
highest-confidence result seen rather than erroring — escalation is a
quality knob, not a hard gate.

### Scalar traits reject confidence strategies

`Tokenizer → usize`, `EmbeddingEngine → Vec<f32>` and
`PreferenceLlm → String` carry no confidence, so `Escalate` and
`MergeAll` are meaningless for them. `Chain::new` **rejects** the
combination at construction rather than silently degrading:

```text
configuration error: Tokenizer produces no confidence value;
strategy 'escalate' is not supported — use 'fallback_on_error'
```

`FallbackOnError` on the same tiers builds fine.

## Steps

```rust
use std::sync::Arc;
use c12n_core::chain::{Chain, ChainStrategy};
use c12n_core::signals::detectors::RegexPiiDetector;
use c12n_core::signals::safety::PiiDetector;

let tiers: Vec<Arc<dyn PiiDetector>> = vec![Arc::new(RegexPiiDetector::new())];
let chain = Chain::new(tiers, ChainStrategy::Escalate { threshold: 0.8 })?;

let outcome = chain.detect_entities("card 4111111111111111").await?;
// outcome.value       → [CREDIT_CARD]
// outcome.provenance  → see below
```

Parsing a strategy from a config string:

```rust
ChainStrategy::parse("escalate")         // Ok(Escalate { threshold: 0.5 })
ChainStrategy::parse("escalate:0.8")     // Ok(Escalate { threshold: 0.8 })
ChainStrategy::parse("merge")            // Ok(MergeAll)
ChainStrategy::parse("fallback")         // Ok(FallbackOnError)
ChainStrategy::parse("eskalate")
// Err: configuration error: unknown chain strategy: eskalate
```

A threshold outside `0.0..=1.0` is rejected the same way.

## Tier provenance

Every result carries a record of what actually ran, folded into
`SignalResult::metadata` under the stable key `"chain"`:

```json
{
  "strategy": "escalate",
  "tiers_attempted": [0],
  "winning_tier": 0,
  "escalated": false,
  "tier_errors": []
}
```

`escalated` is `true` when more than the first tier ran.
`winning_tier` is `null` under `MergeAll`, where several tiers
contribute by definition, and when every tier failed. `tier_errors`
records `{tier, error}` for each tier that failed without aborting
the chain.

Without this an LLM tier that starts firing on every request is
invisible until the bill arrives.

## Error policy

| Strategy | One tier errors | Every tier errors |
|----------|-----------------|-------------------|
| `Escalate` | record + skip, continue | `Err` |
| `MergeAll` | record + skip, merge the rest | `Err` |
| `FallbackOnError` | record + advance | `Err` |

A broken cheap tier must not deny you the answer an expensive tier
can still produce — that would make the cheap tier a single point of
failure, the opposite of what tiering is for.

**Single-tier transparency**: a one-tier chain propagates its error
with the original `SignalError` variant intact, so callers matching
on `Configuration` vs `Inference` vs `Timeout` keep working exactly
as they did before chains existed. Only a genuine multi-tier failure
collapses into one `Inference` carrying the full tally.

## Verify

```bash
cargo test -p hop-top-c12n-core --lib chain::
```

## How it works

Existing single-detector constructors are preserved and delegate to
`Chain::single` — a one-element `FallbackOnError` chain, behaviourally
identical to a bare boxed detector. A parallel `with_chain` /
`with_chains` constructor takes a chain, so adopting tiers is
opt-in and no caller signature changed.

Under `MergeAll`, dedup is per-trait: PII keys on
`(entity_type, start, end)` keeping max confidence; scored-label
traits key on the label; language keys on the code.
`CategoryClassifier` additionally renormalizes the merged
distribution, because merging two distributions without
renormalizing would inflate the downstream Shannon entropy and
silently flip the signal into its multi-label branch.

`EmbeddingEngine` tiers must agree on dimensionality;
`Chain::validate_dimensions` enforces it, since a failover that
silently changed vector width would corrupt every prototype-bank
comparison.

Execution is sequential for v1, including `MergeAll`. `Escalate` is
inherently sequential; concurrent `MergeAll` is a contained
follow-up.

## Tests

- [`core/src/chain.rs`](../../core/src/chain.rs) —
  `parses_strategy_specs`, `rejects_unknown_and_out_of_range_specs`,
  `default_strategy_is_escalate`, `rejects_empty_tier_list`,
  `scalar_trait_rejects_confidence_strategies`,
  `scalar_trait_accepts_fallback_on_error`,
  `confidence_trait_accepts_every_strategy`,
  `escalate_stops_at_confident_first_tier`,
  `escalate_advances_on_low_confidence`,
  `escalate_advances_on_empty_result`,
  `escalate_skips_failing_tier_and_records_it`,
  `escalate_falls_back_to_best_when_none_confident`,
  `escalate_errors_when_every_tier_fails`,
  `merge_all_dedups_pii_on_type_and_span`,
  `merge_all_dedups_scored_labels_keeping_max`,
  `merge_all_tolerates_partial_tier_failure`,
  `merge_all_fails_when_every_tier_fails`,
  `fallback_keeps_unconfident_first_tier`,
  `fallback_advances_only_on_error`,
  `fallback_fails_over_scalar_llm_tier`,
  `provenance_lands_in_metadata`,
  `provenance_reports_swallowed_tier_errors`,
  `merge_all_provenance_has_no_winning_tier`,
  `single_tier_chain_is_transparent`,
  `single_tier_chain_preserves_error_variant`,
  `embedding_chain_rejects_mismatched_dimensions`

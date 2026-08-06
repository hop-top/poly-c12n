---
status: partial
personas: [framework-author, internal-tool-builder, llm-routing-saas]
priority: P1
---

# US-0014: Build detector chains from configuration names

As a tool author, I want to name detectors as strings so a chain can
be assembled from configuration instead of hard-coded types.

## Use this when

- Assembling a chain whose tiers are decided by config, not by a
  build-time `use` statement.
- Listing valid detector names in `--help` or a validation error.
- Preparing for the config plumbing that will eventually let non-Rust
  bindings do the same.

## Scope today: Rust-only

The registry exists **because** trait objects cannot cross a C ABI: a
Go, PHP, Python or TypeScript caller can never hand a
`Box<dyn PiiDetector>` to the engine, so naming detectors in
configuration is the only mechanism that could work for them.

That mechanism is **not wired up yet**. `c12n_pipeline_new`
(`core/src/ffi.rs`), `core/src/wasm.rs` and `py/src/lib.rs` still
construct their pipeline with `vec![]` signals and never consult the
registry. Today only Rust callers can use it. A non-Rust pipeline
detects nothing and reports
[`PipelineError::NoSignals`](US-0015-no-signals-diagnostic.md).

## Result

Registered names, one per slot:

| Slot | Name | Implementation |
|------|------|----------------|
| `pii` | `regex` | [`RegexPiiDetector`](US-0009-regex-pii-baseline.md) |
| `jailbreak` | `heuristic` | [`HeuristicJailbreakDetector`](US-0010-heuristic-jailbreak-baseline.md) |
| `language` | `stopword` | [`StopwordLanguageDetector`](US-0011-stopword-language-baseline.md) |
| `tokenizer` | `approx` | [`ApproxTokenizer`](US-0012-approx-tokenizer-estimate.md) |

**These strings are public API.** They appear in user configuration
files, so renaming one is a breaking change for every binding — add
an alias instead.

## Steps

```rust
use c12n_core::registry;

// One detector by name.
let d = registry::pii_detector("regex")?;

// A whole chain: tier order + strategy, both from config strings.
let chain = registry::pii_chain(&["regex"], "escalate:0.8")?;
let out = chain.detect_entities("card 4111111111111111").await?;
// → [CREDIT_CARD]

// Same shape for the other slots.
let jb   = registry::jailbreak_chain(&["heuristic"], "fallback")?;
let lang = registry::language_chain(&["stopword"], "merge_all")?;
```

Discovering valid names:

```text
registry::available("pii")         → ["regex"]
registry::available("jailbreak")   → ["heuristic"]
registry::available("language")    → ["stopword"]
registry::available("tokenizer")   → ["approx"]
registry::available("nonexistent") → []
```

## Unknown names fail loudly

A typo must never silently yield a pipeline that detects nothing —
that is the failure mode where an operator believes PII is being
caught when it is not. Every rejection names the bad value and lists
the alternatives:

```text
registry::pii_detector("rgex")
// configuration error: unknown pii detector "rgex"; available: regex

registry::pii_chain(&[], "escalate")
// configuration error: PiiDetector chain requires at least one tier

registry::pii_chain(&["regex"], "eskalate")
// configuration error: unknown chain strategy: eskalate
```

All three are `SignalError::Configuration`, so a binding can map them
onto its own config-error type.

## Verify

```bash
cargo test -p hop-top-c12n-core --lib registry::
```

## How it works

Each `*_detector` function matches the name against the registered
constants and returns `Arc<dyn Trait>`. Each `*_chain` function maps
the name list through that lookup, parses the strategy via
`ChainStrategy::parse`, and hands both to `Chain::new` — so chain
construction rules from
[US-0013](US-0013-tiered-detector-chains.md) (empty tier lists,
scalar-trait strategy validation) apply unchanged.

Rust callers are **not** restricted to the registry: they can build a
`Chain` from their own trait implementations directly. The registry
exists so that the other four bindings have *some* way to configure a
working pipeline once the config plumbing lands.

`ApproxTokenizer` has a registered name but no `tokenizer_detector`
constructor — `ContextSignal` owns tokenizer wiring and constructs it
directly. `available("tokenizer")` still reports the name for
diagnostics.

## Tests

- [`core/src/registry.rs`](../../core/src/registry.rs) —
  `builds_registered_pii_detector`,
  `unknown_name_fails_loudly_and_names_alternatives`,
  `single_tier_chain_detects_through_registry`,
  `empty_chain_rejected`, `bad_strategy_rejected`,
  `available_lists_every_slot`, `tokenizer_default_is_registered`

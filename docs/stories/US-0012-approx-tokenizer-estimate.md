---
status: partial
personas: [cost-control-startup, llm-routing-saas, framework-author]
priority: P1
---

# US-0012: Estimate token counts without a vocabulary

As a tool author, I want a rough token count with no model download
so size-based routing works out of the box.

## Use this when

- Coarse size bucketing — "small / medium / huge prompt".
- Rough cost projection for dashboards, with a stated margin.
- Routing decisions where being 15% off changes nothing.

**Do not use this when** the number must be right — see the accuracy
table below.

## Scope today: Rust-only

`ApproxTokenizer` is constructible **from Rust only** — see the scope
note in [US-0009](US-0009-regex-pii-baseline.md).

## Result

`count_tokens` returns a `usize`. `model_name()` returns a string
that always carries an **`approx`** marker, so anything reading
`tokenizer_model` out of `ContextSignal`'s metadata can see the
number is not authoritative:

```text
ApproxTokenizer::new().model_name()                    → "approx-v1"
ApproxTokenizer::for_model("gpt-4o", 3.8).model_name() → "gpt-4o-approx"
ApproxTokenizer::for_model("my-approx-counter", 3.0)   → "my-approx-counter"
```

`for_model` appends `-approx` unless the name already advertises
itself as one. A non-finite or non-positive `chars_per_token` falls
back to the default `4.0` rather than producing nonsense.

## This is an ESTIMATE — read this before shipping

There is no BPE vocabulary, no merge table, and no model. It counts
characters by class and divides. Measured against typical
cl100k-family behaviour:

| Input                            | Expected error |
|----------------------------------|----------------|
| English/Latin prose              | ±10–15%        |
| Prose with heavy punctuation     | ±20%           |
| Source code, JSON, markup        | ±25–35% (usually **under**-counts) |
| CJK text                         | ±30%           |
| Base64 / hashes / random strings | ±50% (badly under-counts) |
| Emoji, rare Unicode              | unbounded (can under-count several-fold) |

**Do not** use this to enforce a hard context-window limit, to bill a
customer, or to decide whether a request fits a model's budget
without a safety margin. An under-count on base64 is not a rounding
error — it is the difference between a request that fits and a
request the provider rejects.

For exact counts, implement the `Tokenizer` trait over the target
model's real tokenizer and pass that instead; the signal takes
`Arc<dyn Tokenizer>`, so nothing else has to change.

## Steps

```rust
use c12n_core::signals::context::Tokenizer;
use c12n_core::signals::detectors::ApproxTokenizer;

let tk = ApproxTokenizer::new();
let estimate = tk.count_tokens(text);
```

Actual counts:

```text
prose:  chars=83  tokens=22   // ~3.8 chars/token — close to the 4.0 rule
base64: chars=68  tokens=18   // real BPE would emit far more
json:   chars=65  tokens=22   // punctuation-heavy, under-counts
cjk:    chars=13  tokens=13   // ~1 token per character
```

Empty input is 0 tokens; any non-empty input is at least 1.

Tune the divisor for a specific model family:

```rust
let tk = ApproxTokenizer::for_model("gpt-4o", 3.8);
```

## Verify

```bash
cargo test -p hop-top-c12n-core --lib signals::detectors::tokenizer_approx
```

## How it works

1. Characters are partitioned into CJK (CJK/Hiragana/Katakana/Hangul
   blocks), punctuation/symbol runs, and everything else.
2. Non-CJK characters are divided by `chars_per_token` (default
   `4.0`); CJK characters by `1.0`, since BPE tokenizers emit roughly
   one token per CJK character.
3. Each run of punctuation adds a flat 0.5-token marginal cost,
   reflecting that BPE rarely merges punctuation into neighbouring
   words.
4. The result is rounded up.

Counting is by **character**, not byte, so multi-byte text is not
double-charged. The function is deterministic and monotonic in
length.

## Tests

- [`core/src/signals/detectors/tokenizer_approx.rs`](../../core/src/signals/detectors/tokenizer_approx.rs) —
  `model_name_advertises_approximation`,
  `divisor_accessors_report_the_configured_value`,
  `invalid_chars_per_token_falls_back_to_default`,
  `empty_text_is_zero_tokens`,
  `any_non_empty_text_is_at_least_one_token`,
  `count_is_monotonic_in_length`, `deterministic`,
  `english_prose_lands_near_four_chars_per_token`,
  `punctuation_increases_the_estimate`,
  `cjk_costs_roughly_one_token_per_character`,
  `cjk_denser_than_latin_of_same_char_count`,
  `multibyte_counted_by_character_not_byte`,
  `emoji_do_not_panic`, `custom_ratio_changes_the_estimate`,
  `wired_into_context_signal`,
  `context_signal_buckets_by_estimated_size`,
  `context_signal_costs_track_estimate`

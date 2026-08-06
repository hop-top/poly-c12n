---
status: partial
personas: [llm-routing-saas, internal-tool-builder]
priority: P1
---

# US-0011: Identify Western European languages by stopword frequency

As a tool author, I want a dependency-free language detector so I can
route non-English prompts to a model that handles them well.

## Use this when

- Routing by language to a locale-appropriate model.
- Tagging traffic for analytics.
- Detecting code-switching in a single prompt.

## Scope today: Rust-only

`StopwordLanguageDetector` is constructible **from Rust only** — see
the scope note in [US-0009](US-0009-regex-pii-baseline.md).

## Result

Six languages, in declaration order:

```rust
StopwordLanguageDetector::supported_codes()
// → ["en", "es", "fr", "de", "pt", "it"]
```

`detect` returns `Option<DetectedLanguage>` (`code`, `name`,
`confidence`); `detect_multiple` returns every candidate above the
floor, sorted by confidence descending. Confidence is **capped at
0.95** and never asserts certainty.

Anything outside those six returns `None` rather than a wrong guess.
That is deliberate: a confident wrong answer is worse than no answer.

## What it does NOT do — read this before shipping

This is a **baseline** detector, not a language-ID model:

- **No script detection.** Chinese, Japanese, Korean, Arabic, Hebrew,
  Cyrillic, Devanagari, Thai and Greek text yields `None` — not
  "unknown script", just nothing.
- **Short text is unreliable.** Under roughly 15–20 words results are
  noisy; below two stopword hits nothing is reported at all.
- **Closely related languages confuse it.** Spanish/Portuguese and
  Spanish/Italian share function-word morphology; expect both to
  appear in `detect_multiple`.
- **Domain text degrades badly.** Source code, log lines, URLs and
  keyword lists have almost no function words and usually yield
  `None`.
- **No dialect or regional variants** — en-GB vs en-US, pt-BR vs
  pt-PT are indistinguishable.

## Steps

```rust
use c12n_core::signals::detectors::StopwordLanguageDetector;
use c12n_core::signals::language::LanguageDetector;

let ld = StopwordLanguageDetector::new();
let hit = ld.detect(text);
let all = ld.detect_multiple(text);
```

Actual output:

```text
en   -> Some(DetectedLanguage { code: "en", name: "English", confidence: 0.95 })
ja   -> None                       // Japanese: unsupported script
code -> None                       // a line of Rust: no function words
```

Code-switched input surfaces every candidate:

```text
detect_multiple -> [
  DetectedLanguage { code: "es", name: "Spanish",    confidence: 0.80 },
  DetectedLanguage { code: "en", name: "English",    confidence: 0.746 },
  DetectedLanguage { code: "pt", name: "Portuguese", confidence: 0.561 },
]
```

Note Portuguese appearing for Spanish text — that is the
related-language confusion described above, working as designed.

## Verify

```bash
cargo test -p hop-top-c12n-core --lib signals::detectors::language_stopword
```

## How it works

Text is lowercased and split into alphabetic tokens. Each token is
looked up in a stopword index built once at construction. A language
is reported only when it clears both floors: at least 2 stopword
hits, and at least 8% of tokens matching. Stopwords shared across the
covered languages (`a`, `no`, `la`, `de`, `in`, `e`) are deliberately
excluded from every list rather than counted for everyone, so the
score discriminates instead of rewarding common forms.

Confidence blends the hit ratio with the margin over the
runner-up, then caps at 0.95.

## Tests

- [`core/src/signals/detectors/language_stopword.rs`](../../core/src/signals/detectors/language_stopword.rs) —
  `detects_each_supported_language`,
  `confidence_never_claims_certainty`,
  `supported_codes_matches_tables`, `empty_and_whitespace_yield_none`,
  `unsupported_scripts_yield_none_not_a_wrong_guess`,
  `short_input_below_hit_floor_yields_none`,
  `digits_and_symbols_only_yield_none`,
  `source_code_is_not_confidently_a_natural_language`,
  `detect_multiple_is_sorted_descending`,
  `detect_multiple_surfaces_code_switching`,
  `monolingual_text_beats_its_neighbours`,
  `accented_forms_are_matched_not_split`, `case_is_normalised`,
  `emoji_and_punctuation_do_not_panic`,
  `wired_into_language_signal`,
  `signal_reports_nothing_for_unsupported_script`

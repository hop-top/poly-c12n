---
status: partial
personas: [llm-routing-saas, middleware-developer, internal-tool-builder]
priority: P0
---

# US-0009: Catch structured PII with the regex baseline

As a tool author, I want a zero-dependency PII detector out of the
box so a pipeline flags obvious identifiers without me shipping a
model.

## Use this when

- Redacting the obvious before a prompt leaves your process.
- Routing prompts that contain identifiers to a private model.
- You want a floor, not a guarantee — and you are prepared to say so
  to your own users.

## Scope today: Rust-only

`RegexPiiDetector` is constructible **from Rust only**. Go, Python,
PHP and TypeScript callers cannot yet select it: `c12n_pipeline_new`
(`core/src/ffi.rs`), `core/src/wasm.rs` and `py/src/lib.rs` all build
their pipeline with `vec![]` signals. The config-schema plumbing that
would let a non-Rust caller name a detector has not landed. Until it
does, a non-Rust pipeline detects **nothing** and reports
[`PipelineError::NoSignals`](US-0015-no-signals-diagnostic.md).

## Result

`detect_entities` returns `Vec<PiiEntity>` — `entity_type`, matched
`text`, byte `start`/`end`, and `confidence`. Five types:

| Entity type   | Basis                                          | Confidence |
|---------------|------------------------------------------------|------------|
| `EMAIL`       | `local@domain.tld` shape                        | 0.95       |
| `PHONE`       | NANP-shaped 10-digit, optional `+1`/`()`        | 0.70       |
| `SSN`         | `NNN-NN-NNNN`, SSA-invalid ranges rejected      | 0.85       |
| `CREDIT_CARD` | 13–19 digits with a known IIN prefix            | 0.60 / 0.90 |
| `IP_ADDRESS`  | Dotted-quad IPv4, each octet ≤ 255              | 0.80       |

Luhn-valid cards score 0.90; Luhn-invalid ones still score 0.60 and
are still reported, because a typo'd card number is still a card
number.

## What it does NOT catch — read this before shipping

This is a **baseline** detector, not compliance tooling. It has no
model, no gazetteer, and no notion of context. It misses:

- **Names** of any kind — people, organisations.
- **Postal addresses** and any free-text location.
- **Dates of birth** and other context-dependent identifiers; a bare
  `1987-04-02` is indistinguishable from a release date.
- **National IDs outside the US** — passport numbers, NHS numbers,
  NINO, SIN, CPF, Aadhaar, and so on.
- **Bank account / IBAN / routing numbers**, medical record numbers,
  licence-plate numbers, biometric references.
- **Non-NANP phone numbers** — most international formats.
- **IPv6 addresses.**
- **All obfuscated forms** — `a [at] b [dot] com`, digits spelled out
  in words, PII split across lines, anything base64- or URL-encoded.

It is **not** GDPR, HIPAA, PCI-DSS or CCPA compliance tooling and
must not be the sole control for a redaction or data-residency
requirement. Treat a clean result as "no *obvious* structured
identifiers", **never** as "no PII".

Deliberate false-negative policies, so you are not surprised: bare
9-digit runs are not reported as `SSN` (invoice numbers); digit runs
without a recognised IIN prefix are not reported as `CREDIT_CARD`;
dotted-quads that look like version strings are not reported as
`IP_ADDRESS`.

## Steps

```rust
use c12n_core::signals::detectors::RegexPiiDetector;
use c12n_core::signals::safety::PiiDetector;

let detector = RegexPiiDetector::new();
let found = detector
    .detect_entities("mail bob@example.com or call 555-123-4567; card 4111 1111 1111 1111")
    .await?;

for e in &found {
    println!("{} {:?} [{}..{}] conf={}",
        e.entity_type, e.text, e.start, e.end, e.confidence);
}
```

Actual output:

```text
EMAIL "bob@example.com" [5..20] conf=0.95
PHONE "555-123-4567" [29..41] conf=0.7
CREDIT_CARD "4111 1111 1111 1111" [48..67] conf=0.9
```

The same call on text full of PII this detector cannot see returns
**zero entities**:

```rust
let missed = detector.detect_entities(
    "Jane Doe, 12 Rue de Rivoli, born 1987-04-02, \
     bob [at] example [dot] com, +33 6 12 34 56 78, 2001:db8::1",
).await?;
assert_eq!(missed.len(), 0); // name, address, DOB, obfuscated email,
                             // non-NANP phone, IPv6 — all missed
```

`start`/`end` are **byte** offsets into the input, directly usable
for slicing. They are not char indices; multi-byte text is handled
correctly.

## Verify

```bash
cargo test -p hop-top-c12n-core --lib signals::detectors::pii_regex
```

## How it works

Five regexes run over the input; matches are validated after the
fact (IPv4 octet range, SSA SSN ranges, IIN prefix, Luhn checksum),
then overlapping spans are resolved so digits inside an email are not
double-reported and an SSN shape wins over a phone shape. Results are
sorted by `start`.

`PiiSignal` wraps the detector and filters to a **deny list** of
entity types. With an empty deny list the signal reports
`confidence: 0.0` and no labels even when entities were found — the
entities still appear in `metadata["entities"]`. Pass the types you
care about:

```rust
let signal = PiiSignal::with_chain(
    chain,
    HashSet::from(["EMAIL".to_string(), "CREDIT_CARD".to_string()]),
    4096,
);
// → confidence=0.95 labels=["EMAIL", "CREDIT_CARD"]
```

## Tests

- [`core/src/signals/detectors/pii_regex.rs`](../../core/src/signals/detectors/pii_regex.rs) —
  `email_true_positives`, `email_true_negatives`,
  `phone_true_positives`, `phone_confidence_is_modest`,
  `ssn_true_positive`, `bare_nine_digit_number_is_not_an_ssn`,
  `ssn_invalid_ranges_rejected`, `luhn_accepts_known_test_cards`,
  `luhn_rejects_mutations`, `luhn_ignores_grouping_separators`,
  `credit_card_true_positive_is_high_confidence`,
  `luhn_invalid_card_is_lower_confidence`,
  `digit_run_without_card_prefix_is_not_a_card`,
  `ipv4_true_positives`, `ipv4_octet_overflow_rejected`,
  `version_strings_are_not_ip_addresses`,
  `offsets_are_byte_offsets_into_input`,
  `offsets_correct_with_multibyte_prefix`,
  `digits_inside_email_not_double_reported`,
  `ssn_wins_over_phone_shape`, `clean_prose_yields_nothing`,
  `wired_into_pii_signal`,
  `pii_signal_chunked_offsets_stay_absolute`

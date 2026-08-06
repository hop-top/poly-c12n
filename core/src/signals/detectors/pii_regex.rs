//! Regex-based PII detection — tier-1 baseline.

use async_trait::async_trait;
use regex::Regex;

use crate::signals::safety::{PiiDetector, PiiEntity};
use crate::types::SignalError;

/// Entity type emitted for RFC-5322-ish email addresses.
pub const ENTITY_EMAIL: &str = "EMAIL";
/// Entity type emitted for North-American-style phone numbers.
pub const ENTITY_PHONE: &str = "PHONE";
/// Entity type emitted for US Social Security Numbers.
pub const ENTITY_SSN: &str = "SSN";
/// Entity type emitted for payment card numbers.
pub const ENTITY_CREDIT_CARD: &str = "CREDIT_CARD";
/// Entity type emitted for IPv4 addresses.
pub const ENTITY_IP_ADDRESS: &str = "IP_ADDRESS";

/// A pattern-matching PII detector built on `regex`.
///
/// # What it catches
///
/// Structurally regular identifiers only:
///
/// | Entity type    | Basis                                              | Confidence |
/// |----------------|----------------------------------------------------|------------|
/// | `EMAIL`        | `local@domain.tld` shape                            | 0.95       |
/// | `PHONE`        | NANP-shaped 10-digit numbers, optional `+1`/`()`/separators | 0.70 |
/// | `SSN`          | `NNN-NN-NNNN` with SSA-invalid ranges rejected      | 0.85       |
/// | `CREDIT_CARD`  | 13–19 digit runs with a known IIN prefix            | 0.60 / 0.90 |
/// | `IP_ADDRESS`   | Dotted-quad IPv4 with each octet ≤ 255              | 0.80       |
///
/// # What it does NOT catch
///
/// This is a **baseline** detector, not compliance tooling. It has no model,
/// no gazetteer, and no notion of context. It will miss, among others:
///
/// - **Names** of any kind (people, organisations).
/// - **Postal addresses**, and any free-text location.
/// - **Dates of birth**, and other context-dependent identifiers — a bare
///   `1987-04-02` is indistinguishable from a release date.
/// - **National IDs outside the US**: passport numbers, NHS numbers, NINO,
///   SIN, CPF, Aadhaar, and so on.
/// - **Bank account / IBAN / routing numbers**, medical record numbers,
///   licence-plate numbers, biometric references.
/// - **Non-NANP phone numbers**: most international formats are missed.
/// - **IPv6 addresses**.
/// - **Obfuscated PII**: `a [at] b [dot] com`, digits spelled out in words,
///   PII split across lines, or anything base64/URL-encoded.
///
/// It is **not** GDPR, HIPAA, PCI-DSS, or CCPA compliance tooling, and must not
/// be relied on as the sole control for a redaction or data-residency
/// requirement. Treat a clean result as "no *obvious* structured identifiers",
/// never as "no PII".
///
/// # False positives
///
/// Deliberate policy choices, each documented at its matcher:
///
/// - Dotted-quads that look like software versions (a 4th component with a
///   leading zero, or 5+ dotted components such as `1.2.3.4.5`) are **not**
///   reported as `IP_ADDRESS`. `1.2.3.4` on its own IS a syntactically valid
///   IPv4 address and IS reported — callers wanting version strings excluded
///   need context this detector does not have.
/// - Bare 9-digit runs are **not** reported as `SSN`; only the dashed
///   `NNN-NN-NNNN` form is, so order/invoice numbers do not trip it.
/// - Digit runs are only reported as `CREDIT_CARD` when the IIN prefix is
///   recognised. Luhn-valid numbers get 0.90; Luhn-invalid ones get 0.60 and
///   are still reported, because typo'd card numbers are still card numbers.
///
/// # Offsets
///
/// `start`/`end` are byte offsets into the string passed to
/// [`PiiDetector::detect_entities`], directly usable for slicing. Multi-byte
/// text is handled correctly; the offsets are *not* char indices.
pub struct RegexPiiDetector {
    email: Regex,
    phone: Regex,
    ssn: Regex,
    card: Regex,
    ipv4: Regex,
}

impl Default for RegexPiiDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl RegexPiiDetector {
    /// Builds the detector. Panics only if the built-in patterns fail to
    /// compile, which is a programming error caught by the test suite.
    pub fn new() -> Self {
        Self {
            email: Regex::new(
                r"(?i)\b[a-z0-9._%+\-]+@[a-z0-9\-]+(?:\.[a-z0-9\-]+)*\.[a-z]{2,24}\b",
            )
            .expect("email pattern"),
            // NANP: optional +1/1 country code, area code (optionally
            // parenthesised), then 3-4 split. Separators: space, dot, hyphen.
            phone: Regex::new(r"(?:\+?1[ .\-]?)?(?:\(\d{3}\)|\d{3})[ .\-]\d{3}[ .\-]\d{4}\b")
                .expect("phone pattern"),
            // Dashed form only — see the false-positive note on the type.
            ssn: Regex::new(r"\b\d{3}-\d{2}-\d{4}\b").expect("ssn pattern"),
            // 13-19 digits, optionally grouped by single spaces or hyphens.
            card: Regex::new(r"\b\d{4}(?:[ \-]?\d{2,6}){2,4}\b").expect("card pattern"),
            // Dotted decimal; octet range is validated after matching.
            ipv4: Regex::new(r"\b\d{1,3}(?:\.\d{1,3}){3}\b").expect("ipv4 pattern"),
        }
    }

    /// Luhn (mod-10) checksum over the ASCII digits of `s`.
    ///
    /// Returns `false` for anything shorter than two digits. Non-digit bytes
    /// are ignored, so grouped forms (`4111 1111 1111 1111`) validate the same
    /// as compact ones.
    pub fn luhn_valid(s: &str) -> bool {
        let mut sum = 0u32;
        let mut count = 0usize;
        // Luhn doubles every second digit counting from the right.
        for (i, d) in s
            .bytes()
            .rev()
            .filter(|b| b.is_ascii_digit())
            .map(|b| u32::from(b - b'0'))
            .enumerate()
        {
            count += 1;
            let v = if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 {
                    doubled - 9
                } else {
                    doubled
                }
            } else {
                d
            };
            sum += v;
        }
        count >= 2 && sum.is_multiple_of(10)
    }

    /// True when every dotted component parses as an octet in `0..=255`.
    fn valid_ipv4(s: &str) -> bool {
        s.split('.').all(|o| o.parse::<u8>().is_ok())
    }

    /// True when the dotted-quad is more plausibly a version string.
    ///
    /// Heuristics, in order:
    /// - the match is part of a longer dotted run (`1.2.3.4.5`, `0.1.2.3.4`);
    /// - a non-first component has a redundant leading zero (`1.02.3.4`),
    ///   which no canonical IPv4 renderer emits;
    /// - the match is immediately preceded by `v`/`V` (`v1.2.3.4`).
    fn looks_like_version(text: &str, start: usize, end: usize) -> bool {
        let m = &text[start..end];
        if m.split('.')
            .skip(1)
            .any(|o| o.len() > 1 && o.starts_with('0'))
        {
            return true;
        }
        // A dot immediately before or after extends the run beyond 4 parts.
        if text[..start].ends_with('.') || text[end..].starts_with('.') {
            return true;
        }
        matches!(text[..start].chars().next_back(), Some('v') | Some('V'))
    }

    /// True when the digit run starts with a recognised payment-card IIN.
    ///
    /// Visa (4), Mastercard (51-55, 2221-2720), Amex (34/37), Discover (6011,
    /// 65, 644-649), JCB (3528-3589), Diners (300-305, 3095, 36, 38-39).
    fn known_card_prefix(digits: &str) -> bool {
        let len = digits.len();
        if !(13..=19).contains(&len) {
            return false;
        }
        let n = |a: usize, b: usize| digits[a..b].parse::<u32>().unwrap_or(u32::MAX);
        let d1 = n(0, 1);
        let d2 = n(0, 2);
        let d3 = n(0, 3);
        let d4 = n(0, 4);

        (d1 == 4 && (len == 13 || len == 16 || len == 19))
            || (16..=16).contains(&len) && (51..=55).contains(&d2)
            || len == 16 && (2221..=2720).contains(&d4)
            || len == 15 && (d2 == 34 || d2 == 37)
            || len == 16 && (d4 == 6011 || d2 == 65 || (644..=649).contains(&d3))
            || (16..=19).contains(&len) && (3528..=3589).contains(&d4)
            || len == 14 && ((300..=305).contains(&d3) || d4 == 3095 || d2 == 36)
            || len == 14 && (38..=39).contains(&d2)
    }

    /// True when an SSN falls in a range the SSA never issues:
    /// area `000`, `666`, or `900-999`; group `00`; serial `0000`.
    fn plausible_ssn(s: &str) -> bool {
        let mut parts = s.split('-');
        let (Some(a), Some(g), Some(sn)) = (parts.next(), parts.next(), parts.next()) else {
            return false;
        };
        let (Ok(a), Ok(g), Ok(sn)) = (a.parse::<u32>(), g.parse::<u32>(), sn.parse::<u32>()) else {
            return false;
        };
        a != 0 && a != 666 && a < 900 && g != 0 && sn != 0
    }

    /// True when `[start, end)` overlaps a range already claimed.
    fn overlaps(taken: &[(usize, usize)], start: usize, end: usize) -> bool {
        taken.iter().any(|&(s, e)| start < e && s < end)
    }

    /// Synchronous core of the detector. Exposed so callers that are not in an
    /// async context (and the chaining layer) can reuse it directly.
    ///
    /// Entities are returned sorted by `start`. Higher-precision entity types
    /// claim their span first, so an SSN is never also reported as a phone
    /// number fragment.
    pub fn scan(&self, text: &str) -> Vec<PiiEntity> {
        let mut out: Vec<PiiEntity> = Vec::new();
        let mut taken: Vec<(usize, usize)> = Vec::new();

        let push = |out: &mut Vec<PiiEntity>,
                    taken: &mut Vec<(usize, usize)>,
                    entity_type: &str,
                    m: &str,
                    start: usize,
                    end: usize,
                    confidence: f64| {
            if Self::overlaps(taken, start, end) {
                return;
            }
            taken.push((start, end));
            out.push(PiiEntity {
                entity_type: entity_type.to_string(),
                text: m.to_string(),
                start,
                end,
                confidence,
            });
        };

        // Email first: its span subsumes digit runs inside addresses.
        for m in self.email.find_iter(text) {
            push(
                &mut out,
                &mut taken,
                ENTITY_EMAIL,
                m.as_str(),
                m.start(),
                m.end(),
                0.95,
            );
        }

        // SSN before phone/card: the dashed form is unambiguous.
        for m in self.ssn.find_iter(text) {
            if Self::plausible_ssn(m.as_str()) {
                push(
                    &mut out,
                    &mut taken,
                    ENTITY_SSN,
                    m.as_str(),
                    m.start(),
                    m.end(),
                    0.85,
                );
            }
        }

        // Cards before phones: a 16-digit run is a card, not two phones.
        for m in self.card.find_iter(text) {
            let digits: String = m.as_str().chars().filter(|c| c.is_ascii_digit()).collect();
            if !Self::known_card_prefix(&digits) {
                continue;
            }
            let confidence = if Self::luhn_valid(&digits) {
                0.90
            } else {
                0.60
            };
            push(
                &mut out,
                &mut taken,
                ENTITY_CREDIT_CARD,
                m.as_str(),
                m.start(),
                m.end(),
                confidence,
            );
        }

        for m in self.phone.find_iter(text) {
            push(
                &mut out,
                &mut taken,
                ENTITY_PHONE,
                m.as_str(),
                m.start(),
                m.end(),
                0.70,
            );
        }

        for m in self.ipv4.find_iter(text) {
            if !Self::valid_ipv4(m.as_str()) || Self::looks_like_version(text, m.start(), m.end()) {
                continue;
            }
            push(
                &mut out,
                &mut taken,
                ENTITY_IP_ADDRESS,
                m.as_str(),
                m.start(),
                m.end(),
                0.80,
            );
        }

        out.sort_by_key(|e| e.start);
        out
    }
}

#[async_trait]
impl PiiDetector for RegexPiiDetector {
    async fn detect_entities(&self, text: &str) -> Result<Vec<PiiEntity>, SignalError> {
        Ok(self.scan(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    use crate::signal::Signal;
    use crate::signals::safety::PiiSignal;
    use crate::types::{ClassificationContext, SignalType};

    async fn detect(text: &str) -> Vec<PiiEntity> {
        RegexPiiDetector::new().detect_entities(text).await.unwrap()
    }

    fn types(entities: &[PiiEntity]) -> Vec<&str> {
        entities.iter().map(|e| e.entity_type.as_str()).collect()
    }

    fn make_ctx(text: &str) -> ClassificationContext {
        ClassificationContext {
            text: text.to_string(),
            history: vec![],
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    // -- Offsets ------------------------------------------------------------

    /// Every reported span must slice back to the reported text.
    fn assert_offsets_consistent(text: &str, entities: &[PiiEntity]) {
        for e in entities {
            assert!(
                e.start < e.end && e.end <= text.len(),
                "bad span {}..{} for {:?} in {:?}",
                e.start,
                e.end,
                e.entity_type,
                text
            );
            assert_eq!(&text[e.start..e.end], e.text, "span/text mismatch");
        }
    }

    #[tokio::test]
    async fn offsets_are_byte_offsets_into_input() {
        let text = "reach me at ada@example.com or 555-867-5309.";
        let entities = detect(text).await;
        assert_offsets_consistent(text, &entities);
        assert_eq!(types(&entities), vec![ENTITY_EMAIL, ENTITY_PHONE]);
    }

    #[tokio::test]
    async fn offsets_correct_with_multibyte_prefix() {
        // "Café" is 5 bytes / 4 chars; the emoji is 4 bytes / 1 char.
        let text = "Café ☕🚀 contact: ada@example.com fin";
        let entities = detect(text).await;
        assert_offsets_consistent(text, &entities);
        let email = &entities[0];
        assert_eq!(email.text, "ada@example.com");
        // Byte offset, not char index — they differ here.
        assert_eq!(email.start, text.find("ada@").unwrap());
        assert_ne!(email.start, text.chars().count() - email.text.len());
    }

    #[tokio::test]
    async fn offsets_correct_with_multibyte_between_entities() {
        let text = "ünïcödé 555-867-5309 中文 ada@example.com";
        let entities = detect(text).await;
        assert_offsets_consistent(text, &entities);
        assert_eq!(types(&entities), vec![ENTITY_PHONE, ENTITY_EMAIL]);
    }

    #[tokio::test]
    async fn entities_sorted_by_start() {
        let text = "ada@example.com 192.168.1.1 4111 1111 1111 1111 078-05-1120";
        let entities = detect(text).await;
        assert_offsets_consistent(text, &entities);
        let starts: Vec<usize> = entities.iter().map(|e| e.start).collect();
        let mut sorted = starts.clone();
        sorted.sort_unstable();
        assert_eq!(starts, sorted);
    }

    // -- Email --------------------------------------------------------------

    #[tokio::test]
    async fn email_true_positives() {
        for s in [
            "ada@example.com",
            "ada.lovelace+tag@sub.example.co.uk",
            "a_b%c@example-host.io",
            "USER@EXAMPLE.COM",
        ] {
            let entities = detect(s).await;
            assert_eq!(
                types(&entities),
                vec![ENTITY_EMAIL],
                "expected email in {s:?}"
            );
            assert!(entities[0].confidence > 0.9);
        }
    }

    #[tokio::test]
    async fn email_true_negatives() {
        for s in [
            "no at sign here",
            "ada [at] example [dot] com",
            "@handle mentions are not emails",
            "user@localhost",
        ] {
            let entities = detect(s).await;
            assert!(
                !types(&entities).contains(&ENTITY_EMAIL),
                "unexpected email in {s:?}: {entities:?}"
            );
        }
    }

    // -- Phone --------------------------------------------------------------

    #[tokio::test]
    async fn phone_true_positives() {
        for s in [
            "555-867-5309",
            "(415) 555-2671",
            "+1 415 555 2671",
            "415.555.2671",
        ] {
            let entities = detect(s).await;
            assert!(
                types(&entities).contains(&ENTITY_PHONE),
                "expected phone in {s:?}: {entities:?}"
            );
        }
    }

    #[tokio::test]
    async fn phone_confidence_is_modest() {
        // Phone shape is weak evidence — do not overstate it.
        let entities = detect("call 555-867-5309").await;
        assert!(entities[0].confidence <= 0.75);
    }

    #[tokio::test]
    async fn phone_true_negatives() {
        for s in [
            "the answer is 42",
            "order 1234567890 shipped",
            "build 20240102 released",
        ] {
            let entities = detect(s).await;
            assert!(
                !types(&entities).contains(&ENTITY_PHONE),
                "unexpected phone in {s:?}: {entities:?}"
            );
        }
    }

    // -- SSN ----------------------------------------------------------------

    #[tokio::test]
    async fn ssn_true_positive() {
        let entities = detect("SSN 078-05-1120 on file").await;
        assert_eq!(types(&entities), vec![ENTITY_SSN]);
        assert_eq!(entities[0].text, "078-05-1120");
    }

    #[tokio::test]
    async fn bare_nine_digit_number_is_not_an_ssn() {
        // Documented policy: only the dashed form is reported.
        let entities = detect("order number 123456789 confirmed").await;
        assert!(!types(&entities).contains(&ENTITY_SSN), "{entities:?}");
    }

    #[tokio::test]
    async fn ssn_invalid_ranges_rejected() {
        for s in [
            "000-12-3456",
            "666-12-3456",
            "900-12-3456",
            "123-00-4567",
            "123-45-0000",
        ] {
            let entities = detect(s).await;
            assert!(
                !types(&entities).contains(&ENTITY_SSN),
                "unexpected SSN in {s:?}: {entities:?}"
            );
        }
    }

    // -- Credit card --------------------------------------------------------

    #[tokio::test]
    async fn luhn_accepts_known_test_cards() {
        for s in [
            "4111111111111111",
            "5500005555555559",
            "378282246310005",
            "6011111111111117",
        ] {
            assert!(RegexPiiDetector::luhn_valid(s), "{s} should pass Luhn");
        }
    }

    #[tokio::test]
    async fn luhn_rejects_mutations() {
        assert!(!RegexPiiDetector::luhn_valid("4111111111111112"));
        assert!(!RegexPiiDetector::luhn_valid("1"));
        assert!(!RegexPiiDetector::luhn_valid(""));
    }

    #[tokio::test]
    async fn luhn_ignores_grouping_separators() {
        assert!(RegexPiiDetector::luhn_valid("4111 1111 1111 1111"));
        assert!(RegexPiiDetector::luhn_valid("4111-1111-1111-1111"));
    }

    #[tokio::test]
    async fn credit_card_true_positive_is_high_confidence() {
        // 4111111111111111 is the canonical Visa test number and IS valid Luhn.
        let entities = detect("card 4111111111111111 expires soon").await;
        assert_eq!(types(&entities), vec![ENTITY_CREDIT_CARD]);
        assert!((entities[0].confidence - 0.90).abs() < 1e-9);
    }

    #[tokio::test]
    async fn credit_card_grouped_form_detected() {
        let text = "card 4111 1111 1111 1111 ok";
        let entities = detect(text).await;
        assert_eq!(types(&entities), vec![ENTITY_CREDIT_CARD]);
        assert_eq!(entities[0].text, "4111 1111 1111 1111");
        assert_offsets_consistent(text, &entities);
    }

    #[tokio::test]
    async fn luhn_invalid_card_is_lower_confidence() {
        let entities = detect("card 4111111111111112").await;
        assert_eq!(types(&entities), vec![ENTITY_CREDIT_CARD]);
        assert!((entities[0].confidence - 0.60).abs() < 1e-9);
    }

    #[tokio::test]
    async fn digit_run_without_card_prefix_is_not_a_card() {
        // 16 digits, Luhn-valid by construction is not required — no IIN.
        let entities = detect("token 9999888877776666 issued").await;
        assert!(
            !types(&entities).contains(&ENTITY_CREDIT_CARD),
            "{entities:?}"
        );
    }

    // -- IP address ---------------------------------------------------------

    #[tokio::test]
    async fn ipv4_true_positives() {
        for s in ["192.168.1.1", "8.8.8.8", "255.255.255.255", "10.0.0.1"] {
            let entities = detect(s).await;
            assert_eq!(types(&entities), vec![ENTITY_IP_ADDRESS], "for {s:?}");
        }
    }

    #[tokio::test]
    async fn ipv4_octet_overflow_rejected() {
        for s in ["999.1.1.1", "192.168.1.256", "300.300.300.300"] {
            let entities = detect(s).await;
            assert!(
                !types(&entities).contains(&ENTITY_IP_ADDRESS),
                "unexpected IP in {s:?}: {entities:?}"
            );
        }
    }

    #[tokio::test]
    async fn version_strings_are_not_ip_addresses() {
        // Documented policy: 5+ components, leading-zero components, and a
        // `v` prefix all suppress the IP_ADDRESS label.
        for s in ["v1.2.3.4", "1.2.3.4.5", "1.02.3.4", "release 0.1.2.3.4"] {
            let entities = detect(s).await;
            assert!(
                !types(&entities).contains(&ENTITY_IP_ADDRESS),
                "unexpected IP in {s:?}: {entities:?}"
            );
        }
    }

    #[tokio::test]
    async fn bare_dotted_quad_is_reported_as_ip() {
        // Documented policy: "1.2.3.4" alone IS a valid IPv4 address and is
        // reported. Distinguishing it from a version needs surrounding
        // context this detector does not have.
        let entities = detect("1.2.3.4").await;
        assert_eq!(types(&entities), vec![ENTITY_IP_ADDRESS]);
    }

    // -- Overlap / precedence -----------------------------------------------

    #[tokio::test]
    async fn digits_inside_email_not_double_reported() {
        let text = "user4111111111111111@example.com";
        let entities = detect(text).await;
        assert_eq!(types(&entities), vec![ENTITY_EMAIL]);
        assert_offsets_consistent(text, &entities);
    }

    #[tokio::test]
    async fn ssn_wins_over_phone_shape() {
        let text = "078-05-1120";
        let entities = detect(text).await;
        assert_eq!(types(&entities), vec![ENTITY_SSN]);
    }

    // -- Clean text ---------------------------------------------------------

    #[tokio::test]
    async fn clean_prose_yields_nothing() {
        let entities =
            detect("The quick brown fox jumps over the lazy dog on a Tuesday afternoon.").await;
        assert!(entities.is_empty(), "{entities:?}");
    }

    #[tokio::test]
    async fn empty_input_yields_nothing() {
        assert!(detect("").await.is_empty());
    }

    // -- Through the real Signal --------------------------------------------

    #[tokio::test]
    async fn wired_into_pii_signal() {
        let mut deny = HashSet::new();
        deny.insert(ENTITY_EMAIL.to_string());
        deny.insert(ENTITY_CREDIT_CARD.to_string());

        let signal = PiiSignal::new(Box::new(RegexPiiDetector::new()), deny, 4096);
        let result = signal
            .evaluate(&make_ctx(
                "mail ada@example.com, card 4111111111111111, ip 192.168.1.1",
            ))
            .await
            .unwrap();

        assert_eq!(result.signal_type, SignalType::PII);
        assert!(result.labels.contains(&ENTITY_EMAIL.to_string()));
        assert!(result.labels.contains(&ENTITY_CREDIT_CARD.to_string()));
        // Not in the deny list, so filtered out.
        assert!(!result.labels.contains(&ENTITY_IP_ADDRESS.to_string()));
        assert!((result.confidence - 0.95).abs() < 1e-9);
    }

    #[tokio::test]
    async fn pii_signal_chunked_offsets_stay_absolute() {
        // Force chunking so PiiSignal exercises its offset arithmetic.
        let filler = "word ".repeat(20);
        let text = format!("{filler}contact ada@example.com now");
        let mut deny = HashSet::new();
        deny.insert(ENTITY_EMAIL.to_string());

        let signal = PiiSignal::new(Box::new(RegexPiiDetector::new()), deny, 32);
        let result = signal.evaluate(&make_ctx(&text)).await.unwrap();

        let entities = result.metadata["entities"].as_array().unwrap();
        assert_eq!(entities.len(), 1);
        let start = entities[0]["start"].as_u64().unwrap() as usize;
        let end = entities[0]["end"].as_u64().unwrap() as usize;
        assert_eq!(&text[start..end], "ada@example.com");
    }
}

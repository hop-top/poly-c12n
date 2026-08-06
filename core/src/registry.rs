//! Detector registry — construct tiered chains from configuration names.
//!
//! Trait objects cannot cross a C ABI, so non-Rust callers (Go, PHP, Python,
//! TypeScript) cannot hand a `Box<dyn PiiDetector>` to the engine. They name
//! detectors in configuration instead, and this module maps those names onto
//! the built-in implementations in [`crate::signals::detectors`].
//!
//! Rust callers are not restricted to the registry — they can build a
//! [`Chain`] from their own trait implementations directly. The registry
//! exists so that the other four bindings have *some* way to configure a
//! working pipeline.
//!
//! # Names are public API
//!
//! The strings accepted here (`"regex"`, `"heuristic"`, `"stopword"`,
//! `"approx"`) appear in user configuration files. Renaming one is a breaking
//! change for every binding; add an alias instead.

use std::sync::Arc;

use crate::chain::{Chain, ChainStrategy};
use crate::signals::detectors::{
    HeuristicJailbreakDetector, RegexPiiDetector, StopwordLanguageDetector,
};
use crate::signals::language::LanguageDetector;
use crate::signals::safety::{JailbreakDetector, PiiDetector};
use crate::types::SignalError;

/// Registered name of the regex-based PII detector.
pub const PII_REGEX: &str = "regex";
/// Registered name of the heuristic jailbreak detector.
pub const JAILBREAK_HEURISTIC: &str = "heuristic";
/// Registered name of the stopword-frequency language detector.
pub const LANGUAGE_STOPWORD: &str = "stopword";
/// Registered name of the approximate tokenizer.
pub const TOKENIZER_APPROX: &str = "approx";

/// Build one PII detector tier by registered name.
///
/// Returns [`SignalError::Configuration`] naming the unknown value and the
/// available alternatives — an unknown detector must fail loudly at build
/// time rather than silently yielding a pipeline that detects nothing.
pub fn pii_detector(name: &str) -> Result<Arc<dyn PiiDetector>, SignalError> {
    match name {
        PII_REGEX => Ok(Arc::new(RegexPiiDetector::new())),
        other => Err(unknown("pii", other, &[PII_REGEX])),
    }
}

/// Build one jailbreak detector tier by registered name.
pub fn jailbreak_detector(name: &str) -> Result<Arc<dyn JailbreakDetector>, SignalError> {
    match name {
        JAILBREAK_HEURISTIC => Ok(Arc::new(HeuristicJailbreakDetector::new())),
        other => Err(unknown("jailbreak", other, &[JAILBREAK_HEURISTIC])),
    }
}

/// Build one language detector tier by registered name.
pub fn language_detector(name: &str) -> Result<Arc<dyn LanguageDetector>, SignalError> {
    match name {
        LANGUAGE_STOPWORD => Ok(Arc::new(StopwordLanguageDetector::new())),
        other => Err(unknown("language", other, &[LANGUAGE_STOPWORD])),
    }
}

/// Build a PII chain from an ordered list of registered names.
///
/// `names` is tier order: `["regex", "nlp"]` tries regex first. `strategy` is
/// parsed by [`ChainStrategy::parse`] (`"escalate"`, `"escalate:0.8"`,
/// `"merge_all"`, `"fallback_on_error"`).
pub fn pii_chain(names: &[&str], strategy: &str) -> Result<Chain<dyn PiiDetector>, SignalError> {
    let tiers = names
        .iter()
        .map(|n| pii_detector(n))
        .collect::<Result<Vec<_>, _>>()?;
    Chain::new(tiers, ChainStrategy::parse(strategy)?)
}

/// Build a jailbreak chain from an ordered list of registered names.
pub fn jailbreak_chain(
    names: &[&str],
    strategy: &str,
) -> Result<Chain<dyn JailbreakDetector>, SignalError> {
    let tiers = names
        .iter()
        .map(|n| jailbreak_detector(n))
        .collect::<Result<Vec<_>, _>>()?;
    Chain::new(tiers, ChainStrategy::parse(strategy)?)
}

/// Build a language chain from an ordered list of registered names.
pub fn language_chain(
    names: &[&str],
    strategy: &str,
) -> Result<Chain<dyn LanguageDetector>, SignalError> {
    let tiers = names
        .iter()
        .map(|n| language_detector(n))
        .collect::<Result<Vec<_>, _>>()?;
    Chain::new(tiers, ChainStrategy::parse(strategy)?)
}

/// Names registered for each detector slot, for diagnostics and `--help`
/// style output in the bindings.
pub fn available(slot: &str) -> &'static [&'static str] {
    match slot {
        "pii" => &[PII_REGEX],
        "jailbreak" => &[JAILBREAK_HEURISTIC],
        "language" => &[LANGUAGE_STOPWORD],
        "tokenizer" => &[TOKENIZER_APPROX],
        _ => &[],
    }
}

fn unknown(slot: &str, got: &str, known: &[&str]) -> SignalError {
    SignalError::Configuration(format!(
        "unknown {slot} detector {got:?}; available: {}",
        known.join(", ")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signals::detectors::ApproxTokenizer as _Approx;

    #[tokio::test]
    async fn builds_registered_pii_detector() {
        let d = pii_detector(PII_REGEX).expect("regex detector is registered");
        let found = d
            .detect_entities("mail bob@example.com")
            .await
            .expect("detection succeeds");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].entity_type, "EMAIL");
    }

    #[test]
    fn unknown_name_fails_loudly_and_names_alternatives() {
        // `dyn PiiDetector` is not `Debug`, so `expect_err` is unavailable.
        let err = match pii_detector("rgex") {
            Ok(_) => panic!("typo must not silently succeed"),
            Err(e) => e,
        };
        let msg = err.to_string();
        assert!(
            msg.contains("rgex"),
            "error should quote the bad name: {msg}"
        );
        assert!(
            msg.contains(PII_REGEX),
            "error should list valid names: {msg}"
        );
        assert!(matches!(err, SignalError::Configuration(_)));
    }

    #[tokio::test]
    async fn single_tier_chain_detects_through_registry() {
        let chain = pii_chain(&[PII_REGEX], "escalate:0.8").expect("chain builds");
        let out = chain
            .detect_entities("card 4111111111111111")
            .await
            .expect("chain runs");
        assert_eq!(out.value.len(), 1);
        assert_eq!(out.value[0].entity_type, "CREDIT_CARD");
    }

    #[test]
    fn empty_chain_rejected() {
        let err = pii_chain(&[], "escalate").expect_err("a chain needs at least one tier");
        assert!(matches!(err, SignalError::Configuration(_)));
    }

    #[test]
    fn bad_strategy_rejected() {
        let err = pii_chain(&[PII_REGEX], "eskalate").expect_err("typo'd strategy must fail");
        assert!(matches!(err, SignalError::Configuration(_)));
    }

    #[test]
    fn available_lists_every_slot() {
        for slot in ["pii", "jailbreak", "language", "tokenizer"] {
            assert!(!available(slot).is_empty(), "{slot} should have a default");
        }
        assert!(available("nonexistent").is_empty());
    }

    #[test]
    fn tokenizer_default_is_registered() {
        // Registry exposes the name; ApproxTokenizer is constructed directly
        // by ContextSignal, which owns tokenizer wiring.
        assert_eq!(available("tokenizer"), &[TOKENIZER_APPROX]);
        let _ = _Approx::new();
    }
}

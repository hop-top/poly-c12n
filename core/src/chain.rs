//! Tiered detector chains.
//!
//! Every detector/engine trait in this crate can be backed by more than one
//! implementation, ordered into *tiers* (tier 0 first). A [`ChainStrategy`]
//! decides how the tiers combine: escalate to a slower/better tier only when
//! the cheap one is not confident, merge every tier's output, or advance only
//! when a tier errors.
//!
//! # Why this exists
//!
//! The canonical case is PII: a regex detector is free and catches the easy
//! 90%, an NLP model catches the rest, a local LLM is the last resort. Wiring
//! all three behind a single `Box<dyn PiiDetector>` would force a build-time
//! choice; a [`Chain`] defers it to configuration, per signal.
//!
//! # Tier provenance
//!
//! Every chain evaluation returns a [`ChainOutcome`] carrying which tiers ran,
//! which one produced the winning value, and any errors that were swallowed on
//! the way. Signals fold that record into `SignalResult::metadata` via
//! [`ChainProvenance::write_metadata`] so an operator can always answer "did
//! this result cost me an LLM call?".
//!
//! # Scalar traits
//!
//! `Tokenizer -> usize`, `EmbeddingEngine -> Vec<f32>` and
//! `PreferenceLlm -> String` have no confidence attached to their output, so
//! [`ChainStrategy::Escalate`] and [`ChainStrategy::MergeAll`] are meaningless
//! for them. That is encoded in the [`ChainableTier::SUPPORTS_CONFIDENCE`]
//! associated constant and enforced by [`Chain::new`], which returns
//! [`SignalError::Configuration`] rather than silently degrading. See the
//! module-level discussion in `docs/adr/0003-tiered-detector-chains.md`.

use std::collections::HashMap;
use std::sync::Arc;

use crate::types::SignalError;

// ---------------------------------------------------------------------------
// Strategy
// ---------------------------------------------------------------------------

/// Default confidence below which [`ChainStrategy::Escalate`] moves on.
pub const DEFAULT_ESCALATE_THRESHOLD: f64 = 0.5;

/// How a [`Chain`] combines the output of its tiers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChainStrategy {
    /// Run tier 0. If it errors, or returns an empty result, or returns a
    /// confidence strictly below `threshold`, try the next tier. The first
    /// tier that clears the bar wins. If no tier clears it, the best result
    /// seen so far is returned (falling back to the last tier's output).
    Escalate { threshold: f64 },

    /// Run every tier and union the results. Confidence is the maximum across
    /// tiers; duplicates are collapsed keeping the highest-confidence copy.
    /// Tier errors are recorded and skipped — the chain fails only when every
    /// tier errors.
    MergeAll,

    /// Run tier 0 and return its result, however unconfident. Advance to the
    /// next tier only when the current one returns `Err`.
    FallbackOnError,
}

impl Default for ChainStrategy {
    fn default() -> Self {
        Self::Escalate {
            threshold: DEFAULT_ESCALATE_THRESHOLD,
        }
    }
}

impl ChainStrategy {
    /// Stable identifier used in metadata and error messages.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Escalate { .. } => "escalate",
            Self::MergeAll => "merge_all",
            Self::FallbackOnError => "fallback_on_error",
        }
    }

    /// `true` when this strategy needs a confidence value to make a decision.
    fn requires_confidence(&self) -> bool {
        matches!(self, Self::Escalate { .. } | Self::MergeAll)
    }

    /// Parse a strategy out of a config string.
    ///
    /// Accepts `"escalate"` (default threshold), `"escalate:0.75"`,
    /// `"merge_all"`/`"merge"`, and `"fallback_on_error"`/`"fallback"`.
    pub fn parse(spec: &str) -> Result<Self, SignalError> {
        let spec = spec.trim();
        let (head, tail) = match spec.split_once(':') {
            Some((h, t)) => (h.trim(), Some(t.trim())),
            None => (spec, None),
        };

        match head {
            "escalate" => {
                let threshold = match tail {
                    Some(t) => t.parse::<f64>().map_err(|_| {
                        SignalError::Configuration(format!("invalid escalate threshold: {t}"))
                    })?,
                    None => DEFAULT_ESCALATE_THRESHOLD,
                };
                if !(0.0..=1.0).contains(&threshold) {
                    return Err(SignalError::Configuration(format!(
                        "escalate threshold must be within 0.0..=1.0, got {threshold}"
                    )));
                }
                Ok(Self::Escalate { threshold })
            }
            "merge_all" | "merge" => Ok(Self::MergeAll),
            "fallback_on_error" | "fallback" => Ok(Self::FallbackOnError),
            other => Err(SignalError::Configuration(format!(
                "unknown chain strategy: {other}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// Chainable marker
// ---------------------------------------------------------------------------

/// Marker implemented by every trait that a [`Chain`] can hold.
///
/// `SUPPORTS_CONFIDENCE` is the compile-time knob that separates traits whose
/// output carries a confidence (so `Escalate`/`MergeAll` are meaningful) from
/// scalar traits where only `FallbackOnError` makes sense. [`Chain::new`]
/// reads it and rejects the invalid combination at construction time.
pub trait ChainableTier {
    /// Human-readable trait name, used in error messages and metadata.
    const TIER_KIND: &'static str;
    /// Whether this trait's output carries a confidence signal.
    const SUPPORTS_CONFIDENCE: bool;
}

// ---------------------------------------------------------------------------
// Provenance
// ---------------------------------------------------------------------------

/// Record of what a chain evaluation actually did.
///
/// Attached to every [`ChainOutcome`] and folded into `SignalResult::metadata`
/// so a caller can tell which tier produced a result without instrumenting the
/// detectors themselves.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChainProvenance {
    /// Strategy identifier (`"escalate"`, `"merge_all"`, ...).
    pub strategy: String,
    /// Tier indices that were invoked, in call order.
    pub tiers_attempted: Vec<usize>,
    /// Tier index that produced the returned value. `None` under `MergeAll`
    /// (several tiers contribute) or when every tier failed.
    pub winning_tier: Option<usize>,
    /// `(tier_index, message)` for every tier that returned `Err` but did not
    /// abort the chain.
    pub tier_errors: Vec<(usize, String)>,
}

/// Metadata key under which [`ChainProvenance::write_metadata`] stores its
/// record. Stable — downstream bindings key off it.
pub const PROVENANCE_METADATA_KEY: &str = "chain";

impl ChainProvenance {
    fn new(strategy: &ChainStrategy) -> Self {
        Self {
            strategy: strategy.as_str().to_string(),
            ..Default::default()
        }
    }

    /// Number of tiers that were actually invoked.
    pub fn tier_count(&self) -> usize {
        self.tiers_attempted.len()
    }

    /// `true` when more than the first tier ran.
    pub fn escalated(&self) -> bool {
        self.tiers_attempted.len() > 1
    }

    /// Render as JSON for `SignalResult::metadata`.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "strategy": self.strategy,
            "tiers_attempted": self.tiers_attempted,
            "winning_tier": self.winning_tier,
            "escalated": self.escalated(),
            "tier_errors": self
                .tier_errors
                .iter()
                .map(|(i, msg)| serde_json::json!({ "tier": i, "error": msg }))
                .collect::<Vec<_>>(),
        })
    }

    /// Insert this record into a signal's metadata map under
    /// [`PROVENANCE_METADATA_KEY`].
    pub fn write_metadata(&self, metadata: &mut HashMap<String, serde_json::Value>) {
        metadata.insert(PROVENANCE_METADATA_KEY.to_string(), self.to_json());
    }
}

/// A chain's value plus the record of how it was produced.
#[derive(Debug, Clone)]
pub struct ChainOutcome<T> {
    pub value: T,
    pub provenance: ChainProvenance,
}

impl<T> ChainOutcome<T> {
    /// Map the carried value, preserving provenance.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> ChainOutcome<U> {
        ChainOutcome {
            value: f(self.value),
            provenance: self.provenance,
        }
    }
}

// ---------------------------------------------------------------------------
// Chain
// ---------------------------------------------------------------------------

/// An ordered list of implementations of a detector trait plus the strategy
/// that combines them.
///
/// `Chain` is generic over the (unsized) trait object type, so
/// `Chain<dyn PiiDetector>`, `Chain<dyn Tokenizer>` and friends all share this
/// one container. The per-trait combination logic lives in `impl` blocks in
/// this module because merging `Vec<PiiEntity>` and merging `Vec<(String,f64)>`
/// are genuinely different operations.
pub struct Chain<T: ?Sized> {
    tiers: Vec<Arc<T>>,
    strategy: ChainStrategy,
}

/// Hand-written because `T` is an unsized trait object and cannot be derived.
/// Prints shape only — the tier implementations are opaque.
impl<T: ?Sized> std::fmt::Debug for Chain<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Chain")
            .field("tiers", &self.tiers.len())
            .field("strategy", &self.strategy)
            .finish()
    }
}

impl<T: ?Sized> Clone for Chain<T> {
    fn clone(&self) -> Self {
        Self {
            tiers: self.tiers.clone(),
            strategy: self.strategy,
        }
    }
}

impl<T: ?Sized + ChainableTier> Chain<T> {
    /// Build a chain from ordered tiers.
    ///
    /// # Errors
    ///
    /// - [`SignalError::Configuration`] when `tiers` is empty.
    /// - [`SignalError::Configuration`] when `strategy` needs a confidence
    ///   value but `T::SUPPORTS_CONFIDENCE` is `false` — the scalar-trait
    ///   guard. This is deliberately loud rather than a silent downgrade to
    ///   `FallbackOnError`.
    pub fn new(tiers: Vec<Arc<T>>, strategy: ChainStrategy) -> Result<Self, SignalError> {
        if tiers.is_empty() {
            return Err(SignalError::Configuration(format!(
                "{} chain requires at least one tier",
                T::TIER_KIND
            )));
        }
        if strategy.requires_confidence() && !T::SUPPORTS_CONFIDENCE {
            return Err(SignalError::Configuration(format!(
                "{} produces no confidence value; strategy '{}' is not supported — use 'fallback_on_error'",
                T::TIER_KIND,
                strategy.as_str()
            )));
        }
        Ok(Self { tiers, strategy })
    }

    /// Single-implementation chain. Always valid: one tier makes every
    /// strategy degenerate to "call it and return what it says".
    ///
    /// This is the backward-compatibility path — existing
    /// `SomeSignal::new(detector)` constructors funnel through here.
    pub fn single(tier: Arc<T>) -> Self {
        Self {
            tiers: vec![tier],
            strategy: ChainStrategy::FallbackOnError,
        }
    }

    /// Build a chain from a config string (see [`ChainStrategy::parse`]).
    pub fn from_spec(tiers: Vec<Arc<T>>, spec: &str) -> Result<Self, SignalError> {
        Self::new(tiers, ChainStrategy::parse(spec)?)
    }

    /// Number of configured tiers.
    pub fn len(&self) -> usize {
        self.tiers.len()
    }

    /// `true` when the chain holds no tiers. Unreachable through the public
    /// constructors, which reject empty tier lists.
    pub fn is_empty(&self) -> bool {
        self.tiers.is_empty()
    }

    /// The configured strategy.
    pub fn strategy(&self) -> ChainStrategy {
        self.strategy
    }

    /// Tiers in call order.
    pub fn tiers(&self) -> &[Arc<T>] {
        &self.tiers
    }

    fn start(&self) -> ChainProvenance {
        ChainProvenance::new(&self.strategy)
    }

    /// Every tier errored.
    ///
    /// A single-tier chain is transparent: the sole error is returned with its
    /// original [`SignalError`] variant intact, so callers that match on the
    /// kind (`Configuration` vs `Inference` vs `Timeout`) keep working exactly
    /// as they did before chaining existed. Only a genuinely multi-tier
    /// failure collapses into one `Inference` carrying the full tally, because
    /// there is no single variant that honestly represents N different
    /// failures.
    fn all_failed(&self, prov: &ChainProvenance, raw: Vec<SignalError>) -> SignalError {
        if self.tiers.len() == 1 {
            if let Some(only) = raw.into_iter().next() {
                return only;
            }
        }
        let detail = prov
            .tier_errors
            .iter()
            .map(|(i, m)| format!("tier {i}: {m}"))
            .collect::<Vec<_>>()
            .join("; ");
        SignalError::Inference(format!("all {} tiers failed ({detail})", T::TIER_KIND))
    }
}

/// Does an escalating tier's result clear the bar?
fn accepts(confidence: f64, empty: bool, strategy: &ChainStrategy) -> bool {
    match strategy {
        ChainStrategy::Escalate { threshold } => !empty && confidence >= *threshold,
        // Both other strategies take whatever the tier returned.
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Dedup helpers
// ---------------------------------------------------------------------------

/// Collapse `(String, f64)` score pairs by key, keeping the maximum score.
/// Preserves first-seen ordering so tier 0's ordering wins ties.
pub(crate) fn merge_scored_labels(merged: &mut Vec<(String, f64)>, incoming: Vec<(String, f64)>) {
    for (label, score) in incoming {
        match merged.iter_mut().find(|(l, _)| *l == label) {
            Some(existing) => existing.1 = existing.1.max(score),
            None => merged.push((label, score)),
        }
    }
}

// ---------------------------------------------------------------------------
// Per-trait chain execution
// ---------------------------------------------------------------------------

use crate::embedding::{EmbeddingEngine, EmbeddingError};
use crate::signals::context::Tokenizer;
use crate::signals::domain::CategoryClassifier;
use crate::signals::feedback::SatisfactionDetector;
use crate::signals::language::{DetectedLanguage, LanguageDetector};
use crate::signals::preference::PreferenceLlm;
use crate::signals::safety::{JailbreakDetector, PiiDetector, PiiEntity, ToxicityDetector};

impl ChainableTier for dyn PiiDetector {
    const TIER_KIND: &'static str = "PiiDetector";
    const SUPPORTS_CONFIDENCE: bool = true;
}

impl ChainableTier for dyn JailbreakDetector {
    const TIER_KIND: &'static str = "JailbreakDetector";
    const SUPPORTS_CONFIDENCE: bool = true;
}

impl ChainableTier for dyn ToxicityDetector {
    const TIER_KIND: &'static str = "ToxicityDetector";
    const SUPPORTS_CONFIDENCE: bool = true;
}

impl ChainableTier for dyn LanguageDetector {
    const TIER_KIND: &'static str = "LanguageDetector";
    const SUPPORTS_CONFIDENCE: bool = true;
}

impl ChainableTier for dyn CategoryClassifier {
    const TIER_KIND: &'static str = "CategoryClassifier";
    const SUPPORTS_CONFIDENCE: bool = true;
}

impl ChainableTier for dyn SatisfactionDetector {
    const TIER_KIND: &'static str = "SatisfactionDetector";
    const SUPPORTS_CONFIDENCE: bool = true;
}

impl ChainableTier for dyn Tokenizer {
    const TIER_KIND: &'static str = "Tokenizer";
    const SUPPORTS_CONFIDENCE: bool = false;
}

impl ChainableTier for dyn EmbeddingEngine {
    const TIER_KIND: &'static str = "EmbeddingEngine";
    const SUPPORTS_CONFIDENCE: bool = false;
}

impl ChainableTier for dyn PreferenceLlm {
    const TIER_KIND: &'static str = "PreferenceLlm";
    const SUPPORTS_CONFIDENCE: bool = false;
}

// -- PiiDetector ------------------------------------------------------------

impl Chain<dyn PiiDetector> {
    /// Run the chain's `detect_entities` across tiers.
    ///
    /// Under `MergeAll`, entities are deduplicated on
    /// `(entity_type, start, end)`, keeping the highest-confidence copy.
    pub async fn detect_entities(
        &self,
        text: &str,
    ) -> Result<ChainOutcome<Vec<PiiEntity>>, SignalError> {
        let mut prov = self.start();
        let mut raw_errors: Vec<SignalError> = Vec::new();
        let mut merged: Vec<PiiEntity> = Vec::new();
        let mut best: Option<(usize, Vec<PiiEntity>)> = None;
        let mut best_conf = f64::NEG_INFINITY;

        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            let entities = match tier.detect_entities(text).await {
                Ok(v) => v,
                Err(e) => {
                    // Escalate records the error and moves on rather than
                    // failing: a broken cheap tier should not deny the caller
                    // the answer an expensive tier can still give.
                    prov.tier_errors.push((idx, e.to_string()));
                    raw_errors.push(e);
                    continue;
                }
            };

            let confidence = entities
                .iter()
                .map(|e| e.confidence)
                .fold(0.0_f64, f64::max);
            let empty = entities.is_empty();

            match self.strategy {
                ChainStrategy::MergeAll => {
                    for entity in entities {
                        match merged.iter_mut().find(|m| {
                            m.entity_type == entity.entity_type
                                && m.start == entity.start
                                && m.end == entity.end
                        }) {
                            Some(existing) => {
                                if entity.confidence > existing.confidence {
                                    existing.confidence = entity.confidence;
                                }
                            }
                            None => merged.push(entity),
                        }
                    }
                }
                _ => {
                    if accepts(confidence, empty, &self.strategy) {
                        prov.winning_tier = Some(idx);
                        return Ok(ChainOutcome {
                            value: entities,
                            provenance: prov,
                        });
                    }
                    if confidence > best_conf {
                        best_conf = confidence;
                        best = Some((idx, entities));
                    }
                }
            }
        }

        if self.strategy == ChainStrategy::MergeAll {
            if prov.tier_errors.len() == self.tiers.len() {
                return Err(self.all_failed(&prov, raw_errors));
            }
            return Ok(ChainOutcome {
                value: merged,
                provenance: prov,
            });
        }

        match best {
            // Nothing cleared the threshold: return the best-scoring tier's
            // output rather than an error. Escalation is a quality knob, not
            // a hard gate.
            Some((idx, entities)) => {
                prov.winning_tier = Some(idx);
                Ok(ChainOutcome {
                    value: entities,
                    provenance: prov,
                })
            }
            None => Err(self.all_failed(&prov, raw_errors)),
        }
    }
}

// -- JailbreakDetector ------------------------------------------------------

impl Chain<dyn JailbreakDetector> {
    /// Run the chain's `detect` across tiers. Under `MergeAll`, labels are
    /// deduplicated and confidence is the max across tiers.
    pub async fn detect(
        &self,
        text: &str,
    ) -> Result<ChainOutcome<(f64, Vec<String>)>, SignalError> {
        let mut prov = self.start();
        let mut raw_errors: Vec<SignalError> = Vec::new();
        let mut merged_labels: Vec<String> = Vec::new();
        let mut merged_conf = 0.0_f64;
        let mut best: Option<(usize, (f64, Vec<String>))> = None;
        let mut best_conf = f64::NEG_INFINITY;

        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            let (confidence, labels) = match tier.detect(text).await {
                Ok(v) => v,
                Err(e) => {
                    prov.tier_errors.push((idx, e.to_string()));
                    raw_errors.push(e);
                    continue;
                }
            };

            match self.strategy {
                ChainStrategy::MergeAll => {
                    merged_conf = merged_conf.max(confidence);
                    for label in labels {
                        if !merged_labels.contains(&label) {
                            merged_labels.push(label);
                        }
                    }
                }
                _ => {
                    if accepts(confidence, labels.is_empty(), &self.strategy) {
                        prov.winning_tier = Some(idx);
                        return Ok(ChainOutcome {
                            value: (confidence, labels),
                            provenance: prov,
                        });
                    }
                    if confidence > best_conf {
                        best_conf = confidence;
                        best = Some((idx, (confidence, labels)));
                    }
                }
            }
        }

        if self.strategy == ChainStrategy::MergeAll {
            if prov.tier_errors.len() == self.tiers.len() {
                return Err(self.all_failed(&prov, raw_errors));
            }
            return Ok(ChainOutcome {
                value: (merged_conf, merged_labels),
                provenance: prov,
            });
        }

        match best {
            Some((idx, value)) => {
                prov.winning_tier = Some(idx);
                Ok(ChainOutcome {
                    value,
                    provenance: prov,
                })
            }
            None => Err(self.all_failed(&prov, raw_errors)),
        }
    }
}

// -- ToxicityDetector -------------------------------------------------------

impl Chain<dyn ToxicityDetector> {
    /// Run the chain's `detect` across tiers. Under `MergeAll`, categories are
    /// deduplicated on the category name keeping the maximum score.
    pub async fn detect(
        &self,
        text: &str,
    ) -> Result<ChainOutcome<Vec<(String, f64)>>, SignalError> {
        let mut prov = self.start();
        let mut raw_errors: Vec<SignalError> = Vec::new();
        let mut merged: Vec<(String, f64)> = Vec::new();
        let mut best: Option<(usize, Vec<(String, f64)>)> = None;
        let mut best_conf = f64::NEG_INFINITY;

        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            let scores = match tier.detect(text).await {
                Ok(v) => v,
                Err(e) => {
                    prov.tier_errors.push((idx, e.to_string()));
                    raw_errors.push(e);
                    continue;
                }
            };

            let confidence = scores.iter().map(|(_, s)| *s).fold(0.0_f64, f64::max);

            match self.strategy {
                ChainStrategy::MergeAll => merge_scored_labels(&mut merged, scores),
                _ => {
                    if accepts(confidence, scores.is_empty(), &self.strategy) {
                        prov.winning_tier = Some(idx);
                        return Ok(ChainOutcome {
                            value: scores,
                            provenance: prov,
                        });
                    }
                    if confidence > best_conf {
                        best_conf = confidence;
                        best = Some((idx, scores));
                    }
                }
            }
        }

        if self.strategy == ChainStrategy::MergeAll {
            if prov.tier_errors.len() == self.tiers.len() {
                return Err(self.all_failed(&prov, raw_errors));
            }
            return Ok(ChainOutcome {
                value: merged,
                provenance: prov,
            });
        }

        match best {
            Some((idx, value)) => {
                prov.winning_tier = Some(idx);
                Ok(ChainOutcome {
                    value,
                    provenance: prov,
                })
            }
            None => Err(self.all_failed(&prov, raw_errors)),
        }
    }
}

// -- CategoryClassifier -----------------------------------------------------

impl Chain<dyn CategoryClassifier> {
    /// Run the chain's `classify` across tiers. Under `MergeAll`, categories
    /// are deduplicated on the category name keeping the maximum probability.
    pub async fn classify(
        &self,
        text: &str,
    ) -> Result<ChainOutcome<Vec<(String, f64)>>, SignalError> {
        let mut prov = self.start();
        let mut raw_errors: Vec<SignalError> = Vec::new();
        let mut merged: Vec<(String, f64)> = Vec::new();
        let mut best: Option<(usize, Vec<(String, f64)>)> = None;
        let mut best_conf = f64::NEG_INFINITY;

        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            let dist = match tier.classify(text).await {
                Ok(v) => v,
                Err(e) => {
                    prov.tier_errors.push((idx, e.to_string()));
                    raw_errors.push(e);
                    continue;
                }
            };

            let confidence = dist.iter().map(|(_, p)| *p).fold(0.0_f64, f64::max);

            match self.strategy {
                ChainStrategy::MergeAll => merge_scored_labels(&mut merged, dist),
                _ => {
                    if accepts(confidence, dist.is_empty(), &self.strategy) {
                        prov.winning_tier = Some(idx);
                        return Ok(ChainOutcome {
                            value: dist,
                            provenance: prov,
                        });
                    }
                    if confidence > best_conf {
                        best_conf = confidence;
                        best = Some((idx, dist));
                    }
                }
            }
        }

        if self.strategy == ChainStrategy::MergeAll {
            if prov.tier_errors.len() == self.tiers.len() {
                return Err(self.all_failed(&prov, raw_errors));
            }
            // Renormalize so downstream entropy maths still sees a
            // distribution after unioning tiers.
            let total: f64 = merged.iter().map(|(_, p)| *p).sum();
            if total > 0.0 {
                for (_, p) in merged.iter_mut() {
                    *p /= total;
                }
            }
            merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            return Ok(ChainOutcome {
                value: merged,
                provenance: prov,
            });
        }

        match best {
            Some((idx, value)) => {
                prov.winning_tier = Some(idx);
                Ok(ChainOutcome {
                    value,
                    provenance: prov,
                })
            }
            None => Err(self.all_failed(&prov, raw_errors)),
        }
    }
}

// -- LanguageDetector -------------------------------------------------------

impl Chain<dyn LanguageDetector> {
    /// Run the chain's `detect`/`detect_multiple` across tiers.
    ///
    /// `LanguageDetector` is infallible (it returns `Option`/`Vec`, not
    /// `Result`), so `FallbackOnError` degenerates to "tier 0 only" and no
    /// tier error can ever be recorded. `Escalate` treats `None` / a
    /// below-threshold confidence as the escalation trigger. Under `MergeAll`
    /// languages are deduplicated on the language code keeping the highest
    /// confidence.
    pub fn detect(
        &self,
        text: &str,
    ) -> ChainOutcome<(Option<DetectedLanguage>, Vec<DetectedLanguage>)> {
        let mut prov = self.start();
        let mut merged: Vec<DetectedLanguage> = Vec::new();
        let mut best: Option<(usize, Option<DetectedLanguage>, Vec<DetectedLanguage>)> = None;
        let mut best_conf = f64::NEG_INFINITY;

        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            let primary = tier.detect(text);
            let all = tier.detect_multiple(text);
            let confidence = primary.as_ref().map(|l| l.confidence).unwrap_or(0.0);

            match self.strategy {
                ChainStrategy::MergeAll => {
                    for lang in all {
                        match merged.iter_mut().find(|m| m.code == lang.code) {
                            Some(existing) => {
                                if lang.confidence > existing.confidence {
                                    existing.confidence = lang.confidence;
                                }
                            }
                            None => merged.push(lang),
                        }
                    }
                }
                _ => {
                    if accepts(confidence, primary.is_none(), &self.strategy) {
                        prov.winning_tier = Some(idx);
                        return ChainOutcome {
                            value: (primary, all),
                            provenance: prov,
                        };
                    }
                    if confidence > best_conf {
                        best_conf = confidence;
                        best = Some((idx, primary, all));
                    }
                }
            }
        }

        if self.strategy == ChainStrategy::MergeAll {
            merged.sort_by(|a, b| {
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            let primary = merged.first().cloned();
            return ChainOutcome {
                value: (primary, merged),
                provenance: prov,
            };
        }

        match best {
            Some((idx, primary, all)) => {
                prov.winning_tier = Some(idx);
                ChainOutcome {
                    value: (primary, all),
                    provenance: prov,
                }
            }
            // Unreachable: chains always hold >= 1 tier, and the loop above
            // always populates `best` for the non-merge strategies.
            None => ChainOutcome {
                value: (None, Vec::new()),
                provenance: prov,
            },
        }
    }
}

// -- SatisfactionDetector ---------------------------------------------------

impl Chain<dyn SatisfactionDetector> {
    /// Run the chain's `score` across tiers.
    ///
    /// The returned score doubles as the confidence, so `Escalate` reads it
    /// directly. Under `MergeAll` the maximum score across tiers wins.
    pub async fn score(&self, text: &str) -> Result<ChainOutcome<f64>, SignalError> {
        let mut prov = self.start();
        let mut raw_errors: Vec<SignalError> = Vec::new();
        let mut merged: Option<f64> = None;
        let mut best: Option<(usize, f64)> = None;
        let mut best_conf = f64::NEG_INFINITY;

        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            let score = match tier.score(text).await {
                Ok(v) => v,
                Err(e) => {
                    prov.tier_errors.push((idx, e.to_string()));
                    raw_errors.push(e);
                    continue;
                }
            };

            match self.strategy {
                ChainStrategy::MergeAll => {
                    merged = Some(merged.map_or(score, |m: f64| m.max(score)));
                }
                _ => {
                    if accepts(score, false, &self.strategy) {
                        prov.winning_tier = Some(idx);
                        return Ok(ChainOutcome {
                            value: score,
                            provenance: prov,
                        });
                    }
                    if score > best_conf {
                        best_conf = score;
                        best = Some((idx, score));
                    }
                }
            }
        }

        if self.strategy == ChainStrategy::MergeAll {
            return match merged {
                Some(value) => Ok(ChainOutcome {
                    value,
                    provenance: prov,
                }),
                None => Err(self.all_failed(&prov, raw_errors)),
            };
        }

        match best {
            Some((idx, value)) => {
                prov.winning_tier = Some(idx);
                Ok(ChainOutcome {
                    value,
                    provenance: prov,
                })
            }
            None => Err(self.all_failed(&prov, raw_errors)),
        }
    }
}

// -- Tokenizer (scalar: FallbackOnError only) -------------------------------

impl Chain<dyn Tokenizer> {
    /// Count tokens using the first tier that does not panic. `Tokenizer` is
    /// infallible, so this always resolves on tier 0 — the chain exists for
    /// symmetry and for future fallible tokenizers.
    pub fn count_tokens(&self, text: &str) -> ChainOutcome<usize> {
        let mut prov = self.start();
        prov.tiers_attempted.push(0);
        prov.winning_tier = Some(0);
        ChainOutcome {
            value: self.tiers[0].count_tokens(text),
            provenance: prov,
        }
    }

    /// Model name of the primary tier.
    pub fn model_name(&self) -> &str {
        self.tiers[0].model_name()
    }
}

// -- EmbeddingEngine (scalar: FallbackOnError only) -------------------------

impl Chain<dyn EmbeddingEngine> {
    /// Embed `text`, advancing to the next tier when a tier errors.
    pub async fn embed(&self, text: &str) -> Result<ChainOutcome<Vec<f32>>, SignalError> {
        let mut prov = self.start();
        let mut raw_errors: Vec<SignalError> = Vec::new();
        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            match tier.embed(text).await {
                Ok(value) => {
                    prov.winning_tier = Some(idx);
                    return Ok(ChainOutcome {
                        value,
                        provenance: prov,
                    });
                }
                Err(e) => {
                    prov.tier_errors.push((idx, e.to_string()));
                    raw_errors.push(e.into());
                }
            }
        }
        Err(self.all_failed(&prov, raw_errors))
    }

    /// Batch-embed `texts`, advancing to the next tier when a tier errors.
    pub async fn embed_batch(
        &self,
        texts: &[&str],
    ) -> Result<ChainOutcome<Vec<Vec<f32>>>, SignalError> {
        let mut prov = self.start();
        let mut raw_errors: Vec<SignalError> = Vec::new();
        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            match tier.embed_batch(texts).await {
                Ok(value) => {
                    prov.winning_tier = Some(idx);
                    return Ok(ChainOutcome {
                        value,
                        provenance: prov,
                    });
                }
                Err(e) => {
                    prov.tier_errors.push((idx, e.to_string()));
                    raw_errors.push(e.into());
                }
            }
        }
        Err(self.all_failed(&prov, raw_errors))
    }

    /// Dimension of the primary tier. Tiers in one chain must agree on
    /// dimensionality — a failover that silently changes vector width would
    /// corrupt every prototype-bank comparison downstream.
    pub fn dimension(&self) -> usize {
        self.tiers[0].dimension()
    }

    /// Verify every tier reports the same dimension as tier 0.
    pub fn validate_dimensions(&self) -> Result<(), SignalError> {
        let expected = self.tiers[0].dimension();
        for (idx, tier) in self.tiers.iter().enumerate().skip(1) {
            if tier.dimension() != expected {
                return Err(SignalError::Configuration(format!(
                    "EmbeddingEngine tier {idx} has dimension {}, expected {expected}",
                    tier.dimension()
                )));
            }
        }
        Ok(())
    }
}

/// Adapter so `Chain<dyn EmbeddingEngine>` can stand in wherever a bare
/// `Arc<dyn EmbeddingEngine>` was expected. Provenance is dropped on this
/// path; call [`Chain::embed`] directly when you need it.
#[async_trait::async_trait]
impl EmbeddingEngine for Chain<dyn EmbeddingEngine> {
    async fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        Chain::embed(self, text)
            .await
            .map(|o| o.value)
            .map_err(|e| EmbeddingError::Inference(e.to_string()))
    }

    async fn embed_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        Chain::embed_batch(self, texts)
            .await
            .map(|o| o.value)
            .map_err(|e| EmbeddingError::Inference(e.to_string()))
    }

    fn dimension(&self) -> usize {
        Chain::dimension(self)
    }
}

// -- PreferenceLlm (scalar: FallbackOnError only) ---------------------------

impl Chain<dyn PreferenceLlm> {
    /// Query the LLM, advancing to the next tier when a tier errors.
    pub async fn query(
        &self,
        prompt: &str,
        system: &str,
    ) -> Result<ChainOutcome<String>, SignalError> {
        let mut prov = self.start();
        let mut raw_errors: Vec<SignalError> = Vec::new();
        for (idx, tier) in self.tiers.iter().enumerate() {
            prov.tiers_attempted.push(idx);
            match tier.query(prompt, system).await {
                Ok(value) => {
                    prov.winning_tier = Some(idx);
                    return Ok(ChainOutcome {
                        value,
                        provenance: prov,
                    });
                }
                Err(e) => {
                    prov.tier_errors.push((idx, e.to_string()));
                    raw_errors.push(e);
                }
            }
        }
        Err(self.all_failed(&prov, raw_errors))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    // -- Test doubles -------------------------------------------------------

    /// PII tier returning a fixed entity set, or an error.
    struct StubPii {
        entities: Vec<PiiEntity>,
        fail: bool,
    }

    impl StubPii {
        fn ok(entities: Vec<PiiEntity>) -> Arc<dyn PiiDetector> {
            Arc::new(Self {
                entities,
                fail: false,
            })
        }
        fn err() -> Arc<dyn PiiDetector> {
            Arc::new(Self {
                entities: vec![],
                fail: true,
            })
        }
    }

    #[async_trait]
    impl PiiDetector for StubPii {
        async fn detect_entities(&self, _text: &str) -> Result<Vec<PiiEntity>, SignalError> {
            if self.fail {
                return Err(SignalError::Inference("tier down".into()));
            }
            Ok(self.entities.clone())
        }
    }

    fn entity(kind: &str, start: usize, end: usize, confidence: f64) -> PiiEntity {
        PiiEntity {
            entity_type: kind.into(),
            text: "x".into(),
            start,
            end,
            confidence,
        }
    }

    /// Toxicity tier returning fixed category scores, or an error.
    struct StubToxicity {
        scores: Vec<(String, f64)>,
        fail: bool,
    }

    #[async_trait]
    impl ToxicityDetector for StubToxicity {
        async fn detect(&self, _text: &str) -> Result<Vec<(String, f64)>, SignalError> {
            if self.fail {
                return Err(SignalError::Inference("tier down".into()));
            }
            Ok(self.scores.clone())
        }
    }

    fn toxicity(scores: &[(&str, f64)]) -> Arc<dyn ToxicityDetector> {
        Arc::new(StubToxicity {
            scores: scores.iter().map(|(c, s)| ((*c).into(), *s)).collect(),
            fail: false,
        })
    }

    fn toxicity_err() -> Arc<dyn ToxicityDetector> {
        Arc::new(StubToxicity {
            scores: vec![],
            fail: true,
        })
    }

    /// Tokenizer tier — scalar trait, used for the strategy-guard tests.
    struct StubTokenizer(usize);

    impl Tokenizer for StubTokenizer {
        fn count_tokens(&self, _text: &str) -> usize {
            self.0
        }
        fn model_name(&self) -> &str {
            "stub"
        }
    }

    /// PreferenceLlm tier — scalar, fails on demand to exercise failover.
    struct StubLlm {
        reply: &'static str,
        fail: bool,
    }

    #[async_trait]
    impl PreferenceLlm for StubLlm {
        async fn query(&self, _prompt: &str, _system: &str) -> Result<String, SignalError> {
            if self.fail {
                return Err(SignalError::Inference("provider down".into()));
            }
            Ok(self.reply.to_string())
        }
    }

    // -- Strategy parsing ---------------------------------------------------

    #[test]
    fn parses_strategy_specs() {
        assert_eq!(
            ChainStrategy::parse("escalate").unwrap(),
            ChainStrategy::Escalate {
                threshold: DEFAULT_ESCALATE_THRESHOLD
            }
        );
        assert_eq!(
            ChainStrategy::parse("escalate:0.75").unwrap(),
            ChainStrategy::Escalate { threshold: 0.75 }
        );
        assert_eq!(
            ChainStrategy::parse("merge_all").unwrap(),
            ChainStrategy::MergeAll
        );
        assert_eq!(
            ChainStrategy::parse("fallback").unwrap(),
            ChainStrategy::FallbackOnError
        );
    }

    #[test]
    fn rejects_unknown_and_out_of_range_specs() {
        assert!(matches!(
            ChainStrategy::parse("nonsense"),
            Err(SignalError::Configuration(_))
        ));
        assert!(matches!(
            ChainStrategy::parse("escalate:9.0"),
            Err(SignalError::Configuration(_))
        ));
        assert!(matches!(
            ChainStrategy::parse("escalate:abc"),
            Err(SignalError::Configuration(_))
        ));
    }

    #[test]
    fn default_strategy_is_escalate() {
        assert_eq!(
            ChainStrategy::default(),
            ChainStrategy::Escalate {
                threshold: DEFAULT_ESCALATE_THRESHOLD
            }
        );
    }

    // -- Construction guards ------------------------------------------------

    #[test]
    fn rejects_empty_tier_list() {
        let err = Chain::<dyn PiiDetector>::new(vec![], ChainStrategy::MergeAll).unwrap_err();
        assert!(matches!(err, SignalError::Configuration(_)));
    }

    /// The scalar-trait guard: `Tokenizer` has no confidence, so `Escalate`
    /// and `MergeAll` must be rejected loudly rather than silently downgraded.
    #[test]
    fn scalar_trait_rejects_confidence_strategies() {
        let tiers: Vec<Arc<dyn Tokenizer>> = vec![Arc::new(StubTokenizer(10))];

        for strategy in [
            ChainStrategy::Escalate { threshold: 0.5 },
            ChainStrategy::MergeAll,
        ] {
            let err = Chain::<dyn Tokenizer>::new(tiers.clone(), strategy).unwrap_err();
            match err {
                SignalError::Configuration(msg) => {
                    assert!(msg.contains("Tokenizer"), "message names the trait: {msg}");
                    assert!(
                        msg.contains("fallback_on_error"),
                        "message names the remedy: {msg}"
                    );
                }
                other => panic!("expected Configuration, got {other:?}"),
            }
        }
    }

    #[test]
    fn scalar_trait_accepts_fallback_on_error() {
        let tiers: Vec<Arc<dyn Tokenizer>> = vec![Arc::new(StubTokenizer(10))];
        assert!(Chain::<dyn Tokenizer>::new(tiers, ChainStrategy::FallbackOnError).is_ok());
    }

    /// The same guard must not fire on traits that do carry a confidence.
    #[test]
    fn confidence_trait_accepts_every_strategy() {
        let tiers: Vec<Arc<dyn PiiDetector>> = vec![StubPii::ok(vec![])];
        for strategy in [
            ChainStrategy::Escalate { threshold: 0.5 },
            ChainStrategy::MergeAll,
            ChainStrategy::FallbackOnError,
        ] {
            assert!(Chain::<dyn PiiDetector>::new(tiers.clone(), strategy).is_ok());
        }
    }

    // -- Escalate -----------------------------------------------------------

    /// Tier 0 is confident: tier 1 must never be consulted.
    #[tokio::test]
    async fn escalate_stops_at_confident_first_tier() {
        let chain = Chain::new(
            vec![
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.95)]),
                StubPii::err(), // would blow up if reached
            ],
            ChainStrategy::Escalate { threshold: 0.5 },
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.value.len(), 1);
        assert_eq!(outcome.provenance.winning_tier, Some(0));
        assert_eq!(outcome.provenance.tiers_attempted, vec![0]);
        assert!(!outcome.provenance.escalated());
    }

    /// Tier 0 is below threshold: tier 1 runs and wins.
    #[tokio::test]
    async fn escalate_advances_on_low_confidence() {
        let chain = Chain::new(
            vec![
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.20)]),
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.90)]),
            ],
            ChainStrategy::Escalate { threshold: 0.5 },
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.value[0].confidence, 0.90);
        assert_eq!(outcome.provenance.winning_tier, Some(1));
        assert_eq!(outcome.provenance.tiers_attempted, vec![0, 1]);
        assert!(outcome.provenance.escalated());
    }

    /// An empty result escalates even when no confidence was reported.
    #[tokio::test]
    async fn escalate_advances_on_empty_result() {
        let chain = Chain::new(
            vec![
                StubPii::ok(vec![]),
                StubPii::ok(vec![entity("SSN", 3, 12, 0.99)]),
            ],
            ChainStrategy::Escalate { threshold: 0.5 },
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.value.len(), 1);
        assert_eq!(outcome.provenance.winning_tier, Some(1));
    }

    /// Error policy under Escalate: a failing tier is recorded and skipped,
    /// not fatal. A broken cheap tier must not deny the caller the answer an
    /// expensive tier can still produce.
    #[tokio::test]
    async fn escalate_skips_failing_tier_and_records_it() {
        let chain = Chain::new(
            vec![
                StubPii::err(),
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.9)]),
            ],
            ChainStrategy::Escalate { threshold: 0.5 },
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.provenance.winning_tier, Some(1));
        assert_eq!(outcome.provenance.tier_errors.len(), 1);
        assert_eq!(outcome.provenance.tier_errors[0].0, 0);
    }

    /// No tier clears the threshold: return the best result seen rather than
    /// erroring. Escalation is a quality knob, not a hard gate.
    #[tokio::test]
    async fn escalate_falls_back_to_best_when_none_confident() {
        let chain = Chain::new(
            vec![
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.10)]),
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.40)]),
            ],
            ChainStrategy::Escalate { threshold: 0.9 },
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.value[0].confidence, 0.40);
        assert_eq!(outcome.provenance.winning_tier, Some(1));
        assert_eq!(outcome.provenance.tiers_attempted, vec![0, 1]);
    }

    /// Every tier errored under Escalate: only then does the chain fail.
    #[tokio::test]
    async fn escalate_errors_when_every_tier_fails() {
        let chain = Chain::new(
            vec![StubPii::err(), StubPii::err()],
            ChainStrategy::Escalate { threshold: 0.5 },
        )
        .unwrap();

        let err = chain.detect_entities("text").await.unwrap_err();
        match err {
            SignalError::Inference(msg) => {
                assert!(msg.contains("all PiiDetector tiers failed"), "{msg}");
                assert!(msg.contains("tier 0"), "{msg}");
                assert!(msg.contains("tier 1"), "{msg}");
            }
            other => panic!("expected Inference, got {other:?}"),
        }
    }

    // -- MergeAll -----------------------------------------------------------

    /// PII dedup key is `(entity_type, start, end)`; the highest-confidence
    /// copy survives and distinct spans are all retained.
    #[tokio::test]
    async fn merge_all_dedups_pii_on_type_and_span() {
        let chain = Chain::new(
            vec![
                StubPii::ok(vec![
                    entity("EMAIL", 0, 7, 0.60),
                    entity("NAME", 10, 15, 0.50),
                ]),
                StubPii::ok(vec![
                    entity("EMAIL", 0, 7, 0.95),   // same span -> dedup, max wins
                    entity("EMAIL", 20, 27, 0.80), // different span -> kept
                ]),
            ],
            ChainStrategy::MergeAll,
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.value.len(), 3, "one duplicate collapsed");

        let email_0 = outcome
            .value
            .iter()
            .find(|e| e.entity_type == "EMAIL" && e.start == 0)
            .unwrap();
        assert_eq!(email_0.confidence, 0.95, "max confidence retained");

        assert!(outcome.value.iter().any(|e| e.entity_type == "NAME"));
        assert!(outcome
            .value
            .iter()
            .any(|e| e.entity_type == "EMAIL" && e.start == 20));

        // Every tier ran; no single tier "won".
        assert_eq!(outcome.provenance.tiers_attempted, vec![0, 1]);
        assert_eq!(outcome.provenance.winning_tier, None);
    }

    /// `Vec<(String, f64)>` traits dedup on the String, keeping the max score.
    #[tokio::test]
    async fn merge_all_dedups_scored_labels_keeping_max() {
        let chain = Chain::new(
            vec![
                toxicity(&[("hate", 0.30), ("violence", 0.10)]),
                toxicity(&[("hate", 0.85), ("harassment", 0.40)]),
            ],
            ChainStrategy::MergeAll,
        )
        .unwrap();

        let outcome = chain.detect(":(").await.unwrap();
        assert_eq!(outcome.value.len(), 3);

        let hate = outcome.value.iter().find(|(c, _)| c == "hate").unwrap();
        assert_eq!(hate.1, 0.85, "max score wins on duplicate key");
        assert!(outcome.value.iter().any(|(c, _)| c == "violence"));
        assert!(outcome.value.iter().any(|(c, _)| c == "harassment"));
    }

    /// Error policy under MergeAll: one failing tier does not fail the chain;
    /// the surviving tiers still merge, and the error is recorded.
    #[tokio::test]
    async fn merge_all_tolerates_partial_tier_failure() {
        let chain = Chain::new(
            vec![toxicity_err(), toxicity(&[("hate", 0.7)])],
            ChainStrategy::MergeAll,
        )
        .unwrap();

        let outcome = chain.detect(":(").await.unwrap();
        assert_eq!(outcome.value.len(), 1);
        assert_eq!(outcome.value[0].1, 0.7);
        assert_eq!(outcome.provenance.tier_errors.len(), 1);
        assert_eq!(outcome.provenance.tier_errors[0].0, 0);
    }

    /// ...but a chain where every tier errors does fail.
    #[tokio::test]
    async fn merge_all_fails_when_every_tier_fails() {
        let chain = Chain::new(
            vec![toxicity_err(), toxicity_err()],
            ChainStrategy::MergeAll,
        )
        .unwrap();
        assert!(chain.detect(":(").await.is_err());
    }

    // -- FallbackOnError ----------------------------------------------------

    /// No confidence-based escalation: an unconfident tier-0 result is
    /// returned as-is, tier 1 untouched.
    #[tokio::test]
    async fn fallback_keeps_unconfident_first_tier() {
        let chain = Chain::new(
            vec![
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.01)]),
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.99)]),
            ],
            ChainStrategy::FallbackOnError,
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.value[0].confidence, 0.01);
        assert_eq!(outcome.provenance.winning_tier, Some(0));
        assert_eq!(outcome.provenance.tiers_attempted, vec![0]);
    }

    /// An empty result is not an error, so it does not trigger failover.
    #[tokio::test]
    async fn fallback_keeps_empty_first_tier_result() {
        let chain = Chain::new(
            vec![
                StubPii::ok(vec![]),
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.99)]),
            ],
            ChainStrategy::FallbackOnError,
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert!(outcome.value.is_empty());
        assert_eq!(outcome.provenance.winning_tier, Some(0));
    }

    /// Only an `Err` advances the chain.
    #[tokio::test]
    async fn fallback_advances_only_on_error() {
        let chain = Chain::new(
            vec![
                StubPii::err(),
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.4)]),
            ],
            ChainStrategy::FallbackOnError,
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.value[0].confidence, 0.4);
        assert_eq!(outcome.provenance.winning_tier, Some(1));
        assert_eq!(outcome.provenance.tier_errors.len(), 1);
    }

    /// Scalar-trait failover: the LLM chain moves to the healthy provider.
    #[tokio::test]
    async fn fallback_fails_over_scalar_llm_tier() {
        let chain = Chain::<dyn PreferenceLlm>::new(
            vec![
                Arc::new(StubLlm {
                    reply: "",
                    fail: true,
                }),
                Arc::new(StubLlm {
                    reply: "concise",
                    fail: false,
                }),
            ],
            ChainStrategy::FallbackOnError,
        )
        .unwrap();

        let outcome = chain.query("p", "s").await.unwrap();
        assert_eq!(outcome.value, "concise");
        assert_eq!(outcome.provenance.winning_tier, Some(1));
        assert_eq!(outcome.provenance.tier_errors.len(), 1);
    }

    // -- Provenance ---------------------------------------------------------

    /// Hard requirement: a caller must be able to tell which tier produced a
    /// result from `SignalResult::metadata` alone.
    #[tokio::test]
    async fn provenance_lands_in_metadata() {
        let chain = Chain::new(
            vec![
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.10)]),
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.95)]),
            ],
            ChainStrategy::Escalate { threshold: 0.5 },
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        let mut metadata = HashMap::new();
        outcome.provenance.write_metadata(&mut metadata);

        let record = metadata
            .get(PROVENANCE_METADATA_KEY)
            .expect("provenance recorded under the stable key");
        assert_eq!(record["strategy"], "escalate");
        assert_eq!(record["winning_tier"], 1);
        assert_eq!(record["escalated"], true);
        assert_eq!(record["tiers_attempted"], serde_json::json!([0, 1]));
    }

    /// Swallowed tier errors stay visible in the metadata record — otherwise
    /// a silently-degrading tier 0 is undebuggable.
    #[tokio::test]
    async fn provenance_reports_swallowed_tier_errors() {
        let chain = Chain::new(
            vec![
                StubPii::err(),
                StubPii::ok(vec![entity("EMAIL", 0, 7, 0.9)]),
            ],
            ChainStrategy::Escalate { threshold: 0.5 },
        )
        .unwrap();

        let outcome = chain.detect_entities("text").await.unwrap();
        let mut metadata = HashMap::new();
        outcome.provenance.write_metadata(&mut metadata);

        let errors = metadata[PROVENANCE_METADATA_KEY]["tier_errors"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0]["tier"], 0);
        assert!(errors[0]["error"].as_str().unwrap().contains("tier down"));
    }

    #[tokio::test]
    async fn merge_all_provenance_has_no_winning_tier() {
        let chain = Chain::new(
            vec![toxicity(&[("hate", 0.3)]), toxicity(&[("hate", 0.8)])],
            ChainStrategy::MergeAll,
        )
        .unwrap();

        let outcome = chain.detect(":(").await.unwrap();
        let mut metadata = HashMap::new();
        outcome.provenance.write_metadata(&mut metadata);
        assert_eq!(
            metadata[PROVENANCE_METADATA_KEY]["winning_tier"],
            serde_json::Value::Null
        );
        assert_eq!(metadata[PROVENANCE_METADATA_KEY]["strategy"], "merge_all");
    }

    // -- Backward compatibility --------------------------------------------

    /// `Chain::single` is the migration path for the old `Box<dyn X>`
    /// constructors: one tier, transparent behaviour, tier 0 always wins.
    #[tokio::test]
    async fn single_tier_chain_is_transparent() {
        let chain = Chain::single(StubPii::ok(vec![entity("EMAIL", 0, 7, 0.05)]));
        assert_eq!(chain.len(), 1);

        // Returns even a very unconfident result, exactly as a bare detector would.
        let outcome = chain.detect_entities("text").await.unwrap();
        assert_eq!(outcome.value[0].confidence, 0.05);
        assert_eq!(outcome.provenance.winning_tier, Some(0));
    }

    /// A single-tier chain propagates the original `SignalError` variant so
    /// callers that match on the kind keep working.
    #[tokio::test]
    async fn single_tier_chain_preserves_error_variant() {
        struct ConfigErrLlm;

        #[async_trait]
        impl PreferenceLlm for ConfigErrLlm {
            async fn query(&self, _p: &str, _s: &str) -> Result<String, SignalError> {
                Err(SignalError::Configuration("bad config".into()))
            }
        }

        let chain = Chain::single(Arc::new(ConfigErrLlm) as Arc<dyn PreferenceLlm>);
        let err = chain.query("p", "s").await.unwrap_err();
        assert!(
            matches!(err, SignalError::Configuration(_)),
            "got {err:?}, expected the original Configuration variant"
        );
    }

    // -- EmbeddingEngine dimension guard ------------------------------------

    #[test]
    fn embedding_chain_rejects_mismatched_dimensions() {
        struct StubEngine(usize);

        #[async_trait]
        impl EmbeddingEngine for StubEngine {
            async fn embed(&self, _t: &str) -> Result<Vec<f32>, EmbeddingError> {
                Ok(vec![0.0; self.0])
            }
            async fn embed_batch(&self, _t: &[&str]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
                Ok(vec![])
            }
            fn dimension(&self) -> usize {
                self.0
            }
        }

        let chain = Chain::<dyn EmbeddingEngine>::new(
            vec![Arc::new(StubEngine(384)), Arc::new(StubEngine(768))],
            ChainStrategy::FallbackOnError,
        )
        .unwrap();

        assert!(matches!(
            chain.validate_dimensions(),
            Err(SignalError::Configuration(_))
        ));

        let ok = Chain::<dyn EmbeddingEngine>::new(
            vec![Arc::new(StubEngine(384)), Arc::new(StubEngine(384))],
            ChainStrategy::FallbackOnError,
        )
        .unwrap();
        assert!(ok.validate_dimensions().is_ok());
    }
}

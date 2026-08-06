use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::chain::{Chain, ChainProvenance};
use crate::signal::Signal;
use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

// ---------------------------------------------------------------------------
// JailbreakSignal
// ---------------------------------------------------------------------------

#[async_trait]
pub trait JailbreakDetector: Send + Sync {
    /// Returns (confidence, labels) where labels describe attack vectors
    /// e.g. ["injection", "roleplay", "encoding"].
    async fn detect(&self, text: &str) -> Result<(f64, Vec<String>), SignalError>;
}

pub struct JailbreakSignal {
    chain: Chain<dyn JailbreakDetector>,
    cached_result: Arc<Mutex<HashMap<u64, SignalResult>>>,
}

impl JailbreakSignal {
    /// Single-detector constructor, preserved for backward compatibility.
    /// Equivalent to a one-element chain.
    pub fn new(detector: Box<dyn JailbreakDetector>) -> Self {
        Self::with_chain(Chain::single(Arc::from(detector)))
    }

    /// Tiered constructor.
    pub fn with_chain(chain: Chain<dyn JailbreakDetector>) -> Self {
        Self {
            chain,
            cached_result: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn hash_text(text: &str) -> u64 {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        hasher.finish()
    }
}

#[async_trait]
impl Signal for JailbreakSignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        let h = Self::hash_text(&ctx.text);

        // Check dedup cache
        {
            let results = self.cached_result.lock().await;
            if let Some(cached) = results.get(&h) {
                return Ok(cached.clone());
            }
        }

        // Fail-closed: when every tier in the chain errors, assume jailbreak.
        let mut metadata = HashMap::new();
        let (confidence, labels) = match self.chain.detect(&ctx.text).await {
            Ok(outcome) => {
                outcome.provenance.write_metadata(&mut metadata);
                outcome.value
            }
            Err(_) => (1.0, vec!["error_failclosed".to_string()]),
        };

        let result = SignalResult {
            name: self.name().to_string(),
            signal_type: self.signal_type(),
            confidence,
            labels,
            metadata,
        };

        // Store in cache
        {
            let mut results = self.cached_result.lock().await;
            results.insert(h, result.clone());
        }

        Ok(result)
    }

    fn name(&self) -> &str {
        "jailbreak"
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Jailbreak
    }
}

// ---------------------------------------------------------------------------
// PiiSignal
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PiiEntity {
    pub entity_type: String,
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub confidence: f64,
}

#[async_trait]
pub trait PiiDetector: Send + Sync {
    async fn detect_entities(&self, text: &str) -> Result<Vec<PiiEntity>, SignalError>;
}

pub struct PiiSignal {
    chain: Chain<dyn PiiDetector>,
    deny_list: HashSet<String>,
    max_chunk_size: usize,
}

impl PiiSignal {
    /// Single-detector constructor, preserved for backward compatibility.
    /// Equivalent to a one-element chain.
    pub fn new(
        detector: Box<dyn PiiDetector>,
        deny_list: HashSet<String>,
        max_chunk_size: usize,
    ) -> Self {
        Self::with_chain(
            Chain::single(Arc::from(detector)),
            deny_list,
            max_chunk_size,
        )
    }

    /// Tiered constructor.
    pub fn with_chain(
        chain: Chain<dyn PiiDetector>,
        deny_list: HashSet<String>,
        max_chunk_size: usize,
    ) -> Self {
        Self {
            chain,
            deny_list,
            max_chunk_size,
        }
    }

    /// Split text into non-overlapping chunks at word boundaries.
    fn chunk_text(text: &str, max_size: usize) -> Vec<(usize, &str)> {
        if text.len() <= max_size {
            return vec![(0, text)];
        }

        let mut chunks = Vec::new();
        let mut start = 0;

        while start < text.len() {
            let end = (start + max_size).min(text.len());
            // Snap back to word boundary if not at end
            let actual_end = if end < text.len() {
                text[start..end]
                    .rfind(char::is_whitespace)
                    .map(|p| start + p)
                    .unwrap_or(end)
            } else {
                end
            };

            chunks.push((start, &text[start..actual_end]));
            start = actual_end;
            // Skip whitespace between chunks
            while start < text.len() && text[start..].starts_with(char::is_whitespace) {
                start += 1;
            }
        }

        chunks
    }
}

#[async_trait]
impl Signal for PiiSignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        let chunks = Self::chunk_text(&ctx.text, self.max_chunk_size);
        let mut all_entities: Vec<PiiEntity> = Vec::new();
        // Chunking calls the chain once per chunk. Fold the per-chunk records
        // into one: the union of tiers touched, and the deepest (most
        // expensive) tier any chunk had to reach.
        let mut provenance = ChainProvenance {
            strategy: self.chain.strategy().as_str().to_string(),
            ..Default::default()
        };

        for (offset, chunk) in chunks {
            let outcome = self.chain.detect_entities(chunk).await?;
            for tier in outcome.provenance.tiers_attempted {
                if !provenance.tiers_attempted.contains(&tier) {
                    provenance.tiers_attempted.push(tier);
                }
            }
            provenance
                .tier_errors
                .extend(outcome.provenance.tier_errors);
            provenance.winning_tier =
                match (provenance.winning_tier, outcome.provenance.winning_tier) {
                    (Some(a), Some(b)) => Some(a.max(b)),
                    (a, b) => a.or(b),
                };

            let mut entities = outcome.value;
            // Adjust offsets for chunked text
            for e in &mut entities {
                e.start += offset;
                e.end += offset;
            }
            all_entities.extend(entities);
        }
        provenance.tiers_attempted.sort_unstable();

        // Filter to denied entity types
        let flagged: Vec<PiiEntity> = all_entities
            .into_iter()
            .filter(|e| self.deny_list.contains(&e.entity_type))
            .collect();

        let confidence = if flagged.is_empty() {
            0.0
        } else {
            flagged.iter().map(|e| e.confidence).fold(0.0_f64, f64::max)
        };

        let labels: Vec<String> = flagged.iter().map(|e| e.entity_type.clone()).collect();

        let mut metadata = HashMap::new();
        metadata.insert(
            "entity_count".to_string(),
            serde_json::Value::Number(serde_json::Number::from(flagged.len())),
        );

        let entity_details: Vec<serde_json::Value> = flagged
            .iter()
            .map(|e| {
                serde_json::json!({
                    "type": e.entity_type,
                    "start": e.start,
                    "end": e.end,
                    "confidence": e.confidence,
                })
            })
            .collect();
        metadata.insert(
            "entities".to_string(),
            serde_json::Value::Array(entity_details),
        );
        provenance.write_metadata(&mut metadata);

        Ok(SignalResult {
            name: self.name().to_string(),
            signal_type: self.signal_type(),
            confidence,
            labels,
            metadata,
        })
    }

    fn name(&self) -> &str {
        "pii"
    }

    fn signal_type(&self) -> SignalType {
        SignalType::PII
    }
}

// ---------------------------------------------------------------------------
// ToxicitySignal
// ---------------------------------------------------------------------------

#[async_trait]
pub trait ToxicityDetector: Send + Sync {
    /// Returns vec of (category, score).
    async fn detect(&self, text: &str) -> Result<Vec<(String, f64)>, SignalError>;
}

pub struct ToxicitySignal {
    chain: Chain<dyn ToxicityDetector>,
    threshold: f64,
}

impl ToxicitySignal {
    /// Single-detector constructor, preserved for backward compatibility.
    /// Equivalent to a one-element chain.
    pub fn new(detector: Box<dyn ToxicityDetector>, threshold: f64) -> Self {
        Self::with_chain(Chain::single(Arc::from(detector)), threshold)
    }

    /// Tiered constructor.
    pub fn with_chain(chain: Chain<dyn ToxicityDetector>, threshold: f64) -> Self {
        Self { chain, threshold }
    }
}

#[async_trait]
impl Signal for ToxicitySignal {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError> {
        let outcome = self.chain.detect(&ctx.text).await?;
        let scores = outcome.value;

        let flagged: Vec<&(String, f64)> = scores
            .iter()
            .filter(|(_, score)| *score > self.threshold)
            .collect();

        let confidence = flagged.iter().map(|(_, s)| *s).fold(0.0_f64, f64::max);

        let labels: Vec<String> = flagged.iter().map(|(cat, _)| cat.clone()).collect();

        let mut metadata = HashMap::new();
        let scores_map: serde_json::Map<String, serde_json::Value> = scores
            .iter()
            .map(|(cat, score)| {
                (
                    cat.clone(),
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(*score).unwrap_or(serde_json::Number::from(0)),
                    ),
                )
            })
            .collect();
        metadata.insert("scores".to_string(), serde_json::Value::Object(scores_map));
        metadata.insert("threshold".to_string(), serde_json::json!(self.threshold));
        outcome.provenance.write_metadata(&mut metadata);

        Ok(SignalResult {
            name: self.name().to_string(),
            signal_type: self.signal_type(),
            confidence,
            labels,
            metadata,
        })
    }

    fn name(&self) -> &str {
        "toxicity"
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Toxicity
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- Mock detectors -----------------------------------------------------

    struct MockJailbreakDetector {
        confidence: f64,
        labels: Vec<String>,
    }

    #[async_trait]
    impl JailbreakDetector for MockJailbreakDetector {
        async fn detect(&self, _text: &str) -> Result<(f64, Vec<String>), SignalError> {
            Ok((self.confidence, self.labels.clone()))
        }
    }

    struct FailingJailbreakDetector;

    #[async_trait]
    impl JailbreakDetector for FailingJailbreakDetector {
        async fn detect(&self, _text: &str) -> Result<(f64, Vec<String>), SignalError> {
            Err(SignalError::Inference("boom".into()))
        }
    }

    struct MockPiiDetector {
        entities: Vec<PiiEntity>,
    }

    #[async_trait]
    impl PiiDetector for MockPiiDetector {
        async fn detect_entities(&self, _text: &str) -> Result<Vec<PiiEntity>, SignalError> {
            Ok(self.entities.clone())
        }
    }

    struct MockToxicityDetector {
        scores: Vec<(String, f64)>,
    }

    #[async_trait]
    impl ToxicityDetector for MockToxicityDetector {
        async fn detect(&self, _text: &str) -> Result<Vec<(String, f64)>, SignalError> {
            Ok(self.scores.clone())
        }
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

    // -- Jailbreak tests ----------------------------------------------------

    #[tokio::test]
    async fn jailbreak_detects_attack() {
        let signal = JailbreakSignal::new(Box::new(MockJailbreakDetector {
            confidence: 0.95,
            labels: vec!["injection".into()],
        }));

        let result = signal.evaluate(&make_ctx("ignore all")).await.unwrap();
        assert_eq!(result.signal_type, SignalType::Jailbreak);
        assert_eq!(result.confidence, 0.95);
        assert!(result.labels.contains(&"injection".to_string()));
    }

    #[tokio::test]
    async fn jailbreak_caches_duplicate_text() {
        let signal = JailbreakSignal::new(Box::new(MockJailbreakDetector {
            confidence: 0.8,
            labels: vec!["roleplay".into()],
        }));

        let ctx = make_ctx("same text");
        let r1 = signal.evaluate(&ctx).await.unwrap();
        let r2 = signal.evaluate(&ctx).await.unwrap();
        assert_eq!(r1.confidence, r2.confidence);
    }

    #[tokio::test]
    async fn jailbreak_fail_closed_on_error() {
        let signal = JailbreakSignal::new(Box::new(FailingJailbreakDetector));

        let result = signal.evaluate(&make_ctx("anything")).await.unwrap();
        assert_eq!(result.confidence, 1.0);
        assert!(result.labels.contains(&"error_failclosed".to_string()));
    }

    // -- PII tests ----------------------------------------------------------

    #[tokio::test]
    async fn pii_flags_denied_entities() {
        let detector = MockPiiDetector {
            entities: vec![
                PiiEntity {
                    entity_type: "EMAIL".into(),
                    text: "a@b.com".into(),
                    start: 0,
                    end: 7,
                    confidence: 0.99,
                },
                PiiEntity {
                    entity_type: "NAME".into(),
                    text: "Alice".into(),
                    start: 10,
                    end: 15,
                    confidence: 0.7,
                },
            ],
        };

        let mut deny = HashSet::new();
        deny.insert("EMAIL".into());

        let signal = PiiSignal::new(Box::new(detector), deny, 4096);
        let result = signal
            .evaluate(&make_ctx("a@b.com hi Alice"))
            .await
            .unwrap();

        assert_eq!(result.signal_type, SignalType::PII);
        assert_eq!(result.confidence, 0.99);
        assert!(result.labels.contains(&"EMAIL".to_string()));
        assert!(!result.labels.contains(&"NAME".to_string()));
    }

    #[tokio::test]
    async fn pii_no_entities_zero_confidence() {
        let signal = PiiSignal::new(
            Box::new(MockPiiDetector { entities: vec![] }),
            HashSet::new(),
            4096,
        );
        let result = signal.evaluate(&make_ctx("hello")).await.unwrap();
        assert_eq!(result.confidence, 0.0);
        assert!(result.labels.is_empty());
    }

    #[tokio::test]
    async fn pii_chunks_long_text() {
        let chunks = PiiSignal::chunk_text("hello world foo bar", 11);
        assert!(chunks.len() >= 2);
        // Every chunk starts at the right offset
        for (offset, chunk) in &chunks {
            assert_eq!(
                &"hello world foo bar"[*offset..*offset + chunk.len()],
                *chunk
            );
        }
    }

    // -- Toxicity tests -----------------------------------------------------

    #[tokio::test]
    async fn toxicity_flags_above_threshold() {
        let detector = MockToxicityDetector {
            scores: vec![
                ("hate_speech".into(), 0.9),
                ("harassment".into(), 0.3),
                ("violence".into(), 0.1),
            ],
        };

        let signal = ToxicitySignal::new(Box::new(detector), 0.5);
        let result = signal.evaluate(&make_ctx("bad text")).await.unwrap();

        assert_eq!(result.signal_type, SignalType::Toxicity);
        assert_eq!(result.confidence, 0.9);
        assert!(result.labels.contains(&"hate_speech".to_string()));
        assert!(!result.labels.contains(&"harassment".to_string()));
    }

    #[tokio::test]
    async fn toxicity_all_below_threshold() {
        let detector = MockToxicityDetector {
            scores: vec![("hate_speech".into(), 0.1), ("violence".into(), 0.2)],
        };

        let signal = ToxicitySignal::new(Box::new(detector), 0.5);
        let result = signal.evaluate(&make_ctx("nice text")).await.unwrap();

        assert_eq!(result.confidence, 0.0);
        assert!(result.labels.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Chain integration tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod chain_tests {
    use super::*;
    use crate::chain::{ChainStrategy, PROVENANCE_METADATA_KEY};

    struct TieredPii {
        entities: Vec<PiiEntity>,
    }

    #[async_trait]
    impl PiiDetector for TieredPii {
        async fn detect_entities(&self, _text: &str) -> Result<Vec<PiiEntity>, SignalError> {
            Ok(self.entities.clone())
        }
    }

    fn entity(kind: &str, confidence: f64) -> PiiEntity {
        PiiEntity {
            entity_type: kind.into(),
            text: "a@b.com".into(),
            start: 0,
            end: 7,
            confidence,
        }
    }

    fn ctx(text: &str) -> ClassificationContext {
        ClassificationContext {
            text: text.to_string(),
            history: vec![],
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    /// End-to-end: a tiered PiiSignal escalates and the winning tier is
    /// visible in the emitted `SignalResult::metadata`.
    #[tokio::test]
    async fn pii_signal_reports_escalation_in_metadata() {
        let chain = Chain::new(
            vec![
                Arc::new(TieredPii {
                    entities: vec![entity("EMAIL", 0.20)],
                }) as Arc<dyn PiiDetector>,
                Arc::new(TieredPii {
                    entities: vec![entity("EMAIL", 0.98)],
                }),
            ],
            ChainStrategy::Escalate { threshold: 0.5 },
        )
        .unwrap();

        let mut deny = HashSet::new();
        deny.insert("EMAIL".to_string());

        let signal = PiiSignal::with_chain(chain, deny, 4096);
        let result = signal.evaluate(&ctx("a@b.com")).await.unwrap();

        assert_eq!(result.confidence, 0.98, "tier 1's answer surfaced");
        let record = &result.metadata[PROVENANCE_METADATA_KEY];
        assert_eq!(record["strategy"], "escalate");
        assert_eq!(record["winning_tier"], 1);
        assert_eq!(record["escalated"], true);
    }

    /// A signal built with the legacy single-detector constructor still emits
    /// provenance, reporting tier 0.
    #[tokio::test]
    async fn legacy_constructor_reports_tier_zero() {
        let signal = PiiSignal::new(
            Box::new(TieredPii {
                entities: vec![entity("EMAIL", 0.9)],
            }),
            HashSet::new(),
            4096,
        );
        let result = signal.evaluate(&ctx("a@b.com")).await.unwrap();
        let record = &result.metadata[PROVENANCE_METADATA_KEY];
        assert_eq!(record["winning_tier"], 0);
        assert_eq!(record["escalated"], false);
    }

    /// Toxicity merges categories across tiers, deduping on category name.
    #[tokio::test]
    async fn toxicity_signal_merges_tiers() {
        struct Tox(Vec<(String, f64)>);

        #[async_trait]
        impl ToxicityDetector for Tox {
            async fn detect(&self, _t: &str) -> Result<Vec<(String, f64)>, SignalError> {
                Ok(self.0.clone())
            }
        }

        let chain = Chain::new(
            vec![
                Arc::new(Tox(vec![("hate".into(), 0.4)])) as Arc<dyn ToxicityDetector>,
                Arc::new(Tox(vec![("hate".into(), 0.92), ("violence".into(), 0.71)])),
            ],
            ChainStrategy::MergeAll,
        )
        .unwrap();

        let signal = ToxicitySignal::with_chain(chain, 0.5);
        let result = signal.evaluate(&ctx("bad")).await.unwrap();

        assert_eq!(result.confidence, 0.92, "max across tiers");
        assert!(result.labels.contains(&"hate".to_string()));
        assert!(result.labels.contains(&"violence".to_string()));
        assert_eq!(
            result.metadata[PROVENANCE_METADATA_KEY]["strategy"],
            "merge_all"
        );
    }
}

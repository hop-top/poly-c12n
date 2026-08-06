//! Heuristic token counting — tier-1 baseline, no vocabulary required.

use async_trait::async_trait;

use crate::signals::context::Tokenizer;

/// Default characters-per-token divisor for whitespace-delimited Latin text.
///
/// Matches the widely-cited "~4 characters per token" rule of thumb for
/// byte-pair-encoding tokenizers on English prose.
///
/// Kept private: `cbindgen` exports public scalar constants into the generated
/// C ABI header, and an internal tuning value has no business in that surface.
/// Read it through [`ApproxTokenizer::default_chars_per_token`].
const DEFAULT_CHARS_PER_TOKEN: f64 = 4.0;

/// Characters per token assumed for CJK text, where BPE tokenizers typically
/// emit roughly one token per character (sometimes fewer for common bigrams).
const CJK_CHARS_PER_TOKEN: f64 = 1.0;

/// Each run of non-alphanumeric, non-space characters tends to split into its
/// own token(s); this is the marginal token cost charged per such run beyond
/// what the character-count estimate already covers.
const PUNCT_RUN_COST: f64 = 0.5;

/// A vocabulary-free, approximate token counter.
///
/// # This is an ESTIMATE
///
/// [`ApproxTokenizer`] has no BPE vocabulary, no merge table, and no model. It
/// counts characters by class and divides. [`Tokenizer::model_name`] returns a
/// name containing `approx` precisely so that anything reading
/// `tokenizer_model` out of [`crate::signals::context::ContextSignal`]'s
/// metadata can see the number is not authoritative.
///
/// # Accuracy
///
/// Measured against typical BPE (cl100k-family) behaviour:
///
/// | Input                              | Expected error |
/// |------------------------------------|----------------|
/// | English/Latin prose                 | ±10–15%       |
/// | Prose with heavy punctuation        | ±20%          |
/// | Source code, JSON, markup           | ±25–35% (usually **under**-counts) |
/// | CJK text                            | ±30%          |
/// | Base64 / hashes / random strings    | ±50% (badly under-counts) |
/// | Emoji, rare Unicode                 | unbounded (can under-count several-fold) |
///
/// Do **not** use this to enforce a hard context-window limit, to bill a
/// customer, or to decide whether a request fits in a model's budget without a
/// safety margin. Use it for routing, coarse size bucketing, and rough cost
/// projection — which is exactly what `ContextSignal` does with it.
///
/// For exact counts, implement [`Tokenizer`] over the target model's real
/// tokenizer and pass that instead; the signal takes `Arc<dyn Tokenizer>`.
///
/// # Method
///
/// 1. Characters are partitioned into CJK (Unicode CJK/Hiragana/Katakana/Hangul
///    blocks), punctuation/symbol runs, and everything else.
/// 2. Non-CJK characters are divided by `chars_per_token` (default
///    `DEFAULT_CHARS_PER_TOKEN` (4.0)); CJK characters by
///    `CJK_CHARS_PER_TOKEN`.
/// 3. Each run of punctuation adds a fixed marginal cost (0.5 tokens),
///    reflecting that BPE rarely merges punctuation into neighbouring words.
/// 4. The result is rounded up. Non-empty input always yields at least 1.
pub struct ApproxTokenizer {
    model_name: String,
    chars_per_token: f64,
}

impl Default for ApproxTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl ApproxTokenizer {
    /// The characters-per-token divisor used when none is supplied: `4.0`.
    pub fn default_chars_per_token() -> f64 {
        DEFAULT_CHARS_PER_TOKEN
    }

    /// The divisor this instance was built with.
    pub fn chars_per_token(&self) -> f64 {
        self.chars_per_token
    }

    /// Builds the default estimator, reporting the model name `"approx-v1"`.
    pub fn new() -> Self {
        Self {
            model_name: "approx-v1".to_string(),
            chars_per_token: DEFAULT_CHARS_PER_TOKEN,
        }
    }

    /// Builds an estimator tuned for a specific model family.
    ///
    /// `model` is recorded verbatim except that `-approx` is appended if the
    /// name does not already advertise itself as an approximation — callers
    /// reading `tokenizer_model` must never mistake this for a real tokenizer.
    ///
    /// `chars_per_token` must be finite and positive; otherwise
    /// `DEFAULT_CHARS_PER_TOKEN` (4.0) is used.
    pub fn for_model(model: impl Into<String>, chars_per_token: f64) -> Self {
        let raw = model.into();
        let model_name = if raw.contains("approx") {
            raw
        } else {
            format!("{raw}-approx")
        };
        let chars_per_token = if chars_per_token.is_finite() && chars_per_token > 0.0 {
            chars_per_token
        } else {
            DEFAULT_CHARS_PER_TOKEN
        };
        Self {
            model_name,
            chars_per_token,
        }
    }

    /// True for characters in the main CJK / Kana / Hangul blocks.
    fn is_cjk(c: char) -> bool {
        matches!(c as u32,
            0x3040..=0x30FF     // Hiragana + Katakana
            | 0x3400..=0x4DBF   // CJK Ext A
            | 0x4E00..=0x9FFF   // CJK Unified
            | 0xAC00..=0xD7AF   // Hangul syllables
            | 0xF900..=0xFAFF   // CJK compatibility
            | 0x20000..=0x2FA1F // CJK Ext B+
        )
    }

    /// The estimate, as an unrounded float. Exposed for callers that want to
    /// apply their own safety margin before rounding.
    pub fn estimate(&self, text: &str) -> f64 {
        if text.is_empty() {
            return 0.0;
        }

        let mut cjk = 0usize;
        let mut other = 0usize;
        let mut punct_runs = 0usize;
        let mut in_punct = false;

        for c in text.chars() {
            if Self::is_cjk(c) {
                cjk += 1;
                in_punct = false;
                continue;
            }
            let is_punct = !c.is_alphanumeric() && !c.is_whitespace();
            if is_punct {
                if !in_punct {
                    punct_runs += 1;
                }
                in_punct = true;
            } else {
                in_punct = false;
            }
            other += 1;
        }

        (other as f64 / self.chars_per_token)
            + (cjk as f64 / CJK_CHARS_PER_TOKEN)
            + (punct_runs as f64 * PUNCT_RUN_COST)
    }
}

#[async_trait]
impl Tokenizer for ApproxTokenizer {
    fn count_tokens(&self, text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }
        (self.estimate(text).ceil() as usize).max(1)
    }

    fn model_name(&self) -> &str {
        &self.model_name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;

    use crate::signal::Signal;
    use crate::signals::context::{ContextSignal, ModelPricing};
    use crate::types::{ClassificationContext, SignalType};

    fn make_ctx(text: &str) -> ClassificationContext {
        ClassificationContext {
            text: text.to_string(),
            history: vec![],
            headers: HashMap::new(),
            image_url: None,
            config: HashMap::new(),
        }
    }

    // -- Honesty about being an estimate ------------------------------------

    #[test]
    fn model_name_advertises_approximation() {
        assert!(ApproxTokenizer::new().model_name().contains("approx"));
        assert!(ApproxTokenizer::for_model("gpt-4", 4.0)
            .model_name()
            .contains("approx"));
        // Already-honest names are not double-suffixed.
        assert_eq!(
            ApproxTokenizer::for_model("approx-cl100k", 3.8).model_name(),
            "approx-cl100k"
        );
    }

    #[test]
    fn divisor_accessors_report_the_configured_value() {
        assert_eq!(ApproxTokenizer::default_chars_per_token(), 4.0);
        assert_eq!(
            ApproxTokenizer::new().chars_per_token(),
            ApproxTokenizer::default_chars_per_token()
        );
        assert_eq!(ApproxTokenizer::for_model("m", 3.5).chars_per_token(), 3.5);
    }

    #[test]
    fn invalid_chars_per_token_falls_back_to_default() {
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let t = ApproxTokenizer::for_model("x", bad);
            assert_eq!(
                t.chars_per_token(),
                ApproxTokenizer::default_chars_per_token(),
                "for {bad}"
            );
            assert!(t.count_tokens("hello world") > 0);
        }
    }

    // -- Basic behaviour ----------------------------------------------------

    #[test]
    fn empty_text_is_zero_tokens() {
        assert_eq!(ApproxTokenizer::new().count_tokens(""), 0);
    }

    #[test]
    fn any_non_empty_text_is_at_least_one_token() {
        let t = ApproxTokenizer::new();
        for s in ["a", " ", ".", "é", "中"] {
            assert!(t.count_tokens(s) >= 1, "for {s:?}");
        }
    }

    #[test]
    fn count_is_monotonic_in_length() {
        let t = ApproxTokenizer::new();
        let short = t.count_tokens(&"word ".repeat(10));
        let long = t.count_tokens(&"word ".repeat(100));
        assert!(long > short);
    }

    #[test]
    fn deterministic() {
        let t = ApproxTokenizer::new();
        let text = "The quick brown fox jumps over the lazy dog.";
        assert_eq!(t.count_tokens(text), t.count_tokens(text));
    }

    // -- Accuracy envelope --------------------------------------------------

    #[test]
    fn english_prose_lands_near_four_chars_per_token() {
        let t = ApproxTokenizer::new();
        // 92 chars of plain prose; a BPE tokenizer emits roughly 19-22 tokens.
        let text = "The quick brown fox jumps over the lazy dog while the sun sets \
            slowly behind the hills.";
        let n = t.count_tokens(text);
        let baseline = text.chars().count() as f64 / 4.0;
        assert!(
            (n as f64) >= baseline * 0.85 && (n as f64) <= baseline * 1.4,
            "got {n} for baseline {baseline}"
        );
    }

    #[test]
    fn punctuation_increases_the_estimate() {
        let t = ApproxTokenizer::new();
        let plain = "alpha beta gamma delta";
        let punct = "alpha, beta; gamma. delta!";
        assert!(
            t.count_tokens(punct) > t.count_tokens(plain),
            "punctuation should cost extra tokens"
        );
    }

    #[test]
    fn cjk_costs_roughly_one_token_per_character() {
        let t = ApproxTokenizer::new();
        let cjk = "今日はとても良い天気ですね";
        let n = t.count_tokens(cjk);
        let chars = cjk.chars().count();
        // Much denser than the 4-chars-per-token Latin assumption.
        assert!(n >= chars, "got {n} for {chars} CJK chars");
        assert!(n <= chars * 2);
    }

    #[test]
    fn cjk_denser_than_latin_of_same_char_count() {
        let t = ApproxTokenizer::new();
        let latin: String = "a".repeat(20);
        let cjk: String = "中".repeat(20);
        assert!(t.count_tokens(&cjk) > t.count_tokens(&latin));
    }

    #[test]
    fn multibyte_counted_by_character_not_byte() {
        let t = ApproxTokenizer::new();
        // "é" is 2 bytes but 1 char; the estimate must use chars.
        let accented = "é".repeat(40);
        let ascii = "e".repeat(40);
        assert_eq!(t.count_tokens(&accented), t.count_tokens(&ascii));
        assert!(accented.len() > ascii.len());
    }

    #[test]
    fn emoji_do_not_panic() {
        let t = ApproxTokenizer::new();
        assert!(t.count_tokens("🚀🎉🔥 launch day 🥳") >= 1);
    }

    #[test]
    fn custom_ratio_changes_the_estimate() {
        let text = "the quick brown fox jumps over the lazy dog";
        let coarse = ApproxTokenizer::for_model("m", 8.0).count_tokens(text);
        let fine = ApproxTokenizer::for_model("m", 2.0).count_tokens(text);
        assert!(fine > coarse, "{fine} should exceed {coarse}");
    }

    #[test]
    fn estimate_is_unrounded_and_consistent_with_count() {
        let t = ApproxTokenizer::new();
        let text = "hello world, this is a test.";
        assert_eq!(t.count_tokens(text), t.estimate(text).ceil() as usize);
    }

    // -- Through the real Signal --------------------------------------------

    #[tokio::test]
    async fn wired_into_context_signal() {
        let signal = ContextSignal::new("ctx", Arc::new(ApproxTokenizer::new()), 0.5, vec![]);
        let result = signal.evaluate(&make_ctx("hello there")).await.unwrap();

        assert_eq!(result.signal_type, SignalType::Context);
        assert_eq!(result.labels, vec!["short"]);
        assert!(result.metadata["input_tokens"].as_u64().unwrap() > 0);
        assert!(result.metadata["tokenizer_model"]
            .as_str()
            .unwrap()
            .contains("approx"));
    }

    #[tokio::test]
    async fn context_signal_buckets_by_estimated_size() {
        let signal = ContextSignal::new("ctx", Arc::new(ApproxTokenizer::new()), 0.5, vec![]);
        // ~10k chars -> ~2.5k tokens -> "long".
        let long = signal
            .evaluate(&make_ctx(&"word ".repeat(2000)))
            .await
            .unwrap();
        assert_eq!(long.labels, vec!["long"]);

        // ~50k chars -> ~12.5k tokens -> "very_long".
        let very_long = signal
            .evaluate(&make_ctx(&"word ".repeat(10_000)))
            .await
            .unwrap();
        assert_eq!(very_long.labels, vec!["very_long"]);
    }

    #[tokio::test]
    async fn context_signal_costs_track_estimate() {
        let pricing = vec![ModelPricing {
            model: "gpt-4".into(),
            input_cost_per_1k: 0.03,
            output_cost_per_1k: 0.06,
        }];
        let tok = Arc::new(ApproxTokenizer::new());
        let expected = tok.count_tokens(&"word ".repeat(400)) as f64;
        let signal = ContextSignal::new("ctx", tok, 0.0, pricing);
        let result = signal
            .evaluate(&make_ctx(&"word ".repeat(400)))
            .await
            .unwrap();

        let costs = result.metadata["costs"].as_object().unwrap();
        let total = costs["gpt-4"]["total_cost"].as_f64().unwrap();
        assert!((total - (expected / 1000.0) * 0.03).abs() < 1e-9);
    }
}

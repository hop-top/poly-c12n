//! Stopword-frequency language identification — tier-1 baseline.

use std::collections::HashMap;

use async_trait::async_trait;

use crate::signals::language::{DetectedLanguage, LanguageDetector};

/// Minimum share of tokens that must be stopwords of the winning language
/// before any detection is reported at all.
const MIN_HIT_RATIO: f64 = 0.08;

/// Minimum number of stopword hits required. Guards against a one-word input
/// scoring 100% on a single accidental match.
const MIN_HITS: usize = 2;

/// Languages covered, with their stopword lists.
///
/// Entries are `(code, name, stopwords)`. Stopwords are lowercase and chosen
/// to be as language-discriminating as possible: shared forms that appear in
/// several of the covered languages (`a`, `no`, `la`, `de`, `in`, `e`) are
/// deliberately excluded rather than counted for everyone.
const LANGUAGES: &[(&str, &str, &[&str])] = &[
    (
        "en",
        "English",
        &[
            "the", "and", "is", "are", "was", "were", "of", "to", "you", "that", "it", "for",
            "with", "this", "have", "has", "not", "but", "they", "from", "what", "which", "would",
            "there", "their", "about", "been", "will", "can", "your", "how", "when", "who",
        ],
    ),
    (
        "es",
        "Spanish",
        &[
            "el", "los", "las", "una", "que", "por", "con", "para", "como", "pero", "más", "esta",
            "este", "son", "sus", "muy", "también", "cuando", "porque", "todo", "hay", "está",
            "ser", "sobre", "desde", "hasta", "donde", "entre",
        ],
    ),
    (
        "fr",
        "French",
        &[
            "les", "des", "une", "est", "que", "pour", "dans", "qui", "avec", "sur", "pas", "plus",
            "sont", "cette", "mais", "aux", "être", "nous", "vous", "leur", "tout", "comme",
            "même", "aussi", "était", "avoir", "faire", "bien", "où", "ils",
        ],
    ),
    (
        "de",
        "German",
        &[
            "der", "die", "das", "und", "ist", "nicht", "ein", "eine", "mit", "auch", "auf", "für",
            "den", "dem", "sich", "sie", "von", "zu", "aber", "wenn", "wie", "war", "werden",
            "haben", "durch", "über", "nach", "oder", "noch", "sind",
        ],
    ),
    (
        "pt",
        "Portuguese",
        &[
            "os", "as", "um", "uma", "que", "não", "com", "para", "por", "mais", "como", "mas",
            "sua", "seu", "ele", "ela", "são", "foi", "está", "isso", "também", "quando", "muito",
            "então", "você", "pelo", "pela", "até", "já", "ser",
        ],
    ),
    (
        "it",
        "Italian",
        &[
            "il", "lo", "gli", "una", "che", "non", "per", "con", "sono", "come", "più", "anche",
            "questo", "questa", "della", "dello", "delle", "degli", "nel", "alla", "essere",
            "hanno", "loro", "molto", "quando", "perché", "ma", "se", "già",
        ],
    ),
];

/// A stopword-frequency language detector.
///
/// # How it works
///
/// The input is lowercased and split on non-alphabetic characters (Unicode
/// alphabetic, so accented forms survive). Each token is looked up in a
/// combined stopword index; every language it belongs to gets a hit. A
/// language's score is `hits(lang) / total_tokens`, and confidence is that
/// ratio scaled by the winner's margin over the runner-up, capped at 0.95.
///
/// [`LanguageDetector::detect`] returns the top scorer;
/// [`LanguageDetector::detect_multiple`] returns every language clearing the
/// thresholds, sorted by confidence descending, which is what surfaces
/// code-switched or mixed-language input.
///
/// # Coverage
///
/// Six languages only: English, Spanish, French, German, Portuguese, Italian.
/// Anything else — including every non-Latin-script language — returns `None`
/// rather than a wrong guess. That is a deliberate choice: a confident "en"
/// for Japanese input is worse than no answer.
///
/// # What it does NOT do
///
/// This is a **baseline** detector, not a language-ID model:
///
/// - **No script detection.** Chinese, Japanese, Korean, Arabic, Hebrew,
///   Cyrillic, Devanagari, Thai, Greek text yields `None`, not "unknown-script".
/// - **Short text is unreliable.** Under roughly 15–20 words, results are
///   noisy; under [`MIN_HITS`] stopword hits nothing is reported at all.
/// - **Closely related languages confuse it.** Spanish/Portuguese and
///   Spanish/Italian share a lot of function-word morphology; expect both to
///   appear in `detect_multiple` for such text.
/// - **Domain text degrades badly.** Source code, log lines, URLs, and
///   keyword lists have almost no function words and will usually yield `None`.
/// - **No dialect or regional variant distinction** (en-GB vs en-US,
///   pt-BR vs pt-PT).
///
/// Confidence is capped at 0.95 and never asserts certainty.
pub struct StopwordLanguageDetector {
    /// token -> indices into `LANGUAGES`
    index: HashMap<&'static str, Vec<usize>>,
}

impl Default for StopwordLanguageDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StopwordLanguageDetector {
    /// Builds the stopword index. Cheap, but worth doing once and sharing via
    /// the `Arc<dyn LanguageDetector>` the signal already takes.
    pub fn new() -> Self {
        let mut index: HashMap<&'static str, Vec<usize>> = HashMap::new();
        for (li, (_, _, words)) in LANGUAGES.iter().enumerate() {
            for w in words.iter() {
                index.entry(w).or_default().push(li);
            }
        }
        Self { index }
    }

    /// Language codes this detector can report, in declaration order.
    pub fn supported_codes() -> Vec<&'static str> {
        LANGUAGES.iter().map(|(c, _, _)| *c).collect()
    }

    /// Lowercases and splits into alphabetic tokens.
    fn tokenize(text: &str) -> Vec<String> {
        text.split(|c: char| !c.is_alphabetic())
            .filter(|t| !t.is_empty())
            .map(|t| t.to_lowercase())
            .collect()
    }

    /// Scores every language, returning `(index, hits, ratio)` sorted by ratio
    /// descending, alongside the total token count.
    fn score(&self, text: &str) -> (usize, Vec<(usize, usize, f64)>) {
        let tokens = Self::tokenize(text);
        let total = tokens.len();
        if total == 0 {
            return (0, Vec::new());
        }

        let mut hits = vec![0usize; LANGUAGES.len()];
        for t in &tokens {
            if let Some(langs) = self.index.get(t.as_str()) {
                for &li in langs {
                    hits[li] += 1;
                }
            }
        }

        let mut scored: Vec<(usize, usize, f64)> = hits
            .iter()
            .enumerate()
            .map(|(li, &h)| (li, h, h as f64 / total as f64))
            .filter(|&(_, h, ratio)| h >= MIN_HITS && ratio >= MIN_HIT_RATIO)
            .collect();

        scored.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
        (total, scored)
    }

    /// Turns a score into a `DetectedLanguage`. `margin` is the winner's ratio
    /// advantage over the next candidate (0.0 for non-winners), which pushes an
    /// unambiguous match toward the cap and holds ambiguous ones down.
    fn to_detected(entry: (usize, usize, f64), margin: f64) -> DetectedLanguage {
        let (li, _, ratio) = entry;
        let (code, name, _) = LANGUAGES[li];
        // Ratio saturates around 0.35 (typical for dense function-word prose);
        // the margin term rewards a clean win over the runner-up.
        let base = (ratio / 0.35).min(1.0);
        let confidence = (0.45 + 0.35 * base + 0.15 * margin.min(1.0)).min(0.95);
        DetectedLanguage {
            code: code.to_string(),
            name: name.to_string(),
            confidence,
        }
    }
}

#[async_trait]
impl LanguageDetector for StopwordLanguageDetector {
    fn detect(&self, text: &str) -> Option<DetectedLanguage> {
        let (_, scored) = self.score(text);
        let first = *scored.first()?;
        let margin = match scored.get(1) {
            Some(&(_, _, second)) if first.2 > 0.0 => (first.2 - second) / first.2,
            _ => 1.0,
        };
        Some(Self::to_detected(first, margin))
    }

    fn detect_multiple(&self, text: &str) -> Vec<DetectedLanguage> {
        let (_, scored) = self.score(text);
        let top = match scored.first() {
            Some(&(_, _, r)) => r,
            None => return Vec::new(),
        };
        scored
            .iter()
            .enumerate()
            .map(|(i, &entry)| {
                let margin = if i == 0 {
                    match scored.get(1) {
                        Some(&(_, _, second)) if top > 0.0 => (top - second) / top,
                        _ => 1.0,
                    }
                } else {
                    0.0
                };
                Self::to_detected(entry, margin)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap as StdHashMap;
    use std::sync::Arc;

    use crate::signal::Signal;
    use crate::signals::language::LanguageSignal;
    use crate::types::{ClassificationContext, SignalType};

    const EN: &str = "The quick brown fox jumps over the lazy dog and then it was clear that \
        there are not many things which you can do about this when the weather is bad.";
    const ES: &str = "El perro corre por el parque cuando hace buen tiempo y las personas que \
        están en la ciudad también salen para disfrutar más del sol porque es muy agradable.";
    const FR: &str = "Les enfants qui jouent dans le parc sont très heureux avec leurs amis mais \
        il faut que nous rentrions à la maison pour le dîner comme tous les soirs.";
    const DE: &str = "Der Hund läuft durch den Park und die Kinder sind auch sehr glücklich mit \
        ihren Freunden aber wenn das Wetter nicht gut ist werden sie nach Hause gehen.";
    const PT: &str = "Os meninos que estão no parque são muito felizes com os seus amigos mas \
        quando não faz bom tempo eles vão para casa porque isso também é importante.";
    const IT: &str = "I bambini che giocano nel parco sono molto felici con i loro amici ma \
        quando il tempo non è buono devono tornare a casa perché anche questo è importante.";

    fn det() -> StopwordLanguageDetector {
        StopwordLanguageDetector::new()
    }

    fn make_ctx(text: &str) -> ClassificationContext {
        ClassificationContext {
            text: text.to_string(),
            history: vec![],
            headers: StdHashMap::new(),
            image_url: None,
            config: StdHashMap::new(),
        }
    }

    // -- True positives -----------------------------------------------------

    #[test]
    fn detects_each_supported_language() {
        let d = det();
        for (text, want) in [
            (EN, "en"),
            (ES, "es"),
            (FR, "fr"),
            (DE, "de"),
            (PT, "pt"),
            (IT, "it"),
        ] {
            let got = d.detect(text).expect("should detect");
            assert_eq!(got.code, want, "for {text:?} got {got:?}");
            assert!(got.confidence > 0.5 && got.confidence <= 0.95);
            assert!(!got.name.is_empty());
        }
    }

    #[test]
    fn confidence_never_claims_certainty() {
        let d = det();
        for text in [EN, ES, FR, DE, PT, IT] {
            assert!(d.detect(text).unwrap().confidence <= 0.95);
        }
    }

    #[test]
    fn supported_codes_matches_tables() {
        assert_eq!(
            StopwordLanguageDetector::supported_codes(),
            vec!["en", "es", "fr", "de", "pt", "it"]
        );
    }

    // -- True negatives -----------------------------------------------------

    #[test]
    fn empty_and_whitespace_yield_none() {
        let d = det();
        assert!(d.detect("").is_none());
        assert!(d.detect("   \n\t ").is_none());
        assert!(d.detect_multiple("").is_empty());
    }

    #[test]
    fn unsupported_scripts_yield_none_not_a_wrong_guess() {
        let d = det();
        for text in [
            "これは日本語のテキストです。今日はとても良い天気ですね。",
            "这是一段中文文本，今天天气非常好，我们去公园散步吧。",
            "Это русский текст, сегодня очень хорошая погода на улице.",
            "هذا نص باللغة العربية والطقس اليوم جميل جدا في المدينة",
        ] {
            assert!(d.detect(text).is_none(), "wrong guess for {text:?}");
            assert!(d.detect_multiple(text).is_empty());
        }
    }

    #[test]
    fn short_input_below_hit_floor_yields_none() {
        let d = det();
        // One stopword hit is below MIN_HITS.
        assert!(d.detect("the").is_none());
        assert!(d.detect("hello").is_none());
    }

    #[test]
    fn digits_and_symbols_only_yield_none() {
        let d = det();
        assert!(d.detect("1234 5678 !!! ??? ---").is_none());
    }

    #[test]
    fn source_code_is_not_confidently_a_natural_language() {
        let d = det();
        let code = "fn main() { let x: Vec<u32> = vec![1,2,3]; println!(\"{:?}\", x); }";
        // Documented limitation: code has almost no function words.
        if let Some(l) = d.detect(code) {
            panic!("code misclassified as {}", l.code);
        }
    }

    // -- detect_multiple ----------------------------------------------------

    #[test]
    fn detect_multiple_is_sorted_descending() {
        let d = det();
        let mixed = format!("{EN} {ES}");
        let all = d.detect_multiple(&mixed);
        assert!(all.len() >= 2, "{all:?}");
        for w in all.windows(2) {
            assert!(w[0].confidence >= w[1].confidence, "{all:?}");
        }
    }

    #[test]
    fn detect_multiple_surfaces_code_switching() {
        let d = det();
        let mixed = format!("{FR} {DE}");
        let codes: Vec<String> = d
            .detect_multiple(&mixed)
            .into_iter()
            .map(|l| l.code)
            .collect();
        assert!(codes.contains(&"fr".to_string()), "{codes:?}");
        assert!(codes.contains(&"de".to_string()), "{codes:?}");
    }

    #[test]
    fn detect_multiple_first_matches_detect() {
        let d = det();
        for text in [EN, ES, FR, DE, PT, IT] {
            let primary = d.detect(text).unwrap();
            let all = d.detect_multiple(text);
            assert_eq!(all[0].code, primary.code);
            assert!((all[0].confidence - primary.confidence).abs() < 1e-9);
        }
    }

    #[test]
    fn monolingual_text_beats_its_neighbours() {
        let d = det();
        let all = d.detect_multiple(EN);
        assert_eq!(all[0].code, "en");
        // A clean win should outscore any runner-up.
        if all.len() > 1 {
            assert!(all[0].confidence > all[1].confidence);
        }
    }

    // -- Unicode ------------------------------------------------------------

    #[test]
    fn accented_forms_are_matched_not_split() {
        let d = det();
        // "está", "también", "más" carry accents and must tokenize intact.
        let got = d.detect(ES).unwrap();
        assert_eq!(got.code, "es");
    }

    #[test]
    fn emoji_and_punctuation_do_not_panic() {
        let d = det();
        let text = format!("🚀🎉 {EN} — «quoted» … 中文混在");
        assert_eq!(d.detect(&text).unwrap().code, "en");
    }

    #[test]
    fn case_is_normalised() {
        let d = det();
        assert_eq!(d.detect(&EN.to_uppercase()).unwrap().code, "en");
    }

    // -- Through the real Signal --------------------------------------------

    #[tokio::test]
    async fn wired_into_language_signal() {
        let signal = LanguageSignal::new("lang", Arc::new(det()));
        let result = signal.evaluate(&make_ctx(FR)).await.unwrap();

        assert_eq!(result.signal_type, SignalType::Language);
        assert_eq!(result.labels, vec!["fr"]);
        assert!(result.confidence > 0.5);
        let primary = result.metadata["primary_language"].as_object().unwrap();
        assert_eq!(primary["name"].as_str().unwrap(), "French");
    }

    #[tokio::test]
    async fn signal_reports_nothing_for_unsupported_script() {
        let signal = LanguageSignal::new("lang", Arc::new(det()));
        let result = signal
            .evaluate(&make_ctx(
                "これは日本語のテキストです。今日はとても良い天気ですね。",
            ))
            .await
            .unwrap();
        assert!(result.labels.is_empty());
        assert_eq!(result.confidence, 0.0);
        assert_eq!(result.metadata["language_count"].as_u64().unwrap(), 0);
    }
}

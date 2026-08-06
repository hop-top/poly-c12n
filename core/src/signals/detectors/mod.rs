//! Tier-1 detector implementations.
//!
//! Every detector in this module is a **baseline**: dependency-free,
//! deterministic, fast, and explicitly *not* a substitute for a trained model
//! or a compliance-grade service. Each type's rustdoc states plainly what it
//! catches, what it misses, and how its confidence values were chosen.
//!
//! They exist so the [`crate::signals`] traits have a real implementation out
//! of the box. Swap in a stronger detector by implementing the same trait —
//! the signals take `Box<dyn Trait>` / `Arc<dyn Trait>`, not these types.

pub mod jailbreak_heuristic;
pub mod language_stopword;
pub mod pii_regex;
pub mod tokenizer_approx;

pub use jailbreak_heuristic::HeuristicJailbreakDetector;
pub use language_stopword::StopwordLanguageDetector;
pub use pii_regex::RegexPiiDetector;
pub use tokenizer_approx::ApproxTokenizer;

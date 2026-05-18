use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use criterion::{
    black_box, criterion_group, criterion_main, BenchmarkId, Criterion,
};

use c12n_core::embedding::cosine_similarity;
use c12n_core::pipeline::Pipeline;
use c12n_core::prototype::PrototypeBank;
use c12n_core::signal::Signal;
use c12n_core::signals::keyword::{
    KeywordRule, KeywordSignal, MatchOperator, MatchStrategy,
};
use c12n_core::types::{
    ClassificationContext, SignalError, SignalResult, SignalType,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Deterministic pseudo-random f32 vector (LCG seeded).
fn rand_vec(dim: usize, seed: u64) -> Vec<f32> {
    let mut state = seed;
    (0..dim)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = state as u32;
            (value as f32) / (u32::MAX as f32) * 2.0 - 1.0
        })
        .collect()
}

fn make_ctx(text: &str) -> ClassificationContext {
    ClassificationContext {
        text: text.into(),
        history: vec![],
        headers: HashMap::new(),
        image_url: None,
        config: HashMap::new(),
    }
}

fn repeat_char(ch: char, len: usize) -> String {
    let word = format!("{ch}bcd efgh ");
    word.repeat((len / word.len()) + 1)[..len].to_string()
}

// ---------------------------------------------------------------------------
// Mock signal (instant return)
// ---------------------------------------------------------------------------

struct InstantSignal {
    label: String,
}

#[async_trait]
impl Signal for InstantSignal {
    async fn evaluate(
        &self,
        _ctx: &ClassificationContext,
    ) -> Result<SignalResult, SignalError> {
        Ok(SignalResult {
            name: self.label.clone(),
            signal_type: SignalType::Custom,
            confidence: 0.5,
            labels: vec![self.label.clone()],
            metadata: HashMap::new(),
        })
    }

    fn name(&self) -> &str {
        &self.label
    }

    fn signal_type(&self) -> SignalType {
        SignalType::Custom
    }
}

// ---------------------------------------------------------------------------
// Benchmark 1: cosine_similarity
// ---------------------------------------------------------------------------

fn bench_cosine_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("cosine_similarity");

    for &dim in &[384, 768, 1536] {
        let a = rand_vec(dim, 42);
        let b = rand_vec(dim, 99);

        group.bench_with_input(
            BenchmarkId::from_parameter(dim),
            &dim,
            |bencher, _| {
                bencher.iter(|| {
                    black_box(cosine_similarity(
                        black_box(&a),
                        black_box(&b),
                    ))
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 2: prototype_scoring
// ---------------------------------------------------------------------------

fn bench_prototype_scoring(c: &mut Criterion) {
    let mut group = c.benchmark_group("prototype_scoring");
    let dim = 384;

    for &count in &[10, 50, 100] {
        let protos: Vec<Vec<f32>> =
            (0..count).map(|i| rand_vec(dim, 1000 + i)).collect();
        let weights = vec![1.0_f32; count as usize];
        let bank = PrototypeBank::new(protos, weights, 0.6, 5).unwrap();
        let query = rand_vec(dim, 777);

        group.bench_with_input(
            BenchmarkId::from_parameter(count),
            &count,
            |bencher, _| {
                bencher.iter(|| {
                    black_box(bank.score(black_box(&query)).unwrap())
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 3: pipeline_latency
// ---------------------------------------------------------------------------

fn bench_pipeline_latency(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("pipeline_latency");
    let ctx = make_ctx("benchmark input text");

    for &n in &[1, 5, 10] {
        let signals: Vec<Box<dyn Signal>> = (0..n)
            .map(|i| -> Box<dyn Signal> {
                Box::new(InstantSignal {
                    label: format!("s{i}"),
                })
            })
            .collect();

        let pipeline = Pipeline::new(signals, 10, Duration::from_secs(5));

        group.bench_with_input(
            BenchmarkId::from_parameter(n),
            &n,
            |bencher, _| {
                bencher.iter(|| {
                    rt.block_on(pipeline.evaluate(black_box(&ctx)))
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------
// Benchmark 4: keyword_throughput
// ---------------------------------------------------------------------------

fn bench_keyword_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let rules: Vec<KeywordRule> = (0..10)
        .map(|i| KeywordRule {
            label: format!("rule_{i}"),
            patterns: vec![format!(r"pattern_{i}\b")],
            operator: MatchOperator::Or,
            strategy: MatchStrategy::Regex,
            threshold: 0.5,
        })
        .collect();

    let signal = KeywordSignal::new("bench_kw", rules);

    let mut group = c.benchmark_group("keyword_throughput");

    for &len in &[100, 1000, 10000] {
        let text = repeat_char('x', len);
        let ctx = make_ctx(&text);

        group.bench_with_input(
            BenchmarkId::from_parameter(len),
            &len,
            |bencher, _| {
                bencher.iter(|| {
                    rt.block_on(signal.evaluate(black_box(&ctx))).unwrap()
                });
            },
        );
    }

    group.finish();
}

// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    bench_cosine_similarity,
    bench_prototype_scoring,
    bench_pipeline_latency,
    bench_keyword_throughput,
);
criterion_main!(benches);

# hop-top-c12n

Idiomatic Rust SDK over the c12n classification engine
([`c12n-core`](https://github.com/hop-top/poly-c12n/tree/main/core)).

> [!WARNING]
> **Alpha — API and tag history may break.** Ships on the
> `c12n-rs/v*` alpha line. Pin to exact tags, not ranges.

## What this crate adds over `c12n-core`

`c12n-core` is the engine — raw classification primitives plus a C ABI
for FFI consumers (Go cgo, Python PyO3, PHP FFI, TS WASM).

This crate (`hop-top-c12n`) wraps the engine with:

- [`PipelineConfig`] + [`PipelineConfigBuilder`] — typed config struct
  mirroring the Go binding's shape.
- [`SdkPipeline`] — thin wrapper around `c12n_core::Pipeline` with
  structured [`tracing`](https://docs.rs/tracing) lifecycle events.
- Re-exports of every engine type so consumers can `use
  hop_top_c12n::{Pipeline, ClassificationContext, ...}` without
  importing `c12n_core` directly. This includes the built-in signal
  implementations (`hop_top_c12n::signals::*`) and `async_trait`, so a
  real pipeline — and custom `Signal` impls — can be built against this
  crate alone.

## Install

```toml
[dependencies]
# pin to the exact latest alpha — see https://crates.io/crates/hop-top-c12n
hop-top-c12n = "=<latest-alpha>"
```

## Quickstart

```rust
use hop_top_c12n::signals::keyword::{
    KeywordRule, KeywordSignal, MatchOperator, MatchStrategy,
};
use hop_top_c12n::{ClassificationContext, PipelineConfig, SdkPipeline};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let config = PipelineConfig::builder()
        .max_concurrency(8)
        .timeout(Duration::from_secs(5))
        .build();

    let keyword = KeywordSignal::new(
        "language",
        vec![KeywordRule {
            label: "python".to_string(),
            // `Regex` patterns are matched verbatim — opt into
            // case-insensitivity explicitly with `(?i)`.
            patterns: vec!["(?i)python".to_string()],
            operator: MatchOperator::Or,
            strategy: MatchStrategy::Regex,
            threshold: 0.5,
        }],
    );

    let pipeline = SdkPipeline::new(vec![Box::new(keyword)], config);
    let ctx = ClassificationContext {
        text: "Write a Python function".to_string(),
        ..Default::default()
    };
    let result = pipeline.evaluate(&ctx).await;
    println!("{:?}", result);
}
```

## Roadmap

- **Full kit-rs integration** — gated on the kit-rs surface growing
  logging/output/cli equivalents to kit-go. See the
  `kit-rs-surface-followup` track.
- **Signal-builder DSL** — fluent registration of signals at config
  time. Currently signals are passed as `Vec<Box<dyn Signal>>` at
  construction.

## License

[MIT](https://github.com/hop-top/poly-c12n/blob/main/LICENSE)

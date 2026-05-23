# hop-top-c12n

Idiomatic Rust SDK over the c12n classification engine
([`c12n-core`](../core/)).

> [!WARNING]
> **Alpha — API and tag history may break.** First published tag is
> `c12n-rs/v0.1.0-alpha.0`. Pin to exact tags, not ranges.

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
  importing `c12n_core` directly.

## Install

```toml
[dependencies]
hop-top-c12n = "0.1.0-alpha.0"
```

## Quickstart

```rust
use hop_top_c12n::{SdkPipeline, PipelineConfig, ClassificationContext};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let config = PipelineConfig::builder()
        .max_concurrency(8)
        .timeout(Duration::from_secs(5))
        .build();

    let pipeline = SdkPipeline::new(vec![/* signals */], config);
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

TBD

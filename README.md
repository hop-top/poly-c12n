# poly-c12n

LLM request classification engine for intelligent model routing.

> [!WARNING]
> **Alpha — API and tag history may break.** First published line is
> `<component>/v0.1.x-alpha.*`. Pin to exact tags, not ranges. Breaking
> changes may land between alpha tags; see [`CHANGELOG.md`](CHANGELOG.md).

## What

c12n analyzes LLM requests via 20 signal types so callers can route
them to the right model — cheap models for simple queries, capable
models for complex ones.

Polyglot monorepo. The canonical repo is **`hop-top/poly-c12n`**; each
language sub-tree mirrors to a read-only repo for that language's
ecosystem:

| Dir | Mirror | Registry artifact | Tag prefix |
|---|---|---|---|
| `core/` | `hop-top/c12n-core` | crates.io `hop-top-c12n-core` (engine) | `c12n-core/v*` |
| `go/` | `hop-top/c12n` | Go module `hop.top/c12n` | `c12n/v*` |
| `rs/` | `hop-top/c12n-rs` | crates.io `hop-top-c12n` (SDK) | `c12n-rs/v*` |
| `py/` | `hop-top/c12n-py` | PyPI `hop-top-c12n` | `c12n-py/v*` |
| `ts/` | `hop-top/c12n-ts` | npm `@hop-top/c12n` | `c12n-ts/v*` |
| `php/` | `hop-top/c12n-php` | packagist `hop-top/c12n` | `c12n-php/v*` |

Each language ships **independently** — no linked-versions group.

## Layout

```
poly-c12n/
├── core/         Rust execution engine (lib + cdylib) → libc12n_core.{so,dylib,dll}
│   ├── src/signal.rs       Signal trait
│   ├── src/pipeline.rs     Fan-out / fan-in orchestrator
│   ├── src/embedding.rs    EmbeddingEngine trait + cosine sim
│   ├── src/prototype.rs    PrototypeBank scoring
│   ├── src/ffi.rs          C ABI (JSON in/out)
│   ├── src/wasm.rs         #[wasm_bindgen] surface (for c12n-ts)
│   ├── src/signals/        15 signal implementations
│   └── include/libc12n_core.h   cbindgen-generated header (PHP FFI consumes)
├── go/           Go bindings (hop.top/c12n) — cgo + stub modes
│   ├── *.go                Pipeline, ClassificationContext, Result
│   └── cmd/c12n/           CLI binary
├── rs/           Rust SDK (hop-top-c12n) — ergonomic layer over core
├── py/           Python bindings (hop-top-c12n) — PyO3 + pure Python
├── ts/           TypeScript bindings (@hop-top/c12n) — WASM via wasm-bindgen
├── php/          PHP bindings (hop-top/c12n) — FFI over libc12n_core
├── docs/         ADRs, personas, stories
└── (Cargo workspace at root: core/ + py/ + rs/)
```

## Install (per language)

### Go

```bash
go get hop.top/c12n@latest
```

Two build modes:
- **stub** (`CGO_ENABLED=0`): types + config + CLI work; `Pipeline.Evaluate`
  returns `errNoCgo`.
- **cgo** (`CGO_ENABLED=1`): links `libc12n_core.{so,dylib,dll}` from the
  Rust engine. Real classification.

### Rust (SDK)

```toml
[dependencies]
hop-top-c12n = "0.1.0-alpha.0"
```

### Rust (engine — direct, advanced)

```toml
[dependencies]
hop-top-c12n-core = "0.1.0-alpha.0"
```

### Python

```bash
pip install hop-top-c12n
```

### TypeScript / JavaScript

```bash
npm install @hop-top/c12n
```

### PHP

```bash
composer require hop-top/c12n
```

PHP 8.1+ with `ext-ffi` required.

## Quickstart (Go)

```go
import "hop.top/c12n"

pipeline, _ := c12n.NewPipeline(c12n.PipelineConfig{
    MaxConcurrency: 8,
    Timeout:        5 * time.Second,
})
defer pipeline.Close()

result, _ := pipeline.Evaluate(c12n.ClassificationContext{
    Text: "Write a Python function to sort a list",
})
```

## Build (monorepo)

```bash
make build       # cargo build --workspace + go build
make test        # cargo test + go test + pytest
make check       # lint + test (CI gate)
```

## Signals (20 types, 15 implemented)

| Category  | Signals                                  |
|-----------|------------------------------------------|
| Core      | Keyword, Embedding, Domain               |
| Safety    | Jailbreak, PII, Toxicity                 |
| Analysis  | Context, Structure, Language, Complexity |
| Routing   | Preference, Feedback                     |
| Detection | OutputFormat, CodeContent, ToolCalling   |
| Cost      | CostEstimate                             |
| Reserved  | Sentiment, Intent, Topic, Custom         |

## Toolchain

- Go 1.26+
- Rust 1.85+
- Python 3.9+
- Node 20+
- PHP 8.3+

## Docs

- [ADR-0001: c12n-ts WASM binding](docs/adr/0001-c12n-ts-wasm-binding.md)
- [ADR-0002: c12n-php FFI binding](docs/adr/0002-c12n-php-ffi-binding.md)
- [Personas](docs/personas/README.md)
- [Stories](docs/stories/README.md)

## License

TBD

# c12n

LLM request classification engine for intelligent model routing.

> [!WARNING]
> **Alpha — API and tag history may break.** First published line is
> `c12n*/v0.1.x-alpha.*`. Pin to exact tags, not ranges. Breaking
> changes may land between alpha tags; see [`CHANGELOG.md`](CHANGELOG.md).

## What

c12n classifies LLM requests via 20 signal types so callers can
route requests to the right model — cheap models for simple
queries, capable models for complex ones.

Polyglot monorepo: Rust execution engine, Go + Python bindings, Pkl
config schema-of-record.

## Layout

```
config.pkl          Schema-of-record (Pkl) → embedded YAML defaults
c12n-core/          Rust execution engine (lib + cdylib)
  src/signal.rs     Signal trait
  src/pipeline.rs   Fan-out/fan-in orchestrator
  src/embedding.rs  EmbeddingEngine trait + cosine sim
  src/prototype.rs  PrototypeBank scoring
  src/ffi.rs        C ABI (JSON in/out)
  src/signals/      15 signal implementations (20 types total)
*.go                Go bindings (hop.top/c12n) — cgo + stub modes
cmd/c12n/           Go CLI binary
c12n-py/            Python bindings (PyO3 + pure Python middleware)
```

## Install

### Go

```bash
go get hop.top/c12n@latest
```

Two build modes:

- **stub** (`CGO_ENABLED=0`): types, config, parsing, and CLI all
  work; `Pipeline.Evaluate` returns `errNoCgo`. Useful for tooling
  that consumes types without needing the engine.
- **cgo** (`CGO_ENABLED=1`): links `libc12n_core.{so,dylib}` from
  the Rust core. Real classification.

### Python

```bash
pip install c12n
# or for development:
cd c12n-py && maturin develop
```

### Rust

```toml
# Cargo.toml
[dependencies]
c12n-core = "0.1.0-alpha.0"
```

## Quick start

### Go

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
// result.Signal(c12n.SignalCodeContent), result.Confidence(), ...
```

### Python

```python
from c12n import Pipeline

pipeline = Pipeline(max_concurrency=8, timeout_ms=5000)
result = pipeline.evaluate("Write a Python function to sort a list")
print(result.json())
```

### CLI

```bash
c12n classify "Write a Python function"   # classify text
c12n bench --iterations 100               # benchmark pipeline
c12n init                                 # initialize config
c12n doctor                               # diagnose environment
```

## Build

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

## Ecosystem dependencies

| Package      | Purpose                                  |
|--------------|------------------------------------------|
| hop.top/kit  | CLI, config, logging, output (Go)        |
| hop.top/xrr  | Record/replay test cassettes (Go)        |

## Status

- **Modules:**
  - `hop.top/c12n` (Go, tag `c12n/v*`)
  - `c12n-core` (Rust, tag `c12n-core/v*`)
  - `c12n` PyPI (Python, tag `c12n-py/v*`)
- **First published tags:** `c12n/v0.1.0-alpha.0`,
  `c12n-core/v0.1.0-alpha.0`, `c12n-py/v0.1.0-alpha.0` (linked
  versions — bump together)
- **Toolchain:** Go 1.26+, Rust 1.85+, Python 3.9+

## Docs

- [Personas](docs/personas/README.md) — who uses c12n
- [Stories](docs/stories/README.md) — user stories with linked tests

## License

TBD

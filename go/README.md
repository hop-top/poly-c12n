# hop.top/c12n

Go bindings over the c12n classification engine (`c12n-core`, written in
Rust). Classify LLM requests by keyword, safety, complexity, code content,
and more.

> [!NOTE]
> **Read-only mirror.** This repo (`hop-top/c12n`) is a subtree mirror of
> the polyglot monorepo. File issues and pull requests against the
> canonical source, [`hop-top/poly-c12n`](https://github.com/hop-top/poly-c12n)
> — changes made here are overwritten on the next mirror push.

> [!WARNING]
> **Alpha — API and tag history may break between alpha tags.** Pin to an
> exact tag, not a range.

## Install

```bash
go get hop.top/c12n@latest
```

Or pin to the latest published alpha tag (recommended for reproducible
builds):

```bash
go get hop.top/c12n@vX.Y.Z-alpha.N
```

Requires Go 1.26+.

## Build modes

The bindings ship in two flavours, selected by the opt-in `c12n_native`
build tag:

| Mode | Build tag | Behaviour |
|------|-----------|-----------|
| **stub** (default) | _(none)_ | Pure Go, regardless of `CGO_ENABLED`. Types, config loading (`Config`, `LoadConfig`), result parsing, and the CLI all work. `NewPipeline` and `Pipeline.Evaluate` return an error — no real classification. |
| **native** | `c12n_native` | Links `libc12n_core.{so,dylib,dll}` from the Rust engine via cgo and performs real classification. |

The stub is the default even when cgo is enabled, so module-proxy
consumers (including projects that need cgo for other dependencies, e.g.
sqlite) build without the Rust library present. Opt in to the native
engine when you need actual scoring:

```bash
# build the Rust cdylib first (produces target/debug/libc12n_core.*)
cargo build

# real classification — requires cgo and libc12n_core on the linker path
go build -tags c12n_native ./...
```

The native build links against `libc12n_core` via
`-L${SRCDIR}/../target/debug -lc12n_core` and requires `CGO_ENABLED=1`
(the default on most toolchains).

## Quickstart

```go
package main

import (
	"fmt"
	"log"
	"time"

	c12n "hop.top/c12n"
)

func main() {
	// Configure and construct the pipeline. Requires -tags c12n_native;
	// under the default stub build NewPipeline returns an error.
	p, err := c12n.NewPipeline(c12n.PipelineConfig{
		MaxConcurrency: 8,
		Timeout:        5 * time.Second,
	})
	if err != nil {
		log.Fatal(err)
	}
	defer p.Close()

	// Evaluate returns the raw JSON result from the engine.
	raw, err := p.Evaluate(c12n.ClassificationContext{
		Text:    "Write a Python function to sort a list.",
		History: []string{},
		Headers: map[string]string{},
	})
	if err != nil {
		log.Fatal(err)
	}

	// Parse it into a typed result and inspect signals.
	result, err := c12n.ParseResult(raw)
	if err != nil {
		log.Fatal(err)
	}

	fmt.Printf("evaluated in %s\n", result.Duration())
	if result.HasSignal(c12n.SignalCodeContent) {
		fmt.Printf("code content confidence: %.2f\n",
			result.Confidence(c12n.SignalCodeContent))
	}
	for _, s := range result.Results {
		fmt.Printf("  %s (%s): %.2f %v\n", s.Name, s.Type, s.Confidence, s.Labels)
	}
}
```

### Loading config from files

`LoadConfig` layers defaults, then user/project YAML, then converts to a
`PipelineConfig`:

```go
cfg, err := c12n.LoadConfig(config.Options{
	UserConfigPath:    "~/.config/c12n/config.yaml",
	ProjectConfigPath: ".c12n.yaml",
})
if err != nil {
	log.Fatal(err)
}
p, err := c12n.NewPipeline(cfg.ToPipelineConfig())
```

## CLI

A `c12n` binary lives at `cmd/c12n`:

```bash
go install hop.top/c12n/cmd/c12n@latest
```

Subcommands: `classify`, `config`, `init`, `signals`, `doctor`, `bench`,
`upgrade`, `tip`, `toolspec`.

## API surface

- `NewPipeline(PipelineConfig) (*Pipeline, error)` — construct a pipeline.
- `(*Pipeline).Evaluate(ClassificationContext) (string, error)` — classify,
  returning raw JSON.
- `(*Pipeline).Close()` — free native resources.
- `ParseResult(string) (*PipelineResult, error)` — parse the JSON output.
- `PipelineResult` helpers: `Duration`, `Signal`, `Signals`, `HasSignal`,
  `Confidence`, `HasErrors`.
- `Config`, `DefaultConfig`, `LoadConfig`, `(*Config).ToPipelineConfig`,
  `(*Config).EnabledSignals` — layered configuration.
- `SignalType` constants: `SignalKeyword`, `SignalJailbreak`, `SignalPII`,
  `SignalCodeContent`, `SignalComplexity`, `SignalCostEstimate`, and more.

## License

MIT.

---
status: planned
bindings:
  go: planned
personas: [cost-control-startup]
priority: P1
---

# US-0004: Benchmark classification overhead

As a tool author, I want a built-in bench command so I can measure
the per-request overhead c12n adds before committing to it.

> **Status: planned.** Blocked twice over. The binary panics at startup
> so `c12n bench` cannot run at all
> ([US-0002](US-0002-classify-cli.md)), and even once it does, the
> pipeline it benchmarks has zero signals — so the numbers measure FFI
> round-trip cost, not classification cost
> ([US-0005](US-0005-low-confidence-detection.md)).

## Use this when

- Evaluating c12n against a latency SLO.
- Sizing capacity for production load.

Not yet usable for "comparing classification cost across signal
combinations" — signals are not selectable from any binding.

## Result

Intended: `c12n bench --iterations <N>` runs N evaluations and prints
latency statistics.

Output is six lines, not three — `min`, `max`, `avg`, `p50`, `p95`,
`p99` ([`go/cmd/c12n/bench.go:77-84`](../../go/cmd/c12n/bench.go)).

## Steps

```bash
# default: 100 iterations against the default --text
c12n bench

# custom iteration count
c12n bench --iterations 1000

# custom prompt
c12n bench --text "Write a Python function to sort a list"

# JSONL file of ClassificationContext objects, one per line
c12n bench --iterations 100 --input prompts.jsonl

# concurrent workers (default 1)
c12n bench --iterations 1000 --concurrency 8

# ben-compatible JSONL output
c12n bench --iterations 100 -o baseline.jsonl
```

Flags ([`go/cmd/c12n/bench.go:95-110`](../../go/cmd/c12n/bench.go)):
`--iterations/-n` (default 100), `--text/-t` (default
`"Hello, how are you?"`), `--input`, `--signal/-s`, `--concurrency/-c`
(default 1), `--output/-o`.

`--signal` **does not filter the pipeline** — it only tags the ben
JSONL output. The command's own help says so
([`go/cmd/c12n/bench.go:44-45`](../../go/cmd/c12n/bench.go)); the flag
value is discarded with `_ = signal` in `runBench`
([`go/cmd/c12n/bench.go:207`](../../go/cmd/c12n/bench.go)).

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestBenchPercentile ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EBenchIterationsFlag ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EBenchAllFlags ./cmd/c12n
```

All pass. `TestBenchPercentile_*` unit-tests the `benchPercentile`
index arithmetic on synthetic duration slices; the `TestE2EBench*Flag`
pair asserts flag registration on a `newTestRoot()` tree. None of them
runs a benchmark, and none runs the binary.

## How it works

[`go/cmd/c12n/bench.go`](../../go/cmd/c12n/bench.go) pulls the pipeline
from the cobra context, resolves input contexts from `--text` or
`--input`, spawns `--iterations` goroutines bounded by a
`--concurrency`-sized semaphore, records each `Evaluate` duration, sorts,
and reports. Any iteration error aborts the whole run.

`benchPercentile` uses `idx = (p * (len-1)) / 100` — nearest-rank on
the low side, no interpolation. Empty input returns `0`.

In stub builds `Evaluate` returns `errNativeDisabled` on the first
iteration, so `bench` fails rather than measuring anything.

## What this story needs to reach `shipped`

1. Startup panic fixed (US-0002).
2. Signals registered, so the measurement includes classification
   (US-0005).
3. A test that executes a real short benchmark end to end.

## Tests

- [`go/cmd/c12n/bench_regressions_test.go:TestBenchPercentile_P50_Returns50thElement`](../../go/cmd/c12n/bench_regressions_test.go)
- [`go/cmd/c12n/bench_regressions_test.go:TestBenchPercentile_P95_P99`](../../go/cmd/c12n/bench_regressions_test.go)
- [`go/cmd/c12n/bench_regressions_test.go:TestBenchPercentile_EmptySlice`](../../go/cmd/c12n/bench_regressions_test.go)
- [`go/cmd/c12n/bench_regressions_test.go:TestLoadJSONLInputs_LargeLine`](../../go/cmd/c12n/bench_regressions_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EBenchIterationsFlag`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EBenchAllFlags`](../../go/cmd/c12n/e2e_test.go)
- [`go/review_regressions_test.go:TestBenchCommand_ZeroIterations`](../../go/review_regressions_test.go)
  — rejects `--iterations 0`.

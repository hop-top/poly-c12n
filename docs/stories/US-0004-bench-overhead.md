---
status: partial
bindings:
  go: partial
personas: [cost-control-startup]
priority: P1
---

# US-0004: Benchmark classification overhead

As a tool author, I want a built-in bench command so I can measure
the per-request overhead c12n adds before committing to it.

> **Status: partial.** `c12n bench` runs and reports real latency
> statistics. But the pipeline it benchmarks has zero signals
> registered, so the numbers measure FFI round-trip cost, not
> classification cost ([US-0002](US-0002-classify-cli.md)). Treat them
> as a floor, never as a capacity estimate.

## Use this when

- Establishing the FFI-boundary floor for a latency budget.

Not yet usable for "sizing capacity for production load" or
"comparing classification cost across signal combinations" — the work
being timed does not include any detector.

## Result

`c12n bench --iterations <N>` runs N evaluations and prints six lines
— `min`, `max`, `avg`, `p50`, `p95`, `p99`
([`go/cmd/c12n/bench.go`](../../go/cmd/c12n/bench.go)).

Actual output, native build:

```console
$ c12n bench --iterations 20
iterations: 20
min:        1.125µs
max:        407.75µs
avg:        22.929µs
p50:        2.042µs
p95:        10.292µs
p99:        10.292µs
```

Microsecond-scale figures are the tell: this is a JSON marshal, a C
ABI hop, and a JSON unmarshal over an empty signal set. A pipeline
with detectors registered would not look like this.

The `-o` flag writes ben-compatible JSONL:

```console
$ c12n bench --iterations 5 -o baseline.jsonl
$ head -2 baseline.jsonl
{"candidate":"c12n","metric":"latency_max","value":0.471,"unit":"ms","tags":{"signal":"all"}}
{"candidate":"c12n","metric":"latency_avg","value":0.096,"unit":"ms","tags":{"signal":"all"}}
```

In a stub build `bench` exits 1 on the first iteration with
`pipeline not available: c12n: native engine disabled`.

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

# concurrent workers (default 1) — long form only, see below
c12n bench --iterations 1000 --concurrency 8

# ben-compatible JSONL output
c12n bench --iterations 100 -o baseline.jsonl
```

Flags ([`go/cmd/c12n/bench.go`](../../go/cmd/c12n/bench.go)):
`--iterations/-n` (default 100), `--text/-t` (default
`"Hello, how are you?"`), `--input`, `--signal/-s`, `--concurrency`
(default 1), `--output/-o`.

### `--concurrency` has no `-c` shorthand

kit reserves `-c`/`--config` as a global persistent flag. `bench -c 2`
does **not** set concurrency — it is parsed as a config token and
fails:

```console
$ c12n bench --iterations 4 -c 2
Error: -c "2": not a key=value pair and no such file
```

The shorthand was removed rather than reassigned. Use `--concurrency`.

`--signal` **does not filter the pipeline** — it only tags the ben
JSONL output. The command's own help says so; the flag value is
discarded with `_ = signal` in `runBench`.

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestBenchPercentile ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EBenchIterationsFlag ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EBenchAllFlags ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestRealRootExecutesSubcommandHelp ./cmd/c12n
```

All pass. `TestBenchPercentile_*` unit-tests the `benchPercentile`
index arithmetic on synthetic duration slices; the `TestE2EBench*Flag`
pair asserts flag registration on a `newTestRoot()` tree.
`TestRealRootExecutesSubcommandHelp` executes `bench --help` through
the real `newRoot()` tree, proving the command is reachable. None of
them runs an actual benchmark — the output above was captured by hand.

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

1. Signals registered, so the measurement includes classification
   (US-0002).
2. A test that executes a real short benchmark end to end and asserts
   on the reported statistics.

## Tests

- [`go/cmd/c12n/bench_regressions_test.go:TestBenchPercentile_P50_Returns50thElement`](../../go/cmd/c12n/bench_regressions_test.go)
- [`go/cmd/c12n/bench_regressions_test.go:TestBenchPercentile_P95_P99`](../../go/cmd/c12n/bench_regressions_test.go)
- [`go/cmd/c12n/bench_regressions_test.go:TestBenchPercentile_EmptySlice`](../../go/cmd/c12n/bench_regressions_test.go)
- [`go/cmd/c12n/bench_regressions_test.go:TestLoadJSONLInputs_LargeLine`](../../go/cmd/c12n/bench_regressions_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EBenchIterationsFlag`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EBenchAllFlags`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/root_regressions_test.go:TestRealRootExecutesSubcommandHelp`](../../go/cmd/c12n/root_regressions_test.go)
  — `bench --help` through the real root.
- [`go/review_regressions_test.go:TestBenchCommand_ZeroIterations`](../../go/review_regressions_test.go)
  — rejects `--iterations 0`.

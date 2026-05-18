---
status: shipped
personas: [cost-control-startup]
priority: P1
---

# US-0004: Benchmark classification overhead

As a tool author, I want a built-in bench command so I can measure
the per-request overhead c12n adds before committing to it.

## Use this when

- Evaluating c12n against a latency SLO.
- Comparing classification cost across model / signal combinations.
- Sizing capacity for production load.

## Result

`c12n bench --iterations <N>` runs N classifications against a
fixture prompt and prints percentile latencies (p50, p95, p99).

## Steps

```bash
# default iterations
c12n bench

# custom iteration count
c12n bench --iterations 1000

# with input file (JSONL of contexts)
c12n bench --iterations 100 --input prompts.jsonl

# output to file
c12n bench --iterations 100 -o baseline.jsonl
```

## Verify

```bash
CGO_ENABLED=0 go test -run TestBenchPercentile ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EBenchIterationsFlag ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EBenchAllFlags ./cmd/c12n
```

## How it works

`cmd/c12n/bench.go` builds a `Pipeline`, loops N times, captures
per-call durations, then computes percentiles via
`benchPercentile`. Empty-slice and single-element edge cases are
covered.

In stub mode, the bench runs the construct/close lifecycle without
hitting the classifier — useful for measuring c12n's own overhead.

## Tests

- [`cmd/c12n/bench_regressions_test.go:TestBenchPercentile_P50_Returns50thElement`](../../cmd/c12n/bench_regressions_test.go)
- [`cmd/c12n/bench_regressions_test.go:TestBenchPercentile_P95_P99`](../../cmd/c12n/bench_regressions_test.go)
- [`cmd/c12n/bench_regressions_test.go:TestBenchPercentile_EmptySlice`](../../cmd/c12n/bench_regressions_test.go)
- [`cmd/c12n/bench_regressions_test.go:TestLoadJSONLInputs_LargeLine`](../../cmd/c12n/bench_regressions_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EBenchIterationsFlag`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EBenchAllFlags`](../../cmd/c12n/e2e_test.go)

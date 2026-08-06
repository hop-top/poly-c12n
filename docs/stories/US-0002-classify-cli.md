---
status: partial
bindings:
  go: partial
personas: [cost-control-startup, internal-tool-builder]
priority: P0
---

# US-0002: Evaluate a prompt via CLI

As a tool author shipping a c12n-backed product, I want a CLI that
classifies a prompt so I can demo, debug, or script ad-hoc routing
decisions without writing Go code.

> **Status: partial — the binary starts, but classifies nothing.**
>
> The startup panic is fixed: `c12n classify` now runs to completion
> and exits 0 in a native build. What it prints is an empty `results`
> array plus a `NoSignals` diagnostic, because no binding can select
> detectors yet. See [Classification returns
> nothing](#classification-returns-nothing).

## Use this when

- Customer support is debugging "why did this prompt route this way".
- Internal user wants to classify-without-coding.
- CI smoke test: pipe `classify` into a script that asserts shape.

The first two are **not** served today — there is no verdict to
inspect. The third is: the envelope shape is real and stable.

## Result

`c12n classify <prompt>` (or `--stdin` / `--file`) prints a
classification envelope; `--format json` makes it machine-readable
(json is the `classify` default).

Actual output, native build:

```console
$ c12n classify "my email is bob@example.com"
{
  "results": [],
  "errors": [
    "pipeline has no registered signals; results will be empty"
  ],
  "duration_ms": 0
}
$ echo $?
0
```

`--stdin` and `--file` produce the same envelope. Exit status is 0 —
a `NoSignals` diagnostic is not treated as a failure.

In a stub build (no `-tags c12n_native`) `classify` exits 1 instead:

```console
$ c12n classify "hello world"
Error: GENERIC: pipeline not available: c12n: native engine disabled (build with -tags c12n_native)
$ echo $?
1
```

Note the contrast with `doctor` / `status` / `toolspec` / `signals`,
which now succeed in a stub build — pipeline construction is no longer
a precondition for every command
([`go/cmd/c12n/root.go`](../../go/cmd/c12n/root.go), `PersistentPreRunE`).

## Classification returns nothing

`c12n_pipeline_new` builds its pipeline with a hardcoded empty signal
vector ([`core/src/ffi.rs:135`](../../core/src/ffi.rs) — `vec![]`).
`core/src/wasm.rs:105` and `py/src/lib.rs:25` do the same. PR #42
landed real detectors, a name-based registry and tiered chains in the
Rust core, but **not** the config-schema plumbing that would let a
caller name one. So the pipeline the CLI drives has zero signals
registered, and every evaluation returns the `NoSignals` diagnostic.

`go/config.go`'s `ToPipelineConfig()` compounds this: it forwards only
`MaxConcurrency` and `Timeout` out of ~25 config fields, and
`EnabledSignals()` is report-only — `c12n signals list` shows signals
marked `true` that reach nothing.

Until detector selection is plumbed through the FFI, `classify` is a
shape-and-plumbing demo, not a classifier.

## Steps

```bash
# direct prompt
c12n classify "Write a Python function to sort a list"

# from stdin
echo "Write a Python function to sort a list" | c12n classify --stdin

# from file
c12n classify --file prompt.txt

# json output (json is already the default)
c12n classify --format json "..."
```

Flags on `classify` ([`go/cmd/c12n/classify.go`](../../go/cmd/c12n/classify.go)):
`--format/-f` (default `json`), `--signal/-s`, `--min-confidence`,
`--file`, `--stdin`.

`--signal` and `--min-confidence` filter the `results` array. With no
signals registered that array is always empty, so neither flag has an
observable effect today.

## Verify

Real-root execution — these build the shipped command tree through
`newRoot()`, not the bare `newTestRoot()` stub, so they would have
caught the startup panic:

```bash
cd go && CGO_ENABLED=0 go test -run TestRealRootExecutesHelp ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestRealRootExecutesSubcommandHelp ./cmd/c12n
```

Flag registration only, on a `newTestRoot()` tree:

```bash
cd go && CGO_ENABLED=0 go test -run TestE2EClassifyFlagsComplete ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EClassifyStdinFlag ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EClassifyFormatFlag ./cmd/c12n
```

All pass. No test executes the built binary and asserts on `classify`
output; the envelope above was captured by hand.

## How it works

[`go/cmd/c12n/classify.go`](../../go/cmd/c12n/classify.go) resolves text
from args / `--file` / `--stdin`, builds a `ClassificationContext` with
only `Text` set, calls `pipeline.Evaluate`, parses the result, applies
`--signal` and `--min-confidence` filters, and renders via
`hop.top/kit/go/console/output`.

In stub builds `Evaluate` errors with `errNativeDisabled`.

## What this story needs to reach `shipped`

1. Detector selection plumbed through `c12n_pipeline_new`, so
   `results` can be non-empty.
2. `ToPipelineConfig()` forwarding the signal-enabling fields it
   currently drops.
3. A smoke test that runs the built binary and asserts on a non-empty
   `results` array.

## Tests

- [`go/cmd/c12n/root_regressions_test.go:TestRealRootExecutesHelp`](../../go/cmd/c12n/root_regressions_test.go)
  — executes the real `newRoot()` tree; pins the fixed panic.
- [`go/cmd/c12n/root_regressions_test.go:TestRealRootExecutesSubcommandHelp`](../../go/cmd/c12n/root_regressions_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EClassifyFlagsComplete`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EClassifyStdinFlag`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EClassifyFormatFlag`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EClassifyUsage`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/cli_test.go:TestClassifyFlags`](../../go/cmd/c12n/cli_test.go)
- [`go/cmd/c12n/cli_test.go:TestClassifyHelpContent`](../../go/cmd/c12n/cli_test.go)

The `TestE2E*` and `cli_test.go` entries build the tree via
`newTestRoot()` and assert flag registration, not behaviour.

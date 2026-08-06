---
status: planned
bindings:
  go: planned
personas: [cost-control-startup, internal-tool-builder]
priority: P0
---

# US-0002: Evaluate a prompt via CLI

As a tool author shipping a c12n-backed product, I want a CLI that
classifies a prompt so I can demo, debug, or script ad-hoc routing
decisions without writing Go code.

> **Status: planned — the binary does not start.**
>
> Every `c12n` invocation panics during startup, before any subcommand
> runs. See [Known blocker](#known-blocker). The cobra command *tree* is
> wired and tested; the assembled program is not runnable. Nothing in
> this story can be executed today.

## Use this when

- Customer support is debugging "why did this prompt route this way".
- Internal user wants to classify-without-coding.
- CI smoke test: pipe `classify` into a script that asserts shape.

## Result

Intended: `c12n classify <prompt>` (or `--stdin` / `--file`) prints
classification output; `--format json` makes it machine-readable.

Actual: panic at startup, exit code 2.

## Known blocker

`cli.New` already registers a persistent `-c/--config` flag
(`hop.top/kit/go/console/cli/cli.go:474`). `run()` then registers a
second flag with the same name:

```go
// go/cmd/c12n/root.go:68
root.Cmd.PersistentFlags().StringVar(&cfgFlag, "config", "",
    "Path to config file (overrides default locations)")
```

pflag panics on the duplicate:

```
$ c12n classify "hello world"
panic: c12n flag redefined: config
  ...
  main.run(...) go/cmd/c12n/root.go:68
  main.main()   go/cmd/c12n/main.go:13
exit status 2
```

Reproduce:

```bash
cd go && CGO_ENABLED=0 go build -buildvcs=false -o /tmp/c12n ./cmd/c12n
/tmp/c12n classify "hello world"   # panics
/tmp/c12n doctor                   # panics
/tmp/c12n --help                   # panics
```

The CLI test suite does not catch this because every test builds the
tree through `newTestRoot()`
([`go/cmd/c12n/cli_test.go:17-29`](../../go/cmd/c12n/cli_test.go)),
a bare `&cobra.Command{Use: "c12n"}` that deliberately skips `cli.New`
and `PersistentPreRunE`. The tests assert on a command tree that the
shipped binary never assembles. Green CLI tests are therefore not
evidence that any command in this story works.

## Steps

Once the binary starts, the intended surface is:

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

Flags on `classify` ([`go/cmd/c12n/classify.go:79-89`](../../go/cmd/c12n/classify.go)):
`--format/-f` (default `json`), `--signal/-s`, `--min-confidence`,
`--file`, `--stdin`.

Even after the panic is fixed, `classify` will print an empty
`results` array plus a `NoSignals` diagnostic in `errors`, because the
FFI registers no signals ([`core/src/ffi.rs:134`](../../core/src/ffi.rs)).
See [US-0005](US-0005-low-confidence-detection.md).

## Verify

These pass, and they assert flag registration only:

```bash
cd go && CGO_ENABLED=0 go test -run TestE2EClassifyFlagsComplete ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EClassifyStdinFlag ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EClassifyFormatFlag ./cmd/c12n
```

There is no test that executes the real binary. Adding one is the
first step toward `shipped`.

## How it works

[`go/cmd/c12n/classify.go`](../../go/cmd/c12n/classify.go) resolves text
from args / `--file` / `--stdin`, builds a `ClassificationContext` with
only `Text` set, calls `pipeline.Evaluate`, parses the result, applies
`--signal` and `--min-confidence` filters, and renders via
`hop.top/kit/go/console/output`.

In stub builds `Evaluate` errors with `errNativeDisabled`.

## What this story needs to reach `shipped`

1. Remove the duplicate `--config` registration at
   [`go/cmd/c12n/root.go:68`](../../go/cmd/c12n/root.go) (or set
   `cli.Config.Config` to suppress kit's).
2. A smoke test that runs the built binary, not `newTestRoot()`.
3. Signals actually registered, so output is non-empty (US-0005).

## Tests

- [`go/cmd/c12n/e2e_test.go:TestE2EClassifyFlagsComplete`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EClassifyStdinFlag`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EClassifyFormatFlag`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EClassifyUsage`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/cli_test.go:TestClassifyFlags`](../../go/cmd/c12n/cli_test.go)
- [`go/cmd/c12n/cli_test.go:TestClassifyHelpContent`](../../go/cmd/c12n/cli_test.go)

All build the tree via `newTestRoot()`; none exercise `run()`.

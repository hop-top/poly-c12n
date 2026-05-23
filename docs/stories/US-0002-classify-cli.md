---
status: shipped
personas: [cost-control-startup, internal-tool-builder]
priority: P0
---

# US-0002: Evaluate a prompt via CLI

As a tool author shipping a c12n-backed product, I want a CLI that
classifies a prompt so I can demo, debug, or script ad-hoc routing
decisions without writing Go code.

## Use this when

- Customer support is debugging "why did this prompt route this way".
- Internal user wants to classify-without-coding.
- CI smoke test: pipe `classify` into a script that asserts shape.

## Result

`c12n classify <prompt>` (or `--stdin` / `--file`) prints
classification output. `--format json` makes it machine-readable.

## Steps

```bash
# direct prompt
c12n classify "Write a Python function to sort a list"

# from stdin
echo "Write a Python function to sort a list" | c12n classify --stdin

# from file
c12n classify --file prompt.txt

# json output
c12n classify --format json "..."
```

## Verify

```bash
CGO_ENABLED=0 go test -run TestE2EClassifyFlagsComplete ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EClassifyStdinFlag ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EClassifyFormatFlag ./cmd/c12n
```

## How it works

`cmd/c12n/classify.go` parses flags via cobra (`--stdin`, `--file`,
`--format`, `--signal`), reads the prompt, constructs a
`ClassificationContext`, and calls `pipeline.Evaluate`. Output
renders via `kit/output` (JSON or table).

In stub mode, the command surface (flags, help, completion) is
fully wired; `Evaluate` errors with `errNoCgo` if you try to actually
classify.

## Tests

- [`cmd/c12n/e2e_test.go:TestE2EClassifyFlagsComplete`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EClassifyStdinFlag`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EClassifyFormatFlag`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EClassifyUsage`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/cli_test.go:TestClassifyFlags`](../../cmd/c12n/cli_test.go)
- [`cmd/c12n/cli_test.go:TestClassifyHelpContent`](../../cmd/c12n/cli_test.go)

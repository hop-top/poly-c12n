---
status: partial
bindings:
  go: partial
personas: [llm-routing-saas, internal-tool-builder]
priority: P0
---

# US-0006: Emit toolspec JSON for AI-agent discovery

As a tool author wiring c12n into an AI-agent stack, I want a
machine-readable tool spec so the agent can discover c12n's commands,
error patterns, and workflows without me writing adapters by hand.

> **Status: partial.** The spec content is complete and well-formed.
> But it is a hand-authored literal that can drift from the real CLI,
> and it cannot be obtained from the shipped binary, which panics at
> startup ([US-0002](US-0002-classify-cli.md)). Consumers can only get
> it by calling `buildToolSpec()` from Go.

## Use this when

- Integrating c12n with OpenAI / Anthropic / MCP tool-use APIs.
- Building a self-documenting agent that surfaces c12n capabilities.
- Generating SDK stubs from the spec.

## Result

`c12n toolspec` prints an indented JSON document
([`go/cmd/c12n/toolspec.go:16-23`](../../go/cmd/c12n/toolspec.go))
describing:

- Subcommands with flags, safety level, contract, and output schema.
- Error patterns with `Cause` + `Fix`.
- Workflows (`quick-classify`, `full-setup`, `benchmark-compare`).
- State introspection (config commands, env vars).

`toolspec` takes **no flags** and `cobra.NoArgs`. There is no
`--format`; output is always JSON.

The spec omits `toolspec` itself from its `Commands` list — an agent
reading the spec cannot discover the command that produced it.

## Steps

```bash
# the only supported invocation
c12n toolspec

# pipe into an MCP loader
c12n toolspec | mcp-loader register --tool c12n
```

`c12n toolspec --format json` is **not** valid — it errors on the
unknown flag. Drop it.

Both invocations above currently fail at startup for the reason in
[US-0002](US-0002-classify-cli.md).

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecValidJSON ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecContainsAllCommands ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecHasErrorPatterns ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecErrorPatternsHaveFix ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecHasWorkflows ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecWorkflowsHaveSteps ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecStateIntrospection ./cmd/c12n
```

All seven pass. Six call `buildToolSpec()` directly; only
`TestE2EToolspecValidJSON` executes the command, and it does so on a
`newTestRoot()` tree rather than the real binary.

## How it works

`buildToolSpec()` returns a **hand-written `toolspec.ToolSpec`
literal** ([`go/cmd/c12n/toolspec.go:26`](../../go/cmd/c12n/toolspec.go)).
It does not walk the cobra command tree and nothing is derived
automatically — every command name, flag, description, error pattern
and workflow is typed out a second time alongside its real definition.

That is the drift risk in this story. `TestE2EToolspecContainsAllCommands`
checks the spec contains eight names from a hardcoded list; no test
compares the spec against the actual command tree, so a flag added to
`classify.go` will not fail any test until someone notices the spec is
stale.

`toolspec` does not invoke the classifier, so it is unaffected by the
missing-signals problem.

## What this story needs to reach `shipped`

1. Startup panic fixed (US-0002), so the spec is reachable from the CLI.
2. A conformance test asserting the spec matches the live cobra tree
   (command names, flag names) — or generation from the tree.
3. `toolspec` listed in its own `Commands`.

## Tests

- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecValidJSON`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecContainsAllCommands`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecHasErrorPatterns`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecErrorPatternsHaveFix`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecHasWorkflows`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecWorkflowsHaveSteps`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecStateIntrospection`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/cli_test.go:TestToolspecOutputJSON`](../../go/cmd/c12n/cli_test.go)
- [`go/cmd/c12n/cli_test.go:TestToolspecWorkflows`](../../go/cmd/c12n/cli_test.go)
- [`go/cmd/c12n/cli_test.go:TestToolspecStateIntrospection`](../../go/cmd/c12n/cli_test.go)

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

> **Status: partial.** `c12n toolspec` is now reachable from the
> shipped binary and emits well-formed JSON — including in a stub
> build, since it needs no pipeline. But the spec is still a
> hand-authored literal that can drift from the real CLI, and it is
> already stale: two shipped commands are missing from it. See
> [The spec is hand-written and drifting](#the-spec-is-hand-written-and-drifting).

## Use this when

- Integrating c12n with OpenAI / Anthropic / MCP tool-use APIs.
- Building a self-documenting agent that surfaces c12n capabilities.
- Generating SDK stubs from the spec.

For all three, budget for the spec under-reporting the command surface.

## Result

`c12n toolspec` prints an indented JSON document
([`go/cmd/c12n/toolspec.go`](../../go/cmd/c12n/toolspec.go)) with six
top-level keys: `name`, `schema_version`, `commands`,
`error_patterns`, `workflows`, `state_introspection`.

Actual output, head:

```console
$ c12n toolspec
{
  "name": "c12n",
  "schema_version": "dev",
  "commands": [
    {
      "name": "classify",
      "flags": [
        {
          "name": "format",
          "short": "f",
          "type": "string",
          "description": "Output format (json|table|text)"
        },
```

The `commands` list holds exactly eight entries:

```console
$ c12n toolspec | jq -r '.commands[].name' | paste -sd' '
classify config init signals bench upgrade doctor tip
```

`toolspec` takes no flags of its own and `cobra.NoArgs`. Its output is
always JSON regardless of kit's global `--format`.

## The spec is hand-written and drifting

`buildToolSpec()` returns a **hand-written `toolspec.ToolSpec`
literal** ([`go/cmd/c12n/toolspec.go:50`](../../go/cmd/c12n/toolspec.go)).
It does not walk the cobra command tree — every command name, flag,
description, error pattern and workflow is typed out a second time
alongside its real definition.

That drift has already happened. The binary ships eleven top-level
commands; the spec lists eight. Missing:

- `status` — added as a real subcommand, absent from the spec.
- `toolspec` — still omits itself, so an agent reading the spec
  cannot discover the command that produced it.
- `help` — cobra's built-in.

`TestE2EToolspecContainsAllCommands` checks the spec contains eight
names from a hardcoded list, so adding a command to the tree without
adding it to the spec fails nothing. No test compares the spec against
the actual command tree.

`toolspec` does not invoke the classifier, so it is unaffected by the
missing-signals problem in [US-0002](US-0002-classify-cli.md).

## Steps

```bash
# the only supported invocation
c12n toolspec

# pipe into an MCP loader
c12n toolspec | mcp-loader register --tool c12n
```

`c12n toolspec --format json` is accepted — `--format` is one of kit's
global persistent flags — but it changes nothing, because the command
writes JSON directly rather than going through kit's renderer. Prefer
the bare form.

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecValidJSON ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecContainsAllCommands ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecHasErrorPatterns ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecErrorPatternsHaveFix ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecHasWorkflows ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecWorkflowsHaveSteps ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecStateIntrospection ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EToolspecNameMatchesBinary ./cmd/c12n
```

All pass. Seven call `buildToolSpec()` directly; only
`TestE2EToolspecValidJSON` executes the command, and it does so on a
`newTestRoot()` tree rather than the real binary. Because they all read
the same literal the spec is built from, none can detect drift against
the command tree.

## How it works

`buildToolSpec()` constructs and returns the literal; the command
marshals it with `json.MarshalIndent` and writes it to stdout. Nothing
is derived from cobra.

## What this story needs to reach `shipped`

1. A conformance test asserting the spec matches the live cobra tree
   (command names, flag names) — or generation from the tree.
2. `status` and `toolspec` present in `Commands`.

## Tests

- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecValidJSON`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecContainsAllCommands`](../../go/cmd/c12n/e2e_test.go)
  — hardcoded eight-name list; cannot detect drift.
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecHasErrorPatterns`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecErrorPatternsHaveFix`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecHasWorkflows`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecWorkflowsHaveSteps`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecStateIntrospection`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EToolspecNameMatchesBinary`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/cli_test.go:TestToolspecOutputJSON`](../../go/cmd/c12n/cli_test.go)
- [`go/cmd/c12n/cli_test.go:TestToolspecWorkflows`](../../go/cmd/c12n/cli_test.go)
- [`go/cmd/c12n/cli_test.go:TestToolspecStateIntrospection`](../../go/cmd/c12n/cli_test.go)

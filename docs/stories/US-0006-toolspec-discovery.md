---
status: shipped
personas: [llm-routing-saas, internal-tool-builder]
priority: P0
---

# US-0006: Emit toolspec JSON for AI-agent discovery

As a tool author wiring c12n into an AI-agent stack, I want a
machine-readable tool spec so the agent can discover c12n's
commands, error patterns, and workflows without me writing
adapters by hand.

## Use this when

- Integrating c12n with OpenAI / Anthropic / MCP tool-use APIs.
- Building a self-documenting agent that surfaces c12n
  capabilities to end users.
- Generating SDK stubs from the spec.

## Result

`c12n toolspec` (or `c12n toolspec --format json`) prints a JSON
document describing:

- All subcommands with descriptions.
- Error patterns + their suggested fixes.
- Workflows (multi-step recipes).
- State introspection hooks.

## Steps

```bash
# default output
c12n toolspec

# explicit json
c12n toolspec --format json

# pipe into an MCP loader
c12n toolspec | mcp-loader register --tool c12n
```

## Verify

```bash
CGO_ENABLED=0 go test -run TestE2EToolspecValidJSON ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EToolspecContainsAllCommands ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EToolspecHasErrorPatterns ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EToolspecErrorPatternsHaveFix ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EToolspecHasWorkflows ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EToolspecWorkflowsHaveSteps ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EToolspecStateIntrospection ./cmd/c12n
```

## How it works

`cmd/c12n/toolspec.go` walks the cobra command tree, extracts
descriptions, and emits structured JSON via `kit/toolspec`. Error
patterns and workflows are declared inline in code and pulled into
the spec automatically.

This works in stub mode — toolspec doesn't invoke the classifier.

## Tests

- [`cmd/c12n/e2e_test.go:TestE2EToolspecValidJSON`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EToolspecContainsAllCommands`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EToolspecHasErrorPatterns`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EToolspecErrorPatternsHaveFix`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EToolspecHasWorkflows`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EToolspecWorkflowsHaveSteps`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EToolspecStateIntrospection`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/cli_test.go:TestToolspecOutputJSON`](../../cmd/c12n/cli_test.go)
- [`cmd/c12n/cli_test.go:TestToolspecWorkflows`](../../cmd/c12n/cli_test.go)
- [`cmd/c12n/cli_test.go:TestToolspecStateIntrospection`](../../cmd/c12n/cli_test.go)

# c12n personas

UCP (Universal Consumer Persona): **tool authors**.

c12n is a library for classifying LLM requests so callers can route
them to the right model. Consumers are people building developer
tools / routing layers / cost-control systems, not end-users running
c12n directly. Personas split by stack position.

## Businesses

- [LLM-routing SaaS](businesses/llm-routing-saas.md) — product
  company shipping a router that picks models per request. Needs
  classification accuracy + cost telemetry.
- [Cost-control startup](businesses/cost-control-startup.md) — small
  team building an LLM-cost dashboard / proxy. Needs signal-level
  introspection.

## Individuals

- [Framework author](individuals/framework-author.md) — building a
  higher-level orchestration framework that delegates classification
  to c12n.
- [Middleware developer](individuals/middleware-developer.md) —
  writing ASGI/HTTP middleware that classifies-then-routes. Needs
  in-process Go bindings.
- [Internal-tool builder](individuals/internal-tool-builder.md) —
  one engineer maintaining a company-internal classifier CLI.

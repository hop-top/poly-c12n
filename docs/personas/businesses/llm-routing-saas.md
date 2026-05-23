# LLM-routing SaaS

Product company shipping an LLM-routing service that picks models
per request based on classification.

## Use c12n when

- Product offers users automatic model selection (cheap vs powerful).
- Routing decision must run in <5ms per request.
- Cost telemetry per signal is required for finance / dashboards.
- Multi-tenant: thresholds may differ per customer.

## Constraints

- CI budget: cannot hit real LLMs for classification tests.
- Compliance: per-tenant signal config must be auditable.
- API stability: customers pin to a tag for months.

## What they get from c12n

- `Pipeline.Evaluate` returns a structured `PipelineResult` with
  per-signal scores — easy to log, threshold, alert on.
- `kit/config` layered YAML (system/user/project) — supports
  per-tenant overrides without code change.
- 20 signal types (14 implemented; 6 reserved) covering keyword,
  embedding, domain, safety, structure, analysis, routing.
- JSON output for metrics ingestion.

## Stories

- [US-0001 Configure pipeline via PipelineConfig](../../stories/US-0001-configure-pipeline.md)
- [US-0003 Parse PipelineResult into typed scores](../../stories/US-0003-parse-pipeline-result.md)
- [US-0005 Detect low-confidence classifications](../../stories/US-0005-low-confidence-detection.md)
- [US-0006 Emit toolspec JSON for AI-agent discovery](../../stories/US-0006-toolspec-discovery.md)
- [US-0008 Configure pipeline scope (system/user/project)](../../stories/US-0008-config-scope.md)

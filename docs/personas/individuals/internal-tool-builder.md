# Internal-tool builder

One engineer maintaining a company-internal classifier CLI.

## Use c12n when

- Internal users (data scientists, support, ops) need a CLI to
  classify prompts ad-hoc.
- Need `c12n doctor` to surface config + env health in support
  tickets.
- Auth + telemetry config rotate; want layered config not env-var
  spaghetti.

## Constraints

- Solo maintainer; long bus factor risk.
- Internal users tolerate brittleness less than external users.
- Onboarding new employees should be `c12n init` + go.

## What they get from c12n

- `c12n doctor` validates user config + project config + env at
  pre-flight.
- `c12n init` writes a usable default config.
- `c12n config get/set` with `--scope` lets users override at the
  right layer.

## Stories

- [US-0002 Evaluate a prompt via CLI](../../stories/US-0002-classify-cli.md)
- [US-0006 Emit toolspec JSON for AI-agent discovery](../../stories/US-0006-toolspec-discovery.md)
- [US-0008 Configure pipeline scope (system/user/project)](../../stories/US-0008-config-scope.md)

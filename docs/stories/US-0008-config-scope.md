---
status: shipped
personas: [llm-routing-saas, framework-author, internal-tool-builder]
priority: P0
---

# US-0008: Configure pipeline scope (system/user/project)

As a tool author, I want layered config (system → user → project)
so my product can ship sane defaults that users / projects override
without env-var spaghetti.

## Use this when

- Single binary serves multiple tenants — per-tenant config at
  user scope.
- Project-level overrides for CI / staging / prod.
- Internal tool ships with org-wide defaults; users tweak personal
  thresholds.

## Result

`c12n config set <key> <value> --scope <system|user|project>` writes
to the right config layer. `c12n config get <key>` reflects the
merged value. `c12n doctor` reports which layer provided each
effective value.

## Steps

```bash
# system-level default (written to ${SYSCONFDIR}/c12n/config.yaml)
sudo c12n config set signal.embedding.threshold 0.75 --scope system

# per-user override (written to ${XDG_CONFIG_HOME}/c12n/config.yaml)
c12n config set signal.embedding.threshold 0.8 --scope user

# project-level (written to ./.c12n/config.yaml)
c12n config set signal.embedding.threshold 0.9 --scope project

# read merged value
c12n config get signal.embedding.threshold
# → 0.9 (project wins)

# doctor surfaces which file supplied which key
c12n doctor
```

## Verify

```bash
CGO_ENABLED=0 go test -run TestE2EConfigSetScopeFlag ./cmd/c12n
CGO_ENABLED=0 go test -run TestE2EConfigSubcommands ./cmd/c12n
CGO_ENABLED=0 go test -run TestDoctorConfigCheck_UserConfigOnly ./cmd/c12n
CGO_ENABLED=0 go test -run TestDoctorConfigCheck_ProjectExists ./cmd/c12n
CGO_ENABLED=0 go test -run TestDoctorConfigCheck_BothMissing_LoadDefaults ./cmd/c12n
```

## How it works

c12n uses `kit/config` for layered YAML. Merge order (later wins):

1. Embedded defaults (`config.pkl`, evaluated at build time).
2. System: `${SYSCONFDIR}/c12n/config.yaml`.
3. User: `${XDG_CONFIG_HOME}/c12n/config.yaml`.
4. Project: `./.c12n/config.yaml`.
5. Environment variables (`C12N_<KEY>`).
6. Flags.

`c12n doctor` walks each layer and reports presence + parse status.

## Why three formats (pkl + YAML + JSON)

c12n separates **schema-of-record** from **user-editable config**
from **machine I/O**:

| Format | Role | Edited by |
|--------|------|-----------|
| `config.pkl` | Schema-of-record + defaults (embedded via `//go:embed`) | c12n maintainers |
| `*.yaml` | Layered user config (system/user/project) | end users / ops |
| JSON | FFI boundary + `c12n config --format json` output | machines |

Pkl ([pkl-lang.org](https://pkl-lang.org/)) gives:

- Cross-field validation (e.g. "`embedding_enabled` implies
  `embedding_model_path`").
- Per-field doc comments that travel with the schema.
- Type safety the YAML defaults alone can't enforce.
- One source of truth: the same `.pkl` evaluates to YAML, JSON,
  Pkl, properties, … via `pkl eval -f <format>`.

Users never edit `config.pkl` directly. They edit YAML files at one
of the three scopes; c12n merges pkl defaults → YAML layers → env
→ flags.

## Tests

- [`cmd/c12n/e2e_test.go:TestE2EConfigSetScopeFlag`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EConfigSubcommands`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EConfigGetKeyCompletion`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/e2e_test.go:TestE2EConfigSetKeyCompletion`](../../cmd/c12n/e2e_test.go)
- [`cmd/c12n/doctor_regressions_test.go:TestDoctorConfigCheck_UserConfigOnly`](../../cmd/c12n/doctor_regressions_test.go)
- [`cmd/c12n/doctor_regressions_test.go:TestDoctorConfigCheck_ProjectExists`](../../cmd/c12n/doctor_regressions_test.go)
- [`cmd/c12n/doctor_regressions_test.go:TestDoctorConfigCheck_BothMissing_LoadDefaults`](../../cmd/c12n/doctor_regressions_test.go)
- [`config_test.go`](../../config_test.go) — unit-level

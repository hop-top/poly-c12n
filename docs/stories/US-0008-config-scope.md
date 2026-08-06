---
status: partial
bindings:
  go: partial
personas: [llm-routing-saas, framework-author, internal-tool-builder]
priority: P0
---

# US-0008: Configure pipeline scope (system/user/project)

As a tool author, I want layered config (system → user → project) so my
product can ship sane defaults that users / projects override without
env-var spaghetti.

> **Status: partial.** Two of the three scopes work. `--scope system`
> fails with `config: scope path is empty` because c12n never sets
> `SystemConfigPath`. There is no environment-variable layer. And every
> `c12n` invocation panics at startup
> ([US-0002](US-0002-classify-cli.md)), so none of the commands below
> can be run today — the behaviour described here is what the library
> layer does when driven from Go.

## Use this when

- Project-level overrides for CI / staging / prod.
- Internal tool ships with defaults; users tweak personal thresholds
  at user scope.

Per-tenant system-wide defaults are not available — see below.

## Result

- `c12n config set <key> <value> --scope <user|project>` writes to that
  layer. `--scope project` is the default.
- `c12n config get <key>` reads the merged value.
- `c12n doctor` reports whether each config **file** was found and
  parsed. It does **not** report which layer supplied which key.

## Steps

```bash
# per-user override (${XDG_CONFIG_HOME}/c12n/config.yaml)
c12n config set embedding_threshold 0.8 --scope user

# project-level (./.c12n.yaml) — this is the default scope
c12n config set embedding_threshold 0.9 --scope project

# read merged value
c12n config get embedding_threshold
# → 0.9 (project wins)

# list entries, optionally filtered by scope
c12n config list --scope user

# file-level diagnostics
c12n doctor
```

Keys are the **flat** names from the pkl schema —
`embedding_threshold`, `keyword_threshold`, `safety_toxicity_threshold`
([`go/config.pkl`](../../go/config.pkl)). Dotted paths such as
`signal.embedding.threshold` do not exist.

The project file is `./.c12n.yaml`
([`go/cmd/c12n/root.go:63`](../../go/cmd/c12n/root.go)), not
`./.c12n/config.yaml`.

## System scope does not work

`root.go` builds `config.Options` with two paths:

```go
// go/cmd/c12n/root.go:61
opts := config.Options{
    UserConfigPath:    filepath.Join(cfgDir, "config.yaml"),
    ProjectConfigPath: ".c12n.yaml",
}
```

`SystemConfigPath` is left empty, so kit rejects the write:

```
config.Set("keyword_threshold", "0.75", config.ScopeSystem, opts)
  → config: scope path is empty
```

(`config.ScopeProject` with the same options succeeds and writes the
file.) kit supports the scope — `config.Options` has a
`SystemConfigPath` field and a `systemConfigPath(tool)` helper — c12n
simply never populates it. `--scope system` is accepted by the flag
parser and by `parseConfigScope`
([`go/cmd/c12n/config_cmd.go:157`](../../go/cmd/c12n/config_cmd.go)),
then fails at write time.

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestE2EConfigSetScopeFlag ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EConfigSubcommands ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestDoctorConfigCheck_UserConfigOnly ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestDoctorConfigCheck_ProjectExists ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestDoctorConfigCheck_BothMissing_LoadDefaults ./cmd/c12n
```

All pass. `TestE2EConfigSetScopeFlag` asserts the `--scope` flag is
*registered* and that its default is `project`; it does not perform a
write, which is why the system-scope failure is invisible to the suite.
The `TestDoctorConfigCheck_*` trio drives `doctor`'s config check with
temp files and covers user-only / project-exists / both-missing.

## How it works

c12n uses `hop.top/kit/go/core/config` for layered YAML. Effective merge
order in c12n today (later wins):

1. Embedded defaults — `DefaultConfig()`
   ([`go/config.go:69`](../../go/config.go)), a Go literal that mirrors
   [`go/config.pkl`](../../go/config.pkl) by hand.
2. ~~System~~ — path never configured; layer absent.
3. User: `${XDG_CONFIG_HOME}/c12n/config.yaml`.
4. Project: `./.c12n.yaml`.
5. `--config <path>`, which **replaces** all file paths rather than
   adding a layer ([`go/cmd/c12n/root.go:79`](../../go/cmd/c12n/root.go)).

There is no environment-variable layer. kit offers one via
`config.Options.EnvPrefix`, but c12n does not set it, so `C12N_<KEY>`
variables are ignored. The only `C12N_` string in the tree is
`C12N_CONFIG`, listed in the toolspec's `EnvVars`
([`go/cmd/c12n/toolspec.go:211`](../../go/cmd/c12n/toolspec.go)) and
read by nothing.

`c12n doctor` runs three checks
([`go/cmd/c12n/doctor.go`](../../go/cmd/c12n/doctor.go)):
`config-file` (layered load succeeds; reports paths + found/not-found),
`model-paths` (enabled model-backed signals have resolvable paths), and
`native-engine` (whether the binary was built with `-tags c12n_native`).
Per-key provenance is not among them.

## Why three formats (pkl + YAML + JSON)

c12n separates **schema-of-record** from **user-editable config** from
**machine I/O**:

| Format | Role | Edited by |
|--------|------|-----------|
| `config.pkl` | Schema-of-record + defaults (embedded via `//go:embed`) | c12n maintainers |
| `*.yaml` | Layered user config (user/project) | end users / ops |
| JSON | FFI boundary + `config list --format json` output | machines |

Pkl ([pkl-lang.org](https://pkl-lang.org/)) gives per-field doc comments
that travel with the schema, constrained types (e.g.
`keyword_strategy: "regex"|"bm25"|"trigram"|"fuzzy"`), and one source
for shell completion of keys and values
([`go/cmd/c12n/config_cmd.go:176-208`](../../go/cmd/c12n/config_cmd.go)).

Two caveats on the schema-of-record framing:

- The pkl file is consumed for *completion and schema introspection*
  only. Runtime defaults come from the `DefaultConfig()` Go literal, so
  the two can drift; nothing tests them against each other.
- `config.pkl` declares no cross-field constraints today. In
  particular `embedding_enabled` does **not** imply
  `embedding_model_path` — `doctor`'s `model-paths` check catches that
  case at runtime instead.

JSON output is `config list --format json`; there is no
`config --format json`.

## What this story needs to reach `shipped`

1. Startup panic fixed (US-0002).
2. `SystemConfigPath` populated, or `--scope system` rejected up front
   with a clear message.
3. A test that performs a real `config set` per scope and asserts the
   file written.
4. Decide whether the env layer is in scope; if so, set `EnvPrefix`.

## Tests

- [`go/cmd/c12n/e2e_test.go:TestE2EConfigSetScopeFlag`](../../go/cmd/c12n/e2e_test.go)
  — flag registration + default only.
- [`go/cmd/c12n/e2e_test.go:TestE2EConfigSubcommands`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EConfigGetKeyCompletion`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/e2e_test.go:TestE2EConfigSetKeyCompletion`](../../go/cmd/c12n/e2e_test.go)
- [`go/cmd/c12n/doctor_regressions_test.go:TestDoctorConfigCheck_UserConfigOnly`](../../go/cmd/c12n/doctor_regressions_test.go)
- [`go/cmd/c12n/doctor_regressions_test.go:TestDoctorConfigCheck_ProjectExists`](../../go/cmd/c12n/doctor_regressions_test.go)
- [`go/cmd/c12n/doctor_regressions_test.go:TestDoctorConfigCheck_BothMissing_LoadDefaults`](../../go/cmd/c12n/doctor_regressions_test.go)
- [`go/config_test.go`](../../go/config_test.go) — unit-level; covers
  `LoadConfig` layering with temp files.

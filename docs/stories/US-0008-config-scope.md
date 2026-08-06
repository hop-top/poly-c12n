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

> **Status: partial.** All three scopes now resolve a real path, and
> the commands are reachable — the startup panic is fixed. Two things
> still fall short: `config set` writes numeric values as quoted
> strings, which makes every later invocation fail to load the file
> ([Setting a numeric key bricks the config](#setting-a-numeric-key-bricks-the-config)),
> and the `C12N_<KEY>` env layer reaches viper but not the typed config
> the pipeline uses ([The env layer does not reach the
> pipeline](#the-env-layer-does-not-reach-the-pipeline)).

## Use this when

- Project-level overrides for CI / staging / prod.
- Internal tool ships with defaults; users tweak personal thresholds
  at user scope.

Restrict yourself to string-valued keys until the quoting bug is
fixed, or hand-edit the YAML.

## Result

- `c12n config set <key> <value> --scope <system|user|project>` writes
  to that layer. `--scope project` is the default.
- `c12n config get <key>` reads the merged value.
- `c12n config list` lists entries.
- `c12n doctor` reports whether each config **file** was found and
  parsed. It does **not** report which layer supplied which key.

`--scope` lives on `config set` and `config list`, not on the `config`
parent.

## System scope now resolves

`rootConfigOptions()` populates all three slots
([`go/cmd/c12n/root.go`](../../go/cmd/c12n/root.go)):

```go
opts := config.Options{
    SystemConfigPath:  filepath.Join("/etc", "c12n", "config.yaml"),
    ProjectConfigPath: ".c12n.yaml",
    EnvPrefix:         "C12N",
}
// UserConfigPath = ${XDG_CONFIG_HOME}/c12n/config.yaml
```

`--scope system` no longer fails with `config: scope path is empty`.
It now fails only on filesystem permissions, which is correct:

```console
$ c12n config set keyword_threshold 0.75 --scope system
Error: GENERIC: config set: mkdir /etc/c12n: permission denied
```

Run it under a privileged account and the write succeeds.
`--scope user` writes the file as expected:

```console
$ XDG_CONFIG_HOME=/tmp/x c12n config set keyword_threshold 0.8 --scope user
$ cat /tmp/x/c12n/config.yaml
keyword_threshold: "0.8"
```

## Setting a numeric key bricks the config

Note the quotes above. `config set` writes every value as a YAML
string, but the typed `Config` struct declares `keyword_threshold` as
`float64`. The file it just wrote therefore fails to load — and
because config loading happens in `PersistentPreRunE`, **every**
subsequent `c12n` invocation in that scope fails, including `doctor`:

```console
$ c12n config set keyword_threshold 0.9 --scope project
$ cat .c12n.yaml
keyword_threshold: "0.9"
$ c12n doctor
Error: load config: load config .c12n.yaml: yaml: unmarshal errors:
  line 1: cannot unmarshal !!str `0.9` into float64
```

Recovery is to hand-edit the file and drop the quotes. String-valued
keys are unaffected:

```console
$ c12n config set keyword_strategy regex --scope project
$ cat .c12n.yaml
keyword_strategy: regex
$ c12n config get keyword_strategy
regex
```

Keys are the **flat** names from the pkl schema —
`embedding_threshold`, `keyword_threshold`, `safety_toxicity_threshold`
([`go/config.pkl`](../../go/config.pkl)). Dotted paths such as
`signal.embedding.threshold` do not exist. The project file is
`./.c12n.yaml`, not `./.c12n/config.yaml`.

## The env layer does not reach the pipeline

`rootConfigOptions()` sets `EnvPrefix: "C12N"` and passes kit's viper
instance, so `C12N_<KEY>` variables are bound. But kit's `BindEnv`
configures *viper*, and `c12n.LoadConfig` decodes the **typed struct**
from the file layers only ([`go/config.go`](../../go/config.go)) —
env values are never merged into it.

Probed directly against `LoadConfig` with `C12N_MAX_CONCURRENCY=7`:

```text
typed MaxConcurrency=8   viper max_concurrency=7
```

The typed value — the one `ToPipelineConfig()` forwards to the engine —
keeps the default. So the env layer is wired but inert for pipeline
behaviour. `config get` does not read it either:

```console
$ C12N_KEYWORD_THRESHOLD=0.42 c12n config get keyword_threshold
Error: GENERIC: config: key not found
```

## Steps

```bash
# per-user override (${XDG_CONFIG_HOME}/c12n/config.yaml)
c12n config set keyword_strategy regex --scope user

# project-level (./.c12n.yaml) — this is the default scope
c12n config set keyword_strategy bm25 --scope project

# read merged value
c12n config get keyword_strategy
# → bm25 (project wins)

# list entries
c12n config list

# file-level diagnostics
c12n doctor
```

## Verify

```bash
cd go && CGO_ENABLED=0 go test -run TestNewRootSetsLayeredConfigPaths ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EConfigSetScopeFlag ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestE2EConfigSubcommands ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestDoctorConfigCheck_UserConfigOnly ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestDoctorConfigCheck_ProjectExists ./cmd/c12n
cd go && CGO_ENABLED=0 go test -run TestDoctorConfigCheck_BothMissing_LoadDefaults ./cmd/c12n
```

All pass. `TestNewRootSetsLayeredConfigPaths` asserts all three path
slots are non-empty — it pins the system-scope fix.
`TestE2EConfigSetScopeFlag` asserts the `--scope` flag is *registered*
and defaults to `project`; it does not perform a write, which is why
the quoting bug above is invisible to the suite. The
`TestDoctorConfigCheck_*` trio drives `doctor`'s config check with temp
files.

## How it works

c12n uses `hop.top/kit/go/core/config` for layered YAML. Effective merge
order in c12n today (later wins):

1. Embedded defaults — `DefaultConfig()`
   ([`go/config.go`](../../go/config.go)), a Go literal that mirrors
   [`go/config.pkl`](../../go/config.pkl) by hand.
2. System: `/etc/c12n/config.yaml`.
3. User: `${XDG_CONFIG_HOME}/c12n/config.yaml`.
4. Project: `./.c12n.yaml`.
5. kit's `-c/--config` tokens. A bare path appends to
   `ExtraConfigPaths`; a `key=value` token becomes an override that
   wins over every file layer. This is additive — it no longer
   replaces the discovered paths.

The `C12N_<KEY>` env layer sits outside this list: bound to viper,
absent from the typed struct.

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
| `*.yaml` | Layered user config (system/user/project) | end users / ops |
| JSON | FFI boundary + `config list --format json` output | machines |

Pkl ([pkl-lang.org](https://pkl-lang.org/)) gives per-field doc comments
that travel with the schema, constrained types (e.g.
`keyword_strategy: "regex"|"bm25"|"trigram"|"fuzzy"`), and one source
for shell completion of keys and values
([`go/cmd/c12n/config_cmd.go`](../../go/cmd/c12n/config_cmd.go)).

Two caveats on the schema-of-record framing:

- The pkl file is consumed for *completion and schema introspection*
  only. Runtime defaults come from the `DefaultConfig()` Go literal, so
  the two can drift; nothing tests them against each other.
- `config.pkl` declares no cross-field constraints today. In
  particular `embedding_enabled` does **not** imply
  `embedding_model_path` — `doctor`'s `model-paths` check catches that
  case at runtime instead.

JSON output is `config list --format json`.

## What this story needs to reach `shipped`

1. `config set` writing values typed to the schema, so numeric keys
   round-trip instead of bricking the file.
2. The `C12N_<KEY>` layer merged into the typed `Config`, or the
   `EnvPrefix` wiring dropped so it does not imply support it lacks.
3. A test that performs a real `config set` per scope, then reloads and
   asserts the value — the gap that let both bugs through.

## Tests

- [`go/cmd/c12n/root_regressions_test.go:TestNewRootSetsLayeredConfigPaths`](../../go/cmd/c12n/root_regressions_test.go)
  — pins non-empty system/user/project paths.
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

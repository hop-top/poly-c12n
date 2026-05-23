# ADR-0002: c12n-php FFI Binding Over the Rust Core

**Status**: Accepted
**Date**: 2026-05-11
**Deciders**: Idea Crafters team
**Track**: `c12n-php-bindings`
**Related**: ADR-0001 (c12n-ts WASM binding, sibling decision)

## Context

c12n is a polyglot LLM-classification engine: a Rust core (`c12n-core`) with a stable
`extern "C"` ABI that fans out into per-language bindings — Go (cgo), Python (PyO3),
TypeScript (wasm-bindgen, ADR-0001), and now PHP. The PHP binding closes the fleet
coverage gap and matches the hop-top fleet's "one tool per language" convention
(`kit`, `tlc`, `aps`, `gym`, all shipping PHP sidecars).

Unlike `kit-php` — pure-PHP, no native dependency — c12n-php is FFI-driven: it
loads `libc12n_core.{so,dylib,dll}` via PHP's `FFI::cdef` against a C header
derived from `core/src/ffi.rs`. The FFI surface is small and stable:
`c12n_pipeline_new`, `c12n_pipeline_evaluate`, `c12n_pipeline_free`,
`c12n_result_free`, `c12n_result_json` — JSON in, JSON out, opaque pointer for
the pipeline.

This is the first FFI-driven PHP package in the hop-top fleet. Several questions
have no canonical hop-top answer yet:

1. What is the mirror named?
2. Who authors the C header — and how does it stay in sync with `ffi.rs`?
3. How does the native cdylib reach end-users? PHP has no equivalent of Python
   wheels (PEP 427) or npm prebuilds. Composer ships `.php` text only.
4. What's the PHP version floor? FFI was experimental in 7.4 / 8.0, stable in
   8.1+.
5. How are versions coordinated with c12n-core, c12n-py, c12n-ts, and the
   top-level c12n release?
6. Does Windows ship at v0?
7. Does the binding pull in kit-php (Composer) for logging / events / CLI
   surface, the way c12n-py and c12n-ts pull in kit's other-language SDKs?

This ADR locks the answers and records the alternatives that were rejected.

## Decision

Seven linked decisions, all locked 2026-05-18 in `plan.md` and ratified here:

1. **Mirror name: `hop-top/c12n-php`** — matches the fleet pattern
   `hop-top/<tool>-<lang>` already used by `kit-php`, `tlc-py`, `aps-ts`, etc.
2. **C header authoring: cbindgen** — `cbindgen` is wired into c12n-core's
   build (`build.rs` step or `cargo make` task) and emits `libc12n_core.h`
   from the Rust `extern "C"` source as a build artifact. Correct-by-construction.
3. **Windows: shipped at v0.1.0-alpha.0** — `libc12n_core.dll` ships in the
   post-install download bucket so PHP-on-Windows users get a working artifact
   from day one. Windows PHPUnit integration is deferred to a follow-up
   (T-0153); v0 CI runs PHPUnit on `ubuntu-latest` + `macos-latest` only.
4. **Native lib distribution: Composer post-install script** — downloads
   `libc12n_core-<version>-<os>-<arch>.tar.gz` from the matching c12n GitHub
   release, SHA256-verified against a release-asset manifest, into a
   composer-managed cache dir. Env-var `C12N_CORE_LIB_PATH` overrides the
   resolved path for airgapped / sandboxed installs.
5. **Versioning: linked-versions** with `c12n`, `c12n-core`, `c12n-py`, and
   `c12n-ts`. All bump together; `release-please-config.json`'s
   `linked-versions` group includes c12n-php.
6. **PHP version floor: 8.1** — composer constraint `"php": "^8.1"`. FFI is
   stable from 8.1+; experimental below.
7. **Kit-php dependency: required** — c12n-php depends on `hop-top/kit`
   (Composer) for logging, event-bus participation, and CLI surface. Mirrors
   c12n-py and c12n-ts kit adoption.

## Rationale

### 1. Mirror name — fleet consistency wins

Every hop-top tool that ships a non-Go binding follows `hop-top/<tool>-<lang>`.
`hop-top/c12n-php` is the only name that doesn't require an exception.
`c12n-org/c12n-php` was considered to namespace classification work separately
from the meta-tooling — rejected because the org split adds discovery friction
(two packagist queries, two GitHub orgs to follow) for zero composition benefit.

### 2. cbindgen — correct-by-construction headers

The Rust FFI surface evolves. `core/src/ffi.rs` already exposes five
functions; future signal types will add more. Two mechanisms keep a C header
in sync:

- **Hand-author once, maintain forever** — a single `.h` file committed to
  the repo, edited each time Rust signatures change. Cheap to set up, expensive
  to maintain: every PR touching `ffi.rs` must remember to update the header,
  and the cgo / PHP / TS jobs all consume the same header. Drift is silent
  until a binding breaks at runtime.
- **cbindgen** — a build-time generator that parses Rust source and emits a
  C header. Wired as a `build.rs` step or `cargo make` task. The header is
  a build artifact, never hand-edited. Drift is impossible by construction.

cbindgen wins. It is the de-facto standard for Rust → C header generation
(used by Mozilla, Cloudflare, the rust-bindgen reverse), and adding it
buys us correctness for the cgo binding *and* the upcoming TS WASM binding,
not just PHP.

### 3. Windows at v0 — Rust gives it for free

The Rust cdylib build matrix already targets `x86_64-pc-windows-msvc` because
c12n-go's cgo job tests Windows. Shipping `libc12n_core.dll` in the
post-install download bucket costs one extra GH release asset and zero
incremental build work. PHP-on-Windows is a non-trivial slice of the userbase
(corporate dev workstations); excluding them would be a foot-gun.

The compromise: ship the cdylib day one, defer the *PHPUnit-on-windows*
matrix entry. Adding `windows-latest` to the CI matrix for one job — when
Rust + Go already cover Windows cdylib correctness — would add ~5 minutes to
every PR for a marginal coverage gain. Track T-0153 (post-v0) folds Windows
PHPUnit in once we have signal on Windows-PHP-FFI adoption.

### 4. Composer post-install — dominant cross-language pattern

PHP's packaging tooling does not natively distribute per-platform native blobs.
Three patterns exist in the wild:

- **Composer post-install script** (chosen) — runs after `composer install`,
  detects host OS/arch, downloads the matching tarball from GitHub Releases,
  verifies SHA256 against an embedded manifest, extracts into
  `vendor/hop-top/php/runtime/` (composer-relative cache). End user runs
  `composer install`; everything else is automatic. Same UX as Python wheels
  (`pip install` fetches the wheel) and npm prebuilds (`npm install` fetches
  via `prebuild-install`). Failure modes: GitHub outage, hash mismatch,
  corporate proxies. Mitigated by env-var override (`C12N_CORE_LIB_PATH`) and
  a clear error message pointing at the manifest URL.
- **Require user to install separately** (rejected) — cleanest packaging
  story, worst UX. Mirrors how `ext-zstd`-style packages work: user runs
  `apt install libzstd-dev` first. For a binding aimed at app developers
  (not extension maintainers), this is friction we should not impose.
- **Composer `extra` path hints + runtime probe** (rejected) — define a list
  of candidate paths in `composer.json#extra` and let the runtime FFI loader
  walk them. Loose, no install-time validation; failure surfaces at
  first-call time with cryptic FFI errors. Worse UX than (1), and the
  validation gap is dangerous.

#### Lib resolution order

The runtime path resolver walks, in order:

1. `getenv('C12N_CORE_LIB_PATH')` — explicit override for airgapped envs.
2. composer-managed cache dir: `vendor/hop-top/php/runtime/lib/libc12n_core.<ext>`.
3. system library path (`/usr/local/lib`, `/usr/lib`) — for distro-packaged
   installs.

If none resolve, throw a `C12nCoreNotFoundException` with the manifest URL
and the override env-var name in the message.

#### Cache dir choice (open implementation question)

`vendor/hop-top/php/runtime/` keeps the artifact composer-relative, which
makes `vendor/`-archive deploys (the common Composer immutable-deploy pattern)
self-contained. The alternative — `~/.composer/cache/c12n/` — survives
`composer install --no-dev` rebuilds but breaks containerised deploys that
mount only the project's `vendor/`. The composer-relative path is the
recommended default; the global cache stays an option for CI-runner reuse.
Final decision deferred to T-0143 (the post-install script implementer).

### 5. Linked versions — single release-please group

c12n is one product surface, five packages. Independent versioning would
require five changelogs, five tags, and five compatibility matrices to
explain to users. `release-please`'s `linked-versions` group bumps all five
together on every release, so `c12n@0.2.0` always pairs with `c12n-php@0.2.0`,
`c12n-py@0.2.0`, etc. Operational cost: one mass bump per release; benefit:
trivial compatibility story ("same version, guaranteed to interoperate").

### 6. PHP 8.1 floor — FFI stability

PHP FFI shipped in 7.4 flagged `experimental` and stayed that way through 8.0.
Stable from 8.1 onward. Shipping a binding atop upstream-experimental tech is
a red flag (forward-compat breaks at PHP minor releases; unclear long-term
support). PHP 7.4 reached EOL in November 2022; PHP 8.0 in November 2023.
PHP 8.1 has been stable for >3 years at v0 release time. The version floor is
the minimum that gets us off experimental APIs.

### 7. Kit-php dependency — observability for free

c12n-py and c12n-ts both depend on their kit SDK counterpart for structured
logging, event-bus participation (so c12n classification events flow into
the same bus that aps, tlc, and gym subscribe to), and CLI helpers
(`c12n-php classify ...` shares ergonomics with `c12n classify ...`). c12n-php
should not be the exception. The dependency is one composer require
(`hop-top/kit: ^0.4`) and unlocks fleet-wide instrumentation.

## Alternatives Considered

### C header authoring: hand-authored

Single committed `libc12n_core.h`, manually synced after every Rust signature
change. Lower upfront cost. Rejected: drift is silent and corrupts cgo + PHP
+ TS bindings simultaneously when forgotten. cbindgen's build-time generation
removes the failure mode entirely.

### Distribution: separate install

Document `libc12n_core` as a system dependency; user runs
`brew install libc12n_core` or downloads manually. Cleanest packaging,
worst onboarding UX, mirrors `ext-zstd`. Rejected: c12n-php targets app
developers, not extension maintainers. Friction is unacceptable.

### Distribution: composer extra + runtime probe

Path hints in `composer.json#extra`, runtime walks candidates. No install-time
validation; first-call failures are cryptic FFI errors. Rejected: the
validation gap is worse than (1)'s network dependency.

### PHP floor: 7.4

FFI shipped experimental in 7.4. Rejected outright: shipping atop
upstream-experimental tech is a red flag, and 7.4 is EOL.

### PHP floor: 8.0

Still experimental FFI. Rejected: 8.0 is EOL and the FFI surface had not
stabilised.

### Windows: defer entirely

Drop Windows from v0; Linux + macOS only. Rejected: the Rust cdylib targets
Windows for free (the cgo job already verifies it), and the post-install
script's OS/arch matrix gets `libc12n_core.dll` for the cost of one extra
GH release asset. Excluding Windows users would be gratuitous.

### Windows: full PHPUnit on `windows-latest`

Add `windows-latest` to the v0 PHPUnit matrix. Rejected: ~5 minutes added
to every PR for a coverage gain already covered by Rust + Go Windows tests
on the same cdylib. Deferred to T-0153 once we have adoption signal.

## Consequences

### Positive

- **Header drift impossible** — cbindgen guarantees `libc12n_core.h` matches
  `ffi.rs` at build time. cgo, PHP, and the upcoming TS WASM binding all
  consume the same generated header.
- **End-user UX matches Python / npm** — `composer install` fetches the
  cdylib transparently. No separate install step, no platform-specific docs.
- **Windows day-one** — PHP-on-Windows users get a working `libc12n_core.dll`
  at v0.1.0-alpha.0. Not gated on PHPUnit-Windows CI maturity.
- **Fleet observability** — kit-php dependency means c12n-php emits the same
  structured events as c12n-py / c12n-ts. aps and tlc can observe
  classification flows without c12n-php-specific adapters.
- **Single compatibility matrix** — linked-versions means
  "c12n@X = c12n-php@X = c12n-py@X" always holds. Users do not need a
  compatibility table.

### Negative

- **cbindgen adds a build-time dep to c12n-core** — standard in the Rust
  ecosystem (Mozilla, Cloudflare, AWS use it) but new for c12n. The
  `build.rs` step adds ~2s to cold builds; cached afterward. Risk: cbindgen
  upstream bug breaks header emission; mitigated by pinning cbindgen in
  `Cargo.lock` and the `build.rs`.
- **Composer post-install introduces a network step** — `composer install`
  in CI now requires GitHub Releases reachability. Failure modes: GH outage
  (rare, but blocks installs), SHA256 mismatch (signals release pipeline
  bug), corporate proxies (handled via standard `HTTPS_PROXY` env vars +
  override env-var fallback).
- **Windows PHPUnit deferred** — actual binding correctness on Windows is
  un-tested via PHP at v0. Mitigated by Rust + Go cgo Windows jobs
  exercising the same cdylib. Residual risk: PHP FFI / Windows linker
  interaction-specific bugs. Tracked in T-0153.
- **PHP 8.1 floor excludes older PHP** — users on PHP 7.4 / 8.0 cannot
  install c12n-php. Acceptable: those versions are EOL, and the FFI
  surface was upstream-experimental.

### Neutral

- **Cache dir location** — composer-relative `vendor/` cache vs global
  `~/.composer/cache/`. Recommended: vendor-relative for deploy-archive
  self-containment; CI can opt into global for cache reuse. Final
  implementation choice in T-0143.
- **Release-asset manifest hosting** — SHA256 manifest published at
  `https://github.com/hop-top/poly-c12n/releases/download/c12n-core/v<version>/manifest.json`
  alongside the tarballs, generated from goreleaser's `dist/checksums.txt`
  by `tools/release-manifest.sh`. Formalized in §"Release-asset
  `manifest.json` contract" below (T-0184). The hop-top/.github reusable
  goreleaser workflow will absorb the conversion step as a separate
  follow-up so every FFI consumer in the fleet gets a manifest for free.
- **kit-php experimental status** — kit-php is at
  `0.4.0-experimental.1`. c12n-php pinning `^0.4` accepts forward-compat
  churn until kit-php stabilises. Acceptable for v0-alpha; revisit at
  c12n-php 1.0.

## Implementation Notes

### FFI surface consumed

c12n-php's `FFI::cdef` will consume `libc12n_core.h` (cbindgen-emitted) and
expose four PHP-visible functions matching `core/src/ffi.rs`:

```c
void* c12n_pipeline_new(const char* config_json);
char* c12n_pipeline_evaluate(const void* pipeline, const char* context_json);
void  c12n_pipeline_free(void* pipeline);
void  c12n_result_free(char* result);
```

`c12n_result_json` (identity function) is omitted from the PHP surface; it
exists in Rust for API completeness but PHP reads the `char*` directly via
`FFI::string`.

### Package layout

```text
php/
├── composer.json              # name: hop-top/c12n-php; require: php ^8.1, hop-top/kit ^0.4
├── phpunit.xml
├── src/
│   ├── Pipeline.php           # FFI::cdef wrapper, lifecycle
│   ├── ClassificationContext.php
│   ├── Result.php
│   └── Ffi.php                # FFI::scope cache, lib path resolver
├── tests/
│   └── ...                    # PHPUnit + cross-language parity fixtures
├── scripts/
│   └── install-cdylib.php     # post-install: download, verify, extract
└── runtime/                   # populated by post-install (gitignored)
    └── lib/libc12n_core.{so,dylib,dll}
```

### Release-asset `manifest.json` contract

The Installer's SHA256 verification step depends on a sibling `manifest.json`
published at the same GitHub release as the cdylib tarballs. This is the
canonical contract for every FFI-driven hop-top consumer; sibling-language
bindings (cgo, PyO3, wasm-bindgen) reuse the same convention when they need
post-install asset verification.

- **Filename**: `manifest.json` — committed as an additional release asset
  alongside the per-platform `libc12n_core-<os>-<arch>.tar.gz` tarballs.
- **Tag**: `c12n-core/v<version>` (linked-versions per §5, e.g.
  `c12n-core/v0.1.0-alpha.0`). The c12n-core slice of the monorepo tags
  independently so each FFI consumer pins against the cdylib version,
  not the umbrella release.
- **URL template**:
  `https://github.com/hop-top/poly-c12n/releases/download/c12n-core/v<version>/manifest.json`.
  Hardcoded in `php/src/Installer.php::MANIFEST_URL_TEMPLATE`; mirrored by
  any future binding that needs verification.
- **Shape**: a flat JSON object mapping the asset filename to its SHA256
  digest. Two value forms are accepted; producers SHOULD emit the
  prefixed form:

  ```json
  {
    "libc12n_core-linux-x86_64.tar.gz":  "sha256:<64-hex>",
    "libc12n_core-linux-aarch64.tar.gz": "sha256:<64-hex>",
    "libc12n_core-macos-x86_64.tar.gz":  "sha256:<64-hex>",
    "libc12n_core-macos-aarch64.tar.gz": "sha256:<64-hex>",
    "libc12n_core-windows-x86_64.tar.gz":"sha256:<64-hex>"
  }
  ```

  The Installer also accepts bare `"<64-hex>"` values without the
  `sha256:` prefix, for tolerance against producers that emit the
  shorter form (e.g. hand-edited test fixtures). New producers MUST use
  the prefixed form so the digest algorithm is self-describing — a
  future SHA-512 migration can coexist with SHA-256 entries on the same
  manifest without breaking older consumers.
- **Generation**: goreleaser emits `dist/checksums.txt` natively (one
  `<hex>  <filename>` line per asset). A post-process step converts
  that file into `dist/manifest.json` before the release-asset upload
  step runs. The conversion is implemented by `tools/release-manifest.sh`
  (POSIX shell, ~30 LOC) at the repo root:

  ```sh
  ./tools/release-manifest.sh dist/checksums.txt > dist/manifest.json
  ```

  Rejected alternative: a goreleaser custom template emitting the
  manifest via `extra_files`. The shell script is simpler to debug,
  has no goreleaser-version coupling, and can be invoked from any
  release pipeline (not just goreleaser).
- **Workflow wiring**: c12n's `.github/workflows/publish.yml` delegates
  release publishing to the reusable goreleaser workflow at
  `hop-top/.github`. A follow-up there will teach that reusable
  workflow to run `tools/release-manifest.sh` after `goreleaser release`
  and before the asset-upload step, so the manifest ships alongside the
  tarballs without per-repo wiring. Until then, c12n's publish.yml is
  unchanged and the manifest is generated on demand by release engineers.

### CI matrix (v0)

PHP 8.1 / 8.2 / 8.3 × {`ubuntu-latest`, `macos-latest`}. `windows-latest`
deferred to T-0153. cdylib artifact downloaded from the Rust job (same
artifact-upload pattern c12n-go uses).

## References

- `plan.md` §"Locked decisions (2026-05-18)" — source of the seven decisions
  ratified here.
- `core/src/ffi.rs` — Rust source cbindgen will parse.
- `kit/hops/main/sdk/experimental/php/composer.json` — canonical kit-php
  manifest shape, copied for c12n-php structure.
- ADR-0001 — c12n-ts WASM binding (sibling decision, same fleet context).
- `release-please-config.json` — linked-versions group definition.
- T-0132 (this ADR), T-0136..T-0150, T-0152..T-0153 — implementation tasks
  gated on this document.

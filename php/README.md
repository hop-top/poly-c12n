# c12n-php

LLM request classification engine — PHP FFI bindings over the Rust core.

## Status

`v0.1.0-alpha.0` — initial scaffold. The FFI surface is wired against the
cbindgen-generated header in `c12n-core/include/libc12n_core.h`; the
native `libc12n_core.{so,dylib,dll}` is downloaded by the post-install
script (T-0143, not yet implemented). For local development, build the
cdylib from `c12n-core/` and either symlink it into `runtime/lib/` or
set `C12N_CORE_LIB_PATH`.

## Requirements

- **PHP 8.1+** — FFI is stable from 8.1 onward (experimental in 7.4/8.0).
- **`ext-ffi`** — PHP's FFI extension. Bundled with most distributions
  but disabled by default; enable in `php.ini` with `ffi.enable=true`
  (CLI is usually fine; web SAPI requires `ffi.enable=preload` and a
  preload script).
- **`libc12n_core`** — the Rust cdylib. Downloaded automatically by
  Composer post-install once T-0143 ships; before then, build from
  `c12n-core` and point `C12N_CORE_LIB_PATH` at the artifact.

## Install

```bash
composer require hop-top/c12n
```

The `post-install-cmd` hook (registered now, implemented in T-0143)
fetches the matching `libc12n_core` tarball from
`https://github.com/hop-top/c12n/releases/download/{tag}/...`,
SHA256-verifies it, and extracts into `vendor/hop-top/c12n/runtime/lib/`.

## Quick start

```php
use HopTop\C12n\ClassificationContext;
use HopTop\C12n\Pipeline;
use HopTop\C12n\PipelineConfig;
use HopTop\C12n\PipelineResult;

$pipeline = new Pipeline(new PipelineConfig(
    maxConcurrency: 8,
    timeoutMs: 5000,
));

$json = $pipeline->evaluate(new ClassificationContext(
    text: 'Hello, world.',
    history: [],
    headers: ['x-trace' => 'abc'],
));

$result = new PipelineResult($json);

foreach ($result->results() as $signal) {
    printf("%s (%s): %.2f\n", $signal->name, $signal->type, $signal->confidence);
}

$pipeline->close(); // optional — __destruct cleans up
```

## Native library resolution

`HopTop\C12n\Ffi::libPath()` resolves the path to `libc12n_core` in this
order (ADR-0002 §4):

1. `C12N_CORE_LIB_PATH` env var — explicit override for airgapped /
   sandboxed installs.
2. `composer.json#extra.c12n-core.local-path` — project-local override,
   useful when developing against a sibling cdylib checkout.
3. Default: `runtime/lib/libc12n_core.{so,dylib,dll}` relative to this
   package, populated by the post-install Installer.

## Header contract

`HopTop\C12n\Ffi::CDEF` mirrors the four public symbols emitted by
`c12n-core/include/libc12n_core.h`:

```c
void *c12n_pipeline_new(const char *config_json);
char *c12n_pipeline_evaluate(const void *pipeline, const char *context_json);
void  c12n_pipeline_free(void *pipeline);
void  c12n_result_free(char *result);
```

PHP's FFI parser does not understand the C preprocessor (`#include
<stdarg.h>`, `#ifdef`, etc.), so the cbindgen header is **not** fed
directly to `FFI::cdef`. Instead the four prototypes are inlined as a
string constant. Any change to the FFI surface in `c12n-core/src/ffi.rs`
that affects the four symbols above requires updating `Ffi::CDEF`.

## Development

```bash
composer install
composer test   # PHPUnit smoke tests + FFI integration (auto-skips if cdylib absent)
```

### FFI integration suite

`tests/PipelineFfiIntegrationTest.php` exercises the full roundtrip
against the real `libc12n_core` cdylib. The suite skips automatically
when the cdylib is not on disk, so `composer test` stays green on
fresh clones.

To run it end-to-end:

```bash
# 1. Build the cdylib (one-time per branch / Rust source change):
cargo build -p hop-top-c12n-core

# 2. Run PHPUnit pointing at the workspace target/ dir. The suite
#    accepts either a directory (auto-resolves the OS-specific lib
#    filename) or a direct file path.
C12N_CORE_LIB_PATH="$(pwd)/../target/debug" vendor/bin/phpunit
```

If `C12N_CORE_LIB_PATH` is unset, the suite resolves the cdylib at
`<workspace>/target/debug/libc12n_core.{dylib,so,dll}` relative to
this package — the same path `cargo build -p hop-top-c12n-core`
populates from the workspace root.

## Troubleshooting

| Symptom                                                  | Fix |
|----------------------------------------------------------|-----|
| `FFI is not enabled`                                     | Add `ffi.enable=true` to `php.ini`. |
| `c12n: native library not found at ...`                  | Run `composer install` to trigger the post-install download, or set `C12N_CORE_LIB_PATH=/abs/path/libc12n_core.dylib`. |
| `c12n: c12n_pipeline_new returned null`                  | Malformed config JSON or tokio runtime init failure. Check `maxConcurrency` is positive and `timeoutMs` fits in `u64`. |
| `c12n: pipeline is closed`                               | The pipeline was explicitly closed (or `__destruct`ed) before `evaluate()`. Construct a fresh `Pipeline`. |
| `c12n: <error message>` (from `evaluate`)                | The Rust core returned a JSON error envelope. The message is the verbatim string from `c12n_pipeline_evaluate`. |

## References

- ADR-0002 — `docs/adr/0002-c12n-php-ffi-binding.md` — locked decisions.
- `c12n-core/include/libc12n_core.h` — cbindgen output (source of truth
  for the FFI surface).
- `c12n_cgo.go` / `c12n-py/src/lib.rs` — reference bindings whose API
  surface c12n-php mirrors.

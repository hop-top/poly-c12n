---
status: shipped
personas: [middleware-developer, internal-tool-builder]
priority: P0
---

# US-0020: Install and load the PHP binding

As a tool author on PHP, I want `hop-top/c12n` installed and the
native library loaded, so `new Pipeline(...)` works instead of
throwing `C12nException` at first use.

## Use this when

- Adding c12n to a PHP service for the first time.
- CI cannot reach GitHub releases and the post-install download fails.
- Developing against a locally-built `libc12n_core` cdylib.

## Result

`HopTop\C12n\Ffi::get()` returns a loaded `\FFI` handle. Requires
PHP >= 8.4 with `ext-ffi` enabled, plus `libc12n_core.{dylib,so,dll}`
on disk.

## Steps

```bash
composer require hop-top/c12n
```

The `post-install-cmd` hook runs `HopTop\C12n\Installer::download`,
which fetches the cdylib from the c12n-core release matching
`composer.json#extra.c12n-core.version`.

**When no matching release is published, `composer install` fails.**
That is the current state of this repo — the installer aborts:

```
c12n-php: Downloading libc12n_core 0.1.0-alpha.0 for macos/aarch64 from
https://github.com/hop-top/poly-c12n/releases/download/c12n-core/v0.1.0-alpha.0/libc12n_core-macos-aarch64.tar.gz
Script HopTop\C12n\Installer::download handling the post-update-cmd event terminated with an exception
  c12n-php: HTTP 404 for https://.../libc12n_core-macos-aarch64.tar.gz; set
  C12N_CORE_LIB_PATH to bypass the installer if you have libc12n_core built locally
```

Build the cdylib yourself and set `C12N_CORE_LIB_PATH` to bypass:

```bash
cargo build -p hop-top-c12n-core --release
export C12N_CORE_LIB_PATH="$PWD/target/release/libc12n_core.dylib"
composer install
```

which reports:

```
> HopTop\C12n\Installer::download
c12n-php: C12N_CORE_LIB_PATH set, skipping download.
```

### `C12N_CORE_LIB_PATH` must be a FILE, not a directory

This is the single most common way to get a working build and a
broken runtime. `Ffi::libPath()` returns the env var **verbatim** and
`Ffi::get()` gates on `is_file()`, so a directory path resolves and
then fails to load:

```php
// WRONG — directory. Resolves, then throws on load.
// C12N_CORE_LIB_PATH=/path/to/target/release
//
// c12n: native library not found at /path/to/target/release.
// Run `composer install` to download it, or set the C12N_CORE_LIB_PATH env var.

// RIGHT — full path including the filename.
// C12N_CORE_LIB_PATH=/path/to/target/release/libc12n_core.dylib
```

Pass the filename even though `Ffi::libPath()`'s docblock and the
installer's own error message both suggest a directory, and even
though `PipelineFfiIntegrationTest::setUpBeforeClass` resolves
directories for you. That directory-resolution lives in the test
harness, not in `Ffi`.

Filename by platform: `libc12n_core.dylib` (macOS),
`libc12n_core.so` (Linux), `libc12n_core.dll` (Windows).

## Verify

```bash
cargo build -p hop-top-c12n-core --release
cd php && composer install
C12N_CORE_LIB_PATH="$(cd .. && pwd)/target/release/libc12n_core.dylib" \
  ./vendor/bin/phpunit --no-coverage
```

```
PHPUnit 11.5.56 by Sebastian Bergmann and contributors.

Runtime:       PHP 8.5.9
Configuration: .../php/phpunit.xml.dist

.............................................                     45 / 45 (100%)

Time: 00:00.154, Memory: 10.00 MB

OK (45 tests, 111 assertions)
```

Point `C12N_CORE_LIB_PATH` at the directory instead and the FFI suite
is silently **skipped** rather than failed — green output that proves
nothing. Check the test count.

## How it works

`Ffi::libPath()` resolves in a fixed order (ADR-0002 §4):

1. `C12N_CORE_LIB_PATH` env var — returned verbatim, no filename
   appended, no directory handling.
2. `composer.json#extra.c12n-core.local-path`.
3. `runtime/lib/libc12n_core.<ext>` under the package root, populated
   by the post-install `Installer`.

`Ffi::get()` then `is_file()`-checks the resolved path and calls
`\FFI::cdef(self::CDEF, $libPath)`. The `CDEF` constant declares only
the four public C symbols — PHP's FFI parser rejects the cbindgen
header's `#include` directives, so the header is not consumed
verbatim. The handle is cached per-process; `Ffi::reset()` clears it
(test-only).

## Tests

- [`php/tests/FfiTest.php`](../../php/tests/FfiTest.php) — path
  resolution order, cdef surface
- [`php/tests/InstallerTest.php`](../../php/tests/InstallerTest.php) —
  download, checksum, bypass behaviour
- [`php/tests/PipelineFfiIntegrationTest.php`](../../php/tests/PipelineFfiIntegrationTest.php)
  — skips gracefully when the cdylib is absent

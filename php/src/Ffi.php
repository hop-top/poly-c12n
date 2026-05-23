<?php

declare(strict_types=1);

namespace HopTop\C12n;

use HopTop\C12n\Exception\C12nException;

/**
 * Central FFI loader.
 *
 * Lazy-loads and caches the FFI handle bound to `libc12n_core`. The
 * native library path is resolved via {@see Ffi::libPath()} per the
 * order locked in ADR-0002 (env var → composer extra → vendor cache →
 * default).
 *
 * The cbindgen-emitted header (`libc12n_core.h`) is consumed verbatim
 * by `FFI::cdef`. PHP FFI's C parser does not support the full C
 * preprocessor, so we feed it a stripped-down `cdef` string that only
 * declares the four public symbols. This avoids parser failures on
 * `#include <stdarg.h>` etc.
 *
 * Cache scope: per-process. Tests may call {@see Ffi::reset()} to clear.
 */
final class Ffi
{
    private static ?\FFI $handle = null;

    /**
     * Minimal C declarations matching the cbindgen header. PHP's FFI
     * parser handles function prototypes + typedefs but rejects
     * standard-library includes. Mirror only what `Pipeline.php` calls.
     *
     * Source of truth: c12n-core/include/libc12n_core.h
     */
    public const CDEF = <<<'CDEF'
        void *c12n_pipeline_new(const char *config_json);
        char *c12n_pipeline_evaluate(const void *pipeline, const char *context_json);
        void  c12n_pipeline_free(void *pipeline);
        void  c12n_result_free(char *result);
        CDEF;

    /**
     * Return the cached FFI handle, loading on first call.
     */
    public static function get(): \FFI
    {
        if (self::$handle === null) {
            $libPath = self::libPath();
            if (!is_file($libPath)) {
                throw new C12nException(sprintf(
                    'c12n: native library not found at %s. '
                    . 'Run `composer install` to download it, or set the '
                    . 'C12N_CORE_LIB_PATH env var.',
                    $libPath,
                ));
            }

            try {
                self::$handle = \FFI::cdef(self::CDEF, $libPath);
            } catch (\FFI\Exception $e) {
                throw new C12nException(
                    sprintf('c12n: FFI::cdef failed for %s: %s', $libPath, $e->getMessage()),
                    previous: $e,
                );
            }
        }

        return self::$handle;
    }

    /**
     * Resolve the path to `libc12n_core.{so,dylib,dll}`.
     *
     * Resolution order (per ADR-0002 §4):
     *   1. `C12N_CORE_LIB_PATH` env var.
     *   2. `composer.json#extra.c12n-core.local-path` (project-local
     *      override; useful for development against a sibling cdylib).
     *   3. Default `runtime/lib/libc12n_core.<ext>` relative to package
     *      root, populated by the post-install Installer (T-0143).
     */
    public static function libPath(): string
    {
        $envPath = getenv('C12N_CORE_LIB_PATH');
        if (is_string($envPath) && $envPath !== '') {
            return $envPath;
        }

        $composerLocal = self::composerLocalPath();
        if ($composerLocal !== null) {
            return $composerLocal;
        }

        return self::defaultLibPath();
    }

    /**
     * Path declared in this package's `composer.json` under
     * `extra.c12n-core.local-path`, or null if absent / unreadable.
     */
    private static function composerLocalPath(): ?string
    {
        $composerJson = self::packageRoot() . '/composer.json';
        if (!is_file($composerJson)) {
            return null;
        }

        $contents = @file_get_contents($composerJson);
        if ($contents === false) {
            return null;
        }

        $decoded = json_decode($contents, true);
        if (!is_array($decoded)) {
            return null;
        }

        $extra = $decoded['extra']['c12n-core']['local-path'] ?? null;
        return is_string($extra) && $extra !== '' ? $extra : null;
    }

    /**
     * OS-aware default path inside the package's `runtime/lib/` cache.
     */
    private static function defaultLibPath(): string
    {
        $ext = match (PHP_OS_FAMILY) {
            'Darwin' => 'dylib',
            'Windows' => 'dll',
            default => 'so',
        };

        return self::packageRoot() . '/runtime/lib/libc12n_core.' . $ext;
    }

    private static function packageRoot(): string
    {
        // src/Ffi.php → c12n-php package root is the parent directory.
        return dirname(__DIR__);
    }

    /**
     * Clear the cached FFI handle. Test-only.
     */
    public static function reset(): void
    {
        self::$handle = null;
    }
}

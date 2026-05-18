<?php

declare(strict_types=1);

namespace HopTop\C12n\Tests;

use HopTop\C12n\Ffi;
use PHPUnit\Framework\TestCase;

/**
 * Path-resolver coverage for {@see Ffi::libPath()}.
 *
 * The resolver order locked in ADR-0002 §4:
 *   1. C12N_CORE_LIB_PATH env var (highest)
 *   2. composer.json#extra.c12n-core.local-path
 *   3. runtime/lib/libc12n_core.<ext> default (lowest)
 *
 * Loading the cdylib itself is out of scope — that requires a built
 * libc12n_core in `runtime/lib/` and lands with T-0142.
 */
final class FfiTest extends TestCase
{
    private string|false $originalEnv;

    protected function setUp(): void
    {
        $this->originalEnv = getenv('C12N_CORE_LIB_PATH');
        putenv('C12N_CORE_LIB_PATH');
        Ffi::reset();
    }

    protected function tearDown(): void
    {
        if ($this->originalEnv === false) {
            putenv('C12N_CORE_LIB_PATH');
        } else {
            putenv('C12N_CORE_LIB_PATH=' . $this->originalEnv);
        }
        Ffi::reset();
    }

    public function testLibPathEnvVarTakesPrecedence(): void
    {
        putenv('C12N_CORE_LIB_PATH=/tmp/custom/libc12n_core.dylib');

        self::assertSame(
            '/tmp/custom/libc12n_core.dylib',
            Ffi::libPath(),
        );
    }

    public function testLibPathFallsBackToDefaultWhenNoOverrides(): void
    {
        $path = Ffi::libPath();

        // Default lives under the package's runtime/lib/ directory.
        self::assertStringContainsString('/runtime/lib/libc12n_core.', $path);

        // OS-aware extension.
        $expectedExt = match (PHP_OS_FAMILY) {
            'Darwin' => 'dylib',
            'Windows' => 'dll',
            default => 'so',
        };
        self::assertStringEndsWith('.' . $expectedExt, $path);
    }

    public function testLibPathIgnoresEmptyEnvVar(): void
    {
        putenv('C12N_CORE_LIB_PATH=');

        $path = Ffi::libPath();

        self::assertStringContainsString('runtime/lib/libc12n_core.', $path);
    }
}

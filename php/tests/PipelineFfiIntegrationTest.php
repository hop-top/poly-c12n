<?php

declare(strict_types=1);

namespace HopTop\C12n\Tests;

use HopTop\C12n\ClassificationContext;
use HopTop\C12n\Exception\PipelineException;
use HopTop\C12n\Ffi;
use HopTop\C12n\Pipeline;
use HopTop\C12n\PipelineConfig;
use HopTop\C12n\PipelineResult;
use PHPUnit\Framework\Attributes\RequiresPhpExtension;
use PHPUnit\Framework\TestCase;

/**
 * Full FFI roundtrip coverage via the real `libc12n_core` cdylib.
 *
 * Build the cdylib first:
 *
 * ```bash
 * cargo build -p hop-top-c12n-core
 * C12N_CORE_LIB_PATH="$(pwd)/../target/debug" vendor/bin/phpunit
 * ```
 *
 * The suite skips gracefully (with `markTestSkipped`) when the cdylib
 * is not present at the resolved path, so `composer test` stays green
 * locally even without a prior `cargo build`.
 *
 * Tests cover T-0140 / T-0141:
 *  - Construct + evaluate + close roundtrip.
 *  - Idempotent close + evaluate-after-close.
 *  - Error envelopes (invalid context, null pointer paths).
 *  - JSON-shape parity with the Go cgo and Python PyO3 bindings for
 *    an empty signal set.
 *
 * Each test resets the cached FFI handle and re-anchors the
 * `C12N_CORE_LIB_PATH` environment variable so the suite is
 * order-independent and survives parallel runner invocations.
 */
#[RequiresPhpExtension('ffi')]
final class PipelineFfiIntegrationTest extends TestCase
{
    private static ?string $libPath = null;

    private string|false $originalEnv = false;

    public static function setUpBeforeClass(): void
    {
        // Resolve the cdylib directory relative to the c12n workspace
        // root: <repo>/target/debug/. The PHP package lives at
        // <repo>/php/, hence `dirname(php-root)` is <repo>.
        $workspaceRoot = dirname(__DIR__, 2);
        $envOverride = getenv('C12N_CORE_LIB_PATH');

        if (is_string($envOverride) && $envOverride !== '') {
            // Caller-supplied path. Accept either a directory or a
            // direct file path; resolve the OS-specific filename when
            // a directory is given.
            $libPath = is_dir($envOverride)
                ? rtrim($envOverride, '/') . '/' . self::libFilename()
                : $envOverride;
        } else {
            $libPath = $workspaceRoot . '/target/debug/' . self::libFilename();
        }

        self::$libPath = $libPath;
    }

    protected function setUp(): void
    {
        if (self::$libPath === null || !is_file(self::$libPath)) {
            self::markTestSkipped(sprintf(
                'libc12n_core cdylib not present at %s. '
                . 'Run `cargo build -p hop-top-c12n-core` first or set '
                . 'C12N_CORE_LIB_PATH to a directory containing %s.',
                self::$libPath ?? '(unresolved)',
                self::libFilename(),
            ));
        }

        $this->originalEnv = getenv('C12N_CORE_LIB_PATH');
        putenv('C12N_CORE_LIB_PATH=' . self::$libPath);
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

    // -----------------------------------------------------------------
    // T-0140: full FFI roundtrip
    // -----------------------------------------------------------------

    public function testEvaluateReturnsValidEnvelopeForDefaultConfig(): void
    {
        $pipeline = new Pipeline(new PipelineConfig());

        $json = $pipeline->evaluate(new ClassificationContext(
            text: 'Write a Python function to sort a list',
        ));

        $decoded = json_decode($json, true);

        self::assertIsArray($decoded);
        self::assertArrayHasKey('results', $decoded);
        self::assertArrayHasKey('errors', $decoded);
        self::assertArrayHasKey('duration_ms', $decoded);
        self::assertSame([], $decoded['results']);
        self::assertSame([], $decoded['errors']);
        self::assertIsInt($decoded['duration_ms']);
        self::assertGreaterThanOrEqual(0, $decoded['duration_ms']);

        $pipeline->close();
    }

    public function testPipelineResultParsesRoundtripJson(): void
    {
        $pipeline = new Pipeline(new PipelineConfig());

        $json = $pipeline->evaluate(new ClassificationContext(
            text: 'classify this prompt',
        ));

        $result = new PipelineResult($json);

        self::assertSame([], $result->results());
        self::assertSame([], $result->errors());
        self::assertFalse($result->hasErrors());
        self::assertGreaterThanOrEqual(0, $result->durationMs());

        $pipeline->close();
    }

    public function testEvaluatePropagatesAllContextFields(): void
    {
        $pipeline = new Pipeline(new PipelineConfig(maxConcurrency: 4, timeoutMs: 2000));

        $json = $pipeline->evaluate(new ClassificationContext(
            text: 'full payload roundtrip',
            history: ['previous'],
            headers: ['x-trace' => 'abc'],
            imageUrl: 'https://example.com/x.png',
            config: ['mode' => 'strict'],
        ));

        $decoded = json_decode($json, true);
        self::assertIsArray($decoded);
        self::assertArrayNotHasKey('error', $decoded, 'FFI rejected populated context');
        self::assertSame([], $decoded['results']);

        $pipeline->close();
    }

    // -----------------------------------------------------------------
    // T-0140: pipeline lifecycle
    // -----------------------------------------------------------------

    public function testCloseIsIdempotent(): void
    {
        $pipeline = new Pipeline(new PipelineConfig());

        $pipeline->close();
        $pipeline->close(); // second close — must not throw.

        $this->expectNotToPerformAssertions();
    }

    public function testEvaluateAfterCloseThrows(): void
    {
        $pipeline = new Pipeline(new PipelineConfig());
        $pipeline->close();

        $this->expectException(PipelineException::class);
        $this->expectExceptionMessage('pipeline is closed');

        $pipeline->evaluate(new ClassificationContext(text: 'post-close'));
    }

    public function testDestructorCleansUpUnclosedPipeline(): void
    {
        // Construct and let go out of scope — __destruct fires close().
        // Survival of this test (no double-free / crash on cleanup) is
        // the assertion.
        $pipeline = new Pipeline(new PipelineConfig());
        $json = $pipeline->evaluate(new ClassificationContext(text: 'hi'));
        unset($pipeline);

        $decoded = json_decode($json, true);
        self::assertIsArray($decoded);
        self::assertArrayHasKey('duration_ms', $decoded);
    }

    // -----------------------------------------------------------------
    // T-0141: error paths
    // -----------------------------------------------------------------

    public function testConstructorThrowsOnInvalidConfigJson(): void
    {
        // Drive an invalid config JSON through the FFI by talking
        // directly to the FFI handle. PipelineConfig's typed
        // constructor prevents PHP-side construction of an invalid
        // shape, but the wire-layer null guard in Pipeline matters
        // for hand-rolled callers and for fuzz-style inputs.
        $ffi = Ffi::get();
        $ptr = $ffi->c12n_pipeline_new('not json');

        self::assertNull(
            $ptr,
            'c12n_pipeline_new should return null for invalid config JSON',
        );
    }

    public function testEvaluateReturnsErrorEnvelopeForMalformedContext(): void
    {
        // Hand-craft a context JSON the Rust deserialiser rejects (the
        // FFI returns a JSON error envelope rather than a null pointer
        // for invalid contexts on a valid pipeline).
        $ffi = Ffi::get();
        $cfgJson = '{"max_concurrency":4,"timeout_ms":1000}';
        $pipelinePtr = $ffi->c12n_pipeline_new($cfgJson);

        try {
            self::assertNotNull($pipelinePtr);

            // `text` must be a string per the Rust ClassificationContext.
            $badContext = '{"text":123}';
            $resultPtr = $ffi->c12n_pipeline_evaluate($pipelinePtr, $badContext);
            self::assertNotNull($resultPtr);

            $resultJson = \FFI::string($resultPtr);
            $ffi->c12n_result_free($resultPtr);

            $decoded = json_decode($resultJson, true);
            self::assertIsArray($decoded);
            self::assertArrayHasKey('error', $decoded);
            self::assertIsString($decoded['error']);
            self::assertNotSame('', $decoded['error']);

            // PipelineResult surfaces the FFI error envelope as a
            // PipelineException with a descriptive message.
            $this->expectException(PipelineException::class);
            $this->expectExceptionMessageMatches('/invalid context JSON|expected/i');
            new PipelineResult($resultJson);
        } finally {
            if ($pipelinePtr !== null) {
                $ffi->c12n_pipeline_free($pipelinePtr);
            }
        }
    }

    // -----------------------------------------------------------------
    // T-0141: parity with Go + Python
    // -----------------------------------------------------------------

    public function testEmptyPipelineJsonShapeMatchesCanonicalParity(): void
    {
        // Canonical parity input — same prompt used by Go's
        // TestIntegration_PipelineEmptyResult / Python equivalent.
        $pipeline = new Pipeline(new PipelineConfig());

        $json = $pipeline->evaluate(new ClassificationContext(
            text: 'Write a Python function to sort a list',
        ));

        $decoded = json_decode($json, true);
        self::assertIsArray($decoded);

        // Wire-shape contract for an empty signal set, asserted by
        // sibling bindings:
        //   - Go (`go/integration_test.go::TestIntegration_PipelineEmptyResult`)
        //     wraps the same JSON via `ParseResult` and asserts a
        //     non-nil result with zero signals.
        //   - The Rust FFI unit test (`core/src/ffi.rs::roundtrip_create_evaluate_free`)
        //     asserts `results` is an array, `errors` is an array, and
        //     `duration_ms` is a number.
        //   - This PHP test asserts the literal shape: top-level keys,
        //     types, and empty-collection invariants.
        self::assertSame(
            ['results', 'errors', 'duration_ms'],
            array_keys($decoded),
            'top-level keys must match Go/Python/Rust envelope order',
        );
        self::assertIsArray($decoded['results']);
        self::assertIsArray($decoded['errors']);
        self::assertIsInt($decoded['duration_ms']);
        self::assertSame([], $decoded['results']);
        self::assertSame([], $decoded['errors']);

        $pipeline->close();
    }

    // -----------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------

    private static function libFilename(): string
    {
        return match (PHP_OS_FAMILY) {
            'Darwin' => 'libc12n_core.dylib',
            'Windows' => 'libc12n_core.dll',
            default => 'libc12n_core.so',
        };
    }
}

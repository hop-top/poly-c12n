<?php

declare(strict_types=1);

namespace HopTop\C12n;

use FFI;
use HopTop\C12n\Exception\PipelineException;
use HopTop\Kit\Output\Output as KitOutput;

/**
 * PHP wrapper around the c12n-core classification pipeline.
 *
 * Lifecycle mirrors the Go cgo binding (`c12n_cgo.go`):
 *
 *   $pipeline = new Pipeline(new PipelineConfig(maxConcurrency: 8, timeoutMs: 5000));
 *   $json     = $pipeline->evaluate(new ClassificationContext(text: 'hello'));
 *   $result   = new PipelineResult($json);
 *   $pipeline->close();      // optional — __destruct calls it
 *
 * `close()` is idempotent; double-close (manual close() then __destruct,
 * or two manual calls) is a no-op after the first invocation. The
 * underlying opaque pointer is nulled out on close so subsequent
 * `evaluate()` calls throw {@see PipelineException} rather than
 * dereferencing freed memory.
 *
 * Not safe for concurrent use without external synchronisation.
 */
final class Pipeline
{
    private FFI $ffi;
    private ?FFI\CData $handle;
    private bool $closed = false;

    public function __construct(PipelineConfig $config)
    {
        $this->ffi = Ffi::get();

        $configJson = json_encode($config->toArray(), JSON_THROW_ON_ERROR);

        // FFI string args are not GC-rooted across the call — keep them
        // anchored on the PHP-side for the duration of the invocation.
        $ptr = $this->ffi->c12n_pipeline_new($configJson);

        if (FFI::isNull($ptr)) {
            throw new PipelineException(
                'c12n: c12n_pipeline_new returned null (invalid config JSON or runtime init failure)',
            );
        }

        $this->handle = $ptr;
        $this->logLifecycle('pipeline.created');
    }

    /**
     * Evaluate a context. Returns the raw JSON envelope emitted by the
     * FFI. Wrap with `new PipelineResult($json)` to access typed
     * accessors.
     */
    public function evaluate(ClassificationContext $ctx): string
    {
        if ($this->closed || $this->handle === null) {
            throw new PipelineException('c12n: pipeline is closed');
        }

        $contextJson = json_encode($ctx->toArray(), JSON_THROW_ON_ERROR);

        $resultPtr = $this->ffi->c12n_pipeline_evaluate($this->handle, $contextJson);

        if (FFI::isNull($resultPtr)) {
            throw new PipelineException(
                'c12n: c12n_pipeline_evaluate returned null',
            );
        }

        try {
            // Copy the C string into PHP memory before freeing the
            // FFI-owned buffer. FFI::string clones into the PHP heap.
            $json = FFI::string($resultPtr);
        } finally {
            $this->ffi->c12n_result_free($resultPtr);
        }

        return $json;
    }

    /**
     * Free the underlying FFI pipeline. Idempotent — safe to call
     * multiple times and from __destruct after manual close().
     */
    public function close(): void
    {
        if ($this->closed) {
            return;
        }
        $this->closed = true;

        if ($this->handle !== null) {
            $this->ffi->c12n_pipeline_free($this->handle);
            $this->handle = null;
        }

        $this->logLifecycle('pipeline.closed');
    }

    public function __destruct()
    {
        $this->close();
    }

    /**
     * Emit a lifecycle event for fleet-wide observability.
     *
     * kit-php's logger / event-bus surface is not yet exported
     * (HopTop\Kit\Output\Output is an empty placeholder as of
     * kit@0.4.0-experimental.1). The dependency is declared per
     * ADR-0002 §7 so the import path is reserved; the call is a no-op
     * today and will start emitting once kit-php ships a Logger class.
     */
    private function logLifecycle(string $event): void
    {
        // Anchor the autoloaded class so static analysis sees the dep.
        // Becomes a real logger.info() call once kit-php exports one.
        if (class_exists(KitOutput::class)) {
            // intentionally empty
        }
        // $event reserved for future kit-php logger.info($event, [...])
        unset($event);
    }
}

<?php

declare(strict_types=1);

namespace HopTop\C12n;

/**
 * Configuration for a {@see Pipeline}.
 *
 * Mirrors the Rust `PipelineConfig` consumed by `c12n_pipeline_new`:
 *
 * ```json
 * {"max_concurrency": 8, "timeout_ms": 5000}
 * ```
 */
final class PipelineConfig
{
    public function __construct(
        public readonly int $maxConcurrency = 8,
        public readonly int $timeoutMs = 5000,
    ) {
    }

    /**
     * @return array{max_concurrency: int, timeout_ms: int}
     */
    public function toArray(): array
    {
        return [
            'max_concurrency' => $this->maxConcurrency,
            'timeout_ms' => $this->timeoutMs,
        ];
    }
}

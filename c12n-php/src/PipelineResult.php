<?php

declare(strict_types=1);

namespace HopTop\C12n;

use HopTop\C12n\Exception\PipelineException;

/**
 * Parsed pipeline output.
 *
 * Mirrors the Go `c12n.PipelineResult` accessor surface. The wire shape
 * follows the FFI envelope emitted by `c12n_pipeline_evaluate`:
 *
 * ```json
 * {
 *   "results": [ {SignalResult}, ... ],
 *   "errors":  [ "string", ... ],
 *   "duration_ms": 12
 * }
 * ```
 *
 * Note the FFI returns `duration_ms` (millis), unlike the cgo Go binding
 * which uses `duration_ns`. c12n-php mirrors the FFI envelope verbatim.
 */
final class PipelineResult
{
    /** @var list<SignalResult> */
    private readonly array $results;

    /** @var list<string> */
    private readonly array $errors;

    private readonly int $durationMs;

    public function __construct(string $rawJson)
    {
        $decoded = json_decode($rawJson, true);

        if (!is_array($decoded)) {
            throw new PipelineException(
                sprintf('c12n: failed to parse result JSON: %s', json_last_error_msg()),
            );
        }

        // FFI error envelope: {"error": "..."}
        if (isset($decoded['error']) && is_string($decoded['error'])) {
            throw new PipelineException(sprintf('c12n: %s', $decoded['error']));
        }

        $rawResults = $decoded['results'] ?? [];
        $rawErrors = $decoded['errors'] ?? [];

        $results = [];
        if (is_array($rawResults)) {
            foreach ($rawResults as $r) {
                if (is_array($r)) {
                    $results[] = SignalResult::fromArray($r);
                }
            }
        }

        $errors = [];
        if (is_array($rawErrors)) {
            foreach ($rawErrors as $e) {
                $errors[] = is_string($e) ? $e : json_encode($e, JSON_THROW_ON_ERROR);
            }
        }

        $this->results = $results;
        $this->errors = $errors;
        $this->durationMs = (int) ($decoded['duration_ms'] ?? 0);
    }

    /**
     * @return list<SignalResult>
     */
    public function results(): array
    {
        return $this->results;
    }

    /**
     * @return list<string>
     */
    public function errors(): array
    {
        return $this->errors;
    }

    public function durationMs(): int
    {
        return $this->durationMs;
    }

    public function hasErrors(): bool
    {
        return $this->errors !== [];
    }

    /**
     * First {@see SignalResult} matching the given signal type, or null.
     */
    public function signal(string $type): ?SignalResult
    {
        foreach ($this->results as $r) {
            if ($r->type === $type) {
                return $r;
            }
        }
        return null;
    }

    /**
     * Confidence for the first matching signal type, or 0.0 if absent.
     *
     * Matches Go's `PipelineResult.Confidence(SignalType) float64`.
     */
    public function confidence(string $type): float
    {
        $s = $this->signal($type);
        return $s?->confidence ?? 0.0;
    }
}

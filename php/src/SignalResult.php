<?php

declare(strict_types=1);

namespace HopTop\C12n;

/**
 * Single signal's classification output.
 *
 * Mirrors the Rust `SignalResult` struct serialised by the FFI layer.
 */
final class SignalResult
{
    /**
     * @param list<string>        $labels
     * @param array<string,mixed> $metadata
     */
    public function __construct(
        public readonly string $name,
        public readonly string $type,
        public readonly float $confidence,
        public readonly array $labels = [],
        public readonly array $metadata = [],
    ) {
    }

    /**
     * Hydrate from a decoded JSON object.
     *
     * @param array<string,mixed> $raw
     */
    public static function fromArray(array $raw): self
    {
        $labels = $raw['labels'] ?? [];
        $metadata = $raw['metadata'] ?? [];

        return new self(
            name: (string) ($raw['name'] ?? ''),
            type: (string) ($raw['signal_type'] ?? ''),
            confidence: (float) ($raw['confidence'] ?? 0.0),
            labels: is_array($labels) ? array_values($labels) : [],
            metadata: is_array($metadata) ? $metadata : [],
        );
    }
}

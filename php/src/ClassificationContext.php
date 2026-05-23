<?php

declare(strict_types=1);

namespace HopTop\C12n;

/**
 * Input to {@see Pipeline::evaluate()}.
 *
 * Mirrors the Rust `ClassificationContext` struct serialised by the FFI
 * layer. Null slice/map fields are normalised to empty arrays on
 * encoding, matching the Go binding's `normalizeContext` behaviour
 * (belt-and-braces against the Rust `#[serde(default)]` attributes).
 */
final class ClassificationContext
{
    /**
     * @param list<string>         $history
     * @param array<string,string> $headers
     * @param array<string,mixed>  $config
     */
    public function __construct(
        public readonly string $text,
        public readonly array $history = [],
        public readonly array $headers = [],
        public readonly ?string $imageUrl = null,
        public readonly array $config = [],
    ) {
    }

    /**
     * Normalised array representation. Null fields default to empty
     * collections to match the Go `normalizeContext` invariant.
     *
     * The `image_url` key is included only when set (matches Rust
     * `#[serde(skip_serializing_if = "Option::is_none")]` semantics in
     * the cgo `ClassificationContext` struct's `omitempty` tag).
     *
     * @return array<string,mixed>
     */
    public function toArray(): array
    {
        $out = [
            'text' => $this->text,
            'history' => $this->history,
            'headers' => $this->headers,
            'config' => $this->config,
        ];

        if ($this->imageUrl !== null) {
            $out['image_url'] = $this->imageUrl;
        }

        return $out;
    }

    /**
     * JSON encoding suitable for the FFI wire. Empty PHP arrays in map
     * positions (`headers`, `config`) become `{}` rather than `[]` so
     * Rust's serde `HashMap` deserialiser accepts them. Go's
     * `map[string]string` and Python's `dict` both encode empty as `{}`,
     * so this normalisation keeps the wire shape consistent across the
     * three bindings.
     *
     * Tested by the FFI integration suite; the array-typed
     * {@see toArray()} keeps the value-object contract intact.
     */
    public function toFfiJson(): string
    {
        $payload = [
            'text' => $this->text,
            'history' => $this->history,
            'headers' => $this->headers === [] ? new \stdClass() : $this->headers,
            'config' => $this->config === [] ? new \stdClass() : $this->config,
        ];

        if ($this->imageUrl !== null) {
            $payload['image_url'] = $this->imageUrl;
        }

        return json_encode($payload, JSON_THROW_ON_ERROR);
    }
}

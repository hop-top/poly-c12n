<?php

declare(strict_types=1);

namespace HopTop\C12n\Tests;

use HopTop\C12n\ClassificationContext;
use HopTop\C12n\Exception\PipelineException;
use HopTop\C12n\PipelineConfig;
use HopTop\C12n\PipelineResult;
use HopTop\C12n\SignalResult;
use PHPUnit\Framework\TestCase;

/**
 * Smoke tests for the marshalling and value-object surface.
 *
 * These tests deliberately avoid the FFI layer; full FFI end-to-end
 * coverage lands with T-0142 (PHPUnit fixture set), which depends on
 * the cdylib + header pair being in place at `runtime/lib/`.
 */
final class PipelineTest extends TestCase
{
    public function testPipelineConfigRoundtrip(): void
    {
        $cfg = new PipelineConfig(maxConcurrency: 4, timeoutMs: 2500);

        self::assertSame(
            ['max_concurrency' => 4, 'timeout_ms' => 2500],
            $cfg->toArray(),
        );
    }

    public function testPipelineConfigDefaults(): void
    {
        $cfg = new PipelineConfig();

        self::assertSame(8, $cfg->maxConcurrency);
        self::assertSame(5000, $cfg->timeoutMs);
    }

    public function testClassificationContextNormalisesEmptyCollections(): void
    {
        $ctx = new ClassificationContext(text: 'hi');

        $out = $ctx->toArray();

        self::assertSame('hi', $out['text']);
        self::assertSame([], $out['history']);
        self::assertSame([], $out['headers']);
        self::assertSame([], $out['config']);
        self::assertArrayNotHasKey('image_url', $out);
    }

    public function testClassificationContextIncludesImageUrlWhenSet(): void
    {
        $ctx = new ClassificationContext(
            text: 'hi',
            imageUrl: 'https://example.com/x.png',
        );

        $out = $ctx->toArray();

        self::assertSame('https://example.com/x.png', $out['image_url']);
    }

    public function testClassificationContextPreservesProvidedValues(): void
    {
        $ctx = new ClassificationContext(
            text: 'hello',
            history: ['previous'],
            headers: ['x-trace' => 'abc'],
            imageUrl: null,
            config: ['mode' => 'strict'],
        );

        $out = $ctx->toArray();

        self::assertSame(['previous'], $out['history']);
        self::assertSame(['x-trace' => 'abc'], $out['headers']);
        self::assertSame(['mode' => 'strict'], $out['config']);
    }

    public function testPipelineResultParsesEmptyEnvelope(): void
    {
        $raw = '{"results":[],"errors":[],"duration_ms":0}';

        $result = new PipelineResult($raw);

        self::assertSame([], $result->results());
        self::assertSame([], $result->errors());
        self::assertSame(0, $result->durationMs());
        self::assertFalse($result->hasErrors());
        self::assertNull($result->signal('Toxicity'));
        self::assertSame(0.0, $result->confidence('Toxicity'));
    }

    public function testPipelineResultParsesPopulatedEnvelope(): void
    {
        $raw = json_encode([
            'results' => [
                [
                    'name' => 'toxicity-default',
                    'signal_type' => 'Toxicity',
                    'confidence' => 0.87,
                    'labels' => ['severe', 'profanity'],
                    'metadata' => ['model' => 'unitary/toxic-bert'],
                ],
            ],
            'errors' => ['SignalFailed: pii timed out'],
            'duration_ms' => 42,
        ], JSON_THROW_ON_ERROR);

        $result = new PipelineResult($raw);

        self::assertCount(1, $result->results());
        self::assertSame(42, $result->durationMs());
        self::assertTrue($result->hasErrors());
        self::assertSame(['SignalFailed: pii timed out'], $result->errors());

        $signal = $result->signal('Toxicity');
        self::assertInstanceOf(SignalResult::class, $signal);
        self::assertSame('toxicity-default', $signal->name);
        self::assertSame(0.87, $signal->confidence);
        self::assertSame(['severe', 'profanity'], $signal->labels);
        self::assertSame(['model' => 'unitary/toxic-bert'], $signal->metadata);

        self::assertSame(0.87, $result->confidence('Toxicity'));
    }

    public function testPipelineResultThrowsOnFfiErrorEnvelope(): void
    {
        $this->expectException(PipelineException::class);
        $this->expectExceptionMessageMatches('/null pipeline pointer/');

        new PipelineResult('{"error":"null pipeline pointer"}');
    }

    public function testPipelineResultThrowsOnInvalidJson(): void
    {
        $this->expectException(PipelineException::class);

        new PipelineResult('not json');
    }
}

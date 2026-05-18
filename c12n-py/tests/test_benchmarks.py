"""Performance benchmarks for c12n-py components.

Run with: python -m pytest c12n-py/tests/test_benchmarks.py -v
Or standalone: python c12n-py/tests/test_benchmarks.py
"""

from __future__ import annotations

import json
import os
import time
from dataclasses import asdict
from typing import Dict, List

import pytest

from c12n.config import Config, default_config
from c12n.middleware import C12NMiddleware, has_signal, signal_confidence
from c12n.router import SignalRouter, SignalRule

_skip_bench = pytest.mark.skipif(
    os.environ.get("C12N_BENCH") != "1",
    reason="Benchmarks skipped; set C12N_BENCH=1 to run",
)


# -------------------------------------------------------------------
# Benchmark harness
# -------------------------------------------------------------------


def benchmark(fn, iterations=1000, label=""):
    """Run fn N times, return timing stats in microseconds."""
    times: List[float] = []
    for _ in range(iterations):
        start = time.perf_counter()
        fn()
        elapsed = (time.perf_counter() - start) * 1_000_000
        times.append(elapsed)
    times.sort()
    n = len(times)
    result: Dict[str, float] = {
        "min": times[0],
        "avg": sum(times) / n,
        "p50": times[n // 2],
        "p95": times[int(n * 0.95)],
        "p99": times[int(n * 0.99)],
        "max": times[-1],
    }
    if label:
        print(f"\n{label}:")
        for k, v in result.items():
            print(f"  {k}: {v:.1f} us")
    return result


# -------------------------------------------------------------------
# Shared data
# -------------------------------------------------------------------

SAMPLE_RESULT = {
    "results": [
        {
            "name": "keyword",
            "signal_type": "Keyword",
            "confidence": 0.85,
            "labels": ["greeting"],
            "metadata": {},
        },
        {
            "name": "format",
            "signal_type": "OutputFormat",
            "confidence": 1.0,
            "labels": ["JSON"],
            "metadata": {},
        },
        {
            "name": "code",
            "signal_type": "CodeContent",
            "confidence": 0.9,
            "labels": ["Python", "Generate"],
            "metadata": {"has_code_fence": False},
        },
        {
            "name": "cost",
            "signal_type": "CostEstimate",
            "confidence": 0.7,
            "labels": ["small"],
            "metadata": {"total_cost": 0.003},
        },
        {
            "name": "complexity",
            "signal_type": "Complexity",
            "confidence": 0.8,
            "labels": ["complex"],
            "metadata": {"hard_score": 0.9, "easy_score": 0.1},
        },
    ],
    "errors": [],
    "duration_ns": 1500000,
}

SAMPLE_JSON = json.dumps(SAMPLE_RESULT)

OPENAI_BODY = {
    "model": "gpt-4",
    "messages": [
        {"role": "system", "content": "You are a helpful assistant."},
        {"role": "user", "content": "Write a Python function to sort a list."},
    ],
    "temperature": 0.7,
    "max_tokens": 1024,
}


def _make_mock_pipeline():
    class _Result:
        def __init__(self, raw):
            self._raw = raw

        def json(self):
            return self._raw

    class MockPipeline:
        def evaluate(self, text, **kwargs):
            return _Result(SAMPLE_JSON)

    return MockPipeline()


# -------------------------------------------------------------------
# 1. Config creation
# -------------------------------------------------------------------


@pytest.mark.benchmark
@_skip_bench
def test_bench_config_creation():
    """Config creation should be < 100us."""
    stats = benchmark(default_config, iterations=5000)
    assert stats["p95"] < 100, (
        f"Config creation p95={stats['p95']:.1f}us exceeds 100us"
    )


# -------------------------------------------------------------------
# 2. Config serialization
# -------------------------------------------------------------------


@pytest.mark.benchmark
@_skip_bench
def test_bench_config_serialization():
    """Config to dict conversion should be < 200us."""
    cfg = default_config()

    def serialize():
        asdict(cfg)

    stats = benchmark(serialize, iterations=5000)
    assert stats["p95"] < 200, (
        f"Config serialization p95={stats['p95']:.1f}us exceeds 200us"
    )


# -------------------------------------------------------------------
# 3. JSON parsing (PipelineResult format)
# -------------------------------------------------------------------


@pytest.mark.benchmark
@_skip_bench
def test_bench_json_parsing():
    """Parsing realistic pipeline result JSON should be < 50us."""
    raw = SAMPLE_JSON

    def parse():
        json.loads(raw)

    stats = benchmark(parse, iterations=5000)
    assert stats["p95"] < 50, (
        f"JSON parsing p95={stats['p95']:.1f}us exceeds 50us"
    )


# -------------------------------------------------------------------
# 4. Signal lookup
# -------------------------------------------------------------------


@pytest.mark.benchmark
@_skip_bench
def test_bench_signal_lookup():
    """has_signal / signal_confidence should be < 10us."""
    scope = {"c12n.signals": SAMPLE_RESULT}

    def lookup():
        has_signal(scope, "CodeContent")
        signal_confidence(scope, "Complexity")
        has_signal(scope, "NonExistent")

    stats = benchmark(lookup, iterations=5000)
    assert stats["p95"] < 10, (
        f"Signal lookup p95={stats['p95']:.1f}us exceeds 10us"
    )


# -------------------------------------------------------------------
# 5. Router rule evaluation
# -------------------------------------------------------------------


@pytest.mark.benchmark
@_skip_bench
def test_bench_router_evaluation():
    """Router.calculate_strong_win_rate should be < 200us."""
    pipeline = _make_mock_pipeline()
    router = SignalRouter(
        pipeline,
        rules=[
            SignalRule(
                "Complexity",
                lambda r: "complex" in r.get("labels", []),
                win_rate=0.9,
                priority=10,
            ),
            SignalRule(
                "CodeContent",
                lambda r: r.get("confidence", 0) > 0.7,
                win_rate=0.8,
                priority=5,
            ),
            SignalRule(
                "Keyword",
                lambda r: r.get("confidence", 0) > 0.5,
                win_rate=0.6,
                priority=1,
            ),
        ],
    )

    def evaluate():
        router.calculate_strong_win_rate("test prompt")

    stats = benchmark(evaluate, iterations=2000)
    assert stats["p95"] < 200, (
        f"Router eval p95={stats['p95']:.1f}us exceeds 200us"
    )


# -------------------------------------------------------------------
# 6. Middleware text extraction
# -------------------------------------------------------------------


@pytest.mark.benchmark
@_skip_bench
def test_bench_text_extraction():
    """_default_extractor with OpenAI chat format should be < 5us."""

    def extract():
        C12NMiddleware._default_extractor(OPENAI_BODY)

    stats = benchmark(extract, iterations=5000)
    assert stats["p95"] < 5, (
        f"Text extraction p95={stats['p95']:.1f}us exceeds 5us"
    )


# -------------------------------------------------------------------
# Standalone runner
# -------------------------------------------------------------------


if __name__ == "__main__":
    print("=" * 60)
    print("c12n-py benchmarks")
    print("=" * 60)

    benchmark(default_config, label="Config creation")

    cfg = default_config()
    benchmark(lambda: asdict(cfg), label="Config serialization")

    benchmark(lambda: json.loads(SAMPLE_JSON), label="JSON parsing")

    scope = {"c12n.signals": SAMPLE_RESULT}
    benchmark(
        lambda: (
            has_signal(scope, "CodeContent"),
            signal_confidence(scope, "Complexity"),
            has_signal(scope, "NonExistent"),
        ),
        label="Signal lookup (3 ops)",
    )

    pipeline = _make_mock_pipeline()
    router = SignalRouter(
        pipeline,
        rules=[
            SignalRule(
                "Complexity",
                lambda r: "complex" in r.get("labels", []),
                win_rate=0.9,
                priority=10,
            ),
            SignalRule(
                "CodeContent",
                lambda r: r.get("confidence", 0) > 0.7,
                win_rate=0.8,
                priority=5,
            ),
        ],
    )
    benchmark(
        lambda: router.calculate_strong_win_rate("test"),
        label="Router evaluation",
    )

    benchmark(
        lambda: C12NMiddleware._default_extractor(OPENAI_BODY),
        label="Middleware text extraction",
    )

    print("\n" + "=" * 60)
    print("Done.")

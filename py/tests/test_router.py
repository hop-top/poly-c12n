"""Tests for SignalRouter."""

from __future__ import annotations

import json
from types import SimpleNamespace
from unittest.mock import MagicMock

import pytest

from c12n.router import SignalRouter, SignalRule


def _make_pipeline(results: list[dict]) -> MagicMock:
    """Create a mock pipeline returning canned JSON."""
    pipeline = MagicMock()
    pipeline.evaluate.return_value = json.dumps(
        {"results": results}
    )
    return pipeline


COMPLEX_SIGNAL = {
    "signal_type": "Complexity",
    "confidence": 0.95,
    "labels": ["complex", "multi-step"],
    "metadata": {"depth": 3},
}
CODE_SIGNAL = {
    "signal_type": "CodeContent",
    "confidence": 0.8,
    "labels": ["python"],
    "metadata": {},
}
SIMPLE_SIGNAL = {
    "signal_type": "Complexity",
    "confidence": 0.2,
    "labels": ["simple"],
    "metadata": {},
}


class TestCalculateStrongWinRate:
    def test_matches_first_rule(self):
        pipeline = _make_pipeline([COMPLEX_SIGNAL, CODE_SIGNAL])
        router = SignalRouter(
            pipeline,
            rules=[
                SignalRule(
                    "Complexity",
                    lambda r: "complex" in r.get("labels", []),
                    win_rate=0.9,
                ),
            ],
        )
        assert router.calculate_strong_win_rate("test") == 0.9

    def test_falls_through_to_default(self):
        pipeline = _make_pipeline([SIMPLE_SIGNAL])
        router = SignalRouter(
            pipeline,
            rules=[
                SignalRule(
                    "Complexity",
                    lambda r: "complex" in r.get("labels", []),
                    win_rate=0.9,
                ),
            ],
            default_win_rate=0.3,
        )
        assert router.calculate_strong_win_rate("test") == 0.3

    def test_returns_fallback_on_pipeline_error(self):
        pipeline = MagicMock()
        pipeline.evaluate.side_effect = RuntimeError("boom")
        router = SignalRouter(
            pipeline,
            rules=[],
            fallback_on_error=0.1,
        )
        assert router.calculate_strong_win_rate("test") == 0.1

    def test_rules_applied_in_priority_order(self):
        pipeline = _make_pipeline([COMPLEX_SIGNAL])
        low = SignalRule(
            "Complexity",
            lambda r: True,
            win_rate=0.3,
            priority=1,
        )
        high = SignalRule(
            "Complexity",
            lambda r: True,
            win_rate=0.9,
            priority=10,
        )
        # Pass low-priority first to verify sorting
        router = SignalRouter(pipeline, rules=[low, high])
        assert router.calculate_strong_win_rate("test") == 0.9

    def test_signal_not_present_is_skipped(self):
        pipeline = _make_pipeline([CODE_SIGNAL])
        router = SignalRouter(
            pipeline,
            rules=[
                SignalRule(
                    "NonExistent",
                    lambda r: True,
                    win_rate=0.9,
                ),
                SignalRule(
                    "CodeContent",
                    lambda r: True,
                    win_rate=0.7,
                ),
            ],
            default_win_rate=0.2,
        )
        # Should skip NonExistent, match CodeContent
        assert router.calculate_strong_win_rate("test") == 0.7


class TestRoute:
    def test_routes_strong_when_above_threshold(self):
        pipeline = _make_pipeline([COMPLEX_SIGNAL])
        router = SignalRouter(
            pipeline,
            rules=[
                SignalRule(
                    "Complexity",
                    lambda r: True,
                    win_rate=0.9,
                ),
            ],
        )
        pair = SimpleNamespace(strong="gpt-4", weak="gpt-3.5")
        assert router.route("test", 0.7, pair) == "gpt-4"

    def test_routes_weak_when_below_threshold(self):
        pipeline = _make_pipeline([SIMPLE_SIGNAL])
        router = SignalRouter(
            pipeline,
            rules=[
                SignalRule(
                    "Complexity",
                    lambda r: "complex" in r.get("labels", []),
                    win_rate=0.9,
                ),
            ],
            default_win_rate=0.3,
        )
        pair = SimpleNamespace(strong="gpt-4", weak="gpt-3.5")
        assert router.route("test", 0.7, pair) == "gpt-3.5"


class TestFromConfig:
    def test_match_labels_condition(self):
        pipeline = _make_pipeline([COMPLEX_SIGNAL])
        config = {
            "rules": [
                {
                    "signal_type": "Complexity",
                    "match_labels": ["complex"],
                    "win_rate": 0.9,
                    "priority": 10,
                }
            ]
        }
        router = SignalRouter.from_config(pipeline, config)
        assert router.calculate_strong_win_rate("test") == 0.9

    def test_min_confidence_condition(self):
        pipeline = _make_pipeline([CODE_SIGNAL])
        config = {
            "rules": [
                {
                    "signal_type": "CodeContent",
                    "min_confidence": 0.7,
                    "win_rate": 0.8,
                    "priority": 5,
                }
            ]
        }
        router = SignalRouter.from_config(pipeline, config)
        assert router.calculate_strong_win_rate("test") == 0.8

    def test_multiple_conditions_and_together(self):
        pipeline = _make_pipeline([COMPLEX_SIGNAL])
        config = {
            "rules": [
                {
                    "signal_type": "Complexity",
                    "match_labels": ["complex"],
                    "min_confidence": 0.9,
                    "win_rate": 0.95,
                }
            ]
        }
        router = SignalRouter.from_config(pipeline, config)
        # Both conditions met (labels include "complex", conf 0.95 >= 0.9)
        assert router.calculate_strong_win_rate("test") == 0.95

    def test_multiple_conditions_fail_when_one_misses(self):
        pipeline = _make_pipeline([SIMPLE_SIGNAL])
        config = {
            "default_win_rate": 0.2,
            "rules": [
                {
                    "signal_type": "Complexity",
                    "match_labels": ["complex"],
                    "min_confidence": 0.9,
                    "win_rate": 0.95,
                }
            ],
        }
        router = SignalRouter.from_config(pipeline, config)
        # Labels don't include "complex" -> fails
        assert router.calculate_strong_win_rate("test") == 0.2

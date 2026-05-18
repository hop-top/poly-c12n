"""Shared fixtures for c12n-py tests."""

from __future__ import annotations

import json
import sys
from pathlib import Path

import pytest

# Ensure the python package is importable without building the native
# extension.  Shared across ALL test files via conftest.
sys.path.insert(0, str(Path(__file__).resolve().parent.parent / "python"))


@pytest.fixture
def sample_result_dict():
    """Realistic pipeline result as a dict."""
    return {
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


@pytest.fixture
def sample_result_json(sample_result_dict):
    """Realistic pipeline result JSON string."""
    return json.dumps(sample_result_dict)


@pytest.fixture
def mock_pipeline(sample_result_json):
    """Mock pipeline that returns sample result."""

    class _Result:
        def __init__(self, raw):
            self._raw = raw

        def json(self):
            return self._raw

    class MockPipeline:
        def evaluate(self, text, **kwargs):
            return _Result(sample_result_json)

    return MockPipeline()

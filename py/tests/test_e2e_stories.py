"""End-to-end tests covering user story acceptance criteria.

Each test class maps to a persona + story from c12n user stories.
Tests exercise the full public API surface without native bindings.
"""

from __future__ import annotations

import asyncio
import json
from dataclasses import asdict, fields
from types import SimpleNamespace
from typing import Any
from unittest.mock import MagicMock

import pytest

import c12n
from c12n.config import Config, SignalsConfig, default_config, load_config
from c12n.middleware import (
    C12NMiddleware,
    get_signals,
    has_signal,
    signal_confidence,
)
from c12n.router import SignalRouter, SignalRule


# -------------------------------------------------------------------
# Helpers
# -------------------------------------------------------------------

SAMPLE_RESULTS = {
    "results": [
        {
            "name": "keyword",
            "signal_type": "Keyword",
            "confidence": 0.85,
            "labels": ["greeting"],
            "metadata": {},
        },
        {
            "name": "code",
            "signal_type": "CodeContent",
            "confidence": 0.9,
            "labels": ["python"],
            "metadata": {"has_code_fence": True},
        },
        {
            "name": "complexity",
            "signal_type": "Complexity",
            "confidence": 0.75,
            "labels": ["complex"],
            "metadata": {"hard_score": 0.8, "easy_score": 0.2},
        },
    ],
    "errors": [],
    "duration_ns": 2500000,
}


def _mock_pipeline(results: dict | None = None):
    """Build a mock pipeline returning canned JSON."""
    pipeline = MagicMock()
    data = results or SAMPLE_RESULTS
    result_obj = MagicMock()
    result_obj.json.return_value = json.dumps(data)
    pipeline.evaluate.return_value = result_obj
    return pipeline


def _make_receive(body: bytes):
    sent = False

    async def receive():
        nonlocal sent
        if not sent:
            sent = True
            return {
                "type": "http.request",
                "body": body,
                "more_body": False,
            }
        return {"type": "http.disconnect"}

    return receive


class _RecordingApp:
    def __init__(self):
        self.scope: dict[str, Any] | None = None

    async def __call__(self, scope, receive, send):
        self.scope = dict(scope)


# -------------------------------------------------------------------
# a) Solo-dev: pip install -> classify in < 5 lines
# -------------------------------------------------------------------


class TestSoloDevQuickStart:
    """AC: import c12n -> create Config -> classify in under 5 lines."""

    def test_default_config_valid(self):
        cfg = default_config()
        assert isinstance(cfg, Config)
        assert cfg.max_concurrency > 0
        assert cfg.timeout_ms > 0
        assert isinstance(cfg.signals, SignalsConfig)

    def test_config_has_signals_dict(self):
        cfg = default_config()
        enabled = cfg.enabled_signals()
        assert isinstance(enabled, list)
        assert len(enabled) > 0

    def test_five_line_classify(self):
        """The entire classify flow fits in 5 lines (excluding import)."""
        # line 1: config
        cfg = default_config()
        # line 2: pipeline kwargs
        kwargs = cfg.to_pipeline_kwargs()
        # line 3: mock pipeline (real pipeline = Pipeline(**kwargs))
        pipeline = _mock_pipeline()
        # line 4: router
        router = SignalRouter(pipeline, rules=[
            SignalRule("Complexity", lambda r: True, win_rate=0.8),
        ])
        # line 5: classify
        result = router.calculate_strong_win_rate("hello world")
        assert isinstance(result, float)
        assert 0.0 <= result <= 1.0


# -------------------------------------------------------------------
# b) Solo-dev: Python integration <= 5 lines
# -------------------------------------------------------------------


class TestSoloDevIntegration:
    """AC: import -> create router with rules -> route text."""

    def test_router_init_with_rules(self):
        pipeline = _mock_pipeline()
        router = SignalRouter(
            pipeline,
            rules=[
                SignalRule(
                    "CodeContent",
                    lambda r: r.get("confidence", 0) > 0.5,
                    win_rate=0.85,
                ),
            ],
        )
        assert len(router.rules) == 1

    def test_router_returns_routing_decision(self):
        pipeline = _mock_pipeline()
        router = SignalRouter(
            pipeline,
            rules=[
                SignalRule(
                    "CodeContent",
                    lambda r: r.get("confidence", 0) > 0.5,
                    win_rate=0.9,
                ),
            ],
        )
        pair = SimpleNamespace(strong="gpt-4o", weak="gpt-4o-mini")
        model = router.route("write python code", 0.7, pair)
        assert model in ("gpt-4o", "gpt-4o-mini")

    def test_from_config_shorthand(self):
        pipeline = _mock_pipeline()
        router = SignalRouter.from_config(pipeline, {
            "rules": [
                {
                    "signal_type": "Complexity",
                    "match_labels": ["complex"],
                    "win_rate": 0.9,
                },
            ],
        })
        assert router.calculate_strong_win_rate("test") == 0.9


# -------------------------------------------------------------------
# c) Agent: structured JSON conformance
# -------------------------------------------------------------------


class TestAgentJSONConformance:
    """AC: router returns typed, structured result."""

    def test_win_rate_is_float(self):
        pipeline = _mock_pipeline()
        router = SignalRouter(pipeline, rules=[])
        result = router.calculate_strong_win_rate("test")
        assert isinstance(result, float)

    def test_pipeline_result_has_expected_keys(self):
        raw = json.dumps(SAMPLE_RESULTS)
        parsed = json.loads(raw)
        assert "results" in parsed
        assert "errors" in parsed
        assert "duration_ns" in parsed
        assert isinstance(parsed["results"], list)

    def test_signal_result_fields_typed(self):
        for signal in SAMPLE_RESULTS["results"]:
            assert isinstance(signal["confidence"], (int, float))
            assert isinstance(signal["labels"], list)
            assert isinstance(signal["signal_type"], str)
            assert isinstance(signal.get("metadata", {}), dict)

    def test_route_returns_model_string(self):
        pipeline = _mock_pipeline()
        router = SignalRouter(pipeline, rules=[
            SignalRule("Keyword", lambda r: True, win_rate=0.9),
        ])
        pair = SimpleNamespace(strong="claude-4-opus", weak="claude-4-haiku")
        result = router.route("hello", 0.5, pair)
        assert isinstance(result, str)
        assert result == "claude-4-opus"


# -------------------------------------------------------------------
# d) Platform-eng: ASGI middleware integration
# -------------------------------------------------------------------


class TestPlatformEngMiddleware:
    """AC: middleware wraps ASGI app, injects classification into scope."""

    @pytest.mark.asyncio
    async def test_middleware_adds_signals_to_scope(self):
        inner = _RecordingApp()
        pipeline = _mock_pipeline()
        mw = C12NMiddleware(inner, pipeline)

        body = json.dumps({"prompt": "classify this"}).encode()
        scope: dict[str, Any] = {"type": "http"}

        await mw(scope, _make_receive(body), lambda _: None)

        assert "c12n.signals" in scope
        signals = scope["c12n.signals"]
        assert "results" in signals

    @pytest.mark.asyncio
    async def test_get_signals_from_scope(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        signals = get_signals(scope)
        assert signals is not None
        assert len(signals["results"]) == 3

    @pytest.mark.asyncio
    async def test_has_signal_returns_bool(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        assert has_signal(scope, "Keyword") is True
        assert has_signal(scope, "NonExistent") is False

    @pytest.mark.asyncio
    async def test_signal_confidence_returns_float(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        conf = signal_confidence(scope, "CodeContent")
        assert isinstance(conf, float)
        assert conf == pytest.approx(0.9)

    @pytest.mark.asyncio
    async def test_signal_confidence_missing_returns_zero(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        assert signal_confidence(scope, "Missing") == 0.0

    @pytest.mark.asyncio
    async def test_middleware_passes_body_downstream(self):
        """Downstream app receives the original request body."""
        pipeline = _mock_pipeline()
        captured_body = None

        async def app(scope, receive, send):
            msg = await receive()
            nonlocal captured_body
            captured_body = msg.get("body", b"")

        mw = C12NMiddleware(app, pipeline)
        payload = json.dumps({"text": "hello"}).encode()
        scope: dict[str, Any] = {"type": "http"}

        await mw(scope, _make_receive(payload), lambda _: None)
        assert captured_body == payload


# -------------------------------------------------------------------
# e) Researcher: config tuning
# -------------------------------------------------------------------


class TestResearcherConfigTuning:
    """AC: modify config thresholds, verify persistence + round-trip."""

    def test_modify_threshold_persists(self):
        cfg = default_config()
        original = cfg.signals.embedding.threshold
        cfg.signals.embedding.threshold = 0.95
        assert cfg.signals.embedding.threshold == 0.95
        assert cfg.signals.embedding.threshold != original

    def test_modify_concurrency(self):
        cfg = default_config()
        cfg.max_concurrency = 32
        assert cfg.max_concurrency == 32
        assert cfg.to_pipeline_kwargs()["max_concurrency"] == 32

    def test_config_dict_round_trip(self):
        """Config -> dict -> Config preserves values."""
        from c12n.config import _dict_to_config

        original = Config(max_concurrency=16, timeout_ms=3000)
        data = {
            "max_concurrency": original.max_concurrency,
            "timeout_ms": original.timeout_ms,
        }
        restored = _dict_to_config(data)
        assert restored.max_concurrency == original.max_concurrency
        assert restored.timeout_ms == original.timeout_ms

    def test_enable_disable_signal(self):
        cfg = default_config()
        assert "Keyword" in cfg.enabled_signals()
        cfg.signals.keyword.enabled = False
        assert "Keyword" not in cfg.enabled_signals()

    def test_load_config_nonexistent_raises(self):
        with pytest.raises(FileNotFoundError):
            load_config("/nonexistent/path/config.yaml")

    def test_yaml_round_trip(self, tmp_path):
        yaml = pytest.importorskip("yaml")

        cfg = Config(max_concurrency=24, timeout_ms=7000)
        data = {
            "max_concurrency": cfg.max_concurrency,
            "timeout_ms": cfg.timeout_ms,
        }
        config_file = tmp_path / "test_config.yaml"
        config_file.write_text(yaml.dump(data))

        loaded = load_config(str(config_file))
        assert loaded.max_concurrency == 24
        assert loaded.timeout_ms == 7000


# -------------------------------------------------------------------
# f) Agent: toolspec discovery (public API completeness)
# -------------------------------------------------------------------


class TestAgentToolspecDiscovery:
    """AC: all __all__ entries importable, no ImportError."""

    def test_all_exports_importable(self):
        for name in c12n.__all__:
            obj = getattr(c12n, name, None)
            assert obj is not None, f"{name} in __all__ but not importable"

    def test_expected_public_api(self):
        expected = {
            "Config",
            "default_config",
            "load_config",
            "C12NMiddleware",
            "get_signals",
            "has_signal",
            "signal_confidence",
            "SignalRouter",
            "SignalRule",
        }
        actual = set(c12n.__all__)
        missing = expected - actual
        assert not missing, f"Missing from __all__: {missing}"

    def test_no_import_error_on_any_name(self):
        for name in c12n.__all__:
            try:
                obj = getattr(c12n, name)
                assert obj is not None
            except ImportError as e:
                pytest.fail(f"ImportError for {name}: {e}")

    def test_config_is_dataclass(self):
        assert hasattr(Config, "__dataclass_fields__")

    def test_signal_rule_is_dataclass(self):
        assert hasattr(SignalRule, "__dataclass_fields__")


# -------------------------------------------------------------------
# g) Error handling
# -------------------------------------------------------------------


class TestErrorHandling:
    """AC: graceful behavior on edge cases."""

    def test_router_empty_rules_returns_default(self):
        pipeline = _mock_pipeline()
        router = SignalRouter(
            pipeline,
            rules=[],
            default_win_rate=0.42,
        )
        assert router.calculate_strong_win_rate("test") == 0.42

    def test_router_pipeline_error_returns_fallback(self):
        pipeline = MagicMock()
        pipeline.evaluate.side_effect = RuntimeError("boom")
        router = SignalRouter(
            pipeline,
            rules=[],
            fallback_on_error=0.15,
        )
        assert router.calculate_strong_win_rate("test") == 0.15

    def test_middleware_none_body_graceful(self):
        """Empty/missing body doesn't crash middleware."""
        inner = _RecordingApp()
        pipeline = _mock_pipeline()
        mw = C12NMiddleware(inner, pipeline)

        scope: dict[str, Any] = {"type": "http"}

        async def run():
            await mw(scope, _make_receive(b""), lambda _: None)

        asyncio.run(run())
        pipeline.evaluate.assert_not_called()
        assert inner.scope is not None

    @pytest.mark.asyncio
    async def test_middleware_invalid_json_graceful(self):
        inner = _RecordingApp()
        pipeline = _mock_pipeline()
        mw = C12NMiddleware(inner, pipeline)

        scope: dict[str, Any] = {"type": "http"}
        await mw(scope, _make_receive(b"not json"), lambda _: None)

        pipeline.evaluate.assert_not_called()
        assert "c12n.signals" not in scope
        assert inner.scope is not None

    @pytest.mark.asyncio
    async def test_middleware_pipeline_error_graceful(self):
        """Pipeline failure doesn't block the request."""
        inner = _RecordingApp()
        pipeline = MagicMock()
        pipeline.evaluate.side_effect = RuntimeError("crash")
        mw = C12NMiddleware(inner, pipeline)

        body = json.dumps({"text": "hello"}).encode()
        scope: dict[str, Any] = {"type": "http"}

        await mw(scope, _make_receive(body), lambda _: None)

        assert "c12n.signals" not in scope
        assert inner.scope is not None

    def test_has_signal_empty_scope(self):
        assert has_signal({}, "Keyword") is False

    def test_signal_confidence_empty_scope(self):
        assert signal_confidence({}, "Keyword") == 0.0

    def test_get_signals_missing_key(self):
        assert get_signals({}) is None

    def test_router_malformed_json_fallback(self):
        pipeline = MagicMock()
        pipeline.evaluate.return_value = "not valid json {{"
        router = SignalRouter(
            pipeline,
            rules=[],
            fallback_on_error=0.25,
        )
        assert router.calculate_strong_win_rate("test") == 0.25

    def test_from_config_empty_rules(self):
        pipeline = _mock_pipeline()
        router = SignalRouter.from_config(pipeline, {"rules": []})
        result = router.calculate_strong_win_rate("test")
        assert result == 0.5  # default_win_rate

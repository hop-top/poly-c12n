"""Integration tests for c12n-py Python API surface.

Tests full flows across config, middleware, and router modules
using mocks (no PyO3 build required).
"""

from __future__ import annotations

import asyncio
import json
import tempfile
from pathlib import Path

import pytest

from c12n.config import Config, default_config, load_config
from c12n.middleware import (
    C12NMiddleware,
    get_signals,
    has_signal,
    signal_confidence,
)
from c12n.router import SignalRouter, SignalRule


# -------------------------------------------------------------------
# 1. Config -> Pipeline flow
# -------------------------------------------------------------------


class TestConfigToPipeline:
    """Verify default_config -> to_pipeline_kwargs produces correct kwargs."""

    def test_default_kwargs(self):
        cfg = default_config()
        kwargs = cfg.to_pipeline_kwargs()
        assert kwargs == {
            "max_concurrency": 8,
            "timeout_ms": 5000,
        }

    def test_custom_kwargs(self):
        cfg = Config(max_concurrency=16, timeout_ms=10000)
        kwargs = cfg.to_pipeline_kwargs()
        assert kwargs["max_concurrency"] == 16
        assert kwargs["timeout_ms"] == 10000

    def test_enabled_signals_default(self):
        cfg = default_config()
        enabled = cfg.enabled_signals()
        # Default: keyword, jailbreak, pii, context, format, code,
        # toolcall, cost are enabled
        assert "Keyword" in enabled
        assert "Jailbreak" in enabled
        assert "PII" in enabled
        assert "Context" in enabled
        assert "OutputFormat" in enabled
        assert "CodeContent" in enabled
        assert "ToolCalling" in enabled
        assert "CostEstimate" in enabled
        # Default disabled
        assert "Embedding" not in enabled
        assert "Domain" not in enabled
        assert "Toxicity" not in enabled
        assert "Language" not in enabled
        assert "Complexity" not in enabled


# -------------------------------------------------------------------
# 2. Config -> Router flow
# -------------------------------------------------------------------


class TestConfigToRouter:
    """Create Config, build SignalRouter.from_config, verify routing."""

    def test_from_config_with_rules(self, mock_pipeline):
        router_cfg = {
            "default_win_rate": 0.4,
            "fallback_on_error": 0.3,
            "rules": [
                {
                    "signal_type": "Complexity",
                    "match_labels": ["complex"],
                    "win_rate": 0.9,
                    "priority": 10,
                },
                {
                    "signal_type": "CodeContent",
                    "min_confidence": 0.7,
                    "win_rate": 0.8,
                    "priority": 5,
                },
            ],
        }
        router = SignalRouter.from_config(mock_pipeline, router_cfg)
        assert router.default_win_rate == 0.4
        assert router.fallback_on_error == 0.3
        assert len(router.rules) == 2
        # Rules sorted by priority descending
        assert router.rules[0].priority == 10
        assert router.rules[1].priority == 5

    def test_routing_decision(self, mock_pipeline):
        """Complexity=complex signal present -> high win_rate -> strong."""
        router_cfg = {
            "rules": [
                {
                    "signal_type": "Complexity",
                    "match_labels": ["complex"],
                    "win_rate": 0.9,
                    "priority": 10,
                },
            ],
        }
        router = SignalRouter.from_config(mock_pipeline, router_cfg)
        win_rate = router.calculate_strong_win_rate("test prompt")
        assert win_rate == 0.9

    def test_no_match_returns_default(self, mock_pipeline):
        """Rules that don't match return default_win_rate."""
        router_cfg = {
            "default_win_rate": 0.3,
            "rules": [
                {
                    "signal_type": "Complexity",
                    "match_labels": ["simple"],  # won't match
                    "win_rate": 0.1,
                },
            ],
        }
        router = SignalRouter.from_config(mock_pipeline, router_cfg)
        win_rate = router.calculate_strong_win_rate("test prompt")
        assert win_rate == 0.3


# -------------------------------------------------------------------
# 3. Middleware -> Router flow
# -------------------------------------------------------------------


class TestMiddlewareToRouter:
    """Middleware stores signals in scope, router reads from result."""

    def test_middleware_populates_scope(self, mock_pipeline):
        """Middleware processes request body, stores signals in scope."""
        scope = {"type": "http"}
        body = json.dumps(
            {"messages": [{"role": "user", "content": "hello world"}]}
        ).encode()

        captured_scope = {}

        async def downstream(scope, receive, send):
            captured_scope.update(scope)

        mw = C12NMiddleware(downstream, mock_pipeline)

        body_sent = False

        async def receive():
            nonlocal body_sent
            if not body_sent:
                body_sent = True
                return {
                    "type": "http.request",
                    "body": body,
                    "more_body": False,
                }
            return {"type": "http.disconnect"}

        asyncio.run(mw(scope, receive, lambda _: None))

        signals = get_signals(captured_scope)
        assert signals is not None
        assert "results" in signals
        assert len(signals["results"]) == 5

    def test_signals_to_router(
        self, mock_pipeline, sample_result_dict
    ):
        """Signals from middleware result feed into router rules."""
        # Simulate scope populated by middleware
        scope = {"c12n.signals": sample_result_dict}

        assert has_signal(scope, "Keyword")
        assert has_signal(scope, "CodeContent")
        assert signal_confidence(scope, "Complexity") == 0.8
        assert not has_signal(scope, "NonExistent")

        # Now use same pipeline for router
        router = SignalRouter(
            mock_pipeline,
            rules=[
                SignalRule(
                    "CodeContent",
                    lambda r: r.get("confidence", 0) > 0.7,
                    win_rate=0.85,
                    priority=5,
                ),
            ],
        )
        win_rate = router.calculate_strong_win_rate("write me code")
        assert win_rate == 0.85


# -------------------------------------------------------------------
# 4. Config YAML round-trip
# -------------------------------------------------------------------


class TestConfigYAMLRoundTrip:
    """Write Config to YAML temp file, load back, verify equality."""

    def test_round_trip(self):
        yaml = pytest.importorskip("yaml")

        original = default_config()
        data = {
            "max_concurrency": original.max_concurrency,
            "timeout_ms": original.timeout_ms,
            "signals": {
                "keyword": {
                    "enabled": original.signals.keyword.enabled,
                    "rules": [],
                },
                "embedding": {
                    "enabled": original.signals.embedding.enabled,
                    "threshold": original.signals.embedding.threshold,
                },
                "domain": {
                    "enabled": original.signals.domain.enabled,
                },
                "safety": {
                    "jailbreak": {
                        "enabled": (
                            original.signals.safety.jailbreak.enabled
                        ),
                    },
                    "pii": {
                        "enabled": (
                            original.signals.safety.pii.enabled
                        ),
                        "deny_list": (
                            original.signals.safety.pii.deny_list
                        ),
                    },
                    "toxicity": {
                        "enabled": (
                            original.signals.safety.toxicity.enabled
                        ),
                        "threshold": (
                            original.signals.safety.toxicity.threshold
                        ),
                    },
                },
                "context": {
                    "enabled": original.signals.context.enabled,
                    "output_ratio": (
                        original.signals.context.output_ratio
                    ),
                },
                "language": {
                    "enabled": original.signals.language.enabled,
                },
                "complexity": {
                    "enabled": original.signals.complexity.enabled,
                    "margin": original.signals.complexity.margin,
                },
                "format_enabled": original.signals.format_enabled,
                "code_enabled": original.signals.code_enabled,
                "toolcall_enabled": original.signals.toolcall_enabled,
                "cost_enabled": original.signals.cost_enabled,
            },
        }

        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".yaml", delete=False
        ) as f:
            yaml.dump(data, f)
            tmp_path = f.name

        try:
            loaded = load_config(tmp_path)
            assert loaded.max_concurrency == original.max_concurrency
            assert loaded.timeout_ms == original.timeout_ms
            assert (
                loaded.enabled_signals()
                == original.enabled_signals()
            )
            assert (
                loaded.signals.embedding.threshold
                == original.signals.embedding.threshold
            )
            assert (
                loaded.signals.safety.pii.deny_list
                == original.signals.safety.pii.deny_list
            )
        finally:
            Path(tmp_path).unlink(missing_ok=True)

    def test_partial_yaml(self):
        """Load YAML with only some fields; rest default."""
        yaml = pytest.importorskip("yaml")

        data = {"max_concurrency": 32}

        with tempfile.NamedTemporaryFile(
            mode="w", suffix=".yaml", delete=False
        ) as f:
            yaml.dump(data, f)
            tmp_path = f.name

        try:
            loaded = load_config(tmp_path)
            assert loaded.max_concurrency == 32
            assert loaded.timeout_ms == 5000  # default
        finally:
            Path(tmp_path).unlink(missing_ok=True)


# -------------------------------------------------------------------
# 5. Full stack mock
# -------------------------------------------------------------------


class TestFullStackMock:
    """Mock pipeline -> middleware -> signal extraction -> router."""

    def test_end_to_end(self, mock_pipeline, sample_result_dict):
        """Full flow: config -> middleware -> signals -> router."""
        # Step 1: Config produces correct pipeline kwargs
        cfg = Config(max_concurrency=4, timeout_ms=2000)
        kwargs = cfg.to_pipeline_kwargs()
        assert kwargs["max_concurrency"] == 4

        # Step 2: Middleware processes request
        scope = {"type": "http"}
        body = json.dumps({"text": "generate python code"}).encode()

        captured_scope = {}

        async def app(scope, receive, send):
            captured_scope.update(scope)

        mw = C12NMiddleware(app, mock_pipeline)

        body_sent = False

        async def receive():
            nonlocal body_sent
            if not body_sent:
                body_sent = True
                return {
                    "type": "http.request",
                    "body": body,
                    "more_body": False,
                }
            return {"type": "http.disconnect"}

        asyncio.run(mw(scope, receive, lambda _: None))

        # Step 3: Extract signals
        signals = get_signals(captured_scope)
        assert signals is not None
        assert has_signal(captured_scope, "CodeContent")
        assert signal_confidence(captured_scope, "CodeContent") == 0.9

        # Step 4: Router decides based on signals
        router = SignalRouter.from_config(
            mock_pipeline,
            {
                "default_win_rate": 0.5,
                "rules": [
                    {
                        "signal_type": "CodeContent",
                        "min_confidence": 0.8,
                        "win_rate": 0.95,
                        "priority": 10,
                    },
                    {
                        "signal_type": "Complexity",
                        "match_labels": ["complex"],
                        "win_rate": 0.9,
                        "priority": 5,
                    },
                ],
            },
        )
        # CodeContent has higher priority and matches
        win_rate = router.calculate_strong_win_rate("test")
        assert win_rate == 0.95

    def test_error_fallback_chain(self):
        """Pipeline error -> middleware graceful, router fallback."""

        class FailingPipeline:
            def evaluate(self, text, **kwargs):
                raise RuntimeError("pipeline down")

        pipeline = FailingPipeline()

        # Middleware: doesn't crash, no signals stored
        scope = {"type": "http"}
        body = json.dumps({"text": "hello"}).encode()

        captured_scope = {}

        async def app(scope, receive, send):
            captured_scope.update(scope)

        mw = C12NMiddleware(app, pipeline)

        body_sent = False

        async def receive():
            nonlocal body_sent
            if not body_sent:
                body_sent = True
                return {
                    "type": "http.request",
                    "body": body,
                    "more_body": False,
                }
            return {"type": "http.disconnect"}

        asyncio.run(mw(scope, receive, lambda _: None))
        assert get_signals(captured_scope) is None

        # Router: returns fallback_on_error
        router = SignalRouter(
            pipeline,
            rules=[],
            fallback_on_error=0.6,
        )
        assert router.calculate_strong_win_rate("test") == 0.6

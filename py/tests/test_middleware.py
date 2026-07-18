"""Tests for c12n ASGI middleware."""

from __future__ import annotations

import asyncio
import json
from typing import Any
from unittest.mock import MagicMock

import pytest

from c12n.middleware import (
    C12NMiddleware,
    get_signals,
    has_signal,
    signal_confidence,
)

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

SAMPLE_RESULTS = {
    "results": [
        {"signal_type": "toxicity", "confidence": 0.92},
        {"signal_type": "pii", "confidence": 0.45},
    ],
    "errors": [],
    "duration_ms": 12,
}


def _make_pipeline(return_json: str | None = None, raise_exc: bool = False):
    """Return a mock pipeline whose evaluate() returns a mock result."""
    pipeline = MagicMock()
    if raise_exc:
        pipeline.evaluate.side_effect = RuntimeError("boom")
    else:
        result_obj = MagicMock()
        result_obj.json.return_value = return_json or json.dumps(SAMPLE_RESULTS)
        pipeline.evaluate.return_value = result_obj
    return pipeline


def _make_receive(body: bytes):
    """Return an ASGI receive callable that yields *body* once."""
    sent = False

    async def receive():
        nonlocal sent
        if not sent:
            sent = True
            return {"type": "http.request", "body": body, "more_body": False}
        # After body is consumed, return disconnect (shouldn't be reached).
        return {"type": "http.disconnect"}

    return receive


async def _noop_send(msg: dict):
    """No-op ASGI send."""


class _RecordingApp:
    """Tiny ASGI app that records the scope it receives."""

    def __init__(self):
        self.scope: dict[str, Any] | None = None
        self.body: bytes | None = None

    async def __call__(self, scope, receive, send):
        self.scope = dict(scope)
        msg = await receive()
        self.body = msg.get("body", b"")


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.asyncio
async def test_passthrough_non_http():
    """Non-HTTP scopes (e.g. websocket) are forwarded untouched."""
    inner = _RecordingApp()
    mw = C12NMiddleware(inner, _make_pipeline())
    scope = {"type": "websocket"}

    await mw(scope, _make_receive(b""), _noop_send)

    assert inner.scope["type"] == "websocket"


@pytest.mark.asyncio
async def test_extract_openai_chat_format():
    """Extracts text from OpenAI chat completion request body."""
    inner = _RecordingApp()
    pipeline = _make_pipeline()
    mw = C12NMiddleware(inner, pipeline)

    body = json.dumps(
        {"messages": [{"role": "user", "content": "hello world"}]}
    ).encode()
    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(body), _noop_send)

    pipeline.evaluate.assert_called_once_with("hello world")
    assert "c12n.signals" in scope


@pytest.mark.asyncio
async def test_extract_prompt_format():
    """Extracts text from simple prompt field."""
    inner = _RecordingApp()
    pipeline = _make_pipeline()
    mw = C12NMiddleware(inner, pipeline)

    body = json.dumps({"prompt": "summarise this"}).encode()
    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(body), _noop_send)

    pipeline.evaluate.assert_called_once_with("summarise this")


@pytest.mark.asyncio
async def test_extract_text_field():
    """Extracts text from generic text field."""
    inner = _RecordingApp()
    pipeline = _make_pipeline()
    mw = C12NMiddleware(inner, pipeline)

    body = json.dumps({"text": "classify me"}).encode()
    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(body), _noop_send)

    pipeline.evaluate.assert_called_once_with("classify me")


@pytest.mark.asyncio
async def test_results_stored_in_scope():
    """Pipeline results are stored in scope under the configured key."""
    inner = _RecordingApp()
    pipeline = _make_pipeline()
    mw = C12NMiddleware(inner, pipeline)

    body = json.dumps({"prompt": "test"}).encode()
    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(body), _noop_send)

    signals = scope["c12n.signals"]
    assert signals["results"][0]["signal_type"] == "toxicity"
    assert signals["duration_ms"] == 12


@pytest.mark.asyncio
async def test_json_decode_error_graceful():
    """Invalid JSON body does not crash the middleware."""
    inner = _RecordingApp()
    pipeline = _make_pipeline()
    mw = C12NMiddleware(inner, pipeline)

    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(b"not json"), _noop_send)

    pipeline.evaluate.assert_not_called()
    assert "c12n.signals" not in scope
    # Inner app still called
    assert inner.scope is not None


@pytest.mark.asyncio
async def test_pipeline_error_graceful():
    """Pipeline exception does not block the request."""
    inner = _RecordingApp()
    pipeline = _make_pipeline(raise_exc=True)
    mw = C12NMiddleware(inner, pipeline)

    body = json.dumps({"prompt": "boom"}).encode()
    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(body), _noop_send)

    assert "c12n.signals" not in scope
    assert inner.scope is not None


@pytest.mark.asyncio
async def test_body_replayed_to_downstream():
    """Downstream app receives the original request body."""
    inner = _RecordingApp()
    pipeline = _make_pipeline()
    mw = C12NMiddleware(inner, pipeline)

    payload = json.dumps({"prompt": "hi"}).encode()
    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(payload), _noop_send)

    assert inner.body == payload


@pytest.mark.asyncio
async def test_custom_text_extractor():
    """A user-provided text_extractor is used instead of the default."""
    inner = _RecordingApp()
    pipeline = _make_pipeline()

    def custom_extractor(body: dict) -> str | None:
        return body.get("input", {}).get("query")

    mw = C12NMiddleware(inner, pipeline, text_extractor=custom_extractor)

    body = json.dumps({"input": {"query": "custom text"}}).encode()
    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(body), _noop_send)

    pipeline.evaluate.assert_called_once_with("custom text")


@pytest.mark.asyncio
async def test_custom_result_key():
    """Results stored under a custom key when configured."""
    inner = _RecordingApp()
    pipeline = _make_pipeline()
    mw = C12NMiddleware(inner, pipeline, result_key="my.signals")

    body = json.dumps({"prompt": "test"}).encode()
    scope: dict[str, Any] = {"type": "http"}

    await mw(scope, _make_receive(body), _noop_send)

    assert "my.signals" in scope
    assert "c12n.signals" not in scope


# ---------------------------------------------------------------------------
# Helper function tests
# ---------------------------------------------------------------------------


class TestGetSignals:
    def test_returns_signals(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        assert get_signals(scope) == SAMPLE_RESULTS

    def test_returns_none_when_missing(self):
        assert get_signals({}) is None

    def test_custom_key(self):
        scope = {"custom": SAMPLE_RESULTS}
        assert get_signals(scope, key="custom") == SAMPLE_RESULTS


class TestHasSignal:
    def test_found(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        assert has_signal(scope, "toxicity") is True

    def test_not_found(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        assert has_signal(scope, "spam") is False

    def test_no_signals(self):
        assert has_signal({}, "toxicity") is False

    def test_no_results_key(self):
        scope = {"c12n.signals": {"errors": []}}
        assert has_signal(scope, "toxicity") is False


class TestSignalConfidence:
    def test_returns_confidence(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        assert signal_confidence(scope, "toxicity") == pytest.approx(0.92)

    def test_returns_zero_when_missing(self):
        scope = {"c12n.signals": SAMPLE_RESULTS}
        assert signal_confidence(scope, "spam") == 0.0

    def test_returns_zero_no_signals(self):
        assert signal_confidence({}, "toxicity") == 0.0

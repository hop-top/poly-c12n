"""ASGI middleware that classifies requests through c12n pipeline."""

from __future__ import annotations

import json
from typing import Any, Callable, Optional


class C12NMiddleware:
    """ASGI middleware that runs c12n classification on incoming requests.

    Intercepts requests, extracts text from the JSON body, evaluates
    through the pipeline, and stores results in request scope for
    downstream routers/handlers to read.

    Usage::

        from c12n.middleware import C12NMiddleware

        pipeline = Pipeline(max_concurrency=8, timeout_ms=5000)
        app = C12NMiddleware(app, pipeline)
    """

    def __init__(
        self,
        app: Any,
        pipeline: Any,  # PyPipeline instance
        text_extractor: Optional[Callable] = None,
        result_key: str = "c12n.signals",
    ):
        """
        Args:
            app: The ASGI application to wrap.
            pipeline: c12n.Pipeline instance.
            text_extractor: Optional callable(body_dict) -> str to extract
                text from request body. Default: body["messages"][-1]["content"]
                or body["prompt"] or body["text"].
            result_key: Key in request scope to store results.
        """
        self.app = app
        self.pipeline = pipeline
        self.text_extractor = text_extractor or self._default_extractor
        self.result_key = result_key

    async def __call__(self, scope, receive, send):
        if scope["type"] != "http":
            await self.app(scope, receive, send)
            return

        # Collect request body
        body = await self._read_body(receive)

        # Try to extract text and classify
        text = None
        result_json = None
        try:
            body_dict = json.loads(body) if body else {}
            text = self.text_extractor(body_dict)
        except (json.JSONDecodeError, KeyError, TypeError, IndexError):
            pass

        if text:
            try:
                result = self.pipeline.evaluate(text)
                result_json = result.json() if hasattr(result, "json") else result
            except Exception:
                pass  # Don't block request on classification failure

        # Store results in scope for downstream
        if result_json:
            try:
                scope[self.result_key] = json.loads(result_json)
            except (json.JSONDecodeError, TypeError):
                pass  # Fail open — don't block request on bad JSON

        # Replay body for downstream
        body_sent = False

        async def receive_wrapper():
            nonlocal body_sent
            if not body_sent:
                body_sent = True
                return {"type": "http.request", "body": body, "more_body": False}
            return await receive()

        await self.app(scope, receive_wrapper, send)

    @staticmethod
    def _default_extractor(body: dict) -> Optional[str]:
        """Extract text from common LLM API request formats."""
        # OpenAI chat format
        if "messages" in body and body["messages"]:
            last = body["messages"][-1]
            if isinstance(last, dict) and "content" in last:
                return last["content"]
        # Simple prompt format
        if "prompt" in body:
            return body["prompt"]
        # Generic text field
        if "text" in body:
            return body["text"]
        return None

    @staticmethod
    async def _read_body(receive) -> bytes:
        """Read full ASGI request body."""
        chunks: list[bytes] = []
        while True:
            msg = await receive()
            chunk = msg.get("body", b"")
            if chunk:
                chunks.append(chunk)
            if not msg.get("more_body", False):
                break
        return b"".join(chunks)


def get_signals(scope: dict, key: str = "c12n.signals") -> Optional[dict]:
    """Read c12n signal results from ASGI request scope."""
    return scope.get(key)


def has_signal(
    scope: dict, signal_type: str, key: str = "c12n.signals"
) -> bool:
    """Check if a specific signal type is present in results."""
    signals = get_signals(scope, key)
    if not signals or "results" not in signals:
        return False
    return any(r.get("signal_type") == signal_type for r in signals["results"])


def signal_confidence(
    scope: dict, signal_type: str, key: str = "c12n.signals"
) -> float:
    """Get confidence for a signal type, or 0.0 if not found."""
    signals = get_signals(scope, key)
    if not signals or "results" not in signals:
        return 0.0
    for r in signals["results"]:
        if r.get("signal_type") == signal_type:
            return r.get("confidence", 0.0)
    return 0.0

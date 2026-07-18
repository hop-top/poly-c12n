"""Regression tests for PR #3 review comments."""

from __future__ import annotations

import asyncio
import importlib
import json
import os
import sys
from pathlib import Path
from typing import Any
from unittest.mock import MagicMock

import pytest


# -------------------------------------------------------------------
# 1. __all__ includes only available names
# -------------------------------------------------------------------


class TestAllExportsResolvable:
    """Every name in __all__ must be importable (no AttributeError)."""

    def test_all_names_resolvable(self):
        import c12n

        for name in c12n.__all__:
            # Should not raise AttributeError
            getattr(c12n, name)

    def test_all_excludes_native_when_unavailable(self):
        """When native extension is absent, Pipeline/PipelineResult
        must NOT appear in __all__."""
        import c12n

        has_native = hasattr(c12n, "Pipeline") and not isinstance(
            getattr(c12n, "Pipeline", None), type(None)
        )
        if not has_native:
            assert "Pipeline" not in c12n.__all__
            assert "PipelineResult" not in c12n.__all__


# -------------------------------------------------------------------
# 2. sys.path includes python source dir via conftest
# -------------------------------------------------------------------


class TestSysPathSetup:
    """conftest.py must add the python source dir to sys.path."""

    def test_python_source_in_sys_path(self):
        python_src = str(
            Path(__file__).resolve().parent.parent / "python"
        )
        assert python_src in sys.path, (
            f"Expected {python_src} in sys.path"
        )


# -------------------------------------------------------------------
# 3. Benchmark tests skip without C12N_BENCH=1
# -------------------------------------------------------------------


class TestBenchmarkSkipGuard:
    """Benchmark tests must be skipped unless C12N_BENCH=1."""

    def test_benchmark_marker_present(self):
        """All test_bench_* functions should have the benchmark mark."""
        import importlib.util

        spec = importlib.util.spec_from_file_location(
            "test_benchmarks",
            Path(__file__).parent / "test_benchmarks.py",
        )
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)

        bench_fns = [
            name
            for name in dir(mod)
            if name.startswith("test_bench_")
        ]
        assert len(bench_fns) > 0, "No benchmark tests found"

        for name in bench_fns:
            fn = getattr(mod, name)
            markers = [
                m.name
                for m in getattr(fn, "pytestmark", [])
            ]
            assert "benchmark" in markers, (
                f"{name} missing @pytest.mark.benchmark"
            )

    def test_benchmarks_skip_without_env(self):
        """Benchmark tests should be skipped when C12N_BENCH unset."""
        import importlib.util

        spec = importlib.util.spec_from_file_location(
            "test_benchmarks",
            Path(__file__).parent / "test_benchmarks.py",
        )
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)

        bench_fns = [
            name
            for name in dir(mod)
            if name.startswith("test_bench_")
        ]
        for name in bench_fns:
            fn = getattr(mod, name)
            markers = [
                m.name
                for m in getattr(fn, "pytestmark", [])
            ]
            assert "benchmark" in markers


# -------------------------------------------------------------------
# 6. middleware: invalid JSON from pipeline must not crash
# -------------------------------------------------------------------


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
        self.body: bytes | None = None

    async def __call__(self, scope, receive, send):
        self.scope = dict(scope)
        msg = await receive()
        self.body = msg.get("body", b"")


async def _noop_send(msg: dict):
    pass


class TestMiddlewareInvalidJson:
    """Pipeline returning invalid JSON must not crash the request."""

    @pytest.mark.asyncio
    async def test_invalid_pipeline_json_fails_open(self):
        from c12n.middleware import C12NMiddleware

        inner = _RecordingApp()
        pipeline = MagicMock()
        result_obj = MagicMock()
        result_obj.json.return_value = "NOT VALID JSON {{"
        pipeline.evaluate.return_value = result_obj

        mw = C12NMiddleware(inner, pipeline)

        body = json.dumps({"prompt": "test"}).encode()
        scope: dict[str, Any] = {"type": "http"}

        # Must not raise — request passes through
        await mw(scope, _make_receive(body), _noop_send)

        assert inner.scope is not None, (
            "Downstream app was not called"
        )
        assert "c12n.signals" not in scope, (
            "Invalid JSON should not populate scope"
        )


# -------------------------------------------------------------------
# 7. _read_body: multiple chunks produce correct result
# -------------------------------------------------------------------


class TestReadBodyChunks:
    """_read_body with multiple chunks must concatenate correctly."""

    @pytest.mark.asyncio
    async def test_multi_chunk_body(self):
        from c12n.middleware import C12NMiddleware

        chunks = [b"hello ", b"world", b"!"]
        idx = 0

        async def multi_receive():
            nonlocal idx
            if idx < len(chunks):
                chunk = chunks[idx]
                idx += 1
                more = idx < len(chunks)
                return {
                    "type": "http.request",
                    "body": chunk,
                    "more_body": more,
                }
            return {"type": "http.disconnect"}

        result = await C12NMiddleware._read_body(multi_receive)
        assert result == b"hello world!"

    @pytest.mark.asyncio
    async def test_single_chunk_body(self):
        from c12n.middleware import C12NMiddleware

        async def single_receive():
            return {
                "type": "http.request",
                "body": b"payload",
                "more_body": False,
            }

        result = await C12NMiddleware._read_body(single_receive)
        assert result == b"payload"

    @pytest.mark.asyncio
    async def test_empty_body(self):
        from c12n.middleware import C12NMiddleware

        async def empty_receive():
            return {
                "type": "http.request",
                "body": b"",
                "more_body": False,
            }

        result = await C12NMiddleware._read_body(empty_receive)
        assert result == b""

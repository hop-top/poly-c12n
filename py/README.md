# hop-top-c12n

LLM request classification engine — Python bindings (PyO3 native core +
pure-Python helpers) over the Rust `c12n-core` engine.

> [!NOTE]
> **Read-only mirror.** This repo (`hop-top/c12n-py`) is subtree-pushed
> from the canonical monorepo. File issues and PRs at
> [`hop-top/poly-c12n`](https://github.com/hop-top/poly-c12n).

> [!WARNING]
> **Alpha — API and tag history may break.** Pin to the latest alpha
> release rather than a range.

## Install

```bash
pip install hop-top-c12n
```

The import module is `c12n`:

```python
import c12n
```

## What you get

The package is a hybrid distribution:

- **PyO3 native extension** — `Pipeline` and `PipelineResult` are a thin
  wrapper around `c12n_core::Pipeline`, built via
  [maturin](https://www.maturin.rs/). They are only importable when the
  compiled extension is present (installing the wheel from PyPI provides
  it).
- **Pure-Python helpers** — `Config` / `load_config`, `SignalRouter` /
  `SignalRule`, and `C12NMiddleware` are always importable, native
  extension or not. If the extension is missing, everything except
  `Pipeline`/`PipelineResult` still works.

## Quickstart

```python
import c12n

# Native pipeline (requires the compiled extension from the wheel).
pipeline = c12n.Pipeline(max_concurrency=8, timeout_ms=5000)

result = pipeline.evaluate(
    "Write a Python function to sort a list.",
    history=[],
    headers={"x-trace": "abc"},
)

# PipelineResult wraps a JSON envelope: {"results": [...],
# "errors": [...], "duration_ms": <int>}.
import json
payload = json.loads(result.json())
for signal in payload["results"]:
    print(signal)
```

### Config from YAML

```python
from c12n import load_config, default_config

cfg = load_config("c12n.yaml")   # or default_config()
pipeline = c12n.Pipeline(**cfg.to_pipeline_kwargs())
print(cfg.enabled_signals())
```

### ASGI middleware

```python
from c12n import C12NMiddleware, get_signals

pipeline = c12n.Pipeline(max_concurrency=8, timeout_ms=5000)
app = C12NMiddleware(app, pipeline)   # wrap any ASGI app

# Downstream handlers read results from request scope:
#   signals = get_signals(scope)
```

### routellm signal router

```python
from c12n import SignalRouter, SignalRule

router = SignalRouter(pipeline, rules=[
    SignalRule(
        "Complexity",
        lambda r: "complex" in r.get("labels", []),
        win_rate=0.9,
    ),
    SignalRule(
        "CodeContent",
        lambda r: r.get("confidence", 0) > 0.7,
        win_rate=0.8,
    ),
])
win_rate = router.calculate_strong_win_rate("Refactor this module.")
```

## Requirements

- **Python 3.9+** (`requires-python = ">=3.9"`).
- Optional: `PyYAML` for `load_config` on `.yaml` files; `pkl-python`
  for `.pkl` config files.

## License

MIT.

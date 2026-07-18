"""c12n -- Classification engine Python bindings."""

_HAS_NATIVE = False
try:
    from .c12n import PyPipeline as Pipeline
    from .c12n import PyPipelineResult as PipelineResult

    _HAS_NATIVE = True
except ImportError:
    pass

from .config import Config, default_config, load_config
from .middleware import C12NMiddleware, get_signals, has_signal, signal_confidence
from .router import SignalRouter, SignalRule

__all__ = [
    "C12NMiddleware",
    "get_signals",
    "has_signal",
    "signal_confidence",
    "Config",
    "default_config",
    "load_config",
    "SignalRouter",
    "SignalRule",
]

if _HAS_NATIVE:
    __all__ += ["Pipeline", "PipelineResult"]

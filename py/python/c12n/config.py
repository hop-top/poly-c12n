"""Configuration loader for c12n pipeline.

Accepts two YAML/PKL document shapes:

* **Flat** -- the cross-language schema of record defined by
  ``go/config.pkl`` and consumed by the Go bindings, e.g.
  ``safety_pii_enabled: false``. Use this for configs shared with
  other language bindings.
* **Nested** -- a Python-side convenience shape that groups related
  keys, e.g. ``signals.safety.pii.enabled: false``.

Both may appear in one document; a flat key and its nested equivalent
address the same setting, and the nested form wins if both are
present. Any key that matches neither shape raises
:class:`ConfigError` -- unrecognised keys are never silently
discarded, because dropping a disabled safety signal on the floor
turns a typo into a security failure.

Optional PKL evaluation when pkl-python is installed.
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Any, Dict, List, Optional


class ConfigError(ValueError):
    """Raised when a config document contains unusable content.

    Most often an unrecognised key: rather than dropping it, the
    loader refuses the whole document so a mistyped or misplaced
    setting can never be mistaken for an applied one.
    """


class SignalType(str, Enum):
    """All classification signal types.

    Mirrors ``c12n_core::SignalType`` in ``core/src/types.rs``, Go's
    ``SignalType`` constants, and TypeScript's ``SignalType`` union.
    New variants added to the core enum must be added here too.

    Values are the exact PascalCase strings emitted on the wire by the
    core engine; lowercase spellings are not accepted anywhere.
    """

    KEYWORD = "Keyword"
    EMBEDDING = "Embedding"
    DOMAIN = "Domain"
    JAILBREAK = "Jailbreak"
    PII = "PII"
    TOXICITY = "Toxicity"
    CONTEXT = "Context"
    STRUCTURE = "Structure"
    LANGUAGE = "Language"
    COMPLEXITY = "Complexity"
    PREFERENCE = "Preference"
    FEEDBACK = "Feedback"
    OUTPUT_FORMAT = "OutputFormat"
    CODE_CONTENT = "CodeContent"
    TOOL_CALLING = "ToolCalling"
    COST_ESTIMATE = "CostEstimate"
    SENTIMENT = "Sentiment"
    INTENT = "Intent"
    TOPIC = "Topic"
    CUSTOM = "Custom"

    def __str__(self) -> str:  # pragma: no cover - trivial
        return self.value


@dataclass
class KeywordRuleConfig:
    label: str
    patterns: List[str]
    operator: str = "OR"      # AND, OR, NOR
    strategy: str = "regex"   # regex, bm25, trigram, fuzzy
    threshold: float = 0.5


@dataclass
class KeywordConfig:
    enabled: bool = True
    # Defaults applied to rules that do not override them. Mirror the
    # flat keyword_strategy / keyword_threshold keys in go/config.pkl.
    strategy: str = "regex"    # regex, bm25, trigram, fuzzy
    threshold: float = 0.5
    rules: List[KeywordRuleConfig] = field(default_factory=list)


@dataclass
class EmbeddingConfig:
    enabled: bool = False
    model_path: Optional[str] = None
    threshold: float = 0.7


@dataclass
class DomainConfig:
    enabled: bool = False
    model_path: Optional[str] = None


@dataclass
class JailbreakConfig:
    enabled: bool = True
    model_path: Optional[str] = None


@dataclass
class PIIConfig:
    enabled: bool = True
    deny_list: List[str] = field(
        default_factory=lambda: ["EMAIL", "PHONE", "SSN"]
    )


@dataclass
class ToxicityConfig:
    enabled: bool = False
    threshold: float = 0.7


@dataclass
class SafetyConfig:
    jailbreak: JailbreakConfig = field(default_factory=JailbreakConfig)
    pii: PIIConfig = field(default_factory=PIIConfig)
    toxicity: ToxicityConfig = field(default_factory=ToxicityConfig)


@dataclass
class ContextConfig:
    enabled: bool = True
    output_ratio: float = 1.5


@dataclass
class LanguageConfig:
    enabled: bool = False


@dataclass
class ComplexityConfig:
    enabled: bool = False
    model_path: Optional[str] = None
    margin: float = 0.2


@dataclass
class SignalsConfig:
    keyword: KeywordConfig = field(default_factory=KeywordConfig)
    embedding: EmbeddingConfig = field(default_factory=EmbeddingConfig)
    domain: DomainConfig = field(default_factory=DomainConfig)
    safety: SafetyConfig = field(default_factory=SafetyConfig)
    context: ContextConfig = field(default_factory=ContextConfig)
    language: LanguageConfig = field(default_factory=LanguageConfig)
    complexity: ComplexityConfig = field(default_factory=ComplexityConfig)
    format_enabled: bool = True
    code_enabled: bool = True
    toolcall_enabled: bool = True
    cost_enabled: bool = True


@dataclass
class Config:
    max_concurrency: int = 8
    timeout_ms: int = 5000
    signals: SignalsConfig = field(default_factory=SignalsConfig)

    def enabled_signals(self) -> List[SignalType]:
        """Return the enabled signal types.

        ``SignalType`` subclasses ``str``, so members compare equal to
        their PascalCase wire spelling.
        """
        enabled = []
        s = self.signals
        if s.keyword.enabled:
            enabled.append(SignalType.KEYWORD)
        if s.embedding.enabled:
            enabled.append(SignalType.EMBEDDING)
        if s.domain.enabled:
            enabled.append(SignalType.DOMAIN)
        if s.safety.jailbreak.enabled:
            enabled.append(SignalType.JAILBREAK)
        if s.safety.pii.enabled:
            enabled.append(SignalType.PII)
        if s.safety.toxicity.enabled:
            enabled.append(SignalType.TOXICITY)
        if s.context.enabled:
            enabled.append(SignalType.CONTEXT)
        if s.language.enabled:
            enabled.append(SignalType.LANGUAGE)
        if s.complexity.enabled:
            enabled.append(SignalType.COMPLEXITY)
        if s.format_enabled:
            enabled.append(SignalType.OUTPUT_FORMAT)
        if s.code_enabled:
            enabled.append(SignalType.CODE_CONTENT)
        if s.toolcall_enabled:
            enabled.append(SignalType.TOOL_CALLING)
        if s.cost_enabled:
            enabled.append(SignalType.COST_ESTIMATE)
        return enabled

    def to_pipeline_kwargs(self) -> Dict[str, Any]:
        """Convert to kwargs for Pipeline constructor."""
        return {
            "max_concurrency": self.max_concurrency,
            "timeout_ms": self.timeout_ms,
        }


def default_config() -> Config:
    """Return default configuration."""
    return Config()


def load_config(path: str) -> Config:
    """Load configuration from a YAML or PKL file.

    For YAML files: uses PyYAML (must be installed).
    For .pkl files: uses pkl-python if available, otherwise
    raises ImportError.
    """
    p = Path(path)

    if p.suffix == ".pkl":
        return _load_pkl(p)

    return _load_yaml(p)


def _load_yaml(path: Path) -> Config:
    """Load config from YAML file."""
    import yaml

    with open(path) as f:
        data = yaml.safe_load(f) or {}

    return _dict_to_config(data)


def _load_pkl(path: Path) -> Config:
    """Load config from PKL file (requires pkl-python)."""
    try:
        import pkl
    except ImportError:
        raise ImportError(
            "pkl-python is required for .pkl config files. "
            "Install with: pip install pkl-python"
        )

    data = pkl.load(str(path))
    return _dict_to_config(data)


# Flat keys from the shared PKL schema (go/config.pkl), mapped to the
# nested attribute path they set. This table is the whole reason a
# Go-shaped document works: every flat key must land somewhere, and
# anything absent from it is rejected rather than ignored.
_FLAT_KEYS: Dict[str, tuple] = {
    "keyword_enabled": ("signals", "keyword", "enabled"),
    "keyword_strategy": ("signals", "keyword", "strategy"),
    "keyword_threshold": ("signals", "keyword", "threshold"),
    "embedding_enabled": ("signals", "embedding", "enabled"),
    "embedding_model_path": ("signals", "embedding", "model_path"),
    "embedding_threshold": ("signals", "embedding", "threshold"),
    "domain_enabled": ("signals", "domain", "enabled"),
    "domain_model_path": ("signals", "domain", "model_path"),
    "safety_jailbreak_enabled": ("signals", "safety", "jailbreak", "enabled"),
    "safety_jailbreak_model_path": (
        "signals", "safety", "jailbreak", "model_path",
    ),
    "safety_pii_enabled": ("signals", "safety", "pii", "enabled"),
    "safety_toxicity_enabled": ("signals", "safety", "toxicity", "enabled"),
    "safety_toxicity_threshold": (
        "signals", "safety", "toxicity", "threshold",
    ),
    "context_enabled": ("signals", "context", "enabled"),
    "context_output_ratio": ("signals", "context", "output_ratio"),
    "language_enabled": ("signals", "language", "enabled"),
    "complexity_enabled": ("signals", "complexity", "enabled"),
    "complexity_model_path": ("signals", "complexity", "model_path"),
    "complexity_margin": ("signals", "complexity", "margin"),
    "format_enabled": ("signals", "format_enabled"),
    "code_enabled": ("signals", "code_enabled"),
    "toolcall_enabled": ("signals", "toolcall_enabled"),
    "cost_enabled": ("signals", "cost_enabled"),
}

# Top-level keys shared by both shapes.
_TOP_LEVEL_KEYS = {"max_concurrency", "timeout_ms"}

# The flat schema has no notion of keyword rules; that is nested-only.
_KEYWORD_STRATEGY_VALUES = {"regex", "bm25", "trigram", "fuzzy"}


def _reject_unknown(data: dict, allowed: set, where: str) -> None:
    """Raise if *data* holds keys outside *allowed*.

    Silence here is what let disabled safety signals vanish, so an
    unrecognised key is always fatal.
    """
    unknown = sorted(set(data) - allowed)
    if unknown:
        raise ConfigError(
            f"unknown config key(s) in {where}: {', '.join(unknown)}. "
            f"Recognised keys: {', '.join(sorted(allowed))}"
        )


def _apply_flat(cfg: Config, key: str, value: Any) -> None:
    """Set the nested attribute addressed by flat *key*."""
    path = _FLAT_KEYS[key]
    target: Any = cfg
    for part in path[:-1]:
        target = getattr(target, part)
    if not hasattr(target, path[-1]):
        # A mapping pointing at a field that does not exist would
        # otherwise create a phantom attribute nothing ever reads.
        raise ConfigError(
            f"internal: flat key {key!r} maps to unknown field "
            f"{'.'.join(path)}"
        )
    setattr(target, path[-1], value)


def _dict_to_config(data: dict) -> Config:
    """Convert a flat and/or nested dict to a Config dataclass.

    Flat keys (the ``go/config.pkl`` schema) are applied first, then
    the nested ``signals`` block overrides them, so a document mixing
    both has deterministic precedence. Unknown keys raise
    :class:`ConfigError` instead of being dropped.
    """
    if not isinstance(data, dict):
        raise ConfigError(
            f"config root must be a mapping, got {type(data).__name__}"
        )

    allowed = _TOP_LEVEL_KEYS | set(_FLAT_KEYS) | {"signals"}
    _reject_unknown(data, allowed, "top level")

    cfg = Config()

    if "max_concurrency" in data:
        cfg.max_concurrency = int(data["max_concurrency"])
    if "timeout_ms" in data:
        cfg.timeout_ms = int(data["timeout_ms"])

    # Flat layer first -- lowest precedence.
    for key in _FLAT_KEYS:
        if key in data:
            _apply_flat(cfg, key, data[key])

    # Nested layer second -- wins on conflict.
    signals = data.get("signals")
    if signals:
        if not isinstance(signals, dict):
            raise ConfigError(
                f"'signals' must be a mapping, got {type(signals).__name__}"
            )
        _parse_signals(signals, cfg.signals)

    return cfg


def _section(data: dict, name: str, allowed: set) -> dict:
    """Return validated sub-mapping *name* from *data*."""
    sub = data[name]
    if not isinstance(sub, dict):
        raise ConfigError(
            f"'signals.{name}' must be a mapping, "
            f"got {type(sub).__name__}"
        )
    _reject_unknown(sub, allowed, f"signals.{name}")
    return sub


def _overlay(target: Any, sub: dict, keys: tuple) -> None:
    """Copy present *keys* from *sub* onto *target*.

    Absent keys leave the current value alone, so an underlying flat
    setting is preserved rather than reset to the dataclass default.
    """
    for key in keys:
        if key in sub:
            setattr(target, key, sub[key])


def _parse_signals(data: dict, sc: SignalsConfig) -> SignalsConfig:
    """Overlay the nested signals section onto *sc* in place.

    Only keys actually present are written, so values already applied
    from the flat layer survive. Unknown keys are fatal at every
    nesting level.
    """
    allowed = {
        "keyword", "embedding", "domain", "safety", "context",
        "language", "complexity", "format_enabled", "code_enabled",
        "toolcall_enabled", "cost_enabled",
    }
    _reject_unknown(data, allowed, "signals")

    if "keyword" in data:
        kw = _section(data, "keyword", {"enabled", "rules", "strategy",
                                        "threshold"})
        _overlay(sc.keyword, kw, ("enabled", "strategy", "threshold"))
        if "rules" in kw:
            sc.keyword.rules = [
                _parse_keyword_rule(r) for r in kw["rules"]
            ]

    if "embedding" in data:
        e = _section(data, "embedding",
                     {"enabled", "model_path", "threshold"})
        _overlay(sc.embedding, e, ("enabled", "model_path", "threshold"))

    if "domain" in data:
        d = _section(data, "domain", {"enabled", "model_path"})
        _overlay(sc.domain, d, ("enabled", "model_path"))

    if "safety" in data:
        s = _section(data, "safety", {"jailbreak", "pii", "toxicity"})
        if "jailbreak" in s:
            jb = _section(s, "jailbreak", {"enabled", "model_path"})
            _overlay(sc.safety.jailbreak, jb, ("enabled", "model_path"))
        if "pii" in s:
            pii = _section(s, "pii", {"enabled", "deny_list"})
            _overlay(sc.safety.pii, pii, ("enabled", "deny_list"))
        if "toxicity" in s:
            tox = _section(s, "toxicity", {"enabled", "threshold"})
            _overlay(sc.safety.toxicity, tox, ("enabled", "threshold"))

    if "context" in data:
        c = _section(data, "context", {"enabled", "output_ratio"})
        _overlay(sc.context, c, ("enabled", "output_ratio"))

    if "language" in data:
        lang = _section(data, "language", {"enabled"})
        _overlay(sc.language, lang, ("enabled",))

    if "complexity" in data:
        cx = _section(data, "complexity",
                      {"enabled", "model_path", "margin"})
        _overlay(sc.complexity, cx, ("enabled", "model_path", "margin"))

    _overlay(sc, data, ("format_enabled", "code_enabled",
                        "toolcall_enabled", "cost_enabled"))

    return sc


def _parse_keyword_rule(raw: Any) -> KeywordRuleConfig:
    """Build a KeywordRuleConfig, rejecting unknown or missing fields."""
    if not isinstance(raw, dict):
        raise ConfigError(
            f"keyword rule must be a mapping, got {type(raw).__name__}"
        )
    allowed = {"label", "patterns", "operator", "strategy", "threshold"}
    _reject_unknown(raw, allowed, "signals.keyword.rules[]")
    missing = sorted({"label", "patterns"} - set(raw))
    if missing:
        raise ConfigError(
            f"keyword rule missing required field(s): {', '.join(missing)}"
        )
    return KeywordRuleConfig(**raw)

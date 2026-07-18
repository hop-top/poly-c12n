"""Configuration loader for c12n pipeline.

Supports YAML config files with the same schema as the Go consumer.
Optional PKL evaluation when pkl-python is installed.
"""

from __future__ import annotations

import os
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Dict, List, Optional


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

    def enabled_signals(self) -> List[str]:
        """Return list of enabled signal type names."""
        enabled = []
        s = self.signals
        if s.keyword.enabled:
            enabled.append("Keyword")
        if s.embedding.enabled:
            enabled.append("Embedding")
        if s.domain.enabled:
            enabled.append("Domain")
        if s.safety.jailbreak.enabled:
            enabled.append("Jailbreak")
        if s.safety.pii.enabled:
            enabled.append("PII")
        if s.safety.toxicity.enabled:
            enabled.append("Toxicity")
        if s.context.enabled:
            enabled.append("Context")
        if s.language.enabled:
            enabled.append("Language")
        if s.complexity.enabled:
            enabled.append("Complexity")
        if s.format_enabled:
            enabled.append("OutputFormat")
        if s.code_enabled:
            enabled.append("CodeContent")
        if s.toolcall_enabled:
            enabled.append("ToolCalling")
        if s.cost_enabled:
            enabled.append("CostEstimate")
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


def _dict_to_config(data: dict) -> Config:
    """Convert a flat or nested dict to Config dataclass."""
    cfg = Config()

    if "max_concurrency" in data:
        cfg.max_concurrency = int(data["max_concurrency"])
    if "timeout_ms" in data:
        cfg.timeout_ms = int(data["timeout_ms"])

    signals = data.get("signals", {})
    if signals:
        cfg.signals = _parse_signals(signals)

    return cfg


def _parse_signals(data: dict) -> SignalsConfig:
    """Parse signals section of config."""
    sc = SignalsConfig()

    if "keyword" in data:
        kw = data["keyword"]
        sc.keyword = KeywordConfig(
            enabled=kw.get("enabled", True),
            rules=[
                KeywordRuleConfig(**r) for r in kw.get("rules", [])
            ],
        )

    if "embedding" in data:
        e = data["embedding"]
        sc.embedding = EmbeddingConfig(
            enabled=e.get("enabled", False),
            model_path=e.get("model_path"),
            threshold=e.get("threshold", 0.7),
        )

    if "domain" in data:
        d = data["domain"]
        sc.domain = DomainConfig(
            enabled=d.get("enabled", False),
            model_path=d.get("model_path"),
        )

    if "safety" in data:
        s = data["safety"]
        if "jailbreak" in s:
            sc.safety.jailbreak = JailbreakConfig(
                enabled=s["jailbreak"].get("enabled", True),
                model_path=s["jailbreak"].get("model_path"),
            )
        if "pii" in s:
            sc.safety.pii = PIIConfig(
                enabled=s["pii"].get("enabled", True),
                deny_list=s["pii"].get(
                    "deny_list", ["EMAIL", "PHONE", "SSN"]
                ),
            )
        if "toxicity" in s:
            sc.safety.toxicity = ToxicityConfig(
                enabled=s["toxicity"].get("enabled", False),
                threshold=s["toxicity"].get("threshold", 0.7),
            )

    if "context" in data:
        c = data["context"]
        sc.context = ContextConfig(
            enabled=c.get("enabled", True),
            output_ratio=c.get("output_ratio", 1.5),
        )

    if "language" in data:
        sc.language = LanguageConfig(
            enabled=data["language"].get("enabled", False),
        )

    if "complexity" in data:
        cx = data["complexity"]
        sc.complexity = ComplexityConfig(
            enabled=cx.get("enabled", False),
            model_path=cx.get("model_path"),
            margin=cx.get("margin", 0.2),
        )

    sc.format_enabled = data.get("format_enabled", True)
    sc.code_enabled = data.get("code_enabled", True)
    sc.toolcall_enabled = data.get("toolcall_enabled", True)
    sc.cost_enabled = data.get("cost_enabled", True)

    return sc

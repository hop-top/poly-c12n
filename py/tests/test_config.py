"""Tests for c12n.config module."""

from __future__ import annotations

import textwrap
from pathlib import Path

import pytest

from c12n.config import (
    Config,
    KeywordConfig,
    KeywordRuleConfig,
    SignalsConfig,
    _dict_to_config,
    default_config,
    load_config,
)


class TestDefaultConfig:
    def test_returns_config_instance(self):
        cfg = default_config()
        assert isinstance(cfg, Config)

    def test_max_concurrency(self):
        cfg = default_config()
        assert cfg.max_concurrency == 8

    def test_timeout_ms(self):
        cfg = default_config()
        assert cfg.timeout_ms == 5000

    def test_keyword_enabled_by_default(self):
        cfg = default_config()
        assert cfg.signals.keyword.enabled is True

    def test_embedding_disabled_by_default(self):
        cfg = default_config()
        assert cfg.signals.embedding.enabled is False

    def test_jailbreak_enabled_by_default(self):
        cfg = default_config()
        assert cfg.signals.safety.jailbreak.enabled is True

    def test_pii_enabled_by_default(self):
        cfg = default_config()
        assert cfg.signals.safety.pii.enabled is True
        assert cfg.signals.safety.pii.deny_list == [
            "EMAIL", "PHONE", "SSN"
        ]


class TestEnabledSignals:
    def test_default_config_signals(self):
        cfg = default_config()
        enabled = cfg.enabled_signals()
        assert "Keyword" in enabled
        assert "Jailbreak" in enabled
        assert "PII" in enabled
        assert "Context" in enabled
        assert "OutputFormat" in enabled
        assert "CodeContent" in enabled
        assert "ToolCalling" in enabled
        assert "CostEstimate" in enabled
        # Disabled by default
        assert "Embedding" not in enabled
        assert "Domain" not in enabled
        assert "Toxicity" not in enabled
        assert "Language" not in enabled
        assert "Complexity" not in enabled

    def test_all_disabled_returns_empty(self):
        from c12n.config import (
            ContextConfig,
            JailbreakConfig,
            PIIConfig,
            SafetyConfig,
        )

        cfg = Config(
            signals=SignalsConfig(
                keyword=KeywordConfig(enabled=False),
                safety=SafetyConfig(
                    jailbreak=JailbreakConfig(enabled=False),
                    pii=PIIConfig(enabled=False),
                ),
                context=ContextConfig(enabled=False),
                format_enabled=False,
                code_enabled=False,
                toolcall_enabled=False,
                cost_enabled=False,
            ),
        )
        assert cfg.enabled_signals() == []


class TestToPipelineKwargs:
    def test_default_kwargs(self):
        cfg = default_config()
        kwargs = cfg.to_pipeline_kwargs()
        assert kwargs == {
            "max_concurrency": 8,
            "timeout_ms": 5000,
        }

    def test_custom_kwargs(self):
        cfg = Config(max_concurrency=4, timeout_ms=10000)
        kwargs = cfg.to_pipeline_kwargs()
        assert kwargs["max_concurrency"] == 4
        assert kwargs["timeout_ms"] == 10000


class TestDictToConfig:
    def test_empty_dict(self):
        cfg = _dict_to_config({})
        assert cfg.max_concurrency == 8
        assert cfg.timeout_ms == 5000
        assert cfg.signals.keyword.enabled is True

    def test_top_level_overrides(self):
        cfg = _dict_to_config({
            "max_concurrency": 16,
            "timeout_ms": 3000,
        })
        assert cfg.max_concurrency == 16
        assert cfg.timeout_ms == 3000

    def test_full_nested_config(self):
        data = {
            "max_concurrency": 4,
            "timeout_ms": 2000,
            "signals": {
                "keyword": {
                    "enabled": True,
                    "rules": [
                        {
                            "label": "test_rule",
                            "patterns": ["foo.*", "bar"],
                            "operator": "AND",
                            "strategy": "bm25",
                            "threshold": 0.8,
                        },
                    ],
                },
                "embedding": {
                    "enabled": True,
                    "model_path": "/models/embed.bin",
                    "threshold": 0.9,
                },
                "domain": {
                    "enabled": True,
                    "model_path": "/models/domain.bin",
                },
                "safety": {
                    "jailbreak": {
                        "enabled": False,
                        "model_path": "/models/jb.bin",
                    },
                    "pii": {
                        "enabled": True,
                        "deny_list": ["EMAIL", "SSN"],
                    },
                    "toxicity": {
                        "enabled": True,
                        "threshold": 0.5,
                    },
                },
                "context": {
                    "enabled": False,
                    "output_ratio": 2.0,
                },
                "language": {"enabled": True},
                "complexity": {
                    "enabled": True,
                    "model_path": "/models/cx.bin",
                    "margin": 0.3,
                },
                "format_enabled": False,
                "code_enabled": False,
                "toolcall_enabled": False,
                "cost_enabled": False,
            },
        }
        cfg = _dict_to_config(data)

        assert cfg.max_concurrency == 4
        assert cfg.timeout_ms == 2000

        s = cfg.signals
        assert s.keyword.enabled is True
        assert len(s.keyword.rules) == 1
        assert s.keyword.rules[0].label == "test_rule"
        assert s.keyword.rules[0].operator == "AND"
        assert s.keyword.rules[0].strategy == "bm25"
        assert s.keyword.rules[0].threshold == 0.8

        assert s.embedding.enabled is True
        assert s.embedding.model_path == "/models/embed.bin"
        assert s.embedding.threshold == 0.9

        assert s.domain.enabled is True
        assert s.domain.model_path == "/models/domain.bin"

        assert s.safety.jailbreak.enabled is False
        assert s.safety.jailbreak.model_path == "/models/jb.bin"
        assert s.safety.pii.deny_list == ["EMAIL", "SSN"]
        assert s.safety.toxicity.enabled is True
        assert s.safety.toxicity.threshold == 0.5

        assert s.context.enabled is False
        assert s.context.output_ratio == 2.0

        assert s.language.enabled is True

        assert s.complexity.enabled is True
        assert s.complexity.model_path == "/models/cx.bin"
        assert s.complexity.margin == 0.3

        assert s.format_enabled is False
        assert s.code_enabled is False
        assert s.toolcall_enabled is False
        assert s.cost_enabled is False


class TestKeywordRuleConfigDefaults:
    def test_defaults(self):
        rule = KeywordRuleConfig(label="test", patterns=["a", "b"])
        assert rule.operator == "OR"
        assert rule.strategy == "regex"
        assert rule.threshold == 0.5

    def test_custom_values(self):
        rule = KeywordRuleConfig(
            label="custom",
            patterns=["x"],
            operator="AND",
            strategy="fuzzy",
            threshold=0.9,
        )
        assert rule.operator == "AND"
        assert rule.strategy == "fuzzy"
        assert rule.threshold == 0.9


class TestLoadConfigYAML:
    def test_load_yaml(self, tmp_path: Path):
        yaml_content = textwrap.dedent("""\
            max_concurrency: 12
            timeout_ms: 3000
            signals:
              keyword:
                enabled: true
                rules:
                  - label: greeting
                    patterns:
                      - "hello.*"
                      - "hi"
                    operator: OR
                    strategy: regex
                    threshold: 0.6
              embedding:
                enabled: true
                model_path: /tmp/model.bin
                threshold: 0.85
              safety:
                jailbreak:
                  enabled: false
                pii:
                  enabled: true
                  deny_list:
                    - EMAIL
              context:
                enabled: true
                output_ratio: 2.5
              format_enabled: false
        """)
        config_file = tmp_path / "config.yaml"
        config_file.write_text(yaml_content)

        cfg = load_config(str(config_file))

        assert cfg.max_concurrency == 12
        assert cfg.timeout_ms == 3000
        assert cfg.signals.keyword.enabled is True
        assert len(cfg.signals.keyword.rules) == 1
        assert cfg.signals.keyword.rules[0].label == "greeting"
        assert cfg.signals.embedding.enabled is True
        assert cfg.signals.embedding.threshold == 0.85
        assert cfg.signals.safety.jailbreak.enabled is False
        assert cfg.signals.safety.pii.deny_list == ["EMAIL"]
        assert cfg.signals.context.output_ratio == 2.5
        assert cfg.signals.format_enabled is False

    def test_load_empty_yaml(self, tmp_path: Path):
        config_file = tmp_path / "empty.yaml"
        config_file.write_text("")

        cfg = load_config(str(config_file))
        assert cfg.max_concurrency == 8
        assert cfg.timeout_ms == 5000

    def test_load_pkl_raises_without_pkl_python(self, tmp_path: Path):
        config_file = tmp_path / "config.pkl"
        config_file.write_text("dummy")

        with pytest.raises(ImportError, match="pkl-python"):
            load_config(str(config_file))

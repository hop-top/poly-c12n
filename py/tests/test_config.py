"""Tests for c12n.config module."""

from __future__ import annotations

import re
import textwrap
from pathlib import Path

import pytest

from c12n.config import (
    Config,
    ConfigError,
    KeywordConfig,
    KeywordRuleConfig,
    SignalsConfig,
    SignalType,
    _dict_to_config,
    _FLAT_KEYS,
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


# ---------------------------------------------------------------------------
# Regression: Go-shaped (flat) config must not be silently discarded.
#
# The flat schema is the cross-language contract in go/config.pkl. It was
# parsed as "no recognised keys", so every flat setting fell back to the
# Python default -- a config disabling PII detection produced a pipeline
# with PII detection ON, with no error and no warning.
# ---------------------------------------------------------------------------


# Every flat key in go/config.pkl paired with a non-default value and the
# nested attribute path it must reach.
FLAT_SCHEMA_CASES = [
    ("max_concurrency", 4, ("max_concurrency",)),
    ("timeout_ms", 1234, ("timeout_ms",)),
    ("keyword_enabled", False, ("signals", "keyword", "enabled")),
    ("keyword_strategy", "bm25", ("signals", "keyword", "strategy")),
    ("keyword_threshold", 0.11, ("signals", "keyword", "threshold")),
    ("embedding_enabled", True, ("signals", "embedding", "enabled")),
    ("embedding_model_path", "/m/e.bin",
     ("signals", "embedding", "model_path")),
    ("embedding_threshold", 0.22, ("signals", "embedding", "threshold")),
    ("domain_enabled", True, ("signals", "domain", "enabled")),
    ("domain_model_path", "/m/d.bin", ("signals", "domain", "model_path")),
    ("safety_jailbreak_enabled", False,
     ("signals", "safety", "jailbreak", "enabled")),
    ("safety_jailbreak_model_path", "/m/jb.bin",
     ("signals", "safety", "jailbreak", "model_path")),
    ("safety_pii_enabled", False, ("signals", "safety", "pii", "enabled")),
    ("safety_toxicity_enabled", True,
     ("signals", "safety", "toxicity", "enabled")),
    ("safety_toxicity_threshold", 0.33,
     ("signals", "safety", "toxicity", "threshold")),
    ("context_enabled", False, ("signals", "context", "enabled")),
    ("context_output_ratio", 3.5, ("signals", "context", "output_ratio")),
    ("language_enabled", True, ("signals", "language", "enabled")),
    ("complexity_enabled", True, ("signals", "complexity", "enabled")),
    ("complexity_model_path", "/m/cx.bin",
     ("signals", "complexity", "model_path")),
    ("complexity_margin", 0.44, ("signals", "complexity", "margin")),
    ("format_enabled", False, ("signals", "format_enabled")),
    ("code_enabled", False, ("signals", "code_enabled")),
    ("toolcall_enabled", False, ("signals", "toolcall_enabled")),
    ("cost_enabled", False, ("signals", "cost_enabled")),
]


def _resolve(cfg, path):
    """Read the attribute addressed by a dotted *path* tuple."""
    node = cfg
    for part in path:
        node = getattr(node, part)
    return node


class TestFlatGoSchemaRegression:
    """A Go-shaped document must actually take effect."""

    @pytest.mark.parametrize("key,value,path", FLAT_SCHEMA_CASES)
    def test_flat_key_applied(self, key, value, path):
        cfg = _dict_to_config({key: value})
        actual = _resolve(cfg, path)
        assert actual == value, (
            f"flat key {key!r}={value!r} was discarded; "
            f"{'.'.join(path)} is {actual!r}"
        )

    def test_flat_key_set_differs_from_default(self):
        """Guard the cases table: each value must be a real change."""
        cfg = default_config()
        for key, value, path in FLAT_SCHEMA_CASES:
            assert _resolve(cfg, path) != value, (
                f"case for {key!r} uses the default value, so it "
                f"cannot detect a silent discard"
            )

    def test_disabling_safety_signals_flat(self):
        """The original bug: disabled safety signals stayed enabled."""
        cfg = _dict_to_config({
            "safety_pii_enabled": False,
            "safety_toxicity_enabled": False,
            "safety_jailbreak_enabled": False,
            "keyword_enabled": False,
            "context_enabled": False,
        })

        assert cfg.signals.safety.pii.enabled is False
        assert cfg.signals.safety.toxicity.enabled is False
        assert cfg.signals.safety.jailbreak.enabled is False
        assert cfg.signals.keyword.enabled is False
        assert cfg.signals.context.enabled is False

        enabled = cfg.enabled_signals()
        for absent in ("PII", "Toxicity", "Jailbreak", "Keyword", "Context"):
            assert absent not in enabled

    def test_flat_yaml_end_to_end(self, tmp_path: Path):
        """Same defect via the real YAML loader, not just the dict path."""
        config_file = tmp_path / "go_shaped.yaml"
        config_file.write_text(textwrap.dedent("""\
            max_concurrency: 4
            timeout_ms: 1000
            safety_pii_enabled: false
            safety_toxicity_enabled: false
            keyword_enabled: false
        """))

        cfg = load_config(str(config_file))

        assert cfg.max_concurrency == 4
        assert cfg.timeout_ms == 1000
        assert cfg.signals.safety.pii.enabled is False
        assert cfg.signals.keyword.enabled is False
        assert "PII" not in cfg.enabled_signals()
        assert "Keyword" not in cfg.enabled_signals()

    def test_flat_schema_covers_pkl_contract(self):
        """Every key in go/config.pkl must be recognised.

        Parses the shared PKL schema so a key added there without a
        Python mapping fails here instead of being silently dropped.
        """
        pkl_path = (
            Path(__file__).resolve().parents[2] / "go" / "config.pkl"
        )
        if not pkl_path.exists():  # pragma: no cover - polyglot checkout
            pytest.skip("go/config.pkl not present in this checkout")

        declared = set(
            re.findall(
                r"^([a-z][a-z0-9_]*)\s*:", pkl_path.read_text(), re.MULTILINE
            )
        )
        assert declared, "failed to parse any keys from config.pkl"

        missing = declared - set(_FLAT_KEYS) - {"max_concurrency",
                                                "timeout_ms"}
        assert not missing, (
            f"config.pkl keys with no Python mapping: {sorted(missing)}"
        )


class TestUnknownKeysRejected:
    """Unrecognised keys must fail loudly, never be dropped."""

    def test_unknown_top_level_key(self):
        with pytest.raises(ConfigError, match="safety_pii_enable"):
            _dict_to_config({"safety_pii_enable": False})

    def test_unknown_nested_signal_key(self):
        with pytest.raises(ConfigError, match="jailbrake"):
            _dict_to_config({"signals": {"safety": {"jailbrake": {}}}})

    def test_unknown_leaf_key(self):
        with pytest.raises(ConfigError, match="enabld"):
            _dict_to_config({"signals": {"safety": {"pii": {
                "enabld": False,
            }}}})

    def test_nested_block_must_be_mapping(self):
        with pytest.raises(ConfigError, match="must be a mapping"):
            _dict_to_config({"signals": {"context": True}})

    def test_root_must_be_mapping(self):
        with pytest.raises(ConfigError, match="must be a mapping"):
            _dict_to_config([1, 2, 3])

    def test_keyword_rule_unknown_field(self):
        with pytest.raises(ConfigError, match="pattern"):
            _dict_to_config({"signals": {"keyword": {"rules": [
                {"label": "x", "pattern": ["a"]},
            ]}}})

    def test_unknown_key_in_yaml_load(self, tmp_path: Path):
        config_file = tmp_path / "typo.yaml"
        config_file.write_text("safety_pii_enabledd: false\n")
        with pytest.raises(ConfigError, match="safety_pii_enabledd"):
            load_config(str(config_file))


class TestMixedShapePrecedence:
    """Both shapes may coexist; nested wins."""

    def test_nested_overrides_flat(self):
        cfg = _dict_to_config({
            "safety_pii_enabled": True,
            "signals": {"safety": {"pii": {"enabled": False}}},
        })
        assert cfg.signals.safety.pii.enabled is False

    def test_flat_survives_unrelated_nested_block(self):
        """A nested block must not reset flat siblings to defaults."""
        cfg = _dict_to_config({
            "safety_pii_enabled": False,
            "signals": {"safety": {"toxicity": {"enabled": True}}},
        })
        assert cfg.signals.safety.pii.enabled is False
        assert cfg.signals.safety.toxicity.enabled is True

    def test_flat_survives_partial_nested_leaf(self):
        cfg = _dict_to_config({
            "context_output_ratio": 9.0,
            "signals": {"context": {"enabled": False}},
        })
        assert cfg.signals.context.enabled is False
        assert cfg.signals.context.output_ratio == 9.0


class TestSignalTypeEnum:
    """SignalType must stay in lockstep with core/src/types.rs."""

    def test_values_are_pascal_case_wire_spellings(self):
        assert SignalType.PII.value == "PII"
        assert SignalType.TOXICITY.value == "Toxicity"
        assert SignalType.OUTPUT_FORMAT.value == "OutputFormat"

    def test_compares_equal_to_plain_string(self):
        assert SignalType.TOXICITY == "Toxicity"
        assert SignalType.TOXICITY != "toxicity"

    def test_matches_core_enum_exactly(self):
        """Parse core/src/types.rs so a new core variant fails here."""
        types_rs = (
            Path(__file__).resolve().parents[2] / "core" / "src" / "types.rs"
        )
        if not types_rs.exists():  # pragma: no cover - polyglot checkout
            pytest.skip("core/src/types.rs not present in this checkout")

        block = re.search(
            r"pub enum SignalType\s*\{(.*?)\}", types_rs.read_text(), re.S
        )
        assert block, "failed to locate SignalType in core/src/types.rs"

        variants = re.findall(r"^\s*([A-Z][A-Za-z]*),", block.group(1),
                              re.MULTILINE)
        assert len(variants) == 20, variants
        assert [s.value for s in SignalType] == variants

    def test_enabled_signals_returns_enum_members(self):
        enabled = default_config().enabled_signals()
        assert all(isinstance(s, SignalType) for s in enabled)
        assert SignalType.PII in enabled

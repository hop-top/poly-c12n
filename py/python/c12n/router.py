"""Signal-based router for routellm using c12n classification signals."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable, Dict, List, Optional


@dataclass
class SignalRule:
    """A rule that maps signal conditions to a strong model win rate.

    If all conditions match, returns `win_rate` value.
    Conditions are evaluated against the c12n signal results.
    """

    signal_type: str  # e.g. "Complexity", "Keyword"
    condition: Callable[[dict], bool]  # predicate on the SignalResult dict
    win_rate: float  # 0-1, what to return if condition matches
    priority: int = 0  # higher = checked first


class SignalRouter:
    """Routes prompts using c12n classification signals.

    Implements the routellm Router interface:
    - calculate_strong_win_rate(prompt) -> float

    Unlike other routellm routers that run their own ML models,
    SignalRouter reads pre-computed c12n signals from the pipeline
    and applies rule-based logic to decide routing.

    Usage::

        pipeline = Pipeline(max_concurrency=8, timeout_ms=5000)
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
        # Use with routellm's routing logic:
        # model = router.route(prompt, threshold=0.7, routed_pair)
    """

    NO_PARALLEL = True  # Pipeline handles its own concurrency

    def __init__(
        self,
        pipeline: Any,
        rules: List[SignalRule],
        default_win_rate: float = 0.5,
        fallback_on_error: float = 0.5,
    ):
        self.pipeline = pipeline
        self.rules = sorted(rules, key=lambda r: -r.priority)
        self.default_win_rate = default_win_rate
        self.fallback_on_error = fallback_on_error

    def calculate_strong_win_rate(self, prompt: str) -> float:
        """Evaluate prompt through c12n pipeline and apply rules."""
        try:
            result_json = self.pipeline.evaluate(prompt)
        except Exception:
            return self.fallback_on_error

        import json

        try:
            raw = (
                result_json
                if isinstance(result_json, str)
                else result_json.json()
            )
            result = json.loads(raw)
        except (json.JSONDecodeError, AttributeError):
            return self.fallback_on_error

        signals: Dict[str, dict] = {
            r["signal_type"]: r for r in result.get("results", [])
        }

        # Apply rules in priority order; first match wins
        for rule in self.rules:
            signal = signals.get(rule.signal_type)
            if signal is None:
                continue
            try:
                if rule.condition(signal):
                    return rule.win_rate
            except Exception:
                continue

        return self.default_win_rate

    def route(self, prompt, threshold, routed_pair):
        """Route to strong or weak model based on signal analysis."""
        if self.calculate_strong_win_rate(prompt) >= threshold:
            return routed_pair.strong
        return routed_pair.weak

    @classmethod
    def from_config(
        cls, pipeline: Any, config: dict
    ) -> "SignalRouter":
        """Create SignalRouter from a config dict.

        Config format::

            {
                "default_win_rate": 0.5,
                "fallback_on_error": 0.5,
                "rules": [
                    {
                        "signal_type": "Complexity",
                        "match_labels": ["complex"],
                        "win_rate": 0.9,
                        "priority": 10
                    },
                    {
                        "signal_type": "CodeContent",
                        "min_confidence": 0.7,
                        "win_rate": 0.8,
                        "priority": 5
                    }
                ]
            }
        """
        rules: List[SignalRule] = []
        for rule_cfg in config.get("rules", []):
            signal_type = rule_cfg["signal_type"]
            win_rate = rule_cfg.get("win_rate", 0.8)
            priority = rule_cfg.get("priority", 0)

            # Build condition from config
            conditions: List[Callable[[dict], bool]] = []
            if "match_labels" in rule_cfg:
                labels = set(rule_cfg["match_labels"])
                conditions.append(
                    lambda r, ls=labels: bool(
                        ls & set(r.get("labels", []))
                    )
                )
            if "min_confidence" in rule_cfg:
                min_conf = rule_cfg["min_confidence"]
                conditions.append(
                    lambda r, mc=min_conf: r.get("confidence", 0)
                    >= mc
                )
            if "has_metadata" in rule_cfg:
                key = rule_cfg["has_metadata"]
                conditions.append(
                    lambda r, k=key: k in r.get("metadata", {})
                )

            if not conditions:
                # Default: signal exists with any confidence > 0
                conditions.append(
                    lambda r: r.get("confidence", 0) > 0
                )

            condition = lambda r, conds=conditions: all(
                c(r) for c in conds
            )
            rules.append(
                SignalRule(signal_type, condition, win_rate, priority)
            )

        return cls(
            pipeline,
            rules,
            default_win_rate=config.get("default_win_rate", 0.5),
            fallback_on_error=config.get(
                "fallback_on_error", 0.5
            ),
        )

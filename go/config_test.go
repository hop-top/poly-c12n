package c12n

import (
	"os"
	"path/filepath"
	"testing"
	"time"

	"hop.top/kit/go/core/config"
)

func TestDefaultConfig(t *testing.T) {
	cfg := DefaultConfig()

	if cfg.MaxConcurrency != 8 {
		t.Errorf("MaxConcurrency = %d, want 8", cfg.MaxConcurrency)
	}
	if cfg.TimeoutMs != 5000 {
		t.Errorf("TimeoutMs = %d, want 5000", cfg.TimeoutMs)
	}
	if !cfg.KeywordEnabled {
		t.Error("KeywordEnabled = false, want true")
	}
	if cfg.KeywordStrategy != "regex" {
		t.Errorf("KeywordStrategy = %q, want %q", cfg.KeywordStrategy, "regex")
	}
	if cfg.KeywordThreshold != 0.5 {
		t.Errorf("KeywordThreshold = %f, want 0.5", cfg.KeywordThreshold)
	}
	if cfg.EmbeddingEnabled {
		t.Error("EmbeddingEnabled = true, want false")
	}
	if cfg.EmbeddingThreshold != 0.7 {
		t.Errorf("EmbeddingThreshold = %f, want 0.7", cfg.EmbeddingThreshold)
	}
	if !cfg.SafetyJailbreakEnabled {
		t.Error("SafetyJailbreakEnabled = false, want true")
	}
	if !cfg.SafetyPIIEnabled {
		t.Error("SafetyPIIEnabled = false, want true")
	}
	if cfg.SafetyToxicityEnabled {
		t.Error("SafetyToxicityEnabled = true, want false")
	}
	if !cfg.ContextEnabled {
		t.Error("ContextEnabled = false, want true")
	}
	if cfg.ContextOutputRatio != 1.5 {
		t.Errorf("ContextOutputRatio = %f, want 1.5", cfg.ContextOutputRatio)
	}
	if cfg.LanguageEnabled {
		t.Error("LanguageEnabled = true, want false")
	}
	if cfg.ComplexityEnabled {
		t.Error("ComplexityEnabled = true, want false")
	}
	if cfg.ComplexityMargin != 0.2 {
		t.Errorf("ComplexityMargin = %f, want 0.2", cfg.ComplexityMargin)
	}
	if !cfg.FormatEnabled {
		t.Error("FormatEnabled = false, want true")
	}
	if !cfg.CodeEnabled {
		t.Error("CodeEnabled = false, want true")
	}
	if !cfg.ToolcallEnabled {
		t.Error("ToolcallEnabled = false, want true")
	}
	if !cfg.CostEnabled {
		t.Error("CostEnabled = false, want true")
	}
}

func TestLoadConfig(t *testing.T) {
	dir := t.TempDir()
	cfgFile := filepath.Join(dir, "c12n.yaml")

	yaml := `max_concurrency: 4
timeout_ms: 10000
keyword_enabled: false
embedding_enabled: true
embedding_threshold: 0.85
safety_toxicity_enabled: true
safety_toxicity_threshold: 0.9
`
	if err := os.WriteFile(cfgFile, []byte(yaml), 0o644); err != nil {
		t.Fatal(err)
	}

	cfg, err := LoadConfig(config.Options{
		ProjectConfigPath: cfgFile,
	})
	if err != nil {
		t.Fatal(err)
	}

	if cfg.MaxConcurrency != 4 {
		t.Errorf("MaxConcurrency = %d, want 4", cfg.MaxConcurrency)
	}
	if cfg.TimeoutMs != 10000 {
		t.Errorf("TimeoutMs = %d, want 10000", cfg.TimeoutMs)
	}
	if cfg.KeywordEnabled {
		t.Error("KeywordEnabled = true, want false")
	}
	if !cfg.EmbeddingEnabled {
		t.Error("EmbeddingEnabled = false, want true")
	}
	if cfg.EmbeddingThreshold != 0.85 {
		t.Errorf("EmbeddingThreshold = %f, want 0.85", cfg.EmbeddingThreshold)
	}
	if !cfg.SafetyToxicityEnabled {
		t.Error("SafetyToxicityEnabled = false, want true")
	}
	if cfg.SafetyToxicityThreshold != 0.9 {
		t.Errorf("SafetyToxicityThreshold = %f, want 0.9", cfg.SafetyToxicityThreshold)
	}
	// Defaults should be preserved for unset fields.
	if cfg.KeywordStrategy != "regex" {
		t.Errorf("KeywordStrategy = %q, want %q (default)", cfg.KeywordStrategy, "regex")
	}
	if !cfg.SafetyPIIEnabled {
		t.Error("SafetyPIIEnabled = false, want true (default)")
	}
}

func TestLoadConfig_MissingFile(t *testing.T) {
	cfg, err := LoadConfig(config.Options{
		ProjectConfigPath: "/nonexistent/c12n.yaml",
	})
	if err != nil {
		t.Fatal(err)
	}
	// Should return defaults when file is missing.
	def := DefaultConfig()
	if cfg.MaxConcurrency != def.MaxConcurrency {
		t.Errorf("MaxConcurrency = %d, want %d", cfg.MaxConcurrency, def.MaxConcurrency)
	}
}

func TestToPipelineConfig(t *testing.T) {
	cfg := DefaultConfig()
	pc := cfg.ToPipelineConfig()

	if pc.MaxConcurrency != 8 {
		t.Errorf("MaxConcurrency = %d, want 8", pc.MaxConcurrency)
	}
	want := 5000 * time.Millisecond
	if pc.Timeout != want {
		t.Errorf("Timeout = %v, want %v", pc.Timeout, want)
	}
}

func TestToPipelineConfig_Custom(t *testing.T) {
	cfg := DefaultConfig()
	cfg.MaxConcurrency = 16
	cfg.TimeoutMs = 30000
	pc := cfg.ToPipelineConfig()

	if pc.MaxConcurrency != 16 {
		t.Errorf("MaxConcurrency = %d, want 16", pc.MaxConcurrency)
	}
	want := 30 * time.Second
	if pc.Timeout != want {
		t.Errorf("Timeout = %v, want %v", pc.Timeout, want)
	}
}

func TestEnabledSignals_Default(t *testing.T) {
	cfg := DefaultConfig()
	signals := cfg.EnabledSignals()

	expected := []SignalType{
		SignalKeyword,
		SignalJailbreak,
		SignalPII,
		SignalContext,
		SignalOutputFormat,
		SignalCodeContent,
		SignalToolCalling,
		SignalCostEstimate,
	}

	if len(signals) != len(expected) {
		t.Fatalf("len(signals) = %d, want %d; got %v",
			len(signals), len(expected), signals)
	}
	for i, s := range signals {
		if s != expected[i] {
			t.Errorf("signals[%d] = %q, want %q", i, s, expected[i])
		}
	}
}

func TestEnabledSignals_AllEnabled(t *testing.T) {
	cfg := DefaultConfig()
	cfg.EmbeddingEnabled = true
	cfg.DomainEnabled = true
	cfg.SafetyToxicityEnabled = true
	cfg.LanguageEnabled = true
	cfg.ComplexityEnabled = true

	signals := cfg.EnabledSignals()
	if len(signals) != 13 {
		t.Errorf("len(signals) = %d, want 13; got %v", len(signals), signals)
	}
}

func TestEnabledSignals_NoneEnabled(t *testing.T) {
	cfg := Config{} // zero value: all booleans false
	signals := cfg.EnabledSignals()
	if len(signals) != 0 {
		t.Errorf("len(signals) = %d, want 0; got %v", len(signals), signals)
	}
}

func TestConfigSchema(t *testing.T) {
	schema, err := ConfigSchema()
	if err != nil {
		t.Fatal(err)
	}
	if schema.ModuleName != "c12n.Config" {
		t.Errorf("ModuleName = %q, want %q", schema.ModuleName, "c12n.Config")
	}
	if len(schema.Fields) == 0 {
		t.Error("expected at least one field in schema")
	}

	// Spot-check a few fields.
	fieldMap := make(map[string]struct{}, len(schema.Fields))
	for _, f := range schema.Fields {
		fieldMap[f.Path] = struct{}{}
	}
	for _, key := range []string{
		"max_concurrency", "timeout_ms", "keyword_enabled",
		"keyword_strategy", "embedding_threshold", "cost_enabled",
	} {
		if _, ok := fieldMap[key]; !ok {
			t.Errorf("missing field %q in schema", key)
		}
	}
}

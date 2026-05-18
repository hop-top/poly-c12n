package c12n

import (
	_ "embed"
	"os"
	"time"

	"hop.top/kit/go/core/config"
	"hop.top/kit/go/core/config/pkl"
)

//go:embed config.pkl
var configPklSource string

// Config holds all c12n pipeline configuration.
type Config struct {
	// Pipeline-level settings.
	MaxConcurrency int `yaml:"max_concurrency" json:"max_concurrency"`
	TimeoutMs      int `yaml:"timeout_ms" json:"timeout_ms"`

	// Keyword signal.
	KeywordEnabled   bool    `yaml:"keyword_enabled" json:"keyword_enabled"`
	KeywordStrategy  string  `yaml:"keyword_strategy" json:"keyword_strategy"`
	KeywordThreshold float64 `yaml:"keyword_threshold" json:"keyword_threshold"`

	// Embedding signal.
	EmbeddingEnabled   bool    `yaml:"embedding_enabled" json:"embedding_enabled"`
	EmbeddingModelPath *string `yaml:"embedding_model_path" json:"embedding_model_path"`
	EmbeddingThreshold float64 `yaml:"embedding_threshold" json:"embedding_threshold"`

	// Domain classification signal.
	DomainEnabled   bool    `yaml:"domain_enabled" json:"domain_enabled"`
	DomainModelPath *string `yaml:"domain_model_path" json:"domain_model_path"`

	// Safety signals.
	SafetyJailbreakEnabled   bool    `yaml:"safety_jailbreak_enabled" json:"safety_jailbreak_enabled"`
	SafetyJailbreakModelPath *string `yaml:"safety_jailbreak_model_path" json:"safety_jailbreak_model_path"`
	SafetyPIIEnabled         bool    `yaml:"safety_pii_enabled" json:"safety_pii_enabled"`
	SafetyToxicityEnabled    bool    `yaml:"safety_toxicity_enabled" json:"safety_toxicity_enabled"`
	SafetyToxicityThreshold  float64 `yaml:"safety_toxicity_threshold" json:"safety_toxicity_threshold"`

	// Context signal.
	ContextEnabled     bool    `yaml:"context_enabled" json:"context_enabled"`
	ContextOutputRatio float64 `yaml:"context_output_ratio" json:"context_output_ratio"`

	// Language detection signal.
	LanguageEnabled bool `yaml:"language_enabled" json:"language_enabled"`

	// Complexity classification signal.
	ComplexityEnabled   bool    `yaml:"complexity_enabled" json:"complexity_enabled"`
	ComplexityModelPath *string `yaml:"complexity_model_path" json:"complexity_model_path"`
	ComplexityMargin    float64 `yaml:"complexity_margin" json:"complexity_margin"`

	// Output format detection signal.
	FormatEnabled bool `yaml:"format_enabled" json:"format_enabled"`

	// Code content detection signal.
	CodeEnabled bool `yaml:"code_enabled" json:"code_enabled"`

	// Tool-calling detection signal.
	ToolcallEnabled bool `yaml:"toolcall_enabled" json:"toolcall_enabled"`

	// Cost estimation signal.
	CostEnabled bool `yaml:"cost_enabled" json:"cost_enabled"`
}

// DefaultConfig returns a Config with sensible defaults matching the PKL schema.
func DefaultConfig() Config {
	return Config{
		MaxConcurrency:          8,
		TimeoutMs:               5000,
		KeywordEnabled:          true,
		KeywordStrategy:         "regex",
		KeywordThreshold:        0.5,
		EmbeddingEnabled:        false,
		EmbeddingThreshold:      0.7,
		DomainEnabled:           false,
		SafetyJailbreakEnabled:  true,
		SafetyPIIEnabled:        true,
		SafetyToxicityEnabled:   false,
		SafetyToxicityThreshold: 0.7,
		ContextEnabled:          true,
		ContextOutputRatio:      1.5,
		LanguageEnabled:         false,
		ComplexityEnabled:       false,
		ComplexityMargin:        0.2,
		FormatEnabled:           true,
		CodeEnabled:             true,
		ToolcallEnabled:         true,
		CostEnabled:             true,
	}
}

// LoadConfig loads a layered YAML config into a Config struct.
// Defaults are applied first, then overridden by file layers.
func LoadConfig(opts config.Options) (*Config, error) {
	cfg := DefaultConfig()
	if err := config.Load(&cfg, opts); err != nil {
		return nil, err
	}
	return &cfg, nil
}

// ToPipelineConfig converts Config to a PipelineConfig suitable for NewPipeline.
func (c *Config) ToPipelineConfig() PipelineConfig {
	return PipelineConfig{
		MaxConcurrency: c.MaxConcurrency,
		Timeout:        time.Duration(c.TimeoutMs) * time.Millisecond,
	}
}

// EnabledSignals returns the list of signal types enabled in this config.
func (c *Config) EnabledSignals() []SignalType {
	var out []SignalType
	if c.KeywordEnabled {
		out = append(out, SignalKeyword)
	}
	if c.EmbeddingEnabled {
		out = append(out, SignalEmbedding)
	}
	if c.DomainEnabled {
		out = append(out, SignalDomain)
	}
	if c.SafetyJailbreakEnabled {
		out = append(out, SignalJailbreak)
	}
	if c.SafetyPIIEnabled {
		out = append(out, SignalPII)
	}
	if c.SafetyToxicityEnabled {
		out = append(out, SignalToxicity)
	}
	if c.ContextEnabled {
		out = append(out, SignalContext)
	}
	if c.LanguageEnabled {
		out = append(out, SignalLanguage)
	}
	if c.ComplexityEnabled {
		out = append(out, SignalComplexity)
	}
	if c.FormatEnabled {
		out = append(out, SignalOutputFormat)
	}
	if c.CodeEnabled {
		out = append(out, SignalCodeContent)
	}
	if c.ToolcallEnabled {
		out = append(out, SignalToolCalling)
	}
	if c.CostEnabled {
		out = append(out, SignalCostEstimate)
	}
	return out
}

// ConfigPklSource returns the embedded PKL schema source.
func ConfigPklSource() string { return configPklSource }

// ConfigSchema loads and returns the PKL schema for c12n configuration.
// Uses the embedded config.pkl source, writing to a temp file for pkl.LoadSchema.
func ConfigSchema() (*pkl.Schema, error) {
	tmp, err := os.CreateTemp("", "c12n-config-*.pkl")
	if err != nil {
		return nil, err
	}
	defer os.Remove(tmp.Name())

	if _, err := tmp.WriteString(configPklSource); err != nil {
		tmp.Close()
		return nil, err
	}
	tmp.Close()

	return pkl.LoadSchema(tmp.Name())
}

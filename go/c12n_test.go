package c12n

import (
	"encoding/json"
	"testing"
	"time"
)

func TestNormalizeContext_NilFieldsBecomeEmpty(t *testing.T) {
	ctx := ClassificationContext{Text: "hello"}
	normalizeContext(&ctx)
	if ctx.History == nil {
		t.Error("History should not be nil after normalize")
	}
	if ctx.Headers == nil {
		t.Error("Headers should not be nil after normalize")
	}
	if ctx.Config == nil {
		t.Error("Config should not be nil after normalize")
	}
}

func TestNormalizeContext_PreservesExistingValues(t *testing.T) {
	imgURL := "https://example.com/x.png"
	ctx := ClassificationContext{
		Text:     "hello",
		History:  []string{"prev"},
		Headers:  map[string]string{"X-Tenant": "acme"},
		ImageURL: &imgURL,
		Config:   map[string]any{"k": "v"},
	}
	normalizeContext(&ctx)
	if len(ctx.History) != 1 || ctx.History[0] != "prev" {
		t.Errorf("History mutated: %v", ctx.History)
	}
	if ctx.Headers["X-Tenant"] != "acme" {
		t.Errorf("Headers mutated: %v", ctx.Headers)
	}
	if ctx.ImageURL == nil || *ctx.ImageURL != imgURL {
		t.Errorf("ImageURL mutated")
	}
	if ctx.Config["k"] != "v" {
		t.Errorf("Config mutated: %v", ctx.Config)
	}
}

func TestClassificationContext_JSONRoundTrip(t *testing.T) {
	imgURL := "https://example.com/image.png"
	ctx := ClassificationContext{
		Text:     "hello world",
		History:  []string{"prev1", "prev2"},
		Headers:  map[string]string{"X-Custom": "value", "Authorization": "Bearer tok"},
		ImageURL: &imgURL,
		Config:   map[string]any{"temperature": 0.7, "max_tokens": 100},
	}

	data, err := json.Marshal(ctx)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var got ClassificationContext
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if got.Text != ctx.Text {
		t.Errorf("Text = %q, want %q", got.Text, ctx.Text)
	}
	if len(got.History) != len(ctx.History) {
		t.Fatalf("History len = %d, want %d", len(got.History), len(ctx.History))
	}
	for i, v := range ctx.History {
		if got.History[i] != v {
			t.Errorf("History[%d] = %q, want %q", i, got.History[i], v)
		}
	}
	if len(got.Headers) != len(ctx.Headers) {
		t.Fatalf("Headers len = %d, want %d", len(got.Headers), len(ctx.Headers))
	}
	for k, v := range ctx.Headers {
		if got.Headers[k] != v {
			t.Errorf("Headers[%q] = %q, want %q", k, got.Headers[k], v)
		}
	}
	if got.ImageURL == nil || *got.ImageURL != imgURL {
		t.Errorf("ImageURL = %v, want %q", got.ImageURL, imgURL)
	}
	if got.Config == nil {
		t.Fatal("Config is nil")
	}
}

func TestClassificationContext_EmptyDefaults(t *testing.T) {
	ctx := ClassificationContext{}

	data, err := json.Marshal(ctx)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var got ClassificationContext
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if got.Text != "" {
		t.Errorf("Text = %q, want empty", got.Text)
	}
	if got.ImageURL != nil {
		t.Errorf("ImageURL = %v, want nil", got.ImageURL)
	}

	// Verify omitempty: image_url should be absent from JSON.
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatalf("unmarshal raw: %v", err)
	}
	if _, ok := raw["image_url"]; ok {
		t.Error("image_url present in JSON for nil ImageURL")
	}
}

func TestPipelineConfig_FromConfig(t *testing.T) {
	cfg := DefaultConfig()
	pc := cfg.ToPipelineConfig()

	if pc.MaxConcurrency != cfg.MaxConcurrency {
		t.Errorf("MaxConcurrency = %d, want %d", pc.MaxConcurrency, cfg.MaxConcurrency)
	}

	wantTimeout := time.Duration(cfg.TimeoutMs) * time.Millisecond
	if pc.Timeout != wantTimeout {
		t.Errorf("Timeout = %v, want %v", pc.Timeout, wantTimeout)
	}
}

func TestPipelineConfig_CustomValues(t *testing.T) {
	cfg := Config{
		MaxConcurrency: 16,
		TimeoutMs:      10000,
	}
	pc := cfg.ToPipelineConfig()

	if pc.MaxConcurrency != 16 {
		t.Errorf("MaxConcurrency = %d, want 16", pc.MaxConcurrency)
	}
	if pc.Timeout != 10*time.Second {
		t.Errorf("Timeout = %v, want 10s", pc.Timeout)
	}
}

func TestAllSignalTypes_Count(t *testing.T) {
	types := AllSignalTypes()
	if len(types) != 20 {
		t.Errorf("AllSignalTypes() len = %d, want 20", len(types))
	}
}

func TestAllSignalTypes_Unique(t *testing.T) {
	types := AllSignalTypes()
	seen := make(map[string]bool, len(types))
	for _, s := range types {
		if seen[s] {
			t.Errorf("duplicate signal type: %q", s)
		}
		seen[s] = true
	}
}

func TestAllSignalTypes_MatchConsts(t *testing.T) {
	expected := []SignalType{
		SignalKeyword, SignalEmbedding, SignalDomain, SignalJailbreak,
		SignalPII, SignalToxicity, SignalContext, SignalStructure,
		SignalLanguage, SignalComplexity, SignalPreference, SignalFeedback,
		SignalOutputFormat, SignalCodeContent, SignalToolCalling,
		SignalCostEstimate, SignalSentiment, SignalIntent, SignalTopic,
		SignalCustom,
	}

	types := AllSignalTypes()
	if len(types) != len(expected) {
		t.Fatalf("len = %d, want %d", len(types), len(expected))
	}
	for i, e := range expected {
		if types[i] != string(e) {
			t.Errorf("types[%d] = %q, want %q", i, types[i], e)
		}
	}
}

func TestPipelineError_SignalFailed(t *testing.T) {
	raw := `{"SignalFailed":{"name":"keyword","error":"model not found"}}`

	var pe PipelineError
	if err := json.Unmarshal([]byte(raw), &pe); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if pe.SignalFailed == nil {
		t.Fatal("SignalFailed is nil")
	}
	if pe.SignalFailed.Name != "keyword" {
		t.Errorf("Name = %q, want %q", pe.SignalFailed.Name, "keyword")
	}
	if pe.SignalFailed.Error != "model not found" {
		t.Errorf("Error = %q, want %q", pe.SignalFailed.Error, "model not found")
	}
	if pe.Timeout != nil {
		t.Error("Timeout should be nil")
	}
}

func TestPipelineError_Timeout(t *testing.T) {
	raw := `{"Timeout":{"name":"embedding"}}`

	var pe PipelineError
	if err := json.Unmarshal([]byte(raw), &pe); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if pe.Timeout == nil {
		t.Fatal("Timeout is nil")
	}
	if pe.Timeout.Name != "embedding" {
		t.Errorf("Name = %q, want %q", pe.Timeout.Name, "embedding")
	}
	if pe.SignalFailed != nil {
		t.Error("SignalFailed should be nil")
	}
}

func TestDefaultConfig_EnabledSignals(t *testing.T) {
	cfg := DefaultConfig()
	signals := cfg.EnabledSignals()

	// DefaultConfig enables: Keyword, Jailbreak, PII, Context,
	// OutputFormat, CodeContent, ToolCalling, CostEstimate.
	want := map[SignalType]bool{
		SignalKeyword:      true,
		SignalJailbreak:    true,
		SignalPII:          true,
		SignalContext:      true,
		SignalOutputFormat: true,
		SignalCodeContent:  true,
		SignalToolCalling:  true,
		SignalCostEstimate: true,
	}

	if len(signals) != len(want) {
		t.Errorf("EnabledSignals() len = %d, want %d", len(signals), len(want))
	}

	got := make(map[SignalType]bool, len(signals))
	for _, s := range signals {
		got[s] = true
	}
	for s := range want {
		if !got[s] {
			t.Errorf("missing expected signal: %s", s)
		}
	}
}

func TestEnabledSignals_AllEnabled_Roundtrip(t *testing.T) {
	cfg := Config{
		KeywordEnabled:         true,
		EmbeddingEnabled:       true,
		DomainEnabled:          true,
		SafetyJailbreakEnabled: true,
		SafetyPIIEnabled:       true,
		SafetyToxicityEnabled:  true,
		ContextEnabled:         true,
		LanguageEnabled:        true,
		ComplexityEnabled:      true,
		FormatEnabled:          true,
		CodeEnabled:            true,
		ToolcallEnabled:        true,
		CostEnabled:            true,
	}

	signals := cfg.EnabledSignals()
	if len(signals) != 13 {
		t.Errorf("EnabledSignals() len = %d, want 13", len(signals))
	}

	// Verify round-trip: re-marshal config, reload, compare signals.
	data, err := json.Marshal(cfg)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var got Config
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	gotSignals := got.EnabledSignals()
	if len(gotSignals) != len(signals) {
		t.Errorf("round-trip EnabledSignals() len = %d, want %d", len(gotSignals), len(signals))
	}
}

func TestEnabledSignals_NoneEnabled_ZeroValue(t *testing.T) {
	cfg := Config{}
	signals := cfg.EnabledSignals()
	if len(signals) != 0 {
		t.Errorf("EnabledSignals() len = %d, want 0", len(signals))
	}
}

func TestPipelineConfig_JSONRoundTrip(t *testing.T) {
	pc := PipelineConfig{
		MaxConcurrency: 12,
		Timeout:        3 * time.Second,
	}

	data, err := json.Marshal(pc)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var got PipelineConfig
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if got.MaxConcurrency != pc.MaxConcurrency {
		t.Errorf("MaxConcurrency = %d, want %d", got.MaxConcurrency, pc.MaxConcurrency)
	}
	if got.Timeout != pc.Timeout {
		t.Errorf("Timeout = %v, want %v", got.Timeout, pc.Timeout)
	}
}

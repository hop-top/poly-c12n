package c12n

import (
	"encoding/json"
	"testing"
	"time"
)

// ---------------------------------------------------------------------------
// a) Config round-trip
// ---------------------------------------------------------------------------

func TestE2E_DefaultConfigToPipeline(t *testing.T) {
	cfg := DefaultConfig()
	pc := cfg.ToPipelineConfig()

	if pc.MaxConcurrency != 8 {
		t.Errorf("MaxConcurrency = %d, want 8", pc.MaxConcurrency)
	}
	if pc.Timeout != 5*time.Second {
		t.Errorf("Timeout = %v, want 5s", pc.Timeout)
	}

	// Stub returns errNoCgo; the point is that the conversion path works.
	_, err := NewPipeline(pc)
	if err == nil {
		t.Skip("cgo pipeline available; stub path not exercised")
	}
	if err.Error() != "c12n: built without cgo support" {
		t.Errorf("unexpected error: %v", err)
	}
}

func TestE2E_EnabledSignals_NonEmpty(t *testing.T) {
	cfg := DefaultConfig()
	signals := cfg.EnabledSignals()

	if len(signals) == 0 {
		t.Fatal("EnabledSignals() returned empty slice for default config")
	}
}

func TestE2E_ConfigSchema_ValidEmbedded(t *testing.T) {
	schema, err := ConfigSchema()
	if err != nil {
		t.Fatalf("ConfigSchema: %v", err)
	}
	if schema == nil {
		t.Fatal("ConfigSchema returned nil")
	}
	if len(schema.Fields) == 0 {
		t.Error("schema has no fields")
	}
}

// ---------------------------------------------------------------------------
// b) Pipeline evaluate + parse
// ---------------------------------------------------------------------------

// validResultJSON is a realistic pipeline JSON result for stub-mode tests.
const validResultJSON = `{
	"results": [
		{
			"name": "format_detect",
			"signal_type": "OutputFormat",
			"confidence": 0.92,
			"labels": ["json"],
			"metadata": {}
		},
		{
			"name": "cost_est",
			"signal_type": "CostEstimate",
			"confidence": 0.6,
			"labels": ["small"],
			"metadata": {"input_tokens": 42}
		}
	],
	"errors": [],
	"duration_ns": 5000000
}`

func TestE2E_ParseResult_Accessors(t *testing.T) {
	r, err := ParseResult(validResultJSON)
	if err != nil {
		t.Fatalf("ParseResult: %v", err)
	}

	if r.HasErrors() {
		t.Error("HasErrors() = true, want false")
	}

	sigs := r.Signals(SignalOutputFormat)
	if len(sigs) != 1 {
		t.Fatalf("Signals(OutputFormat) len = %d, want 1", len(sigs))
	}

	if r.Duration() < 0 {
		t.Errorf("Duration() = %v, want >= 0", r.Duration())
	}
	if r.Duration() != 5*time.Millisecond {
		t.Errorf("Duration() = %v, want 5ms", r.Duration())
	}
}

func TestE2E_ParseResult_HasErrors(t *testing.T) {
	withErr := `{
		"results": [],
		"errors": [{"SignalFailed":{"name":"broken","error":"boom"}}],
		"duration_ns": 100
	}`
	r, err := ParseResult(withErr)
	if err != nil {
		t.Fatalf("ParseResult: %v", err)
	}
	if !r.HasErrors() {
		t.Error("HasErrors() = false, want true")
	}
}

// ---------------------------------------------------------------------------
// c) Signal types completeness
// ---------------------------------------------------------------------------

func TestE2E_AllSignalTypes_Count20(t *testing.T) {
	types := AllSignalTypes()
	if len(types) != 20 {
		t.Fatalf("AllSignalTypes() len = %d, want 20", len(types))
	}
}

func TestE2E_AllSignalTypes_Unique(t *testing.T) {
	types := AllSignalTypes()
	seen := make(map[string]bool, len(types))
	for _, s := range types {
		if seen[s] {
			t.Errorf("duplicate signal type: %q", s)
		}
		seen[s] = true
	}
}

func TestE2E_AllSignalTypes_KnownPresent(t *testing.T) {
	types := AllSignalTypes()
	set := make(map[string]bool, len(types))
	for _, s := range types {
		set[s] = true
	}

	required := []string{
		"Keyword", "Embedding", "Domain", "Jailbreak", "PII",
		"Toxicity", "Context", "Structure", "Language", "Complexity",
		"Preference", "Feedback", "OutputFormat", "CodeContent",
		"ToolCalling", "CostEstimate", "Sentiment", "Intent",
		"Topic", "Custom",
	}

	for _, r := range required {
		if !set[r] {
			t.Errorf("missing required signal type: %q", r)
		}
	}
}

// ---------------------------------------------------------------------------
// d) CLI command structure (structural assertions via cobra)
// ---------------------------------------------------------------------------

func TestE2E_ClassifyCommand_Flags(t *testing.T) {
	cmd := NewClassifyCommand(&Pipeline{})

	flags := []string{"format", "signal"}
	for _, f := range flags {
		if cmd.Flags().Lookup(f) == nil {
			t.Errorf("classify missing --%s flag", f)
		}
	}
}

func TestE2E_BenchCommand_Flags(t *testing.T) {
	cmd := NewBenchCommand(&Pipeline{})

	flags := []string{"iterations", "text"}
	for _, f := range flags {
		if cmd.Flags().Lookup(f) == nil {
			t.Errorf("bench missing --%s flag", f)
		}
	}
}

// ---------------------------------------------------------------------------
// e) PipelineResult accessors
// ---------------------------------------------------------------------------

func TestE2E_PipelineResult_Signal(t *testing.T) {
	r, _ := ParseResult(validResultJSON)

	s := r.Signal(SignalOutputFormat)
	if s == nil {
		t.Fatal("Signal(OutputFormat) = nil")
	}
	if s.Type != SignalOutputFormat {
		t.Errorf("Type = %q, want OutputFormat", s.Type)
	}

	miss := r.Signal(SignalPII)
	if miss != nil {
		t.Error("Signal(PII) should be nil for missing type")
	}
}

func TestE2E_PipelineResult_Confidence_Range(t *testing.T) {
	r, _ := ParseResult(validResultJSON)

	for _, sig := range r.Results {
		if sig.Confidence < 0 || sig.Confidence > 1 {
			t.Errorf("signal %q confidence %f outside [0,1]", sig.Name, sig.Confidence)
		}
	}
}

func TestE2E_ParseResult_InvalidJSON_Error(t *testing.T) {
	cases := []struct {
		name  string
		input string
	}{
		{"empty", ""},
		{"garbage", "not json at all"},
		{"truncated", `{"results":[`},
		{"wrong_type", `"just a string"`},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			_, err := ParseResult(tc.input)
			if err == nil {
				t.Error("expected error for invalid JSON")
			}
		})
	}
}

func TestE2E_PipelineResult_HasSignal(t *testing.T) {
	r, _ := ParseResult(validResultJSON)

	if !r.HasSignal(SignalCostEstimate) {
		t.Error("HasSignal(CostEstimate) = false, want true")
	}
	if r.HasSignal(SignalJailbreak) {
		t.Error("HasSignal(Jailbreak) = true, want false")
	}
}

func TestE2E_PipelineResult_Confidence_Accessor(t *testing.T) {
	r, _ := ParseResult(validResultJSON)

	c := r.Confidence(SignalOutputFormat)
	if c != 0.92 {
		t.Errorf("Confidence(OutputFormat) = %f, want 0.92", c)
	}

	c = r.Confidence(SignalDomain)
	if c != 0 {
		t.Errorf("Confidence(Domain) = %f, want 0 for missing", c)
	}
}

// ---------------------------------------------------------------------------
// f) ClassificationContext JSON round-trip
// ---------------------------------------------------------------------------

func TestE2E_ClassificationContext_FullRoundTrip(t *testing.T) {
	imgURL := "https://example.com/screenshot.png"
	ctx := ClassificationContext{
		Text:     "Classify this prompt with all fields",
		History:  []string{"previous turn", "another turn"},
		Headers:  map[string]string{"X-Request-Id": "abc", "Authorization": "Bearer t"},
		ImageURL: &imgURL,
		Config:   map[string]any{"model": "gpt-4", "temperature": 0.7},
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

func TestE2E_ClassificationContext_MinimalFields(t *testing.T) {
	ctx := ClassificationContext{Text: "just text"}

	data, err := json.Marshal(ctx)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var got ClassificationContext
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if got.Text != "just text" {
		t.Errorf("Text = %q, want 'just text'", got.Text)
	}
	if got.ImageURL != nil {
		t.Error("ImageURL should be nil for omitted field")
	}

	// Verify image_url is absent from JSON (omitempty)
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(data, &raw); err != nil {
		t.Fatalf("unmarshal raw: %v", err)
	}
	if _, ok := raw["image_url"]; ok {
		t.Error("image_url present in JSON for nil ImageURL")
	}
}

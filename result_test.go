package c12n

import (
	"testing"
	"time"
)

const testJSON = `{
	"results": [
		{
			"name": "keyword",
			"signal_type": "Keyword",
			"confidence": 0.85,
			"labels": ["greeting", "polite"],
			"metadata": {"matched_rule": "hello_pattern"}
		},
		{
			"name": "toxicity_check",
			"signal_type": "Toxicity",
			"confidence": 0.12,
			"labels": [],
			"metadata": {}
		},
		{
			"name": "keyword_secondary",
			"signal_type": "Keyword",
			"confidence": 0.60,
			"labels": ["farewell"],
			"metadata": {}
		}
	],
	"errors": [
		{"SignalFailed": {"name": "domain", "error": "model not loaded"}}
	],
	"duration_ns": 1234567
}`

func TestParseResult(t *testing.T) {
	r, err := ParseResult(testJSON)
	if err != nil {
		t.Fatalf("ParseResult: %v", err)
	}
	if len(r.Results) != 3 {
		t.Fatalf("expected 3 results, got %d", len(r.Results))
	}
	if len(r.Errors) != 1 {
		t.Fatalf("expected 1 error, got %d", len(r.Errors))
	}
	if r.DurationNs != 1234567 {
		t.Fatalf("expected duration_ns 1234567, got %d", r.DurationNs)
	}
}

func TestParseResultInvalid(t *testing.T) {
	_, err := ParseResult("not json")
	if err == nil {
		t.Fatal("expected error for invalid JSON")
	}
}

func TestSignal(t *testing.T) {
	r, _ := ParseResult(testJSON)

	s := r.Signal(SignalKeyword)
	if s == nil {
		t.Fatal("expected Keyword signal")
	}
	if s.Name != "keyword" {
		t.Fatalf("expected name 'keyword', got %q", s.Name)
	}

	s = r.Signal(SignalDomain)
	if s != nil {
		t.Fatal("expected nil for missing signal type")
	}
}

func TestHasSignal(t *testing.T) {
	r, _ := ParseResult(testJSON)

	if !r.HasSignal(SignalKeyword) {
		t.Fatal("expected HasSignal(Keyword) = true")
	}
	if r.HasSignal(SignalPII) {
		t.Fatal("expected HasSignal(PII) = false")
	}
}

func TestConfidence(t *testing.T) {
	r, _ := ParseResult(testJSON)

	c := r.Confidence(SignalKeyword)
	if c != 0.85 {
		t.Fatalf("expected confidence 0.85, got %f", c)
	}

	c = r.Confidence(SignalDomain)
	if c != 0 {
		t.Fatalf("expected confidence 0 for missing signal, got %f", c)
	}
}

func TestSignals(t *testing.T) {
	r, _ := ParseResult(testJSON)

	keywords := r.Signals(SignalKeyword)
	if len(keywords) != 2 {
		t.Fatalf("expected 2 Keyword signals, got %d", len(keywords))
	}

	toxicity := r.Signals(SignalToxicity)
	if len(toxicity) != 1 {
		t.Fatalf("expected 1 Toxicity signal, got %d", len(toxicity))
	}

	missing := r.Signals(SignalJailbreak)
	if len(missing) != 0 {
		t.Fatalf("expected 0 Jailbreak signals, got %d", len(missing))
	}
}

func TestDuration(t *testing.T) {
	r, _ := ParseResult(testJSON)

	d := r.Duration()
	expected := time.Duration(1234567) * time.Nanosecond
	if d != expected {
		t.Fatalf("expected duration %v, got %v", expected, d)
	}
}

func TestHasErrors(t *testing.T) {
	r, _ := ParseResult(testJSON)
	if !r.HasErrors() {
		t.Fatal("expected HasErrors() = true")
	}

	noErr, _ := ParseResult(`{"results":[],"errors":[],"duration_ns":0}`)
	if noErr.HasErrors() {
		t.Fatal("expected HasErrors() = false for empty errors")
	}
}

func TestPipelineErrorFields(t *testing.T) {
	r, _ := ParseResult(testJSON)

	e := r.Errors[0]
	if e.SignalFailed == nil {
		t.Fatal("expected SignalFailed != nil")
	}
	if e.SignalFailed.Name != "domain" {
		t.Fatalf("expected error name 'domain', got %q", e.SignalFailed.Name)
	}
	if e.SignalFailed.Error != "model not loaded" {
		t.Fatalf("expected error 'model not loaded', got %q", e.SignalFailed.Error)
	}
	if e.Timeout != nil {
		t.Fatal("expected Timeout = nil")
	}
}

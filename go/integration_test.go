//go:build cgo && integration

// Integration tests require the c12n-core cdylib and cgo.
// Run with:
//   CGO_ENABLED=1 go test -tags integration -run TestIntegration ./...

package c12n

import (
	"encoding/json"
	"testing"
	"time"
)

func TestIntegration_PipelineLifecycle(t *testing.T) {
	cfg := PipelineConfig{MaxConcurrency: 4, Timeout: 5 * time.Second}
	p, err := NewPipeline(cfg)
	if err != nil {
		t.Fatalf("NewPipeline: %v", err)
	}
	defer p.Close()

	raw, err := p.Evaluate(ClassificationContext{Text: "test input"})
	if err != nil {
		t.Fatalf("Evaluate: %v", err)
	}
	if raw == "" {
		t.Fatal("Evaluate returned empty string")
	}
}

func TestIntegration_PipelineEmptyResult(t *testing.T) {
	cfg := PipelineConfig{MaxConcurrency: 4, Timeout: 5 * time.Second}
	p, err := NewPipeline(cfg)
	if err != nil {
		t.Fatalf("NewPipeline: %v", err)
	}
	defer p.Close()

	raw, err := p.Evaluate(ClassificationContext{Text: "hello"})
	if err != nil {
		t.Fatalf("Evaluate: %v", err)
	}

	result, err := ParseResult(raw)
	if err != nil {
		t.Fatalf("ParseResult: %v", err)
	}

	// Pipeline with no registered signals should return a valid but
	// empty result set.
	if result == nil {
		t.Fatal("ParseResult returned nil")
	}
}

func TestIntegration_PipelineCloseIdempotent(t *testing.T) {
	cfg := PipelineConfig{MaxConcurrency: 4, Timeout: 5 * time.Second}
	p, err := NewPipeline(cfg)
	if err != nil {
		t.Fatalf("NewPipeline: %v", err)
	}

	// Close twice — must not panic.
	p.Close()
	p.Close()
}

func TestIntegration_PipelineEvaluateAfterClose(t *testing.T) {
	cfg := PipelineConfig{MaxConcurrency: 4, Timeout: 5 * time.Second}
	p, err := NewPipeline(cfg)
	if err != nil {
		t.Fatalf("NewPipeline: %v", err)
	}

	p.Close()

	_, err = p.Evaluate(ClassificationContext{Text: "post-close"})
	if err == nil {
		t.Fatal("expected error evaluating closed pipeline")
	}
}

func TestIntegration_JSONRoundTripThroughFFI(t *testing.T) {
	cfg := PipelineConfig{MaxConcurrency: 4, Timeout: 5 * time.Second}
	p, err := NewPipeline(cfg)
	if err != nil {
		t.Fatalf("NewPipeline: %v", err)
	}
	defer p.Close()

	imgURL := "https://example.com/img.png"
	ctx := ClassificationContext{
		Text:     "classify this text please",
		History:  []string{"previous message"},
		Headers:  map[string]string{"X-Request-Id": "abc123"},
		ImageURL: &imgURL,
		Config:   map[string]any{"mode": "fast"},
	}

	raw, err := p.Evaluate(ctx)
	if err != nil {
		t.Fatalf("Evaluate: %v", err)
	}

	result, err := ParseResult(raw)
	if err != nil {
		t.Fatalf("ParseResult: %v", err)
	}

	// Verify it's valid JSON by re-marshalling.
	if _, err := json.Marshal(result); err != nil {
		t.Fatalf("re-marshal result: %v", err)
	}

	// DurationNs should be non-negative.
	if result.DurationNs < 0 {
		t.Errorf("DurationNs = %d, want >= 0", result.DurationNs)
	}
}

func TestIntegration_ContextCancellation(t *testing.T) {
	cfg := PipelineConfig{MaxConcurrency: 4, Timeout: 5 * time.Second}
	p, err := NewPipeline(cfg)
	if err != nil {
		t.Fatalf("NewPipeline: %v", err)
	}
	defer p.Close()

	// Current API doesn't accept context.Context — verify the pipeline
	// still handles a minimal call gracefully. If context cancellation
	// support is added later, this test should be updated to use
	// context.WithCancel.
	_, err = p.Evaluate(ClassificationContext{Text: ""})
	// Either success or a well-formed error is acceptable.
	if err != nil {
		t.Logf("Evaluate with empty text returned error (acceptable): %v", err)
	}
}

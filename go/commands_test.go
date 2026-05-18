package c12n

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"
)

func TestNewClassifyCommand(t *testing.T) {
	cmd := NewClassifyCommand(&Pipeline{})
	if cmd == nil {
		t.Fatal("NewClassifyCommand returned nil")
	}
	if cmd.Use != "classify [text...]" {
		t.Errorf("unexpected Use: %s", cmd.Use)
	}
	if cmd.Short == "" {
		t.Error("Short description is empty")
	}

	// Verify flags exist.
	if f := cmd.Flags().Lookup("format"); f == nil {
		t.Error("missing --format flag")
	}
	if f := cmd.Flags().Lookup("signal"); f == nil {
		t.Error("missing --signal flag")
	}
}

func TestNewBenchCommand(t *testing.T) {
	cmd := NewBenchCommand(&Pipeline{})
	if cmd == nil {
		t.Fatal("NewBenchCommand returned nil")
	}
	if cmd.Use != "bench" {
		t.Errorf("unexpected Use: %s", cmd.Use)
	}
	if cmd.Short == "" {
		t.Error("Short description is empty")
	}

	// Verify flags exist.
	if f := cmd.Flags().Lookup("iterations"); f == nil {
		t.Error("missing --iterations flag")
	}
	if f := cmd.Flags().Lookup("text"); f == nil {
		t.Error("missing --text flag")
	}
}

func TestRegisterCompletions(t *testing.T) {
	// Should not panic.
	cmd := NewClassifyCommand(&Pipeline{})
	RegisterCompletions(cmd)
}

func TestAllSignalTypes(t *testing.T) {
	types := AllSignalTypes()
	if len(types) != 20 {
		t.Errorf("expected 20 signal types, got %d", len(types))
	}
}

func TestRenderResultJSON(t *testing.T) {
	r := &PipelineResult{
		Results: []SignalResult{
			{Name: "test", Type: SignalIntent, Confidence: 0.95, Labels: []string{"greeting"}},
		},
		DurationNs: 1000000,
	}

	var buf bytes.Buffer
	if err := renderResult(&buf, r, "json"); err != nil {
		t.Fatalf("renderResult json: %v", err)
	}

	var decoded PipelineResult
	if err := json.Unmarshal(buf.Bytes(), &decoded); err != nil {
		t.Fatalf("invalid JSON output: %v", err)
	}
	if len(decoded.Results) != 1 {
		t.Errorf("expected 1 result, got %d", len(decoded.Results))
	}
}

func TestRenderResultTable(t *testing.T) {
	r := &PipelineResult{
		Results: []SignalResult{
			{Name: "kw", Type: SignalKeyword, Confidence: 0.5, Labels: []string{"a", "b"}},
		},
	}

	var buf bytes.Buffer
	if err := renderResult(&buf, r, "table"); err != nil {
		t.Fatalf("renderResult table: %v", err)
	}

	out := buf.String()
	if !strings.Contains(out, "NAME") {
		t.Error("table missing header")
	}
	if !strings.Contains(out, "kw") {
		t.Error("table missing result row")
	}
}

func TestRenderResultText(t *testing.T) {
	r := &PipelineResult{
		Results: []SignalResult{
			{Name: "sent", Type: SignalSentiment, Confidence: 0.8, Labels: []string{"positive"}},
		},
	}

	var buf bytes.Buffer
	if err := renderResult(&buf, r, "text"); err != nil {
		t.Fatalf("renderResult text: %v", err)
	}

	out := buf.String()
	if !strings.Contains(out, "sent (Sentiment)") {
		t.Errorf("unexpected text output: %s", out)
	}
	if !strings.Contains(out, "[positive]") {
		t.Errorf("missing labels in text output: %s", out)
	}
}

func TestRenderResultUnknownFormat(t *testing.T) {
	r := &PipelineResult{}
	var buf bytes.Buffer
	if err := renderResult(&buf, r, "xml"); err == nil {
		t.Error("expected error for unknown format")
	}
}

func TestFilterSignal(t *testing.T) {
	r := &PipelineResult{
		Results: []SignalResult{
			{Name: "a", Type: SignalIntent},
			{Name: "b", Type: SignalTopic},
			{Name: "c", Type: SignalIntent},
		},
	}

	filtered := filterSignal(r, SignalIntent)
	if len(filtered.Results) != 2 {
		t.Errorf("expected 2 filtered results, got %d", len(filtered.Results))
	}
}

func TestResolveTextFromArgs(t *testing.T) {
	text, err := resolveText([]string{"hello", "world"}, strings.NewReader(""))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if text != "hello world" {
		t.Errorf("unexpected text: %s", text)
	}
}

func TestResolveTextFromStdin(t *testing.T) {
	text, err := resolveText(nil, strings.NewReader("from stdin\n"))
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if text != "from stdin" {
		t.Errorf("unexpected text: %q", text)
	}
}

func TestResolveTextEmpty(t *testing.T) {
	_, err := resolveText(nil, strings.NewReader(""))
	if err == nil {
		t.Error("expected error for empty input")
	}
}

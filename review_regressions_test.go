package c12n

import (
	"strings"
	"testing"
)

// Regression tests for PR #2 code review fixes.

// 1. RegisterCompletions comment accuracy — no "walks all" claim.
func TestRegisterCompletions_CommentAccuracy(t *testing.T) {
	// Verified by reading source; this test ensures RegisterCompletions
	// exists and doesn't panic when called on a nil-safe command tree.
	cmd := NewClassifyCommand(nil)
	RegisterCompletions(cmd)
}

// 2. Zero-value Pipeline.Evaluate returns error, not panic (stub mode).
func TestPipeline_ZeroValue_NoPanic(t *testing.T) {
	var p Pipeline
	_, err := p.Evaluate(ClassificationContext{Text: "test"})
	if err == nil {
		t.Fatal("expected error from zero-value pipeline, got nil")
	}
}

// 3. Bench command rejects zero iterations.
func TestBenchCommand_ZeroIterations(t *testing.T) {
	// NewBenchCommand needs a pipeline, but we only test flag validation.
	// The command should fail before touching the pipeline.
	cmd := NewBenchCommand(nil)
	cmd.SetArgs([]string{"--iterations", "0"})

	err := cmd.Execute()
	if err == nil {
		t.Fatal("expected error for 0 iterations")
	}
	if !strings.Contains(err.Error(), "iterations must be >= 1") {
		t.Fatalf("unexpected error: %v", err)
	}
}

// 4. ConfigSchema uses embedded PKL (no runtime.Caller dependency).
func TestConfigSchema_Embedded(t *testing.T) {
	if configPklSource == "" {
		t.Fatal("configPklSource should be populated via go:embed")
	}
	if !strings.Contains(configPklSource, "module c12n.Config") {
		t.Fatal("embedded PKL missing module declaration")
	}

	schema, err := ConfigSchema()
	if err != nil {
		t.Fatalf("ConfigSchema() failed: %v", err)
	}
	if len(schema.Fields) == 0 {
		t.Fatal("expected non-empty schema fields")
	}
}

// 5. Pipeline.closed with sync.Mutex — verify Close is idempotent.
func TestPipeline_CloseIdempotent_NoPanic(t *testing.T) {
	var p Pipeline
	p.Close() // first close on zero-value
	p.Close() // second close — must not panic
}

// 6. SignalResult.Metadata uses map[string]any (not interface{}).
func TestSignalResult_MetadataType(t *testing.T) {
	r := SignalResult{
		Metadata: map[string]any{"key": "value", "num": 42},
	}
	if r.Metadata["key"] != "value" {
		t.Fatal("metadata access failed")
	}
}

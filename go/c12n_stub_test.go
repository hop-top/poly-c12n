//go:build !c12n_native

package c12n

import (
	"testing"
	"time"
)

func TestNewPipeline_NativeDisabled(t *testing.T) {
	_, err := NewPipeline(PipelineConfig{MaxConcurrency: 4, Timeout: 5 * time.Second})
	if err == nil {
		t.Fatal("expected error from stub NewPipeline")
	}
	if err.Error() != "c12n: native engine disabled (build with -tags c12n_native)" {
		t.Errorf("unexpected error: %v", err)
	}
}

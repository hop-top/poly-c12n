//go:build !cgo

package c12n

import (
	"testing"
	"time"
)

func TestNewPipeline_NoCgo(t *testing.T) {
	_, err := NewPipeline(PipelineConfig{MaxConcurrency: 4, Timeout: 5 * time.Second})
	if err == nil {
		t.Fatal("expected error from stub NewPipeline")
	}
	if err.Error() != "c12n: built without cgo support" {
		t.Errorf("unexpected error: %v", err)
	}
}

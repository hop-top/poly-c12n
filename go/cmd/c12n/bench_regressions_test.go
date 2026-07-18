package main

import (
	"encoding/json"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	c12n "hop.top/c12n"
)

// --- T-0069: benchPercentile nearest-rank indexing ---
//
// PR #4 review comment #3: idx := (p * len(ds)) / 100 is off-by-one.
// For a sorted slice of 100 elements (0..99), p50 should return
// ds[49] (the 50th element, 0-indexed), not ds[50] (the 51st).
// Fix: idx := (p * (len(ds) - 1)) / 100

func TestBenchPercentile_P50_Returns50thElement(t *testing.T) {
	// Build sorted slice: 0ms, 1ms, 2ms, ..., 99ms (100 elements).
	ds := make([]time.Duration, 100)
	for i := range ds {
		ds[i] = time.Duration(i) * time.Millisecond
	}

	got := benchPercentile(ds, 50)
	want := 49 * time.Millisecond // index 49 = 50th element (0-indexed)

	if got != want {
		t.Errorf("p50 of [0..99]ms: got %v (index 50), want %v (index 49)",
			got, want)
	}
}

func TestBenchPercentile_P0_ReturnsFirstElement(t *testing.T) {
	ds := make([]time.Duration, 100)
	for i := range ds {
		ds[i] = time.Duration(i) * time.Millisecond
	}

	got := benchPercentile(ds, 0)
	if got != 0 {
		t.Errorf("p0: got %v, want 0", got)
	}
}

func TestBenchPercentile_P100_ReturnsLastElement(t *testing.T) {
	ds := make([]time.Duration, 100)
	for i := range ds {
		ds[i] = time.Duration(i) * time.Millisecond
	}

	got := benchPercentile(ds, 100)
	want := 99 * time.Millisecond
	if got != want {
		t.Errorf("p100: got %v, want %v", got, want)
	}
}

func TestBenchPercentile_SingleElement(t *testing.T) {
	ds := []time.Duration{42 * time.Millisecond}

	for _, p := range []int{0, 25, 50, 75, 100} {
		got := benchPercentile(ds, p)
		if got != 42*time.Millisecond {
			t.Errorf("p%d of single-element slice: got %v, want 42ms", p, got)
		}
	}
}

func TestBenchPercentile_EmptySlice(t *testing.T) {
	got := benchPercentile(nil, 50)
	if got != 0 {
		t.Errorf("p50 of empty slice: got %v, want 0", got)
	}
}

func TestBenchPercentile_P95_P99(t *testing.T) {
	// 100 elements: 0..99ms. nearest-rank:
	// p95 -> index (95*(100-1))/100 = 94 -> 94ms
	// p99 -> index (99*(100-1))/100 = 98 -> 98ms
	ds := make([]time.Duration, 100)
	for i := range ds {
		ds[i] = time.Duration(i) * time.Millisecond
	}

	if got := benchPercentile(ds, 95); got != 94*time.Millisecond {
		t.Errorf("p95: got %v, want 94ms", got)
	}
	if got := benchPercentile(ds, 99); got != 98*time.Millisecond {
		t.Errorf("p99: got %v, want 98ms", got)
	}
}

// --- T-0070: loadJSONLInputs bufio.Scanner token limit ---
//
// PR #4 review comment #4: default bufio.Scanner caps at ~64KB.
// Large ClassificationContext JSONL lines will fail silently.
// Fix: scanner.Buffer(make([]byte, 0, 64*1024), 10*1024*1024)

func TestLoadJSONLInputs_LargeLine(t *testing.T) {
	// Build a ClassificationContext with a ~100KB text field.
	// Default bufio.Scanner max token size is 64KB, so this must fail
	// before the fix and pass after.
	bigText := strings.Repeat("x", 100*1024)
	ctx := c12n.ClassificationContext{Text: bigText}
	data, err := json.Marshal(ctx)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	dir := t.TempDir()
	fpath := filepath.Join(dir, "large.jsonl")
	if err := os.WriteFile(fpath, data, 0644); err != nil {
		t.Fatalf("write temp file: %v", err)
	}

	contexts, err := loadJSONLInputs(fpath)
	if err != nil {
		t.Fatalf("loadJSONLInputs failed on %d byte line: %v", len(data), err)
	}
	if len(contexts) != 1 {
		t.Fatalf("expected 1 context, got %d", len(contexts))
	}
	if len(contexts[0].Text) != len(bigText) {
		t.Errorf("text truncated: got %d bytes, want %d",
			len(contexts[0].Text), len(bigText))
	}
}

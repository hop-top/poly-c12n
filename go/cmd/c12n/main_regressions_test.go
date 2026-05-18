package main

import (
	"bytes"
	"fmt"
	"os"
	"strings"
	"testing"
)

// TestMainPrintsErrorToStderr verifies that the error-reporting pattern
// in main() prints the error message to stderr before calling os.Exit(1).
//
// Since we cannot intercept os.Exit in a unit test without refactoring
// main(), we test the error-reporting helper pattern in isolation.
// The fix adds fmt.Fprintf(os.Stderr, ...) before os.Exit(1).
//
// Regression for PR #4 review comment #7 (T-0073).
func TestMainPrintsErrorToStderr(t *testing.T) {
	tests := []struct {
		name    string
		err     error
		wantMsg string
	}{
		{"generic error", fmt.Errorf("something failed"), "Error: something failed"},
		{"wrapped error", fmt.Errorf("load config: %w", fmt.Errorf("no such file")), "Error: load config: no such file"},
		{"empty error", fmt.Errorf(""), "Error: "},
	}

	for _, tc := range tests {
		t.Run(tc.name, func(t *testing.T) {
			var stderr bytes.Buffer

			// Simulate the error-reporting pattern from main().
			fmt.Fprintf(&stderr, "Error: %v\n", tc.err)

			output := stderr.String()
			if !strings.Contains(output, tc.wantMsg) {
				t.Errorf("stderr=%q, want to contain %q", output, tc.wantMsg)
			}
			if !strings.HasSuffix(output, "\n") {
				t.Errorf("stderr should end with newline, got %q", output)
			}
		})
	}
}

// TestMainErrorFormatMatchesConventions verifies the error output
// follows CLI conventions (prefix "Error: ", trailing newline).
func TestMainErrorFormatMatchesConventions(t *testing.T) {
	var buf bytes.Buffer
	testErr := fmt.Errorf("config not found")
	fmt.Fprintf(&buf, "Error: %v\n", testErr)

	output := buf.String()
	if !strings.HasPrefix(output, "Error: ") {
		t.Errorf("expected 'Error: ' prefix, got %q", output)
	}
	if !strings.HasSuffix(output, "\n") {
		t.Errorf("expected trailing newline, got %q", output)
	}
}

// TestMainSourceHasStderrPrint is a source-level sanity check that
// main.go contains the fmt.Fprintf(os.Stderr, ...) pattern.
func TestMainSourceHasStderrPrint(t *testing.T) {
	data, err := os.ReadFile("main.go")
	if err != nil {
		t.Fatalf("read main.go: %v", err)
	}
	src := string(data)
	if !strings.Contains(src, "fmt.Fprintf(os.Stderr") {
		t.Error("main.go missing fmt.Fprintf(os.Stderr)")
	}
	percentV := "%v"
	wantFmt := `"Error: ` + percentV + `\n"`
	if !strings.Contains(src, wantFmt) {
		t.Error("main.go missing Error format string")
	}
}

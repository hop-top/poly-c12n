package main

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"

	"github.com/spf13/cobra"

	"hop.top/kit/go/ai/toolspec"
)

// ---------------------------------------------------------------------------
// a) Solo-dev: c12n classify
// ---------------------------------------------------------------------------

func TestE2EClassifyFlagsComplete(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")
	if cmd == nil {
		t.Fatal("classify not found")
	}

	required := []string{"format", "signal", "min-confidence", "file", "stdin"}
	for _, name := range required {
		if cmd.Flags().Lookup(name) == nil {
			t.Errorf("classify missing flag --%s", name)
		}
	}
}

func TestE2EClassifyFormatFlag(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")

	f := cmd.Flags().Lookup("format")
	if f == nil {
		t.Fatal("--format flag not found")
	}
	if f.DefValue != "json" {
		t.Errorf("expected default format=json, got %q", f.DefValue)
	}
	if f.Shorthand != "f" {
		t.Errorf("expected shorthand -f, got %q", f.Shorthand)
	}
}

func TestE2EClassifyStdinFlag(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")

	f := cmd.Flags().Lookup("stdin")
	if f == nil {
		t.Fatal("--stdin flag not found")
	}
	if f.DefValue != "false" {
		t.Errorf("expected default stdin=false, got %q", f.DefValue)
	}
}

func TestE2EClassifyUsage(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")
	if cmd.Use != "classify [text]" {
		t.Errorf("unexpected Use: %q", cmd.Use)
	}
}

func TestE2EClassifyLongMentionsPipeline(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")
	if !strings.Contains(cmd.Long, "pipeline") {
		t.Error("Long help should mention pipeline")
	}
}

func TestE2EClassifyLongMentionsStdin(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")
	if !strings.Contains(cmd.Long, "stdin") {
		t.Error("Long help should mention stdin input method")
	}
}

func TestE2EClassifyLongMentionsFile(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")
	if !strings.Contains(cmd.Long, "file") {
		t.Error("Long help should mention --file input method")
	}
}

func TestE2EClassifySignalFlagCompletion(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")

	f := cmd.Flags().Lookup("signal")
	if f == nil {
		t.Fatal("--signal flag not found")
	}
	if f.Shorthand != "s" {
		t.Errorf("expected shorthand -s, got %q", f.Shorthand)
	}
}

// ---------------------------------------------------------------------------
// b) Platform-eng: c12n doctor
// ---------------------------------------------------------------------------

func TestE2EDoctorStructure(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "doctor")
	if cmd == nil {
		t.Fatal("doctor not found")
	}
	if cmd.Short == "" {
		t.Error("doctor has no Short description")
	}
	if cmd.Use != "doctor" {
		t.Errorf("unexpected Use: %q", cmd.Use)
	}
}

// ---------------------------------------------------------------------------
// c) Researcher: c12n bench
// ---------------------------------------------------------------------------

func TestE2EBenchIterationsFlag(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "bench")
	if cmd == nil {
		t.Fatal("bench not found")
	}

	f := cmd.Flags().Lookup("iterations")
	if f == nil {
		t.Fatal("--iterations flag not found")
	}
	if f.Shorthand != "n" {
		t.Errorf("expected shorthand -n, got %q", f.Shorthand)
	}
}

func TestE2EBenchAllFlags(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "bench")

	cases := []struct {
		name      string
		shorthand string
	}{
		{"iterations", "n"},
		{"text", "t"},
		{"input", ""},
		{"signal", "s"},
		{"concurrency", "c"},
		{"output", "o"},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			f := cmd.Flags().Lookup(tc.name)
			if f == nil {
				t.Fatalf("missing flag --%s", tc.name)
			}
			if tc.shorthand != "" && f.Shorthand != tc.shorthand {
				t.Errorf("expected shorthand -%s, got %q",
					tc.shorthand, f.Shorthand)
			}
		})
	}
}

func TestE2EBenchLongMentionsIterations(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "bench")
	if !strings.Contains(cmd.Long, "iterations") {
		t.Error("bench Long should mention iterations")
	}
}

// ---------------------------------------------------------------------------
// d) Agent: c12n toolspec
// ---------------------------------------------------------------------------

func TestE2EToolspecValidJSON(t *testing.T) {
	root := newTestRoot()
	var buf bytes.Buffer
	root.SetOut(&buf)
	root.SetArgs([]string{"toolspec"})

	if err := root.Execute(); err != nil {
		t.Fatalf("toolspec execute: %v", err)
	}

	var spec toolspec.ToolSpec
	if err := json.Unmarshal(buf.Bytes(), &spec); err != nil {
		t.Fatalf("invalid JSON: %v", err)
	}
}

func TestE2EToolspecContainsAllCommands(t *testing.T) {
	spec := buildToolSpec()

	expected := []string{
		"classify", "config", "init", "signals",
		"bench", "upgrade", "doctor", "tip",
	}
	for _, name := range expected {
		if spec.FindCommand(name) == nil {
			t.Errorf("toolspec missing command %q", name)
		}
	}
}

func TestE2EToolspecHasErrorPatterns(t *testing.T) {
	spec := buildToolSpec()
	if len(spec.ErrorPatterns) == 0 {
		t.Fatal("no error patterns in toolspec")
	}

	// Verify key error patterns for automated recovery
	patterns := make(map[string]bool)
	for _, ep := range spec.ErrorPatterns {
		patterns[ep.Pattern] = true
	}

	critical := []string{
		"pipeline not available",
		"no input: provide text as args, --file, or --stdin",
		"unknown signal type",
	}
	for _, p := range critical {
		if !patterns[p] {
			t.Errorf("missing error pattern: %q", p)
		}
	}
}

func TestE2EToolspecErrorPatternsHaveFix(t *testing.T) {
	spec := buildToolSpec()
	for _, ep := range spec.ErrorPatterns {
		if ep.Fix == "" {
			t.Errorf("error pattern %q has no Fix", ep.Pattern)
		}
		if ep.Cause == "" {
			t.Errorf("error pattern %q has no Cause", ep.Pattern)
		}
	}
}

func TestE2EToolspecHasWorkflows(t *testing.T) {
	spec := buildToolSpec()
	if len(spec.Workflows) == 0 {
		t.Fatal("no workflows in toolspec")
	}

	expected := map[string]bool{
		"quick-classify":    false,
		"full-setup":        false,
		"benchmark-compare": false,
	}
	for _, w := range spec.Workflows {
		if _, ok := expected[w.Name]; ok {
			expected[w.Name] = true
		}
	}
	for name, found := range expected {
		if !found {
			t.Errorf("missing workflow %q", name)
		}
	}
}

func TestE2EToolspecWorkflowsHaveSteps(t *testing.T) {
	spec := buildToolSpec()
	for _, w := range spec.Workflows {
		if len(w.Steps) == 0 {
			t.Errorf("workflow %q has no steps", w.Name)
		}
	}
}

func TestE2EToolspecStateIntrospection(t *testing.T) {
	spec := buildToolSpec()
	if spec.StateIntrospection == nil {
		t.Fatal("state introspection is nil")
	}
	if len(spec.StateIntrospection.ConfigCommands) == 0 {
		t.Error("no config commands in state introspection")
	}
	if len(spec.StateIntrospection.EnvVars) == 0 {
		t.Error("no env vars in state introspection")
	}
}

// ---------------------------------------------------------------------------
// e) Solo-dev: c12n init
// ---------------------------------------------------------------------------

func TestE2EInitFlags(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "init")
	if cmd == nil {
		t.Fatal("init not found")
	}

	cases := []struct {
		name     string
		flagType string
	}{
		{"dry-run", "bool"},
		{"answers-file", "string"},
		{"scope", "string"},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			f := cmd.Flags().Lookup(tc.name)
			if f == nil {
				t.Fatalf("missing flag --%s on init", tc.name)
			}
			if f.Value.Type() != tc.flagType {
				t.Errorf("expected type %s, got %s",
					tc.flagType, f.Value.Type())
			}
		})
	}
}

// ---------------------------------------------------------------------------
// f) Platform-eng: c12n config
// ---------------------------------------------------------------------------

func TestE2EConfigSubcommands(t *testing.T) {
	root := newTestRoot()

	cases := []struct {
		path []string
		use  string
	}{
		{[]string{"config"}, "config"},
		{[]string{"config", "get"}, "get <key>"},
		{[]string{"config", "set"}, "set <key> <value>"},
		{[]string{"config", "list"}, "list"},
	}

	for _, tc := range cases {
		name := strings.Join(tc.path, " ")
		t.Run(name, func(t *testing.T) {
			cmd := findCmd(root, tc.path...)
			if cmd == nil {
				t.Fatalf("command %q not found", name)
			}
			if cmd.Use != tc.use {
				t.Errorf("expected Use=%q, got %q", tc.use, cmd.Use)
			}
		})
	}
}

func TestE2EConfigSetScopeFlag(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "config", "set")
	if cmd == nil {
		t.Fatal("config set not found")
	}
	f := cmd.Flags().Lookup("scope")
	if f == nil {
		t.Fatal("missing --scope flag on config set")
	}
	if f.Value.Type() != "string" {
		t.Errorf("expected string type, got %s", f.Value.Type())
	}
}

func TestE2EConfigGetKeyCompletion(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "config", "get")
	if cmd == nil {
		t.Fatal("config get not found")
	}
	if cmd.ValidArgsFunction == nil {
		t.Error("config get has no ValidArgsFunction; key completion not wired")
	}
}

func TestE2EConfigSetKeyCompletion(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "config", "set")
	if cmd == nil {
		t.Fatal("config set not found")
	}
	if cmd.ValidArgsFunction == nil {
		t.Error("config set has no ValidArgsFunction; key completion not wired")
	}
	// First arg should complete config keys
	completions, directive := cmd.ValidArgsFunction(
		cmd, []string{}, "",
	)
	if directive != cobra.ShellCompDirectiveNoFileComp {
		t.Errorf("expected ShellCompDirectiveNoFileComp, got %v", directive)
	}
	if len(completions) == 0 {
		t.Error("expected config key completions, got none")
	}
}

// ---------------------------------------------------------------------------
// Cross-cutting: command tree integrity
// ---------------------------------------------------------------------------

func TestE2EAllCommandsHaveShortDesc(t *testing.T) {
	root := newTestRoot()

	var walk func(*cobra.Command)
	walk = func(cmd *cobra.Command) {
		skip := cmd.Name() == "c12n" ||
			cmd.Name() == "help" ||
			cmd.Name() == "completion"
		if !skip && cmd.Short == "" {
			t.Errorf("command %q has no Short description", cmd.Name())
		}
		for _, c := range cmd.Commands() {
			walk(c)
		}
	}
	walk(root)
}

func TestE2EToolspecNameMatchesBinary(t *testing.T) {
	spec := buildToolSpec()
	if spec.Name != "c12n" {
		t.Errorf("expected name=c12n, got %q", spec.Name)
	}
}

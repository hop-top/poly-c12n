package main

import (
	"bytes"
	"encoding/json"
	"strings"
	"testing"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
	"hop.top/kit/go/ai/toolspec"
)

// newTestRoot builds the root command tree without executing PersistentPreRunE
// (which requires config/pipeline). Used for structural assertions.
func newTestRoot() *cobra.Command {
	root := &cobra.Command{Use: "c12n"}
	root.AddCommand(benchCmd())
	root.AddCommand(classifyCmd())
	root.AddCommand(configCmd())
	root.AddCommand(initCmd())
	root.AddCommand(signalsCmd())
	root.AddCommand(upgradeCmd())
	root.AddCommand(doctorCmd())
	root.AddCommand(tipCmd())
	root.AddCommand(toolspecCmd())
	registerCompletions(root)
	return root
}

func findCmd(root *cobra.Command, path ...string) *cobra.Command {
	cmd := root
	for _, name := range path {
		var found *cobra.Command
		for _, c := range cmd.Commands() {
			if c.Name() == name {
				found = c
				break
			}
		}
		if found == nil {
			return nil
		}
		cmd = found
	}
	return cmd
}

func TestRootBuilds(t *testing.T) {
	root := newTestRoot()
	if root == nil {
		t.Fatal("root command is nil")
	}
	if root.Use != "c12n" {
		t.Fatalf("unexpected Use: %s", root.Use)
	}
}

func TestSubcommandsExist(t *testing.T) {
	root := newTestRoot()

	cases := []struct {
		path []string
		use  string
	}{
		{[]string{"classify"}, "classify [text]"},
		{[]string{"config"}, "config"},
		{[]string{"config", "get"}, "get <key>"},
		{[]string{"config", "set"}, "set <key> <value>"},
		{[]string{"config", "list"}, "list"},
		{[]string{"init"}, "init"},
		{[]string{"signals"}, "signals"},
		{[]string{"signals", "inspect"}, "inspect <signal>"},
		{[]string{"bench"}, "bench"},
		{[]string{"upgrade"}, "upgrade"},
		{[]string{"doctor"}, "doctor"},
		{[]string{"tip"}, "tip"},
		{[]string{"tip", "suggest"}, "suggest"},
		{[]string{"toolspec"}, "toolspec"},
	}

	for _, tc := range cases {
		name := strings.Join(tc.path, " ")
		t.Run(name, func(t *testing.T) {
			cmd := findCmd(root, tc.path...)
			if cmd == nil {
				t.Fatalf("command %q not found", name)
			}
			if cmd.Use != tc.use {
				t.Fatalf("expected Use=%q, got %q", tc.use, cmd.Use)
			}
		})
	}
}

func TestSubcommandsHaveShortDesc(t *testing.T) {
	root := newTestRoot()

	var walk func(*cobra.Command)
	walk = func(cmd *cobra.Command) {
		if cmd.Name() == "c12n" || cmd.Name() == "help" || cmd.Name() == "completion" {
			// skip root and built-in helpers
		} else if cmd.Short == "" {
			t.Errorf("command %q has no Short description", cmd.Name())
		}
		for _, c := range cmd.Commands() {
			walk(c)
		}
	}
	walk(root)
}

func TestClassifyFlags(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")
	if cmd == nil {
		t.Fatal("classify not found")
	}

	for _, name := range []string{"format", "signal", "min-confidence", "file", "stdin"} {
		if cmd.Flags().Lookup(name) == nil {
			t.Errorf("missing flag --%s", name)
		}
	}
}

func TestBenchFlags(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "bench")
	if cmd == nil {
		t.Fatal("bench not found")
	}

	for _, name := range []string{"iterations", "text", "input", "signal", "concurrency", "output"} {
		if cmd.Flags().Lookup(name) == nil {
			t.Errorf("missing flag --%s", name)
		}
	}
}

func TestUpgradeFlags(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "upgrade")
	if cmd == nil {
		t.Fatal("upgrade not found")
	}
	if cmd.Flags().Lookup("check") == nil {
		t.Error("missing flag --check")
	}
}

func TestDoctorExists(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "doctor")
	if cmd == nil {
		t.Fatal("doctor not found")
	}
	if cmd.Short == "" {
		t.Error("doctor has no Short description")
	}
}

func TestToolspecOutputJSON(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "toolspec")
	if cmd == nil {
		t.Fatal("toolspec not found")
	}

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

	if spec.Name != "c12n" {
		t.Errorf("expected name=c12n, got %q", spec.Name)
	}
	if len(spec.Commands) == 0 {
		t.Error("no commands in toolspec")
	}
	if len(spec.ErrorPatterns) == 0 {
		t.Error("no error patterns in toolspec")
	}
	if len(spec.Workflows) == 0 {
		t.Error("no workflows in toolspec")
	}
	if spec.StateIntrospection == nil {
		t.Error("no state introspection in toolspec")
	}

	// Verify expected commands are present.
	expected := []string{"classify", "config", "init", "signals", "bench", "upgrade", "doctor", "tip"}
	for _, name := range expected {
		if spec.FindCommand(name) == nil {
			t.Errorf("toolspec missing command %q", name)
		}
	}
}

func TestToolspecWorkflows(t *testing.T) {
	spec := buildToolSpec()

	names := map[string]bool{
		"quick-classify":    false,
		"full-setup":        false,
		"benchmark-compare": false,
	}

	for _, w := range spec.Workflows {
		if _, ok := names[w.Name]; ok {
			names[w.Name] = true
		}
		if len(w.Steps) == 0 {
			t.Errorf("workflow %q has no steps", w.Name)
		}
	}

	for name, found := range names {
		if !found {
			t.Errorf("missing workflow %q", name)
		}
	}
}

func TestToolspecStateIntrospection(t *testing.T) {
	spec := buildToolSpec()

	if spec.StateIntrospection == nil {
		t.Fatal("state introspection is nil")
	}
	if len(spec.StateIntrospection.ConfigCommands) == 0 {
		t.Error("no config commands")
	}
	if len(spec.StateIntrospection.EnvVars) == 0 {
		t.Error("no env vars")
	}

	hasC12N := false
	hasCGO := false
	for _, v := range spec.StateIntrospection.EnvVars {
		if v == "C12N_CONFIG" {
			hasC12N = true
		}
		if v == "CGO_ENABLED" {
			hasCGO = true
		}
	}
	if !hasC12N {
		t.Error("missing C12N_CONFIG env var")
	}
	if !hasCGO {
		t.Error("missing CGO_ENABLED env var")
	}
}

func TestCompletionRegistrationNoPanic(t *testing.T) {
	// Ensure registerCompletions doesn't panic on a fully wired tree.
	defer func() {
		if r := recover(); r != nil {
			t.Fatalf("registerCompletions panicked: %v", r)
		}
	}()

	root := newTestRoot()
	_ = root
}

func TestConfigSetScopeFlag(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "config", "set")
	if cmd == nil {
		t.Fatal("config set not found")
	}
	if cmd.Flags().Lookup("scope") == nil {
		t.Error("missing --scope flag on config set")
	}
}

func TestInitFlags(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "init")
	if cmd == nil {
		t.Fatal("init not found")
	}
	for _, name := range []string{"dry-run", "answers-file", "scope"} {
		if cmd.Flags().Lookup(name) == nil {
			t.Errorf("missing flag --%s on init", name)
		}
	}
}

func TestSignalsEnabledFlag(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "signals")
	if cmd == nil {
		t.Fatal("signals not found")
	}
	if cmd.Flags().Lookup("enabled") == nil {
		t.Error("missing --enabled flag on signals")
	}
}

func TestTipSuggestHelp(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "tip", "suggest")
	if cmd == nil {
		t.Fatal("tip suggest not found")
	}
	if !strings.Contains(cmd.Long, "stdin") {
		t.Error("tip suggest Long help should mention stdin")
	}
}

func TestClassifyHelpContent(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "classify")
	if cmd == nil {
		t.Fatal("classify not found")
	}
	if !strings.Contains(cmd.Long, "pipeline") {
		t.Error("classify Long should mention pipeline")
	}
}

func TestBenchHelpContent(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "bench")
	if cmd == nil {
		t.Fatal("bench not found")
	}
	if !strings.Contains(cmd.Long, "iterations") {
		t.Error("bench Long should mention iterations")
	}
}

// Regression: "config set" must have key completion wired (PR #4 comment #6).
// configSetCmd already wires its own ValidArgsFunction that handles both key
// and value completion. Verify it is present and functional.
func TestConfigSetKeyCompletion(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "config", "set")
	if cmd == nil {
		t.Fatal("config set not found")
	}
	if cmd.ValidArgsFunction == nil {
		t.Error("config set has no ValidArgsFunction; key completion not wired")
	}
	// Verify the first argument completes config keys (not empty).
	_, directive := cmd.ValidArgsFunction(cmd, []string{}, "")
	if directive != cobra.ShellCompDirectiveNoFileComp {
		t.Errorf("expected ShellCompDirectiveNoFileComp, got %v", directive)
	}
}

// Regression: "config get" key completion should also be wired.
func TestConfigGetKeyCompletion(t *testing.T) {
	root := newTestRoot()
	cmd := findCmd(root, "config", "get")
	if cmd == nil {
		t.Fatal("config get not found")
	}
	if cmd.ValidArgsFunction == nil {
		t.Error("config get has no ValidArgsFunction; key completion not wired")
	}
}

// Regression: PII description must read "Personally identifiable" not
// "Personal identifiable" (PR #4 comment #8).
func TestPIIDescription(t *testing.T) {
	desc, ok := signalDescriptions[c12n.SignalPII]
	if !ok {
		t.Fatal("no description for SignalPII")
	}
	if !strings.Contains(desc, "Personally") {
		t.Errorf("SignalPII description should contain 'Personally', got %q", desc)
	}
	if strings.Contains(desc, "Personal ident") && !strings.Contains(desc, "Personally") {
		t.Errorf("SignalPII description has 'Personal' instead of 'Personally': %q", desc)
	}
}

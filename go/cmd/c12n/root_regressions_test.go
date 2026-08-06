package main

import (
	"bytes"
	"context"
	"strings"
	"testing"
)

// TestNewRootConstructs is the guard against duplicate global-flag
// registration.
//
// The rest of the CLI suite builds its tree with newTestRoot(), a bare
// &cobra.Command that never calls cli.New. That made every structural
// assertion pass while the shipped binary panicked on startup with
// "c12n flag redefined: config" — the local --config registration
// collided with the one kit's cli.New already owns.
//
// This test calls the SAME constructor main() uses, so any flag or
// command the local code registers on top of a kit-provided one fails
// here instead of in the user's terminal.
func TestNewRootConstructs(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}
	if root == nil || root.Cmd == nil {
		t.Fatal("newRoot returned nil root")
	}
	if got := root.Cmd.Name(); got != "c12n" {
		t.Fatalf("root name = %q, want c12n", got)
	}
}

// TestNewRootConfigFlagIsKitOwned asserts the --config flag on the real
// root is kit's repeatable -c/--config (a stringArray with the "c"
// shorthand), not a locally re-registered plain string flag. A local
// StringVar registration would panic in newRoot; a rename would show up
// here as a missing shorthand or the wrong type.
func TestNewRootConfigFlagIsKitOwned(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	flag := root.Cmd.PersistentFlags().Lookup("config")
	if flag == nil {
		t.Fatal("--config not registered on root")
	}
	if flag.Shorthand != "c" {
		t.Errorf("--config shorthand = %q, want c (kit-owned flag)", flag.Shorthand)
	}
	if got := flag.Value.Type(); got != "stringArray" {
		t.Errorf("--config type = %q, want stringArray (kit-owned flag)", got)
	}
}

// TestRealRootExecutesHelp runs --help through the real root. This is the
// exact invocation that panicked before the fix: cli.New registers the
// global flag suite, and the duplicate registration blew up during
// construction, long before cobra ever rendered help.
func TestRealRootExecutesHelp(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	var out bytes.Buffer
	root.Cmd.SetOut(&out)
	root.Cmd.SetErr(&out)
	root.Cmd.SetArgs([]string{"--help"})

	if err := root.Cmd.ExecuteContext(context.Background()); err != nil {
		t.Fatalf("execute --help: %v", err)
	}

	got := out.String()
	if !strings.Contains(got, "c12n") {
		t.Errorf("help output missing tool name:\n%s", got)
	}
	for _, sub := range []string{"bench", "classify", "config", "doctor"} {
		if !strings.Contains(got, sub) {
			t.Errorf("help output missing subcommand %q:\n%s", sub, got)
		}
	}
}

// TestRealRootExecutesSubcommandHelp exercises a subcommand through the
// real root so the kit-wired command tree — not just the bare cobra tree
// newTestRoot builds — is proven reachable.
func TestRealRootExecutesSubcommandHelp(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	var out bytes.Buffer
	root.Cmd.SetOut(&out)
	root.Cmd.SetErr(&out)
	root.Cmd.SetArgs([]string{"bench", "--help"})

	if err := root.Cmd.ExecuteContext(context.Background()); err != nil {
		t.Fatalf("execute bench --help: %v", err)
	}

	got := out.String()
	if !strings.Contains(got, "--iterations") {
		t.Errorf("bench help missing --iterations flag:\n%s", got)
	}
}

// TestNewRootSetsLayeredConfigPaths pins the config layering wired in
// newRoot: system, user, and project slots must all be populated.
// SystemConfigPath was previously left empty, which made
// `config set --scope system` fail with "config: scope path is empty".
func TestNewRootSetsLayeredConfigPaths(t *testing.T) {
	if _, err := newRoot(); err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	opts := rootConfigOptions()
	if opts.SystemConfigPath == "" {
		t.Error("SystemConfigPath is empty: --scope system cannot resolve a path")
	}
	if opts.UserConfigPath == "" {
		t.Error("UserConfigPath is empty")
	}
	if opts.ProjectConfigPath == "" {
		t.Error("ProjectConfigPath is empty")
	}
	if opts.EnvPrefix != "C12N" {
		t.Errorf("EnvPrefix = %q, want C12N", opts.EnvPrefix)
	}
}

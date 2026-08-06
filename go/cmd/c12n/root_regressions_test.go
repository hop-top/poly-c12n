package main

import (
	"bytes"
	"context"
	"strings"
	"testing"

	"github.com/spf13/cobra"

	"hop.top/kit/go/console/cli"
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

// TestRealRootPassesKitValidation is the guard that would have caught
// the CLI shipping in a state where it could not start at all.
//
// kit's cli.New sets EnforceValidate=true by default, and Root.Execute
// runs Root.Validate() as a pre-flight. When that validation fails the
// binary exits 2 having printed nothing but the validation error — no
// help, no subcommand, nothing. Every invocation is dead.
//
// This is exactly what happened: the codebase carried ZERO kit
// annotations, and the whole CLI exited 2 on `--help`. It went
// unnoticed because the rest of the suite builds its tree with
// newTestRoot(), a bare &cobra.Command that never calls cli.New and so
// never runs the validator. Green tests, dead binary.
//
// Calling Validate() directly (rather than asserting on --help output)
// makes the failure message enumerate every offending command, so a
// regression names the command that lost its annotation instead of just
// reporting a non-zero exit.
func TestRealRootPassesKitValidation(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}
	if err := root.Validate(); err != nil {
		t.Fatalf("kit cli validation failed — the binary cannot start:\n%v", err)
	}
}

// TestEveryLeafCarriesSideEffectAndIdempotency pins the annotations
// per-command rather than relying solely on Validate().
//
// Validate() proves the annotations are PRESENT. This proves each one
// still says what we decided it says. These values tell agents and
// tooling whether a command is safe to run unattended, so a silent
// downgrade — upgrade going from destructive-local to read, say — is a
// real safety regression, not a cosmetic one. Validate() would happily
// accept that change; this test will not.
func TestEveryLeafCarriesSideEffectAndIdempotency(t *testing.T) {
	want := map[string]struct {
		sideEffect cli.SideEffect
		idempotent cli.Idempotency
	}{
		// Read-only: evaluate, inspect, or serialize. Nothing persists.
		"c12n classify":        {cli.SideEffectRead, cli.IdempotencyYes},
		"c12n doctor":          {cli.SideEffectRead, cli.IdempotencyYes},
		"c12n toolspec":        {cli.SideEffectRead, cli.IdempotencyYes},
		"c12n config get":      {cli.SideEffectRead, cli.IdempotencyYes},
		"c12n config list":     {cli.SideEffectRead, cli.IdempotencyYes},
		"c12n signals inspect": {cli.SideEffectRead, cli.IdempotencyYes},
		"c12n tip suggest":     {cli.SideEffectRead, cli.IdempotencyYes},

		// bench truncates the --output path via os.Create. Local scope.
		"c12n bench": {cli.SideEffectWriteLocal, cli.IdempotencyYes},

		// init and config set both accept --scope system, which writes
		// /etc/c12n/config.yaml — shared, so unscoped "write", not
		// write-local.
		"c12n init":       {cli.SideEffectWrite, cli.IdempotencyYes},
		"c12n config set": {cli.SideEffectWrite, cli.IdempotencyYes},

		// upgrade overwrites the running binary with no backup and no
		// c12n-side rollback. Irreversible => destructive band. Not
		// idempotent: the second run is a no-op against a mutated
		// target.
		"c12n upgrade": {cli.SideEffectDestructiveLocal, cli.IdempotencyNo},
	}

	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	seen := map[string]bool{}
	walkCommands(root.Cmd, func(cmd *cobra.Command) {
		if cmd.HasSubCommands() || !cmd.Runnable() {
			return
		}
		path := cmd.CommandPath()
		exp, tracked := want[path]
		if !tracked {
			// Not an assertion failure by itself — kit ships `status`
			// and the completion leaves, which kit annotates.
			return
		}
		seen[path] = true

		got, ok := cli.GetSideEffect(cmd)
		if !ok {
			t.Errorf("%s: kit/side-effect annotation missing", path)
		} else if got != exp.sideEffect {
			t.Errorf("%s: kit/side-effect = %q, want %q",
				path, got, exp.sideEffect)
		}

		gotIdem, ok := cli.GetIdempotency(cmd)
		if !ok {
			t.Errorf("%s: kit/idempotent annotation missing", path)
		} else if gotIdem != exp.idempotent {
			t.Errorf("%s: kit/idempotent = %q, want %q",
				path, gotIdem, exp.idempotent)
		}
	})

	for path := range want {
		if !seen[path] {
			t.Errorf("%s: expected leaf command not found in the tree "+
				"(renamed or unmounted?)", path)
		}
	}
}

// TestNoLeafIsSilentlyDowngradedToRead guards the specific direction of
// regression that actually hurts.
//
// A mutating command mislabelled `read` tells an agent it is safe to
// run unattended, with no confirmation and no policy gate. That is the
// hazardous direction; the reverse (a read command labelled write) only
// costs an unnecessary prompt. This test therefore hard-codes the set
// of commands known to touch state and asserts none of them ever
// reports itself as read-only.
func TestNoLeafIsSilentlyDowngradedToRead(t *testing.T) {
	mutating := []string{
		"c12n bench", // truncates --output
		"c12n init",  // writes a config file
		"c12n config set",
		"c12n upgrade", // overwrites the binary
	}

	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	byPath := map[string]*cobra.Command{}
	walkCommands(root.Cmd, func(cmd *cobra.Command) {
		byPath[cmd.CommandPath()] = cmd
	})

	for _, path := range mutating {
		cmd, ok := byPath[path]
		if !ok {
			t.Errorf("%s: command not found", path)
			continue
		}
		if cli.IsReadOnly(cmd) {
			t.Errorf("%s is annotated read-only, but it mutates state. "+
				"An agent will run this unattended.", path)
		}
		if !cli.IsMutating(cmd) {
			se, _ := cli.GetSideEffect(cmd)
			t.Errorf("%s: kit/side-effect = %q is neither write nor "+
				"destructive, but the command mutates state", path, se)
		}
	}
}

// TestReservedStatusSubcommandIsMounted pins the reserved `status`
// subcommand kit's validator requires on the root. Losing it puts the
// binary straight back to exit 2 on every invocation.
func TestReservedStatusSubcommandIsMounted(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}
	for _, c := range root.Cmd.Commands() {
		if c.Name() == "status" {
			return
		}
	}
	t.Fatal("reserved 'status' subcommand not mounted on root")
}

// TestStatusRunsInThisBuildMode proves `c12n status` actually executes
// and reports the engine section, in WHICHEVER build mode the test
// binary was compiled for.
//
// The same assertions run for the stub build and the -tags c12n_native
// build; only the expected build_mode string differs. In a stub build
// status must still succeed and say so plainly rather than failing with
// the engine — status is most valuable exactly when the engine is
// broken.
func TestStatusRunsInThisBuildMode(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	var out bytes.Buffer
	root.Cmd.SetOut(&out)
	root.Cmd.SetErr(&out)
	root.Cmd.SetArgs([]string{"status", "--format", "json"})

	if err := root.Cmd.ExecuteContext(context.Background()); err != nil {
		t.Fatalf("execute status: %v\noutput:\n%s", err, out.String())
	}

	got := out.String()
	wantMode := "stub"
	if nativeBuild {
		wantMode = "c12n_native"
	}
	if !strings.Contains(got, `"build_mode": "`+wantMode+`"`) {
		t.Errorf("status did not report build_mode %q:\n%s", wantMode, got)
	}
	for _, section := range []string{"engine", "config-paths", "signals"} {
		if !strings.Contains(got, `"title": "`+section+`"`) {
			t.Errorf("status missing %q section:\n%s", section, got)
		}
	}
	if nativeBuild && !strings.Contains(got, `"pipeline_loadable": true`) {
		t.Errorf("native build should load a pipeline:\n%s", got)
	}
}

// TestStatusPrintsWithoutExplicitFormat guards the default-format
// regression: c12n's root defaults --format to table, and kit's table
// renderer cannot flatten the nested StatusOutput, so bare
// `c12n status` exited 0 having printed zero bytes. Silent success is
// worse than an error — the user has no signal anything went wrong.
func TestStatusPrintsWithoutExplicitFormat(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	var out bytes.Buffer
	root.Cmd.SetOut(&out)
	root.Cmd.SetErr(&out)
	root.Cmd.SetArgs([]string{"status"})

	if err := root.Cmd.ExecuteContext(context.Background()); err != nil {
		t.Fatalf("execute status: %v", err)
	}

	if strings.TrimSpace(out.String()) == "" {
		t.Fatal("bare `c12n status` printed nothing")
	}
	if !strings.Contains(out.String(), "engine") {
		t.Errorf("bare `c12n status` missing engine section:\n%s", out.String())
	}
}

// TestReadOnlyCommandsRunWithoutAPipeline is the regression guard for
// the stub-build lockout.
//
// PersistentPreRunE used to treat pipeline construction as a hard
// precondition for EVERY command. In a stub build NewPipeline always
// errors, so `doctor`, `status`, `toolspec` and `signals` all exited 1
// — including doctor, whose entire purpose is to diagnose a missing
// native engine, and status, which must report the engine as
// unavailable rather than die alongside it.
//
// In a native build the pipeline constructs fine and this test simply
// confirms the same commands still work.
func TestReadOnlyCommandsRunWithoutAPipeline(t *testing.T) {
	for _, args := range [][]string{
		{"doctor"},
		{"toolspec"},
		{"signals"},
		{"config", "list"},
	} {
		name := strings.Join(args, " ")
		t.Run(name, func(t *testing.T) {
			root, err := newRoot()
			if err != nil {
				t.Fatalf("newRoot: %v", err)
			}
			var out bytes.Buffer
			root.Cmd.SetOut(&out)
			root.Cmd.SetErr(&out)
			root.Cmd.SetArgs(args)

			if err := root.Cmd.ExecuteContext(context.Background()); err != nil {
				t.Fatalf("`c12n %s` failed with no pipeline available: %v\n%s",
					name, err, out.String())
			}
		})
	}
}

// TestPipelineCommandsReportWhyTheEngineIsMissing asserts that the
// commands which genuinely need an engine still fail — but with the
// underlying cause attached, not a bare "pipeline not available" that
// tells the user nothing actionable.
//
// Only meaningful in a stub build; in a native build the pipeline is
// present and there is nothing to report.
func TestPipelineCommandsReportWhyTheEngineIsMissing(t *testing.T) {
	if nativeBuild {
		t.Skip("native build: pipeline is available")
	}

	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}
	var out bytes.Buffer
	root.Cmd.SetOut(&out)
	root.Cmd.SetErr(&out)
	root.Cmd.SetArgs([]string{"classify", "hello"})

	err = root.Cmd.ExecuteContext(context.Background())
	if err == nil {
		t.Fatal("classify should fail without a pipeline")
	}
	if !strings.Contains(err.Error(), "native engine disabled") {
		t.Errorf("error should explain WHY the pipeline is missing, got: %v", err)
	}
}

// TestEveryRunnableCommandHasLongHelp pins the Long help text kit
// requires on every runnable command. Nine commands were missing it,
// which contributed to the binary refusing to start.
func TestEveryRunnableCommandHasLongHelp(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	walkCommands(root.Cmd, func(cmd *cobra.Command) {
		if cmd == root.Cmd || !cmd.Runnable() {
			return
		}
		if cmd.Name() == "completion" || cmd.Name() == "help" ||
			(cmd.Parent() != nil && cmd.Parent().Name() == "completion") {
			return
		}
		if strings.TrimSpace(cmd.Long) == "" {
			t.Errorf("%s: Long help is empty", cmd.CommandPath())
		}
		if cmd.Long == cmd.Short {
			t.Errorf("%s: Long is a copy of Short, not real help text",
				cmd.CommandPath())
		}
	})
}

// TestDepthOneLeavesAreMarkedTopLevelVerbs pins the
// kit/top-level-verb annotation on the seven depth-1 verbs. kit's shape
// validator rejects an unannotated depth-1 runnable leaf outright.
func TestDepthOneLeavesAreMarkedTopLevelVerbs(t *testing.T) {
	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	for _, want := range []string{
		"bench", "classify", "doctor", "init",
		"signals", "toolspec", "upgrade",
	} {
		var found *cobra.Command
		for _, c := range root.Cmd.Commands() {
			if c.Name() == want {
				found = c
				break
			}
		}
		if found == nil {
			t.Errorf("%s: depth-1 command not mounted", want)
			continue
		}
		if !cli.IsTopLevelVerb(found) {
			t.Errorf("c12n %s: missing kit/top-level-verb annotation", want)
		}
	}
}

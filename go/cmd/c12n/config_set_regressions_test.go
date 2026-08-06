package main

import (
	"bytes"
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

// runRealRoot executes args through the SAME constructor main() uses.
//
// The point is that nothing here is a stand-in: newRoot() wires the real
// PersistentPreRunE that loads config, so a config file this test writes
// through "config set" is loaded back by the real loader on the next
// invocation. A test built on a bare &cobra.Command would assert on a
// tree the shipped binary never runs and would have passed throughout
// the bug this file guards.
func runRealRoot(t *testing.T, args ...string) (string, error) {
	t.Helper()

	root, err := newRoot()
	if err != nil {
		t.Fatalf("newRoot: %v", err)
	}

	var out bytes.Buffer
	root.Cmd.SetOut(&out)
	root.Cmd.SetErr(&out)
	root.Cmd.SetArgs(args)

	execErr := root.Cmd.ExecuteContext(context.Background())
	return out.String(), execErr
}

// projectConfigDir isolates the project config layer.
//
// rootConfigOptions resolves ProjectConfigPath to the relative path
// ".c12n.yaml", so chdir'ing into a temp dir is what redirects writes
// away from the developer's own tree. XDG_CONFIG_HOME is redirected too
// so the user layer cannot leak a real ~/.config/c12n/config.yaml into
// the assertions.
func projectConfigDir(t *testing.T) string {
	t.Helper()

	dir := t.TempDir()
	t.Chdir(dir)
	t.Setenv("XDG_CONFIG_HOME", filepath.Join(dir, "xdg"))

	return dir
}

func readProjectConfig(t *testing.T, dir string) string {
	t.Helper()

	data, err := os.ReadFile(filepath.Join(dir, ".c12n.yaml"))
	if err != nil {
		t.Fatalf("read .c12n.yaml: %v", err)
	}
	return string(data)
}

// TestConfigSetWritesSchemaTypedScalars is the regression guard for
// "config set" poisoning every later invocation.
//
// kit's config.Set hard-codes Tag: "!!str" on the scalar it writes, so
// `c12n config set keyword_threshold 0.9` produced
//
//	keyword_threshold: "0.9"
//
// Config is loaded in PersistentPreRunE, so that quoted scalar failed to
// unmarshal into float64 and aborted EVERY subsequent command — doctor
// included — before it could run. Recovery meant hand-editing the YAML.
//
// String-valued keys were unaffected, which is why the bug hid: the
// obvious smoke test (set a string key, read it back) passes either way.
// This test therefore covers each schema type, and asserts on the bytes
// on disk rather than on a round-tripped value, because "config get"
// reports 0.9 for both the correct and the broken file.
func TestConfigSetWritesSchemaTypedScalars(t *testing.T) {
	cases := []struct {
		name string
		key  string
		val  string
		want string
	}{
		{"float", "keyword_threshold", "0.9", "keyword_threshold: 0.9"},
		{"int", "max_concurrency", "16", "max_concurrency: 16"},
		{"bool", "keyword_enabled", "false", "keyword_enabled: false"},
		{"enum", "keyword_strategy", "bm25", "keyword_strategy: bm25"},
		// A string-typed key must keep string semantics even when the
		// value looks like a number, so this one stays quoted.
		{
			"string_numeric_looking",
			"embedding_model_path",
			"0.9",
			`embedding_model_path: "0.9"`,
		},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := projectConfigDir(t)

			out, err := runRealRoot(t,
				"config", "set", tc.key, tc.val, "--scope", "project")
			if err != nil {
				t.Fatalf("config set %s=%s: %v\noutput:\n%s",
					tc.key, tc.val, err, out)
			}

			got := readProjectConfig(t, dir)
			if !strings.Contains(got, tc.want) {
				t.Fatalf("config file missing %q:\n%s", tc.want, got)
			}
		})
	}
}

// TestConfigSetNumericKeepsLaterCommandsRunnable reproduces the actual
// user-visible failure: it is not that the YAML looks wrong, it is that
// the next command dies.
//
// Before the fix, "config set keyword_threshold 0.9" followed by
// "doctor" failed with
//
//	cannot unmarshal !!str `0.9` into float64
//
// which is the diagnostic command itself refusing to run. Asserting only
// on file contents would let a future change that writes valid-looking
// YAML with the wrong type slip through, so this drives a second real
// root over the file the first one wrote.
func TestConfigSetNumericKeepsLaterCommandsRunnable(t *testing.T) {
	projectConfigDir(t)

	if out, err := runRealRoot(t,
		"config", "set", "keyword_threshold", "0.9", "--scope", "project",
	); err != nil {
		t.Fatalf("config set: %v\noutput:\n%s", err, out)
	}

	// Fresh root: PersistentPreRunE loads the file just written.
	out, err := runRealRoot(t, "doctor")
	if err != nil {
		t.Fatalf("doctor after numeric config set: %v\noutput:\n%s", err, out)
	}

	if strings.Contains(out, "cannot unmarshal") {
		t.Fatalf("doctor reported an unmarshal failure:\n%s", out)
	}
}

// TestConfigSetRejectsInvalidValues asserts a bad value is a clean error
// at set time.
//
// The command's help text has always claimed the value "is validated
// against c12n's config schema", but nothing called pkl.ValidateValue —
// so `config set max_concurrency abc` wrote the file and turned every
// later invocation into a parse error. The file-absence check matters as
// much as the error: a rejected set must leave no config behind to poison.
func TestConfigSetRejectsInvalidValues(t *testing.T) {
	cases := []struct {
		name    string
		key     string
		val     string
		wantMsg string
	}{
		{"float", "keyword_threshold", "abc", "expects float"},
		{"int", "max_concurrency", "1.5", "expects integer"},
		{"bool", "keyword_enabled", "maybe", "expects boolean"},
		{"enum", "keyword_strategy", "nope", "enum"},
		{"unknown_key", "no_such_key", "1", "unknown key"},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			dir := projectConfigDir(t)

			out, err := runRealRoot(t,
				"config", "set", tc.key, tc.val, "--scope", "project")
			if err == nil {
				t.Fatalf("config set %s=%s succeeded, want error\noutput:\n%s",
					tc.key, tc.val, out)
			}
			if !strings.Contains(err.Error(), tc.wantMsg) {
				t.Errorf("error = %q, want it to mention %q", err.Error(), tc.wantMsg)
			}

			if _, statErr := os.Stat(filepath.Join(dir, ".c12n.yaml")); statErr == nil {
				t.Errorf("rejected set still wrote a config file:\n%s",
					readProjectConfig(t, dir))
			}
		})
	}
}

// TestConfigSetPreservesUnrelatedKeys guards the repair step.
//
// Fixing the tag means re-reading and re-encoding the file after
// config.Set, so this asserts that pass does not drop or requote keys it
// was not asked to touch — including previously-written numeric keys,
// which must not silently revert to strings.
func TestConfigSetPreservesUnrelatedKeys(t *testing.T) {
	dir := projectConfigDir(t)

	sets := [][]string{
		{"keyword_threshold", "0.9"},
		{"max_concurrency", "16"},
		{"keyword_strategy", "bm25"},
		{"context_output_ratio", "2.5"},
	}
	for _, s := range sets {
		if out, err := runRealRoot(t,
			"config", "set", s[0], s[1], "--scope", "project",
		); err != nil {
			t.Fatalf("config set %s: %v\noutput:\n%s", s[0], err, out)
		}
	}

	got := readProjectConfig(t, dir)
	for _, want := range []string{
		"keyword_threshold: 0.9",
		"max_concurrency: 16",
		"keyword_strategy: bm25",
		"context_output_ratio: 2.5",
	} {
		if !strings.Contains(got, want) {
			t.Errorf("config file missing %q:\n%s", want, got)
		}
	}

	// And the accumulated file must still load.
	if out, err := runRealRoot(t, "doctor"); err != nil {
		t.Fatalf("doctor over accumulated config: %v\noutput:\n%s", err, out)
	}
}

package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
	"hop.top/kit/go/console/cli"
	"hop.top/kit/go/core/config/pkl"
)

func initCmd() *cobra.Command {
	var (
		dryRun      bool
		answersFile string
		scopeStr    string
	)

	cmd := &cobra.Command{
		Use:   "init",
		Short: "Initialize c12n configuration interactively",
		Long: `Walk through an interactive wizard and write a c12n config file.

The wizard is generated from c12n's embedded PKL schema, so the
questions it asks and the values it accepts always match the config keys
this version understands.

  c12n init                    write ./.c12n.yaml (project scope)
  c12n init --scope user       write to the XDG user config directory
  c12n init --dry-run          run the wizard, print the result, write nothing
  c12n init --answers-file a.yaml   answer non-interactively from a file

--answers-file makes the command usable in CI and by agents, where no
TTY is available to drive the prompts.

Caveats:

  - Writing to an existing config replaces it. Use --dry-run first if
    you are not certain the target file is disposable.
  - --scope system targets /etc/c12n/config.yaml, which affects every
    user on the machine and normally requires elevated privileges.
    Prefer project or user scope.
  - init only creates config. It does not download models; a config
    referencing a model path that does not exist will pass init and
    then show up as a FAIL in 'c12n doctor'.`,
		Args: cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			// Write embedded PKL to temp file for the wizard.
			tmp, err := os.CreateTemp("", "c12n-init-*.pkl")
			if err != nil {
				return fmt.Errorf("create temp file: %w", err)
			}
			defer os.Remove(tmp.Name())

			if _, err := tmp.WriteString(c12n.ConfigPklSource()); err != nil {
				tmp.Close()
				return fmt.Errorf("write pkl schema: %w", err)
			}
			tmp.Close()

			opts := ConfigOptsFromContext(cmd)
			scope, err := parseConfigScope(scopeStr)
			if err != nil {
				return err
			}

			inner := pkl.NewConfigCommand(tmp.Name(), pkl.CommandOpts{
				ConfigOpts: opts,
				Scope:      scope,
			})
			inner.SetIn(cmd.InOrStdin())
			inner.SetOut(cmd.OutOrStdout())
			inner.SetErr(cmd.ErrOrStderr())
			inner.SetContext(cmd.Context())

			if dryRun {
				_ = inner.Flags().Set("dry-run", "true")
			}
			if answersFile != "" {
				_ = inner.Flags().Set("answers-file", answersFile)
			}

			return inner.RunE(inner, nil)
		},
	}

	cmd.Flags().BoolVar(&dryRun, "dry-run", false,
		"Preview without writing config")
	cmd.Flags().StringVar(&answersFile, "answers-file", "",
		"Path to YAML answers file")
	cmd.Flags().StringVar(&scopeStr, "scope", "project",
		"Config scope (system|user|project)")

	_ = cmd.RegisterFlagCompletionFunc("scope",
		func(_ *cobra.Command, _ []string, _ string) ([]string, cobra.ShellCompDirective) {
			return []string{"system", "user", "project"},
				cobra.ShellCompDirectiveNoFileComp
		})

	// write, not write-local: --scope is caller-chosen and reaches
	// /etc/c12n/config.yaml, which is shared across every user on the
	// host. write-local would understate that. The unscoped legacy
	// "write" is the honest annotation for a command whose blast radius
	// is a runtime argument rather than a fixed property.
	//
	// Not destructive despite overwriting an existing config: config is
	// declarative and regenerable, and the wizard shows what it is about
	// to write. Losing a hand-tuned config is annoying, not the kind of
	// irreversible loss that puts upgrade in the destructive band.
	//
	// Idempotent: the wizard is driven to a fixed answer set (via
	// --answers-file, the only way an agent can run it at all without a
	// TTY), and writing the same answers twice yields the same file.
	cli.SetSideEffect(cmd, cli.SideEffectWrite)
	cli.SetIdempotency(cmd, cli.IdempotencyYes)
	cli.SetTopLevelVerb(cmd)

	return cmd
}

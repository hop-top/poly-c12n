package main

import (
	"fmt"
	"os"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
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
		Args:  cobra.NoArgs,
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

	return cmd
}

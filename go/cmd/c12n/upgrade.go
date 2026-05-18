package main

import (
	"github.com/spf13/cobra"

	"hop.top/kit/go/core/upgrade"
)

func upgradeCmd() *cobra.Command {
	var checkOnly bool

	cmd := &cobra.Command{
		Use:   "upgrade",
		Short: "Check for and install c12n updates",
		RunE: func(cmd *cobra.Command, _ []string) error {
			checker := upgrade.New(
				upgrade.WithBinary("c12n", version),
				upgrade.WithGitHub("hop-top/c12n"),
			)

			return upgrade.RunCLI(cmd.Context(), checker, upgrade.CLIOptions{
				AutoUpgrade: !checkOnly,
				Out:         cmd.OutOrStdout(),
			})
		},
	}

	cmd.Flags().BoolVar(&checkOnly, "check", false,
		"Check for updates without installing")

	return cmd
}

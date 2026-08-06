package main

import (
	"github.com/spf13/cobra"

	"hop.top/kit/go/console/cli"
	"hop.top/kit/go/core/upgrade"
)

func upgradeCmd() *cobra.Command {
	var checkOnly bool

	cmd := &cobra.Command{
		Use:   "upgrade",
		Short: "Check for and install c12n updates",
		Long: `Check GitHub for a newer c12n release and replace this binary with it.

By default the command both checks and installs: the running executable
is overwritten in place with the downloaded release. Pass --check to
query the latest version and print the result without touching disk.

  c12n upgrade --check    report available version, change nothing
  c12n upgrade            download and overwrite this binary

Caveats worth knowing before running this unattended:

  - The replacement is not reversible by c12n. There is no
    'c12n downgrade' and no backup of the previous binary. Rolling back
    means reinstalling the old version yourself.
  - It needs network access to the GitHub releases API, and write
    permission on the installed binary's path. A system-wide install
    (/usr/local/bin) will fail without elevated privileges.
  - If c12n was installed by a package manager, upgrading here puts the
    binary out of sync with what that manager believes is installed.
    Prefer the package manager in that case.`,
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

	// destructive-local, not write. The default (no --check) path
	// overwrites the running executable with no backup and no c12n-side
	// rollback: the previous binary is unrecoverable through this tool.
	// That is irreversible loss, which puts it in the destructive band
	// rather than the write band, even though the "loss" is a file the
	// user can reinstall.
	//
	// -local because the blast radius is this machine's installed
	// binary. Nothing shared or upstream is touched — contrast a
	// publish or a force-push, which would be destructive-shared.
	//
	// Deliberately annotated for the worst case rather than for
	// --check. An agent choosing whether to run this unattended must
	// see the destructive tier; a caller who only wants the safe half
	// passes --check explicitly.
	//
	// Not idempotent: the first run mutates the binary and the second
	// run against the new version is a no-op, so successive
	// invocations are not observably equivalent.
	cli.SetSideEffect(cmd, cli.SideEffectDestructiveLocal)
	cli.SetIdempotency(cmd, cli.IdempotencyNo)
	cli.SetTopLevelVerb(cmd)

	return cmd
}

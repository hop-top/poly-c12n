package main

import (
	"fmt"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
	"hop.top/kit/go/console/cli"
	"hop.top/kit/go/console/output"
	"hop.top/kit/go/core/config"
	"hop.top/kit/go/core/config/pkl"
)

func configCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "config",
		Short: "Manage c12n configuration",
		Args:  cobra.NoArgs,
	}

	cmd.AddCommand(
		configGetCmd(),
		configSetCmd(),
		configListCmd(),
	)

	return cmd
}

func configGetCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "get <key>",
		Short: "Get a config value",
		Long: `Print the effective value of a single config key.

"Effective" means the value c12n would actually use, after layering all
config sources in precedence order: built-in defaults, then the system
file (/etc/c12n/config.yaml), then the user file, then the project file
(./.c12n.yaml), then C12N_* environment variables, then any -c overrides
passed on the command line.

  c12n config get keyword.threshold

Only the resolved value is printed, with no indication of which layer it
came from. Use 'c12n config list' when you need to see the layer a value
originated in, or 'c12n doctor' when you need to know which files were
found at all. Unknown keys are an error rather than an empty result.`,
		Args: cobra.ExactArgs(1),
		RunE: func(cmd *cobra.Command, args []string) error {
			opts := ConfigOptsFromContext(cmd)
			val, err := config.Get(args[0], opts)
			if err != nil {
				return err
			}
			fmt.Fprintln(cmd.OutOrStdout(), val)
			return nil
		},
		ValidArgsFunction: completeKeys,
	}

	// Resolves and prints one key. No file is opened for writing.
	cli.SetSideEffect(cmd, cli.SideEffectRead)
	cli.SetIdempotency(cmd, cli.IdempotencyYes)

	return cmd
}

func configSetCmd() *cobra.Command {
	var scopeStr string

	cmd := &cobra.Command{
		Use:   "set <key> <value>",
		Short: "Set a config value",
		Long: `Write a single config key into one config layer.

--scope selects which file is edited; it defaults to project, so the key
lands in ./.c12n.yaml unless you say otherwise:

  c12n config set keyword.threshold 0.8
  c12n config set keyword.threshold 0.8 --scope user

Only the named key is rewritten — the rest of the target file is left
alone, so this is safe to run against a config you edited by hand.

Caveats:

  - Writing to a lower-precedence layer may have no visible effect. If
    the project file also sets the key, 'config set --scope user' will
    appear to do nothing, because the project layer still wins. Confirm
    with 'c12n config get'.
  - --scope system writes /etc/c12n/config.yaml, affecting every user on
    the machine, and usually needs elevated privileges.
  - The value is validated against c12n's config schema; an out-of-range
    or misspelled key is rejected rather than written.`,
		Args: cobra.ExactArgs(2),
		RunE: func(cmd *cobra.Command, args []string) error {
			opts := ConfigOptsFromContext(cmd)
			scope, err := parseConfigScope(scopeStr)
			if err != nil {
				return err
			}
			return config.Set(args[0], args[1], scope, opts)
		},
		ValidArgsFunction: func(
			cmd *cobra.Command,
			args []string,
			toComplete string,
		) ([]string, cobra.ShellCompDirective) {
			switch len(args) {
			case 0:
				return completeKeys(cmd, args, toComplete)
			case 1:
				return completeValues(cmd, args[0])
			default:
				return nil, cobra.ShellCompDirectiveNoFileComp
			}
		},
	}

	cmd.Flags().StringVar(&scopeStr, "scope", "project",
		"Config scope (system|user|project)")
	_ = cmd.RegisterFlagCompletionFunc("scope",
		func(_ *cobra.Command, _ []string, _ string) ([]string, cobra.ShellCompDirective) {
			return []string{"system", "user", "project"},
				cobra.ShellCompDirectiveNoFileComp
		})

	// write for the same reason as init: --scope can name the system
	// file, so the reachable blast radius is shared, not CWD-local.
	//
	// Not destructive — it rewrites one key and preserves the rest of
	// the file, so nothing unrelated is lost.
	//
	// Idempotent: setting the same key to the same value twice leaves
	// the file in the same state.
	cli.SetSideEffect(cmd, cli.SideEffectWrite)
	cli.SetIdempotency(cmd, cli.IdempotencyYes)

	return cmd
}

func configListCmd() *cobra.Command {
	var (
		scopeStr string
		format   string
	)

	cmd := &cobra.Command{
		Use:   "list",
		Short: "List config entries",
		Long: `List config entries with the scope each one was set in.

Unlike 'config get', which collapses the layers into one answer, list
shows the SCOPE column so you can see where a value actually came from
and why an edit in another layer had no effect:

  c12n config list                  every entry, all scopes
  c12n config list --scope project  only entries from ./.c12n.yaml
  c12n config list -f json          machine-readable

Entries are the keys explicitly present in a config file. Keys left at
their built-in default do not appear here — an empty listing means no
config file set anything, not that c12n has no configuration.`,
		Args: cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			opts := ConfigOptsFromContext(cmd)
			entries, err := config.List(opts)
			if err != nil {
				return err
			}

			// Filter by scope if not "all".
			if scopeStr != "all" {
				scope, err := parseConfigScope(scopeStr)
				if err != nil {
					return err
				}
				var filtered []config.Entry
				for _, e := range entries {
					if e.Scope == scope {
						filtered = append(filtered, e)
					}
				}
				entries = filtered
			}

			type row struct {
				Key   string `table:"KEY"   json:"key"`
				Value string `table:"VALUE" json:"value"`
				Scope string `table:"SCOPE" json:"scope"`
			}
			rows := make([]row, len(entries))
			for i, e := range entries {
				rows[i] = row{
					Key:   e.Key,
					Value: e.Value,
					Scope: scopeLabel(e.Scope),
				}
			}

			return output.Render(cmd.OutOrStdout(), format, rows)
		},
	}

	cmd.Flags().StringVar(&scopeStr, "scope", "all",
		"Filter by scope (all|system|user|project)")
	cmd.Flags().StringVarP(&format, "format", "f", "table",
		"Output format (json|table|yaml)")

	_ = cmd.RegisterFlagCompletionFunc("scope",
		func(_ *cobra.Command, _ []string, _ string) ([]string, cobra.ShellCompDirective) {
			return []string{"all", "system", "user", "project"},
				cobra.ShellCompDirectiveNoFileComp
		})
	_ = cmd.RegisterFlagCompletionFunc("format",
		func(_ *cobra.Command, _ []string, _ string) ([]string, cobra.ShellCompDirective) {
			return []string{"json", "table", "yaml"},
				cobra.ShellCompDirectiveNoFileComp
		})

	// Enumerates existing entries; opens config files read-only.
	cli.SetSideEffect(cmd, cli.SideEffectRead)
	cli.SetIdempotency(cmd, cli.IdempotencyYes)

	return cmd
}

// --- helpers ---

func parseConfigScope(s string) (config.Scope, error) {
	switch s {
	case "system":
		return config.ScopeSystem, nil
	case "user":
		return config.ScopeUser, nil
	case "project":
		return config.ScopeProject, nil
	default:
		return 0, fmt.Errorf("unknown scope %q: use system, user, or project", s)
	}
}

func scopeLabel(s config.Scope) string {
	switch s {
	case config.ScopeSystem:
		return "system"
	case config.ScopeUser:
		return "user"
	case config.ScopeProject:
		return "project"
	default:
		return "unknown"
	}
}

func completeKeys(
	_ *cobra.Command,
	_ []string,
	_ string,
) ([]string, cobra.ShellCompDirective) {
	schema, err := c12n.ConfigSchema()
	if err != nil {
		return nil, cobra.ShellCompDirectiveNoFileComp
	}
	items := pkl.CompletionKeys(schema)
	out := make([]string, len(items))
	for i, it := range items {
		if it.Description != "" {
			out[i] = fmt.Sprintf("%s\t%s", it.Value, it.Description)
		} else {
			out[i] = it.Value
		}
	}
	return out, cobra.ShellCompDirectiveNoFileComp
}

func completeValues(
	_ *cobra.Command,
	key string,
) ([]string, cobra.ShellCompDirective) {
	schema, err := c12n.ConfigSchema()
	if err != nil {
		return nil, cobra.ShellCompDirectiveNoFileComp
	}
	items := pkl.CompletionValues(schema, key)
	out := make([]string, len(items))
	for i, it := range items {
		out[i] = it.Value
	}
	return out, cobra.ShellCompDirectiveNoFileComp
}

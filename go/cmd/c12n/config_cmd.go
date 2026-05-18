package main

import (
	"fmt"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
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
		Args:  cobra.ExactArgs(1),
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
	return cmd
}

func configSetCmd() *cobra.Command {
	var scopeStr string

	cmd := &cobra.Command{
		Use:   "set <key> <value>",
		Short: "Set a config value",
		Args:  cobra.ExactArgs(2),
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
		Args:  cobra.NoArgs,
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

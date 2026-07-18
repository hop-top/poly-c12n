package main

import (
	"fmt"

	"github.com/spf13/cobra"
	"github.com/spf13/pflag"

	c12n "hop.top/c12n"
	"hop.top/kit/go/core/config/pkl"
)

// registerCompletions wires shell completion functions for flags across all
// commands. Call after all subcommands are added to root.
func registerCompletions(root *cobra.Command) {
	signalFn := func(
		_ *cobra.Command, _ []string, _ string,
	) ([]string, cobra.ShellCompDirective) {
		return c12n.AllSignalTypes(), cobra.ShellCompDirectiveNoFileComp
	}

	formatFn := func(
		_ *cobra.Command, _ []string, _ string,
	) ([]string, cobra.ShellCompDirective) {
		return []string{"json", "table", "text"},
			cobra.ShellCompDirectiveNoFileComp
	}

	scopeFn := func(
		_ *cobra.Command, _ []string, _ string,
	) ([]string, cobra.ShellCompDirective) {
		return []string{"system", "user", "project"},
			cobra.ShellCompDirectiveNoFileComp
	}

	keyFn := func(
		_ *cobra.Command, _ []string, _ string,
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

	walkCommands(root, func(cmd *cobra.Command) {
		cmd.Flags().VisitAll(func(f *pflag.Flag) {
			switch f.Name {
			case "signal":
				_ = cmd.RegisterFlagCompletionFunc("signal", signalFn)
			case "format":
				_ = cmd.RegisterFlagCompletionFunc("format", formatFn)
			case "scope":
				_ = cmd.RegisterFlagCompletionFunc("scope", scopeFn)
			}
		})
	})

	// Register signal name completion for "signals inspect" positional arg.
	if inspectCmd := findSubCommand(root, "signals", "inspect"); inspectCmd != nil {
		inspectCmd.ValidArgsFunction = signalFn
	}

	// Register config key completion for "config get".
	// Note: "config set" has its own ValidArgsFunction in config_cmd.go
	// that handles both key and value completion.
	if getCmd := findSubCommand(root, "config", "get"); getCmd != nil {
		getCmd.ValidArgsFunction = keyFn
	}
}

// walkCommands recursively visits cmd and all descendants.
func walkCommands(cmd *cobra.Command, fn func(*cobra.Command)) {
	fn(cmd)
	for _, child := range cmd.Commands() {
		walkCommands(child, fn)
	}
}

// findSubCommand traverses a path of subcommand names from root.
func findSubCommand(root *cobra.Command, names ...string) *cobra.Command {
	cmd := root
	for _, name := range names {
		found := false
		for _, child := range cmd.Commands() {
			if child.Name() == name {
				cmd = child
				found = true
				break
			}
		}
		if !found {
			return nil
		}
	}
	return cmd
}

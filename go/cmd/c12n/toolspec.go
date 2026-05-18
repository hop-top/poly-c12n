package main

import (
	"encoding/json"
	"fmt"

	"github.com/spf13/cobra"

	"hop.top/kit/go/ai/toolspec"
)

func toolspecCmd() *cobra.Command {
	return &cobra.Command{
		Use:   "toolspec",
		Short: "Output the c12n ToolSpec as JSON",
		Args:  cobra.NoArgs,
		RunE: func(cmd *cobra.Command, _ []string) error {
			spec := buildToolSpec()
			enc := json.NewEncoder(cmd.OutOrStdout())
			enc.SetIndent("", "  ")
			return enc.Encode(spec)
		},
	}
}

func buildToolSpec() toolspec.ToolSpec {
	return toolspec.ToolSpec{
		Name:          "c12n",
		SchemaVersion: version,
		Commands: []toolspec.Command{
			{
				Name: "classify",
				Flags: []toolspec.Flag{
					{Name: "format", Short: "f", Type: "string", Description: "Output format (json|table|text)"},
					{Name: "signal", Short: "s", Type: "string", Description: "Filter to a specific signal type"},
					{Name: "min-confidence", Type: "float64", Description: "Minimum confidence threshold (0.0-1.0)"},
					{Name: "file", Type: "string", Description: "Read input text from file"},
					{Name: "stdin", Type: "bool", Description: "Read input text from stdin"},
				},
				Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
				Contract: &toolspec.Contract{Idempotent: true},
				OutputSchema: &toolspec.OutputSchema{
					Format: "json",
					Fields: []string{"results", "duration_ns", "errors"},
				},
				Intent: &toolspec.Intent{
					Domain:   "classification",
					Category: "llm-routing",
				},
			},
			{
				Name: "config",
				Children: []toolspec.Command{
					{
						Name: "get",
						Flags: []toolspec.Flag{
							{Name: "key", Type: "string", Description: "Config key path"},
						},
						Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
						Contract: &toolspec.Contract{Idempotent: true},
					},
					{
						Name: "set",
						Flags: []toolspec.Flag{
							{Name: "scope", Type: "string", Description: "Config scope (system|user|project)"},
						},
						Safety: &toolspec.Safety{Level: toolspec.SafetyLevelCaution},
						Contract: &toolspec.Contract{
							Idempotent:  true,
							SideEffects: []string{"writes config file"},
						},
					},
					{
						Name: "list",
						Flags: []toolspec.Flag{
							{Name: "scope", Type: "string", Description: "Filter by scope (all|system|user|project)"},
							{Name: "format", Short: "f", Type: "string", Description: "Output format (json|table|yaml)"},
						},
						Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
						Contract: &toolspec.Contract{Idempotent: true},
					},
				},
				Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
				Contract: &toolspec.Contract{Idempotent: true},
			},
			{
				Name: "init",
				Flags: []toolspec.Flag{
					{Name: "dry-run", Type: "bool", Description: "Preview without writing config"},
					{Name: "answers-file", Type: "string", Description: "Path to YAML answers file"},
					{Name: "scope", Type: "string", Description: "Config scope (system|user|project)"},
				},
				Safety: &toolspec.Safety{Level: toolspec.SafetyLevelCaution},
				Contract: &toolspec.Contract{
					SideEffects: []string{"creates config file"},
				},
			},
			{
				Name: "signals",
				Children: []toolspec.Command{
					{
						Name:     "inspect",
						Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
						Contract: &toolspec.Contract{Idempotent: true},
					},
				},
				Flags: []toolspec.Flag{
					{Name: "enabled", Type: "bool", Description: "Show only enabled signals"},
				},
				Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
				Contract: &toolspec.Contract{Idempotent: true},
			},
			{
				Name: "bench",
				Flags: []toolspec.Flag{
					{Name: "iterations", Short: "n", Type: "int", Description: "Number of iterations"},
					{Name: "text", Short: "t", Type: "string", Description: "Text to classify"},
					{Name: "input", Type: "string", Description: "JSONL file with ClassificationContext objects"},
					{Name: "signal", Short: "s", Type: "string", Description: "Filter to a specific signal type"},
					{Name: "concurrency", Short: "c", Type: "int", Description: "Number of concurrent workers"},
					{Name: "output", Short: "o", Type: "string", Description: "Write ben-compatible JSONL to file"},
				},
				Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
				Contract: &toolspec.Contract{Idempotent: true},
			},
			{
				Name: "upgrade",
				Flags: []toolspec.Flag{
					{Name: "check", Type: "bool", Description: "Check for updates without installing"},
				},
				Safety: &toolspec.Safety{Level: toolspec.SafetyLevelCaution},
				Contract: &toolspec.Contract{
					SideEffects: []string{"replaces binary"},
				},
			},
			{
				Name:     "doctor",
				Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
				Contract: &toolspec.Contract{Idempotent: true},
			},
			{
				Name: "tip",
				Children: []toolspec.Command{
					{
						Name:     "suggest",
						Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
						Contract: &toolspec.Contract{Idempotent: true},
					},
				},
				Safety:   &toolspec.Safety{Level: toolspec.SafetyLevelSafe},
				Contract: &toolspec.Contract{Idempotent: true},
			},
		},
		ErrorPatterns: []toolspec.ErrorPattern{
			{
				Pattern: "pipeline not available",
				Fix:     "Ensure config is valid and pipeline can be created",
				Cause:   "Config load or pipeline creation failed in PersistentPreRunE",
			},
			{
				Pattern: "no input: provide text as args, --file, or --stdin",
				Fix:     "Pass text as positional args, use --file <path>, or --stdin",
				Cause:   "classify command received no input",
			},
			{
				Pattern: "unknown signal type",
				Fix:     "Run 'c12n signals' to see valid signal type names",
				Cause:   "Invalid signal type name provided",
			},
			{
				Pattern: "load config:",
				Fix:     "Run 'c12n init' to create a valid config or check YAML syntax",
				Cause:   "Config file missing or malformed",
			},
			{
				Pattern: "--iterations must be >= 1",
				Fix:     "Provide a positive integer for --iterations",
				Cause:   "Bench iterations set to 0 or negative",
			},
		},
		Workflows: []toolspec.Workflow{
			{
				Name:  "quick-classify",
				Steps: []string{"c12n classify 'your text here'"},
			},
			{
				Name: "full-setup",
				Steps: []string{
					"c12n init",
					"c12n doctor",
					"c12n classify --stdin < input.txt",
				},
			},
			{
				Name: "benchmark-compare",
				Steps: []string{
					"c12n bench -n 100 -o baseline.jsonl",
					"# adjust config",
					"c12n bench -n 100 -o experiment.jsonl",
					fmt.Sprintf("# compare baseline.jsonl vs experiment.jsonl"),
				},
			},
		},
		StateIntrospection: &toolspec.StateIntrospection{
			ConfigCommands: []string{
				"c12n config list",
				"c12n config get <key>",
				"c12n doctor",
			},
			EnvVars: []string{
				"C12N_CONFIG",
				"CGO_ENABLED",
			},
		},
	}
}

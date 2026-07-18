package main

import (
	"fmt"
	"strings"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
	"hop.top/kit/go/console/markdown"
	"hop.top/kit/go/console/output"
)

// signalInfo holds display data for a single signal type.
type signalInfo struct {
	Type        string `table:"TYPE"        json:"type"`
	Enabled     bool   `table:"ENABLED"     json:"enabled"`
	Description string `table:"DESCRIPTION" json:"description"`
}

// signalDescriptions maps each signal type to a human-readable description.
var signalDescriptions = map[c12n.SignalType]string{
	c12n.SignalKeyword:      "Regex/keyword pattern matching",
	c12n.SignalEmbedding:    "Semantic similarity via embeddings",
	c12n.SignalDomain:       "Domain classification model",
	c12n.SignalJailbreak:    "Jailbreak attempt detection",
	c12n.SignalPII:          "Personally identifiable information detection",
	c12n.SignalToxicity:     "Toxic/harmful content scoring",
	c12n.SignalContext:      "Context window analysis",
	c12n.SignalStructure:    "Input structure classification",
	c12n.SignalLanguage:     "Natural language detection",
	c12n.SignalComplexity:   "Request complexity estimation",
	c12n.SignalPreference:   "User preference inference",
	c12n.SignalFeedback:     "Feedback loop signal",
	c12n.SignalOutputFormat: "Output format detection",
	c12n.SignalCodeContent:  "Code content detection",
	c12n.SignalToolCalling:  "Tool-calling intent detection",
	c12n.SignalCostEstimate: "Cost estimation signal",
	c12n.SignalSentiment:    "Sentiment analysis",
	c12n.SignalIntent:       "Intent classification",
	c12n.SignalTopic:        "Topic classification",
	c12n.SignalCustom:       "User-defined custom signal",
}

func signalsCmd() *cobra.Command {
	var enabledOnly bool

	cmd := &cobra.Command{
		Use:   "signals",
		Short: "List and inspect classification signal types",
		RunE: func(cmd *cobra.Command, _ []string) error {
			cfg := ConfigFromContext(cmd)
			enabled := make(map[c12n.SignalType]bool)
			if cfg != nil {
				for _, s := range cfg.EnabledSignals() {
					enabled[s] = true
				}
			}

			all := c12n.AllSignalTypes()
			var rows []signalInfo
			for _, name := range all {
				st := c12n.SignalType(name)
				on := enabled[st]
				if enabledOnly && !on {
					continue
				}
				rows = append(rows, signalInfo{
					Type:        name,
					Enabled:     on,
					Description: signalDescriptions[st],
				})
			}

			if len(rows) == 0 {
				fmt.Fprintln(cmd.OutOrStdout(), "No signals match the filter.")
				return nil
			}

			return output.Render(cmd.OutOrStdout(), output.Table, rows)
		},
	}

	cmd.Flags().BoolVar(&enabledOnly, "enabled", false,
		"Show only enabled signals")

	cmd.AddCommand(signalInspectCmd())

	return cmd
}

func signalInspectCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:               "inspect <signal>",
		Short:             "Show detailed config for a signal type",
		Args:              cobra.ExactArgs(1),
		ValidArgsFunction: completeSignalNames,
		RunE: func(cmd *cobra.Command, args []string) error {
			cfg := ConfigFromContext(cmd)
			st := c12n.SignalType(args[0])

			desc, ok := signalDescriptions[st]
			if !ok {
				return fmt.Errorf("unknown signal type %q", args[0])
			}

			enabled := make(map[c12n.SignalType]bool)
			if cfg != nil {
				for _, s := range cfg.EnabledSignals() {
					enabled[s] = true
				}
			}

			var sb strings.Builder
			sb.WriteString(fmt.Sprintf("# %s\n\n", st))
			sb.WriteString(fmt.Sprintf("**Description:** %s\n\n", desc))
			sb.WriteString(fmt.Sprintf("**Enabled:** %v\n\n", enabled[st]))

			detail := signalConfigDetail(cfg, st)
			if detail != "" {
				sb.WriteString("## Configuration\n\n")
				sb.WriteString(detail)
			}

			rendered, err := markdown.Render(sb.String(), false)
			if err != nil {
				return err
			}
			fmt.Fprint(cmd.OutOrStdout(), rendered)
			return nil
		},
	}
	return cmd
}

// completeSignalNames provides tab-completion for signal type names.
func completeSignalNames(
	_ *cobra.Command, _ []string, _ string,
) ([]string, cobra.ShellCompDirective) {
	return c12n.AllSignalTypes(), cobra.ShellCompDirectiveNoFileComp
}

// signalConfigDetail returns markdown-formatted config details for a signal.
func signalConfigDetail(cfg *c12n.Config, st c12n.SignalType) string {
	if cfg == nil {
		return ""
	}
	var lines []string
	add := func(k string, v any) {
		lines = append(lines, fmt.Sprintf("- **%s:** `%v`", k, v))
	}

	switch st {
	case c12n.SignalKeyword:
		add("strategy", cfg.KeywordStrategy)
		add("threshold", cfg.KeywordThreshold)
	case c12n.SignalEmbedding:
		add("threshold", cfg.EmbeddingThreshold)
		if cfg.EmbeddingModelPath != nil {
			add("model_path", *cfg.EmbeddingModelPath)
		}
	case c12n.SignalDomain:
		if cfg.DomainModelPath != nil {
			add("model_path", *cfg.DomainModelPath)
		}
	case c12n.SignalJailbreak:
		if cfg.SafetyJailbreakModelPath != nil {
			add("model_path", *cfg.SafetyJailbreakModelPath)
		}
	case c12n.SignalToxicity:
		add("threshold", cfg.SafetyToxicityThreshold)
	case c12n.SignalContext:
		add("output_ratio", cfg.ContextOutputRatio)
	case c12n.SignalComplexity:
		add("margin", cfg.ComplexityMargin)
		if cfg.ComplexityModelPath != nil {
			add("model_path", *cfg.ComplexityModelPath)
		}
	}

	if len(lines) == 0 {
		return ""
	}
	return strings.Join(lines, "\n") + "\n"
}

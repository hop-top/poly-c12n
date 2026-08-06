package main

import (
	"encoding/json"
	"fmt"
	"strings"

	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
	"hop.top/kit/go/console/cli"
)

// ContextEnvelope is the JSON input for the tip suggest command.
type ContextEnvelope struct {
	Signal     string `json:"signal,omitempty"`
	Format     string `json:"format,omitempty"`
	Subcommand string `json:"subcommand,omitempty"`
}

// SuggestResponse is the JSON output of the tip suggest command.
type SuggestResponse struct {
	Corrections []Correction `json:"corrections,omitempty"`
}

// Correction describes a single suggestion.
type Correction struct {
	Field      string `json:"field"`
	Input      string `json:"input"`
	Suggestion string `json:"suggestion"`
	Reason     string `json:"reason"`
}

// knownSubcommands lists top-level subcommands for fuzzy matching.
var knownSubcommands = []string{
	"classify", "bench", "signals", "upgrade", "doctor", "tip",
}

// knownFormats lists recognized output formats.
var knownFormats = []string{"json", "table", "text"}

func tipCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "tip",
		Short: "Contextual suggestions and corrections",
	}

	cmd.AddCommand(tipSuggestCmd())
	return cmd
}

func tipSuggestCmd() *cobra.Command {
	cmd := &cobra.Command{
		Use:   "suggest",
		Short: "Suggest corrections for common input mistakes",
		Long: `Map a mistyped signal, format, or subcommand name to what was meant.

Reads a JSON ContextEnvelope on stdin and writes corrections to stdout.
Any subset of the three fields may be present:

  echo '{"signal":"jailbrek"}' | c12n tip suggest
  echo '{"subcommand":"clasify","format":"jsn"}' | c12n tip suggest

Matching is by Levenshtein distance against the known names. A field
that already holds a valid name produces no correction, so an empty
corrections array means "your input was fine". When nothing is close
enough, the correction carries an empty suggestion and a reason
explaining that the name is unrecognized.

Built for the error-recovery path of agents and shell wrappers rather
than for direct human use. It is advisory only: it never runs the
corrected command and never edits config.`,
		RunE: func(cmd *cobra.Command, _ []string) error {
			var env ContextEnvelope
			if err := json.NewDecoder(cmd.InOrStdin()).Decode(&env); err != nil {
				return fmt.Errorf("decode input: %w", err)
			}

			resp := SuggestResponse{}

			if env.Signal != "" {
				if c := matchSignal(env.Signal); c != nil {
					resp.Corrections = append(resp.Corrections, *c)
				}
			}
			if env.Format != "" {
				if c := matchFormat(env.Format); c != nil {
					resp.Corrections = append(resp.Corrections, *c)
				}
			}
			if env.Subcommand != "" {
				if c := matchSubcommand(env.Subcommand); c != nil {
					resp.Corrections = append(resp.Corrections, *c)
				}
			}

			enc := json.NewEncoder(cmd.OutOrStdout())
			enc.SetIndent("", "  ")
			return enc.Encode(resp)
		},
	}

	// Pure function of stdin: string-distance matching against
	// in-binary name lists. Nothing is persisted, no config is read,
	// and the suggested command is never executed.
	cli.SetSideEffect(cmd, cli.SideEffectRead)
	cli.SetIdempotency(cmd, cli.IdempotencyYes)

	return cmd
}

func matchSignal(input string) *Correction {
	all := c12n.AllSignalTypes()
	lower := strings.ToLower(input)

	// Exact match (case-insensitive).
	for _, s := range all {
		if strings.ToLower(s) == lower {
			return nil
		}
	}

	best, dist := closest(lower, all)
	if dist <= 3 {
		return &Correction{
			Field:      "signal",
			Input:      input,
			Suggestion: best,
			Reason:     "closest match by edit distance",
		}
	}
	return &Correction{
		Field:      "signal",
		Input:      input,
		Suggestion: "",
		Reason:     "unknown signal type",
	}
}

func matchFormat(input string) *Correction {
	lower := strings.ToLower(input)
	for _, f := range knownFormats {
		if f == lower {
			return nil
		}
	}

	best, dist := closest(lower, knownFormats)
	if dist <= 2 {
		return &Correction{
			Field:      "format",
			Input:      input,
			Suggestion: best,
			Reason:     "did you mean this format?",
		}
	}
	return &Correction{
		Field:      "format",
		Input:      input,
		Suggestion: strings.Join(knownFormats, "|"),
		Reason:     "unknown format; valid options listed",
	}
}

func matchSubcommand(input string) *Correction {
	lower := strings.ToLower(input)
	for _, s := range knownSubcommands {
		if s == lower {
			return nil
		}
	}

	best, dist := closest(lower, knownSubcommands)
	if dist <= 3 {
		return &Correction{
			Field:      "subcommand",
			Input:      input,
			Suggestion: best,
			Reason:     "closest subcommand match",
		}
	}
	return &Correction{
		Field:      "subcommand",
		Input:      input,
		Suggestion: "",
		Reason:     "unknown subcommand",
	}
}

// closest returns the best match from candidates and its Levenshtein distance.
func closest(input string, candidates []string) (string, int) {
	best := ""
	bestDist := len(input) + 10
	for _, c := range candidates {
		d := levenshtein(strings.ToLower(input), strings.ToLower(c))
		if d < bestDist {
			bestDist = d
			best = c
		}
	}
	return best, bestDist
}

// levenshtein computes the edit distance between two strings.
func levenshtein(a, b string) int {
	la, lb := len(a), len(b)
	if la == 0 {
		return lb
	}
	if lb == 0 {
		return la
	}

	prev := make([]int, lb+1)
	curr := make([]int, lb+1)
	for j := range prev {
		prev[j] = j
	}

	for i := 1; i <= la; i++ {
		curr[0] = i
		for j := 1; j <= lb; j++ {
			cost := 1
			if a[i-1] == b[j-1] {
				cost = 0
			}
			curr[j] = min(
				prev[j]+1,
				curr[j-1]+1,
				prev[j-1]+cost,
			)
		}
		prev, curr = curr, prev
	}
	return prev[lb]
}

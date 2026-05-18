package c12n

import (
	"encoding/json"
	"fmt"
	"io"
	"sort"
	"strings"
	"text/tabwriter"
	"time"

	"github.com/spf13/cobra"
)

// AllSignalTypes returns every defined SignalType value.
func AllSignalTypes() []string {
	return []string{
		string(SignalKeyword),
		string(SignalEmbedding),
		string(SignalDomain),
		string(SignalJailbreak),
		string(SignalPII),
		string(SignalToxicity),
		string(SignalContext),
		string(SignalStructure),
		string(SignalLanguage),
		string(SignalComplexity),
		string(SignalPreference),
		string(SignalFeedback),
		string(SignalOutputFormat),
		string(SignalCodeContent),
		string(SignalToolCalling),
		string(SignalCostEstimate),
		string(SignalSentiment),
		string(SignalIntent),
		string(SignalTopic),
		string(SignalCustom),
	}
}

// NewClassifyCommand returns a cobra command that classifies text
// through the c12n pipeline.
func NewClassifyCommand(pipeline *Pipeline) *cobra.Command {
	var (
		format string
		signal string
	)

	cmd := &cobra.Command{
		Use:   "classify [text...]",
		Short: "Classify text through the c12n pipeline",
		RunE: func(cmd *cobra.Command, args []string) error {
			text, err := resolveText(args, cmd.InOrStdin())
			if err != nil {
				return err
			}

			raw, err := pipeline.Evaluate(ClassificationContext{Text: text})
			if err != nil {
				return fmt.Errorf("evaluate: %w", err)
			}

			result, err := ParseResult(raw)
			if err != nil {
				return fmt.Errorf("parse result: %w", err)
			}

			if signal != "" {
				result = filterSignal(result, SignalType(signal))
			}

			return renderResult(cmd.OutOrStdout(), result, format)
		},
	}

	cmd.Flags().StringVarP(&format, "format", "f", "text",
		"Output format (json|table|text)")
	cmd.Flags().StringVarP(&signal, "signal", "s", "",
		"Filter to a specific signal type")

	return cmd
}

// NewBenchCommand returns a cobra command that benchmarks the
// classification pipeline latency.
func NewBenchCommand(pipeline *Pipeline) *cobra.Command {
	var (
		iterations int
		text       string
	)

	cmd := &cobra.Command{
		Use:   "bench",
		Short: "Benchmark classification pipeline latency",
		RunE: func(cmd *cobra.Command, args []string) error {
			if iterations < 1 {
				return fmt.Errorf("iterations must be >= 1, got %d", iterations)
			}
			ctx := ClassificationContext{Text: text}
			durations := make([]time.Duration, 0, iterations)

			for i := 0; i < iterations; i++ {
				start := time.Now()
				if _, err := pipeline.Evaluate(ctx); err != nil {
					return fmt.Errorf("iteration %d: %w", i, err)
				}
				durations = append(durations, time.Since(start))
			}

			sort.Slice(durations, func(i, j int) bool {
				return durations[i] < durations[j]
			})

			w := cmd.OutOrStdout()
			fmt.Fprintf(w, "iterations: %d\n", iterations)
			fmt.Fprintf(w, "min:        %s\n", durations[0])
			fmt.Fprintf(w, "max:        %s\n", durations[len(durations)-1])
			fmt.Fprintf(w, "avg:        %s\n", avg(durations))
			fmt.Fprintf(w, "p50:        %s\n", percentile(durations, 50))
			fmt.Fprintf(w, "p95:        %s\n", percentile(durations, 95))
			fmt.Fprintf(w, "p99:        %s\n", percentile(durations, 99))

			return nil
		},
	}

	cmd.Flags().IntVarP(&iterations, "iterations", "n", 100,
		"Number of iterations")
	cmd.Flags().StringVarP(&text, "text", "t", "Hello, how are you?",
		"Text to classify")

	return cmd
}

// RegisterCompletions registers flag completions on the given command
// and its direct children for --signal and --format flags.
func RegisterCompletions(cmd *cobra.Command) {
	registerOn := func(c *cobra.Command) {
		if f := c.Flags().Lookup("signal"); f != nil {
			_ = c.RegisterFlagCompletionFunc("signal",
				func(_ *cobra.Command, _ []string, _ string) ([]string, cobra.ShellCompDirective) {
					return AllSignalTypes(), cobra.ShellCompDirectiveNoFileComp
				})
		}
		if f := c.Flags().Lookup("format"); f != nil {
			_ = c.RegisterFlagCompletionFunc("format",
				func(_ *cobra.Command, _ []string, _ string) ([]string, cobra.ShellCompDirective) {
					return []string{"json", "table", "text"}, cobra.ShellCompDirectiveNoFileComp
				})
		}
	}

	registerOn(cmd)
	for _, child := range cmd.Commands() {
		registerOn(child)
	}
}

// --- internal helpers ---

// resolveText reads text from args or stdin.
func resolveText(args []string, stdin io.Reader) (string, error) {
	if len(args) > 0 {
		return strings.Join(args, " "), nil
	}
	data, err := io.ReadAll(stdin)
	if err != nil {
		return "", fmt.Errorf("read stdin: %w", err)
	}
	text := strings.TrimSpace(string(data))
	if text == "" {
		return "", fmt.Errorf("no input text provided")
	}
	return text, nil
}

// filterSignal returns a copy of the result containing only matching signals.
func filterSignal(r *PipelineResult, t SignalType) *PipelineResult {
	out := &PipelineResult{
		DurationNs: r.DurationNs,
		Errors:     r.Errors,
	}
	for _, s := range r.Results {
		if s.Type == t {
			out.Results = append(out.Results, s)
		}
	}
	return out
}

// renderResult writes the pipeline result in the requested format.
func renderResult(w io.Writer, r *PipelineResult, format string) error {
	switch format {
	case "json":
		enc := json.NewEncoder(w)
		enc.SetIndent("", "  ")
		return enc.Encode(r)

	case "table":
		tw := tabwriter.NewWriter(w, 0, 4, 2, ' ', 0)
		fmt.Fprintln(tw, "NAME\tTYPE\tCONFIDENCE\tLABELS")
		for _, s := range r.Results {
			fmt.Fprintf(tw, "%s\t%s\t%.4f\t%s\n",
				s.Name, s.Type, s.Confidence, strings.Join(s.Labels, ","))
		}
		return tw.Flush()

	case "text":
		for _, s := range r.Results {
			labels := ""
			if len(s.Labels) > 0 {
				labels = " [" + strings.Join(s.Labels, ", ") + "]"
			}
			fmt.Fprintf(w, "%s (%s): %.4f%s\n",
				s.Name, s.Type, s.Confidence, labels)
		}
		return nil

	default:
		return fmt.Errorf("unknown format %q", format)
	}
}

// avg returns the mean duration.
func avg(ds []time.Duration) time.Duration {
	if len(ds) == 0 {
		return 0
	}
	var total time.Duration
	for _, d := range ds {
		total += d
	}
	return total / time.Duration(len(ds))
}

// percentile returns the p-th percentile from a sorted slice.
func percentile(ds []time.Duration, p int) time.Duration {
	if len(ds) == 0 {
		return 0
	}
	idx := (p * len(ds)) / 100
	if idx >= len(ds) {
		idx = len(ds) - 1
	}
	return ds[idx]
}

package main

import (
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/mattn/go-isatty"
	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
	"hop.top/kit/go/console/output"
)

func classifyCmd() *cobra.Command {
	var (
		format        string
		signal        string
		minConfidence float64
		file          string
		stdin         bool
	)

	cmd := &cobra.Command{
		Use:   "classify [text]",
		Short: "Classify text through the c12n pipeline",
		Long: `Classify text through the c12n pipeline.

Text can be provided as positional arguments (joined with spaces),
via --file to read from a file, or via --stdin to read from standard input.`,
		RunE: func(cmd *cobra.Command, args []string) error {
			text, err := resolveInput(args, file, stdin, cmd.InOrStdin())
			if err != nil {
				return err
			}

			pipeline := PipelineFromContext(cmd)
			if pipeline == nil {
				return fmt.Errorf("pipeline not available")
			}

			// Use spinner when interactive tty with no args piped.
			interactive := !stdin && file == "" &&
				isatty.IsTerminal(os.Stdout.Fd())

			if interactive {
				fmt.Fprint(cmd.ErrOrStderr(), "classifying… ")
			}

			raw, err := pipeline.Evaluate(c12n.ClassificationContext{
				Text: text,
			})
			if err != nil {
				return fmt.Errorf("evaluate: %w", err)
			}

			if interactive {
				fmt.Fprintln(cmd.ErrOrStderr(), "done")
			}

			result, err := c12n.ParseResult(raw)
			if err != nil {
				return fmt.Errorf("parse result: %w", err)
			}

			// Filter by signal type.
			if signal != "" {
				result = filterBySignal(result, c12n.SignalType(signal))
			}

			// Filter by minimum confidence.
			if minConfidence > 0 {
				result = filterByConfidence(result, minConfidence)
			}

			return output.Render(cmd.OutOrStdout(), format, result)
		},
	}

	cmd.Flags().StringVarP(&format, "format", "f", "json",
		"Output format (json|table|text)")
	cmd.Flags().StringVarP(&signal, "signal", "s", "",
		"Filter to a specific signal type")
	cmd.Flags().Float64Var(&minConfidence, "min-confidence", 0,
		"Minimum confidence threshold (0.0-1.0)")
	cmd.Flags().StringVar(&file, "file", "",
		"Read input text from file")
	cmd.Flags().BoolVar(&stdin, "stdin", false,
		"Read input text from stdin")

	_ = cmd.RegisterFlagCompletionFunc("signal",
		func(_ *cobra.Command, _ []string, _ string) ([]string, cobra.ShellCompDirective) {
			return c12n.AllSignalTypes(), cobra.ShellCompDirectiveNoFileComp
		})
	_ = cmd.RegisterFlagCompletionFunc("format",
		func(_ *cobra.Command, _ []string, _ string) ([]string, cobra.ShellCompDirective) {
			return []string{"json", "table", "text"}, cobra.ShellCompDirectiveNoFileComp
		})

	return cmd
}

// resolveInput reads text from args, file, or stdin.
func resolveInput(args []string, filePath string, useStdin bool, r io.Reader) (string, error) {
	switch {
	case filePath != "":
		data, err := os.ReadFile(filePath)
		if err != nil {
			return "", fmt.Errorf("read file: %w", err)
		}
		return strings.TrimSpace(string(data)), nil
	case useStdin:
		data, err := io.ReadAll(r)
		if err != nil {
			return "", fmt.Errorf("read stdin: %w", err)
		}
		text := strings.TrimSpace(string(data))
		if text == "" {
			return "", fmt.Errorf("no input text on stdin")
		}
		return text, nil
	case len(args) > 0:
		return strings.Join(args, " "), nil
	default:
		return "", fmt.Errorf("no input: provide text as args, --file, or --stdin")
	}
}

// filterBySignal returns a copy containing only matching signal types.
func filterBySignal(r *c12n.PipelineResult, t c12n.SignalType) *c12n.PipelineResult {
	out := &c12n.PipelineResult{
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

// filterByConfidence returns a copy containing only results above threshold.
func filterByConfidence(r *c12n.PipelineResult, min float64) *c12n.PipelineResult {
	out := &c12n.PipelineResult{
		DurationNs: r.DurationNs,
		Errors:     r.Errors,
	}
	for _, s := range r.Results {
		if s.Confidence >= min {
			out.Results = append(out.Results, s)
		}
	}
	return out
}

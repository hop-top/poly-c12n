package main

import (
	"bufio"
	"encoding/json"
	"fmt"
	"os"
	"sort"
	"sync"
	"time"

	"github.com/mattn/go-isatty"
	"github.com/spf13/cobra"

	c12n "hop.top/c12n"
)

// benchRecord is a ben-compatible JSONL entry.
type benchRecord struct {
	Candidate string            `json:"candidate"`
	Metric    string            `json:"metric"`
	Value     float64           `json:"value"`
	Unit      string            `json:"unit"`
	Tags      map[string]string `json:"tags"`
}

func benchCmd() *cobra.Command {
	var (
		iterations  int
		text        string
		input       string
		signal      string
		concurrency int
		outputPath  string
	)

	cmd := &cobra.Command{
		Use:   "bench",
		Short: "Benchmark classification pipeline latency",
		Long: `Run N iterations of the classification pipeline and report
latency statistics (min, max, avg, p50, p95, p99).

Input can be a single --text string or a JSONL file via --input where
each line is a ClassificationContext object.

The --signal flag tags ben JSONL output; it does not filter the
classification pipeline.`,
		RunE: func(cmd *cobra.Command, _ []string) error {
			if iterations < 1 {
				return fmt.Errorf("--iterations must be >= 1, got %d", iterations)
			}

			pipeline := PipelineFromContext(cmd)
			if pipeline == nil {
				return fmt.Errorf("pipeline not available")
			}

			// Build input contexts.
			contexts, err := benchInputs(text, input)
			if err != nil {
				return err
			}

			isTTY := isatty.IsTerminal(os.Stderr.Fd())

			// Run benchmark.
			durations, err := runBench(
				pipeline, contexts, iterations,
				concurrency, signal, isTTY, cmd,
			)
			if err != nil {
				return err
			}

			sort.Slice(durations, func(i, j int) bool {
				return durations[i] < durations[j]
			})

			// Print stats.
			w := cmd.OutOrStdout()
			fmt.Fprintf(w, "iterations: %d\n", len(durations))
			fmt.Fprintf(w, "min:        %s\n", durations[0])
			fmt.Fprintf(w, "max:        %s\n", durations[len(durations)-1])
			fmt.Fprintf(w, "avg:        %s\n", benchAvg(durations))
			fmt.Fprintf(w, "p50:        %s\n", benchPercentile(durations, 50))
			fmt.Fprintf(w, "p95:        %s\n", benchPercentile(durations, 95))
			fmt.Fprintf(w, "p99:        %s\n", benchPercentile(durations, 99))

			// Write ben-compatible JSONL output.
			if outputPath != "" {
				return writeBenOutput(outputPath, durations, signal)
			}
			return nil
		},
	}

	cmd.Flags().IntVarP(&iterations, "iterations", "n", 100,
		"Number of iterations")
	cmd.Flags().StringVarP(&text, "text", "t", "Hello, how are you?",
		"Text to classify")
	cmd.Flags().StringVar(&input, "input", "",
		"JSONL file with ClassificationContext objects (one per line)")
	cmd.Flags().StringVarP(&signal, "signal", "s", "",
		"Signal label for ben output (does not filter pipeline)")
	cmd.Flags().IntVarP(&concurrency, "concurrency", "c", 1,
		"Number of concurrent workers")
	cmd.Flags().StringVarP(&outputPath, "output", "o", "",
		"Write ben-compatible JSONL to file")

	return cmd
}

// benchInputs resolves classification contexts from --text or --input.
func benchInputs(text, inputPath string) ([]c12n.ClassificationContext, error) {
	if inputPath != "" {
		return loadJSONLInputs(inputPath)
	}
	return []c12n.ClassificationContext{{Text: text}}, nil
}

// loadJSONLInputs reads a JSONL file of ClassificationContext objects.
func loadJSONLInputs(path string) ([]c12n.ClassificationContext, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, fmt.Errorf("open input file: %w", err)
	}
	defer f.Close()

	var contexts []c12n.ClassificationContext
	scanner := bufio.NewScanner(f)
	scanner.Buffer(make([]byte, 0, 64*1024), 10*1024*1024)
	lineNo := 0
	for scanner.Scan() {
		lineNo++
		line := scanner.Bytes()
		if len(line) == 0 {
			continue
		}
		var ctx c12n.ClassificationContext
		if err := json.Unmarshal(line, &ctx); err != nil {
			return nil, fmt.Errorf("line %d: %w", lineNo, err)
		}
		contexts = append(contexts, ctx)
	}
	if err := scanner.Err(); err != nil {
		return nil, fmt.Errorf("read input: %w", err)
	}
	if len(contexts) == 0 {
		return nil, fmt.Errorf("input file is empty")
	}
	return contexts, nil
}

// runBench executes the benchmark iterations, optionally concurrent.
func runBench(
	pipeline *c12n.Pipeline,
	contexts []c12n.ClassificationContext,
	iterations, concurrency int,
	signal string,
	isTTY bool,
	cmd *cobra.Command,
) ([]time.Duration, error) {
	if concurrency < 1 {
		concurrency = 1
	}

	durations := make([]time.Duration, iterations)
	errs := make([]error, iterations)

	sem := make(chan struct{}, concurrency)
	var wg sync.WaitGroup
	var mu sync.Mutex
	completed := 0

	for i := 0; i < iterations; i++ {
		wg.Add(1)
		go func(idx int) {
			defer wg.Done()
			sem <- struct{}{}
			defer func() { <-sem }()

			ctx := contexts[idx%len(contexts)]
			start := time.Now()
			_, err := pipeline.Evaluate(ctx)
			d := time.Since(start)

			durations[idx] = d
			errs[idx] = err

			if isTTY {
				mu.Lock()
				completed++
				pct := float64(completed) / float64(iterations) * 100
				fmt.Fprintf(cmd.ErrOrStderr(),
					"\r  bench: %d/%d (%.0f%%)", completed, iterations, pct)
				mu.Unlock()
			}
		}(i)
	}
	wg.Wait()

	if isTTY {
		fmt.Fprintln(cmd.ErrOrStderr())
	}

	// Return first error encountered.
	for i, err := range errs {
		if err != nil {
			return nil, fmt.Errorf("iteration %d: %w", i, err)
		}
	}

	_ = signal // reserved for future per-signal filtering
	return durations, nil
}

// writeBenOutput writes ben-compatible JSONL to the given path.
func writeBenOutput(path string, durations []time.Duration, signal string) error {
	sort.Slice(durations, func(i, j int) bool {
		return durations[i] < durations[j]
	})

	tag := "all"
	if signal != "" {
		tag = signal
	}

	metrics := map[string]float64{
		"latency_min": float64(durations[0].Microseconds()) / 1000.0,
		"latency_max": float64(durations[len(durations)-1].Microseconds()) / 1000.0,
		"latency_avg": float64(benchAvg(durations).Microseconds()) / 1000.0,
		"latency_p50": float64(benchPercentile(durations, 50).Microseconds()) / 1000.0,
		"latency_p95": float64(benchPercentile(durations, 95).Microseconds()) / 1000.0,
		"latency_p99": float64(benchPercentile(durations, 99).Microseconds()) / 1000.0,
	}

	f, err := os.Create(path)
	if err != nil {
		return fmt.Errorf("create output file: %w", err)
	}
	defer f.Close()

	enc := json.NewEncoder(f)
	for name, value := range metrics {
		rec := benchRecord{
			Candidate: "c12n",
			Metric:    name,
			Value:     value,
			Unit:      "ms",
			Tags:      map[string]string{"signal": tag},
		}
		if err := enc.Encode(rec); err != nil {
			return fmt.Errorf("write record: %w", err)
		}
	}
	return nil
}

func benchAvg(ds []time.Duration) time.Duration {
	if len(ds) == 0 {
		return 0
	}
	var total time.Duration
	for _, d := range ds {
		total += d
	}
	return total / time.Duration(len(ds))
}

func benchPercentile(ds []time.Duration, p int) time.Duration {
	if len(ds) == 0 {
		return 0
	}
	idx := (p * (len(ds) - 1)) / 100
	if idx >= len(ds) {
		idx = len(ds) - 1
	}
	return ds[idx]
}

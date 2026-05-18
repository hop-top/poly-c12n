package c12n

import (
	"encoding/json"
	"time"
)

// PipelineResult holds the full classification output.
type PipelineResult struct {
	Results    []SignalResult  `json:"results"`
	Errors     []PipelineError `json:"errors"`
	DurationNs int64           `json:"duration_ns"`
}

// ParseResult deserializes a JSON result string.
func ParseResult(raw string) (*PipelineResult, error) {
	var r PipelineResult
	if err := json.Unmarshal([]byte(raw), &r); err != nil {
		return nil, err
	}
	return &r, nil
}

// Duration returns the pipeline execution duration.
func (r *PipelineResult) Duration() time.Duration {
	return time.Duration(r.DurationNs) * time.Nanosecond
}

// Signal returns the first result matching the given type.
func (r *PipelineResult) Signal(t SignalType) *SignalResult {
	for i := range r.Results {
		if r.Results[i].Type == t {
			return &r.Results[i]
		}
	}
	return nil
}

// HasSignal returns true if a result with the given type exists.
func (r *PipelineResult) HasSignal(t SignalType) bool {
	return r.Signal(t) != nil
}

// Confidence returns the confidence for the given signal type, or 0.
func (r *PipelineResult) Confidence(t SignalType) float64 {
	s := r.Signal(t)
	if s == nil {
		return 0
	}
	return s.Confidence
}

// Signals returns all results matching the given type.
func (r *PipelineResult) Signals(t SignalType) []SignalResult {
	var out []SignalResult
	for _, s := range r.Results {
		if s.Type == t {
			out = append(out, s)
		}
	}
	return out
}

// HasErrors returns true if any signal errors occurred.
func (r *PipelineResult) HasErrors() bool {
	return len(r.Errors) > 0
}

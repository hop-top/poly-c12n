package c12n

import (
	"sync"
	"time"
)

// PipelineConfig configures a classification pipeline.
type PipelineConfig struct {
	MaxConcurrency int           `json:"max_concurrency"`
	Timeout        time.Duration `json:"timeout"`
}

// Pipeline wraps the C classification pipeline.
// Not safe for concurrent use without external synchronization.
type Pipeline struct {
	mu     sync.Mutex
	ptr    any // unsafe.Pointer stored as any to avoid importing unsafe in this file
	closed bool
}

// ClassificationContext is the input to a classification pipeline.
type ClassificationContext struct {
	Text     string            `json:"text"`
	History  []string          `json:"history"`
	Headers  map[string]string `json:"headers"`
	ImageURL *string           `json:"image_url,omitempty"`
	Config   map[string]any    `json:"config"`
}

// normalizeContext replaces nil slices/maps with empty values so the
// JSON sent to the Rust core never contains `null` for fields the
// engine treats as collections. The Rust side also tolerates `null`
// (via #[serde(default)]) — this is belt + braces.
func normalizeContext(ctx *ClassificationContext) {
	if ctx.History == nil {
		ctx.History = []string{}
	}
	if ctx.Headers == nil {
		ctx.Headers = map[string]string{}
	}
	if ctx.Config == nil {
		ctx.Config = map[string]any{}
	}
}

//go:build !cgo

package c12n

import "errors"

var errNoCgo = errors.New("c12n: built without cgo support")

// NewPipeline is a stub that returns an error when built without cgo.
func NewPipeline(cfg PipelineConfig) (*Pipeline, error) {
	return nil, errNoCgo
}

// Evaluate is a stub that returns an error when built without cgo.
func (p *Pipeline) Evaluate(ctx ClassificationContext) (string, error) {
	return "", errNoCgo
}

// Close is a no-op stub when built without cgo.
func (p *Pipeline) Close() {}

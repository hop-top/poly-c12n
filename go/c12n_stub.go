//go:build !c12n_native

package c12n

import "errors"

var errNativeDisabled = errors.New("c12n: native engine disabled (build with -tags c12n_native)")

// NewPipeline is a stub that returns an error unless built with -tags c12n_native.
func NewPipeline(cfg PipelineConfig) (*Pipeline, error) {
	return nil, errNativeDisabled
}

// Evaluate is a stub that returns an error unless built with -tags c12n_native.
func (p *Pipeline) Evaluate(ctx ClassificationContext) (string, error) {
	return "", errNativeDisabled
}

// Close is a no-op stub when built without the native engine.
func (p *Pipeline) Close() {}

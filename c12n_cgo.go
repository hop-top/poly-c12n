//go:build cgo

package c12n

// Requires libc12n_core built from the Rust c12n-core crate.
// Build: cd <repo-root> && cargo build (produces target/debug/libc12n_core.{so,dylib})

/*
#cgo LDFLAGS: -L${SRCDIR}/target/debug -lc12n_core
#include <stdlib.h>

extern void* c12n_pipeline_new(const char* config_json);
extern char* c12n_pipeline_evaluate(const void* pipeline, const char* context_json);
extern void c12n_pipeline_free(void* pipeline);
extern void c12n_result_free(char* result);
*/
import "C"

import (
	"encoding/json"
	"errors"
	"time"
	"unsafe"
)

// cgoConfig mirrors PipelineConfig for JSON marshalling to the C API.
type cgoConfig struct {
	MaxConcurrency int `json:"max_concurrency"`
	TimeoutMs      int `json:"timeout_ms"`
}

// NewPipeline creates a classification pipeline backed by the C library.
func NewPipeline(cfg PipelineConfig) (*Pipeline, error) {
	cc := cgoConfig{
		MaxConcurrency: cfg.MaxConcurrency,
		TimeoutMs:      int(cfg.Timeout / time.Millisecond),
	}

	data, err := json.Marshal(cc)
	if err != nil {
		return nil, err
	}

	cstr := C.CString(string(data))
	defer C.free(unsafe.Pointer(cstr))

	ptr := C.c12n_pipeline_new(cstr)
	if ptr == nil {
		return nil, errors.New("c12n: failed to create pipeline")
	}

	return &Pipeline{ptr: unsafe.Pointer(ptr)}, nil
}

// Evaluate classifies a context and returns the raw JSON result.
func (p *Pipeline) Evaluate(ctx ClassificationContext) (string, error) {
	p.mu.Lock()
	defer p.mu.Unlock()

	if p.closed {
		return "", errors.New("c12n: pipeline is closed")
	}

	cPtr, ok := p.ptr.(unsafe.Pointer)
	if !ok || cPtr == nil {
		return "", errors.New("c12n: pipeline not initialized")
	}

	normalizeContext(&ctx)

	data, err := json.Marshal(ctx)
	if err != nil {
		return "", err
	}

	cstr := C.CString(string(data))
	defer C.free(unsafe.Pointer(cstr))

	result := C.c12n_pipeline_evaluate(cPtr, cstr)
	if result == nil {
		return "", errors.New("c12n: evaluation returned nil")
	}
	defer C.c12n_result_free(result)

	return C.GoString(result), nil
}

// Close frees the underlying C resources.
func (p *Pipeline) Close() {
	p.mu.Lock()
	defer p.mu.Unlock()

	if p.closed {
		return
	}
	p.closed = true

	cPtr, ok := p.ptr.(unsafe.Pointer)
	if !ok || cPtr == nil {
		return
	}
	C.c12n_pipeline_free(cPtr)
}

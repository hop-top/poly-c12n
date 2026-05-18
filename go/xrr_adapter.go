package c12n

import (
	"context"
	"crypto/sha256"
	"encoding/json"
	"fmt"

	"hop.top/xrr"
)

const adapterID = "c12n-pipeline"

// EvaluateRequest wraps a ClassificationContext for xrr record/replay.
type EvaluateRequest struct {
	Ctx ClassificationContext `json:"ctx"`
}

// AdapterID implements xrr.Request.
func (r *EvaluateRequest) AdapterID() string { return adapterID }

// EvaluateResponse wraps a pipeline result string for xrr record/replay.
type EvaluateResponse struct {
	Result string `json:"result"`
}

// AdapterID implements xrr.Response.
func (r *EvaluateResponse) AdapterID() string { return adapterID }

// PipelineAdapter implements xrr.Adapter for Pipeline.Evaluate calls.
type PipelineAdapter struct{}

// ID implements xrr.Adapter.
func (a *PipelineAdapter) ID() string { return adapterID }

// Fingerprint implements xrr.Adapter. It hashes the request JSON to
// produce a deterministic key for cassette lookup.
func (a *PipelineAdapter) Fingerprint(req xrr.Request) (string, error) {
	data, err := json.Marshal(req)
	if err != nil {
		return "", fmt.Errorf("fingerprint marshal: %w", err)
	}
	h := sha256.Sum256(data)
	return fmt.Sprintf("%x", h[:8]), nil
}

// Serialize implements xrr.Adapter.
func (a *PipelineAdapter) Serialize(v any) ([]byte, error) {
	return json.Marshal(v)
}

// Deserialize implements xrr.Adapter.
func (a *PipelineAdapter) Deserialize(data []byte, target any) error {
	return json.Unmarshal(data, target)
}

// RecordablePipeline wraps a Pipeline with xrr session for
// record/replay of Evaluate calls.
type RecordablePipeline struct {
	inner   *Pipeline
	session xrr.Session
	adapter *PipelineAdapter
}

// NewRecordablePipeline wraps pipeline with xrr record/replay.
func NewRecordablePipeline(
	pipeline *Pipeline,
	session xrr.Session,
) *RecordablePipeline {
	return &RecordablePipeline{
		inner:   pipeline,
		session: session,
		adapter: &PipelineAdapter{},
	}
}

// Evaluate runs Pipeline.Evaluate through the xrr session.
func (rp *RecordablePipeline) Evaluate(
	ctx context.Context,
	input ClassificationContext,
) (string, error) {
	req := &EvaluateRequest{Ctx: input}

	resp, err := rp.session.Record(ctx, rp.adapter, req,
		func() (xrr.Response, error) {
			result, err := rp.inner.Evaluate(input)
			if err != nil {
				return nil, err
			}
			return &EvaluateResponse{Result: result}, nil
		},
	)
	if err != nil {
		return "", err
	}

	switch v := resp.(type) {
	case *EvaluateResponse:
		return v.Result, nil
	case *xrr.RawResponse:
		// Replay returns a RawResponse; extract the result field.
		raw, ok := v.Payload["result"]
		if !ok {
			return "", fmt.Errorf("xrr: missing result in replay payload")
		}
		s, ok := raw.(string)
		if !ok {
			return "", fmt.Errorf("xrr: result is not a string")
		}
		return s, nil
	default:
		return "", fmt.Errorf("xrr: unexpected response type %T", resp)
	}
}

// Close closes the underlying session.
func (rp *RecordablePipeline) Close() error {
	return rp.session.Close()
}

// NewTestPipeline creates a RecordablePipeline for testing.
// In record mode it calls the real pipeline; in replay mode it uses
// cassettes from dir.
func NewTestPipeline(
	pipeline *Pipeline,
	mode xrr.Mode,
	cassetteDir string,
) *RecordablePipeline {
	cassette := xrr.NewFileCassette(cassetteDir)
	session := xrr.NewSession(mode, cassette)
	return NewRecordablePipeline(pipeline, session)
}

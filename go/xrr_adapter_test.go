package c12n

import (
	"context"
	"testing"

	"hop.top/xrr"
)

func TestPipelineAdapter_ID(t *testing.T) {
	a := &PipelineAdapter{}
	if got := a.ID(); got != adapterID {
		t.Fatalf("ID() = %q, want %q", got, adapterID)
	}
}

func TestPipelineAdapter_Fingerprint_Deterministic(t *testing.T) {
	a := &PipelineAdapter{}
	req := &EvaluateRequest{Ctx: ClassificationContext{Text: "hello"}}

	fp1, err := a.Fingerprint(req)
	if err != nil {
		t.Fatal(err)
	}
	fp2, err := a.Fingerprint(req)
	if err != nil {
		t.Fatal(err)
	}
	if fp1 != fp2 {
		t.Fatalf("fingerprint not deterministic: %q != %q", fp1, fp2)
	}
}

func TestPipelineAdapter_Fingerprint_Varies(t *testing.T) {
	a := &PipelineAdapter{}
	r1 := &EvaluateRequest{Ctx: ClassificationContext{Text: "hello"}}
	r2 := &EvaluateRequest{Ctx: ClassificationContext{Text: "world"}}

	fp1, _ := a.Fingerprint(r1)
	fp2, _ := a.Fingerprint(r2)
	if fp1 == fp2 {
		t.Fatal("different inputs should produce different fingerprints")
	}
}

func TestPipelineAdapter_SerializeDeserialize(t *testing.T) {
	a := &PipelineAdapter{}
	orig := &EvaluateResponse{Result: `{"results":[]}`}

	data, err := a.Serialize(orig)
	if err != nil {
		t.Fatal(err)
	}

	var decoded EvaluateResponse
	if err := a.Deserialize(data, &decoded); err != nil {
		t.Fatal(err)
	}
	if decoded.Result != orig.Result {
		t.Fatalf("round-trip mismatch: %q != %q", decoded.Result, orig.Result)
	}
}

func TestRecordablePipeline_RecordReplay(t *testing.T) {
	dir := t.TempDir()
	ctx := context.Background()

	// The stub pipeline returns errNoCgo, so record will capture an error.
	// We verify the record/replay round-trip mechanics work.
	pipeline := &Pipeline{}

	// Record phase.
	rp := NewTestPipeline(pipeline, xrr.ModeRecord, dir)
	_, recordErr := rp.Evaluate(ctx, ClassificationContext{Text: "test"})
	// Expect error from stub pipeline.
	if recordErr == nil {
		t.Fatal("expected error from stub pipeline in record mode")
	}
	rp.Close()

	// Replay phase — should reproduce the same error without calling pipeline.
	rp2 := NewTestPipeline(pipeline, xrr.ModeReplay, dir)
	_, replayErr := rp2.Evaluate(ctx, ClassificationContext{Text: "test"})
	if replayErr == nil {
		t.Fatal("expected error from replay")
	}
	if replayErr.Error() != recordErr.Error() {
		t.Fatalf("replay error mismatch: %q != %q",
			replayErr.Error(), recordErr.Error())
	}
	rp2.Close()
}

func TestRecordablePipeline_ReplayCassetteMiss(t *testing.T) {
	dir := t.TempDir()
	ctx := context.Background()

	pipeline := &Pipeline{}
	rp := NewTestPipeline(pipeline, xrr.ModeReplay, dir)
	defer rp.Close()

	_, err := rp.Evaluate(ctx, ClassificationContext{Text: "missing"})
	if err == nil {
		t.Fatal("expected cassette miss error")
	}
}

func TestEvaluateRequest_AdapterID(t *testing.T) {
	r := &EvaluateRequest{}
	if r.AdapterID() != adapterID {
		t.Fatalf("AdapterID() = %q, want %q", r.AdapterID(), adapterID)
	}
}

func TestEvaluateResponse_AdapterID(t *testing.T) {
	r := &EvaluateResponse{}
	if r.AdapterID() != adapterID {
		t.Fatalf("AdapterID() = %q, want %q", r.AdapterID(), adapterID)
	}
}

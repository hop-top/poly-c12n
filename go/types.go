package c12n

// SignalType represents the type of classification signal.
type SignalType string

const (
	SignalKeyword      SignalType = "Keyword"
	SignalEmbedding    SignalType = "Embedding"
	SignalDomain       SignalType = "Domain"
	SignalJailbreak    SignalType = "Jailbreak"
	SignalPII          SignalType = "PII"
	SignalToxicity     SignalType = "Toxicity"
	SignalContext      SignalType = "Context"
	SignalStructure    SignalType = "Structure"
	SignalLanguage     SignalType = "Language"
	SignalComplexity   SignalType = "Complexity"
	SignalPreference   SignalType = "Preference"
	SignalFeedback     SignalType = "Feedback"
	SignalOutputFormat SignalType = "OutputFormat"
	SignalCodeContent  SignalType = "CodeContent"
	SignalToolCalling  SignalType = "ToolCalling"
	SignalCostEstimate SignalType = "CostEstimate"
	SignalSentiment    SignalType = "Sentiment"
	SignalIntent       SignalType = "Intent"
	SignalTopic        SignalType = "Topic"
	SignalCustom       SignalType = "Custom"
)

// SignalResult is a single signal's classification output.
type SignalResult struct {
	Name       string         `json:"name"`
	Type       SignalType     `json:"signal_type"`
	Confidence float64        `json:"confidence"`
	Labels     []string       `json:"labels"`
	Metadata   map[string]any `json:"metadata"`
}

// PipelineError is a diagnostic emitted by the pipeline, rendered by the core
// as a human-readable message. The wire format is a plain JSON string, e.g.
// "signal 'keyword' failed: model not found", "signal 'embedding' timed out",
// or "pipeline has no registered signals; results will be empty". The core does
// not emit a structured variant discriminator.
type PipelineError string

// Error implements the error interface so diagnostics can be wrapped or
// compared like any other error value.
func (e PipelineError) Error() string { return string(e) }

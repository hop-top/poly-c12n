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

// PipelineError represents an error from a single signal.
type PipelineError struct {
	SignalFailed *struct {
		Name  string `json:"name"`
		Error string `json:"error"`
	} `json:"SignalFailed,omitempty"`
	Timeout *struct {
		Name string `json:"name"`
	} `json:"Timeout,omitempty"`
}

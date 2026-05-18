use std::time::Duration;

/// Configuration for a [`crate::SdkPipeline`]. Mirrors the Go binding's
/// `PipelineConfig` shape — pure data, no signals (signals are added
/// via the [`SdkPipeline`] constructor / builder).
#[derive(Debug, Clone)]
pub struct PipelineConfig {
    pub max_concurrency: usize,
    pub timeout: Duration,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 8,
            timeout: Duration::from_secs(5),
        }
    }
}

impl PipelineConfig {
    pub fn builder() -> PipelineConfigBuilder {
        PipelineConfigBuilder::default()
    }
}

#[derive(Debug, Clone)]
pub struct PipelineConfigBuilder {
    cfg: PipelineConfig,
}

impl Default for PipelineConfigBuilder {
    fn default() -> Self {
        Self {
            cfg: PipelineConfig::default(),
        }
    }
}

impl PipelineConfigBuilder {
    pub fn max_concurrency(mut self, n: usize) -> Self {
        self.cfg.max_concurrency = n;
        self
    }

    pub fn timeout(mut self, t: Duration) -> Self {
        self.cfg.timeout = t;
        self
    }

    pub fn build(self) -> PipelineConfig {
        self.cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values() {
        let cfg = PipelineConfig::default();
        assert_eq!(cfg.max_concurrency, 8);
        assert_eq!(cfg.timeout, Duration::from_secs(5));
    }

    #[test]
    fn builder_overrides_defaults() {
        let cfg = PipelineConfig::builder()
            .max_concurrency(16)
            .timeout(Duration::from_secs(30))
            .build();
        assert_eq!(cfg.max_concurrency, 16);
        assert_eq!(cfg.timeout, Duration::from_secs(30));
    }
}

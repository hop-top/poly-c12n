use async_trait::async_trait;

use crate::types::{ClassificationContext, SignalError, SignalResult, SignalType};

#[async_trait]
pub trait Signal: Send + Sync {
    async fn evaluate(&self, ctx: &ClassificationContext) -> Result<SignalResult, SignalError>;
    fn name(&self) -> &str;
    fn signal_type(&self) -> SignalType;
}

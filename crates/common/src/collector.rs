use std::future::Future;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error("failed to parse {path}: {reason}")]
    Parse {
        path: &'static str,
        reason: String,
    },
}

pub trait Collector: Send + 'static {
    type Output: Send + 'static;

    fn name(&self) -> &'static str;

    fn interval(&self) -> Duration;

    fn collect(
        &mut self,
    ) -> impl Future<Output = Result<Self::Output, CollectorError>> + Send;
}
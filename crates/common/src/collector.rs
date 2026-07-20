//! The contract every data source in SysForge implements.

use std::future::Future;
use std::time::Duration;

use thiserror::Error;

/// Errors a collector can produce while sampling.
#[derive(Debug, Error)]
pub enum CollectorError {
    /// Reading the underlying source failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// The source was read but its contents were not understood.
    #[error("failed to parse {path}: {reason}")]
    Parse {
        /// What was being parsed (e.g. `/proc/meminfo`).
        path: &'static str,
        /// Why parsing failed.
        reason: String,
    },
}

/// A periodic producer of domain data.
///
/// Implementors write plain `async fn collect`; the desugared signature
/// below exists only to attach the `Send` bound that `tokio::spawn`
/// requires — something `async fn` in traits cannot express yet.
pub trait Collector: Send + 'static {
    /// The sample type this collector produces.
    type Output: Send + 'static;

    /// Name used in logs and diagnostics.
    fn name(&self) -> &'static str;

    /// How often [`Collector::collect`] should be called.
    fn interval(&self) -> Duration;

    /// Produces one sample.
    fn collect(&mut self) -> impl Future<Output = Result<Self::Output, CollectorError>> + Send;
}

//! Availability tracking for domains that can be offline.
//!
//! Domains backed by an external source (a socket, a subprocess) may be
//! legitimately unreachable. This helper wraps a domain's own result
//! into an "available or offline" status *and* logs only the
//! transitions between the two — never every sample.

/// Tracks the last known availability so only transitions are logged.
#[derive(Debug, Default)]
pub struct AvailabilityTracker {
    was_available: Option<bool>,
    /// The collector name, for log context.
    name: &'static str,
}

/// The outcome of one probe: the domain answered, or it could not be
/// reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Availability<T> {
    /// The source answered.
    Available(T),
    /// The source could not be reached.
    Unavailable {
        /// Short human-readable cause.
        reason: String,
    },
}

impl AvailabilityTracker {
    /// Creates a tracker labelling logs with `name`.
    #[must_use]
    pub fn new(name: &'static str) -> Self {
        Self { was_available: None, name }
    }

    /// Wraps a domain result into an [`Availability`], logging the
    /// online↔offline transition (and the first observation) but never
    /// every sample.
    pub fn wrap<T>(&mut self, result: Result<T, String>) -> Availability<T> {
        match result {
            Ok(value) => {
                self.note(true, "");
                Availability::Available(value)
            }
            Err(reason) => {
                self.note(false, &reason);
                Availability::Unavailable { reason }
            }
        }
    }

    fn note(&mut self, up: bool, detail: &str) {
        if self.was_available == Some(up) {
            return;
        }
        if up {
            tracing::info!(domain = self.name, "domain available");
        } else {
            tracing::warn!(domain = self.name, reason = detail, "domain unavailable");
        }
        self.was_available = Some(up);
    }
}
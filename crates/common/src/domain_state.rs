//! The UI lifecycle shared by every observable domain.

/// How the UI sees a domain: turned off, awaiting its first sample, or
/// showing the latest observation.
///
/// Generic over the domain's own observation type `T`. This models only
/// the *UI* lifecycle; whatever `T` is — including a domain's own
/// "available vs. offline" enum — stays the domain's concern.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DomainState<T> {
    /// Disabled in the configuration; the view is not shown.
    Disabled,
    /// Enabled, no sample yet.
    #[default]
    Pending,
    /// The latest observation from the collector.
    Observed(T),
}

impl<T> DomainState<T> {
    /// Creates the initial state: `Pending` if enabled, else `Disabled`.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        if enabled {
            Self::Pending
        } else {
            Self::Disabled
        }
    }

    /// Whether the domain is disabled.
    #[must_use]
    pub fn is_disabled(&self) -> bool {
        matches!(self, Self::Disabled)
    }

    /// The observation, if one has arrived.
    #[must_use]
    pub fn observed(&self) -> Option<&T> {
        match self {
            Self::Observed(value) => Some(value),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_reflects_enabled_flag() {
        assert_eq!(DomainState::<u8>::new(true), DomainState::Pending);
        assert_eq!(DomainState::<u8>::new(false), DomainState::Disabled);
        assert!(DomainState::<u8>::new(false).is_disabled());
    }

    #[test]
    fn observed_exposes_value() {
        let state = DomainState::Observed(42u8);
        assert_eq!(state.observed(), Some(&42));
        assert_eq!(DomainState::<u8>::Pending.observed(), None);
    }
}

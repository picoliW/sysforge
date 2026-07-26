//! Application state shared between the UI and background collectors.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use sysforge_common::availability::Availability;
use sysforge_common::domain_state::DomainState;
use sysforge_disk::collector::DiskSnapshot;
use sysforge_docker::collector::DockerSnapshot;
use sysforge_git::collector::GitStatus;
use sysforge_k8s::collector::K8sSnapshot;
use sysforge_network::collector::NetworkSnapshot;
use sysforge_system::cpu::CpuSnapshot;
use sysforge_system::memory::MemorySnapshot;
use sysforge_system::process::ProcessSnapshot;
use sysforge_systemd::collector::SystemdSnapshot;

use crate::history::History;

/// Docker domain as the UI sees it.
pub type DockerUiState = DomainState<Availability<DockerSnapshot>>;
/// Git domain as the UI sees it.
pub type GitUiState = DomainState<GitStatus>;
/// systemd domain as the UI sees it.
pub type SystemdUiState = DomainState<Availability<SystemdSnapshot>>;
/// Kubernetes domain as the UI sees it.
pub type K8sUiState = DomainState<Availability<K8sSnapshot>>;

/// Everything the UI needs in order to render a frame.
///
/// Collectors write to it; the render loop reads from it. No component
/// other than the render loop should ever *read* state for rendering
/// purposes, and the UI never *writes* domain data.
#[derive(Debug, Default, Clone)]
pub struct AppState {
    /// Latest CPU reading, `None` until the second sample arrives.
    pub cpu: Option<CpuSnapshot>,
    /// Aggregate CPU utilization over the last few minutes.
    pub cpu_history: History,
    /// Docker domain as last observed.
    pub docker: DockerUiState,
    /// Latest memory reading, `None` until the first sample arrives.
    pub memory: Option<MemorySnapshot>,
    /// Memory utilization over the last few minutes.
    pub memory_history: History,
    /// Latest process table reading.
    pub processes: Option<ProcessSnapshot>,
    /// Git domain as last observed.
    pub git: GitUiState,
    /// Latest network reading.
    pub network: Option<NetworkSnapshot>,
    /// Per-interface throughput history, keyed by interface name.
    pub network_history: HashMap<String, History>,
    /// Latest disk reading.
    pub disk: Option<DiskSnapshot>,
    /// Per-device I/O history, keyed by device name.
    pub disk_history: HashMap<String, History>,
    /// systemd domain as last observed.
    pub systemd: SystemdUiState,
    /// Kubernetes domain as last observed.
    pub k8s: K8sUiState,
}

/// Which optional domains are enabled, passed to [`AppState::new`].
///
/// A named struct instead of a row of `bool` parameters: the fields
/// can't be silently swapped, and adding a domain is one more named
/// field rather than one more positional flag.
///
/// These flags are genuinely independent on/off toggles, one per
/// optional domain — not a confused state that wants an enum or a
/// state machine, which is what `struct_excessive_bools` guards
/// against. Named bools are the right representation here.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy)]
pub struct DomainsEnabled {
    /// Docker collector enabled.
    pub docker: bool,
    /// Git collector enabled.
    pub git: bool,
    /// systemd collector enabled.
    pub systemd: bool,
    /// Kubernetes collector enabled.
    pub k8s: bool,
}

impl AppState {
    /// Creates an empty state with the configured history retention.
    #[must_use]
    pub fn new(history_capacity: usize, enabled: DomainsEnabled) -> Self {
        Self {
            cpu: None,
            cpu_history: History::new(history_capacity),
            docker: DomainState::new(enabled.docker),
            memory: None,
            memory_history: History::new(history_capacity),
            processes: None,
            git: DomainState::new(enabled.git),
            network: None,
            network_history: HashMap::new(),
            disk: None,
            disk_history: HashMap::new(),
            systemd: DomainState::new(enabled.systemd),
            k8s: DomainState::new(enabled.k8s),
        }
    }
}

/// Shared, thread-safe handle to [`AppState`].
pub type SharedState = Arc<RwLock<AppState>>;

use std::sync::{Arc, RwLock};

use std::collections::HashMap;
use sysforge_disk::collector::DiskSnapshot;
use sysforge_docker::collector::DockerStatus;
use sysforge_git::collector::GitStatus;
use sysforge_network::collector::NetworkSnapshot;
use sysforge_system::cpu::CpuSnapshot;
use sysforge_system::memory::MemorySnapshot;
use sysforge_system::process::ProcessSnapshot;

use crate::history::History;

#[derive(Debug, Clone, Default, PartialEq)]
pub enum DockerUiState {
    Disabled,
    #[default]
    Pending,
    Observed(DockerStatus),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum GitUiState {
    Disabled,
    #[default]
    Pending,
    Observed(GitStatus),
}

#[derive(Debug, Default, Clone)]
pub struct AppState {
    pub cpu: Option<CpuSnapshot>,
    pub cpu_history: History,
    pub docker: DockerUiState,
    pub memory: Option<MemorySnapshot>,
    pub memory_history: History,
    pub processes: Option<ProcessSnapshot>,
    pub git: GitUiState,
    pub network: Option<NetworkSnapshot>,
    pub network_history: HashMap<String, History>,
    pub disk: Option<DiskSnapshot>,
    pub disk_history: HashMap<String, History>,
}

impl AppState {
    #[must_use]
    pub fn new(history_capacity: usize, docker_enabled: bool, git_enabled: bool) -> Self {
        Self {
            cpu: None,
            cpu_history: History::new(history_capacity),
            docker: if docker_enabled {
                DockerUiState::Pending
            } else {
                DockerUiState::Disabled
            },
            memory: None,
            memory_history: History::new(history_capacity),
            processes: None,
            git: if git_enabled {
                GitUiState::Pending
            } else {
                GitUiState::Disabled
            },
            network: None,
            network_history: HashMap::new(),
            disk: None,
            disk_history: HashMap::new(),
        }
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

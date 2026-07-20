use std::sync::{Arc, RwLock};

use sysforge_docker::collector::DockerStatus;
use sysforge_git::collector::GitStatus;
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
        }
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

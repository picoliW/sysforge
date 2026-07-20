use std::sync::{Arc, RwLock};

use sysforge_system::cpu::CpuSnapshot;
use sysforge_system::memory::MemorySnapshot;

use crate::history::History;

#[derive(Debug, Default, Clone)]
pub struct AppState {
    pub cpu: Option<CpuSnapshot>,
    pub cpu_history: History,
    pub memory: Option<MemorySnapshot>,
    pub memory_history: History,
}

impl AppState {
    #[must_use]
    pub fn new(history_capacity: usize) -> Self {
        Self {
            cpu: None,
            cpu_history: History::new(history_capacity),
            memory: None,
            memory_history: History::new(history_capacity),
        }
    }
}

pub type SharedState = Arc<RwLock<AppState>>;

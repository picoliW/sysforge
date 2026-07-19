use std::sync::{Arc, RwLock};

use sysforge_system::memory::MemorySnapshot;

#[derive(Debug, Default)]
pub struct AppState {
    pub memory: Option<MemorySnapshot>,
}

pub type SharedState = Arc<RwLock<AppState>>;
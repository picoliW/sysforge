use std::sync::{Arc, RwLock};

#[derive(Debug, Default)]
pub struct AppState {
    pub ticks: u64,
}

pub type SharedState = Arc<RwLock<AppState>>;
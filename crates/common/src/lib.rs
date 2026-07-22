//! Shared types, traits and error handling for all SysForge crates.
//!
//! Every domain crate (`system`, `docker`, ...) depends on this crate
//! and **only** on this crate within the workspace. Cross-domain
//! communication happens through types defined here.

pub mod availability;
pub mod collector;
pub mod domain_state;
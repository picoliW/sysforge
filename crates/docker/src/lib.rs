//! Docker Engine integration for SysForge.
//!
//! Docker is treated as a domain that may be legitimately offline:
//! a missing socket, stopped daemon or denied permission is reported
//! as observable state, never as a fatal error.

pub mod collector;
pub mod config;
pub mod logs;

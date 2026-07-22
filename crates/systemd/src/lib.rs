//! systemd integration for SysForge.
//!
//! Reads service units by invoking `systemctl --output=json` and
//! parsing the structured output. A missing systemctl or a system
//! without systemd as init is reported as observable state, never as
//! a fatal error.

pub mod collector;
pub mod config;

//! Git repository integration for SysForge.
//!
//! Reads the working-directory repository by invoking the `git` binary
//! and parsing its machine-readable output. A missing binary or a
//! non-repository directory is reported as observable state, never as
//! a fatal error — mirroring how Docker is treated when offline.

pub mod collector;
pub mod config;

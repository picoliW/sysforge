//! Network interface integration for SysForge.
//!
//! Reads `/proc/net/dev` and derives per-interface throughput from the
//! delta between two samples — combining the stateful-delta pattern of
//! the CPU collector with the multi-entity nature of processes.

pub mod collector;
pub mod config;

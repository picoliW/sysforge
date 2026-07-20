//! Disk integration for SysForge.
//!
//! Combines two sources into one snapshot: `/proc/diskstats` for
//! per-device I/O throughput (derived from deltas, like the network
//! domain) and `statvfs` over mount points for filesystem usage
//! (a direct reading, like memory).

pub mod collector;
pub mod config;

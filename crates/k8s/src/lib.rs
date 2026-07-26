//! Kubernetes integration for SysForge.
//!
//! Talks to the cluster of the current kubeconfig context via
//! [`kube`], the standard async Kubernetes client. Like every external
//! domain, an unreachable or unconfigured cluster is reported as
//! observable state (`Unavailable`), never as a fatal error.
//!
//! This crate is scaffolded in step 25.1 (stack choice); the read-only
//! Pod collector arrives in 25.2.

pub mod config;

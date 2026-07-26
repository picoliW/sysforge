//! Kubernetes integration for SysForge.
//!
//! Talks to the cluster of the current kubeconfig context via
//! [`kube`], the standard async Kubernetes client. Like every external
//! domain, an unreachable or unconfigured cluster is reported as
//! observable state (`Unavailable`), never as a fatal error.
//!
//! The `kube` and `k8s-openapi` types never leave this crate: the
//! collector converts each `Pod` into a plain [`collector::PodInfo`]
//! immediately, so the rest of SysForge never depends on the client
//! library — exactly as the Docker domain hides bollard.

pub mod collector;
pub mod config;

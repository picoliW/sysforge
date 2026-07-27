//! Pod observation via a Kubernetes watch, exposed as a pull collector.
//!
//! Internally this collector runs a `kube` watcher + reflector on a
//! background task: the cluster *pushes* pod changes into a
//! library-maintained store (reconnection and `resourceVersion`
//! handling included), and `collect()` merely snapshots that store.
//! To the runtime this is an ordinary pull [`Collector`] — the push
//! nature never leaves this crate, exactly as the bollard stream never
//! leaves the docker crate.
//!
//! Each pod is converted into a plain [`PodInfo`] before anything else
//! in SysForge sees it. The status shown reproduces `kubectl`'s
//! composite logic (reading container states), not the raw pod
//! `phase`, which can mislead (a crash-looping pod still reports
//! `phase: Running`).

use std::time::Duration;

use futures::StreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use kube::api::Api;
use kube::runtime::reflector::Store;
use kube::runtime::{WatchStreamExt, reflector, watcher};
use sysforge_common::availability::{Availability, AvailabilityTracker};
use sysforge_common::collector::{Collector, CollectorError};
use tokio::task::JoinHandle;

/// One pod as shown in the UI. A SysForge model, not a `kube` type:
/// the client library never leaves this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PodInfo {
    /// Pod name.
    pub name: String,
    /// Namespace the pod lives in.
    pub namespace: String,
    /// Displayed status, following `kubectl` semantics
    /// (`Running`, `CrashLoopBackOff`, `ImagePullBackOff`, ...).
    pub status: String,
    /// Containers reporting ready.
    pub ready: usize,
    /// Total containers in the pod.
    pub total: usize,
    /// Sum of container restarts.
    pub restarts: i32,
}

/// One reading of the cluster's pods.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct K8sSnapshot {
    /// Pods, not-ready first (so problems surface), then by namespace/name.
    pub pods: Vec<PodInfo>,
    /// How many pods are fully ready.
    pub ready_pods: usize,
    /// Total pods observed.
    pub total_pods: usize,
}

/// How long the first full pod list may take before the cluster is
/// declared unreachable for this attempt.
const READY_TIMEOUT: Duration = Duration::from_secs(10);

/// The running watch machinery: the library-maintained store plus the
/// background task that keeps it fed. Dropping it aborts the task, so
/// a watch never outlives its collector.
struct Watch {
    /// Read handle over the reflector's cache of pods.
    store: Store<Pod>,
    /// The engine: polls the watch stream so the store stays fresh.
    task: JoinHandle<()>,
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Observes pods through a background watch, snapshotting the local
/// store at a fixed interval. No network call happens per tick — the
/// network side is the continuous, delta-only watch.
pub struct K8sCollector {
    interval: Duration,
    availability: AvailabilityTracker,
    watch: Option<Watch>,
}

impl K8sCollector {
    /// Creates a collector snapshotting the store at the given interval.
    /// The watch itself starts lazily on the first `collect` call.
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            availability: AvailabilityTracker::new("k8s"),
            watch: None,
        }
    }

    /// Ensures the watch is running, then snapshots the store.
    async fn observe(&mut self) -> Result<K8sSnapshot, String> {
        let needs_start = match &self.watch {
            Some(watch) => watch.task.is_finished(),
            None => true,
        };
        let watch = if needs_start {
            // Dropping a finished/stale watch aborts its task first.
            self.watch = None;
            self.watch.insert(start_watch().await?)
        } else {
            // The match above guarantees a live watch on this branch.
            self.watch
                .as_ref()
                .ok_or_else(|| String::from("watch state lost"))?
        };
        let pods = watch
            .store
            .state()
            .iter()
            .map(|pod| to_pod_info(pod.as_ref()))
            .collect();
        Ok(build_snapshot(pods))
    }
}

impl Collector for K8sCollector {
    type Output = Availability<K8sSnapshot>;

    fn name(&self) -> &'static str {
        "k8s"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    async fn collect(&mut self) -> Result<Availability<K8sSnapshot>, CollectorError> {
        let result = self.observe().await;
        Ok(self.availability.wrap(result))
    }
}

/// Connects to the cluster and starts the watch + reflector, waiting
/// (bounded) for the first full list so the store never reads as an
/// empty cluster. On failure the engine task is aborted before
/// returning, so nothing is leaked.
async fn start_watch() -> Result<Watch, String> {
    let client = Client::try_default()
        .await
        .map_err(|e| format!("connecting to cluster: {e}"))?;
    let api: Api<Pod> = Api::all(client);

    let (store, writer) = reflector::store();
    let stream = watcher(api, watcher::Config::default())
        .default_backoff()
        .reflect(writer)
        .applied_objects()
        .boxed();

    // Streams are lazy: this task is the engine that keeps the store
    // fresh. Transient errors are the watcher's business (it backs off
    // and reconnects); we only log them for observability.
    let task = tokio::spawn(async move {
        let mut stream = stream;
        while let Some(event) = stream.next().await {
            if let Err(err) = event {
                tracing::debug!(error = %err, "k8s watch error; backing off");
            }
        }
    });

    match tokio::time::timeout(READY_TIMEOUT, store.wait_until_ready()).await {
        Ok(Ok(())) => Ok(Watch { store, task }),
        Ok(Err(err)) => {
            task.abort();
            Err(format!("watch failed to start: {err}"))
        }
        Err(_) => {
            task.abort();
            Err(String::from("timed out reaching the cluster"))
        }
    }
}

/// Sorts and counts a set of pods into a snapshot.
fn build_snapshot(mut pods: Vec<PodInfo>) -> K8sSnapshot {
    let ready_pods = pods
        .iter()
        .filter(|p| p.ready == p.total && p.total > 0)
        .count();
    let total_pods = pods.len();
    // Not-ready pods first, so problems are visible at the top.
    pods.sort_by(|a, b| {
        let a_ready = a.ready == a.total && a.total > 0;
        let b_ready = b.ready == b.total && b.total > 0;
        a_ready
            .cmp(&b_ready)
            .then_with(|| a.namespace.cmp(&b.namespace))
            .then_with(|| a.name.cmp(&b.name))
    });
    K8sSnapshot {
        pods,
        ready_pods,
        total_pods,
    }
}

/// Converts a `kube` [`Pod`] into a plain [`PodInfo`]. This is the only
/// function that reads the API type; everything else works on `PodInfo`.
fn to_pod_info(pod: &Pod) -> PodInfo {
    let name = pod.metadata.name.clone().unwrap_or_default();
    let namespace = pod.metadata.namespace.clone().unwrap_or_default();
    let (ready, total, restarts) = container_readiness(pod);
    let status = derive_status(pod);
    PodInfo {
        name,
        namespace,
        status,
        ready,
        total,
        restarts,
    }
}

/// Ready count, total count and total restarts from container statuses.
fn container_readiness(pod: &Pod) -> (usize, usize, i32) {
    let statuses = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref());
    let Some(statuses) = statuses else {
        return (0, 0, 0);
    };
    let ready = statuses.iter().filter(|c| c.ready).count();
    let total = statuses.len();
    let restarts = statuses.iter().map(|c| c.restart_count).sum();
    (ready, total, restarts)
}

/// Reproduces `kubectl`'s displayed status.
///
/// `kubectl` does not show the raw `phase`. It scans container states:
/// a container `waiting` with a reason (`CrashLoopBackOff`,
/// `ImagePullBackOff`, ...) makes that reason the pod's status; an
/// abnormal `terminated` reason likewise. Only if nothing overrides
/// does it fall back to the phase. A deleted pod shows `Terminating`.
fn derive_status(pod: &Pod) -> String {
    // A pod with a deletion timestamp is being torn down.
    if pod.metadata.deletion_timestamp.is_some() {
        return String::from("Terminating");
    }

    let phase = pod
        .status
        .as_ref()
        .and_then(|s| s.phase.clone())
        .unwrap_or_else(|| String::from("Unknown"));

    let Some(statuses) = pod
        .status
        .as_ref()
        .and_then(|s| s.container_statuses.as_ref())
    else {
        return phase;
    };

    // A waiting or abnormally-terminated container overrides the phase.
    for cs in statuses {
        if let Some(state) = &cs.state {
            if let Some(waiting) = &state.waiting {
                if let Some(reason) = &waiting.reason {
                    // "ContainerCreating" and "PodInitializing" are normal
                    // transient states; keep them, they are informative.
                    return reason.clone();
                }
            }
            if let Some(terminated) = &state.terminated {
                if let Some(reason) = &terminated.reason {
                    if reason != "Completed" {
                        return reason.clone();
                    }
                }
            }
        }
    }

    phase
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{
        ContainerState, ContainerStateWaiting, ContainerStatus, PodStatus,
    };
    use kube::api::ObjectMeta;

    /// Builds a Pod with the given container statuses and phase.
    fn pod_with(phase: &str, statuses: Vec<ContainerStatus>) -> Pod {
        Pod {
            metadata: ObjectMeta {
                name: Some("test-pod".to_owned()),
                namespace: Some("default".to_owned()),
                ..Default::default()
            },
            status: Some(PodStatus {
                phase: Some(phase.to_owned()),
                container_statuses: Some(statuses),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    fn container(ready: bool, restarts: i32) -> ContainerStatus {
        ContainerStatus {
            name: "c".to_owned(),
            ready,
            restart_count: restarts,
            image: "img".to_owned(),
            image_id: String::new(),
            ..Default::default()
        }
    }

    fn waiting_container(reason: &str) -> ContainerStatus {
        let mut c = container(false, 5);
        c.state = Some(ContainerState {
            waiting: Some(ContainerStateWaiting {
                reason: Some(reason.to_owned()),
                message: None,
            }),
            ..Default::default()
        });
        c
    }

    #[test]
    fn readiness_counts_ready_over_total() {
        let pod = pod_with(
            "Running",
            vec![container(true, 0), container(false, 2), container(true, 1)],
        );
        let (ready, total, restarts) = container_readiness(&pod);
        assert_eq!(ready, 2);
        assert_eq!(total, 3);
        assert_eq!(restarts, 3);
    }

    #[test]
    fn running_pod_shows_running() {
        let pod = pod_with("Running", vec![container(true, 0)]);
        assert_eq!(derive_status(&pod), "Running");
    }

    #[test]
    fn crash_loop_overrides_running_phase() {
        // The lie kubectl corrects: phase Running, but a container is
        // stuck waiting in CrashLoopBackOff.
        let pod = pod_with("Running", vec![waiting_container("CrashLoopBackOff")]);
        assert_eq!(derive_status(&pod), "CrashLoopBackOff");
    }

    #[test]
    fn image_pull_error_is_shown() {
        let pod = pod_with("Pending", vec![waiting_container("ImagePullBackOff")]);
        assert_eq!(derive_status(&pod), "ImagePullBackOff");
    }

    #[test]
    fn not_ready_pods_sort_first() {
        let ready = PodInfo {
            name: "a".to_owned(),
            namespace: "default".to_owned(),
            status: "Running".to_owned(),
            ready: 1,
            total: 1,
            restarts: 0,
        };
        let broken = PodInfo {
            name: "b".to_owned(),
            namespace: "default".to_owned(),
            status: "CrashLoopBackOff".to_owned(),
            ready: 0,
            total: 1,
            restarts: 9,
        };
        let snap = build_snapshot(vec![ready.clone(), broken.clone()]);
        assert_eq!(snap.pods[0].name, "b"); // broken first
        assert_eq!(snap.ready_pods, 1);
        assert_eq!(snap.total_pods, 2);
    }
}

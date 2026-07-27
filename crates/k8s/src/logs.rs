//! Pod log retrieval — a one-shot snapshot of the most recent lines,
//! mirroring how the Docker domain fetches container logs. Not a live
//! follow; the `kube` client stays inside this crate.

use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use kube::api::{Api, LogParams};

/// How many trailing log lines to fetch.
const TAIL_LINES: i64 = 200;

/// Fetches the most recent log lines of a pod, newest last.
///
/// # Errors
/// Returns `Err` with a display message if the cluster rejects the
/// request or cannot be reached.
pub async fn fetch_logs(namespace: &str, name: &str) -> Result<Vec<String>, String> {
    let client = Client::try_default()
        .await
        .map_err(|e| format!("connecting to cluster: {e}"))?;
    let api: Api<Pod> = Api::namespaced(client, namespace);
    let params = LogParams {
        tail_lines: Some(TAIL_LINES),
        ..Default::default()
    };
    let raw = api
        .logs(name, &params)
        .await
        .map_err(|e| format!("fetching logs: {e}"))?;
    Ok(raw.lines().map(str::to_owned).collect())
}

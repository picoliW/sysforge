//! Pod actions. The write side of the Kubernetes domain: like the
//! collector, it keeps the `kube` client inside this crate. The
//! application only ever sees `Result<(), String>`.

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::Pod;
use kube::Client;
use kube::api::{Api, DeleteParams, Patch, PatchParams};

/// Deletes a pod. A bare pod stays gone; a managed pod (owned by a
/// Deployment/ReplicaSet) is recreated by its controller.
///
/// # Errors
/// Returns `Err` with a display message if the cluster rejects the
/// request or cannot be reached.
pub async fn delete_pod(namespace: &str, name: &str) -> Result<(), String> {
    let client = Client::try_default()
        .await
        .map_err(|e| format!("connecting to cluster: {e}"))?;
    let api: Api<Pod> = Api::namespaced(client, namespace);
    api.delete(name, &DeleteParams::default())
        .await
        .map(|_| ())
        .map_err(|e| format!("deleting pod: {e}"))
}

/// Requests a rolling restart of a deployment by stamping the standard
/// `restartedAt` annotation — the same mechanism as
/// `kubectl rollout restart`. Returns as soon as the request is
/// accepted; it does not wait for the rollout to converge.
///
/// # Errors
/// Returns `Err` with a display message if the cluster rejects the
/// patch or cannot be reached.
pub async fn rollout_restart(namespace: &str, deployment: &str) -> Result<(), String> {
    let client = Client::try_default()
        .await
        .map_err(|e| format!("connecting to cluster: {e}"))?;
    let api: Api<Deployment> = Api::namespaced(client, namespace);
    let now = chrono::Utc::now().to_rfc3339();
    let patch = serde_json::json!({
        "spec": {
            "template": {
                "metadata": {
                    "annotations": {
                        "kubectl.kubernetes.io/restartedAt": now
                    }
                }
            }
        }
    });
    api.patch(deployment, &PatchParams::default(), &Patch::Merge(&patch))
        .await
        .map(|_| ())
        .map_err(|e| format!("restarting deployment: {e}"))
}

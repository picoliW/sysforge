//! Container actions. The write side of the Docker domain: like the
//! collector, it encapsulates the bollard client so the application
//! never sees it.

use bollard::Docker;
use bollard::container::RestartContainerOptions;

/// Restarts a container by id. Returns a human-readable error on
/// failure (no such container, daemon unreachable, ...).
///
/// # Errors
/// Returns `Err` with a display message if the daemon rejects the
/// request or cannot be reached.
pub async fn restart(socket: &str, id: &str) -> Result<(), String> {
    let docker = Docker::connect_with_socket(socket, 120, bollard::API_DEFAULT_VERSION)
        .map_err(|e| format!("connecting to Docker: {e}"))?;
    docker
        .restart_container(id, None::<RestartContainerOptions>)
        .await
        .map_err(|e| format!("restarting container: {e}"))
}

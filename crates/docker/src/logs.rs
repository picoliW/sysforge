use bollard::Docker;
use bollard::container::LogsOptions;
use futures::StreamExt;

const TAIL_LINES: usize = 200;

const CONNECT_TIMEOUT_SECS: u64 = 5;

pub async fn fetch_logs(socket: &str, container_id: &str) -> Result<Vec<String>, String> {
    let client = Docker::connect_with_unix(
        socket,
        CONNECT_TIMEOUT_SECS,
        bollard::API_DEFAULT_VERSION,
    )
    .map_err(|e| format!("connect: {e}"))?;

    let options = LogsOptions::<String> {
        stdout: true,
        stderr: true,
        tail: TAIL_LINES.to_string(),
        ..Default::default()
    };

    let mut stream = client.logs(container_id, Some(options));
    let mut lines = Vec::new();
    while let Some(item) = stream.next().await {
        let output = item.map_err(|e| format!("read logs: {e}"))?;
        for line in output.to_string().lines() {
            lines.push(line.to_owned());
        }
    }
    Ok(lines)
}
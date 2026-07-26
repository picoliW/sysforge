//! Service actions via `systemctl`. The write side of the systemd
//! domain. A privilege failure is turned into a clear message rather
//! than an opaque error — SysForge never escalates on its own.

use tokio::process::Command;

/// A systemctl verb that changes a unit's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    /// `systemctl start`.
    Start,
    /// `systemctl stop`.
    Stop,
    /// `systemctl restart`.
    Restart,
}

impl Verb {
    fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Restart => "restart",
        }
    }
}

/// Runs `systemctl <verb> <unit>`, returning a clear error on failure.
///
/// A permission failure (the common case when SysForge runs unprivileged
/// against system units) becomes a message telling the user to run with
/// privileges — SysForge never tries to elevate on its own.
///
/// # Errors
/// Returns `Err` with a display message if systemctl cannot be run or
/// the operation fails.
pub async fn run(verb: Verb, unit: &str) -> Result<(), String> {
    let output = Command::new("systemctl")
        .args([verb.as_str(), unit])
        .output()
        .await
        .map_err(|e| format!("running systemctl: {e}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    // systemctl signals a privilege problem via specific wording or a
    // non-zero exit tied to authentication.
    if is_permission_error(stderr, output.status.code()) {
        return Err(
            "Permission denied.\n\nThis operation requires elevated privileges. \
             Run SysForge with the required permissions, or run the command manually."
                .to_owned(),
        );
    }

    Err(if stderr.is_empty() {
        format!("systemctl {} failed", verb.as_str())
    } else {
        stderr.to_owned()
    })
}

/// Whether the failure looks like a privilege problem.
fn is_permission_error(stderr: &str, code: Option<i32>) -> bool {
    let lower = stderr.to_lowercase();
    lower.contains("permission denied")
        || lower.contains("access denied")
        || lower.contains("authentication required")
        || lower.contains("interactive authentication required")
        || code == Some(1) && lower.contains("failed to")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verb_maps_to_systemctl_command() {
        assert_eq!(Verb::Start.as_str(), "start");
        assert_eq!(Verb::Stop.as_str(), "stop");
        assert_eq!(Verb::Restart.as_str(), "restart");
    }

    #[test]
    fn permission_wording_is_detected() {
        assert!(is_permission_error("Access denied", None));
        assert!(is_permission_error(
            "Interactive authentication required.",
            Some(1)
        ));
        assert!(!is_permission_error(
            "Unit nginx.service not found.",
            Some(5)
        ));
    }
}

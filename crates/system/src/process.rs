//! Process sampling via `/proc/[pid]`.
//!
//! The first collector over hundreds of short-lived entities: PIDs
//! appear and vanish between ticks, so every per-process read tolerates
//! the process being gone, and per-PID CPU deltas are kept in a map
//! rebuilt on each sample (dead PIDs are pruned for free).

use std::collections::HashMap;
use std::fs;
use std::time::Duration;

use sysforge_common::collector::{Collector, CollectorError};

/// How many processes the snapshot carries.
const TOP_N: usize = 20;

/// One process as shown in the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    /// Kernel process id.
    pub pid: i32,
    /// Executable name (`comm`).
    pub name: String,
    /// One-letter kernel state (R, S, D, Z, ...).
    pub state: char,
    /// CPU utilization; 100% is one full core. `None` on first sight.
    pub cpu_percent: Option<f64>,
    /// Resident memory (`VmRSS`), in bytes. Zero for kernel threads.
    pub memory: u64,
}

/// One reading of the process table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessSnapshot {
    /// Top processes by CPU, then memory.
    pub processes: Vec<ProcessInfo>,
    /// How many processes existed at sampling time.
    pub total: usize,
}

/// One raw per-process reading.
struct RawProcess {
    pid: i32,
    name: String,
    state: char,
    jiffies: u64,
    memory: u64,
}

/// Everything one `/proc` sweep yields.
struct Scan {
    total_jiffies: u64,
    cpus: u64,
    processes: Vec<RawProcess>,
}

/// The previous sample, kept for CPU deltas.
struct PrevSample {
    total_jiffies: u64,
    per_pid: HashMap<i32, u64>,
}

/// Samples the process table at a configurable interval.
pub struct ProcessCollector {
    interval: Duration,
    previous: Option<PrevSample>,
}

impl ProcessCollector {
    /// Creates a collector sampling at the given interval.
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            previous: None,
        }
    }
}

impl Collector for ProcessCollector {
    type Output = ProcessSnapshot;

    fn name(&self) -> &'static str {
        "process"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    async fn collect(&mut self) -> Result<ProcessSnapshot, CollectorError> {
        let scan =
            tokio::task::spawn_blocking(scan_proc)
                .await
                .map_err(|e| CollectorError::Parse {
                    path: "/proc",
                    reason: format!("scan task failed: {e}"),
                })??;

        let (snapshot, previous) = digest(self.previous.as_ref(), scan);
        self.previous = Some(previous);
        Ok(snapshot)
    }
}

/// Reads the system totals and every readable `/proc/[pid]`.
fn scan_proc() -> Result<Scan, CollectorError> {
    let stat = fs::read_to_string("/proc/stat")?;
    let (total_jiffies, cpus) = parse_system_stat(&stat)?;

    let mut processes = Vec::new();
    for entry in fs::read_dir("/proc")? {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name();
        let Some(pid) = name.to_str().and_then(|s| s.parse::<i32>().ok()) else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some((name, state, jiffies)) = parse_process_stat(&stat) else {
            continue;
        };
        let memory = fs::read_to_string(entry.path().join("status"))
            .ok()
            .and_then(|s| parse_vm_rss(&s))
            .unwrap_or(0);
        processes.push(RawProcess {
            pid,
            name,
            state,
            jiffies,
            memory,
        });
    }
    Ok(Scan {
        total_jiffies,
        cpus,
        processes,
    })
}

/// Total accumulated jiffies and core count from `/proc/stat`.
fn parse_system_stat(raw: &str) -> Result<(u64, u64), CollectorError> {
    let mut total = None;
    let mut cpus = 0;
    for line in raw.lines() {
        let mut fields = line.split_whitespace();
        match fields.next() {
            Some("cpu") => {
                total = Some(fields.take(8).filter_map(|f| f.parse::<u64>().ok()).sum());
            }
            Some(label) if label.starts_with("cpu") => cpus += 1,
            _ => break,
        }
    }
    let total = total.ok_or(CollectorError::Parse {
        path: "/proc/stat",
        reason: String::from("aggregate `cpu` line missing"),
    })?;
    Ok((total, cpus.max(1)))
}

/// Parses `pid (comm) state ... utime stime ...`.
///
/// `comm` may itself contain spaces and parentheses, so the parse
/// anchors on the *last* `)` before splitting the remainder.
fn parse_process_stat(raw: &str) -> Option<(String, char, u64)> {
    let open = raw.find('(')?;
    let close = raw.rfind(')')?;
    let name = raw.get(open + 1..close)?.to_owned();
    let rest: Vec<&str> = raw.get(close + 1..)?.split_whitespace().collect();
    let state = rest.first()?.chars().next()?;
    let utime: u64 = rest.get(11)?.parse().ok()?;
    let stime: u64 = rest.get(12)?.parse().ok()?;
    Some((name, state, utime + stime))
}

/// Extracts `VmRSS` (kB, despite meaning KiB) from `/proc/[pid]/status`.
/// Kernel threads have no `VmRSS` line.
fn parse_vm_rss(raw: &str) -> Option<u64> {
    let line = raw.lines().find(|l| l.starts_with("VmRSS:"))?;
    let kib: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
    Some(kib * 1024)
}

/// Pure derivation: raw scan + previous sample -> UI snapshot + the
/// sample to keep. Unit-testable without `/proc`.
fn digest(previous: Option<&PrevSample>, scan: Scan) -> (ProcessSnapshot, PrevSample) {
    let Scan {
        total_jiffies,
        cpus,
        processes,
    } = scan;
    let total = processes.len();
    let total_delta = previous.map(|p| total_jiffies.saturating_sub(p.total_jiffies));
    let per_pid: HashMap<i32, u64> = processes.iter().map(|r| (r.pid, r.jiffies)).collect();

    let mut infos: Vec<ProcessInfo> = processes
        .into_iter()
        .map(|raw| {
            let cpu_percent = match (previous, total_delta) {
                (Some(prev), Some(delta)) => prev
                    .per_pid
                    .get(&raw.pid)
                    .map(|before| cpu_percent(raw.jiffies.saturating_sub(*before), delta, cpus)),
                _ => None,
            };
            ProcessInfo {
                pid: raw.pid,
                name: raw.name,
                state: raw.state,
                cpu_percent,
                memory: raw.memory,
            }
        })
        .collect();

    infos.sort_by(|a, b| {
        b.cpu_percent
            .partial_cmp(&a.cpu_percent)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.memory.cmp(&a.memory))
    });
    infos.truncate(TOP_N);

    (
        ProcessSnapshot {
            processes: infos,
            total,
        },
        PrevSample {
            total_jiffies,
            per_pid,
        },
    )
}

/// htop's convention: 100% is one full core.
#[allow(clippy::cast_precision_loss)]
fn cpu_percent(proc_delta: u64, total_delta: u64, cpus: u64) -> f64 {
    if total_delta == 0 {
        return 0.0;
    }
    proc_delta as f64 / total_delta as f64 * cpus as f64 * 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hostile_comm_names() {
        let raw = "42 (we (ird) name) R 1 1 1 0 -1 0 0 0 0 0 7 3 0 0 20 0 1 0 100 0 0";
        let (name, state, jiffies) = parse_process_stat(raw).expect("hostile comm must parse");
        assert_eq!(name, "we (ird) name");
        assert_eq!(state, 'R');
        assert_eq!(jiffies, 10);
    }

    #[test]
    fn vm_rss_is_converted_to_bytes() {
        let raw = "Name:\tx\nVmRSS:\t     4 kB\nThreads:\t1\n";
        assert_eq!(parse_vm_rss(raw), Some(4096));
    }

    #[test]
    fn first_sighting_has_no_cpu_and_second_does() {
        let scan = |jiffies, total| Scan {
            total_jiffies: total,
            cpus: 1,
            processes: vec![RawProcess {
                pid: 7,
                name: String::from("p"),
                state: 'S',
                jiffies,
                memory: 0,
            }],
        };
        let (first, prev) = digest(None, scan(100, 1_000));
        assert_eq!(first.processes[0].cpu_percent, None);

        let (second, _) = digest(Some(&prev), scan(150, 1_100));
        let pct = second.processes[0]
            .cpu_percent
            .expect("second tick has cpu");
        assert!((pct - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn snapshot_is_sorted_and_truncated() {
        let processes = (0..30)
            .map(|i| RawProcess {
                pid: i,
                name: format!("p{i}"),
                state: 'S',
                jiffies: 0,
                memory: u64::from(u32::try_from(i).expect("small")) * 1024,
            })
            .collect();
        let scan = Scan {
            total_jiffies: 1_000,
            cpus: 1,
            processes,
        };
        let (snapshot, _) = digest(None, scan);
        assert_eq!(snapshot.processes.len(), TOP_N);
        assert_eq!(snapshot.total, 30);
        assert_eq!(snapshot.processes[0].pid, 29);
    }
}

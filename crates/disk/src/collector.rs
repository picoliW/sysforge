//! Disk I/O throughput and filesystem usage.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use sysforge_common::collector::{Collector, CollectorError};

const DISKSTATS: &str = "/proc/diskstats";
const MOUNTS: &str = "/proc/mounts";
/// Sector size assumed by `/proc/diskstats`, in bytes (Linux constant).
const SECTOR_SIZE: u64 = 512;

/// One block device's I/O rates.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceIo {
    /// Device name (`sda`, `nvme0n1`).
    pub name: String,
    /// Read rate in bytes/second. `None` on first sight.
    pub read_rate: Option<f64>,
    /// Write rate in bytes/second. `None` on first sight.
    pub write_rate: Option<f64>,
}

impl DeviceIo {
    /// Combined throughput this tick, for sorting.
    #[must_use]
    pub fn total_rate(&self) -> f64 {
        self.read_rate.unwrap_or(0.0) + self.write_rate.unwrap_or(0.0)
    }
}

/// One mounted filesystem's space usage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Filesystem {
    /// Mount point (`/`, `/home`).
    pub mount_point: String,
    /// Total capacity in bytes.
    pub total: u64,
    /// Used bytes (total minus available).
    pub used: u64,
}

impl Filesystem {
    /// Used space as a percentage of total, `0.0..=100.0`.
    #[must_use]
    #[allow(clippy::cast_precision_loss)] // filesystem sizes fit f64 for a gauge
    pub fn used_percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.used as f64 / self.total as f64 * 100.0
    }
}

/// One reading of the disk domain.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DiskSnapshot {
    /// Block devices, busiest first.
    pub devices: Vec<DeviceIo>,
    /// Mounted filesystems, fullest first.
    pub filesystems: Vec<Filesystem>,
}

/// Raw accumulated I/O counters for one device.
#[derive(Debug, Clone, Copy)]
struct IoCounters {
    read_bytes: u64,
    write_bytes: u64,
}

/// The previous I/O sample, kept for rate derivation.
struct PrevSample {
    at: Instant,
    per_device: HashMap<String, IoCounters>,
}

/// Samples disk I/O and filesystem usage at a configurable interval.
pub struct DiskCollector {
    interval: Duration,
    previous: Option<PrevSample>,
}

impl DiskCollector {
    /// Creates a collector sampling at the given interval.
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            previous: None,
        }
    }
}

impl Collector for DiskCollector {
    type Output = DiskSnapshot;

    fn name(&self) -> &'static str {
        "disk"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    async fn collect(&mut self) -> Result<DiskSnapshot, CollectorError> {
        let diskstats = tokio::fs::read_to_string(DISKSTATS).await?;
        let mounts = tokio::fs::read_to_string(MOUNTS).await?;
        let now = Instant::now();

        let counters = parse_diskstats(&diskstats);
        let filesystems = read_filesystems(&mounts);

        let (devices, previous) = derive_io(self.previous.as_ref(), counters, now);
        self.previous = Some(previous);
        Ok(DiskSnapshot {
            devices,
            filesystems,
        })
    }
}

/// Parses `/proc/diskstats` into per-device byte counters.
///
/// Fields (1-indexed): 3 = device name, 6 = sectors read, 10 = sectors
/// written. Partitions and zero-activity devices are kept; the view
/// sorts and trims.
fn parse_diskstats(raw: &str) -> HashMap<String, IoCounters> {
    let mut result = HashMap::new();
    for line in raw.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let (Some(&name), Some(read), Some(written)) =
            (fields.get(2), fields.get(5), fields.get(9))
        else {
            continue;
        };
        // Skip loop and ram devices: noise for a disk dashboard.
        if name.starts_with("loop") || name.starts_with("ram") {
            continue;
        }
        let (Ok(read_sectors), Ok(write_sectors)) = (read.parse::<u64>(), written.parse::<u64>())
        else {
            continue;
        };
        result.insert(
            name.to_owned(),
            IoCounters {
                read_bytes: read_sectors * SECTOR_SIZE,
                write_bytes: write_sectors * SECTOR_SIZE,
            },
        );
    }
    result
}

/// Reads real mounted filesystems and their usage via `statvfs`.
fn read_filesystems(mounts: &str) -> Vec<Filesystem> {
    let mut filesystems = Vec::new();
    for line in mounts.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        let (Some(&_device), Some(&mount_point), Some(&fstype)) =
            (fields.first(), fields.get(1), fields.get(2))
        else {
            continue;
        };
        if is_pseudo_fs(fstype) || is_noise_mount(mount_point) {
            continue;
        }
        if let Some(fs) = statvfs_usage(mount_point) {
            filesystems.push(fs);
        }
    }

    filesystems.sort_by(|a, b| a.mount_point.cmp(&b.mount_point));
    filesystems.dedup_by_key(|fs| (fs.total, fs.used));

    filesystems.sort_by(|a, b| {
        b.used_percent()
            .partial_cmp(&a.used_percent())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    filesystems
}

/// Filesystem types that are not real storage and clutter a disk view.
fn is_pseudo_fs(fstype: &str) -> bool {
    matches!(
        fstype,
        "proc"
            | "sysfs"
            | "cgroup"
            | "cgroup2"
            | "tmpfs"
            | "devtmpfs"
            | "devpts"
            | "mqueue"
            | "hugetlbfs"
            | "debugfs"
            | "tracefs"
            | "securityfs"
            | "pstore"
            | "bpf"
            | "configfs"
            | "fusectl"
            | "overlay"
            | "squashfs"
            | "autofs"
            | "binfmt_misc"
    )
}

/// Mount points that clutter a disk dashboard: snap package mounts and
/// WSL/system bind mounts that duplicate a real filesystem.
fn is_noise_mount(mount_point: &str) -> bool {
    mount_point.starts_with("/snap/")
        || mount_point.starts_with("/usr/lib/wsl")
        || mount_point == "/init"
}

/// Space usage of the filesystem at `mount_point`, via `statvfs`.
fn statvfs_usage(mount_point: &str) -> Option<Filesystem> {
    let stat = rustix::fs::statvfs(mount_point).ok()?;
    let block = stat.f_frsize;
    let total = stat.f_blocks * block;
    let available = stat.f_bavail * block;
    let used = total.saturating_sub(stat.f_bfree * block);
    if total == 0 {
        return None;
    }
    let _ = available;
    Some(Filesystem {
        mount_point: mount_point.to_owned(),
        total,
        used,
    })
}

/// Derives per-device rates from the delta against the previous sample.
fn derive_io(
    previous: Option<&PrevSample>,
    counters: HashMap<String, IoCounters>,
    now: Instant,
) -> (Vec<DeviceIo>, PrevSample) {
    let elapsed = previous.map(|p| (now - p.at).as_secs_f64());

    let mut devices: Vec<DeviceIo> = counters
        .iter()
        .map(|(name, current)| {
            let (read_rate, write_rate) = match (previous, elapsed) {
                (Some(prev), Some(secs)) if secs > 0.0 => prev
                    .per_device
                    .get(name)
                    .map(|before| {
                        (
                            rate(current.read_bytes, before.read_bytes, secs),
                            rate(current.write_bytes, before.write_bytes, secs),
                        )
                    })
                    .map_or((None, None), |(r, w)| (Some(r), Some(w))),
                _ => (None, None),
            };
            DeviceIo {
                name: name.clone(),
                read_rate,
                write_rate,
            }
        })
        .collect();

    devices.sort_by(|a, b| {
        b.total_rate()
            .partial_cmp(&a.total_rate())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });

    let previous = PrevSample {
        at: now,
        per_device: counters,
    };
    (devices, previous)
}

/// Bytes/second between two readings, guarding counter resets.
#[allow(clippy::cast_precision_loss)]
fn rate(current: u64, previous: u64, secs: f64) -> f64 {
    current.saturating_sub(previous) as f64 / secs
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISKSTATS_SAMPLE: &str = "\
   8       0 sda 1000 0 4000 500 2000 0 8000 800 0 300 1300
   8       1 sda1 500 0 2000 250 1000 0 4000 400 0 150 650
   7       0 loop0 10 0 40 5 0 0 0 0 0 2 5
";

    #[test]
    fn parses_bytes_and_skips_loop() {
        let counters = parse_diskstats(DISKSTATS_SAMPLE);
        assert!(!counters.contains_key("loop0"));
        // field 6 (sectors read) = 4000 for sda => 4000 * 512 bytes.
        assert_eq!(counters["sda"].read_bytes, 4000 * SECTOR_SIZE);
        assert_eq!(counters["sda"].write_bytes, 8000 * SECTOR_SIZE);
    }

    #[test]
    fn pseudo_filesystems_are_filtered() {
        assert!(is_pseudo_fs("tmpfs"));
        assert!(is_pseudo_fs("cgroup2"));
        assert!(!is_pseudo_fs("ext4"));
        assert!(!is_pseudo_fs("btrfs"));
    }

    #[test]
    fn first_io_sample_has_no_rates() {
        let counters = parse_diskstats(DISKSTATS_SAMPLE);
        let (devices, _) = derive_io(None, counters, Instant::now());
        assert!(devices.iter().all(|d| d.read_rate.is_none()));
    }

    #[test]
    fn second_io_sample_computes_rate() {
        let t0 = Instant::now();
        let first = parse_diskstats(DISKSTATS_SAMPLE);
        let (_, prev) = derive_io(None, first, t0);

        let mut second = HashMap::new();
        second.insert(
            "sda".to_owned(),
            IoCounters {
                read_bytes: 8000 * SECTOR_SIZE,
                write_bytes: 8000 * SECTOR_SIZE,
            },
        );
        let (devices, _) = derive_io(Some(&prev), second, t0 + Duration::from_secs(2));
        let sda = devices
            .iter()
            .find(|d| d.name == "sda")
            .expect("sda present");
        assert!((sda.read_rate.expect("has rate") - 1_024_000.0).abs() < 1.0);
    }

    #[test]
    fn used_percent_is_sane() {
        let fs = Filesystem {
            mount_point: "/".into(),
            total: 1000,
            used: 250,
        };
        assert!((fs.used_percent() - 25.0).abs() < f64::EPSILON);
    }

    #[test]
    fn noise_mounts_are_filtered() {
        assert!(is_noise_mount("/snap/core/17284"));
        assert!(is_noise_mount("/usr/lib/wsl/drivers"));
        assert!(!is_noise_mount("/"));
        assert!(!is_noise_mount("/home"));
    }
}

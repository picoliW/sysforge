use std::time::Duration;

use sysforge_common::collector::{Collector, CollectorError};

const MEMINFO: &str = "/proc/meminfo";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub total: u64,
    pub available: u64,
    pub swap_total: u64,
    pub swap_free: u64,
}

impl MemorySnapshot {
    #[must_use]
    pub fn used(&self) -> u64 {
        self.total.saturating_sub(self.available)
    }

    #[must_use]
    #[allow(clippy::cast_precision_loss)] 
    pub fn used_percent(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.used() as f64 / self.total as f64 * 100.0
    }

    #[must_use]
    pub fn swap_used(&self) -> u64 {
        self.swap_total.saturating_sub(self.swap_free)
    }
}

#[derive(Debug)]
pub struct MemoryCollector {
    interval: Duration,
}

impl MemoryCollector {
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl Default for MemoryCollector {
    fn default() -> Self {
        Self::new(Duration::from_secs(1))
    }
}

impl Collector for MemoryCollector {
    type Output = MemorySnapshot;

    fn name(&self) -> &'static str {
        "memory"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    async fn collect(&mut self) -> Result<MemorySnapshot, CollectorError> {
        let raw = tokio::fs::read_to_string(MEMINFO).await?;
        parse_meminfo(&raw)
    }
}

fn parse_meminfo(raw: &str) -> Result<MemorySnapshot, CollectorError> {
    let mut snapshot = MemorySnapshot::default();

    for line in raw.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let field = match key {
            "MemTotal" => &mut snapshot.total,
            "MemAvailable" => &mut snapshot.available,
            "SwapTotal" => &mut snapshot.swap_total,
            "SwapFree" => &mut snapshot.swap_free,
            _ => continue,
        };

        let kib: u64 = value
            .trim()
            .trim_end_matches("kB")
            .trim()
            .parse()
            .map_err(|e| CollectorError::Parse {
                path: MEMINFO,
                reason: format!("{key}: {e}"),
            })?;
        *field = kib * 1024;
    }

    if snapshot.total == 0 {
        return Err(CollectorError::Parse {
            path: MEMINFO,
            reason: "MemTotal missing or zero".into(),
        });
    }
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
MemTotal:       16308668 kB
MemFree:         1201388 kB
MemAvailable:    9673544 kB
Buffers:          492752 kB
SwapTotal:       4194304 kB
SwapFree:        4194304 kB
";

    #[test]
    fn parses_relevant_fields_as_bytes() {
        let snap = parse_meminfo(SAMPLE).expect("sample must parse");
        assert_eq!(snap.total, 16_308_668 * 1024);
        assert_eq!(snap.available, 9_673_544 * 1024);
        assert_eq!(snap.swap_used(), 0);
        assert_eq!(snap.used(), (16_308_668 - 9_673_544) * 1024);
    }

    #[test]
    fn missing_memtotal_is_an_error() {
        assert!(parse_meminfo("MemFree: 10 kB\n").is_err());
    }
}
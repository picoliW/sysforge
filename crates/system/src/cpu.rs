use std::time::Duration;

use sysforge_common::collector::{Collector, CollectorError};

const PROC_STAT: &str = "/proc/stat";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CoreTimes {
    user: u64,
    nice: u64,
    system: u64,
    idle: u64,
    iowait: u64,
    irq: u64,
    softirq: u64,
    steal: u64,
}

impl CoreTimes {
    fn idle_all(self) -> u64 {
        self.idle + self.iowait
    }

    fn total(self) -> u64 {
        self.user
            + self.nice
            + self.system
            + self.idle
            + self.iowait
            + self.irq
            + self.softirq
            + self.steal
    }

    #[allow(clippy::cast_precision_loss)]
    fn usage_since(self, earlier: Self) -> f64 {
        let total = self.total().saturating_sub(earlier.total());
        let idle = self.idle_all().saturating_sub(earlier.idle_all());
        if total == 0 {
            return 0.0;
        }
        total.saturating_sub(idle) as f64 / total as f64 * 100.0
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct CpuSample {
    aggregate: CoreTimes,
    per_core: Vec<CoreTimes>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct CpuSnapshot {
    pub total: f64,
    pub per_core: Vec<f64>,
}

#[derive(Debug)]
pub struct CpuCollector {
    interval: Duration,
    previous: Option<CpuSample>,
}

impl CpuCollector {
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            previous: None,
        }
    }
}

impl Default for CpuCollector {
    fn default() -> Self {
        Self::new(Duration::from_secs(1))
    }
}

impl Collector for CpuCollector {
    type Output = Option<CpuSnapshot>;

    fn name(&self) -> &'static str {
        "cpu"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    async fn collect(&mut self) -> Result<Self::Output, CollectorError> {
        let raw = tokio::fs::read_to_string(PROC_STAT).await?;
        let current = parse_proc_stat(&raw)?;

        let snapshot = self.previous.as_ref().map(|prev| CpuSnapshot {
            total: current.aggregate.usage_since(prev.aggregate),
            per_core: current
                .per_core
                .iter()
                .zip(&prev.per_core)
                .map(|(now, then)| now.usage_since(*then))
                .collect(),
        });

        self.previous = Some(current);
        Ok(snapshot)
    }
}

fn parse_proc_stat(raw: &str) -> Result<CpuSample, CollectorError> {
    let mut aggregate = None;
    let mut per_core = Vec::new();

    for line in raw.lines() {
        let mut fields = line.split_whitespace();
        let Some(label) = fields.next() else { continue };
        if !label.starts_with("cpu") {
            break;
        }
        let times = parse_core_times(fields, label)?;
        if label == "cpu" {
            aggregate = Some(times);
        } else {
            per_core.push(times);
        }
    }

    let aggregate = aggregate.ok_or(CollectorError::Parse {
        path: PROC_STAT,
        reason: String::from("aggregate `cpu` line missing"),
    })?;
    Ok(CpuSample {
        aggregate,
        per_core,
    })
}

fn parse_core_times<'a>(
    mut fields: impl Iterator<Item = &'a str>,
    label: &str,
) -> Result<CoreTimes, CollectorError> {
    let mut values = [0u64; 8];
    for value in &mut values {
        *value = fields
            .next()
            .ok_or_else(|| CollectorError::Parse {
                path: PROC_STAT,
                reason: format!("{label}: fewer fields than expected"),
            })?
            .parse()
            .map_err(|e| CollectorError::Parse {
                path: PROC_STAT,
                reason: format!("{label}: {e}"),
            })?;
    }
    let [user, nice, system, idle, iowait, irq, softirq, steal] = values;
    Ok(CoreTimes {
        user,
        nice,
        system,
        idle,
        iowait,
        irq,
        softirq,
        steal,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
cpu  100 0 100 700 100 0 0 0 0 0
cpu0 50 0 50 350 50 0 0 0 0 0
cpu1 50 0 50 350 50 0 0 0 0 0
intr 12345
";

    #[test]
    fn parses_aggregate_and_cores() {
        let sample = parse_proc_stat(SAMPLE).expect("sample must parse");
        assert_eq!(sample.per_core.len(), 2);
        assert_eq!(sample.aggregate.user, 100);
        assert_eq!(sample.aggregate.idle_all(), 800);
    }

    #[test]
    fn usage_is_busy_delta_over_total_delta() {
        let earlier = CoreTimes {
            user: 100,
            idle: 700,
            iowait: 100,
            system: 100,
            ..CoreTimes::default()
        };
        let now = CoreTimes {
            user: 200,
            idle: 800,
            iowait: 100,
            system: 100,
            ..CoreTimes::default()
        };
        let usage = now.usage_since(earlier);
        assert!((usage - 50.0).abs() < f64::EPSILON);
    }

    #[test]
    fn zero_delta_means_zero_usage() {
        let t = CoreTimes::default();
        assert!(t.usage_since(t).abs() < f64::EPSILON);
    }
}

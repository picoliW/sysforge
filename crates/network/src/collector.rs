//! Interface throughput via `/proc/net/dev`.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use sysforge_common::collector::{Collector, CollectorError};

const PROC_NET_DEV: &str = "/proc/net/dev";

/// One interface as shown in the UI.
#[derive(Debug, Clone, PartialEq)]
pub struct InterfaceInfo {
    /// Interface name (`eth0`, `wg0`, `lo`, ...).
    pub name: String,
    /// Total bytes received since boot.
    pub rx_bytes: u64,
    /// Total bytes transmitted since boot.
    pub tx_bytes: u64,
    /// Receive rate in bytes/second. `None` on first sight.
    pub rx_rate: Option<f64>,
    /// Transmit rate in bytes/second. `None` on first sight.
    pub tx_rate: Option<f64>,
}

impl InterfaceInfo {
    /// Combined throughput this tick, for sorting. Zero until rates
    /// exist.
    #[must_use]
    pub fn total_rate(&self) -> f64 {
        self.rx_rate.unwrap_or(0.0) + self.tx_rate.unwrap_or(0.0)
    }
}

/// One reading of the network interfaces.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NetworkSnapshot {
    /// Interfaces, busiest first.
    pub interfaces: Vec<InterfaceInfo>,
}

/// Raw accumulated counters for one interface.
#[derive(Debug, Clone, Copy)]
struct Counters {
    rx_bytes: u64,
    tx_bytes: u64,
}

/// The previous sample, kept for rate derivation.
struct PrevSample {
    at: Instant,
    per_iface: HashMap<String, Counters>,
}

/// Samples `/proc/net/dev` at a configurable interval.
pub struct NetworkCollector {
    interval: Duration,
    previous: Option<PrevSample>,
}

impl NetworkCollector {
    /// Creates a collector sampling at the given interval.
    #[must_use]
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            previous: None,
        }
    }
}

impl Collector for NetworkCollector {
    type Output = NetworkSnapshot;

    fn name(&self) -> &'static str {
        "network"
    }

    fn interval(&self) -> Duration {
        self.interval
    }

    async fn collect(&mut self) -> Result<NetworkSnapshot, CollectorError> {
        let raw = tokio::fs::read_to_string(PROC_NET_DEV).await?;
        let now = Instant::now();
        let counters = parse_net_dev(&raw)?;

        let (snapshot, previous) = digest(self.previous.as_ref(), counters, now);
        self.previous = Some(previous);
        Ok(snapshot)
    }
}

/// Parses `/proc/net/dev` into per-interface counters. The first two
/// lines are headers; each remaining line is `name: rx_bytes ... (8
/// rx fields) tx_bytes ... (8 tx fields)`.
fn parse_net_dev(raw: &str) -> Result<HashMap<String, Counters>, CollectorError> {
    let mut result = HashMap::new();
    for line in raw.lines().skip(2) {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let fields: Vec<u64> = rest
            .split_whitespace()
            .filter_map(|f| f.parse().ok())
            .collect();
        let (Some(&rx_bytes), Some(&tx_bytes)) = (fields.first(), fields.get(8)) else {
            continue;
        };
        result.insert(name.trim().to_owned(), Counters { rx_bytes, tx_bytes });
    }
    if result.is_empty() {
        return Err(CollectorError::Parse {
            path: PROC_NET_DEV,
            reason: String::from("no interfaces parsed"),
        });
    }
    Ok(result)
}

/// Pure derivation: current counters + previous sample -> snapshot with
/// rates + the sample to keep. Unit-testable without `/proc`.
fn digest(
    previous: Option<&PrevSample>,
    counters: HashMap<String, Counters>,
    now: Instant,
) -> (NetworkSnapshot, PrevSample) {
    let elapsed = previous.map(|p| (now - p.at).as_secs_f64());

    let mut interfaces: Vec<InterfaceInfo> = counters
        .iter()
        .map(|(name, current)| {
            let (rx_rate, tx_rate) = match (previous, elapsed) {
                (Some(prev), Some(secs)) if secs > 0.0 => prev
                    .per_iface
                    .get(name)
                    .map(|before| {
                        (
                            rate(current.rx_bytes, before.rx_bytes, secs),
                            rate(current.tx_bytes, before.tx_bytes, secs),
                        )
                    })
                    .map_or((None, None), |(rx, tx)| (Some(rx), Some(tx))),
                _ => (None, None),
            };
            InterfaceInfo {
                name: name.clone(),
                rx_bytes: current.rx_bytes,
                tx_bytes: current.tx_bytes,
                rx_rate,
                tx_rate,
            }
        })
        .collect();

    interfaces.sort_by(|a, b| {
        b.total_rate()
            .partial_cmp(&a.total_rate())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.name.cmp(&b.name))
    });

    let previous = PrevSample {
        at: now,
        per_iface: counters,
    };
    (NetworkSnapshot { interfaces }, previous)
}

/// Bytes/second between two readings, guarding counter resets.
#[allow(clippy::cast_precision_loss)]
fn rate(current: u64, previous: u64, secs: f64) -> f64 {
    current.saturating_sub(previous) as f64 / secs
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Inter-|   Receive                                                |  Transmit
 face |bytes    packets errs drop fifo frame compressed multicast|bytes    packets
    lo:    1000     10    0    0    0     0          0         0     1000      10
  eth0:  500000    400    0    0    0     0          0         0   200000     300
";

    #[test]
    fn parses_rx_and_tx_bytes() {
        let counters = parse_net_dev(SAMPLE).expect("sample must parse");
        assert_eq!(counters["eth0"].rx_bytes, 500_000);
        assert_eq!(counters["eth0"].tx_bytes, 200_000);
        assert_eq!(counters.len(), 2);
    }

    #[test]
    fn first_sample_has_no_rates() {
        let counters = parse_net_dev(SAMPLE).expect("sample must parse");
        let (snapshot, _) = digest(None, counters, Instant::now());
        assert!(snapshot.interfaces.iter().all(|i| i.rx_rate.is_none()));
    }

    #[test]
    fn second_sample_computes_rate() {
        let t0 = Instant::now();
        let first = parse_net_dev(SAMPLE).expect("sample must parse");
        let (_, prev) = digest(None, first, t0);

        let mut second = HashMap::new();
        second.insert(
            "eth0".to_owned(),
            Counters {
                rx_bytes: 1_000_000,
                tx_bytes: 200_000,
            },
        );
        let (snapshot, _) = digest(Some(&prev), second, t0 + Duration::from_secs(2));

        let eth0 = &snapshot.interfaces[0];
        assert!((eth0.rx_rate.expect("has rate") - 250_000.0).abs() < 1.0);
    }
}

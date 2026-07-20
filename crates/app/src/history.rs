use std::collections::VecDeque;

const DEFAULT_CAPACITY: usize = 600;

#[derive(Debug, Clone)]
pub struct History {
    values: VecDeque<u64>,
    capacity: usize,
}

impl History {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            values: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub fn push(&mut self, value: u64) {
        if self.values.len() == self.capacity {
            self.values.pop_front();
        }
        self.values.push_back(value);
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn push_percent(&mut self, percent: f64) {
        self.push(percent.clamp(0.0, 100.0).round() as u64);
    }

    #[must_use]
    pub fn last(&self, n: usize) -> Vec<u64> {
        self.values.iter().rev().take(n).rev().copied().collect()
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new(DEFAULT_CAPACITY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evicts_oldest_at_capacity() {
        let mut h = History::new(3);
        for v in [1, 2, 3, 4] {
            h.push(v);
        }
        assert_eq!(h.last(10), vec![2, 3, 4]);
    }

    #[test]
    fn last_returns_newest_in_order() {
        let mut h = History::new(10);
        for v in [1, 2, 3, 4, 5] {
            h.push(v);
        }
        assert_eq!(h.last(2), vec![4, 5]);
    }

    #[test]
    fn percent_is_clamped() {
        let mut h = History::new(4);
        h.push_percent(-5.0);
        h.push_percent(250.0);
        assert_eq!(h.last(4), vec![0, 100]);
    }
}

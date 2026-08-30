use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lock-free atomic state of a single spot exchange with continuous 5-second sliding window
pub struct SpotExchangeState {
    pub last_price_micro_usd: AtomicU64,
    pub price_history: [AtomicU64; 5],
    pub last_update_sec: AtomicU64,
}

impl SpotExchangeState {
    pub fn new(initial_price_usd: f64) -> Self {
        let micro_cents = (initial_price_usd * 1_000_000.0) as u64;
        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            last_price_micro_usd: AtomicU64::new(micro_cents),
            price_history: [
                AtomicU64::new(micro_cents),
                AtomicU64::new(micro_cents),
                AtomicU64::new(micro_cents),
                AtomicU64::new(micro_cents),
                AtomicU64::new(micro_cents),
            ],
            last_update_sec: AtomicU64::new(now_sec),
        }
    }

    #[inline(always)]
    pub fn update_price(&self, price_usd: f64, now_ms: u64) {
        let micro_cents = (price_usd * 1_000_000.0) as u64;
        let now_sec = now_ms / 1000;
        let prev_sec = self.last_update_sec.load(Ordering::Relaxed);

        if now_sec > prev_sec {
            let slot = (now_sec % 5) as usize;
            self.price_history[slot].store(micro_cents, Ordering::Relaxed);
            self.last_update_sec.store(now_sec, Ordering::Relaxed);
        }

        self.last_price_micro_usd.store(micro_cents, Ordering::Relaxed);
    }

    #[inline(always)]
    pub fn get_baseline_price_5s_ago(&self) -> f64 {
        let now_sec = self.last_update_sec.load(Ordering::Relaxed);
        let oldest_slot = ((now_sec + 1) % 5) as usize;
        self.price_history[oldest_slot].load(Ordering::Relaxed) as f64 / 1_000_000.0
    }
}

/// Multi-Exchange Zero-Allocation Spot Consensus Streamer
pub struct MultiExchangeConsensus {
    pub binance: SpotExchangeState,
    pub coinbase: SpotExchangeState,
    pub kraken: SpotExchangeState,
}

impl MultiExchangeConsensus {
    pub fn new(initial_btc_price_usd: f64) -> Self {
        Self {
            binance: SpotExchangeState::new(initial_btc_price_usd),
            coinbase: SpotExchangeState::new(initial_btc_price_usd),
            kraken: SpotExchangeState::new(initial_btc_price_usd),
        }
    }

    /// Evaluates multi-exchange spot consensus spike
    /// Returns Some((is_up, max_delta_usd)) if >= 2 exchanges confirm and Coinbase is confirmed
    pub fn evaluate_consensus_spike(&self, min_delta_usd: f64) -> Option<(bool, f64)> {
        let bn_delta = self.get_delta(&self.binance);
        let cb_delta = self.get_delta(&self.coinbase);
        let kr_delta = self.get_delta(&self.kraken);

        let cb_confirmed = cb_delta.abs() >= min_delta_usd;
        let is_up = cb_delta > 0.0;
        let mut confirming_count = 0;

        if (bn_delta > 0.0) == is_up && bn_delta.abs() >= min_delta_usd {
            confirming_count += 1;
        }
        if (cb_delta > 0.0) == is_up && cb_confirmed {
            confirming_count += 1;
        }
        if (kr_delta > 0.0) == is_up && kr_delta.abs() >= min_delta_usd {
            confirming_count += 1;
        }

        if cb_confirmed && confirming_count >= 2 {
            let max_delta = bn_delta.abs().max(cb_delta.abs()).max(kr_delta.abs());
            Some((is_up, max_delta))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn get_delta(&self, exchange: &SpotExchangeState) -> f64 {
        let now_p = exchange.last_price_micro_usd.load(Ordering::Relaxed) as f64 / 1_000_000.0;
        let prev_p = exchange.get_baseline_price_5s_ago();
        now_p - prev_p
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_exchange_spot_consensus_spike() {
        let consensus = MultiExchangeConsensus::new(65000.0);

        // Initially no spike
        assert!(consensus.evaluate_consensus_spike(18.0).is_none());

        // Binance moves +$25, Coinbase moves +$22, Kraken moves +$5
        consensus.binance.last_price_micro_usd.store(65025_000000, Ordering::Relaxed);
        consensus.coinbase.last_price_micro_usd.store(65022_000000, Ordering::Relaxed);
        consensus.kraken.last_price_micro_usd.store(65005_000000, Ordering::Relaxed);

        // Coinbase confirmed & Binance confirmed -> Spike detected (UP, max delta $25.0)
        let res = consensus.evaluate_consensus_spike(18.0);
        assert!(res.is_some());
        let (is_up, max_delta) = res.unwrap();
        assert!(is_up);
        assert_eq!(max_delta, 25.0);

        // If Coinbase is NOT confirmed (e.g. only Binance moved), spike is rejected (fakeout protection)
        consensus.coinbase.last_price_micro_usd.store(65000_000000, Ordering::Relaxed);
        assert!(consensus.evaluate_consensus_spike(18.0).is_none());
    }
}

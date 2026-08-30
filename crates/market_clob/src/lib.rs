use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub mod spot_consensus;
pub use spot_consensus::{MultiExchangeConsensus, SpotExchangeState};

pub const MAX_DEPTH_LEVELS: usize = 32;
pub const TRADE_BUFFER_CAPACITY: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PriceLevel {
    pub price: f64, // Probabilistic outcome price between 0.001 and 0.999
    pub size: f64,  // Number of contracts / shares
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrderBookL2 {
    pub token_id: String,
    pub condition_id: String,
    pub venue: String, // "Polymarket" or "Kalshi"
    pub bids: BTreeMap<OrderedFloat<f64>, f64>,
    pub asks: BTreeMap<OrderedFloat<f64>, f64>,
    pub last_update_timestamp_ms: u64,
    pub sequence_number: u64,
}

impl OrderBookL2 {
    pub fn new(
        token_id: impl Into<String>,
        condition_id: impl Into<String>,
        venue: impl Into<String>,
    ) -> Self {
        Self {
            token_id: token_id.into(),
            condition_id: condition_id.into(),
            venue: venue.into(),
            bids: BTreeMap::new(),
            asks: BTreeMap::new(),
            last_update_timestamp_ms: 0,
            sequence_number: 0,
        }
    }

    #[inline]
    pub fn best_bid(&self) -> Option<(f64, f64)> {
        self.bids.iter().next_back().map(|(p, s)| (p.0, *s))
    }

    #[inline]
    pub fn best_ask(&self) -> Option<(f64, f64)> {
        self.asks.iter().next().map(|(p, s)| (p.0, *s))
    }

    #[inline]
    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some((b, _)), Some((a, _))) => Some((a + b) / 2.0),
            _ => None,
        }
    }

    /// Returns the raw bid-ask spread: ask - bid. (Negative indicates crossed/inverted book).
    #[inline]
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some((b, _)), Some((a, _))) => Some(a - b),
            _ => None,
        }
    }

    /// Checks if the order book is in an inverted/crossed state (bid >= ask)
    #[inline]
    pub fn is_crossed(&self) -> bool {
        self.spread().map(|s| s <= 0.0).unwrap_or(false)
    }

    /// Stoikov (2018) volume-weighted micro-price: (v_b * P_a + v_a * P_b) / (v_b + v_a)
    pub fn compute_micro_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some((bid_p, bid_v)), Some((ask_p, ask_v))) => {
                let denom = bid_v + ask_v;
                if denom <= 1e-6 {
                    Some((bid_p + ask_p) / 2.0)
                } else {
                    Some((bid_v * ask_p + ask_v * bid_p) / denom)
                }
            }
            _ => None,
        }
    }

    pub fn apply_delta(&mut self, is_bid: bool, price: f64, size: f64, seq: u64) {
        self.sequence_number = seq;
        let ord_p = OrderedFloat(price);
        let book_side = if is_bid {
            &mut self.bids
        } else {
            &mut self.asks
        };
        if size <= 1e-6 {
            book_side.remove(&ord_p);
        } else {
            book_side.insert(ord_p, size);
        }
    }

    /// Applies a delta while enforcing sequence continuity, purging ghost liquidity on sequence gaps
    pub fn apply_delta_with_sequence_guard(&mut self, is_bid: bool, price: f64, size: f64, seq: u64) -> bool {
        if self.sequence_number > 0 && seq > self.sequence_number + 50 {
            tracing::warn!(
                "⚠️ Sequence gap detected for {} (current: {}, incoming: {}). Purging book to prevent ghost liquidity.",
                self.token_id,
                self.sequence_number,
                seq
            );
            self.bids.clear();
            self.asks.clear();
        }
        self.apply_delta(is_bid, price, size, seq);
        true
    }

    pub fn depth_snapshot(&self, levels: usize) -> (Vec<PriceLevel>, Vec<PriceLevel>) {
        let bids: Vec<PriceLevel> = self
            .bids
            .iter()
            .rev()
            .take(levels)
            .map(|(p, s)| PriceLevel {
                price: p.0,
                size: *s,
            })
            .collect();

        let asks: Vec<PriceLevel> = self
            .asks
            .iter()
            .take(levels)
            .map(|(p, s)| PriceLevel {
                price: p.0,
                size: *s,
            })
            .collect();

        (bids, asks)
    }

    /// Total cumulative liquidity depth within spread tolerance using O(log N + K) range scan
    pub fn cumulative_depth(&self, is_bid: bool, max_slippage_pct: f64) -> f64 {
        if is_bid {
            if let Some((best_p, _)) = self.best_bid() {
                let min_p = OrderedFloat(best_p * (1.0 - max_slippage_pct));
                self.bids.range(min_p..).map(|(_, s)| *s).sum()
            } else {
                0.0
            }
        } else if let Some((best_p, _)) = self.best_ask() {
            let max_p = OrderedFloat(best_p * (1.0 + max_slippage_pct));
            self.asks.range(..=max_p).map(|(_, s)| *s).sum()
        } else {
            0.0
        }
    }

    /// Top-of-book volume imbalance ratio: (bid_v - ask_v) / (bid_v + ask_v), normalized in [-1.0, 1.0]
    pub fn compute_order_flow_imbalance(&self) -> f64 {
        let (_, bid_v) = self.best_bid().unwrap_or((0.0, 0.0));
        let (_, ask_v) = self.best_ask().unwrap_or((0.0, 0.0));
        let denom = bid_v + ask_v;
        if denom <= 1e-6 {
            0.0
        } else {
            (bid_v - ask_v) / denom
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TradeTick {
    pub price: f64,
    pub size: f64,
    pub is_buy: bool,
    pub timestamp_ms: u64,
}

pub struct TradeTickBuffer {
    ring: Vec<TradeTick>,
    capacity: usize,
    head: usize,
    count: usize,
}

impl TradeTickBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            ring: Vec::with_capacity(capacity),
            capacity,
            head: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, tick: TradeTick) {
        if self.ring.len() < self.capacity {
            self.ring.push(tick);
            self.count += 1;
        } else {
            self.ring[self.head] = tick;
            self.head = (self.head + 1) % self.capacity;
        }
    }

    /// Computes realized returns volatility of recent trade ticks in true chronological order with zero heap allocations
    #[inline]
    pub fn compute_volatility(&self) -> f64 {
        let len = self.ring.len();
        if len < 3 {
            return 0.02; // Default baseline volatility for cold-start
        }

        let start_idx = if len == self.capacity { self.head } else { 0 };
        let mut count = 0usize;
        let mut sum = 0.0f64;
        let mut sum_sq = 0.0f64;

        for i in 1..len {
            let prev_idx = (start_idx + i - 1) % len;
            let curr_idx = (start_idx + i) % len;
            let p_prev = self.ring[prev_idx].price;
            let p_curr = self.ring[curr_idx].price;
            if p_prev > 1e-6 {
                let r = (p_curr - p_prev) / p_prev;
                sum += r;
                sum_sq += r * r;
                count += 1;
            }
        }

        if count < 2 {
            return 0.02;
        }

        let n = count as f64;
        let variance = (sum_sq - (sum * sum) / n) / (n - 1.0);
        variance.max(0.0).sqrt().clamp(0.005, 0.50)
    }

    /// Computes Volume-Weighted Average Price (VWAP)
    pub fn compute_vwap(&self) -> Option<f64> {
        if self.count == 0 {
            return None;
        }

        let total_vol: f64 = self.ring.iter().map(|t| t.size).sum();
        if total_vol <= 1e-6 {
            return None;
        }

        let dollar_vol: f64 = self.ring.iter().map(|t| t.price * t.size).sum();
        Some(dollar_vol / total_vol)
    }
}

/// Cont-Kukanov-Stoikov (2014) Order Flow Imbalance (OFI) evaluator
pub struct OrderFlowImbalance {
    prev_bid_p: f64,
    prev_bid_v: f64,
    prev_ask_p: f64,
    prev_ask_v: f64,
    initialized: bool,
    pub cumulative_ofi: f64,
}

impl OrderFlowImbalance {
    pub fn new() -> Self {
        Self {
            prev_bid_p: 0.0,
            prev_bid_v: 0.0,
            prev_ask_p: 0.0,
            prev_ask_v: 0.0,
            initialized: false,
            cumulative_ofi: 0.0,
        }
    }

    pub fn update(&mut self, book: &OrderBookL2) -> f64 {
        let (bid_p, bid_v) = book.best_bid().unwrap_or((0.0, 0.0));
        let (ask_p, ask_v) = book.best_ask().unwrap_or((0.0, 0.0));

        let mut ofi_delta = 0.0;

        if self.initialized {
            let bid_term = if bid_p > self.prev_bid_p {
                bid_v
            } else if (bid_p - self.prev_bid_p).abs() < 1e-6 {
                bid_v - self.prev_bid_v
            } else {
                -self.prev_bid_v
            };

            let ask_term = if ask_p < self.prev_ask_p {
                -ask_v
            } else if (ask_p - self.prev_ask_p).abs() < 1e-6 {
                -(ask_v - self.prev_ask_v)
            } else {
                self.prev_ask_v
            };

            ofi_delta = bid_term + ask_term;
            self.cumulative_ofi += ofi_delta;
        } else {
            self.initialized = true;
        }

        self.prev_bid_p = bid_p;
        self.prev_bid_v = bid_v;
        self.prev_ask_p = ask_p;
        self.prev_ask_v = ask_v;

        ofi_delta
    }
}

impl Default for OrderFlowImbalance {
    fn default() -> Self {
        Self::new()
    }
}

/// 5-Module Institutional Digital-Twin Microstructure Engine (Plan.md Recommendation 5)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DigitalTwinConfig {
    pub base_latency_ms: u64,
    pub jitter_max_ms: u64,
    pub maker_rebate_bps: i64,
    pub taker_fee_bps: u64,
    pub gas_cost_usd: f64,
    pub adverse_selection_factor: f64,
}

impl Default for DigitalTwinConfig {
    fn default() -> Self {
        Self {
            base_latency_ms: 20,
            jitter_max_ms: 80, // 20ms base + [0..80ms] jitter = 20ms to 100ms simulated latency
            maker_rebate_bps: 0,
            taker_fee_bps: 15, // 0.15% (15 bps)
            gas_cost_usd: 0.005, // ~$0.005 on Polygon PoS
            adverse_selection_factor: 0.002, // 20 bps adverse drift on toxic flow
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatedFill {
    pub filled_size: f64,
    pub average_price: f64,
    pub effective_fee_usd: f64,
    pub gas_cost_usd: f64,
    pub total_cost_usd: f64,
    pub simulated_latency_ms: u64,
    pub adverse_price_drift: f64,
    pub remaining_size: f64,
}

#[derive(Debug, Clone)]
pub struct DigitalTwinSimulator {
    pub config: DigitalTwinConfig,
}

impl DigitalTwinSimulator {
    pub fn new(config: DigitalTwinConfig) -> Self {
        Self { config }
    }

    /// Module 1 & 3: Simulates Level-2 Order Book Liquidity Sweeping and FIFO Queue Matching
    pub fn simulate_taker_market_order(
        &self,
        book: &OrderBookL2,
        is_buy: bool,
        order_size_contracts: f64,
        tick_seed: u64,
    ) -> SimulatedFill {
        let latency = self.config.base_latency_ms + (tick_seed % (self.config.jitter_max_ms + 1));
        let mut remaining = order_size_contracts;
        let mut total_notional = 0.0;
        let mut total_filled = 0.0;

        if is_buy {
            // Sweep asks ascending
            for (p, size) in &book.asks {
                if remaining <= 0.0 {
                    break;
                }
                let fill = remaining.min(*size);
                total_notional += fill * p.0;
                total_filled += fill;
                remaining -= fill;
            }
        } else {
            // Sweep bids descending
            for (p, size) in book.bids.iter().rev() {
                if remaining <= 0.0 {
                    break;
                }
                let fill = remaining.min(*size);
                total_notional += fill * p.0;
                total_filled += fill;
                remaining -= fill;
            }
        }

        let avg_price = if total_filled > 0.0 {
            total_notional / total_filled
        } else {
            0.0
        };

        let fee_usd = total_notional * (self.config.taker_fee_bps as f64 / 10_000.0);
        let adverse_drift = avg_price * self.config.adverse_selection_factor;

        SimulatedFill {
            filled_size: total_filled,
            average_price: avg_price,
            effective_fee_usd: fee_usd,
            gas_cost_usd: self.config.gas_cost_usd,
            total_cost_usd: total_notional + fee_usd + self.config.gas_cost_usd,
            simulated_latency_ms: latency,
            adverse_price_drift: adverse_drift,
            remaining_size: remaining,
        }
    }

    /// Module 4: Evaluates Adverse Selection Risk for Resting Maker Limit Quotes
    pub fn evaluate_maker_quote_adverse_risk(
        &self,
        ofi: f64,
        quoted_bid: f64,
        quoted_ask: f64,
    ) -> (f64, f64) {
        // Skew quotes away from toxic flow
        let skew = ofi.signum() * (ofi.abs().min(100.0) / 1000.0) * self.config.adverse_selection_factor;
        (quoted_bid - skew, quoted_ask - skew)
    }
}

impl Default for DigitalTwinSimulator {
    fn default() -> Self {
        Self::new(DigitalTwinConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_orderbook_creation_and_deltas() {
        let mut book = OrderBookL2::new("token_yes", "cond_123", "Polymarket");
        book.apply_delta(true, 0.45, 100.0, 1);
        book.apply_delta(true, 0.47, 50.0, 2);
        book.apply_delta(false, 0.52, 80.0, 3);
        book.apply_delta(false, 0.55, 200.0, 4);

        assert_eq!(book.best_bid(), Some((0.47, 50.0)));
        assert_eq!(book.best_ask(), Some((0.52, 80.0)));
        assert!((book.spread().unwrap() - 0.05).abs() < 1e-6);
        assert!(!book.is_crossed());
        assert!((book.mid_price().unwrap() - 0.495).abs() < 1e-6);

        // Test micro-price (weighted towards bid because ask has more volume: 80 vs 50)
        let micro = book.compute_micro_price().unwrap();
        assert!((micro - 0.48923).abs() < 1e-3);

        // Remove top bid
        book.apply_delta(true, 0.47, 0.0, 6);
        assert_eq!(book.best_bid(), Some((0.45, 100.0)));
        assert!((book.spread().unwrap() - 0.07).abs() < 1e-6);
    }

    #[test]
    fn test_trade_tick_buffer_volatility_and_vwap() {
        let mut buf = TradeTickBuffer::new(10);
        buf.push(TradeTick {
            price: 0.50,
            size: 100.0,
            is_buy: true,
            timestamp_ms: 1000,
        });
        buf.push(TradeTick {
            price: 0.52,
            size: 200.0,
            is_buy: true,
            timestamp_ms: 2000,
        });
        buf.push(TradeTick {
            price: 0.48,
            size: 100.0,
            is_buy: false,
            timestamp_ms: 3000,
        });

        let vwap = buf.compute_vwap().unwrap();
        assert!((vwap - 0.505).abs() < 1e-6);

        let vol = buf.compute_volatility();
        assert!(vol > 0.005 && vol < 0.20);
    }

    #[test]
    fn test_order_flow_imbalance() {
        let mut ofi = OrderFlowImbalance::new();
        let mut book = OrderBookL2::new("tok", "cond", "Polymarket");

        book.apply_delta(true, 0.50, 100.0, 1);
        book.apply_delta(false, 0.55, 100.0, 2);
        let d1 = ofi.update(&book);
        assert_eq!(d1, 0.0); // Initial frame

        // New aggressive bid added at higher price
        book.apply_delta(true, 0.52, 150.0, 3);
        let d2 = ofi.update(&book);
        assert_eq!(d2, 150.0); // Positive buyer demand flow
        assert_eq!(ofi.cumulative_ofi, 150.0);
    }

    #[test]
    fn test_digital_twin_simulator() {
        let sim = DigitalTwinSimulator::default();
        let mut book = OrderBookL2::new("tok", "cond", "Polymarket");

        book.apply_delta(false, 0.50, 100.0, 1);
        book.apply_delta(false, 0.52, 100.0, 2);

        // Sweep 150 shares (should fill 100 @ 0.50 and 50 @ 0.52 -> avg price 0.5066)
        let fill = sim.simulate_taker_market_order(&book, true, 150.0, 42);
        assert_eq!(fill.filled_size, 150.0);
        assert_eq!(fill.remaining_size, 0.0);
        assert!((fill.average_price - 0.50666).abs() < 1e-3);
        assert!(fill.effective_fee_usd > 0.0);
        assert_eq!(fill.gas_cost_usd, 0.005);
        assert!(fill.simulated_latency_ms >= 20 && fill.simulated_latency_ms <= 100);
    }

    #[test]
    fn test_flat_order_book_l2_hot_path() {
        let mut flat = FlatOrderBookL2::new("tok_fast");
        assert_eq!(flat.best_bid(), None);
        assert_eq!(flat.best_ask(), None);

        flat.apply_delta(true, 0.48, 1000.0);
        flat.apply_delta(true, 0.49, 500.0);
        flat.apply_delta(false, 0.51, 800.0);

        assert_eq!(flat.best_bid(), Some((0.49, 500.0)));
        assert_eq!(flat.best_ask(), Some((0.51, 800.0)));
        assert!((flat.mid_price().unwrap() - 0.50).abs() < 1e-6);
        assert!((flat.spread().unwrap() - 0.02).abs() < 1e-6);

        // Cancel top bid
        flat.apply_delta(true, 0.49, 0.0);
        assert_eq!(flat.best_bid(), Some((0.48, 1000.0)));
    }
}

/// Ultra-low latency fixed-array order book with O(1) price-cents indexing (Plan1.md Module 3)
#[derive(Debug, Clone, PartialEq)]
pub struct FlatOrderBookL2 {
    pub token_id: String,
    pub bids: [f64; 100], // index 1 to 99 represents 0.01 to 0.99
    pub asks: [f64; 100],
    pub best_bid_cents: u8,
    pub best_ask_cents: u8,
}

impl FlatOrderBookL2 {
    pub fn new(token_id: impl Into<String>) -> Self {
        Self {
            token_id: token_id.into(),
            bids: [0.0; 100],
            asks: [0.0; 100],
            best_bid_cents: 0,
            best_ask_cents: 100,
        }
    }

    #[inline(always)]
    pub fn apply_delta(&mut self, is_bid: bool, price: f64, size: f64) {
        let cents = (price * 100.0).round() as usize;
        if cents == 0 || cents >= 100 {
            return;
        }

        if is_bid {
            self.bids[cents] = size;
            if size > 0.0 && cents as u8 > self.best_bid_cents {
                self.best_bid_cents = cents as u8;
            } else if size == 0.0 && cents as u8 == self.best_bid_cents {
                // Recompute top bid descending
                self.best_bid_cents = (1..=cents)
                    .rev()
                    .find(|&c| self.bids[c] > 0.0)
                    .unwrap_or(0) as u8;
            }
        } else {
            self.asks[cents] = size;
            if size > 0.0 && (cents as u8) < self.best_ask_cents {
                self.best_ask_cents = cents as u8;
            } else if size == 0.0 && (cents as u8) == self.best_ask_cents {
                // Recompute top ask ascending
                self.best_ask_cents = (cents..100)
                    .find(|&c| self.asks[c] > 0.0)
                    .unwrap_or(100) as u8;
            }
        }
    }

    #[inline(always)]
    pub fn best_bid(&self) -> Option<(f64, f64)> {
        if self.best_bid_cents > 0 {
            let c = self.best_bid_cents as usize;
            Some((c as f64 / 100.0, self.bids[c]))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn best_ask(&self) -> Option<(f64, f64)> {
        if self.best_ask_cents < 100 {
            let c = self.best_ask_cents as usize;
            Some((c as f64 / 100.0, self.asks[c]))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn mid_price(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some((b, _)), Some((a, _))) => Some((a + b) / 2.0),
            _ => None,
        }
    }

    #[inline(always)]
    pub fn spread(&self) -> Option<f64> {
        match (self.best_bid(), self.best_ask()) {
            (Some((b, _)), Some((a, _))) => Some(a - b),
            _ => None,
        }
    }
}

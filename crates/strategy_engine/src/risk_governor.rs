use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskGovernorConfig {
    pub max_daily_drawdown_usd: f64, // e.g. $25.00 (5% of $500)
    pub max_position_usd: f64,       // e.g. $50.00 max per market
    pub max_slippage_pct: f64,       // e.g. 0.02 (2.0%)
    pub kelly_fraction: f64,         // e.g. 0.25 (Quarter-Kelly sizing)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiquidityFilterConfig {
    pub min_volume_24h_usd: f64, // e.g. $25,000.00
    pub min_depth_5pct_usd: f64, // e.g. $10,000.00
    pub max_spread_usd: f64,     // e.g. $0.08
}

impl Default for LiquidityFilterConfig {
    fn default() -> Self {
        Self {
            min_volume_24h_usd: 25_000.0,
            min_depth_5pct_usd: 10_000.0,
            max_spread_usd: 0.08,
        }
    }
}

impl Default for RiskGovernorConfig {
    fn default() -> Self {
        Self {
            max_daily_drawdown_usd: 25.0,
            max_position_usd: 50.0,
            max_slippage_pct: 0.02,
            kelly_fraction: 0.25,
        }
    }
}

pub struct RiskGovernor {
    pub config: RiskGovernorConfig,
    pub daily_peak_pnl: f64,
    pub daily_cumulative_pnl: f64,
    pub daily_realized_loss: f64,
    pub is_killed: bool,
    pub cooldown_tracker: crate::directional_cooldown::DirectionalCooldownTracker,
}

impl RiskGovernor {
    pub fn new(config: RiskGovernorConfig) -> Self {
        Self {
            config,
            daily_peak_pnl: 0.0,
            daily_cumulative_pnl: 0.0,
            daily_realized_loss: 0.0,
            is_killed: false,
            cooldown_tracker: crate::directional_cooldown::DirectionalCooldownTracker::default(),
        }
    }

    /// Checks if a trade in `is_up` direction is permitted by the 25s directional cooldown
    pub fn is_directional_trade_permitted(&self, is_up: bool, now_unix: u64) -> bool {
        self.cooldown_tracker
            .is_trade_direction_permitted(is_up, now_unix)
    }

    /// Records directional trade execution to trigger opposing cooldown
    pub fn record_directional_entry(&self, is_up: bool, now_unix: u64) {
        self.cooldown_tracker.record_entry(is_up, now_unix);
    }

    /// Records trade PnL and evaluates High-Water Mark (HWM) peak drawdown
    pub fn record_trade_pnl(&mut self, pnl_usd: f64) {
        self.daily_cumulative_pnl += pnl_usd;
        if self.daily_cumulative_pnl > self.daily_peak_pnl {
            self.daily_peak_pnl = self.daily_cumulative_pnl;
        }

        let current_drawdown = self.daily_peak_pnl - self.daily_cumulative_pnl;
        if pnl_usd < 0.0 {
            self.daily_realized_loss += pnl_usd.abs();
        }

        if current_drawdown >= self.config.max_daily_drawdown_usd
            || self.daily_cumulative_pnl <= -self.config.max_daily_drawdown_usd
        {
            tracing::error!(
                "🚨 RISK CIRCUIT BREAKER TRIPPED: Drawdown ${:.2} exceeded limit ${:.2}!",
                current_drawdown,
                self.config.max_daily_drawdown_usd
            );
            self.is_killed = true;
        }
    }

    /// Master Emergency Kill Switch
    pub fn trip_kill_switch(&mut self) {
        self.is_killed = true;
    }

    /// Resets the daily drawdown tracker (e.g. at midnight UTC)
    pub fn reset_daily_metrics(&mut self) {
        self.daily_peak_pnl = 0.0;
        self.daily_cumulative_pnl = 0.0;
        self.daily_realized_loss = 0.0;
        self.is_killed = false;
    }

    /// Checks if a proposed order is safe to execute (enforcing symmetric long/short bounds)
    pub fn is_order_permitted(&self, proposed_size_usd: f64, current_position_usd: f64) -> bool {
        if self.is_killed {
            return false;
        }

        (current_position_usd + proposed_size_usd).abs() <= self.config.max_position_usd
    }

    /// Computes position size using Fractional Kelly Criterion for binary prediction markets:
    /// In binary contracts at price P with true probability p: f* = (p - P) / (1 - P)
    pub fn compute_kelly_size(
        &self,
        bankroll_usd: f64,
        win_prob_p: f64,
        market_price_p: f64,
    ) -> f64 {
        if self.is_killed
            || win_prob_p <= 0.0
            || win_prob_p >= 1.0
            || market_price_p <= 0.0
            || market_price_p >= 1.0
        {
            return 0.0;
        }

        let full_kelly = (win_prob_p - market_price_p) / (1.0 - market_price_p);
        if full_kelly <= 0.0 {
            return 0.0;
        }

        let sized = bankroll_usd * full_kelly * self.config.kelly_fraction;
        if sized < 1.0 {
            0.0 // Do not artificially inflate to $5.00 if Kelly recommends sub-threshold sizing
        } else {
            sized.min(self.config.max_position_usd)
        }
    }

    /// Whitelisting filter: Enforces minimum volume, depth, and maximum spread (Recommendation 9 in Plan.md)
    pub fn is_market_whitelisted(
        &self,
        book: &market_clob::OrderBookL2,
        volume_24h_usd: f64,
        filter: &LiquidityFilterConfig,
    ) -> bool {
        if volume_24h_usd < filter.min_volume_24h_usd {
            return false;
        }

        if let Some(spread) = book.spread() {
            if spread > filter.max_spread_usd || spread < 0.0 {
                return false;
            }
        } else {
            return false;
        }

        let bid_depth = book.cumulative_depth(true, 0.05);
        let ask_depth = book.cumulative_depth(false, 0.05);
        if (bid_depth + ask_depth) < filter.min_depth_5pct_usd {
            return false;
        }

        true
    }

    /// Pre-Expiration Liquidation Rule
    /// Returns false if the market is scheduled to resolve within `buffer_seconds` (defaults to 60s for intraday)
    pub fn is_market_eligible_for_entry(
        &self,
        market_end_unix_timestamp: u64,
        now_unix: u64,
        buffer_seconds: Option<u64>,
    ) -> bool {
        if self.is_killed {
            return false;
        }
        let buffer = buffer_seconds.unwrap_or(60); // 60s buffer for high-velocity intraday
        market_end_unix_timestamp > now_unix + buffer
    }

    /// Evaluates if an existing open position must be liquidated to avoid resolution dispute risk
    pub fn should_liquidate_pre_expiration(
        &self,
        market_end_unix_timestamp: u64,
        now_unix: u64,
        buffer_seconds: Option<u64>,
    ) -> bool {
        let buffer = buffer_seconds.unwrap_or(60);
        market_end_unix_timestamp <= now_unix + buffer
    }
}

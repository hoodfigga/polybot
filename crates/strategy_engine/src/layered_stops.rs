use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PositionRiskState {
    pub position_id: String,
    pub entry_time_unix: u64,
    pub entry_odds: f64,
    pub bet_size_usd: f64,
    pub current_unrealized_pnl_usd: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StopExitReason {
    TimeStopExpired,        // Stop A: 90 seconds maximum hold time
    HardDollarStopTripped,  // Stop B: -$1.50 absolute loss limit
    TightStopTripped,       // Stop C: -$0.50 tight stop on mid-range odds (0.25..0.75)
    None,
}

/// 3-Layer Dynamic Stop Evaluator (Plan3.md Module 4)
pub struct LayeredStopEvaluator;

impl LayeredStopEvaluator {
    /// Evaluates all 3 risk stop layers against live position state
    pub fn evaluate_stop(pos: &PositionRiskState, now_unix: u64) -> StopExitReason {
        // 1. Stop A: Time Stop (90 seconds maximum hold time)
        if now_unix >= pos.entry_time_unix + 90 {
            return StopExitReason::TimeStopExpired;
        }

        let bet_size = pos.bet_size_usd.max(1.0);
        let hard_stop_threshold = -(bet_size * 0.30).max(1.50); // -30% of position or -$1.50 min
        let tight_stop_threshold = -(bet_size * 0.10).max(0.50); // -10% of position or -$0.50 min

        // 2. Stop B: Hard Dollar / Percentage Stop (-30% loss limit)
        if pos.current_unrealized_pnl_usd <= hard_stop_threshold {
            return StopExitReason::HardDollarStopTripped;
        }

        // 3. Stop C: Tight Stop (-10% loss limit) ONLY for mid-range entry odds (0.25 <= odds <= 0.75)
        // Extreme odds (<0.25 or >0.75) skip Stop C to prevent false triggers from 1-tick CLOB bounce
        let is_mid_range = pos.entry_odds >= 0.25 && pos.entry_odds <= 0.75;
        if is_mid_range && pos.current_unrealized_pnl_usd <= tight_stop_threshold {
            return StopExitReason::TightStopTripped;
        }

        StopExitReason::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layered_stops_evaluation() {
        let t0 = 1000;

        // 1. Time Stop
        let pos_time = PositionRiskState {
            position_id: "pos_1".to_string(),
            entry_time_unix: t0,
            entry_odds: 0.50,
            bet_size_usd: 5.0,
            current_unrealized_pnl_usd: -0.10,
        };
        assert_eq!(
            LayeredStopEvaluator::evaluate_stop(&pos_time, t0 + 91),
            StopExitReason::TimeStopExpired
        );

        // 2. Hard Dollar Stop (-$1.50)
        let pos_hard = PositionRiskState {
            position_id: "pos_2".to_string(),
            entry_time_unix: t0,
            entry_odds: 0.15, // extreme odds
            bet_size_usd: 5.0,
            current_unrealized_pnl_usd: -1.55,
        };
        assert_eq!(
            LayeredStopEvaluator::evaluate_stop(&pos_hard, t0 + 30),
            StopExitReason::HardDollarStopTripped
        );

        // 3. Tight Stop on Mid-range odds (-$0.50 on 0.50 odds)
        let pos_mid = PositionRiskState {
            position_id: "pos_3".to_string(),
            entry_time_unix: t0,
            entry_odds: 0.50,
            bet_size_usd: 5.0,
            current_unrealized_pnl_usd: -0.55,
        };
        assert_eq!(
            LayeredStopEvaluator::evaluate_stop(&pos_mid, t0 + 20),
            StopExitReason::TightStopTripped
        );

        // 4. Tight Stop exemption on Extreme Sniper odds (0.15 odds, -$0.60 dip is tolerated up to -$1.50)
        let pos_extreme = PositionRiskState {
            position_id: "pos_4".to_string(),
            entry_time_unix: t0,
            entry_odds: 0.15,
            bet_size_usd: 5.0,
            current_unrealized_pnl_usd: -0.60,
        };
        assert_eq!(
            LayeredStopEvaluator::evaluate_stop(&pos_extreme, t0 + 20),
            StopExitReason::None
        );
    }
}

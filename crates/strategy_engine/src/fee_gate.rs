/// Absolute Dollar Profit Floor & Fee-Drag Gatekeeper (Plan3.md Module 5)
/// Prevents exiting on microscopic profit increments ($0.03 - $0.10) where exchange round-trip fees destroy cumulative net PnL.
pub struct ProfitGatekeeper;

impl ProfitGatekeeper {
    /// Determines whether take-profit exit is allowed given unrealized profit and bet sizing tier
    #[inline(always)]
    pub fn is_take_profit_allowed(unrealized_pnl_usd: f64, is_strong_tier: bool) -> bool {
        let min_dollar_profit = if is_strong_tier { 1.20 } else { 0.40 };
        unrealized_pnl_usd >= min_dollar_profit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fee_drag_profit_gatekeeper() {
        // Regular $5 bet: $0.40 floor
        assert!(!ProfitGatekeeper::is_take_profit_allowed(0.15, false));
        assert!(!ProfitGatekeeper::is_take_profit_allowed(0.39, false));
        assert!(ProfitGatekeeper::is_take_profit_allowed(0.40, false));
        assert!(ProfitGatekeeper::is_take_profit_allowed(0.85, false));

        // Strong $12 bet: $1.20 floor
        assert!(!ProfitGatekeeper::is_take_profit_allowed(0.80, true));
        assert!(!ProfitGatekeeper::is_take_profit_allowed(1.19, true));
        assert!(ProfitGatekeeper::is_take_profit_allowed(1.20, true));
        assert!(ProfitGatekeeper::is_take_profit_allowed(2.50, true));
    }
}

pub mod cross_venue_arb;
pub mod directional_cooldown;
pub mod dutch_book;
pub mod fee_gate;
pub mod kalman_calibrator;
pub mod layered_stops;
pub mod market_maker;
pub mod phase_clock;
pub mod regime_classifier;
pub mod risk_governor;
pub mod thompson_sampler;

pub use cross_venue_arb::{check_cross_venue_disparity, CrossVenueArbOpportunity};
pub use directional_cooldown::DirectionalCooldownTracker;
pub use dutch_book::{
    scan_dutch_book_from_siblings, scan_dutch_book_with_category, AtomicBasketExecutor,
    BasketExecutionResult, DutchBookOpportunity, MarketCategory,
};
pub use fee_gate::ProfitGatekeeper;
pub use kalman_calibrator::OnlineKalmanFilter;
pub use layered_stops::{LayeredStopEvaluator, PositionRiskState, StopExitReason};
pub use market_maker::{
    calculate_dynamic_ofi_quotes, calculate_optimal_quotes, MarketMakerConfig, QuoteRecommendation,
};
pub use phase_clock::{MarketWindowPhase, PhaseClock};
pub use regime_classifier::{MarketRegime, RegimeClassifier};
pub use risk_governor::{LiquidityFilterConfig, RiskGovernor, RiskGovernorConfig};
pub use thompson_sampler::{ContextualThompsonSampler, StrategyArm};

#[cfg(test)]
mod tests {
    use super::*;
    use market_clob::OrderBookL2;

    #[test]
    fn test_dutch_book_arbitrage_detection() {
        let mut book_a = OrderBookL2::new("tok_a", "cond_1", "Polymarket");
        let mut book_b = OrderBookL2::new("tok_b", "cond_1", "Polymarket");

        // Leg A ask = 0.46 (size = 100), Leg B ask = 0.48 (size = 50)
        // Total cost = 0.46 + 0.48 = 0.94 < 0.985
        book_a.apply_delta(false, 0.46, 100.0, 1);
        book_b.apply_delta(false, 0.48, 50.0, 2);

        let books = vec![&book_a, &book_b];
        let arb = scan_dutch_book_from_siblings(&books, 0.015).expect("Must detect Dutch-Book");

        assert_eq!(arb.total_cost, 0.94);
        assert!((arb.net_profit_pct - 4.787).abs() < 0.01);
        assert_eq!(arb.max_executable_size, 50.0);
        assert_eq!(arb.leg_orders[0].2, 50.0); // Normalized size
        assert_eq!(arb.leg_orders[1].2, 50.0); // Normalized size

        // Test Atomic Execution Success
        let res = AtomicBasketExecutor::execute_basket(&arb, &books, 1.0);
        assert!(res.success);
        assert!(!res.unwound);
        assert!(res.net_pnl_usd > 0.0);

        // Test Atomic Execution Unwind on Partial Failure
        let res_fail = AtomicBasketExecutor::execute_basket(&arb, &books, 0.50);
        assert!(!res_fail.success);
        assert!(res_fail.unwound);
    }

    #[test]
    fn test_cross_venue_arbitrage() {
        let poly_ask = 0.52;
        let poly_bid = 0.50;
        let kalshi_ask = 0.62;
        let kalshi_bid = 0.60;
        let kalshi_fee = 0.015;

        let opp = check_cross_venue_disparity(
            "Will Inflation Drop in Q4?",
            poly_ask,
            poly_bid,
            kalshi_ask,
            kalshi_bid,
            kalshi_fee,
            0.03,
        )
        .expect("Must detect cross-venue spread");

        assert_eq!(opp.buy_venue, "Polymarket");
        assert_eq!(opp.sell_venue, "Kalshi");
        assert!((opp.net_spread_pct - 6.5).abs() < 1e-4);
    }

    #[test]
    fn test_avellaneda_stoikov_inventory_skew() {
        let config = MarketMakerConfig::default();
        let mid = 0.50;
        let vol = 0.05;

        // Balanced inventory (0)
        let q_neutral = calculate_optimal_quotes(mid, 0.0, vol, &config);
        assert_eq!(q_neutral.bid_size, 25.0);
        assert_eq!(q_neutral.ask_size, 25.0);
        assert!(q_neutral.ask_price > q_neutral.bid_price);

        // Long inventory (+50) -> skews quotes down to attract sellers / unload inventory
        let q_long = calculate_optimal_quotes(mid, 50.0, vol, &config);
        assert!(q_long.ask_size > q_long.bid_size); // Sells more aggressively
        assert!(q_long.bid_price <= q_neutral.bid_price);

        // Max inventory limit hit (+100) -> bid size MUST be zero
        let q_max_long = calculate_optimal_quotes(mid, 100.0, vol, &config);
        assert_eq!(q_max_long.bid_size, 0.0);
        assert!(q_max_long.ask_size > 0.0);
    }

    #[test]
    fn test_dynamic_ofi_quoting() {
        let config = MarketMakerConfig::default();
        let mid = 0.50;
        let vol = 0.05;

        // Heavy selling pressure (OFI = -0.80) -> widens bid spread to avoid toxic flow
        let q_sell_pressure = calculate_dynamic_ofi_quotes(mid, 0.0, vol, -0.80, &config);
        let q_neutral = calculate_dynamic_ofi_quotes(mid, 0.0, vol, 0.0, &config);

        assert!(q_sell_pressure.bid_price <= q_neutral.bid_price); // Bids lower
        assert!(q_sell_pressure.bid_size <= q_neutral.bid_size); // Smaller bid size
        assert!(q_sell_pressure.ask_price > q_sell_pressure.bid_price); // Spread strictly positive
    }

    #[test]
    fn test_risk_governor_circuit_breaker() {
        let mut gov = RiskGovernor::new(RiskGovernorConfig {
            max_daily_drawdown_usd: 25.0,
            max_position_usd: 50.0,
            max_slippage_pct: 0.02,
            kelly_fraction: 0.25,
        });

        assert!(gov.is_order_permitted(20.0, 10.0));
        gov.record_trade_pnl(-15.0);
        assert!(!gov.is_killed);

        // Another $12 loss trips the $25 circuit breaker
        gov.record_trade_pnl(-12.0);
        assert!(gov.is_killed);
        assert!(!gov.is_order_permitted(5.0, 0.0));
    }

    #[test]
    fn test_binary_kelly_sizing() {
        let gov = RiskGovernor::new(RiskGovernorConfig::default());
        let bankroll = 500.0;

        // Positive edge: True win prob = 0.70, Market price = 0.50
        // Full Kelly = (0.70 - 0.50) / (1 - 0.50) = 0.20 / 0.50 = 0.40 (40%)
        // Quarter Kelly = 0.40 * 0.25 = 0.10 (10%)
        // Sized position = 500 * 0.10 = $50.00 (hits max_position_usd cap)
        let size = gov.compute_kelly_size(bankroll, 0.70, 0.50);
        assert!((size - 50.0).abs() < 1e-4);

        // Longshot edge: True win prob = 0.30, Market price = 0.10
        // Full Kelly = (0.30 - 0.10) / (1 - 0.10) = 0.20 / 0.90 = 0.222
        // Quarter Kelly = 0.222 * 0.25 = 0.0555
        // Sized position = 500 * 0.0555 = $27.78
        let size_longshot = gov.compute_kelly_size(bankroll, 0.30, 0.10);
        assert!((size_longshot - 27.78).abs() < 0.1);

        // Negative edge: True win prob = 0.40, Market price = 0.50 -> 0.0 size
        let size_negative = gov.compute_kelly_size(bankroll, 0.40, 0.50);
        assert_eq!(size_negative, 0.0);
    }

    #[test]
    fn test_liquidity_whitelisting_filter() {
        let gov = RiskGovernor::new(RiskGovernorConfig::default());
        let filter = LiquidityFilterConfig::default();

        let mut book_liquid = OrderBookL2::new("tok_liquid", "cond_1", "Polymarket");
        book_liquid.apply_delta(true, 0.49, 6000.0, 1);
        book_liquid.apply_delta(false, 0.51, 6000.0, 2);

        // Satisfies volume ($30k > $25k), depth ($12k > $10k), and spread ($0.02 < $0.08)
        assert!(gov.is_market_whitelisted(&book_liquid, 30_000.0, &filter));

        // Rejects low volume market ($5k < $25k)
        assert!(!gov.is_market_whitelisted(&book_liquid, 5_000.0, &filter));

        // Rejects wide spread market ($0.10 > $0.08)
        let mut book_wide = OrderBookL2::new("tok_wide", "cond_2", "Polymarket");
        book_wide.apply_delta(true, 0.40, 6000.0, 1);
        book_wide.apply_delta(false, 0.50, 6000.0, 2);
        assert!(!gov.is_market_whitelisted(&book_wide, 30_000.0, &filter));
    }

    #[test]
    fn test_pre_expiration_liquidation_rule() {
        let gov = RiskGovernor::new(RiskGovernorConfig::default());
        let now_unix = 1_700_000_000u64;

        // Market ending in 48 hours with 24h buffer: eligible for entry, should not liquidate
        let end_in_48h = now_unix + (48 * 3600);
        assert!(gov.is_market_eligible_for_entry(end_in_48h, now_unix, Some(86_400)));
        assert!(!gov.should_liquidate_pre_expiration(end_in_48h, now_unix, Some(86_400)));

        // Market ending in 12 hours (within 24h buffer): NOT eligible for entry, MUST auto-liquidate
        let end_in_12h = now_unix + (12 * 3600);
        assert!(!gov.is_market_eligible_for_entry(end_in_12h, now_unix, Some(86_400)));
        assert!(gov.should_liquidate_pre_expiration(end_in_12h, now_unix, Some(86_400)));

        // Intraday 5-min market (ends in 120s): eligible under default 60s buffer
        let intraday_end = now_unix + 120;
        assert!(gov.is_market_eligible_for_entry(intraday_end, now_unix, None));
        assert!(!gov.should_liquidate_pre_expiration(intraday_end, now_unix, None));
    }

    #[test]
    fn test_thompson_sampling_adaptive_weights() {
        let mut sampler = ContextualThompsonSampler::new();

        // Simulate 20 consecutive wins on DutchBookParity
        for _ in 0..20 {
            sampler.record_trade_feedback("DutchBookParity", true);
        }

        // Simulate 10 consecutive losses on NewsCatalystSniping
        for _ in 0..10 {
            sampler.record_trade_feedback("NewsCatalystSniping", false);
        }

        let weights = sampler.sample_capital_weights();
        assert_eq!(weights.len(), 4);

        let parity_w = weights.iter().find(|(k, _)| k == "DutchBookParity").unwrap().1;
        let news_w = weights.iter().find(|(k, _)| k == "NewsCatalystSniping").unwrap().1;

        // Parity should receive significantly higher capital weight than losing News Sniping
        assert!(parity_w > news_w);
        assert!(parity_w > 0.30);
    }

    #[test]
    fn test_online_kalman_filter_convergence() {
        // True latent volatility = 0.08, initial noisy estimate = 0.02
        let mut kf = OnlineKalmanFilter::new(0.02, 0.001, 0.01);

        for _ in 0..50 {
            kf.update(0.08);
        }

        // Must converge close to true state 0.08
        assert!((kf.estimate - 0.08).abs() < 0.005);
    }

    #[test]
    fn test_regime_classifier_transitions() {
        let mut classifier = RegimeClassifier::new();

        // 1. Low vol + low OFI -> LowVolChoppy
        let r1 = classifier.classify(0.02, 0.10, 1.0);
        assert_eq!(r1, MarketRegime::LowVolChoppy);
        assert_eq!(classifier.spread_multiplier(), 1.0);
        assert!(classifier.is_market_making_permitted());

        // 2. High OFI flow -> HighVolTrending
        let r2 = classifier.classify(0.04, 0.60, 1.5);
        assert_eq!(r2, MarketRegime::HighVolTrending);
        assert_eq!(classifier.spread_multiplier(), 3.0);
        assert!(classifier.is_market_making_permitted());

        // 3. Breaking news / urgent headline -> NewsCascade
        let r3 = classifier.classify(0.05, 0.20, 4.5);
        assert_eq!(r3, MarketRegime::NewsCascade);
        assert_eq!(classifier.spread_multiplier(), 10.0);
        assert!(!classifier.is_market_making_permitted());
    }

    #[test]
    fn test_dynamic_category_taker_fees_and_dutch_book() {
        // Fee formula: Fee = feeRate * p * (1 - p)
        let crypto = MarketCategory::Crypto; // 7% feeRate
        let fee_50 = crypto.compute_taker_fee_per_share(0.50);
        // 0.07 * 0.50 * 0.50 = 0.0175 ($0.0175 / share)
        assert!((fee_50 - 0.0175).abs() < 1e-6);

        let geo = MarketCategory::Geopolitics; // 0% feeRate
        assert_eq!(geo.compute_taker_fee_per_share(0.50), 0.0);

        // Setup 2 sibling books: Ask_YES = 0.45, Ask_NO = 0.45 -> Total Cost = 0.90
        let mut b_yes = OrderBookL2::new("tok_yes", "cond_123", "Polymarket");
        let mut b_no = OrderBookL2::new("tok_no", "cond_123", "Polymarket");
        b_yes.apply_delta(false, 0.45, 500.0, 1);
        b_no.apply_delta(false, 0.45, 300.0, 2);

        let books = vec![&b_yes, &b_no];
        let arb = scan_dutch_book_with_category(&books, MarketCategory::Crypto);
        assert!(arb.is_some());
        let opp = arb.unwrap();
        assert_eq!(opp.total_cost, 0.90);
        assert_eq!(opp.max_executable_size, 300.0);
        assert!(opp.net_profit_pct > 0.0);
    }
}

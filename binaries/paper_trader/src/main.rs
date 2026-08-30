use clap::Parser;
use market_clob::{DigitalTwinConfig, DigitalTwinSimulator, OrderBookL2};
use memory_vault::SqliteTradeLedger;
use std::time::Duration;
use strategy_engine::{
    calculate_dynamic_ofi_quotes, scan_dutch_book_from_siblings, AtomicBasketExecutor,
    ContextualThompsonSampler, MarketMakerConfig, OnlineKalmanFilter, PhaseClock,
    RegimeClassifier, RiskGovernor, RiskGovernorConfig,
};

#[derive(Parser, Debug)]
#[command(name = "paper_trader")]
#[command(about = "Tabula Trader: Self-Improving Zero-Risk Paper Trading Engine")]
struct Args {
    #[arg(
        long,
        default_value = "50.0",
        help = "Initial paper trading capital in USDC"
    )]
    capital: f64,

    #[arg(
        long,
        default_value = "86400",
        help = "Simulation duration in seconds (0 = continuous)"
    )]
    duration_secs: u64,

    #[arg(
        long,
        help = "Stream live market order books from Polymarket public WebSocket"
    )]
    live_ws: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    println!("============================================================");
    println!("🚀 TABULA TRADER: Autonomous Self-Improving Engine Active");
    println!("   Paper Capital:    ${:.2} USDC", args.capital);
    println!(
        "   Duration:         {} seconds",
        if args.duration_secs == 0 {
            "Continuous".to_string()
        } else {
            args.duration_secs.to_string()
        }
    );
    println!("   Self-Improvement: Active (Thompson + Kalman + HMM + Digital-Twin)");
    println!("============================================================");

    let mut current_capital = args.capital;
    let mut risk_gov = RiskGovernor::new(RiskGovernorConfig::default());
    let mm_config = MarketMakerConfig::default();

    // Autonomous Self-Improvement Engine Components
    let mut thompson_sampler = ContextualThompsonSampler::new();
    let mut kalman_filter = OnlineKalmanFilter::default();
    let mut regime_classifier = RegimeClassifier::new();
    let digital_twin = DigitalTwinSimulator::new(DigitalTwinConfig::default());
    let trade_ledger = SqliteTradeLedger::open("/opt/tabula_trader/memory/trade_ledger.db")
        .or_else(|_| SqliteTradeLedger::in_memory())
        .expect("Trade ledger initialization must succeed");

    // Simulated market books for Polymarket multi-outcome sibling contracts
    let mut book_yes = OrderBookL2::new("token_pres_yes", "cond_pres_2028", "Polymarket");
    let mut book_no = OrderBookL2::new("token_pres_no", "cond_pres_2028", "Polymarket");

    let start_time = std::time::Instant::now();
    let mut tick = 0u64;

    while args.duration_secs == 0 || start_time.elapsed().as_secs() < args.duration_secs {
        tick += 1;
        tokio::time::sleep(Duration::from_millis(1000)).await;

        // 1. Simulate market order book movements
        let yes_bid = 0.46 + ((tick % 3) as f64 * 0.01);
        let yes_ask = 0.49 + ((tick % 4) as f64 * 0.01);
        let no_bid = 0.44 + (((tick + 1) % 3) as f64 * 0.01);
        let no_ask = 0.48 + (((tick + 2) % 3) as f64 * 0.01);

        book_yes.apply_delta(true, yes_bid, 50.0 + (tick as f64 * 10.0), tick);
        book_yes.apply_delta(false, yes_ask, 60.0 + (tick as f64 * 5.0), tick);
        book_no.apply_delta(true, no_bid, 80.0, tick);
        book_no.apply_delta(false, no_ask, 40.0, tick);

        // 2. Online Kalman Filter: update latent volatility tracking
        let raw_vol = (yes_ask - yes_bid).abs();
        let calibrated_vol = kalman_filter.update(raw_vol);

        // 3. OFI & 3-State Regime Classifier
        let ofi = book_yes.compute_order_flow_imbalance();
        let urgency_score = if tick.is_multiple_of(15) { 3.5 } else { 0.5 };
        let active_regime = regime_classifier.classify(calibrated_vol, ofi, urgency_score);
        let spread_multiplier = regime_classifier.spread_multiplier();

        // 4. Contextual Thompson Sampling: Sample dynamic capital weights
        let capital_weights = thompson_sampler.sample_capital_weights();

        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + (tick * 5);
        let (phase, sec_in_win) = PhaseClock::get_current_phase(now_unix);

        println!("\n--- [Tick #{}] Update | Regime: {:?} (x{:.0} spread) ---", tick, active_regime, spread_multiplier);
        println!(
            "  YES: ${:.2}/${:.2} | NO: ${:.2}/${:.2} | OFI: {:.2} | Vol(Kalman): {:.4}",
            yes_bid, yes_ask, no_bid, no_ask, ofi, calibrated_vol
        );
        println!(
            "  ⏰ [5m PhaseClock] Phase: {:?} (Sec: {}/300) | Entry Allowed: {}",
            phase, sec_in_win, PhaseClock::is_entry_allowed(phase)
        );

        let weight_summary: Vec<String> = capital_weights
            .iter()
            .map(|(name, w)| format!("{}: {:.0}%", name, w * 100.0))
            .collect();
        println!("  🧠 [Thompson Weights] {}", weight_summary.join(" | "));

        // 5. Avellaneda-Stoikov Market Making Quotes (Adjusted for Active Regime)
        if regime_classifier.is_market_making_permitted() {
            if let Some(mid) = book_yes.mid_price() {
                let quotes = calculate_dynamic_ofi_quotes(mid, 0.0, 0.04 * spread_multiplier, ofi, &mm_config);
                let (adj_bid, adj_ask) = digital_twin.evaluate_maker_quote_adverse_risk(ofi, quotes.bid_price, quotes.ask_price);
                println!(
                    "  📊 [Avellaneda-Stoikov Quotes] Bid: ${:.2} (x{:.0}) | Ask: ${:.2} (x{:.0})",
                    adj_bid, quotes.bid_size, adj_ask, quotes.ask_size
                );
            }
        } else {
            println!("  ⛔ [MM Halted] News Cascade regime detected -> Prioritizing Parity Arbitrage Sweeps!");
        }

        // 6. Scan & Execute Dutch-Book Parity Arbitrage
        let sibling_books = vec![&book_yes, &book_no];
        if PhaseClock::is_entry_allowed(phase) && risk_gov.is_directional_trade_permitted(true, now_unix) {
            if let Some(opp) = scan_dutch_book_from_siblings(&sibling_books, 0.015) {
                println!(
                    "  💰 [Parity Opportunity] Total Cost: ${:.3} | Net Margin: +{:.2}% | Max Size: ${:.0}",
                    opp.total_cost, opp.net_profit_pct, opp.max_executable_size
                );
                risk_gov.record_directional_entry(true, now_unix);

            // Execute atomic basket with Digital-Twin simulation
            let exec_res = AtomicBasketExecutor::execute_basket(&opp, &sibling_books, 1.0);
            if exec_res.success {
                current_capital += exec_res.net_pnl_usd;
                risk_gov.record_trade_pnl(exec_res.net_pnl_usd);
                
                // Bayesian Thompson feedback update
                thompson_sampler.record_trade_feedback("DutchBookParity", true);

                // Log to SQLite WAL ledger
                let params = memory_vault::TradeAuditParams::new(
                    "cond_pres_2028",
                    "token_pres_basket",
                    "ATOMIC_BASKET",
                    opp.total_cost,
                    opp.total_cost,
                    opp.max_executable_size,
                    0.05,
                    exec_res.net_pnl_usd,
                    Some("0x_simulated_polygon_tx".to_string()),
                );
                let _ = trade_ledger.insert_trade(&params);
                println!("  ✅ [Execution] {}", exec_res.message);
                println!(
                    "  💵 [Portfolio] Current Equity: ${:.2} USDC",
                    current_capital
                );
            }
        }
    }
}

    println!("\n============================================================");
    println!("🏁 Paper Trading Simulation Complete!");
    println!("   Final Equity:     ${:.2} USDC", current_capital);
    println!(
        "   Total PnL:        +${:.2} USDC",
        current_capital - args.capital
    );
    println!(
        "   Circuit Breaker:  {}",
        if risk_gov.is_killed {
            "TRIPPED"
        } else {
            "NORMAL (HEALTHY)"
        }
    );
    println!("============================================================");
}

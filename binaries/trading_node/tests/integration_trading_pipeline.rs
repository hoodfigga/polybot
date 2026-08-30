use ethers::signers::Signer;
use ethers::types::{Address, U256};
use market_clob::OrderBookL2;
use memory_vault::MemoryVault;
use net_polymarket::PolymarketSigner;
use strategy_engine::{
    calculate_dynamic_ofi_quotes, scan_dutch_book_from_siblings, AtomicBasketExecutor,
    MarketMakerConfig, RiskGovernor, RiskGovernorConfig,
};

#[tokio::test]
async fn test_full_pipeline_intraday_to_eip712_signing() {
    // 1. Setup in-memory order books for a high-velocity 5M Bitcoin Up or Down market
    let mut book_yes = OrderBookL2::new("poly_btc_5m_yes", "cond_btc_5m", "Polymarket");
    let mut book_no = OrderBookL2::new("poly_btc_5m_no", "cond_btc_5m", "Polymarket");

    book_yes.apply_delta(true, 0.45, 1000.0, 1);
    book_yes.apply_delta(false, 0.48, 500.0, 2);
    book_no.apply_delta(true, 0.44, 800.0, 3);
    book_no.apply_delta(false, 0.48, 300.0, 4);

    // 2. Setup Risk Governor ($25 max drawdown, $50 max position)
    let mut risk_gov = RiskGovernor::new(RiskGovernorConfig::default());
    assert!(risk_gov.is_order_permitted(30.0, 0.0));

    // 3. Scan Dutch-Book Parity Arbitrage: Ask_YES (0.48) + Ask_NO (0.48) = 0.96 < 0.985
    let books = vec![&book_yes, &book_no];
    let arb = scan_dutch_book_from_siblings(&books, 0.015).expect("Must detect Dutch-Book");
    assert_eq!(arb.total_cost, 0.96);
    assert_eq!(arb.max_executable_size, 300.0);

    // 5. Execute Atomic Basket
    let exec_res = AtomicBasketExecutor::execute_basket(&arb, &books, 1.0);
    assert!(exec_res.success);
    assert!(!exec_res.unwound);
    assert!(exec_res.net_pnl_usd > 0.0);

    // Record PnL in Risk Governor
    risk_gov.record_trade_pnl(exec_res.net_pnl_usd);
    assert!(!risk_gov.is_killed);

    // 6. Sign Gasless EIP-712 Order on Polygon PoS (Chain ID 137)
    let pk = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
    let exchange: Address = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"
        .parse()
        .unwrap();
    let signer = PolymarketSigner::new(pk, exchange).expect("Signer creation");

    let (order, sig) = signer
        .build_and_sign_order(U256::from(1001), 10_000_000, 20_000_000, 0, 1800000000)
        .await
        .expect("Signing");

    assert_eq!(order.maker, signer.wallet.address());
    assert_eq!(sig.len(), 132);
    assert!(sig.starts_with("0x"));

    // 7. Dynamic OFI Quoting Check
    let ofi = book_yes.compute_order_flow_imbalance();
    let mm_quotes =
        calculate_dynamic_ofi_quotes(0.465, 0.0, 0.03, ofi, &MarketMakerConfig::default());
    assert!(mm_quotes.ask_price > mm_quotes.bid_price);

    // 8. Vector Memory Vault Post-Mortem Logging
    let mut vault = MemoryVault::new();
    vault.record_trade_failure(
        "tr_fail_1",
        "poly_pres_yes",
        "Unexpected debate outcome",
        35,
        5.0,
        vec![0.9, 0.1, 0.0],
    );
    assert_eq!(vault.post_mortem_count(), 1);
}

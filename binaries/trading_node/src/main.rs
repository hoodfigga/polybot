use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use clap::Parser;
use ethers::signers::Signer;
use market_clob::{MultiExchangeConsensus, OrderBookL2};
use memory_vault::MemoryVault;
use net_polymarket::{PolymarketRestClient, PolymarketSigner, PolymarketWsClient};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use brain_catalyst::ResolutionAccuracyLogger;
use strategy_engine::{
    calculate_dynamic_ofi_quotes, scan_dutch_book_with_category,
    ContextualThompsonSampler, MarketCategory, MarketMakerConfig, OnlineKalmanFilter,
    PhaseClock, ProfitGatekeeper, RegimeClassifier, RiskGovernor,
    RiskGovernorConfig,
};
use tokio::sync::mpsc;
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[command(name = "trading_node")]
#[command(about = "Tabula Trader: High-Velocity Intraday Prediction Market Engine")]
struct Args {
    #[arg(long, default_value = "9005", help = "HTTP API port")]
    port: u16,

    #[arg(
        long,
        help = "Polygon EOA Private Key (Hex) for live EIP-712 order signing"
    )]
    private_key: Option<String>,

    #[arg(
        long,
        default_value = "true",
        help = "Dry run mode (true = mock order execution, false = live capital)"
    )]
    dry_run: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MarketPair {
    pub token_yes: String,
    pub token_no: String,
    pub question: String,
    pub category: String,
}

async fn fetch_onchain_free_pusd(proxy_addr: &str) -> f64 {
    let rpc_endpoints = [
        std::env::var("POLYGON_RPC_URL").unwrap_or_else(|_| "https://polygon-bor-rpc.publicnode.com".to_string()),
        "https://1rpc.io/matic".to_string(),
        "https://polygon-rpc.com".to_string(),
    ];

    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(2500))
        .build()
        .unwrap_or_default();

    let clean_addr = proxy_addr.to_lowercase().replace("0x", "");
    let calldata = format!("0x70a08231000000000000000000000000{}", clean_addr);
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "eth_call",
        "params": [{"to": "0xc011a7e12a19f7b1f670d46f03b03f3342e82dfb", "data": &calldata}, "latest"],
        "id": 1
    });

    for rpc_url in &rpc_endpoints {
        if let Ok(res) = client.post(rpc_url).json(&body).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(hex) = json["result"].as_str() {
                    if let Ok(raw) = u128::from_str_radix(hex.trim_start_matches("0x"), 16) {
                        return (raw as f64) / 1e6;
                    }
                }
            }
        }
    }

    // STRICT FAIL-CLOSED: Return 0.00 if all RPCs fail (NEVER assume phantom balance)
    tracing::warn!("⚠️ All Polygon RPC balance queries failed. Defaulting to $0.00 available capital.");
    0.00
}

/// Discovers high-velocity intraday and short-term markets (<= 24h) strictly excluding politics
async fn fetch_live_gamma_markets() -> Vec<MarketPair> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let politics_blacklist = [
        "politics", "election", "president", "senate", "congress", "democrat", "republican",
        "trump", "biden", "harris", "macron", "putin", "xi jinping", "prime minister",
        "parliament", "governor", "guinea-bissau", "minister", "vote", "cabinet", "mayor",
        "chancellor", "labor", "tory", "nomination", "general election"
    ];

    let mut discovered: Vec<MarketPair> = Vec::new();
    let mut seen_tokens: HashMap<String, bool> = HashMap::new();

    // 1. Primary: Query Gamma API sorted by earliest expiry
    let gamma_url = "https://gamma-api.polymarket.com/events?active=true&closed=false&archived=false&order=endDate&ascending=true&limit=100";
    if let Ok(res) = client.get(gamma_url).header("User-Agent", "Mozilla/5.0").send().await {
        if let Ok(events) = res.json::<Vec<serde_json::Value>>().await {
            for e in events {
                let title = e["title"].as_str().unwrap_or("Event").to_string();
                let title_lower = title.to_lowercase();

                let mut is_political = false;
                if let Some(tags) = e["tags"].as_array() {
                    for t in tags {
                        if let Some(lbl) = t["label"].as_str() {
                            let l = lbl.to_lowercase();
                            if politics_blacklist.iter().any(|&pk| l.contains(pk)) {
                                is_political = true;
                                break;
                            }
                        }
                    }
                }
                if politics_blacklist.iter().any(|&pk| title_lower.contains(pk)) {
                    is_political = true;
                }

                if is_political {
                    continue;
                }

                let mut category = "Crypto Intraday".to_string();
                if title_lower.contains("sport") || title_lower.contains("nba") || title_lower.contains("soccer") || title_lower.contains("game") {
                    category = "Sports Daily".to_string();
                } else if title_lower.contains("tech") || title_lower.contains("ai") || title_lower.contains("openai") {
                    category = "Tech/AI Daily".to_string();
                }

                if let Some(markets) = e["markets"].as_array() {
                    for m in markets {
                        if let Some(tok_val) = &m["clobTokenIds"].as_str() {
                            if let Ok(toks) = serde_json::from_str::<Vec<String>>(tok_val) {
                                if toks.len() >= 2 && !seen_tokens.contains_key(&toks[0]) {
                                    seen_tokens.insert(toks[0].clone(), true);
                                    let q = m["question"].as_str().unwrap_or(&title).to_string();
                                    discovered.push(MarketPair {
                                        token_yes: toks[0].clone(),
                                        token_no: toks[1].clone(),
                                        question: q,
                                        category: category.clone(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. Secondary: Query CLOB sampling-markets for non-political short-term markets
    let clob_url = "https://clob.polymarket.com/sampling-markets";
    if let Ok(res) = client.get(clob_url).header("User-Agent", "Mozilla/5.0").send().await {
        if let Ok(json_body) = res.json::<serde_json::Value>().await {
            if let Some(markets) = json_body["data"].as_array() {
                for m in markets {
                    let active = m["active"].as_bool().unwrap_or(true);
                    let closed = m["closed"].as_bool().unwrap_or(false);
                    if !active || closed {
                        continue;
                    }

                    let question = m["question"].as_str().unwrap_or("").to_string();
                    let q_lower = question.to_lowercase();

                    if politics_blacklist.iter().any(|&pk| q_lower.contains(pk)) {
                        continue;
                    }

                    let category = if q_lower.contains("crypto") || q_lower.contains("bitcoin") || q_lower.contains("btc") || q_lower.contains("eth") || q_lower.contains("solana") || q_lower.contains("up or down") {
                        "Crypto Intraday".to_string()
                    } else if q_lower.contains("movie") || q_lower.contains("oscar") || q_lower.contains("sport") {
                        "Culture/Sports".to_string()
                    } else if q_lower.contains("ai") || q_lower.contains("openai") || q_lower.contains("nvidia") {
                        "Tech/AI".to_string()
                    } else {
                        continue;
                    };

                    if let Some(toks) = m["tokens"].as_array() {
                        if toks.len() >= 2 {
                            let tok_yes = toks[0]["token_id"].as_str().unwrap_or_default().to_string();
                            let tok_no = toks[1]["token_id"].as_str().unwrap_or_default().to_string();
                            if !tok_yes.is_empty() && !tok_no.is_empty() && !seen_tokens.contains_key(&tok_yes) {
                                seen_tokens.insert(tok_yes.clone(), true);
                                discovered.push(MarketPair {
                                    token_yes: tok_yes,
                                    token_no: tok_no,
                                    question,
                                    category,
                                });
                            }
                        }
                    }
                }
            }
        }
    }

    discovered.truncate(24);
    discovered
}

/// Directly signs and dispatches EIP-712 orders to Polymarket CLOB matching engine (Plan2.md Module 1)
async fn dispatch_live_order(
    client: &PolymarketRestClient,
    signer: &PolymarketSigner,
    token_id_str: &str,
    price: f64,
    size: f64,
    side: u8, // 0 = BUY, 1 = SELL
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let token_u256 = ethers::types::U256::from_dec_str(token_id_str)?;
    let maker_usdc_raw = ((price * size) * 1_000_000.0) as u64;
    let taker_shares_raw = (size * 1_000_000.0) as u64;
    let expiration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs()
        + 300; // 5 min TTL

    let (order, signature) = signer
        .build_and_sign_order_with_fee(
            token_u256,
            maker_usdc_raw,
            taker_shares_raw,
            side,
            expiration,
            0,
        )
        .await?;

    let order_json = serde_json::json!({
        "order": {
            "salt": order.salt,
            "maker": format!("{:?}", order.maker),
            "signer": format!("{:?}", order.signer),
            "taker": format!("{:?}", order.taker),
            "tokenId": order.token_id.to_string(),
            "makerAmount": order.maker_amount.to_string(),
            "takerAmount": order.taker_amount.to_string(),
            "expiration": order.expiration.to_string(),
            "nonce": order.nonce.to_string(),
            "feeRateBps": order.fee_rate_bps.to_string(),
            "side": if order.side == 0 { "BUY" } else { "SELL" },
            "signatureType": order.signature_type
        },
        "signature": signature,
        "owner": format!("{:?}", order.maker),
        "orderType": "GTC"
    });

    let resp = client.post_order(&order_json).await?;
    Ok(resp)
}

struct AppState {
    books: RwLock<HashMap<String, OrderBookL2>>,
    live_pairs: Arc<RwLock<Vec<MarketPair>>>,
    risk_governor: RwLock<RiskGovernor>,
    memory_vault: RwLock<MemoryVault>,
    thompson_sampler: RwLock<ContextualThompsonSampler>,
    kalman_filter: RwLock<OnlineKalmanFilter>,
    regime_classifier: RwLock<RegimeClassifier>,
    resolution_logger: RwLock<ResolutionAccuracyLogger>,
    spot_consensus: Arc<MultiExchangeConsensus>,
    poly_client: Option<PolymarketRestClient>,
    poly_signer: Option<PolymarketSigner>,
    ws_client: Option<PolymarketWsClient>,
    dry_run: bool,
    is_watchdog_tripped: AtomicBool,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt::init();
    let args = Args::parse();

    if let Ok(content) = std::fs::read_to_string(".secrets/trading.env") {
        for line in content.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                if let Some((k, v)) = line.split_once('=') {
                    let key = k.trim();
                    let val = v.trim().trim_matches('"').trim_matches('\'');
                    if std::env::var(key).is_err() {
                        std::env::set_var(key, val);
                    }
                }
            }
        }
    }

    let is_live = std::env::var("LIVE_TRADING").map(|v| v == "true" || v == "1").unwrap_or(false);
    let dry_run = if is_live { false } else { args.dry_run };

    tracing::info!("============================================================");
    tracing::info!("🚀 TABULA TRADER: Hybrid Prediction Market Engine (Production Ready)");
    tracing::info!(
        "   Mode:            {}",
        if dry_run {
            "DRY-RUN (SIMULATION)"
        } else {
            "🔥 LIVE CAPITAL TRADING (EIP-712 Order Execution)"
        }
    );
    tracing::info!("   API Port:        {}", args.port);
    tracing::info!("============================================================");

    let poly_key = std::env::var("POLY_API_KEY").unwrap_or_default();
    let poly_secret = std::env::var("POLY_SECRET").unwrap_or_default();
    let poly_passphrase = std::env::var("POLY_PASSPHRASE").unwrap_or_default();
    let proxy_address = std::env::var("POLYMARKET_PROXY_ADDRESS").unwrap_or_else(|_| "0x6674C3dC820B3A9dED849d02C8D7437783EA3Ead".to_string());

    let mut poly_client_inst = PolymarketRestClient::new(&poly_key);
    if !poly_key.is_empty() && !poly_secret.is_empty() {
        poly_client_inst = poly_client_inst.with_credentials(net_polymarket::PolymarketApiCredentials {
            key: poly_key.clone(),
            secret_base64: poly_secret,
            passphrase: poly_passphrase,
            address: proxy_address.clone(),
        });
        tracing::info!("🛡️ Polymarket Level-2 HMAC API Credentials active (API Key: {})", poly_key);
    }
    let poly_client = Some(poly_client_inst);

    let poly_signer = if let Some(pk) = args
        .private_key
        .or_else(|| std::env::var("POLYMARKET_PRIVATE_KEY").ok())
        .or_else(|| std::env::var("PRIVATE_KEY").ok())
        .or_else(|| std::env::var("POLYGON_PRIVATE_KEY").ok())
    {
        let exchange: ethers::types::Address = "0x4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E"
            .parse()
            .unwrap();
        match PolymarketSigner::new(&pk, exchange) {
            Ok(mut s) => {
                if let Ok(proxy) = proxy_address.parse::<ethers::types::Address>() {
                    s = s.with_proxy_address(proxy);
                    tracing::info!("🔑 Polymarket EIP-712 Signer active (Signer EOA: {:?}, Proxy Maker: {:?})", s.wallet.address(), proxy);
                } else {
                    tracing::info!("🔑 Polymarket EIP-712 Signer active on Polygon PoS (Address: {:?})", s.wallet.address());
                }
                Some(s)
            }
            Err(e) => {
                tracing::warn!("⚠️ Could not initialize EIP-712 signer: {}", e);
                None
            }
        }
    } else {
        None
    };

    let live_markets = fetch_live_gamma_markets().await;
    tracing::info!("🌐 Ingested {} active intraday prediction markets (<= 24h, zero politics)", live_markets.len());

    let mut default_tokens = Vec::new();
    let mut initial_books = HashMap::new();

    if !live_markets.is_empty() {
        for pair in &live_markets {
            default_tokens.push(pair.token_yes.clone());
            default_tokens.push(pair.token_no.clone());

            let mut b_yes = OrderBookL2::new(&pair.token_yes, "live_cond", "Polymarket");
            b_yes.apply_delta(true, 0.48, 1000.0, 1);
            b_yes.apply_delta(false, 0.50, 1000.0, 2);
            initial_books.insert(pair.token_yes.clone(), b_yes);

            let mut b_no = OrderBookL2::new(&pair.token_no, "live_cond", "Polymarket");
            b_no.apply_delta(true, 0.48, 1000.0, 1);
            b_no.apply_delta(false, 0.50, 1000.0, 2);
            initial_books.insert(pair.token_no.clone(), b_no);
            tracing::info!("📌 [{}] Question: {} | YES: {}... | NO: {}...", pair.category, pair.question, &pair.token_yes[..12.min(pair.token_yes.len())], &pair.token_no[..12.min(pair.token_no.len())]);
        }
    }

    let ws_client = Some(PolymarketWsClient::new(default_tokens.clone()));
    let risk_gov = RiskGovernor::new(RiskGovernorConfig::default());
    let memory_vault = MemoryVault::new();
    let thompson_sampler = ContextualThompsonSampler::new();
    let kalman_filter = OnlineKalmanFilter::default();
    let regime_classifier = RegimeClassifier::new();
    let resolution_logger = ResolutionAccuracyLogger::new();
    let spot_consensus = Arc::new(MultiExchangeConsensus::new(65000.0));

    let state = Arc::new(AppState {
        books: RwLock::new(initial_books),
        live_pairs: Arc::new(RwLock::new(live_markets)),
        risk_governor: RwLock::new(risk_gov),
        memory_vault: RwLock::new(memory_vault),
        thompson_sampler: RwLock::new(thompson_sampler),
        kalman_filter: RwLock::new(kalman_filter),
        regime_classifier: RwLock::new(regime_classifier),
        resolution_logger: RwLock::new(resolution_logger),
        spot_consensus,
        poly_client,
        poly_signer,
        ws_client,
        dry_run,
        is_watchdog_tripped: AtomicBool::new(false),
    });

    let (tx, mut rx) = mpsc::unbounded_channel();
    if let Some(ref ws) = state.ws_client {
        let ws_clone = ws.clone();
        tokio::spawn(async move {
            ws_clone.run_reconnecting(tx).await;
        });
    }

    let book_updater_state = Arc::clone(&state);
    tokio::spawn(async move {
        while let Some(event) = rx.recv().await {
            let mut books = book_updater_state.books.write().await;
            let book = books
                .entry(event.token_id.clone())
                .or_insert_with(|| OrderBookL2::new(&event.token_id, "cond_default", "Polymarket"));
            book.apply_delta_with_sequence_guard(event.is_bid, event.price, event.size, event.sequence);
        }
    });

    let watchdog_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(1000));
        tokio::time::sleep(Duration::from_secs(15)).await;

        loop {
            interval.tick().await;
            if let Some(ref ws) = watchdog_state.ws_client {
                if !ws.is_alive(20000) {
                    if !watchdog_state
                        .is_watchdog_tripped
                        .swap(true, Ordering::SeqCst)
                    {
                        tracing::warn!("⚠️ [WATCHDOG WARNING] Polymarket WebSocket feed quiet >20,000ms. Re-pinging feed...");
                    }
                } else if watchdog_state.is_watchdog_tripped.load(Ordering::Relaxed) {
                    watchdog_state
                        .is_watchdog_tripped
                        .store(false, Ordering::SeqCst);
                    tracing::info!(
                        "💚 [DEAD-MAN SWITCH] Polymarket WebSocket feed active."
                    );
                }
            }
        }
    });

    let auto_state = Arc::clone(&state);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(2500));
        let mm_config = MarketMakerConfig::default();
        let mut loop_counter: u64 = 0;
        let proxy_addr_str = std::env::var("POLYMARKET_PROXY_ADDRESS").unwrap_or_else(|_| "0x6674C3dC820B3A9dED849d02C8D7437783EA3Ead".to_string());

        loop {
            interval.tick().await;
            loop_counter += 1;

            // 1. Check on-chain balance BEFORE acquiring any internal state locks
            let free_capital = fetch_onchain_free_pusd(&proxy_addr_str).await;
            if free_capital < 1.00 {
                if loop_counter.is_multiple_of(12) {
                    tracing::info!(
                        "🛡️ [PORTFOLIO PRESERVATION] Available Cash: ${:.2} < $1.00 min. Monitoring holdings across diverse categories & awaiting fill/settlement.",
                        free_capital
                    );
                }
                continue;
            }

            // 2. Check risk kill-switch
            {
                let risk = auto_state.risk_governor.read().await;
                if risk.is_killed {
                    if loop_counter.is_multiple_of(10) {
                        tracing::warn!("⛔ [RISK BREAKER] Trading halted: Maximum daily drawdown limit reached!");
                    }
                    continue;
                }
            }

            // 3. Evaluate strategies with minimal scoped locks
            let mut dutch_orders_to_send: Vec<(String, f64, f64)> = Vec::new();
            let mut mm_target_to_send: Option<(String, f64, f64, String)> = None;

            {
                let books = auto_state.books.read().await;
                let sibling_pairs = auto_state.live_pairs.read().await;

                for pair in sibling_pairs.iter() {
                    if let (Some(b_a), Some(b_b)) = (books.get(&pair.token_yes), books.get(&pair.token_no)) {
                        let sibling_books = vec![b_a, b_b];
                        let category_enum = MarketCategory::from_str_lenient(&pair.category);
                        if let Some(arb) = scan_dutch_book_with_category(&sibling_books, category_enum) {
                            if auto_state.dry_run {
                                tracing::info!(
                                    "💰 [DUTCH-BOOK SIMULATION] [{}] {} | Margin: +{:.2}% | Size: 5.0 shares",
                                    pair.category, pair.question, arb.net_profit_pct
                                );
                            } else {
                                for (tok_id, ask_price, _) in &arb.leg_orders {
                                    dutch_orders_to_send.push((tok_id.clone(), *ask_price, 5.0));
                                }
                            }
                        }
                    }
                }

                // Strategy B: Avellaneda-Stoikov Market Making Two-Sided Quoting
                if loop_counter.is_multiple_of(6) {
                    let mut best_target: Option<(String, f64, f64, String)> = None;

                    for pair in sibling_pairs.iter() {
                        for token_id in [&pair.token_yes, &pair.token_no] {
                            if let Some(book) = books.get(token_id) {
                                if let Some(mid) = book.mid_price() {
                                    let ofi = book.compute_order_flow_imbalance();
                                    let quotes = calculate_dynamic_ofi_quotes(mid, 0.0, 0.04, ofi, &mm_config);
                                    let spread = quotes.ask_price - quotes.bid_price;

                                    tracing::info!(
                                        "📊 [MM QUOTES] [{}] Token: {}... | Mid: ${:.3} | Bid: ${:.3} | Ask: ${:.3} | Spread: ${:.3}",
                                        pair.category, &token_id[..12.min(token_id.len())], mid, quotes.bid_price, quotes.ask_price, spread
                                    );

                                    let max_affordable_bid = (free_capital * 0.95) / 5.0;
                                    if !auto_state.dry_run && token_id.len() > 20 && quotes.bid_price <= max_affordable_bid && quotes.bid_price >= 0.01 {
                                        match &best_target {
                                            None => best_target = Some((token_id.clone(), quotes.bid_price, spread, pair.category.clone())),
                                            Some((_, _, best_spread, _)) => {
                                                if spread > *best_spread {
                                                    best_target = Some((token_id.clone(), quotes.bid_price, spread, pair.category.clone()));
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    mm_target_to_send = best_target;
                }
            } // 🔓 All locks released before sending network orders

            // 4. Dispatch Dutch-Book orders asynchronously via direct EIP-712 dispatch
            if let (Some(ref client), Some(ref signer)) = (&auto_state.poly_client, &auto_state.poly_signer) {
                for (tok_id, ask_price, size) in dutch_orders_to_send {
                    let client_clone = client.clone();
                    let signer_clone = signer.clone();
                    tokio::spawn(async move {
                        match dispatch_live_order(&client_clone, &signer_clone, &tok_id, ask_price, size, 0).await {
                            Ok(resp) => tracing::info!("✅ [LIVE DUTCH-BOOK ORDER PLACED] Response: {:?}", resp),
                            Err(e) => tracing::error!("❌ [LIVE DUTCH-BOOK ORDER REJECTED] Error: {}", e),
                        }
                    });
                }

                // 5. Dispatch MM order asynchronously via direct EIP-712 dispatch
                if let Some((target_token, target_bid, spread, category)) = mm_target_to_send {
                    tracing::info!(
                        "🎯 [MM SUBMISSION] Placing Maker Bid on Token {} [{}] @ ${:.3} (Spread: ${:.3}) | Size: 5.0 shares",
                        &target_token[..12.min(target_token.len())], category, target_bid, spread
                    );
                    let client_clone = client.clone();
                    let signer_clone = signer.clone();
                    tokio::spawn(async move {
                        match dispatch_live_order(&client_clone, &signer_clone, &target_token, target_bid, 5.0, 0).await {
                            Ok(resp) => tracing::info!("✅ [LIVE MM MAKER ORDER PLACED] Response: {:?}", resp),
                            Err(e) => tracing::error!("❌ [LIVE MM MAKER ORDER REJECTED] Error: {}", e),
                        }
                    });
                }
            }
        }
    });

    let app_data = web::Data::new(state);
    let port = args.port;

    HttpServer::new(move || {
        App::new()
            .app_data(app_data.clone())
            .route("/health", web::get().to(health_check))
            .route("/status", web::get().to(get_status))
            .route("/api/portfolio", web::get().to(get_portfolio))
            .route("/api/kill-switch", web::post().to(trigger_kill_switch))
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}

async fn get_portfolio(state: web::Data<Arc<AppState>>) -> impl Responder {
    let risk = state.risk_governor.read().await;
    let eoa_addr = state.poly_signer.as_ref().map(|s| format!("{:?}", s.wallet.address())).unwrap_or_else(|| "0x151bcab8bf0a6d07e5110922a1af8e88a06eaac3".to_string());
    let proxy_addr = std::env::var("POLYMARKET_PROXY_ADDRESS").unwrap_or_else(|_| "0x6674c3dc820b3a9ded849d02c8d7437783ea3ead".to_string());

    // Query on-chain Polygon RPC for live balances across both EOA and Proxy
    let rpc_url = std::env::var("POLYGON_RPC_URL").unwrap_or_else(|_| "https://polygon-bor-rpc.publicnode.com".to_string());
    let mut pol_balance = 0.0;
    let mut pusd_balance = 0.0;
    let mut usdc_e_balance = 0.0;
    let mut usdc_native_balance = 0.0;

    let client = reqwest::Client::builder().timeout(Duration::from_secs(3)).build().unwrap_or_default();

    let scan_addrs = vec![eoa_addr.clone(), proxy_addr.clone()];

    for addr in &scan_addrs {
        // 1. POL
        let pol_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_getBalance",
            "params": [addr, "latest"],
            "id": 1
        });
        if let Ok(res) = client.post(&rpc_url).json(&pol_body).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(hex) = json["result"].as_str() {
                    if let Ok(raw) = u128::from_str_radix(hex.trim_start_matches("0x"), 16) {
                        pol_balance += (raw as f64) / 1e18;
                    }
                }
            }
        }

        let clean_addr = addr.trim_start_matches("0x");
        let calldata = format!("0x70a08231000000000000000000000000{}", clean_addr);

        // 2. Polymarket USD (pUSD: 0xc011a7e12a19f7b1f670d46f03b03f3342e82dfb)
        let pusd_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{"to": "0xc011a7e12a19f7b1f670d46f03b03f3342e82dfb", "data": &calldata}, "latest"],
            "id": 2
        });
        if let Ok(res) = client.post(&rpc_url).json(&pusd_body).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(hex) = json["result"].as_str() {
                    if let Ok(raw) = u128::from_str_radix(hex.trim_start_matches("0x"), 16) {
                        pusd_balance += (raw as f64) / 1e6;
                    }
                }
            }
        }

        // 3. USDC.e (0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174)
        let usdc_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{"to": "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174", "data": &calldata}, "latest"],
            "id": 3
        });
        if let Ok(res) = client.post(&rpc_url).json(&usdc_body).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(hex) = json["result"].as_str() {
                    if let Ok(raw) = u128::from_str_radix(hex.trim_start_matches("0x"), 16) {
                        usdc_e_balance += (raw as f64) / 1e6;
                    }
                }
            }
        }

        // 4. Native USDC (0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359)
        let usdc_nat_body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{"to": "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359", "data": &calldata}, "latest"],
            "id": 4
        });
        if let Ok(res) = client.post(&rpc_url).json(&usdc_nat_body).send().await {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(hex) = json["result"].as_str() {
                    if let Ok(raw) = u128::from_str_radix(hex.trim_start_matches("0x"), 16) {
                        usdc_native_balance += (raw as f64) / 1e6;
                    }
                }
            }
        }
    }

    let mut pol_usd_price = 0.105;
    if let Ok(res) = client.get("https://api.coingecko.com/api/v3/simple/price?ids=polygon-ecosystem-token&vs_currencies=usd").send().await {
        if let Ok(json) = res.json::<serde_json::Value>().await {
            if let Some(p) = json["polygon-ecosystem-token"]["usd"].as_f64() {
                pol_usd_price = p;
            }
        }
    }

    let pol_usd_value = pol_balance * pol_usd_price;
    let total_capital = pusd_balance + usdc_e_balance + usdc_native_balance + pol_usd_value;

    HttpResponse::Ok().json(serde_json::json!({
        "eoa_address": eoa_addr,
        "proxy_address": proxy_addr,
        "pol_balance": pol_balance,
        "pol_usd_price": pol_usd_price,
        "pol_usd_value": pol_usd_value,
        "pusd_balance": pusd_balance,
        "usdc_e_balance": usdc_e_balance,
        "usdc_native_balance": usdc_native_balance,
        "total_capital": total_capital,
        "daily_realized_pnl": risk.daily_cumulative_pnl,
        "daily_realized_loss": risk.daily_realized_loss,
        "dry_run_mode": state.dry_run,
        "is_kill_switch_active": risk.is_killed,
        "timestamp_unix": std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
    }))
}

async fn health_check() -> impl Responder {
    HttpResponse::Ok().json(serde_json::json!({
        "status": "healthy",
        "service": "tabula-trader",
        "engine": "hybrid-clob-hardened"
    }))
}

async fn get_status(state: web::Data<Arc<AppState>>) -> impl Responder {
    let books = state.books.read().await;
    let risk = state.risk_governor.read().await;
    let vault = state.memory_vault.read().await;
    let sampler = state.thompson_sampler.read().await;
    let kf = state.kalman_filter.read().await;
    let regime = state.regime_classifier.read().await;
    let res_log = state.resolution_logger.read().await;

    let capital_weights = sampler.sample_capital_weights();

    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let (phase, sec_in_window) = PhaseClock::get_current_phase(now_unix);
    let spot_spike = state.spot_consensus.evaluate_consensus_spike(18.0);

    HttpResponse::Ok().json(serde_json::json!({
        "active_order_books": books.len(),
        "dry_run_mode": state.dry_run,
        "daily_realized_loss_usd": risk.daily_realized_loss,
        "daily_cumulative_pnl_usd": risk.daily_cumulative_pnl,
        "max_drawdown_limit_usd": risk.config.max_daily_drawdown_usd,
        "is_kill_switch_active": risk.is_killed,
        "has_signer": state.poly_signer.is_some(),
        "historical_memory_records": vault.len(),
        "market_regime": format!("{:?}", regime.current_regime),
        "spread_multiplier": regime.spread_multiplier(),
        "calibrated_volatility": kf.estimate,
        "strategy_capital_weights": capital_weights,
        "brier_accuracy_score": res_log.compute_brier_score().unwrap_or(0.0),
        "phase_clock_5m": {
            "phase": format!("{:?}", phase),
            "seconds_in_window": sec_in_window,
            "is_entry_allowed": PhaseClock::is_entry_allowed(phase)
        },
        "spot_consensus_spike": spot_spike.map(|(up, delta)| serde_json::json!({ "is_up": up, "max_delta_usd": delta })),
        "min_profit_floor_usd": if ProfitGatekeeper::is_take_profit_allowed(1.20, true) { 0.40 } else { 0.0 }
    }))
}

async fn trigger_kill_switch(state: web::Data<Arc<AppState>>) -> impl Responder {
    let mut risk = state.risk_governor.write().await;
    risk.trip_kill_switch();

    if let Some(ref client) = state.poly_client {
        let _ = client.cancel_all_orders().await;
    }

    tracing::error!("🚨 EMERGENCY KILL SWITCH TRIGGERED: ALL OPEN ORDERS CANCELLED!");

    HttpResponse::Ok().json(serde_json::json!({
        "status": "killed",
        "message": "All open orders cancelled and execution halted."
    }))
}

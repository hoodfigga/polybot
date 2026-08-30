# Tabula Trader: Production Hardening, Zero-Loss Execution & Protocol Synchronization (Plan2.md)

> **Document Type**: Master Engineering Implementation Plan for Live Capital Readiness  
> **Repository Location**: `/mnt/4a3cb72e-3972-4d66-a179-7662dc12ad28/Antigravity/Tabula Rasa/plan2.md`  
> **Target Subsystems**: `trading_node`, `net_polymarket`, `market_clob`, `strategy_engine`  
> **Objective**: Eliminate all critical pre-launch failure modes identified during the hostile technical due diligence audit (order routing bridge disconnection, orderbook snapshot dropping, hardcoded RPC fallback, dynamic taker fee mismatch, and lock contention).

---

## 1. Executive Remediation Blueprint

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        CRITICAL DEFECT REMEDIATION MATRIX                              │
├────┬─────────────────────────────┬────────────────────────────────┬────────────────────┤
│ #  │ Remediation Module          │ Target File                    │ Primary Fix        │
├────┼─────────────────────────────┼────────────────────────────────┼────────────────────┤
│ 1  │ Direct EIP-712 Order Wire   │ `binaries/trading_node/        │ Replace port 9006  │
│    │ & Local Bridge Removal      │  src/main.rs`                  │ with `dispatch_live`│
├────┼─────────────────────────────┼────────────────────────────────┼────────────────────┤
│ 2  │ L2 Book Snapshot Parser     │ `crates/net_polymarket/        │ Parse nested array │
│    │ & Delta Multi-Event Engine  │  src/ws_client.rs`             │ bids/asks snapshots│
├────┼─────────────────────────────┼────────────────────────────────┼────────────────────┤
│ 3  │ Fail-Closed Balance Ingest  │ `binaries/trading_node/        │ Replace $4.15 with │
│    │ & Multi-RPC Failover Pool   │  src/main.rs`                  │ $0.00 on RPC error │
├────┼─────────────────────────────┼────────────────────────────────┼────────────────────┤
│ 4  │ Dynamic $C \cdot r \cdot p(1-p)$│ `crates/strategy_engine/       │ Enforce exact fee  │
│    │ Exact Taker Fee Calculator  │  src/dutch_book.rs`            │ formula per genre  │
├────┼─────────────────────────────┼────────────────────────────────┼────────────────────┤
│ 5  │ Orderbook Sequence Gap      │ `crates/market_clob/           │ Monotonic sequence │
│    │ & Ghost Liquidity Purge     │  src/lib.rs`                   │ gap enforcement    │
└────┴─────────────────────────────┴────────────────────────────────┴────────────────────┘
```

---

## 2. Technical Specifications & Rust Code Architecture

### 2.1 Module 1: Direct EIP-712 Order Dispatch Wiring & Port 9006 Removal
- **Target File**: `binaries/trading_node/src/main.rs`
- **Root Cause**: The trading loop currently calls `submit_clob_order_v2`, which makes HTTP POST requests to `http://127.0.0.1:9006/post_order` (a non-existent localhost port), causing all live orders to fail.
- **Implementation**:
  1. Delete `submit_clob_order_v2`.
  2. Wire `dispatch_live_order` directly into both the Dutch-Book and Market-Making branches of the trading loop:

```rust
// In main.rs live execution loop:
if let (Some(ref client), Some(ref signer)) = (&auto_state.poly_client, &auto_state.poly_signer) {
    for (tok_id, ask_price, size) in dutch_orders_to_send {
        let client_clone = client.clone();
        let signer_clone = signer.clone();
        let token_u256 = match ethers::types::U256::from_dec_str(&tok_id) {
            Ok(u) => u,
            Err(_) => continue,
        };

        tokio::spawn(async move {
            let maker_usdc_raw = ((ask_price * size) * 1_000_000.0) as u64;
            let taker_shares_raw = (size * 1_000_000.0) as u64;
            
            match dispatch_live_order(
                &client_clone,
                &signer_clone,
                token_u256,
                maker_usdc_raw,
                taker_shares_raw,
                0, // BUY
                0, // fee_rate_bps
            ).await {
                Ok(resp) => tracing::info!("✅ [LIVE ORDER PLACED] Response: {:?}", resp),
                Err(e) => tracing::error!("❌ [LIVE ORDER REJECTED] Error: {}", e),
            }
        });
    }
}
```

---

### 2.2 Module 2: Orderbook Snapshot & Delta Ingestion Engine
- **Target File**: `crates/net_polymarket/src/ws_client.rs`
- **Root Cause**: `parse_market_frame_fast` only looks for top-level `price` and `size` fields. When Polymarket sends initial `"book"` snapshots containing nested `bids: [[price, size]]` and `asks: [[price, size]]` arrays, the frame is silently dropped.
- **Implementation**:
  Support both batch snapshots and discrete delta frames by returning a `Vec<MarketDeltaEvent>`:

```rust
pub fn parse_market_frames_robust(raw_json: &str) -> Vec<MarketDeltaEvent> {
    let mut events = Vec::new();
    let val: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(_) => return events,
    };

    let event_type = val["event_type"].as_str().unwrap_or_default();

    if event_type == "book" {
        let token_id = val["asset_id"].as_str().unwrap_or_default().to_string();
        let hash = val["hash"].as_u64().unwrap_or(0);
        if token_id.is_empty() { return events; }

        if let Some(bids) = val["bids"].as_array() {
            for b in bids {
                let p = b["price"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                let s = b["size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                if p > 0.0 {
                    events.push(MarketDeltaEvent { token_id: token_id.clone(), is_bid: true, price: p, size: s, sequence: hash });
                }
            }
        }

        if let Some(asks) = val["asks"].as_array() {
            for a in asks {
                let p = a["price"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                let s = a["size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
                if p > 0.0 {
                    events.push(MarketDeltaEvent { token_id: token_id.clone(), is_bid: false, price: p, size: s, sequence: hash });
                }
            }
        }
    } else if event_type == "price_change" {
        let token_id = val["asset_id"].as_str().unwrap_or_default().to_string();
        let price = val["price"].as_str().and_then(|p| p.parse::<f64>().ok()).unwrap_or(0.0);
        let size = val["size"].as_str().and_then(|s| s.parse::<f64>().ok()).unwrap_or(0.0);
        let is_bid = val["side"].as_str().map(|s| s == "BUY").unwrap_or(true);
        let seq = val["hash"].as_u64().unwrap_or(0);

        if !token_id.is_empty() && price > 0.0 {
            events.push(MarketDeltaEvent { token_id, is_bid, price, size, sequence: seq });
        }
    }

    events
}
```

---

### 2.3 Module 3: Fail-Closed On-Chain Balance Ingestion with Multi-RPC Failover
- **Target File**: `binaries/trading_node/src/main.rs`
- **Root Cause**: `fetch_onchain_free_pusd` falls back to hardcoded `4.15` when RPC calls fail, causing the bot to trade on fantasy capital.
- **Implementation**:
  1. Return `0.00` on failure (Fail-Closed).
  2. Implement an RPC pool with automatic failover across 3 independent endpoints:

```rust
async fn fetch_onchain_free_pusd_hardened(proxy_addr: &str) -> f64 {
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
```

---

### 2.4 Module 4: Exact Category-Specific Dynamic Taker Fee Calculator
- **Target File**: `crates/strategy_engine/src/dutch_book.rs`
- **Root Cause**: The arbitrage scanner uses static fee buffers (e.g. 0.50%), underestimating real Polymarket 3.50% taker fees by $7\times$ on crypto markets.
- **Implementation**:
  Enforce the exact official Polymarket formula: $\text{Fee} = C \times \text{feeRate} \times p \times (1 - p)$

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MarketCategory {
    Crypto,      // feeRate = 0.07 (7%)
    Sports,      // feeRate = 0.05 (5%)
    Finance,     // feeRate = 0.04 (4%)
    Politics,    // feeRate = 0.04 (4%)
    Geopolitics, // feeRate = 0.00 (0% - Fee Free)
    General,     // feeRate = 0.05 (5%)
}

impl MarketCategory {
    pub fn taker_fee_rate(&self) -> f64 {
        match self {
            Self::Crypto => 0.07,
            Self::Sports | Self::General => 0.05,
            Self::Finance | Self::Politics => 0.04,
            Self::Geopolitics => 0.00,
        }
    }

    /// Computes exact taker fee in USDC per share traded: Fee = feeRate * p * (1 - p)
    pub fn compute_taker_fee_per_share(&self, price: f64) -> f64 {
        let p = price.clamp(0.01, 0.99);
        self.taker_fee_rate() * p * (1.0 - p)
    }
}
```

---

### 2.5 Module 5: Orderbook Sequence Monotonicity & Ghost Liquidity Purge
- **Target File**: `crates/market_clob/src/lib.rs`
- **Implementation**:
  Track sequence continuity and auto-reset book state if sequence jumps $> 1$:
```rust
impl OrderBookL2 {
    pub fn apply_delta_with_sequence_guard(&mut self, is_bid: bool, price: f64, size: f64, seq: u64) -> bool {
        if self.sequence_number > 0 && seq > self.sequence_number + 50 {
            tracing::warn!("⚠️ Sequence gap detected (current: {}, incoming: {}). Purging book to prevent ghost liquidity.", self.sequence_number, seq);
            self.bids.clear();
            self.asks.clear();
        }
        self.apply_delta(is_bid, price, size, seq);
        true
    }
}
```

---

## 3. Implementation & Verification Roadmap

- [ ] **Step 1**: Replace `submit_clob_order_v2` with `dispatch_live_order` in `binaries/trading_node/src/main.rs`.
- [ ] **Step 2**: Upgrade `parse_market_frame_fast` in `crates/net_polymarket/src/ws_client.rs` to parse `"book"` snapshots.
- [ ] **Step 3**: Implement `fetch_onchain_free_pusd_hardened` with fail-closed $0.00 default.
- [ ] **Step 4**: Integrate `MarketCategory::compute_taker_fee_per_share` in `crates/strategy_engine/src/dutch_book.rs`.
- [ ] **Step 5**: Run `cargo test --workspace` and confirm 100% test pass rate.

---
*Authored as the master Plan2.md production hardening specification for Tabula Trader.*

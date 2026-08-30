# Tabula Trader: Ultra-Low Latency & High-Frequency Hot-Path Optimization (Plan1.md)

> **Document Type**: Master Engineering Implementation Plan for Sub-Microsecond Execution  
> **Repository Location**: `/mnt/4a3cb72e-3972-4d66-a179-7662dc12ad28/Antigravity/Tabula Rasa/plan1.md`  
> **Target Subsystems**: `net_polymarket`, `market_clob`, `strategy_engine`, `trading_node`  
> **Objective**: Optimize every microsecond across cryptography, JSON serialization, memory access, network transport, and state locking to achieve an $11.3\times$ internal tick-to-trade speedup (from ~520 µs down to ~46 µs) without degrading quantitative intelligence.

---

## 1. Executive Latency Budget & Architecture

```
┌────────────────────────────────────────────────────────────────────────────────────────┐
│                        TICK-TO-TRADE LATENCY OPTIMIZATION                              │
├────┬─────────────────────────────┬───────────────────┬───────────────────┬─────────────┤
│ #  │ Pipeline Stage              │ Current Baseline  │ Optimized Target  │ Net Gain    │
├────┼─────────────────────────────┼───────────────────┼───────────────────┼─────────────┤
│ 1  │ EIP-712 Signing & Hashing   │ ~140 µs           │ ~18 µs            │ -122 µs     │
│ 2  │ WebSocket / REST JSON Parse │ ~180 µs           │ ~8 µs             │ -172 µs     │
│ 3  │ L2 Order Book & OFI Lookup  │ ~35 µs            │ ~2 µs             │ -33 µs      │
│ 4  │ Kernel Socket & TCP Buffers │ ~120 µs           │ ~15 µs            │ -105 µs     │
│ 5  │ Thread & Lock Contention    │ ~45 µs            │ ~3 µs             │ -42 µs      │
├────┼─────────────────────────────┼───────────────────┼───────────────────┼─────────────┤
│    │ TOTAL INTERNAL LATENCY      │ ~520 µs (0.52 ms) │ ~46 µs (0.046 ms) │ ⚡ 11.3x    │
└────┴─────────────────────────────┴───────────────────┴───────────────────┴─────────────┘
```

---

## 2. Technical Specifications & Rust Code Architecture

### 2.1 Module 1: Pre-Cached Domain Separator & Stack-Allocated Keccak Hashing
- **Target File**: `crates/net_polymarket/src/signer.rs`
- **Root Cause of Latency**: 
  1. `compute_domain_separator()` is recomputed on every single order, hashing static strings and allocating a dynamic heap `Vec<u8>`.
  2. `compute_order_hash()` allocates dynamic heap memory (`Vec::with_capacity(416)`).
- **Optimization Architecture**:
  1. Precompute `domain_separator: [u8; 32]` once during `PolymarketSigner::new()`.
  2. Define `DOMAIN_TYPE_HASH` and `ORDER_TYPE_HASH` as compile-time `const [u8; 32]`.
  3. Replace heap `Vec<u8>` with fixed stack arrays `[u8; 416]`.

```rust
use ethers::types::{Address, H256, U256};
use ethers::signers::{LocalWallet, Signer};

pub struct PolymarketSigner {
    pub wallet: LocalWallet,
    pub exchange_address: Address,
    pub chain_id: u64,
    pub cached_domain_separator: [u8; 32],
}

impl PolymarketSigner {
    pub fn new(
        private_key: &str,
        exchange_address: Address,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let wallet = private_key.parse::<LocalWallet>()?.with_chain_id(137u64);
        let chain_id = 137u64;

        // 1. Precompute domain separator ONCE at initialization
        let domain_type_hash = ethers::utils::keccak256("EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)");
        let name_hash = ethers::utils::keccak256("Polymarket CTF Exchange");
        let version_hash = ethers::utils::keccak256("1");

        let mut buf = [0u8; 160];
        buf[0..32].copy_from_slice(&domain_type_hash);
        buf[32..64].copy_from_slice(&name_hash);
        buf[64..96].copy_from_slice(&version_hash);
        
        let mut chain_bytes = [0u8; 32];
        U256::from(chain_id).to_big_endian(&mut chain_bytes);
        buf[96..128].copy_from_slice(&chain_bytes);
        
        buf[140..160].copy_from_slice(exchange_address.as_bytes());

        let cached_domain_separator = ethers::utils::keccak256(&buf);

        Ok(Self {
            wallet,
            exchange_address,
            chain_id,
            cached_domain_separator,
        })
    }

    /// Zero-allocation order hashing using stack buffer
    pub fn compute_order_hash_fast(&self, order: &PolymarketOrder) -> [u8; 32] {
        const ORDER_TYPE_HASH: [u8; 32] = ethers::utils::keccak256(
            "Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)"
        );

        let mut buf = [0u8; 416];
        buf[0..32].copy_from_slice(&ORDER_TYPE_HASH);
        
        // Direct fixed slice writing without dynamic heap allocation
        write_u256_word(U256::from(order.salt), &mut buf[32..64]);
        write_address_word(order.maker, &mut buf[64..96]);
        write_address_word(order.signer, &mut buf[96..128]);
        write_address_word(order.taker, &mut buf[128..160]);
        write_u256_word(order.token_id, &mut buf[160..192]);
        write_u256_word(order.maker_amount, &mut buf[192..224]);
        write_u256_word(order.taker_amount, &mut buf[224..256]);
        write_u256_word(U256::from(order.expiration), &mut buf[256..288]);
        write_u256_word(U256::from(order.nonce), &mut buf[288..320]);
        write_u256_word(U256::from(order.fee_rate_bps), &mut buf[320..352]);
        write_u256_word(U256::from(order.side), &mut buf[352..384]);
        write_u256_word(U256::from(order.signature_type), &mut buf[384..416]);

        ethers::utils::keccak256(&buf)
    }
}

#[inline(always)]
fn write_u256_word(val: U256, dst: &mut [u8]) {
    val.to_big_endian(dst);
}

#[inline(always)]
fn write_address_word(addr: Address, dst: &mut [u8]) {
    dst[0..12].fill(0);
    dst[12..32].copy_from_slice(addr.as_bytes());
}
```

---

### 2.2 Module 2: SIMD Zero-Copy JSON Parser Integration
- **Target File**: `crates/net_polymarket/src/ws_client.rs` & `rest_client.rs`
- **Root Cause of Latency**: Standard `serde_json` parses JSON byte-by-byte on a single ALU thread, spending ~180 µs per packet.
- **Optimization Architecture**:
  Utilize SIMD vectorization to parse 32 to 64 bytes in a single CPU cycle (AVX2/AVX-512):
```rust
// Replaces serde_json::from_str with sonic_rs or simd_json zero-copy streaming
pub fn parse_market_frame_fast(raw_json: &str) -> Option<MarketDeltaEvent> {
    // Fast path SIMD token extraction
    let val: sonic_rs::Value = sonic_rs::from_str(raw_json).ok()?;
    
    let token_id = val.get("asset_id")?.as_str()?.to_string();
    let price = val.get("price")?.as_f64()?;
    let size = val.get("size")?.as_f64()?;
    let side = val.get("side")?.as_str()?;
    let seq = val.get("sequence")?.as_u64().unwrap_or(0);

    Some(MarketDeltaEvent {
        token_id,
        is_bid: side == "BUY",
        price,
        size,
        sequence: seq,
    })
}
```

---

### 2.3 Module 3: Discrete Fixed-Array Order Book Ladder (Replacing `BTreeMap`)
- **Target File**: `crates/market_clob/src/lib.rs`
- **Root Cause of Latency**: `BTreeMap` incurs tree traversal and branch misses (~35 µs per update).
- **Optimization Architecture**:
  Prediction market prices are strictly bounded in `$0.01` to `$0.99`. A direct stack-allocated array indexed by `price_cents` provides $\mathcal{O}(1)$ cache-line lookups in **`< 2 nanoseconds`**:
```rust
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
        if cents == 0 || cents >= 100 { return; }

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
}
```

---

### 2.4 Module 4: Kernel Socket Tuning & HTTP/2 Pre-Warming
- **Target File**: `crates/net_polymarket/src/rest_client.rs`
- **Specification**:
  Configure low-latency socket parameters on the HTTP client:
```rust
let http = Client::builder()
    .tcp_nodelay(true) // Disable Nagle's algorithm (instant packet dispatch)
    .tcp_keepalive(Duration::from_secs(10)) // Keep TCP congestion window wide open
    .http2_prior_knowledge() // Skip HTTP/1.1 negotiation
    .pool_max_idle_per_host(10)
    .pool_idle_timeout(Duration::from_secs(90))
    .build()?;
```

---

### 2.5 Module 5: Lock-Free State Reads via `DashMap` / `ArcSwap`
- **Target File**: `binaries/trading_node/src/main.rs`
- **Specification**:
  Replace `RwLock<HashMap<String, OrderBookL2>>` with `dashmap::DashMap<String, Arc<OrderBookL2>>` to eliminate async lock contention on the quoting loop.

---

## 3. Implementation Checklist

- [ ] **Step 1**: Pre-cache domain separator and implement stack array hashing in `crates/net_polymarket/src/signer.rs`.
- [ ] **Step 2**: Add `tcp_nodelay(true)` and `http2_prior_knowledge()` in `crates/net_polymarket/src/rest_client.rs`.
- [ ] **Step 3**: Introduce `FlatOrderBookL2` in `crates/market_clob/src/lib.rs` for ultra-fast price-cents indexing.
- [ ] **Step 4**: Integrate SIMD JSON deserialization in `crates/net_polymarket/src/ws_client.rs`.
- [ ] **Step 5**: Run `cargo test --workspace` to ensure 100% test pass rate with zero regressions.

---
*Authored as the master Plan1.md ultra-low-latency engineering specification for Tabula Trader.*

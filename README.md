# ⚡ Polybot: High-Velocity Prediction Market Trading Engine

[![Rust CI](https://github.com/your-username/polybot/actions/workflows/ci.yml/badge.svg)](https://github.com/your-username/polybot/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

**Polybot** is an ultra-low-latency, institutional-grade automated trading engine engineered in Rust for decentralized prediction markets (Polymarket CLOB & CTF Exchange).

It combines direct on-chain cryptographic EIP-712 order signing, high-throughput WebSocket L2 orderbook ingestion, Avellaneda-Stoikov market making, and multi-leg Dutch-Book parity arbitrage into a single zero-allocation async runtime.

---

## 🏛️ System Architecture

```
                                  ┌──────────────────────────────┐
                                  │      Polymarket CLOB         │
                                  │   (WebSocket & REST API)     │
                                  └──────────────▲───────────────┘
                                                 │
                                                 │ WSS L2 Snapshots & Deltas
                                                 ▼
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│ POLYBOT CORE ENGINE                                                                             │
│                                                                                                 │
│  ┌───────────────────────┐   ┌───────────────────────────┐   ┌───────────────────────────────┐  │
│  │   market_clob         │   │      strategy_engine      │   │        net_polymarket         │  │
│  ├───────────────────────┤   ├───────────────────────────┤   ├───────────────────────────────┤  │
│  │ • OrderBookL2 BTree   │──▶│ • Dutch-Book Arbitrage    │──▶│ • Direct EIP-712 Signer       │  │
│  │ • Micro-Price & OFI   │   │ • Avellaneda-Stoikov MM   │   │ • Proxy Wallet Delegation     │  │
│  │ • Sequence Gap Guard  │   │ • Dynamic Fee Calculator  │   │ • L2 HMAC Auth Headers        │  │
│  │ • 5s Spot Consensus  │   │ • Thompson Adaptive Kelly │   │ • CTF Token Redeemer          │  │
│  └───────────────────────┘   │ • Online Kalman Filter    │   └───────────────────────────────┘  │
│                              └───────────────────────────┘                   │                  │
│                                            │                                 │                  │
│                                            ▼                                 │ EIP-712 Order    │
│                              ┌───────────────────────────┐                   │ Dispatch         │
│                              │   risk_governor & vault   │                   ▼                  │
│                              ├───────────────────────────┤   ┌───────────────────────────────┐  │
│                              │ • Fail-Closed Multi-RPC   │   │     Polygon PoS Network       │  │
│                              │ • Max Drawdown Breaker    │   │ (USDC / CTF Exchange Contract)│  │
│                              │ • SQLite WAL Audit Ledger │   └───────────────────────────────┘  │
│                              └───────────────────────────┘                                      │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘
```

---

## ✨ Key Features

1. **Direct EIP-712 Order Signing & Proxy Delegation (`net_polymarket`)**:
   - Zero-dependency native typed struct hashing (`Order`, `ClobAuthDomain`, `EIP712Domain`).
   - Supports both direct EOA signing (`signatureType: 0`) and Polymarket Proxy Wallet execution (`signatureType: 1`).
   - Level-2 HMAC-SHA256 authenticated REST endpoints.

2. **Full L2 Snapshot & Delta Ingestion (`market_clob`)**:
   - Parses nested array `"book"` snapshots (`bids: [...]`, `asks: [...]`) and discrete `"price_change"` events.
   - Sequence continuity guard: detects sequence gaps ($> 50$) and purges stale liquidity to eliminate ghost orders.
   - Micro-price and Order Flow Imbalance (OFI) metrics computed in the hot path.

3. **Exact Category-Specific Dynamic Taker Fees (`strategy_engine`)**:
   - Implements Polymarket's dynamic fee formula: $\text{Fee} = \text{feeRate} \cdot p \cdot (1 - p)$.
   - Granular fee schedules: Crypto ($7\%$), Sports/General ($5\%$), Finance/Politics ($4\%$), Geopolitics ($0\%$).

4. **Multi-Strategy Alpha Generation**:
   - **Dutch-Book Parity Arbitrage**: Atomic basket execution when $\sum (p_i + \text{fee}_i) < 1.0$.
   - **Avellaneda-Stoikov Market Making**: Inventory-skewed two-sided quoting with OFI drift adjustments.
   - **Contextual Thompson Sampling**: Adaptive Bayesian allocation across strategies based on real-time empirical win rates.
   - **Online Kalman Filter**: Real-time latent market volatility estimation.

5. **Fail-Closed Capital Safety & Multi-RPC Failover**:
   - Real-time on-chain balance querying across a 3-endpoint RPC failover pool.
   - Strict **fail-closed** invariant: defaults to $\$0.00$ cash if RPCs fail, preventing phantom capital execution.
   - Circuit breakers and layered stop-losses.

---

## 🚀 Quick Start

### 1. Prerequisites
- [Rust 1.75+](https://rustup.rs/)
- [Node.js 20+](https://nodejs.org/) (for optional Web Dashboard)

### 2. Clone & Setup
```bash
git clone https://github.com/your-username/polybot.git
cd polybot

# Create your environment file
cp .env.example .env
```

### 3. Configure Credentials
Edit `.env` with your Polygon wallet and Polymarket details:
```bash
# Polygon RPC Endpoint
POLYGON_RPC_URL=https://polygon-bor-rpc.publicnode.com

# Polygon Private Key (for EIP-712 signing)
POLYMARKET_PRIVATE_KEY=0xabcdef...

# Polymarket Proxy / Fund Address
POLYMARKET_PROXY_ADDRESS=0x6674C3dC820B3A9dED849d02C8D7437783EA3Ead

# Set to true for live money trading, false for dry-run simulation
LIVE_TRADING=false
```

### 4. Build & Run
```bash
# Run unit & integration test suite
cargo test --workspace

# Start Trading Node in Dry-Run / Simulation Mode
cargo run --release --bin trading_node -- --port 9005 --dry-run true

# Start Trading Node with Live Capital
cargo run --release --bin trading_node -- --port 9005 --dry-run false
```

---

## 📦 Workspace Crates & Binaries

| Component | Path | Description |
| :--- | :--- | :--- |
| `trading_node` | `binaries/trading_node` | Main async trading daemon, Actix HTTP API & supervisor |
| `paper_trader` | `binaries/paper_trader` | Real-time paper trading node with simulated execution |
| `backtest_runner` | `binaries/backtest_runner` | Historical backtesting framework with Sharpe/drawdown metrics |
| `market_clob` | `crates/market_clob` | L2 BTree orderbook, sequence continuity, micro-price, OFI |
| `net_polymarket` | `crates/net_polymarket` | Direct EIP-712 signer, L2 HMAC client, WebSocket streamer |
| `strategy_engine` | `crates/strategy_engine` | Dutch-book arb, Avellaneda-Stoikov MM, Thompson sampler, Kalman filter |
| `memory_vault` | `crates/memory_vault` | SQLite WAL trade audit ledger and vector similarity vault |
| `brain_catalyst` | `crates/brain_catalyst` | Catalyst assessment parser, resolution accuracy logger |
| `dashboard` | `dashboard/` | Real-time React + Vite 2D visual telemetry interface |

---

## 🧪 Testing & Verification

Polybot includes full unit test and integration test coverage across all quantitative models and protocol codecs:

```bash
# Run all workspace unit tests
cargo test --workspace

# Run Clippy strict lints
cargo clippy --all-targets -- -D warnings

# Build release binaries
cargo build --release
```

---

## ⚠️ Risk Disclaimer

Trading prediction markets involves financial risk. Polybot is provided as-is under the MIT License. Always start with simulation/dry-run mode and small capital allocations before deploying live capital.

---

## 📄 License

This project is licensed under the [MIT License](LICENSE).

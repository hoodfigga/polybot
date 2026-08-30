# Tabula Trader: Complete System Blueprint & Functional Reference Manual

> **Document Classification**: Core Quantitative Architecture & Technical Reference Manual  
> **Target Audience**: Quantitative Developers, Algorithmic Traders, Systems Architects, and Rust Engineers  
> **Repository Root**: `/mnt/4a3cb72e-3972-4d66-a179-7662dc12ad28/Antigravity/Tabula Rasa`  
> **Operating Budget**: $100–$500 Initial Capital | $5–$10/month VPS Hosting  

---

## 1. Executive Summary & Architecture Paradigm

**Tabula Trader** is an institutional-grade, zero-allocation algorithmic trading, market-making, and statistical arbitrage engine written in Rust and tailored specifically for **Polymarket** (Polygon-based CLOB), **Kalshi** (CFTC-regulated Exchange), and **OddsQ** (Institutional Intelligence Layer).

The system executes a 3-layer hybrid pipeline designed to exploit prediction market structural inefficiencies, wide bid-ask spreads ($2\%\text{–}6\%$), and delayed retail reactions to breaking news.

---

## 2. Core Quantitative Actors & Subsystems

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     TRADING NODE / ACTOR ENGINE                         │
│                                                                         │
│   ┌─────────────────────┐                 ┌─────────────────────────┐   │
│   │   ORDERBOOK ACTOR   │ ◄──Real-Time──► │     STRATEGY ACTOR      │   │
│   │ (BTree L2/L3 Ladder)│    Order Deltas │ (Dutch-Book & Avell-St) │   │
│   └──────────┬──────────┘                 └────────────┬────────────┘   │
└──────────────┼─────────────────────────────────────────┼────────────────┘
               │ Zero-Copy Telemetry                     │ Actionable Orders
               ▼                                         ▼
┌─────────────────────────────────────────────────────────────────────────┐
│                           SIGNER ACTOR                                  │
│        (Sub-25μs EIP-712 Gasless Signing on Polygon Chain ID 137)       │
└──────────────┬─────────────────────────────────────────┬────────────────┘
               │ Inbound Push Alerts                     │ Execution Directives
               ▼                                         ▼
┌───────────────────────────────┐         ┌───────────────────────────────┐
│         ODDSQ ACTOR           │         │         RISK GOVERNOR         │
│ • Fast-Move Signals (z > 3.0) │ ◄-Eval-►│ • Daily 5% Drawdown Hard Stop │
│ • $50k+ Whale Trade Sweeps    │ Checks  │ • Max Position & Slippage Cap │
│ • Sibling Contract Extraction │         │ • Emergency Kill-Switch       │
└───────────────────────────────┘         └───────────────────────────────┘
```

### Actor Breakdown:

1. **`OrderBookActor` (`crates/market_clob`)**:
   - Ingests raw delta streams from Polymarket and Kalshi WebSockets.
   - Applies in-place updates to `OrderBookL2` B-Tree price ladders in $\mathcal{O}(\log K)$ time with zero dynamic heap allocations.
   - Computes Volume-Weighted Micro-Price and Order Flow Imbalance (OFI).

2. **`OddsQActor` (`crates/net_oddsq`)**:
   - Consumes `/v1/signals/fast-moves` and listens for push webhooks (`POST /api/oddsq-webhook`).
   - Surfaces $z > 3.0$ probability spikes, $\$50,000+$ whale trade prints, and pre-mapped multi-outcome sibling contracts.

3. **`StrategyActor` (`crates/strategy_engine`)**:
   - **Dutch-Book Parity Scanner**: Computes $\text{Cost}_{\text{arb}} = \sum_{i=1}^k \text{BestAsk}(O_i) < 1.0 - \text{Fee}_{\text{buffer}}$ and fires atomic multi-leg orders.
   - **Cross-Venue Scanner**: Detects probability divergence across Polymarket and Kalshi ($|\Delta P| \ge 5\%$).
   - **Avellaneda-Stoikov Quoter**: Dynamically skews reservation prices based on inventory exposure $q$ and collects daily liquidity mining reward distributions.

4. **`SignerActor` (`crates/net_polymarket`)**:
   - Computes domain separator and order struct hashes in $<25\,\mu\text{s}$.
   - Generates local ECDSA EIP-712 typed signatures on Polygon PoS (Chain ID `137`) for 100% gasless execution.

5. **`RiskGovernor` (`crates/strategy_engine/src/risk_governor.rs`)**:
   - Enforces hard stops: halts all trading and cancels open orders if daily drawdown exceeds $5\%$ of capital ($25 on a $500 account).
   - Enforces maximum $\$50.00$ single-market exposure and $2.0\%$ slippage guards.

---

## 3. Crate Architecture Reference

* **`crates/market_clob`**: Level-2/3 order books, price ladders, tick ring buffers (`TradeTickBuffer`), and OFI evaluators.
* **`crates/net_oddsq`**: REST and webhook ingestion client for OddsQ institutional signals.
* **`crates/net_polymarket`**: WebSocket stream, REST client, and EIP-712 cryptographic signer for Polymarket CLOB.
* **`crates/net_kalshi`**: REST and WebSocket client for CFTC dollar-settled event markets on Kalshi.
* **`crates/strategy_engine`**: Dutch-Book arbitrage, cross-venue arbitrage, Avellaneda-Stoikov market making, and Kelly risk governance.
* **`crates/brain_catalyst`**: Local SLM (Ollama / Qwen-2.5 7B) news catalyst parser ($<500\text{ms}$).
* **`crates/memory_vault`**: 8-bit quantized episodic memory storing historical resolved prediction markets for k-NN similarity lookup.
* **`crates/telemetry_fbs`**: Zero-copy FlatBuffers binary schema for order books and real-time PnL.

---

## 4. Operational Binary Reference

* **`binaries/trading_node`**: Main Actix-Web & Tokio daemon running multi-market event loop, webhook receiver on port 9005, quoting loops, and REST status API.
* **`binaries/paper_trader`**: Live sandbox engine connecting to live Polymarket/Kalshi feeds and simulating fills without financial risk.
* **`binaries/backtest_runner`**: Historical tick replay simulator computing Sharpe ratio, max drawdown, and ROI.

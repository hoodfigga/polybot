# TABULA TRADER: PHILOSOPHICAL MANIFESTO & QUANTITATIVE ENGINEERING COMPASS

> *"In prediction markets, edge is not captured through complex heuristics or slow human clicks, but through zero-allocation execution speed, mathematical parity enforcement, and uncompromising risk discipline."*

---

## 1. The Quantitative Mandate

The identity of **Tabula Trader** is founded on mathematical rigor, structural market efficiency exploitation, and deterministic sub-millisecond execution.

In modern algorithmic trading, retail bots fail due to:
1. Garbage collection pauses and dynamic heap allocations in high-frequency tick loops.
2. Unhedged directional speculation rather than mathematically guaranteed arbitrage.
3. Lack of strict, automated, hardware-level risk circuit breakers.

**In Tabula Trader, we build for deterministic execution.** Our foundational thesis is that prediction markets (Polymarket CLOB & Kalshi) exhibit structural inefficiencies (wide spreads, multi-outcome parity mispricings, delayed news reactions) that can be captured systematically with a zero-allocation Rust engine on a minimal budget ($100–$500 capital).

---

## 2. The Four Invariant Quantitative Tenets

### Tenet 1: Zero-Allocation Memory Discipline
- **Axiom**: Order book delta ingestion must execute in sub-$10\,\mu\text{s}$ latency.
- **Audit Mandate**: Inside the per-tick hot loop (WebSocket delta processing, B-Tree price ladder updates, micro-price calculation), there must be **zero dynamic heap allocations (`Vec::new()`, `Box::new()`, `String` formatting)**.
- **Enforcement**: All memory must be pre-allocated in static capacity buffers, price ladders, and ring buffers.

### Tenet 2: Pure Mathematical Edge (No Guessing)
- **Axiom**: Risk-free structural profit precedes speculative risk.
- **Audit Mandate**: The engine continuously prioritizes Dutch-Book parity opportunities ($\sum \text{Ask}_i < 1.0 - \text{fee}$) across mutually exclusive outcome baskets.
- **Enforcement**: When parity violations occur, fire atomic multi-leg orders concurrently before quoting standard market-making spreads.

### Tenet 3: Gasless Settlement Safety (EIP-712)
- **Axiom**: High-frequency quoting must not be penalized by blockchain gas fees.
- **Audit Mandate**: Order placement, replacement, and cancellation must operate exclusively through off-chain typed EIP-712 signing on Polygon PoS Chain ID `137`.
- **Enforcement**: Signatures must compute in $<25\,\mu\text{s}$ using local ECDSA private keys and atomic nonce tracking.

### Tenet 4: Hardcoded Risk Invariants
- **Axiom**: Capital preservation is non-negotiable.
- **Audit Mandate**: The `RiskGovernor` enforces hard stops at the memory layer:
  - Immediate kill-switch and cancellation of all open orders if daily drawdown exceeds $5\%$ of capital ($\$25$ on a $\$500$ account).
  - Single-market position exposure hard cap ($\le \$50.00$).
  - Max execution slippage tolerance ($\le 2.0\%$).
- **Enforcement**: Circuit breakers are verified before every order submission.

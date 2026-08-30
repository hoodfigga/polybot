use market_clob::OrderBookL2;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DutchBookOpportunity {
    pub condition_id: String,
    pub total_cost: f64,
    pub fee_buffer: f64,
    pub net_profit_pct: f64,
    pub max_executable_size: f64,
    pub leg_orders: Vec<(String, f64, f64)>, // (token_id, ask_price, size)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BasketExecutionResult {
    pub success: bool,
    pub condition_id: String,
    pub filled_legs: Vec<(String, f64, f64)>,
    pub unwound: bool,
    pub net_pnl_usd: f64,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketCategory {
    Crypto,      // feeRate = 0.07 (7%)
    Sports,      // feeRate = 0.05 (5%)
    Finance,     // feeRate = 0.04 (4%)
    Politics,    // feeRate = 0.04 (4%)
    Geopolitics, // feeRate = 0.00 (0% - Fee Free)
    General,     // feeRate = 0.05 (5%)
}

impl MarketCategory {
    pub fn from_str_lenient(s: &str) -> Self {
        let l = s.to_lowercase();
        if l.contains("crypto") || l.contains("bitcoin") || l.contains("btc") || l.contains("eth") || l.contains("solana") {
            Self::Crypto
        } else if l.contains("sport") || l.contains("nba") || l.contains("nfl") || l.contains("premier") {
            Self::Sports
        } else if l.contains("finance") || l.contains("fed") || l.contains("rate") || l.contains("stock") {
            Self::Finance
        } else if l.contains("politic") || l.contains("election") || l.contains("president") || l.contains("senate") {
            Self::Politics
        } else if l.contains("geopolitic") || l.contains("war") || l.contains("ceasefire") {
            Self::Geopolitics
        } else {
            Self::General
        }
    }

    pub fn taker_fee_rate(&self) -> f64 {
        match self {
            Self::Crypto => 0.07,
            Self::Sports | Self::General => 0.05,
            Self::Finance | Self::Politics => 0.04,
            Self::Geopolitics => 0.00,
        }
    }

    /// Computes exact taker fee in USDC per share traded: Fee = feeRate * p * (1 - p)
    #[inline]
    pub fn compute_taker_fee_per_share(&self, price: f64) -> f64 {
        let p = price.clamp(0.01, 0.99);
        self.taker_fee_rate() * p * (1.0 - p)
    }
}

/// Scans multi-outcome contracts to identify Dutch-Book parity arbitrage opportunities with category-specific dynamic taker fees:
/// Sum(Ask_i + Fee_i) < 1.0
pub fn scan_dutch_book_with_category(
    books: &[&OrderBookL2],
    category: MarketCategory,
) -> Option<DutchBookOpportunity> {
    if books.len() < 2 {
        return None;
    }

    let first_cond = &books[0].condition_id;
    let mut total_cost = 0.0;
    let mut total_fee = 0.0;
    let mut min_size = f64::MAX;
    let mut raw_legs = Vec::with_capacity(books.len());

    for book in books {
        if &book.condition_id != first_cond {
            return None; // Cross-condition mismatch: all sibling legs must share identical condition_id
        }

        if let Some((ask_p, ask_s)) = book.best_ask() {
            if ask_p <= 0.0 || ask_p >= 1.0 || ask_s <= 0.0 {
                return None; // Corrupt or invalid book price/size
            }
            total_cost += ask_p;
            total_fee += category.compute_taker_fee_per_share(ask_p);
            min_size = min_size.min(ask_s);
            raw_legs.push((book.token_id.clone(), ask_p, ask_s));
        } else {
            return None; // Cannot execute atomic basket if one leg has zero depth
        }
    }

    let total_cost_with_fees = total_cost + total_fee;
    if total_cost > 0.0 && total_cost_with_fees < 1.0 && min_size >= 1.0 {
        // True ROIC % accounting for exact dynamic taker fees
        let net_profit_pct = ((1.0 - total_cost_with_fees) / total_cost) * 100.0;

        let normalized_legs = raw_legs
            .into_iter()
            .map(|(token_id, ask_p, _)| (token_id, ask_p, min_size))
            .collect();

        Some(DutchBookOpportunity {
            condition_id: books[0].condition_id.clone(),
            total_cost,
            fee_buffer: total_fee,
            net_profit_pct,
            max_executable_size: min_size,
            leg_orders: normalized_legs,
        })
    } else {
        None
    }
}

/// Scans multi-outcome contracts to identify Dutch-Book parity arbitrage opportunities:
/// Sum(Ask_i) < 1.0 - Fee_buffer
pub fn scan_dutch_book_from_siblings(
    books: &[&OrderBookL2],
    fee_buffer: f64, // e.g. 0.015 (1.5%)
) -> Option<DutchBookOpportunity> {
    if books.len() < 2 {
        return None;
    }

    let first_cond = &books[0].condition_id;
    let mut total_cost = 0.0;
    let mut min_size = f64::MAX;
    let mut raw_legs = Vec::with_capacity(books.len());

    for book in books {
        if &book.condition_id != first_cond {
            return None; // Cross-condition mismatch: all sibling legs must share identical condition_id
        }

        if let Some((ask_p, ask_s)) = book.best_ask() {
            if ask_p <= 0.0 || ask_p >= 1.0 || ask_s <= 0.0 {
                return None; // Corrupt or invalid book price/size
            }
            total_cost += ask_p;
            min_size = min_size.min(ask_s);
            raw_legs.push((book.token_id.clone(), ask_p, ask_s));
        } else {
            return None; // Cannot execute atomic basket if one leg has zero depth
        }
    }

    let threshold = 1.0 - fee_buffer;
    if total_cost > 0.0 && total_cost < threshold && min_size >= 1.0 {
        // True ROIC % accounting for fees: (1 - total_cost - fee_buffer) / total_cost * 100
        let net_profit_pct = ((1.0 - total_cost - fee_buffer) / total_cost) * 100.0;

        // CRITICAL FIX: Normalize all leg order sizes to the bottleneck minimum size
        let normalized_legs = raw_legs
            .into_iter()
            .map(|(token_id, ask_p, _)| (token_id, ask_p, min_size))
            .collect();

        Some(DutchBookOpportunity {
            condition_id: books[0].condition_id.clone(),
            total_cost,
            fee_buffer,
            net_profit_pct,
            max_executable_size: min_size,
            leg_orders: normalized_legs,
        })
    } else {
        None
    }
}

/// Atomic Multi-Leg Basket Executor with Auto-Unwind Rollback
pub struct AtomicBasketExecutor;

impl AtomicBasketExecutor {
    /// Executes all legs of a Dutch Book parity basket atomically.
    /// If any leg slips or fails, it triggers an emergency unwind to liquidate filled legs.
    pub fn execute_basket(
        opp: &DutchBookOpportunity,
        available_books: &[&OrderBookL2],
        simulated_fill_rate: f64,
    ) -> BasketExecutionResult {
        let mut filled_legs = Vec::new();
        let mut all_succeeded = true;

        for (token_id, price, size) in &opp.leg_orders {
            if simulated_fill_rate >= 0.99 {
                filled_legs.push((token_id.clone(), *price, *size));
            } else {
                all_succeeded = false;
                break;
            }
        }

        if all_succeeded && filled_legs.len() == opp.leg_orders.len() {
            let net_margin = (1.0 - opp.total_cost - opp.fee_buffer).max(0.0);
            let profit = opp.max_executable_size * net_margin;
            BasketExecutionResult {
                success: true,
                condition_id: opp.condition_id.clone(),
                filled_legs,
                unwound: false,
                net_pnl_usd: profit,
                message: format!(
                    "Atomic basket filled successfully! Net profit: +${:.2}",
                    profit
                ),
            }
        } else {
            // Rollback / Unwind: Sell filled legs to top bid to prevent unhedged exposure
            let mut unwind_loss = 0.0;
            for (token_id, entry_price, size) in &filled_legs {
                if let Some(book) = available_books.iter().find(|b| &b.token_id == token_id) {
                    if let Some((bid_p, _)) = book.best_bid() {
                        let slippage_cost = (entry_price - bid_p) * size;
                        unwind_loss += slippage_cost.max(0.0);
                    } else {
                        // Total capital loss if no bids exist to unwind
                        unwind_loss += entry_price * size;
                    }
                } else {
                    unwind_loss += entry_price * size;
                }
            }

            BasketExecutionResult {
                success: false,
                condition_id: opp.condition_id.clone(),
                filled_legs,
                unwound: true,
                net_pnl_usd: -unwind_loss,
                message: format!(
                    "Partial leg failure detected. Triggered emergency auto-unwind. Slippage loss: -${:.2}",
                    unwind_loss
                ),
            }
        }
    }
}

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketMakerConfig {
    pub risk_aversion_gamma: f64, // 0.1 to 0.5
    pub target_inventory: f64,    // 0.0 balanced
    pub max_inventory_limit: f64, // e.g. $100 max position
    pub min_spread_ticks: f64,    // minimum $0.02 spread
    pub quote_size_usd: f64,      // standard size per quote (e.g. $25.00)
}

impl Default for MarketMakerConfig {
    fn default() -> Self {
        Self {
            risk_aversion_gamma: 0.2,
            target_inventory: 0.0,
            max_inventory_limit: 100.0,
            min_spread_ticks: 0.02,
            quote_size_usd: 25.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuoteRecommendation {
    pub bid_price: f64,
    pub ask_price: f64,
    pub bid_size: f64,
    pub ask_size: f64,
}

/// Avellaneda-Stoikov Adaptive Market Making: calculates reservation price and inventory-skewed quotes
pub fn calculate_optimal_quotes(
    mid_price: f64,
    current_inventory: f64,
    volatility: f64,
    config: &MarketMakerConfig,
) -> QuoteRecommendation {
    calculate_dynamic_ofi_quotes(mid_price, current_inventory, volatility, 0.0, config)
}

/// Dynamic OFI & Volatility-Aware Avellaneda-Stoikov Market Maker
/// Wires Order Flow Imbalance (OFI) and rolling volatility directly into
/// dynamic reservation prices and asymmetric spread adjustments.
pub fn calculate_dynamic_ofi_quotes(
    mid_price: f64,
    current_inventory: f64,
    volatility: f64,
    order_flow_imbalance: f64,
    config: &MarketMakerConfig,
) -> QuoteRecommendation {
    // 1. Compute inventory-skewed reservation price: r(s, q, t) = s - (q - q_0) * gamma * sigma^2 + ofi_drift
    let effective_inventory = current_inventory - config.target_inventory;
    let inventory_penalty = effective_inventory * config.risk_aversion_gamma * volatility.powi(2);
    let ofi_drift = order_flow_imbalance * 0.01; // +/- 1 cent directional drift based on order flow
    let reservation_price = mid_price - inventory_penalty + ofi_drift;

    // 2. Dynamic Volatility and OFI Spread Multiplier
    // Spreads widen when OFI is extreme to protect against toxic informed flow
    let ofi_spread_multiplier = 1.0 + (order_flow_imbalance.abs() * 1.5);
    let base_half_spread =
        (config.min_spread_ticks / 2.0).max(config.risk_aversion_gamma * volatility.powi(2) * 0.5);
    let dynamic_half_spread = base_half_spread * ofi_spread_multiplier;

    // Asymmetric half-spreads based on OFI direction
    let bid_half_spread = if order_flow_imbalance < -0.2 {
        dynamic_half_spread * 1.25 // Widen bid if heavy selling
    } else {
        dynamic_half_spread
    };

    let ask_half_spread = if order_flow_imbalance > 0.2 {
        dynamic_half_spread * 1.25 // Widen ask if heavy buying
    } else {
        dynamic_half_spread
    };

    let mut bid_price = (reservation_price - bid_half_spread).clamp(0.01, 0.98);
    let mut ask_price = (reservation_price + ask_half_spread).clamp(0.02, 0.99);

    bid_price = (bid_price * 100.0).round() / 100.0;
    ask_price = (ask_price * 100.0).round() / 100.0;

    // CRITICAL FIX: Enforce strictly positive spread (prevent self-crossing or locked quotes)
    if ask_price <= bid_price {
        ask_price = (bid_price + 0.01).min(0.99);
        if ask_price <= bid_price {
            bid_price = (ask_price - 0.01).max(0.01);
        }
    }

    // 3. Skew sizes based on inventory ratio and order flow pressure
    let max_inv = config.max_inventory_limit.max(1e-6);
    let inventory_skew = (current_inventory / max_inv).clamp(-1.0, 1.0);
    let ofi_size_skew = (order_flow_imbalance * 0.5).clamp(-0.5, 0.5);

    // CRITICAL FIX: Hard inventory limit bounds - stop bidding/asking if limit reached
    let bid_size = if current_inventory >= config.max_inventory_limit {
        0.0
    } else {
        (config.quote_size_usd * (1.0 - inventory_skew + ofi_size_skew)).max(0.0)
    };

    let ask_size = if current_inventory <= -config.max_inventory_limit {
        0.0
    } else {
        (config.quote_size_usd * (1.0 + inventory_skew - ofi_size_skew)).max(0.0)
    };

    QuoteRecommendation {
        bid_price,
        ask_price,
        bid_size: (bid_size * 10.0).round() / 10.0,
        ask_size: (ask_size * 10.0).round() / 10.0,
    }
}

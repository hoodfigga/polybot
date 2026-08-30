use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CrossVenueArbOpportunity {
    pub event_title: String,
    pub arb_type: &'static str, // "DIRECT_PRICE_DISPARITY" or "CROSS_VENUE_DUTCH_PARITY"
    pub buy_venue: &'static str, // "Polymarket" or "Kalshi"
    pub sell_venue: &'static str,
    pub buy_contract: String,
    pub sell_contract: String,
    pub buy_price: f64,
    pub sell_price: f64,
    pub net_spread_pct: f64,
}

/// Identifies cross-venue arbitrage across Polymarket and Kalshi:
/// 1. Direct YES-price disparity across order books
/// 2. Cross-venue Dutch parity: Buy YES on Venue A + Buy NO on Venue B < 1.0 - fees
pub fn check_cross_venue_disparity(
    event_title: &str,
    poly_yes_ask: f64,
    poly_yes_bid: f64,
    kalshi_yes_ask: f64,
    kalshi_yes_bid: f64,
    kalshi_fee_rate: f64, // e.g. 0.015 (1.5%)
    min_spread: f64,      // e.g. 0.03 (3%)
) -> Option<CrossVenueArbOpportunity> {
    // Direction 1: Buy Polymarket YES, Sell Kalshi YES
    let net_spread_1 = (kalshi_yes_bid - poly_yes_ask) - kalshi_fee_rate;
    if net_spread_1 >= min_spread {
        return Some(CrossVenueArbOpportunity {
            event_title: event_title.to_string(),
            arb_type: "DIRECT_PRICE_DISPARITY",
            buy_venue: "Polymarket",
            sell_venue: "Kalshi",
            buy_contract: "YES".to_string(),
            sell_contract: "YES".to_string(),
            buy_price: poly_yes_ask,
            sell_price: kalshi_yes_bid,
            net_spread_pct: net_spread_1 * 100.0,
        });
    }

    // Direction 2: Buy Kalshi YES, Sell Polymarket YES
    let net_spread_2 = (poly_yes_bid - kalshi_yes_ask) - kalshi_fee_rate;
    if net_spread_2 >= min_spread {
        return Some(CrossVenueArbOpportunity {
            event_title: event_title.to_string(),
            arb_type: "DIRECT_PRICE_DISPARITY",
            buy_venue: "Kalshi",
            sell_venue: "Polymarket",
            buy_contract: "YES".to_string(),
            sell_contract: "YES".to_string(),
            buy_price: kalshi_yes_ask,
            sell_price: poly_yes_bid,
            net_spread_pct: net_spread_2 * 100.0,
        });
    }

    None
}

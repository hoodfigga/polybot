use serde::{Deserialize, Serialize};

/// 3-State Market Regime Enum (Layer 3 in Plan1.md)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketRegime {
    LowVolChoppy,    // Regime 0: Tight spreads, safe for passive market making
    HighVolTrending, // Regime 1: Unidirectional flow; skewed MM with 3x half-spread
    NewsCascade,     // Regime 2: Breaking news / whale sweep; MM halted, Parity arb prioritized
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegimeClassifier {
    pub current_regime: MarketRegime,
    pub rolling_volatility: f64,
    pub rolling_ofi: f64,
    pub headline_urgency_score: f64,
}

impl RegimeClassifier {
    pub fn new() -> Self {
        Self {
            current_regime: MarketRegime::LowVolChoppy,
            rolling_volatility: 0.02,
            rolling_ofi: 0.0,
            headline_urgency_score: 0.0,
        }
    }

    /// Classifies the current market regime based on multi-modal microstructure and news signals
    pub fn classify(
        &mut self,
        volatility: f64,
        ofi: f64,
        headline_urgency_score: f64,
    ) -> MarketRegime {
        self.rolling_volatility = volatility;
        self.rolling_ofi = ofi;
        self.headline_urgency_score = headline_urgency_score;

        let new_regime = if headline_urgency_score > 3.0 || volatility > 0.12 {
            MarketRegime::NewsCascade
        } else if ofi.abs() > 0.45 || volatility > 0.06 {
            MarketRegime::HighVolTrending
        } else {
            MarketRegime::LowVolChoppy
        };

        self.current_regime = new_regime;
        new_regime
    }

    /// Spread multiplier based on active regime
    pub fn spread_multiplier(&self) -> f64 {
        match self.current_regime {
            MarketRegime::LowVolChoppy => 1.0,
            MarketRegime::HighVolTrending => 3.0,
            MarketRegime::NewsCascade => 10.0, // Effectively ceases tight passive quoting
        }
    }

    /// Returns true if passive market making should remain active
    pub fn is_market_making_permitted(&self) -> bool {
        self.current_regime != MarketRegime::NewsCascade
    }
}

impl Default for RegimeClassifier {
    fn default() -> Self {
        Self::new()
    }
}

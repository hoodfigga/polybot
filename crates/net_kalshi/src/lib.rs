use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalshiMarket {
    pub ticker: String,
    pub title: String,
    pub subtitle: String,
    pub yes_bid: Option<f64>,
    pub yes_ask: Option<f64>,
    pub no_bid: Option<f64>,
    pub no_ask: Option<f64>,
    pub volume: u64,
    pub open_interest: u64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KalshiOrder {
    pub action: String, // "buy" or "sell"
    pub count: u64,
    pub side: String, // "yes" or "no"
    pub ticker: String,
    pub r#type: String,         // "limit" or "market"
    pub yes_price: Option<u64>, // Price in cents (1 to 99)
}

pub struct KalshiClient {
    http: Client,
    base_url: String,
    api_key: String,
}

impl KalshiClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::builder()
                .timeout(Duration::from_secs(4))
                .build()
                .unwrap_or_default(),
            base_url: "https://api.elections.kalshi.com/trade-api/v2".to_string(),
            api_key: api_key.into(),
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_string();
        self
    }

    /// Fetches market data for a given ticker
    pub async fn get_market(
        &self,
        ticker: &str,
    ) -> Result<Option<KalshiMarket>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/markets/{}", self.base_url, ticker);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(None);
        }

        let json_val: serde_json::Value = resp.json().await?;
        let m = &json_val["market"];
        if m.is_null() {
            return Ok(None);
        }

        let market = KalshiMarket {
            ticker: m["ticker"].as_str().unwrap_or(ticker).to_string(),
            title: m["title"].as_str().unwrap_or("").to_string(),
            subtitle: m["subtitle"].as_str().unwrap_or("").to_string(),
            yes_bid: m["yes_bid"].as_f64().map(|c| c / 100.0),
            yes_ask: m["yes_ask"].as_f64().map(|c| c / 100.0),
            no_bid: m["no_bid"].as_f64().map(|c| c / 100.0),
            no_ask: m["no_ask"].as_f64().map(|c| c / 100.0),
            volume: m["volume"].as_u64().unwrap_or(0),
            open_interest: m["open_interest"].as_u64().unwrap_or(0),
            status: m["status"].as_str().unwrap_or("active").to_string(),
        };

        Ok(Some(market))
    }

    /// Places a CFTC-regulated dollar-settled order on Kalshi
    pub async fn place_order(
        &self,
        order: &KalshiOrder,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!("{}/portfolio/orders", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(order)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(format!("Kalshi order failed with status {}: {}", status, err_text).into());
        }

        let val = resp.json().await?;
        Ok(val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_kalshi_order_serialization() {
        let order = KalshiOrder {
            action: "buy".to_string(),
            count: 25,
            side: "yes".to_string(),
            ticker: "FED-26DEC-T4.50".to_string(),
            r#type: "limit".to_string(),
            yes_price: Some(54), // 54 cents
        };

        let json_str = serde_json::to_string(&order).expect("Must serialize");
        assert!(json_str.contains("FED-26DEC-T4.50"));
        assert!(json_str.contains("\"yes_price\":54"));
    }
}

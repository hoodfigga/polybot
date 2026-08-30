use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use hmac::{Hmac, Mac};
use reqwest::header::HeaderMap;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::Sha256;
use std::time::Duration;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PolymarketApiCredentials {
    pub key: String,
    pub secret_base64: String,
    pub passphrase: String,
    pub address: String,
}

#[derive(Clone)]
pub struct PolymarketRestClient {
    http: Client,
    base_url: String,
    api_key: String,
    credentials: Option<PolymarketApiCredentials>,
}

impl PolymarketRestClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::builder()
                .tcp_nodelay(true)
                .tcp_keepalive(Some(Duration::from_secs(10)))
                .pool_max_idle_per_host(10)
                .pool_idle_timeout(Some(Duration::from_secs(90)))
                .timeout(Duration::from_secs(4))
                .build()
                .unwrap_or_default(),
            base_url: "https://clob.polymarket.com".to_string(),
            api_key: api_key.into(),
            credentials: None,
        }
    }

    pub fn with_credentials(mut self, creds: PolymarketApiCredentials) -> Self {
        self.credentials = Some(creds);
        self
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_string();
        self
    }

    /// Builds mandatory Level-2 HMAC-SHA256 authentication headers (Plan2.md Module 1)
    pub fn build_l2_headers(
        &self,
        method: &str,
        path: &str,
        body_str: &str,
    ) -> Result<HeaderMap, Box<dyn std::error::Error + Send + Sync>> {
        let mut headers = HeaderMap::new();
        let creds = self
            .credentials
            .as_ref()
            .ok_or("Missing Polymarket L2 credentials (key, secret, passphrase)")?;

        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs()
            .to_string();

        let message = format!("{}{}{}{}", now_unix, method.to_uppercase(), path, body_str);

        let secret_bytes = BASE64.decode(&creds.secret_base64)?;
        let mut mac = Hmac::<Sha256>::new_from_slice(&secret_bytes)?;
        mac.update(message.as_bytes());
        let signature = BASE64.encode(mac.finalize().into_bytes());

        headers.insert("POLY_API_KEY", creds.key.parse()?);
        headers.insert("POLY_SIGNATURE", signature.parse()?);
        headers.insert("POLY_TIMESTAMP", now_unix.parse()?);
        headers.insert("POLY_PASSPHRASE", creds.passphrase.parse()?);
        headers.insert("POLY_ADDRESS", creds.address.parse()?);
        headers.insert("Content-Type", "application/json".parse()?);

        Ok(headers)
    }

    /// Fetches the live on-exchange nonce for an address (Plan2.md Module 4)
    pub async fn fetch_exchange_nonce(&self, address: &str) -> Result<u64, reqwest::Error> {
        let url = format!("{}/nonce?address={}", self.base_url, address);
        let resp: serde_json::Value = self
            .http
            .get(&url)
            .header("POLY_API_KEY", &self.api_key)
            .send()
            .await?
            .json()
            .await?;

        let nonce = resp["nonce"]
            .as_u64()
            .or_else(|| {
                resp["nonce"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(0);

        Ok(nonce)
    }

    /// Submits a signed order to the Polymarket CLOB matching engine with L2 auth or fallback
    pub async fn post_order(
        &self,
        signed_order: &serde_json::Value,
    ) -> Result<serde_json::Value, reqwest::Error> {
        let url = format!("{}/order", self.base_url);
        let body_str = serde_json::to_string(signed_order).unwrap_or_default();

        let req = if self.credentials.is_some() {
            if let Ok(l2_headers) = self.build_l2_headers("POST", "/order", &body_str) {
                self.http.post(&url).headers(l2_headers).json(signed_order)
            } else {
                self.http
                    .post(&url)
                    .header("Content-Type", "application/json")
                    .header("POLY_API_KEY", &self.api_key)
                    .json(signed_order)
            }
        } else {
            self.http
                .post(&url)
                .header("Content-Type", "application/json")
                .header("POLY_API_KEY", &self.api_key)
                .json(signed_order)
        };

        let resp = req.send().await?;
        resp.json().await
    }

    /// Cancels an open order by order ID
    pub async fn cancel_order(&self, order_id: &str) -> Result<serde_json::Value, reqwest::Error> {
        let url = format!("{}/order", self.base_url);
        let payload = json!({ "orderID": order_id });
        let resp = self
            .http
            .delete(&url)
            .header("Content-Type", "application/json")
            .header("POLY_API_KEY", &self.api_key)
            .json(&payload)
            .send()
            .await?;

        resp.json().await
    }

    /// Cancels all open orders across all markets (Emergency Kill-Switch)
    pub async fn cancel_all_orders(&self) -> Result<serde_json::Value, reqwest::Error> {
        let url = format!("{}/cancel-all", self.base_url);
        let resp = self
            .http
            .delete(&url)
            .header("POLY_API_KEY", &self.api_key)
            .send()
            .await?;

        resp.json().await
    }

    /// Fetches collateral USDC.e balance
    pub async fn get_balance(&self, address: &str) -> Result<f64, reqwest::Error> {
        let url = format!("{}/balance?address={}", self.base_url, address);
        let resp: serde_json::Value = self
            .http
            .get(&url)
            .header("POLY_API_KEY", &self.api_key)
            .send()
            .await?
            .json()
            .await?;

        let bal = resp["balance"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| resp["balance"].as_f64())
            .unwrap_or(0.0);

        Ok(bal)
    }

    /// Dynamic `/fee-rate` query hook (Feature 3 in Prompt/Plan.md)
    /// Queries the live taker fee rate in basis points for candidate token
    pub async fn fetch_fee_rate(&self, token_id: &str) -> Result<u64, reqwest::Error> {
        let url = format!("{}/fee-rate?token_id={}", self.base_url, token_id);
        let resp: serde_json::Value = self
            .http
            .get(&url)
            .header("POLY_API_KEY", &self.api_key)
            .send()
            .await?
            .json()
            .await?;

        let fee_bps = resp["fee_rate_bps"]
            .as_u64()
            .or_else(|| {
                resp["fee_rate"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
            })
            .unwrap_or(0);

        Ok(fee_bps)
    }

    /// Evaluates if gross arbitrage margin safely exceeds live dynamic taker fee + hurdle margin
    pub fn is_arbitrage_profitable_after_fees(
        gross_profit_pct: f64,
        fee_rate_bps: u64,
        hurdle_bps: u64,
    ) -> bool {
        let gross_bps = (gross_profit_pct * 100.0) as u64; // e.g. 2.5% = 250 bps
        gross_bps >= fee_rate_bps + hurdle_bps
    }
}

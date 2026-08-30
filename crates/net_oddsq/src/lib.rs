use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FastMoveSignal {
    pub signal_id: String,
    pub venue: String, // "Polymarket", "Kalshi"
    pub market_id: String,
    pub event_id: String,
    pub outcome: String,
    pub urgency: String,
    pub z_score: f64,
    pub new_price: f64,
    pub prev_price: f64,
    pub delta_points: f64,
    pub trade_size_usd: Option<f64>,
    pub headline: String,
    pub siblings: Vec<SiblingContract>,
    pub detected_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SiblingContract {
    pub question: String,
    pub market_id: String,
    pub current_price: f64,
    pub delta_points: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WhaleTradeAlert {
    pub market_id: String,
    pub condition_id: String,
    pub outcome: String,
    pub size_usd: f64,
    pub price: f64,
    pub timestamp_utc: String,
}

pub struct OddsQClient {
    http: Client,
    api_key: String,
    base_url: String,
}

impl OddsQClient {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            http: Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            base_url: "https://oddsq.com/v1".to_string(),
            api_key: api_key.into(),
        }
    }

    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into().trim_end_matches('/').to_string();
        self
    }

    /// Fetches fast-move momentum signals filtered by z-score threshold
    pub async fn fetch_fast_moves(
        &self,
        limit: usize,
        min_z_score: f64,
    ) -> Result<Vec<FastMoveSignal>, Box<dyn std::error::Error + Send + Sync>> {
        let url = format!(
            "{}/signals/fast-moves?limit={}&min_z={}",
            self.base_url, limit, min_z_score
        );
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status();
            tracing::warn!("OddsQ API returned non-success status: {}", status);
            return Err(format!("OddsQ API error: {}", status).into());
        }

        let signals: Vec<FastMoveSignal> = resp.json().await?;
        Ok(signals)
    }

    /// Parses incoming webhook JSON payload
    pub fn parse_webhook_payload(payload_json: &str) -> Result<FastMoveSignal, serde_json::Error> {
        serde_json::from_str::<FastMoveSignal>(payload_json)
    }

    /// Computes length-extension-resistant HMAC-Keccak256 signature
    pub fn compute_webhook_signature(secret: &str, body: &[u8]) -> String {
        let mut key = [0u8; 64];
        let secret_bytes = secret.as_bytes();
        if secret_bytes.len() > 64 {
            let khash = ethers::utils::keccak256(secret_bytes);
            key[..32].copy_from_slice(&khash);
        } else {
            key[..secret_bytes.len()].copy_from_slice(secret_bytes);
        }

        let mut k_ipad = [0x36u8; 64];
        let mut k_opad = [0x5cu8; 64];
        for i in 0..64 {
            k_ipad[i] ^= key[i];
            k_opad[i] ^= key[i];
        }

        let mut inner_buf = Vec::with_capacity(64 + body.len());
        inner_buf.extend_from_slice(&k_ipad);
        inner_buf.extend_from_slice(body);
        let inner_hash = ethers::utils::keccak256(&inner_buf);

        let mut outer_buf = Vec::with_capacity(64 + 32);
        outer_buf.extend_from_slice(&k_opad);
        outer_buf.extend_from_slice(&inner_hash);
        let final_hmac = ethers::utils::keccak256(&outer_buf);

        format!("0x{:x}", ethers::types::H256::from(final_hmac))
    }

    /// Constant-time verification of webhook signature against secret
    pub fn verify_webhook_signature(secret: &str, body: &[u8], signature_hex: &str) -> bool {
        let expected = Self::compute_webhook_signature(secret, body);
        let expected_clean = expected.trim_start_matches("0x");
        let received_clean = signature_hex.trim().trim_start_matches("0x");

        if expected_clean.len() != received_clean.len() {
            return false;
        }

        let mut diff = 0u8;
        for (a, b) in expected_clean
            .as_bytes()
            .iter()
            .zip(received_clean.as_bytes().iter())
        {
            diff |= a.to_ascii_lowercase() ^ b.to_ascii_lowercase();
        }
        diff == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_webhook_hmac_verification() {
        let secret = "oddsq_super_secret_webhook_key";
        let payload = r#"{"signal_id":"sig_1","z_score":3.5}"#;
        let sig = OddsQClient::compute_webhook_signature(secret, payload.as_bytes());

        assert!(OddsQClient::verify_webhook_signature(
            secret,
            payload.as_bytes(),
            &sig
        ));
        assert!(!OddsQClient::verify_webhook_signature(
            "wrong_secret",
            payload.as_bytes(),
            &sig
        ));
        assert!(!OddsQClient::verify_webhook_signature(
            secret,
            b"tampered_payload",
            &sig
        ));
    }

    #[test]
    fn test_webhook_parsing() {
        let raw_json = r#"{
            "signal_id": "sig_9876",
            "venue": "Polymarket",
            "market_id": "0x123abc",
            "event_id": "pres_2028",
            "outcome": "YES",
            "urgency": "HIGH",
            "z_score": 3.45,
            "new_price": 0.62,
            "prev_price": 0.51,
            "delta_points": 11.0,
            "trade_size_usd": 65000.0,
            "headline": "Candidate surges in state primary polling",
            "siblings": [
                { "question": "Will Candidate B win?", "market_id": "0x456def", "current_price": 0.35, "delta_points": -9.0 }
            ],
            "detected_at": "2026-08-29T10:00:00Z"
        }"#;

        let parsed =
            OddsQClient::parse_webhook_payload(raw_json).expect("Must parse valid webhook");
        assert_eq!(parsed.signal_id, "sig_9876");
        assert_eq!(parsed.z_score, 3.45);
        assert_eq!(parsed.trade_size_usd, Some(65000.0));
        assert_eq!(parsed.siblings.len(), 1);
        assert_eq!(parsed.siblings[0].market_id, "0x456def");
    }
}

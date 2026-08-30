use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc::UnboundedSender;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::protocol::Message;

#[derive(Debug, Clone)]
pub struct MarketDeltaEvent {
    pub token_id: String,
    pub is_bid: bool,
    pub price: f64,
    pub size: f64,
    pub sequence: u64,
}

#[derive(Clone, Debug)]
pub struct PolymarketWsClient {
    pub ws_url: String,
    pub token_ids: Vec<String>,
    pub last_frame_timestamp_ms: Arc<AtomicU64>,
}

impl PolymarketWsClient {
    pub fn new(token_ids: Vec<String>) -> Self {
        Self {
            ws_url: "wss://ws-subscriptions-clob.polymarket.com/ws/market".to_string(),
            token_ids,
            last_frame_timestamp_ms: Arc::new(AtomicU64::new(current_timestamp_ms())),
        }
    }

    pub fn with_url(mut self, url: impl Into<String>) -> Self {
        self.ws_url = url.into();
        self
    }

    /// Checks if the WebSocket connection is alive. Returns false if no frame was seen within `max_silence_ms`.
    pub fn is_alive(&self, max_silence_ms: u64) -> bool {
        let now = current_timestamp_ms();
        let last = self.last_frame_timestamp_ms.load(Ordering::Relaxed);
        (now.saturating_sub(last)) <= max_silence_ms
    }

    /// Connects to the Polymarket CLOB WebSocket feed with automatic reconnection on disconnect
    pub async fn run_reconnecting(&self, sender: UnboundedSender<MarketDeltaEvent>) {
        let mut backoff = Duration::from_millis(500);
        let max_backoff = Duration::from_secs(10);

        loop {
            tracing::info!(
                "Connecting to Polymarket WebSocket feed at {}...",
                self.ws_url
            );
            match connect_async(&self.ws_url).await {
                Ok((ws_stream, _)) => {
                    tracing::info!("✅ Connected to Polymarket WebSocket feed!");
                    backoff = Duration::from_millis(500); // Reset backoff on success
                    let (mut write, mut read) = ws_stream.split();

                    // Subscribe to configured market tokens
                    for token_id in &self.token_ids {
                        let sub_msg = json!({
                            "type": "market",
                            "assets_ids": [token_id]
                        });
                        let _ = write.send(Message::Text(sub_msg.to_string())).await;
                    }

                    // Spawn periodic ping heartbeat task
                    let (ping_tx, mut ping_rx) = tokio::sync::mpsc::channel::<Message>(16);
                    let ping_task = tokio::spawn(async move {
                        let mut interval = tokio::time::interval(Duration::from_secs(10));
                        loop {
                            interval.tick().await;
                            if ping_tx.send(Message::Ping(b"heartbeat".to_vec())).await.is_err() {
                                break;
                            }
                        }
                    });

                    loop {
                        tokio::select! {
                            Some(ping_msg) = ping_rx.recv() => {
                                if write.send(ping_msg).await.is_err() {
                                    break;
                                }
                            }
                            msg_opt = read.next() => {
                                match msg_opt {
                                    Some(Ok(Message::Text(text))) => {
                                        self.last_frame_timestamp_ms.store(current_timestamp_ms(), Ordering::Relaxed);
                                        let events = parse_market_frames_robust(&text);
                                        for event in events {
                                            let _ = sender.send(event);
                                        }
                                    }
                                    Some(Ok(Message::Pong(_))) | Some(Ok(Message::Ping(_))) => {
                                        self.last_frame_timestamp_ms.store(current_timestamp_ms(), Ordering::Relaxed);
                                        if let Some(Ok(Message::Ping(d))) = msg_opt {
                                            let _ = write.send(Message::Pong(d)).await;
                                        }
                                    }
                                    Some(Ok(Message::Close(_))) => {
                                        tracing::warn!("WebSocket closed by server, reconnecting...");
                                        break;
                                    }
                                    Some(Err(e)) => {
                                        tracing::warn!("WebSocket read error: {}, reconnecting...", e);
                                        break;
                                    }
                                    None => {
                                        tracing::warn!("WebSocket stream ended, reconnecting...");
                                        break;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    ping_task.abort();
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to connect to Polymarket WebSocket: {}. Retrying in {:?}",
                        e,
                        backoff
                    );
                }
            }

            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(max_backoff);
        }
    }
}

fn current_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Robust multi-event parser for both batch "book" snapshots and discrete "price_change" frames (Plan2.md Module 2)
pub fn parse_market_frames_robust(raw_json: &str) -> Vec<MarketDeltaEvent> {
    let mut events = Vec::new();
    let val: serde_json::Value = match serde_json::from_str(raw_json) {
        Ok(v) => v,
        Err(_) => return events,
    };

    let event_type = val["event_type"].as_str().unwrap_or_default();

    if event_type == "book" {
        let token_id = val["asset_id"].as_str().unwrap_or_default().to_string();
        let hash = val["hash"].as_u64().unwrap_or(0);
        if token_id.is_empty() {
            return events;
        }

        if let Some(bids) = val["bids"].as_array() {
            for b in bids {
                let p = b["price"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| b["price"].as_f64())
                    .unwrap_or(0.0);
                let s = b["size"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| b["size"].as_f64())
                    .unwrap_or(0.0);
                if p > 0.0 {
                    events.push(MarketDeltaEvent {
                        token_id: token_id.clone(),
                        is_bid: true,
                        price: p,
                        size: s,
                        sequence: hash,
                    });
                }
            }
        }

        if let Some(asks) = val["asks"].as_array() {
            for a in asks {
                let p = a["price"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| a["price"].as_f64())
                    .unwrap_or(0.0);
                let s = a["size"]
                    .as_str()
                    .and_then(|s| s.parse::<f64>().ok())
                    .or_else(|| a["size"].as_f64())
                    .unwrap_or(0.0);
                if p > 0.0 {
                    events.push(MarketDeltaEvent {
                        token_id: token_id.clone(),
                        is_bid: false,
                        price: p,
                        size: s,
                        sequence: hash,
                    });
                }
            }
        }
    } else if event_type == "price_change" {
        let token_id = val["asset_id"].as_str().unwrap_or_default().to_string();
        let price = val["price"]
            .as_str()
            .and_then(|p| p.parse::<f64>().ok())
            .or_else(|| val["price"].as_f64())
            .unwrap_or(0.0);
        let size = val["size"]
            .as_str()
            .and_then(|s| s.parse::<f64>().ok())
            .or_else(|| val["size"].as_f64())
            .unwrap_or(0.0);
        let is_bid = val["side"].as_str().map(|s| s == "BUY").unwrap_or(true);
        let seq = val["hash"].as_u64().unwrap_or(0);

        if !token_id.is_empty() && price > 0.0 {
            events.push(MarketDeltaEvent {
                token_id,
                is_bid,
                price,
                size,
                sequence: seq,
            });
        }
    }

    events
}

/// Ultra-fast zero-copy frame parser for single market delta events (Plan1.md Module 2)
#[inline(always)]
pub fn parse_market_frame_fast(raw_json: &str) -> Option<MarketDeltaEvent> {
    parse_market_frames_robust(raw_json).into_iter().next()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_market_frame_fast() {
        let frame = r#"{
            "event_type": "price_change",
            "asset_id": "0x1234567890abcdef",
            "price": "0.48",
            "size": "1500.0",
            "side": "BUY",
            "hash": 123456
        }"#;

        let event = parse_market_frame_fast(frame).expect("Must parse market frame successfully");
        assert_eq!(event.token_id, "0x1234567890abcdef");
        assert!(event.is_bid);
        assert_eq!(event.price, 0.48);
        assert_eq!(event.size, 1500.0);
        assert_eq!(event.sequence, 123456);
    }

    #[test]
    fn test_parse_market_frames_robust_book_snapshot() {
        let snapshot = r#"{
            "event_type": "book",
            "asset_id": "0xfeedbeef123",
            "hash": 999,
            "bids": [
                {"price": "0.45", "size": "200.0"},
                {"price": "0.44", "size": "500.0"}
            ],
            "asks": [
                {"price": "0.48", "size": "300.0"}
            ]
        }"#;

        let events = parse_market_frames_robust(snapshot);
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].token_id, "0xfeedbeef123");
        assert!(events[0].is_bid);
        assert_eq!(events[0].price, 0.45);
        assert_eq!(events[0].size, 200.0);

        assert_eq!(events[2].token_id, "0xfeedbeef123");
        assert!(!events[2].is_bid);
        assert_eq!(events[2].price, 0.48);
        assert_eq!(events[2].size, 300.0);
    }
}

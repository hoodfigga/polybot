use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_VAULT_RECORDS: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketResolutionRecord {
    pub event_id: String,
    pub question: String,
    pub winning_outcome: String,
    pub final_yes_price: f64,
    pub catalyst_summary: String,
    pub vector_embedding: Vec<f32>, // 32-D or 64-D semantic representation
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradePostMortemRecord {
    pub trade_id: String,
    pub market_id: String,
    pub headline: String,
    pub latency_ms: u64,
    pub loss_usd: f64,
    pub vector_embedding: Vec<f32>,
    pub recorded_at_unix: u64,
}

pub struct MemoryVault {
    records: VecDeque<MarketResolutionRecord>,
    post_mortems: VecDeque<TradePostMortemRecord>,
}

impl MemoryVault {
    pub fn new() -> Self {
        Self {
            records: VecDeque::new(),
            post_mortems: VecDeque::new(),
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    #[inline]
    pub fn post_mortem_count(&self) -> usize {
        self.post_mortems.len()
    }

    pub fn insert(&mut self, record: MarketResolutionRecord) {
        if self.records.len() >= MAX_VAULT_RECORDS {
            self.records.pop_front(); // O(1) eviction of oldest record
        }
        self.records.push_back(record);
    }

    pub fn record_trade_failure(
        &mut self,
        trade_id: impl Into<String>,
        market_id: impl Into<String>,
        headline: impl Into<String>,
        latency_ms: u64,
        loss_usd: f64,
        vector_embedding: Vec<f32>,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if self.post_mortems.len() >= MAX_VAULT_RECORDS {
            self.post_mortems.pop_front(); // O(1) eviction
        }

        self.post_mortems.push_back(TradePostMortemRecord {
            trade_id: trade_id.into(),
            market_id: market_id.into(),
            headline: headline.into(),
            latency_ms,
            loss_usd,
            vector_embedding,
            recorded_at_unix: now,
        });
    }

    /// Checks if a current opportunity has high cosine similarity to past trading loss clusters
    pub fn check_historical_failure_risk(
        &self,
        query_embedding: &[f32],
        similarity_threshold: f32,
    ) -> Option<&TradePostMortemRecord> {
        self.post_mortems.iter().find(|pm| {
            cosine_similarity(query_embedding, &pm.vector_embedding) >= similarity_threshold
        })
    }

    /// Finds Top-K most similar historical market resolutions by cosine similarity
    pub fn search_similar(
        &self,
        query_embedding: &[f32],
        top_k: usize,
    ) -> Vec<(&MarketResolutionRecord, f32)> {
        if query_embedding.is_empty() || self.records.is_empty() || top_k == 0 {
            return Vec::new();
        }

        let query_norm = query_embedding
            .iter()
            .filter(|x| x.is_finite())
            .map(|x| x * x)
            .sum::<f32>()
            .sqrt();

        if query_norm <= 1e-6 {
            return Vec::new();
        }

        let mut scored: Vec<(&MarketResolutionRecord, f32)> = self
            .records
            .iter()
            .map(|r| {
                let score = cosine_similarity_with_norm(query_embedding, query_norm, &r.vector_embedding);
                (r, score)
            })
            .filter(|(_, score)| score.is_finite())
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }
}

impl Default for MemoryVault {
    fn default() -> Self {
        Self::new()
    }
}

/// Normalized cosine similarity between two vector embeddings
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;

    for (x, y) in a.iter().zip(b.iter()) {
        if x.is_finite() && y.is_finite() {
            dot += x * y;
            norm_a += x * x;
            norm_b += y * y;
        }
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom <= 1e-6 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

/// Fast cosine similarity utilizing precomputed query norm
#[inline(always)]
pub fn cosine_similarity_with_norm(a: &[f32], norm_a: f32, b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() || norm_a <= 1e-6 {
        return 0.0;
    }

    let mut dot = 0.0f32;
    let mut norm_b = 0.0f32;

    for (&x, &y) in a.iter().zip(b.iter()) {
        if x.is_finite() && y.is_finite() {
            dot += x * y;
            norm_b += y * y;
        }
    }

    let denom = norm_a * norm_b.sqrt();
    if denom <= 1e-6 {
        0.0
    } else {
        (dot / denom).clamp(-1.0, 1.0)
    }
}

/// Trade Audit Parameters for recording and persisting trade executions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeAuditParams {
    pub market_id: String,
    pub token_id: String,
    pub side: String,
    pub requested_price: f64,
    pub filled_price: f64,
    pub size_usd: f64,
    pub fee_usd: f64,
    pub realized_pnl_usd: f64,
    pub polygon_tx_hash: Option<String>,
}

impl TradeAuditParams {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        market_id: impl Into<String>,
        token_id: impl Into<String>,
        side: impl Into<String>,
        requested_price: f64,
        filled_price: f64,
        size_usd: f64,
        fee_usd: f64,
        realized_pnl_usd: f64,
        polygon_tx_hash: Option<String>,
    ) -> Self {
        Self {
            market_id: market_id.into(),
            token_id: token_id.into(),
            side: side.into(),
            requested_price,
            filled_price,
            size_usd,
            fee_usd,
            realized_pnl_usd,
            polygon_tx_hash,
        }
    }
}

/// Trade Audit Ledger Record (Feature 4 in Prompt/Plan.md)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TradeAuditLedgerRecord {
    pub id: u64,
    pub timestamp_unix: u64,
    pub market_id: String,
    pub token_id: String,
    pub side: String,
    pub requested_price: f64,
    pub filled_price: f64,
    pub size_usd: f64,
    pub fee_usd: f64,
    pub realized_pnl_usd: f64,
    pub polygon_tx_hash: Option<String>,
}

/// In-Memory / WAL Trade Audit Ledger
pub struct TradeAuditLedger {
    records: Vec<TradeAuditLedgerRecord>,
    next_id: u64,
}

impl TradeAuditLedger {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            next_id: 1,
        }
    }

    pub fn record_trade(&mut self, params: TradeAuditParams) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let id = self.next_id;
        self.next_id += 1;

        self.records.push(TradeAuditLedgerRecord {
            id,
            timestamp_unix: now,
            market_id: params.market_id,
            token_id: params.token_id,
            side: params.side,
            requested_price: params.requested_price,
            filled_price: params.filled_price,
            size_usd: params.size_usd,
            fee_usd: params.fee_usd,
            realized_pnl_usd: params.realized_pnl_usd,
            polygon_tx_hash: params.polygon_tx_hash,
        });

        id
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.records.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn records(&self) -> &[TradeAuditLedgerRecord] {
        &self.records
    }
}

impl Default for TradeAuditLedger {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistent SQLite WAL Trade Audit Logger (Plan.md Feature 4)
pub struct SqliteTradeLedger {
    conn: rusqlite::Connection,
}

impl SqliteTradeLedger {
    /// Opens or creates an SQLite trade ledger database with WAL journal mode
    pub fn open(path: impl AsRef<std::path::Path>) -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open(path)?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    /// Opens an in-memory SQLite trade ledger (useful for testing and zero-disk execution)
    pub fn in_memory() -> Result<Self, rusqlite::Error> {
        let conn = rusqlite::Connection::open_in_memory()?;
        Self::init_schema(&conn)?;
        Ok(Self { conn })
    }

    fn init_schema(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
        let _ = conn.pragma_update(None, "journal_mode", "WAL");
        let _ = conn.pragma_update(None, "synchronous", "NORMAL");

        conn.execute(
            "CREATE TABLE IF NOT EXISTS trade_ledger (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp_unix INTEGER NOT NULL,
                market_id TEXT NOT NULL,
                token_id TEXT NOT NULL,
                side TEXT NOT NULL,
                requested_price REAL NOT NULL,
                filled_price REAL NOT NULL,
                size_usd REAL NOT NULL,
                fee_usd REAL NOT NULL,
                realized_pnl_usd REAL NOT NULL,
                polygon_tx_hash TEXT
            );",
            [],
        )?;

        Ok(())
    }

    pub fn insert_trade(&self, params: &TradeAuditParams) -> Result<i64, rusqlite::Error> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        self.conn.execute(
            "INSERT INTO trade_ledger (
                timestamp_unix, market_id, token_id, side, requested_price, filled_price, size_usd, fee_usd, realized_pnl_usd, polygon_tx_hash
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10);",
            rusqlite::params![
                now as i64,
                params.market_id,
                params.token_id,
                params.side,
                params.requested_price,
                params.filled_price,
                params.size_usd,
                params.fee_usd,
                params.realized_pnl_usd,
                params.polygon_tx_hash,
            ],
        )?;

        Ok(self.conn.last_insert_rowid())
    }

    pub fn query_total_realized_pnl(&self) -> Result<f64, rusqlite::Error> {
        let mut stmt = self
            .conn
            .prepare("SELECT COALESCE(SUM(realized_pnl_usd), 0.0) FROM trade_ledger;")?;
        let sum: f64 = stmt.query_row([], |row| row.get(0))?;
        Ok(sum)
    }

    pub fn count_records(&self) -> Result<usize, rusqlite::Error> {
        let mut stmt = self.conn.prepare("SELECT COUNT(*) FROM trade_ledger;")?;
        let count: usize = stmt.query_row([], |row| row.get(0))?;
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_vault_similarity_search() {
        let mut vault = MemoryVault::new();
        vault.insert(MarketResolutionRecord {
            event_id: "e1".to_string(),
            question: "Fed cut 25bps".to_string(),
            winning_outcome: "YES".to_string(),
            final_yes_price: 1.0,
            catalyst_summary: "Inflation CPI cooler than expected".to_string(),
            vector_embedding: vec![1.0, 0.0, 0.0],
        });

        vault.insert(MarketResolutionRecord {
            event_id: "e2".to_string(),
            question: "Election Candidate Win".to_string(),
            winning_outcome: "NO".to_string(),
            final_yes_price: 0.0,
            catalyst_summary: "Exit polls showed drop in support".to_string(),
            vector_embedding: vec![0.0, 1.0, 0.0],
        });

        let query = vec![0.9, 0.1, 0.0];
        let results = vault.search_similar(&query, 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.event_id, "e1");
        assert!(results[0].1 > 0.85);
        assert_eq!(vault.len(), 2);
    }

    #[test]
    fn test_post_mortem_failure_risk() {
        let mut vault = MemoryVault::new();
        vault.record_trade_failure(
            "tr_99",
            "cond_fed_sept",
            "Fed holds rates unexpectedly",
            45,
            12.50,
            vec![0.8, 0.2, 0.0],
        );

        let query_risky = vec![0.85, 0.15, 0.0];
        let query_safe = vec![0.0, 0.0, 1.0];

        assert!(vault
            .check_historical_failure_risk(&query_risky, 0.90)
            .is_some());
        assert!(vault
            .check_historical_failure_risk(&query_safe, 0.90)
            .is_none());
        assert_eq!(vault.post_mortem_count(), 1);
    }

    #[test]
    fn test_trade_audit_ledger() {
        let mut ledger = TradeAuditLedger::new();
        let params = TradeAuditParams::new(
            "cond_pres_2028",
            "token_pres_yes",
            "BUY",
            0.48,
            0.485,
            50.0,
            0.15,
            1.25,
            Some("0xabcdef123456".to_string()),
        );
        let id1 = ledger.record_trade(params);

        assert_eq!(id1, 1);
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger.records()[0].realized_pnl_usd, 1.25);
    }

    #[test]
    fn test_sqlite_wal_trade_audit_ledger() {
        let db = SqliteTradeLedger::in_memory().expect("In-memory SQLite DB initialization must succeed");

        let p1 = TradeAuditParams::new(
            "cond_pres_2028",
            "token_pres_yes",
            "BUY",
            0.48,
            0.485,
            50.0,
            0.15,
            1.25,
            Some("0x1234567890abcdef".to_string()),
        );
        let rowid1 = db.insert_trade(&p1).unwrap();

        let p2 = TradeAuditParams::new(
            "cond_fed_sept",
            "token_fed_yes",
            "BUY",
            0.46,
            0.465,
            40.0,
            0.12,
            0.85,
            Some("0xabcdef1234567890".to_string()),
        );
        let rowid2 = db.insert_trade(&p2).unwrap();

        assert_eq!(rowid1, 1);
        assert_eq!(rowid2, 2);
        assert_eq!(db.count_records().unwrap(), 2);

        let total_pnl = db.query_total_realized_pnl().unwrap();
        assert!((total_pnl - 2.10).abs() < 1e-6);
    }
}

//! SQLite-backed persistent storage for settlements
//!
//! This module provides durable storage for pending settlements to ensure
//! they survive server restarts and don't result in lost funds.

use crate::models::{PaymentPayload, VerifyPaymentRequirements};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Status of a settlement in the database
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettlementStatus {
    /// Queued and waiting to be processed
    Pending,
    /// Currently being processed by a worker
    InProgress,
    /// Successfully settled on chain
    Completed,
    /// Failed after all retries exhausted
    Failed,
}

impl SettlementStatus {
    fn as_str(&self) -> &'static str {
        match self {
            SettlementStatus::Pending => "pending",
            SettlementStatus::InProgress => "in_progress",
            SettlementStatus::Completed => "completed",
            SettlementStatus::Failed => "failed",
        }
    }

    fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(SettlementStatus::Pending),
            "in_progress" => Some(SettlementStatus::InProgress),
            "completed" => Some(SettlementStatus::Completed),
            "failed" => Some(SettlementStatus::Failed),
            _ => None,
        }
    }
}

/// A settlement record stored in the database
#[derive(Debug, Clone)]
pub struct StoredSettlement {
    /// Unique database ID
    pub id: i64,
    /// The nonce (hex string) - used as unique identifier
    pub nonce: String,
    /// Serialized payment payload (JSON)
    pub payment_payload_json: String,
    /// Serialized payment requirements (JSON)
    pub payment_requirements_json: String,
    /// When the settlement was queued
    pub queued_at: DateTime<Utc>,
    /// Current status
    pub status: SettlementStatus,
    /// Number of retry attempts so far
    pub retry_count: i32,
    /// Last error message if any
    pub last_error: Option<String>,
    /// Transaction hash if settled successfully
    pub tx_hash: Option<String>,
    /// When the settlement was last updated
    pub updated_at: DateTime<Utc>,
}

impl StoredSettlement {
    /// Deserialize the payment payload
    pub fn payment_payload(&self) -> Result<PaymentPayload, serde_json::Error> {
        serde_json::from_str(&self.payment_payload_json)
    }

    /// Deserialize the payment requirements
    pub fn payment_requirements(&self) -> Result<VerifyPaymentRequirements, serde_json::Error> {
        serde_json::from_str(&self.payment_requirements_json)
    }
}

/// SQLite-backed settlement store
pub struct SettlementStore {
    conn: Mutex<Connection>,
}

impl SettlementStore {
    /// Open or create a settlement store at the given path
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, rusqlite::Error> {
        let conn = Connection::open(path)?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;

        // Create the settlements table if it doesn't exist
        conn.execute(
            "CREATE TABLE IF NOT EXISTS settlements (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                nonce TEXT UNIQUE NOT NULL,
                payment_payload_json TEXT NOT NULL,
                payment_requirements_json TEXT NOT NULL,
                queued_at TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                retry_count INTEGER NOT NULL DEFAULT 0,
                last_error TEXT,
                tx_hash TEXT,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        // Create index on status for efficient queue queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_settlements_status ON settlements(status)",
            [],
        )?;

        // Create index on nonce for fast lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_settlements_nonce ON settlements(nonce)",
            [],
        )?;

        info!("Settlement store initialized");
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Open an in-memory store (for testing)
    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self, rusqlite::Error> {
        Self::open(":memory:")
    }

    /// Insert a new pending settlement
    /// Returns the database ID, or None if the nonce already exists
    pub fn insert(
        &self,
        nonce: &str,
        payment_payload: &PaymentPayload,
        payment_requirements: &VerifyPaymentRequirements,
    ) -> Result<Option<i64>, SettlementStoreError> {
        let payload_json = serde_json::to_string(payment_payload)?;
        let requirements_json = serde_json::to_string(payment_requirements)?;
        let now = Utc::now().to_rfc3339();

        let conn = self.conn.lock().unwrap();

        // Use INSERT OR IGNORE to handle duplicate nonces gracefully
        let rows_affected = conn.execute(
            "INSERT OR IGNORE INTO settlements
             (nonce, payment_payload_json, payment_requirements_json, queued_at, status, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', ?4)",
            params![nonce, payload_json, requirements_json, now],
        )?;

        if rows_affected == 0 {
            // Nonce already exists
            debug!("Settlement for nonce {} already exists in store", nonce);
            return Ok(None);
        }

        let id = conn.last_insert_rowid();
        debug!("Inserted settlement {} for nonce {}", id, nonce);
        Ok(Some(id))
    }

    /// Get the next pending settlement (FIFO order by queued_at)
    /// Also marks it as in_progress
    pub fn claim_next(&self) -> Result<Option<StoredSettlement>, SettlementStoreError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        // Atomically select and update the oldest pending settlement
        let result: Option<i64> = conn
            .query_row(
                "UPDATE settlements
                 SET status = 'in_progress', updated_at = ?1
                 WHERE id = (
                     SELECT id FROM settlements
                     WHERE status = 'pending'
                     ORDER BY queued_at ASC
                     LIMIT 1
                 )
                 RETURNING id",
                params![now],
                |row| row.get(0),
            )
            .optional()?;

        match result {
            Some(id) => self.get_by_id_internal(&conn, id),
            None => Ok(None),
        }
    }

    /// Get a settlement by its database ID
    fn get_by_id_internal(
        &self,
        conn: &Connection,
        id: i64,
    ) -> Result<Option<StoredSettlement>, SettlementStoreError> {
        let result = conn
            .query_row(
                "SELECT id, nonce, payment_payload_json, payment_requirements_json,
                        queued_at, status, retry_count, last_error, tx_hash, updated_at
                 FROM settlements WHERE id = ?1",
                params![id],
                |row| {
                    Ok(StoredSettlement {
                        id: row.get(0)?,
                        nonce: row.get(1)?,
                        payment_payload_json: row.get(2)?,
                        payment_requirements_json: row.get(3)?,
                        queued_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                        status: SettlementStatus::from_str(&row.get::<_, String>(5)?)
                            .unwrap_or(SettlementStatus::Pending),
                        retry_count: row.get(6)?,
                        last_error: row.get(7)?,
                        tx_hash: row.get(8)?,
                        updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                            .map(|dt| dt.with_timezone(&Utc))
                            .unwrap_or_else(|_| Utc::now()),
                    })
                },
            )
            .optional()?;

        Ok(result)
    }

    /// Mark a settlement as completed with the transaction hash
    pub fn mark_completed(&self, id: i64, tx_hash: &str) -> Result<(), SettlementStoreError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE settlements SET status = 'completed', tx_hash = ?1, updated_at = ?2 WHERE id = ?3",
            params![tx_hash, now, id],
        )?;

        debug!("Marked settlement {} as completed, tx: {}", id, tx_hash);
        Ok(())
    }

    /// Mark a settlement as failed with the error message
    pub fn mark_failed(&self, id: i64, error: &str) -> Result<(), SettlementStoreError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE settlements SET status = 'failed', last_error = ?1, updated_at = ?2 WHERE id = ?3",
            params![error, now, id],
        )?;

        warn!("Marked settlement {} as failed: {}", id, error);
        Ok(())
    }

    /// Increment retry count and update last error, keeping status as pending for retry
    pub fn record_retry(&self, id: i64, error: &str) -> Result<(), SettlementStoreError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "UPDATE settlements SET status = 'pending', retry_count = retry_count + 1,
             last_error = ?1, updated_at = ?2 WHERE id = ?3",
            params![error, now, id],
        )?;

        debug!("Recorded retry for settlement {}: {}", id, error);
        Ok(())
    }

    /// Get count of settlements by status
    pub fn count_by_status(&self, status: SettlementStatus) -> Result<i64, SettlementStoreError> {
        let conn = self.conn.lock().unwrap();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM settlements WHERE status = ?1",
            params![status.as_str()],
            |row| row.get(0),
        )?;
        Ok(count)
    }

    /// Get count of pending settlements
    pub fn pending_count(&self) -> Result<i64, SettlementStoreError> {
        self.count_by_status(SettlementStatus::Pending)
    }

    /// Get count of in-progress settlements
    pub fn in_progress_count(&self) -> Result<i64, SettlementStoreError> {
        self.count_by_status(SettlementStatus::InProgress)
    }

    /// Reset any in_progress settlements back to pending (for recovery after crash)
    pub fn recover_in_progress(&self) -> Result<i64, SettlementStoreError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE settlements SET status = 'pending', updated_at = ?1 WHERE status = 'in_progress'",
            params![now],
        )?;

        if count > 0 {
            info!(
                "Recovered {} in-progress settlements back to pending",
                count
            );
        }

        Ok(count as i64)
    }

    /// Get all failed settlements (for manual review/retry)
    pub fn get_failed(&self, limit: i64) -> Result<Vec<StoredSettlement>, SettlementStoreError> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, nonce, payment_payload_json, payment_requirements_json,
                    queued_at, status, retry_count, last_error, tx_hash, updated_at
             FROM settlements WHERE status = 'failed'
             ORDER BY updated_at DESC
             LIMIT ?1",
        )?;

        let settlements = stmt
            .query_map(params![limit], |row| {
                Ok(StoredSettlement {
                    id: row.get(0)?,
                    nonce: row.get(1)?,
                    payment_payload_json: row.get(2)?,
                    payment_requirements_json: row.get(3)?,
                    queued_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                    status: SettlementStatus::Failed,
                    retry_count: row.get(6)?,
                    last_error: row.get(7)?,
                    tx_hash: row.get(8)?,
                    updated_at: DateTime::parse_from_rfc3339(&row.get::<_, String>(9)?)
                        .map(|dt| dt.with_timezone(&Utc))
                        .unwrap_or_else(|_| Utc::now()),
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(settlements)
    }

    /// Retry a failed settlement by resetting it to pending
    pub fn retry_failed(&self, id: i64) -> Result<bool, SettlementStoreError> {
        let conn = self.conn.lock().unwrap();
        let now = Utc::now().to_rfc3339();

        let count = conn.execute(
            "UPDATE settlements SET status = 'pending', updated_at = ?1 WHERE id = ?2 AND status = 'failed'",
            params![now, id],
        )?;

        Ok(count > 0)
    }
}

/// Errors that can occur in the settlement store
#[derive(Debug, thiserror::Error)]
pub enum SettlementStoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domains::{DomainBytes32, DomainEthAddress, DomainUint256};
    use crate::models::{AcceptedRequirements, Eip3009Authorization, ExactEvmPayload};

    fn mock_payment_payload() -> PaymentPayload {
        PaymentPayload {
            x402_version: 2,
            accepted: AcceptedRequirements {
                scheme: "exact".to_string(),
                network: "eip155:8453".to_string(),
                amount: "1000".to_string(),
                pay_to: DomainEthAddress::from_hex(
                    "0x0000000000000000000000000000000000000001",
                )
                .unwrap(),
                max_timeout_seconds: 60,
                asset: DomainEthAddress::from_hex(
                    "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
                )
                .unwrap(),
                extra: None,
            },
            payload: ExactEvmPayload {
                signature: "0xtest".to_string(),
                authorization: Eip3009Authorization {
                    from: DomainEthAddress::from_hex(
                        "0x0000000000000000000000000000000000000002",
                    )
                    .unwrap(),
                    to: DomainEthAddress::from_hex(
                        "0x0000000000000000000000000000000000000001",
                    )
                    .unwrap(),
                    value: DomainUint256::from_str("1000").unwrap(),
                    valid_after: DomainUint256::from_str("0").unwrap(),
                    valid_before: DomainUint256::from_str("999999999999").unwrap(),
                    nonce: DomainBytes32::from_hex(
                        "0x0000000000000000000000000000000000000000000000000000000000000001",
                    )
                    .unwrap(),
                },
            },
        }
    }

    fn mock_requirements() -> VerifyPaymentRequirements {
        VerifyPaymentRequirements {
            scheme: "exact".to_string(),
            network: "eip155:8453".to_string(),
            amount: "1000".to_string(),
            pay_to: DomainEthAddress::from_hex("0x0000000000000000000000000000000000000001")
                .unwrap(),
            max_timeout_seconds: 60,
            asset: DomainEthAddress::from_hex("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913")
                .unwrap(),
            extra: None,
        }
    }

    #[test]
    fn test_insert_and_claim() {
        let store = SettlementStore::open_in_memory().unwrap();

        let payload = mock_payment_payload();
        let requirements = mock_requirements();

        // Insert a settlement
        let id = store
            .insert("nonce1", &payload, &requirements)
            .unwrap()
            .unwrap();
        assert!(id > 0);

        // Claim it
        let settlement = store.claim_next().unwrap().unwrap();
        assert_eq!(settlement.nonce, "nonce1");
        assert_eq!(settlement.status, SettlementStatus::InProgress);

        // No more pending
        assert!(store.claim_next().unwrap().is_none());
    }

    #[test]
    fn test_duplicate_nonce() {
        let store = SettlementStore::open_in_memory().unwrap();

        let payload = mock_payment_payload();
        let requirements = mock_requirements();

        // First insert succeeds
        let id1 = store.insert("nonce1", &payload, &requirements).unwrap();
        assert!(id1.is_some());

        // Second insert with same nonce returns None
        let id2 = store.insert("nonce1", &payload, &requirements).unwrap();
        assert!(id2.is_none());
    }

    #[test]
    fn test_mark_completed() {
        let store = SettlementStore::open_in_memory().unwrap();

        let payload = mock_payment_payload();
        let requirements = mock_requirements();

        store.insert("nonce1", &payload, &requirements).unwrap();
        let settlement = store.claim_next().unwrap().unwrap();

        store
            .mark_completed(settlement.id, "0xabc123")
            .unwrap();

        assert_eq!(store.pending_count().unwrap(), 0);
        assert_eq!(
            store.count_by_status(SettlementStatus::Completed).unwrap(),
            1
        );
    }

    #[test]
    fn test_fifo_order() {
        let store = SettlementStore::open_in_memory().unwrap();

        let payload = mock_payment_payload();
        let requirements = mock_requirements();

        store.insert("first", &payload, &requirements).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        store.insert("second", &payload, &requirements).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        store.insert("third", &payload, &requirements).unwrap();

        assert_eq!(store.claim_next().unwrap().unwrap().nonce, "first");
        assert_eq!(store.claim_next().unwrap().unwrap().nonce, "second");
        assert_eq!(store.claim_next().unwrap().unwrap().nonce, "third");
    }

    #[test]
    fn test_recover_in_progress() {
        let store = SettlementStore::open_in_memory().unwrap();

        let payload = mock_payment_payload();
        let requirements = mock_requirements();

        store.insert("nonce1", &payload, &requirements).unwrap();
        store.claim_next().unwrap(); // Mark as in_progress

        assert_eq!(store.in_progress_count().unwrap(), 1);
        assert_eq!(store.pending_count().unwrap(), 0);

        // Simulate crash recovery
        store.recover_in_progress().unwrap();

        assert_eq!(store.in_progress_count().unwrap(), 0);
        assert_eq!(store.pending_count().unwrap(), 1);
    }
}

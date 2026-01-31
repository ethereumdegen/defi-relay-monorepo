//! Settlement queue for decoupling x402 settlement from HTTP request flow
//!
//! This module provides a FIFO queue for pending settlements that allows
//! the HTTP request to return immediately after payment verification,
//! while settlement is processed asynchronously by a background worker.
//!
//! The queue is backed by SQLite for persistence, ensuring settlements
//! survive server restarts.

use crate::models::{PaymentPayload, VerifyPaymentRequirements};
use crate::services::settlement_store::{SettlementStatus, SettlementStore};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Notify;
use tracing::{debug, info, warn};

/// Default maximum queue size (can be overridden via env var)
pub const DEFAULT_MAX_QUEUE_SIZE: usize = 10_000;

/// A pending settlement waiting to be processed
#[derive(Debug, Clone)]
pub struct PendingSettlement {
    /// The payment payload from the client
    pub payment_payload: PaymentPayload,
    /// The payment requirements for verification/settlement
    pub payment_requirements: VerifyPaymentRequirements,
    /// When this settlement was queued
    pub queued_at: Instant,
    /// Nonce for tracking/deduplication (hex string)
    pub nonce: String,
}

impl PendingSettlement {
    /// Create a new pending settlement
    pub fn new(
        payment_payload: PaymentPayload,
        payment_requirements: VerifyPaymentRequirements,
        nonce: String,
    ) -> Self {
        Self {
            payment_payload,
            payment_requirements,
            queued_at: Instant::now(),
            nonce,
        }
    }
}

/// FIFO queue for pending settlements backed by SQLite
pub struct SettlementQueue {
    /// SQLite-backed persistent store
    store: Arc<SettlementStore>,
    /// Notify handle to wake up workers when items are added
    notify: Arc<Notify>,
    /// Maximum queue size
    max_size: usize,
    /// Current queue length (atomic for fast access without lock)
    len: AtomicUsize,
}

impl SettlementQueue {
    /// Create a new settlement queue with the default max size
    /// Uses default database path "data/settlements.db"
    pub fn new() -> Self {
        Self::with_max_size(DEFAULT_MAX_QUEUE_SIZE)
    }

    /// Create a new settlement queue with a custom max size
    /// Uses default database path "data/settlements.db"
    pub fn with_max_size(max_size: usize) -> Self {
        Self::with_store_and_max_size(Self::default_db_path(), max_size)
            .expect("Failed to initialize settlement store")
    }

    /// Default database path
    fn default_db_path() -> &'static str {
        "data/settlements.db"
    }

    /// Create a settlement queue with a specific database path and max size
    pub fn with_store_and_max_size(
        db_path: &str,
        max_size: usize,
    ) -> Result<Self, crate::services::settlement_store::SettlementStoreError> {
        // Ensure the data directory exists
        if let Some(parent) = std::path::Path::new(db_path).parent() {
            std::fs::create_dir_all(parent).ok();
        }

        let store = Arc::new(SettlementStore::open(db_path)?);

        // Recover any settlements that were in_progress when we crashed
        let recovered = store.recover_in_progress()?;
        if recovered > 0 {
            info!(
                "Recovered {} in-progress settlements from previous session",
                recovered
            );
        }

        // Get initial count from database
        let pending = store.pending_count()? as usize;
        let in_progress = store.in_progress_count()? as usize;
        let initial_len = pending + in_progress;

        if initial_len > 0 {
            info!(
                "Loaded {} pending settlements from database ({} pending, {} in-progress)",
                initial_len, pending, in_progress
            );
        }

        Ok(Self {
            store,
            notify: Arc::new(Notify::new()),
            max_size,
            len: AtomicUsize::new(initial_len),
        })
    }

    /// Get a reference to the underlying store
    pub fn store(&self) -> &Arc<SettlementStore> {
        &self.store
    }

    /// Push a settlement to the queue (persisted to SQLite)
    /// Returns Ok(()) if successful, Err(PendingSettlement) if queue is full
    pub async fn push(&self, settlement: PendingSettlement) -> Result<(), PendingSettlement> {
        let current_len = self.len.load(Ordering::SeqCst);

        if current_len >= self.max_size {
            warn!(
                "Settlement queue full ({}/{}), rejecting settlement for nonce {}",
                current_len, self.max_size, settlement.nonce
            );
            return Err(settlement);
        }

        // Insert into SQLite (blocking call, but SQLite is fast)
        match self.store.insert(
            &settlement.nonce,
            &settlement.payment_payload,
            &settlement.payment_requirements,
        ) {
            Ok(Some(_id)) => {
                let new_len = self.len.fetch_add(1, Ordering::SeqCst) + 1;
                debug!(
                    "Queuing settlement for nonce {} (persisted), queue depth: {}",
                    settlement.nonce, new_len
                );

                // Notify waiting workers
                self.notify.notify_one();
                Ok(())
            }
            Ok(None) => {
                // Duplicate nonce - already exists in store
                debug!(
                    "Settlement for nonce {} already exists, skipping",
                    settlement.nonce
                );
                Ok(()) // Return Ok since the settlement is already persisted
            }
            Err(e) => {
                warn!("Failed to persist settlement for nonce {}: {}", settlement.nonce, e);
                // Return the settlement so caller knows it wasn't persisted
                Err(settlement)
            }
        }
    }

    /// Update the cached length from the database
    fn refresh_len(&self) {
        if let Ok(pending) = self.store.pending_count() {
            self.len.store(pending as usize, Ordering::SeqCst);
        }
    }

    /// Get the current queue length (pending settlements)
    pub fn len(&self) -> usize {
        self.len.load(Ordering::SeqCst)
    }

    /// Check if the queue is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Check if the queue is full
    pub fn is_full(&self) -> bool {
        self.len() >= self.max_size
    }

    /// Get the maximum queue size
    pub fn max_size(&self) -> usize {
        self.max_size
    }

    /// Wake up all waiting workers (used for shutdown)
    pub fn notify_all(&self) {
        self.notify.notify_waiters();
    }

    /// Wait for notification of new items
    pub async fn wait_for_items(&self) {
        self.notify.notified().await;
    }

    /// Get counts by status for metrics
    pub fn get_status_counts(&self) -> (i64, i64, i64, i64) {
        let pending = self.store.pending_count().unwrap_or(0);
        let in_progress = self.store.in_progress_count().unwrap_or(0);
        let completed = self
            .store
            .count_by_status(SettlementStatus::Completed)
            .unwrap_or(0);
        let failed = self
            .store
            .count_by_status(SettlementStatus::Failed)
            .unwrap_or(0);
        (pending, in_progress, completed, failed)
    }
}

impl Default for SettlementQueue {
    fn default() -> Self {
        Self::new()
    }
}

// Tests moved to settlement_store.rs which uses in-memory SQLite

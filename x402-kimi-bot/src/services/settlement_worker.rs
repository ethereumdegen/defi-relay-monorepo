//! Background worker for processing settlement queue
//!
//! This worker runs in the background, processing settlements from the queue
//! with retry logic and graceful shutdown support.
//!
//! Settlements are persisted to SQLite, so they survive server restarts.

use super::facilitator::FacilitatorClient;
use super::settlement_queue::SettlementQueue;
use super::settlement_store::StoredSettlement;
use backoff::{backoff::Backoff, ExponentialBackoff};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

/// Maximum retry attempts for a single settlement
const MAX_SETTLEMENT_RETRIES: u32 = 5;

/// Metrics for the settlement worker
#[derive(Default)]
pub struct SettlementMetrics {
    /// Total settlements processed
    pub total_processed: AtomicU64,
    /// Successful settlements
    pub success_count: AtomicU64,
    /// Failed settlements (after all retries)
    pub failure_count: AtomicU64,
    /// Total retry attempts
    pub retry_count: AtomicU64,
}

impl SettlementMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_success(&self) {
        self.total_processed.fetch_add(1, Ordering::Relaxed);
        self.success_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_failure(&self) {
        self.total_processed.fetch_add(1, Ordering::Relaxed);
        self.failure_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn record_retry(&self) {
        self.retry_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn get_stats(&self) -> (u64, u64, u64, u64) {
        (
            self.total_processed.load(Ordering::Relaxed),
            self.success_count.load(Ordering::Relaxed),
            self.failure_count.load(Ordering::Relaxed),
            self.retry_count.load(Ordering::Relaxed),
        )
    }
}

/// Background worker that processes settlements from the queue
pub struct SettlementWorker {
    /// The settlement queue to process from
    queue: Arc<SettlementQueue>,
    /// The facilitator client for settlement calls
    facilitator: FacilitatorClient,
    /// Metrics for observability
    metrics: Arc<SettlementMetrics>,
}

impl SettlementWorker {
    /// Create a new settlement worker
    pub fn new(queue: Arc<SettlementQueue>, facilitator: FacilitatorClient) -> Self {
        Self {
            queue,
            facilitator,
            metrics: Arc::new(SettlementMetrics::new()),
        }
    }

    /// Get a reference to the metrics
    pub fn metrics(&self) -> Arc<SettlementMetrics> {
        self.metrics.clone()
    }

    /// Run the worker until shutdown signal is received
    pub async fn run(&self, mut shutdown: broadcast::Receiver<()>) {
        info!("Settlement worker started (with SQLite persistence)");

        loop {
            // Try to claim the next pending settlement from the store
            let settlement = match self.queue.store().claim_next() {
                Ok(Some(s)) => {
                    // Decrement in-memory counter since we claimed one
                    self.queue.len();
                    Some(s)
                }
                Ok(None) => None,
                Err(e) => {
                    error!("Failed to claim settlement from store: {}", e);
                    None
                }
            };

            if let Some(s) = settlement {
                // Store the ID before processing in case we need it for recovery
                let settlement_id = s.id;

                // Process without blocking the shutdown check for too long
                tokio::select! {
                    biased;

                    _ = shutdown.recv() => {
                        // Put it back to pending since we didn't finish
                        if let Err(e) = self.queue.store().record_retry(settlement_id, "Worker shutdown") {
                            error!("Failed to re-queue settlement {} on shutdown: {}", settlement_id, e);
                        }
                        info!("Settlement worker received shutdown signal during processing");
                        break;
                    }

                    _ = self.process_settlement(s) => {
                        // Settlement processed (success or permanent failure)
                    }
                }
            } else {
                // No pending settlements, wait for notification or shutdown
                tokio::select! {
                    biased;

                    _ = shutdown.recv() => {
                        info!("Settlement worker received shutdown signal");
                        break;
                    }

                    _ = self.queue.wait_for_items() => {
                        // New item added, loop back to claim it
                    }

                    _ = tokio::time::sleep(Duration::from_secs(5)) => {
                        // Periodic wake-up to check for items (in case notify was missed)
                    }
                }
            }
        }

        // Graceful shutdown: log remaining queue size (persisted, so not lost!)
        let remaining = self.queue.len();
        if remaining > 0 {
            info!(
                "Settlement worker shutting down with {} pending settlements (persisted to disk)",
                remaining
            );
        }

        let (total, success, failure, retries) = self.metrics.get_stats();
        info!(
            "Settlement worker stopped. Stats: total={}, success={}, failure={}, retries={}",
            total, success, failure, retries
        );
    }

    /// Process a single settlement with retry logic
    async fn process_settlement(&self, settlement: StoredSettlement) {
        let nonce = &settlement.nonce;
        let id = settlement.id;
        let queued_duration = chrono::Utc::now()
            .signed_duration_since(settlement.queued_at)
            .to_std()
            .unwrap_or_default();

        // Deserialize the stored JSON
        let payment_payload = match settlement.payment_payload() {
            Ok(p) => p,
            Err(e) => {
                error!(
                    "Failed to deserialize payment payload for settlement {}: {}",
                    id, e
                );
                let _ = self
                    .queue
                    .store()
                    .mark_failed(id, &format!("Deserialization error: {}", e));
                self.metrics.record_failure();
                return;
            }
        };

        let payment_requirements = match settlement.payment_requirements() {
            Ok(r) => r,
            Err(e) => {
                error!(
                    "Failed to deserialize payment requirements for settlement {}: {}",
                    id, e
                );
                let _ = self
                    .queue
                    .store()
                    .mark_failed(id, &format!("Deserialization error: {}", e));
                self.metrics.record_failure();
                return;
            }
        };

        debug!(
            "Processing settlement {} for nonce {} (queued for {:?}, {} previous retries)",
            id, nonce, queued_duration, settlement.retry_count
        );

        let mut backoff = Self::create_backoff();
        let mut attempts = settlement.retry_count as u32;
        let max_total_attempts = MAX_SETTLEMENT_RETRIES + settlement.retry_count as u32;

        loop {
            attempts += 1;

            let start = Instant::now();
            let result = self
                .facilitator
                .settle(payment_payload.clone(), payment_requirements.clone())
                .await;

            let elapsed = start.elapsed();

            match result {
                Ok(settle_response) if settle_response.success => {
                    let tx_hash = settle_response
                        .transaction
                        .as_deref()
                        .unwrap_or("unknown");
                    info!(
                        "Settlement successful for nonce {} (attempt {}, took {:?}). Tx: {}",
                        nonce, attempts, elapsed, tx_hash
                    );
                    if let Err(e) = self.queue.store().mark_completed(id, tx_hash) {
                        error!("Failed to mark settlement {} as completed: {}", id, e);
                    }
                    self.metrics.record_success();
                    return;
                }
                Ok(settle_response) => {
                    // Settlement call succeeded but settlement itself failed
                    let error_msg = settle_response
                        .error_reason
                        .unwrap_or_else(|| "Unknown error".to_string());

                    // Check if this is a retryable error
                    if Self::is_retryable_error(&error_msg) && attempts < max_total_attempts {
                        warn!(
                            "Settlement failed for nonce {} (attempt {}/{}): {}. Retrying...",
                            nonce, attempts, max_total_attempts, error_msg
                        );
                        self.metrics.record_retry();

                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    // Non-retryable or max retries exceeded
                    error!(
                        "Settlement permanently failed for nonce {} after {} attempts: {}. \
                         Settlement ID {} saved in database for manual retry.",
                        nonce, attempts, error_msg, id
                    );
                    if let Err(e) = self.queue.store().mark_failed(id, &error_msg) {
                        error!("Failed to mark settlement {} as failed: {}", id, e);
                    }
                    self.metrics.record_failure();
                    return;
                }
                Err(e) => {
                    let error_msg = e.to_string();

                    // Network/connection error - always retry these
                    if attempts < max_total_attempts {
                        warn!(
                            "Settlement error for nonce {} (attempt {}/{}): {}. Retrying...",
                            nonce, attempts, max_total_attempts, error_msg
                        );
                        self.metrics.record_retry();

                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    error!(
                        "Settlement permanently failed for nonce {} after {} attempts: {}. \
                         Settlement ID {} saved in database for manual retry.",
                        nonce, attempts, error_msg, id
                    );
                    if let Err(e) = self.queue.store().mark_failed(id, &error_msg) {
                        error!("Failed to mark settlement {} as failed: {}", id, e);
                    }
                    self.metrics.record_failure();
                    return;
                }
            }
        }
    }

    /// Create a backoff strategy for retries
    /// Blockchain settlements can be slow during congestion, so we use longer intervals
    fn create_backoff() -> ExponentialBackoff {
        ExponentialBackoff {
            initial_interval: Duration::from_secs(10),
            max_interval: Duration::from_secs(120),
            max_elapsed_time: Some(Duration::from_secs(600)), // 10 minutes total
            multiplier: 2.0,
            ..ExponentialBackoff::default()
        }
    }

    /// Check if an error message indicates a retryable condition
    fn is_retryable_error(error: &str) -> bool {
        let error_lower = error.to_lowercase();
        error_lower.contains("timeout")
            || error_lower.contains("connection")
            || error_lower.contains("network")
            || error_lower.contains("rate limit")
            || error_lower.contains("too many requests")
            || error_lower.contains("unavailable")
            || error_lower.contains("temporary")
            || error_lower.contains("retry")
            || error_lower.contains("congestion")
            || error_lower.contains("nonce too low") // Blockchain nonce issues
            || error_lower.contains("replacement transaction") // Tx replacement issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_retryable_error() {
        assert!(SettlementWorker::is_retryable_error("Connection timeout"));
        assert!(SettlementWorker::is_retryable_error("Network error"));
        assert!(SettlementWorker::is_retryable_error("Service temporarily unavailable"));
        assert!(SettlementWorker::is_retryable_error("Rate limit exceeded"));
        assert!(SettlementWorker::is_retryable_error("nonce too low"));

        assert!(!SettlementWorker::is_retryable_error("Invalid signature"));
        assert!(!SettlementWorker::is_retryable_error("Insufficient balance"));
    }

    #[test]
    fn test_metrics() {
        let metrics = SettlementMetrics::new();

        metrics.record_success();
        metrics.record_success();
        metrics.record_failure();
        metrics.record_retry();
        metrics.record_retry();
        metrics.record_retry();

        let (total, success, failure, retries) = metrics.get_stats();
        assert_eq!(total, 3);
        assert_eq!(success, 2);
        assert_eq!(failure, 1);
        assert_eq!(retries, 3);
    }
}

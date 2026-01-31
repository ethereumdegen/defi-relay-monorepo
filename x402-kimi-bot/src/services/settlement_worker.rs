//! Background worker for processing settlement queue
//!
//! This worker runs in the background, processing settlements from the queue
//! with retry logic and graceful shutdown support.

use super::facilitator::FacilitatorClient;
use super::settlement_queue::{PendingSettlement, SettlementQueue};
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
        info!("Settlement worker started");

        loop {
            tokio::select! {
                biased;

                // Check for shutdown signal first
                _ = shutdown.recv() => {
                    info!("Settlement worker received shutdown signal");
                    break;
                }

                // Process settlements from queue
                settlement = self.queue.pop() => {
                    if let Some(s) = settlement {
                        self.process_settlement(s).await;
                    }
                }
            }
        }

        // Graceful shutdown: log remaining queue size
        let remaining = self.queue.len();
        if remaining > 0 {
            warn!(
                "Settlement worker shutting down with {} pending settlements in queue",
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
    async fn process_settlement(&self, settlement: PendingSettlement) {
        let nonce = &settlement.nonce;
        let queued_duration = settlement.queued_at.elapsed();

        debug!(
            "Processing settlement for nonce {} (queued for {:?})",
            nonce, queued_duration
        );

        let mut backoff = Self::create_backoff();
        let mut attempts = 0;

        loop {
            attempts += 1;

            let start = Instant::now();
            let result = self
                .facilitator
                .settle(
                    settlement.payment_payload.clone(),
                    settlement.payment_requirements.clone(),
                )
                .await;

            let elapsed = start.elapsed();

            match result {
                Ok(settle_response) if settle_response.success => {
                    info!(
                        "Settlement successful for nonce {} (attempt {}, took {:?}). Tx: {:?}",
                        nonce, attempts, elapsed, settle_response.transaction
                    );
                    self.metrics.record_success();
                    return;
                }
                Ok(settle_response) => {
                    // Settlement call succeeded but settlement itself failed
                    let error_msg = settle_response
                        .error_reason
                        .unwrap_or_else(|| "Unknown error".to_string());

                    // Check if this is a retryable error
                    if Self::is_retryable_error(&error_msg) && attempts < MAX_SETTLEMENT_RETRIES {
                        warn!(
                            "Settlement failed for nonce {} (attempt {}/{}): {}. Retrying...",
                            nonce, attempts, MAX_SETTLEMENT_RETRIES, error_msg
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
                         Manual intervention may be required. Payload: {:?}",
                        nonce, attempts, error_msg, settlement.payment_payload
                    );
                    self.metrics.record_failure();
                    return;
                }
                Err(e) => {
                    // Network/connection error - always retry these
                    if attempts < MAX_SETTLEMENT_RETRIES {
                        warn!(
                            "Settlement error for nonce {} (attempt {}/{}): {}. Retrying...",
                            nonce, attempts, MAX_SETTLEMENT_RETRIES, e
                        );
                        self.metrics.record_retry();

                        if let Some(duration) = backoff.next_backoff() {
                            tokio::time::sleep(duration).await;
                            continue;
                        }
                    }

                    error!(
                        "Settlement permanently failed for nonce {} after {} attempts: {}. \
                         Manual intervention may be required. Payload: {:?}",
                        nonce, attempts, e, settlement.payment_payload
                    );
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

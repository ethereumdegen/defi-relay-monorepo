//! Settlement queue for decoupling x402 settlement from HTTP request flow
//!
//! This module provides a FIFO queue for pending settlements that allows
//! the HTTP request to return immediately after payment verification,
//! while settlement is processed asynchronously by a background worker.

use crate::models::{PaymentPayload, VerifyPaymentRequirements};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{Mutex, Notify};
use tracing::{debug, warn};

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

/// FIFO queue for pending settlements
pub struct SettlementQueue {
    /// The inner queue protected by a mutex
    queue: Mutex<VecDeque<PendingSettlement>>,
    /// Notify handle to wake up workers when items are added
    notify: Arc<Notify>,
    /// Maximum queue size
    max_size: usize,
    /// Current queue length (atomic for fast access without lock)
    len: AtomicUsize,
}

impl SettlementQueue {
    /// Create a new settlement queue with the default max size
    pub fn new() -> Self {
        Self::with_max_size(DEFAULT_MAX_QUEUE_SIZE)
    }

    /// Create a new settlement queue with a custom max size
    pub fn with_max_size(max_size: usize) -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            notify: Arc::new(Notify::new()),
            max_size,
            len: AtomicUsize::new(0),
        }
    }

    /// Push a settlement to the queue
    /// Returns Ok(()) if successful, Err(PendingSettlement) if queue is full
    pub async fn push(&self, settlement: PendingSettlement) -> Result<(), PendingSettlement> {
        let mut queue = self.queue.lock().await;

        if queue.len() >= self.max_size {
            warn!(
                "Settlement queue full ({}/{}), rejecting settlement for nonce {}",
                queue.len(),
                self.max_size,
                settlement.nonce
            );
            return Err(settlement);
        }

        debug!(
            "Queuing settlement for nonce {}, queue depth: {}",
            settlement.nonce,
            queue.len() + 1
        );

        queue.push_back(settlement);
        self.len.store(queue.len(), Ordering::SeqCst);

        // Notify waiting workers
        self.notify.notify_one();

        Ok(())
    }

    /// Pop the next settlement from the queue (FIFO)
    /// This will wait until an item is available or the notify is triggered
    pub async fn pop(&self) -> Option<PendingSettlement> {
        loop {
            // Try to get an item
            {
                let mut queue = self.queue.lock().await;
                if let Some(settlement) = queue.pop_front() {
                    self.len.store(queue.len(), Ordering::SeqCst);
                    debug!(
                        "Dequeued settlement for nonce {}, queue depth: {}",
                        settlement.nonce,
                        queue.len()
                    );
                    return Some(settlement);
                }
            }

            // Wait for notification that new items were added
            self.notify.notified().await;
        }
    }

    /// Try to pop without waiting (non-blocking)
    pub async fn try_pop(&self) -> Option<PendingSettlement> {
        let mut queue = self.queue.lock().await;
        let settlement = queue.pop_front();
        if settlement.is_some() {
            self.len.store(queue.len(), Ordering::SeqCst);
        }
        settlement
    }

    /// Get the current queue length
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
}

impl Default for SettlementQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::domains::{DomainEthAddress, DomainUint256};

    fn mock_settlement(nonce_id: &str) -> PendingSettlement {
        // Create a valid 32-byte hex nonce from the test identifier
        // Use a simple hash-like pattern: pad with zeros
        let nonce_hex = format!("0x{:0>64}", format!("{:x}", nonce_id.len() * 1000 + nonce_id.chars().map(|c| c as usize).sum::<usize>()));

        // Create minimal mock data for testing
        let payment_payload = PaymentPayload {
            x402_version: 2,
            accepted: crate::models::AcceptedRequirements {
                scheme: "exact".to_string(),
                network: "eip155:8453".to_string(),
                amount: "1000".to_string(),
                pay_to: DomainEthAddress::from_hex("0x0000000000000000000000000000000000000001").unwrap(),
                max_timeout_seconds: 60,
                asset: DomainEthAddress::from_hex("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913").unwrap(),
                extra: None,
            },
            payload: crate::models::ExactEvmPayload {
                signature: "0x".to_string(),
                authorization: crate::models::Eip3009Authorization {
                    from: DomainEthAddress::from_hex("0x0000000000000000000000000000000000000002").unwrap(),
                    to: DomainEthAddress::from_hex("0x0000000000000000000000000000000000000001").unwrap(),
                    value: DomainUint256::from_str("1000").unwrap(),
                    valid_after: DomainUint256::from_str("0").unwrap(),
                    valid_before: DomainUint256::from_str("999999999999").unwrap(),
                    nonce: crate::models::domains::DomainBytes32::from_hex(&nonce_hex).unwrap(),
                },
            },
        };

        let payment_requirements = VerifyPaymentRequirements {
            scheme: "exact".to_string(),
            network: "eip155:8453".to_string(),
            amount: "1000".to_string(),
            pay_to: DomainEthAddress::from_hex("0x0000000000000000000000000000000000000001").unwrap(),
            max_timeout_seconds: 60,
            asset: DomainEthAddress::from_hex("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913").unwrap(),
            extra: None,
        };

        // Use the original nonce_id as the tracking string (not the hex)
        PendingSettlement::new(payment_payload, payment_requirements, nonce_id.to_string())
    }

    #[tokio::test]
    async fn test_push_pop() {
        let queue = SettlementQueue::new();

        let settlement = mock_settlement("test1");
        queue.push(settlement).await.unwrap();

        assert_eq!(queue.len(), 1);

        let popped = queue.try_pop().await;
        assert!(popped.is_some());
        assert_eq!(popped.unwrap().nonce, "test1");
        assert_eq!(queue.len(), 0);
    }

    #[tokio::test]
    async fn test_fifo_order() {
        let queue = SettlementQueue::new();

        queue.push(mock_settlement("first")).await.unwrap();
        queue.push(mock_settlement("second")).await.unwrap();
        queue.push(mock_settlement("third")).await.unwrap();

        assert_eq!(queue.try_pop().await.unwrap().nonce, "first");
        assert_eq!(queue.try_pop().await.unwrap().nonce, "second");
        assert_eq!(queue.try_pop().await.unwrap().nonce, "third");
    }

    #[tokio::test]
    async fn test_max_size() {
        let queue = SettlementQueue::with_max_size(2);

        queue.push(mock_settlement("1")).await.unwrap();
        queue.push(mock_settlement("2")).await.unwrap();

        // Third should fail
        let result = queue.push(mock_settlement("3")).await;
        assert!(result.is_err());
        assert!(queue.is_full());
    }
}

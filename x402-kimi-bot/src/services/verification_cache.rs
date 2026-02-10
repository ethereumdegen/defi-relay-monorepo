use moka::sync::Cache;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

/// Default TTL for verified addresses (30 seconds).
const DEFAULT_CACHE_TTL_SECS: u64 = 30;

/// TTL for failure records (60 seconds).
/// Addresses with recent failures are downgraded to the sequential
/// verify-then-serve path so no Kimi API call is wasted.
const FAILURE_TTL_SECS: u64 = 60;

/// Cache of recently verified payer addresses.
///
/// When an address passes payment verification, it's cached so subsequent
/// requests from the same address skip the synchronous verification step.
/// Settlement is still queued for every request — the cache only removes
/// the verify-round-trip from the critical path.
///
/// Addresses that fail verification are tracked separately. While they have
/// a recent failure on record, requests are handled sequentially (verify
/// first, then call the downstream service) to avoid wasting API calls.
pub struct VerificationCache {
    cache: Cache<String, ()>,
    failures: Cache<String, ()>,
    hits: AtomicU64,
    misses: AtomicU64,
    downgrades: AtomicU64,
}

impl VerificationCache {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Cache::builder()
                .time_to_live(ttl)
                .max_capacity(10_000)
                .build(),
            failures: Cache::builder()
                .time_to_live(Duration::from_secs(FAILURE_TTL_SECS))
                .max_capacity(10_000)
                .build(),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            downgrades: AtomicU64::new(0),
        }
    }

    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(DEFAULT_CACHE_TTL_SECS))
    }

    /// Returns `true` if the address was recently verified.
    pub fn is_verified(&self, address: &str) -> bool {
        if self.cache.get(address).is_some() {
            self.hits.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            self.misses.fetch_add(1, Ordering::Relaxed);
            false
        }
    }

    /// Mark an address as recently verified.
    /// Also clears any failure record for this address.
    pub fn mark_verified(&self, address: &str) {
        self.cache.insert(address.to_string(), ());
        self.failures.invalidate(address);
    }

    /// Returns `true` if the address has a recent verification failure.
    /// When true, the caller should use the sequential path (verify first,
    /// then serve) instead of the parallel path.
    pub fn has_recent_failure(&self, address: &str) -> bool {
        if self.failures.get(address).is_some() {
            self.downgrades.fetch_add(1, Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    /// Record a verification failure for an address.
    pub fn record_failure(&self, address: &str) {
        self.failures.insert(address.to_string(), ());
    }

    /// Returns `(hits, misses, downgrades)`.
    pub fn stats(&self) -> (u64, u64, u64) {
        (
            self.hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
            self.downgrades.load(Ordering::Relaxed),
        )
    }
}

use moka::sync::Cache;
use std::time::Duration;

/// Tracks used payment nonces to prevent replay attacks.
#[derive(Clone)]
pub struct NonceTracker {
    cache: Cache<String, ()>,
}

impl NonceTracker {
    /// Creates a new nonce tracker with the specified TTL.
    pub fn new(ttl: Duration) -> Self {
        let cache = Cache::builder()
            .time_to_live(ttl)
            .max_capacity(100_000)
            .build();

        NonceTracker { cache }
    }

    /// Creates a nonce tracker with the default 10 minute TTL.
    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(10 * 60))
    }

    /// Attempts to mark a nonce as used.
    /// Returns `true` if the nonce was successfully marked (first use).
    /// Returns `false` if the nonce was already used (replay attempt).
    pub fn try_use_nonce(&self, nonce: &str) -> bool {
        if self.cache.contains_key(nonce) {
            return false;
        }

        let was_present = self.cache.get(nonce).is_some();
        if was_present {
            return false;
        }

        self.cache.insert(nonce.to_string(), ());
        true
    }
}

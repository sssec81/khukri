use std::time::Instant;
use tokio::time::{sleep, Duration};

/// Token-bucket rate limiter for bandwidth capping.
///
/// Capacity = 1 second of burst. Tokens refill continuously at `bytes_per_sec`.
/// `consume` blocks (via sleep) until enough tokens are available.
///
/// Wrapped in `Arc<tokio::sync::Mutex<TokenBucket>>` when shared across
/// segment tasks — tokio's Mutex allows holding the guard across `.await`.
pub struct TokenBucket {
    bytes_per_sec: u64,
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    /// Create a new bucket. `bytes_per_sec = 0` is treated as unlimited.
    pub fn new(bytes_per_sec: u64) -> Self {
        Self {
            bytes_per_sec,
            tokens: bytes_per_sec as f64, // start full
            last_refill: Instant::now(),
        }
    }

    /// Consume `bytes` tokens, sleeping if the bucket is empty.
    pub async fn consume(&mut self, bytes: u64) {
        if self.bytes_per_sec == 0 {
            return; // unlimited
        }
        self.refill();

        let bytes_f = bytes as f64;
        if self.tokens < bytes_f {
            let deficit = bytes_f - self.tokens;
            let wait_secs = deficit / self.bytes_per_sec as f64;
            sleep(Duration::from_secs_f64(wait_secs)).await;
            self.refill();
        }

        self.tokens = (self.tokens - bytes_f).max(0.0);
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        let cap = self.bytes_per_sec as f64; // 1-second burst cap
        self.tokens = (self.tokens + elapsed * self.bytes_per_sec as f64).min(cap);
        self.last_refill = now;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    /// Consuming less than a full bucket should not sleep.
    #[tokio::test]
    async fn test_no_sleep_when_tokens_available() {
        let mut bucket = TokenBucket::new(1_000_000); // 1 MB/s
        let start = Instant::now();
        bucket.consume(512).await; // well within 1-second capacity
        assert!(start.elapsed().as_millis() < 50, "should not sleep");
    }

    /// Consuming more than capacity forces a delay proportional to the deficit.
    #[tokio::test]
    async fn test_sleeps_on_deficit() {
        let bps = 10_000u64; // 10 KB/s
        let mut bucket = TokenBucket::new(bps);
        // Drain the starting tokens first.
        bucket.consume(bps).await;

        let start = Instant::now();
        bucket.consume(bps / 2).await; // needs ~0.5 s to refill
        let elapsed_ms = start.elapsed().as_millis();

        // Allow generous margin (CI can be slow): at least 300 ms, less than 1500 ms.
        assert!(elapsed_ms >= 300, "expected a delay, got {elapsed_ms}ms");
        assert!(elapsed_ms < 1_500, "delay too long: {elapsed_ms}ms");
    }

    /// bytes_per_sec = 0 means unlimited — never sleeps.
    #[tokio::test]
    async fn test_unlimited_never_sleeps() {
        let mut bucket = TokenBucket::new(0);
        let start = Instant::now();
        bucket.consume(u64::MAX).await;
        assert!(start.elapsed().as_millis() < 10);
    }
}

use std::future::Future;
use std::time::Duration;

/// Fixed exponential backoff sequence, in seconds, per the project requirements:
/// retry after 2, 4, 8, 16, 32, 64 seconds, then stop.
const DELAYS_SECS: [u64; 6] = [2, 4, 8, 16, 32, 64];

/// Whether a failure is worth retrying at all.
/// Implemented per error type so `with_retry` can fail fast on non-transient errors
/// (e.g. a 4xx API response) instead of burning the full backoff sequence on something
/// that will never succeed.
pub trait Retryable {
    fn is_retryable(&self) -> bool;
}

/// Retries `op` using the fixed backoff sequence above.
/// If `enabled` is false, `op` is attempted once and any error is returned immediately.
/// A non-retryable error is also returned immediately, regardless of `enabled` or
/// how many attempts remain.
pub async fn with_retry<F, Fut, T, E>(enabled: bool, mut op: F) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, E>>,
    E: Retryable + std::fmt::Display,
{
    let mut attempt = 0;
    loop {
        match op().await {
            Ok(value) => return Ok(value),
            Err(err) => {
                if !enabled || !err.is_retryable() || attempt >= DELAYS_SECS.len() {
                    return Err(err);
                }
                let delay = DELAYS_SECS[attempt];
                tracing::warn!(
                    attempt = attempt + 1,
                    error = %err,
                    delay_secs = delay,
                    "attempt failed, retrying"
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
                attempt += 1;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[derive(Debug)]
    struct FakeError {
        retryable: bool,
    }

    impl std::fmt::Display for FakeError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "fake error")
        }
    }

    impl Retryable for FakeError {
        fn is_retryable(&self) -> bool {
            self.retryable
        }
    }

    #[tokio::test(start_paused = true)]
    async fn retries_with_exact_backoff_sequence_then_succeeds() {
        let attempts = Rc::new(Cell::new(0));
        let start = tokio::time::Instant::now();

        let result: Result<u32, FakeError> = with_retry(true, || {
            attempts.set(attempts.get() + 1);
            let attempts = attempts.clone();
            async move {
                if attempts.get() < 7 {
                    Err(FakeError { retryable: true })
                } else {
                    Ok(42)
                }
            }
        })
        .await;

        assert_eq!(result.unwrap(), 42);
        assert_eq!(attempts.get(), 7);
        assert_eq!(
            start.elapsed(),
            Duration::from_secs(2 + 4 + 8 + 16 + 32 + 64)
        );
    }

    #[tokio::test(start_paused = true)]
    async fn stops_after_final_retry() {
        let attempts = Rc::new(Cell::new(0));

        let result: Result<u32, FakeError> = with_retry(true, || {
            attempts.set(attempts.get() + 1);
            async move { Err(FakeError { retryable: true }) }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.get(), 7); // 1 initial attempt + 6 retries
    }

    #[tokio::test]
    async fn disabled_retry_fails_fast() {
        let attempts = Cell::new(0);

        let result: Result<u32, FakeError> = with_retry(false, || {
            attempts.set(attempts.get() + 1);
            async move { Err(FakeError { retryable: true }) }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.get(), 1);
    }

    #[tokio::test]
    async fn non_retryable_error_fails_fast_even_when_enabled() {
        let attempts = Cell::new(0);

        let result: Result<u32, FakeError> = with_retry(true, || {
            attempts.set(attempts.get() + 1);
            async move { Err(FakeError { retryable: false }) }
        })
        .await;

        assert!(result.is_err());
        assert_eq!(attempts.get(), 1);
    }
}

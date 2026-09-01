use std::time::Duration;

/// Asynchronously sleeps for `duration` using the Tokio timer.
///
/// Requires the `tokio` cargo feature (enabled by default).
pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::sleep;

    #[tokio::test]
    async fn waits_at_least_the_requested_duration() {
        let start = Instant::now();
        sleep(Duration::from_millis(20)).await;
        assert!(start.elapsed() >= Duration::from_millis(20));
    }

    #[tokio::test]
    async fn zero_duration_completes() {
        sleep(Duration::ZERO).await;
    }
}

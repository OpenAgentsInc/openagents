//! Executor and monotonic clocks for one authoritative client.
use std::future::Future;
#[cfg(target_arch = "wasm32")]
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
pub use tokio::time::{
    Instant, MissedTickBehavior, interval, sleep, sleep_until, timeout, timeout_at,
};
#[cfg(not(target_arch = "wasm32"))]
pub type Task = tokio::task::JoinHandle<()>;
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn(future: impl Future<Output = ()> + Send + 'static) -> Task {
    tokio::spawn(future)
}
#[cfg(target_arch = "wasm32")]
pub use browser::*;
#[cfg(target_arch = "wasm32")]
mod browser {
    use super::*;
    use futures_util::future::{AbortHandle, Abortable, Either, select};
    pub use web_time::Instant;
    pub type Task = AbortHandle;
    pub fn spawn(future: impl Future<Output = ()> + 'static) -> Task {
        let (handle, registration) = AbortHandle::new_pair();
        wasm_bindgen_futures::spawn_local(async move {
            let _ = Abortable::new(future, registration).await;
        });
        handle
    }
    pub async fn sleep(duration: Duration) {
        // Long waits are split without wrapping the browser's millisecond timer.
        let mut left = duration;
        while !left.is_zero() {
            let part = left.min(Duration::from_secs(86400));
            let millis = part.as_nanos().div_ceil(1_000_000).max(1) as u32;
            gloo_timers::future::TimeoutFuture::new(millis).await;
            left = left.saturating_sub(part);
        }
    }
    pub async fn sleep_until(deadline: Instant) {
        sleep(deadline.saturating_duration_since(Instant::now())).await;
    }
    pub async fn timeout<T>(duration: Duration, future: impl Future<Output = T>) -> Result<T, ()> {
        match select(Box::pin(future), Box::pin(sleep(duration))).await {
            Either::Left((value, _)) => Ok(value),
            Either::Right(_) => Err(()),
        }
    }
    pub async fn timeout_at<T>(
        deadline: Instant,
        future: impl Future<Output = T>,
    ) -> Result<T, ()> {
        timeout(deadline.saturating_duration_since(Instant::now()), future).await
    }
    pub enum MissedTickBehavior {
        Skip,
    }
    pub struct Interval {
        next: Instant,
        period: Duration,
    }
    pub fn interval(period: Duration) -> Interval {
        assert!(!period.is_zero());
        Interval {
            next: Instant::now(),
            period,
        }
    }
    impl Interval {
        pub fn set_missed_tick_behavior(&mut self, _: MissedTickBehavior) {}
        pub async fn tick(&mut self) -> Instant {
            sleep_until(self.next).await;
            let now = Instant::now();
            self.next = now + self.period;
            now
        }
    }
}

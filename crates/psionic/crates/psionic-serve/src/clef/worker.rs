//! A bounded worker whose owner waits for its resources to be released.

use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicUsize, Ordering},
    mpsc,
};
use std::thread::JoinHandle;

/// A dropped HTTP future can stop queued work even when its blocking task
/// remains alive. The executor checks the flag between completed GPU chunks.
pub(super) struct CancellationGuard(pub Arc<AtomicBool>);

impl CancellationGuard {
    pub fn new() -> Self {
        Self(Arc::new(AtomicBool::new(false)))
    }
}

impl Drop for CancellationGuard {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

pub(super) async fn blocking<T: Send + 'static>(
    work: impl FnOnce(Arc<AtomicBool>) -> T + Send + 'static,
) -> Result<T, tokio::task::JoinError> {
    let cancelled = CancellationGuard::new();
    let flag = Arc::clone(&cancelled.0);
    tokio::task::spawn_blocking(move || work(flag)).await
}

/// Announces serial work before taking the execution lock. The batch worker
/// drains its active set and pauses admission until this permit drops.
pub(super) struct SerialWaiter(Arc<AtomicUsize>);

impl SerialWaiter {
    pub fn new(count: &Arc<AtomicUsize>) -> Self {
        count.fetch_add(1, Ordering::AcqRel);
        Self(Arc::clone(count))
    }
}

impl Drop for SerialWaiter {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

pub(super) struct Worker<T> {
    sender: Option<mpsc::SyncSender<T>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl<T: Send + 'static> Worker<T> {
    pub fn spawn(
        capacity: usize,
        run: impl FnOnce(mpsc::Receiver<T>, Arc<AtomicBool>) + Send + 'static,
    ) -> Result<Self, std::io::Error> {
        let (sender, receiver) = mpsc::sync_channel(capacity);
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = Arc::clone(&stop);
        let thread = std::thread::Builder::new()
            .name(String::from("clef-batch"))
            .spawn(move || run(receiver, worker_stop))?;
        Ok(Self {
            sender: Some(sender),
            stop,
            thread: Some(thread),
        })
    }

    pub fn submit(&self, job: T) -> Result<(), mpsc::TrySendError<T>> {
        self.sender
            .as_ref()
            .expect("live worker sender")
            .try_send(job)
    }
}

impl<T> Drop for Worker<T> {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        // Disconnect also wakes an idle worker blocked in recv().
        self.sender.take();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn http_disconnect_cancels_blocking_work() {
        use axum::{Router, routing::post};
        use std::time::{Duration, Instant};
        use tokio::io::AsyncWriteExt;
        let (started, mut started_rx) = tokio::sync::mpsc::unbounded_channel();
        let (stopped, mut stopped_rx) = tokio::sync::mpsc::unbounded_channel();
        let app = Router::new().route(
            "/decision",
            post(move || {
                let started = started.clone();
                let stopped = stopped.clone();
                async move {
                    blocking(move |cancelled| {
                        started.send(()).unwrap();
                        let deadline = Instant::now() + Duration::from_secs(10);
                        while !cancelled.load(Ordering::Acquire) && Instant::now() < deadline {
                            std::thread::sleep(Duration::from_millis(1));
                        }
                        stopped.send(cancelled.load(Ordering::Acquire)).unwrap();
                    })
                    .await
                    .unwrap();
                    "done"
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let mut client = tokio::net::TcpStream::connect(address).await.unwrap();
        client
            .write_all(b"POST /decision HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\n\r\n")
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), started_rx.recv())
            .await
            .unwrap()
            .unwrap();
        drop(client);
        let cancelled = tokio::time::timeout(Duration::from_secs(5), stopped_rx.recv()).await;
        server.abort();
        let _ = server.await;
        assert_eq!(cancelled.unwrap(), Some(true));
    }

    #[test]
    fn serial_waiter_is_visible_before_lock_and_until_serial_work_finishes() {
        let running = Arc::new(Mutex::new(()));
        let active = running.lock().unwrap();
        let count = Arc::new(AtomicUsize::new(0));
        let worker_running = Arc::clone(&running);
        let worker_count = Arc::clone(&count);
        let (waiting, waiting_rx) = mpsc::channel();
        let (entered, entered_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let serial = std::thread::spawn(move || {
            let _waiter = SerialWaiter::new(&worker_count);
            waiting.send(()).unwrap();
            let _running = worker_running.lock().unwrap();
            entered.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        waiting_rx.recv().unwrap();
        assert_eq!(count.load(Ordering::Acquire), 1);
        assert!(entered_rx.try_recv().is_err());
        drop(active);
        entered_rx.recv().unwrap();
        assert_eq!(count.load(Ordering::Acquire), 1);
        release.send(()).unwrap();
        serial.join().unwrap();
        assert_eq!(count.load(Ordering::Acquire), 0);
    }

    #[test]
    fn dropping_idle_worker_joins_and_releases_model() {
        let model = Arc::new(());
        let worker_model = Arc::clone(&model);
        let worker = Worker::<()>::spawn(1, move |receiver, _| {
            let _owned = worker_model;
            assert!(receiver.recv().is_err());
        })
        .unwrap();
        drop(worker);
        assert_eq!(Arc::strong_count(&model), 1);
    }

    #[test]
    fn shutdown_finishes_current_work_then_drops_queued_work_and_unlocks() {
        let running = Arc::new(Mutex::new(()));
        let worker_running = Arc::clone(&running);
        let (entered, entered_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        let model = Arc::new(());
        let worker_model = Arc::clone(&model);
        let worker = Worker::spawn(1, move |receiver, stop| {
            let _model = worker_model;
            let job = receiver.recv().unwrap();
            let guard = worker_running.lock().unwrap();
            entered.send(()).unwrap();
            release_rx.recv().unwrap();
            assert!(stop.load(Ordering::Acquire));
            drop(job);
            drop(guard);
            // The receiver drops the queued job before the thread ends.
        })
        .unwrap();
        worker.submit(Arc::clone(&model)).unwrap();
        entered_rx.recv().unwrap();
        worker.submit(Arc::clone(&model)).unwrap();
        assert!(matches!(
            worker.submit(Arc::clone(&model)),
            Err(mpsc::TrySendError::Full(_))
        ));
        assert!(running.try_lock().is_err());
        let stop = Arc::clone(&worker.stop);
        let dropping = std::thread::spawn(move || drop(worker));
        while !stop.load(Ordering::Acquire) {
            std::thread::yield_now();
        }
        release.send(()).unwrap();
        dropping.join().unwrap();
        assert_eq!(Arc::strong_count(&model), 1);
        assert!(running.try_lock().is_ok());
    }
}

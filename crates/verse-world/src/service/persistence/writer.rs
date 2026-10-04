//! One storage owner, one queued persistence copy, and ordered completions.
use super::{Commit, Store};
use crate::service::save::Prepared;
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Instant,
};
use tokio::sync::mpsc;

pub(in crate::service) struct Work {
    pub token: u64,
    pub prepared: Prepared,
}
pub(in crate::service) struct Done {
    pub token: u64,
    pub result: Result<Commit, String>,
    pub seconds: f64,
}
pub(in crate::service) struct Writer {
    pub send: Option<mpsc::Sender<Work>>,
    pub done: mpsc::UnboundedReceiver<Done>,
    cancel: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}
impl Writer {
    pub(in crate::service) fn start(mut store: Store) -> Result<Self, String> {
        let (send, mut receive) = mpsc::channel::<Work>(1);
        // At most two submitted copies exist, so completions are bounded by those tokens.
        let (finished, done) = mpsc::unbounded_channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let canceled = cancel.clone();
        let thread = std::thread::Builder::new()
            .name("verse-world-storage".into())
            .spawn(move || {
                while let Some(work) = receive.blocking_recv() {
                    if canceled.load(Ordering::Acquire) {
                        break;
                    }
                    let start = Instant::now();
                    let result = store.commit_prepared(work.prepared);
                    let failed = result.is_err();
                    if finished
                        .send(Done {
                            token: work.token,
                            result,
                            seconds: start.elapsed().as_secs_f64(),
                        })
                        .is_err()
                        || failed
                    {
                        break;
                    }
                }
            })
            .map_err(|_| "Cannot start chamber storage writer")?;
        Ok(Self {
            send: Some(send),
            done,
            cancel,
            thread: Some(thread),
        })
    }
}
impl Drop for Writer {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
        self.send.take();
        // Joining prevents an aborted host from leaving a writer behind its lock.
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

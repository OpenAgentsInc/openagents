use std::cell::RefCell;
use std::future::Future;
use std::marker::PhantomData;
use std::rc::Rc;

use futures::channel::oneshot;
#[cfg(any(not(target_arch = "wasm32"), all(target_arch = "wasm32", not(feature = "web"))))]
use futures::executor::block_on;
use futures::executor::LocalPool;
use futures::task::LocalSpawnExt;

use super::Task;

#[derive(Clone, Copy, Default)]
pub struct BackgroundExecutor;

impl BackgroundExecutor {
    pub fn new() -> Self {
        Self
    }

    pub fn spawn<R>(&self, future: impl Future<Output = R> + Send + 'static) -> Task<R>
    where
        R: Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();

        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result = block_on(future);
            let _ = sender.send(result);
        });

        #[cfg(all(target_arch = "wasm32", feature = "web"))]
        wasm_bindgen_futures::spawn_local(async move {
            let result = future.await;
            let _ = sender.send(result);
        });

        #[cfg(all(target_arch = "wasm32", not(feature = "web")))]
        {
            let result = block_on(future);
            let _ = sender.send(result);
        }

        Task::from_receiver(receiver)
    }
}

pub struct ForegroundExecutor {
    pool: RefCell<LocalPool>,
    _not_send: PhantomData<Rc<()>>,
}

impl ForegroundExecutor {
    pub fn new() -> Self {
        Self {
            pool: RefCell::new(LocalPool::new()),
            _not_send: PhantomData,
        }
    }

    pub fn spawn<R>(&self, future: impl Future<Output = R> + 'static) -> Task<R>
    where
        R: 'static,
    {
        let (sender, receiver) = oneshot::channel();
        let spawner = self.pool.borrow().spawner();
        spawner
            .spawn_local(async move {
                let result = future.await;
                let _ = sender.send(result);
            })
            .expect("foreground spawn failed");
        Task::from_receiver(receiver)
    }

    pub fn run_until_stalled(&self) {
        self.pool.borrow_mut().run_until_stalled();
    }
}

impl Default for ForegroundExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::{BackgroundExecutor, ForegroundExecutor};
    use std::time::Duration;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn background_executor_completes_task() {
        let executor = BackgroundExecutor::new();
        let mut task = executor.spawn(async { 7 });

        for _ in 0..50 {
            if let Some(value) = task.try_take() {
                assert_eq!(value, 7);
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }

        panic!("background task did not complete");
    }

    #[test]
    fn foreground_executor_requires_polling() {
        let executor = ForegroundExecutor::new();
        let mut task = executor.spawn(async { 11 });

        assert_eq!(task.try_take(), None);
        executor.run_until_stalled();
        assert_eq!(task.try_take(), Some(11));
    }
}

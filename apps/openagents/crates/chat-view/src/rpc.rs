//! Calls to the engine that serves a chat, as the views see them.
//!
//! A view asks for a tool's full output, an attachment's bytes, or a
//! resend through one method call and a JSON answer. The desktop answers
//! through its engine connection; a host without one passes no [`Rpc`], and
//! the views draw what they already have.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use gpui::BackgroundExecutor;

/// The answer to one call: the reply's JSON, or why there is none.
pub type Reply = Pin<Box<dyn Future<Output = Result<serde_json::Value, String>> + Send>>;

/// One way to call the engine's methods.
pub trait Call: Send + Sync + 'static {
    fn call(&self, method: &str, params: serde_json::Value) -> Reply;
}

impl Call for Rpc {
    fn call(&self, method: &str, params: serde_json::Value) -> Reply {
        self.0.call(method, params)
    }
}

/// Call `method` on `engine`, giving up after `timeout` on `executor`'s
/// clock. A stalled but open link never fails a call on its own.
pub async fn call_with_timeout(
    engine: &(impl Call + ?Sized),
    executor: &BackgroundExecutor,
    method: &str,
    params: serde_json::Value,
    timeout: Duration,
) -> Result<serde_json::Value, String> {
    let call = engine.call(method, params);
    let timer = executor.timer(timeout);
    match first_of(call, timer).await {
        Some(result) => result,
        None => Err(format!("{method} timed out")),
    }
}

/// A cheaply cloned handle to the engine's methods.
#[derive(Clone)]
pub struct Rpc(Arc<dyn Call>);

impl Rpc {
    pub fn new(call: impl Call) -> Self {
        Self(Arc::new(call))
    }
}

/// The first of `call` and `timer` to finish: the call's result, or `None`
/// when the timer won.
async fn first_of<T>(
    call: Pin<Box<dyn Future<Output = T> + Send>>,
    timer: impl Future<Output = ()>,
) -> Option<T> {
    use std::task::Poll;
    let mut call = call;
    let mut timer = Box::pin(timer);
    std::future::poll_fn(move |cx| {
        if let Poll::Ready(value) = call.as_mut().poll(cx) {
            return Poll::Ready(Some(value));
        }
        if timer.as_mut().poll(cx).is_ready() {
            return Poll::Ready(None);
        }
        Poll::Pending
    })
    .await
}

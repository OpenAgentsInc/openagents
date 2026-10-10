//! Graceful shutdown (audit WEB-01).
//!
//! On SIGTERM (a Cloud Run rollout or scale-in) the server stops taking
//! connections, lets requests in flight finish, ends its open event
//! streams so they don't hold the drain forever, and gives up after a
//! bounded drain so the caller can still flush analytics before the
//! platform kills the process.

use std::future::{Future, IntoFuture};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use futures_util::{Stream, StreamExt};
use tokio::sync::watch;

/// How long open requests may take to finish after the signal. Cloud Run
/// allows ten seconds between SIGTERM and SIGKILL; this leaves room to
/// flush analytics.
pub const DRAIN: Duration = Duration::from_secs(8);

/// A shutdown that every long-lived response can watch.
#[derive(Clone)]
pub struct Shutdown(Arc<watch::Sender<bool>>);

impl Default for Shutdown {
    fn default() -> Self {
        Self(Arc::new(watch::channel(false).0))
    }
}

impl Shutdown {
    /// Starts the shutdown; every [`Shutdown::wait`] returns.
    pub fn trigger(&self) {
        self.0.send_replace(true);
    }

    /// Whether the shutdown has started.
    #[must_use]
    pub fn started(&self) -> bool {
        *self.0.borrow()
    }

    /// Returns once the shutdown has started.
    pub async fn wait(&self) {
        let mut receiver = self.0.subscribe();
        let _ = receiver.wait_for(|started| *started).await;
    }

    /// `stream`, ending when the shutdown starts (an event stream would
    /// otherwise keep the drain waiting until it times out).
    pub fn until<S: Stream>(&self, stream: S) -> impl Stream<Item = S::Item> + use<S> {
        let shutdown = self.clone();
        stream.take_until(async move { shutdown.wait().await })
    }
}

/// Resolves on SIGTERM or ctrl-c.
pub async fn signal() {
    #[cfg(unix)]
    {
        let term = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut term) => {
                    term.recv().await;
                }
                Err(_) => std::future::pending::<()>().await,
            }
        };
        tokio::select! {
            () = term => {}
            _ = tokio::signal::ctrl_c() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

/// Serves `router` until `signal` resolves, then triggers `shutdown`,
/// stops accepting, and waits up to `drain` for open requests. Returns
/// `Ok(false)` when the drain timed out with requests still open.
pub async fn serve(
    listener: tokio::net::TcpListener,
    router: Router,
    shutdown: Shutdown,
    signal: impl Future<Output = ()> + Send + 'static,
    drain: Duration,
) -> std::io::Result<bool> {
    let started = shutdown.clone();
    let server = axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        signal.await;
        started.trigger();
    })
    .into_future();
    let deadline = async {
        shutdown.wait().await;
        tokio::time::sleep(drain).await;
    };
    tokio::select! {
        served = server => served.map(|()| true),
        () = deadline => Ok(false),
    }
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;
    use std::time::Instant;

    use axum::response::sse::{Event, Sse};
    use axum::routing::get;

    use super::*;

    async fn start(
        router: Router,
        drain: Duration,
    ) -> (
        SocketAddr,
        Shutdown,
        tokio::sync::oneshot::Sender<()>,
        tokio::task::JoinHandle<std::io::Result<bool>>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let shutdown = Shutdown::default();
        let (send, receive) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(serve(
            listener,
            router,
            shutdown.clone(),
            async move {
                let _ = receive.await;
            },
            drain,
        ));
        (address, shutdown, send, server)
    }

    #[tokio::test]
    async fn a_request_in_flight_finishes_after_the_signal() {
        let router = Router::new().route(
            "/slow",
            get(|| async {
                tokio::time::sleep(Duration::from_millis(400)).await;
                "done"
            }),
        );
        let (address, shutdown, signal, server) = start(router, Duration::from_secs(5)).await;
        let request =
            tokio::spawn(async move { reqwest::get(format!("http://{address}/slow")).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        signal.send(()).unwrap();
        let response = request.await.unwrap().unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().await.unwrap(), "done");
        assert!(server.await.unwrap().unwrap(), "drained cleanly");
        assert!(shutdown.started());
    }

    #[tokio::test]
    async fn an_event_stream_that_watches_the_shutdown_ends_and_does_not_hold_the_drain() {
        let router = Router::new().route(
            "/events",
            get(
                |axum::extract::State(shutdown): axum::extract::State<Shutdown>| async move {
                    let endless = futures_util::stream::repeat_with(|| {
                        Ok::<_, Infallible>(Event::default().comment("tick"))
                    })
                    .then(|event| async move {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        event
                    });
                    Sse::new(shutdown.until(endless))
                },
            ),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let shutdown = Shutdown::default();
        let (signal, receive) = tokio::sync::oneshot::channel::<()>();
        let server = tokio::spawn(serve(
            listener,
            router.with_state(shutdown.clone()),
            shutdown.clone(),
            async move {
                let _ = receive.await;
            },
            Duration::from_secs(30),
        ));
        let response = reqwest::get(format!("http://{address}/events"))
            .await
            .unwrap();
        let begun = Instant::now();
        signal.send(()).unwrap();
        // The body ends because the stream observed the shutdown.
        let _ = response.bytes().await.unwrap();
        assert!(server.await.unwrap().unwrap(), "drained cleanly");
        assert!(begun.elapsed() < Duration::from_secs(10));
    }

    #[tokio::test]
    async fn a_request_that_never_ends_is_cut_off_after_the_drain() {
        let router = Router::new().route(
            "/forever",
            get(|| async {
                std::future::pending::<()>().await;
                "never"
            }),
        );
        let (address, _shutdown, signal, server) = start(router, Duration::from_millis(300)).await;
        let _request =
            tokio::spawn(async move { reqwest::get(format!("http://{address}/forever")).await });
        tokio::time::sleep(Duration::from_millis(100)).await;
        let begun = Instant::now();
        signal.send(()).unwrap();
        assert!(!server.await.unwrap().unwrap(), "the drain timed out");
        assert!(begun.elapsed() < Duration::from_secs(5));
    }
}

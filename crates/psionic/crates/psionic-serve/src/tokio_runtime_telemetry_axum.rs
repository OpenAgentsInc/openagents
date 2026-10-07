use std::{convert::Infallible, io, net::SocketAddr, pin::pin};

use axum::{Router, body::Body, extract::Request, response::Response};
use futures_util::FutureExt;
use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder,
    service::TowerToHyperService,
};
use psionic_observe::spawn_runtime_task;
use tokio::{net::TcpListener, sync::watch};
use tower::ServiceExt as _;
use tower_service::Service;

pub async fn serve_with_runtime_telemetry(listener: TcpListener, router: Router) -> io::Result<()> {
    let make_service = router.into_make_service_with_connect_info::<SocketAddr>();
    serve_tcp_listener_with_runtime_telemetry(listener, make_service).await
}

async fn serve_tcp_listener_with_runtime_telemetry<M, S>(
    listener: TcpListener,
    mut make_service: M,
) -> io::Result<()>
where
    M: Service<SocketAddr, Error = Infallible, Response = S> + Send + 'static,
    M::Future: Send,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    let (signal_tx, _signal_rx) = watch::channel(());
    let (_close_tx, close_rx) = watch::channel(());
    loop {
        let (io, remote_addr) = listener.accept().await?;
        handle_connection(&mut make_service, &signal_tx, &close_rx, io, remote_addr).await;
    }
}

async fn handle_connection<M, S>(
    make_service: &mut M,
    signal_tx: &watch::Sender<()>,
    close_rx: &watch::Receiver<()>,
    io: tokio::net::TcpStream,
    remote_addr: SocketAddr,
) where
    M: Service<SocketAddr, Error = Infallible, Response = S> + Send + 'static,
    M::Future: Send,
    S: Service<Request, Response = Response, Error = Infallible> + Clone + Send + 'static,
    S::Future: Send,
{
    let io = TokioIo::new(io);

    make_service
        .ready()
        .await
        .unwrap_or_else(|error| match error {});

    let tower_service = make_service
        .call(remote_addr)
        .await
        .unwrap_or_else(|error| match error {})
        .map_request(|request: Request<Incoming>| request.map(Body::new));

    let hyper_service = TowerToHyperService::new(tower_service);
    let signal_tx = signal_tx.clone();
    let close_rx = close_rx.clone();

    let _join = spawn_runtime_task(async move {
        let mut builder = Builder::new(TokioExecutor::new());
        builder.http2().enable_connect_protocol();

        let mut connection = pin!(builder.serve_connection_with_upgrades(io, hyper_service));
        let mut signal_closed = pin!(signal_tx.closed().fuse());

        loop {
            tokio::select! {
                result = connection.as_mut() => {
                    let _ = result;
                    break;
                }
                _ = &mut signal_closed => {
                    connection.as_mut().graceful_shutdown();
                }
            }
        }

        drop(close_rx);
    });
}

#[cfg(test)]
mod tests {
    use super::serve_with_runtime_telemetry;
    use axum::{Router, routing::get};
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn serve_with_runtime_telemetry_handles_basic_http_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let router = Router::new().route("/health", get(|| async { "ok" }));
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let address = listener.local_addr()?;
        let task = tokio::spawn(async move {
            let _ = serve_with_runtime_telemetry(listener, router).await;
        });
        let response = reqwest::get(format!("http://{address}/health")).await?;
        let body = response.text().await?;
        task.abort();

        assert_eq!(body, "ok");
        Ok(())
    }
}

#![cfg(unix)]

//! Control socket integration tests.

use autopilot::daemon::config::DaemonConfig;
use autopilot::daemon::control::{ControlClient, ControlServer};
use autopilot::daemon::supervisor::{DaemonMetrics, SharedMetrics, WorkerSupervisor};
use std::sync::{Arc, RwLock};
use tempfile::tempdir;
use tokio::sync::{mpsc, Mutex};
use tokio::time::{sleep, Duration};

#[tokio::test]
async fn stop_worker_via_control_socket() {
    let temp = tempdir().expect("tempdir");
    let socket_path = temp.path().join("autopilotd.sock");

    let mut config = DaemonConfig::default();
    config.socket_path = socket_path.clone();
    config.working_dir = temp.path().to_path_buf();

    let supervisor = Arc::new(Mutex::new(WorkerSupervisor::new(config)));
    let shared_metrics: SharedMetrics = Arc::new(RwLock::new(DaemonMetrics::default()));
    let (shutdown_tx, _shutdown_rx) = mpsc::channel(1);

    let server = ControlServer::new(&socket_path);
    let server_task = tokio::spawn(async move {
        server
            .run(supervisor, shared_metrics, shutdown_tx)
            .await
            .expect("control server");
    });

    for _ in 0..50 {
        if socket_path.exists() {
            break;
        }
        sleep(Duration::from_millis(10)).await;
    }

    assert!(socket_path.exists(), "control socket should exist");

    let client = ControlClient::new(&socket_path);
    client.stop_worker().await.expect("stop worker");

    server_task.abort();
    let _ = server_task.await;
}

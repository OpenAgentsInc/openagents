//! Autopilot Container Durable Object
//!
//! Manages the lifecycle of an autopilot container and proxies requests to it.
//! Each user gets their own DO instance (keyed by user_id).

use std::convert::TryInto;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;
use worker::*;

/// Autopilot Container Durable Object
///
/// Manages a container that runs the autopilot HTTP service.
/// Handles container startup, health checks, and request forwarding.
#[durable_object]
pub struct AutopilotContainer {
    state: State,
    #[allow(dead_code)]
    env: Env,
    ready: Arc<AtomicBool>,
}

impl DurableObject for AutopilotContainer {
    fn new(state: State, env: Env) -> Self {
        // Get container from state
        let container = match state.container() {
            Some(c) => c,
            None => {
                console_log!("Warning: No container available in state");
                return Self {
                    state,
                    env,
                    ready: Arc::new(AtomicBool::new(false)),
                };
            }
        };

        // Start container if not running
        if !container.running() {
            let mut opts = ContainerStartupOptions::new();
            opts.enable_internet(true);

            if let Err(e) = container.start(Some(opts)) {
                console_log!("Failed to start container: {:?}", e);
            } else {
                console_log!("Container started");
            }
        }

        // Health check polling
        let ready = Arc::new(AtomicBool::new(false));
        let ready_clone = Arc::clone(&ready);

        // Spawn health check task
        spawn_local(async move {
            for attempt in 0..30 {
                // Try for 30 seconds
                match container.get_tcp_port(8080) {
                    Ok(fetcher) => {
                        match fetcher
                            .fetch("http://container.internal/ping", None)
                            .await
                        {
                            Ok(resp) => {
                                // With http feature, resp is http::Response<Body>
                                // Use .status().is_success() for HTTP response
                                if resp.status().is_success() {
                                    console_log!(
                                        "Container ready after {} attempts",
                                        attempt + 1
                                    );
                                    ready_clone.store(true, Ordering::Release);
                                    return;
                                }
                            }
                            Err(e) => {
                                if attempt % 5 == 0 {
                                    console_log!("Health check attempt {}: {:?}", attempt + 1, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        if attempt % 5 == 0 {
                            console_log!("Get TCP port attempt {}: {:?}", attempt + 1, e);
                        }
                    }
                }

                Delay::from(Duration::from_secs(1)).await;
            }

            console_log!("Container failed to become ready after 30 attempts");
        });

        Self { state, env, ready }
    }

    async fn fetch(&self, req: Request) -> Result<Response> {
        let url = req.url()?;
        let path = url.path();

        // Handle status request locally
        if path == "/status" {
            return Response::from_json(&serde_json::json!({
                "ready": self.ready.load(Ordering::Acquire),
                "has_container": self.state.container().is_some(),
            }));
        }

        // Wait for container to be ready (with timeout)
        let mut attempts = 0;
        while !self.ready.load(Ordering::Acquire) {
            if attempts >= 100 {
                // 10 second timeout
                return Response::error("Container not ready", 503);
            }
            Delay::from(Duration::from_millis(100)).await;
            attempts += 1;
        }

        // Get container and forward request
        let container = match self.state.container() {
            Some(c) => c,
            None => return Response::error("No container available", 500),
        };

        let fetcher = container.get_tcp_port(8080)?;

        // Build the internal URL for the container
        let internal_url = format!("http://container.internal{}", path);

        // fetch() returns http::Response<Body> when http feature is enabled
        // Convert to worker::Response using try_into()
        let http_resp = fetcher.fetch(&internal_url, None).await?;
        let worker_resp: Response = http_resp.try_into()?;
        Ok(worker_resp)
    }
}

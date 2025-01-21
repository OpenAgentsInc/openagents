use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use bytes::Bytes;
use futures::{sink::SinkExt, stream::StreamExt};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{broadcast, mpsc};
use tracing::{error, info};

use super::solver::SolverService;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Clone)]
pub struct SolverWsState {
    solver_service: Arc<SolverService>,
    connections: Arc<tokio::sync::RwLock<HashMap<String, mpsc::Sender<Message>>>>,
    update_tx: broadcast::Sender<SolverUpdate>,
}

use crate::server::services::solver::ws::types::{SolverStage, SolverUpdate};

impl SolverWsState {
    pub fn new(solver_service: Arc<SolverService>) -> Self {
        let (update_tx, _) = broadcast::channel(100);
        Self {
            solver_service,
            connections: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
            update_tx,
        }
    }

    pub async fn broadcast_update(&self, update: SolverUpdate) {
        let conns = self.connections.read().await;
        
        match &update {
            SolverUpdate::Progress { stage, message, data } => {
                // Send stage update (progress bar)
                let progress_html = format!(
                    r#"<div id="solver-progress" hx-swap-oob="true">
                        <div class="progress-bar" style="width: {}%">
                            {}
                        </div>
                    </div>
                    <div id="solver-stage" hx-swap-oob="true">
                        Stage {}: {}
                    </div>"#,
                    match stage {
                        SolverStage::Init => 0,
                        SolverStage::Repomap => 25,
                        SolverStage::Analysis => 50,
                        SolverStage::Solution => 75,
                        SolverStage::PR => 90,
                    },
                    message,
                    match stage {
                        SolverStage::Init => "1/5",
                        SolverStage::Repomap => "2/5",
                        SolverStage::Analysis => "3/5",
                        SolverStage::Solution => "4/5",
                        SolverStage::PR => "5/5",
                    },
                    message
                );

                // Send content updates if any
                if let Some(data) = data {
                    if let Some(files_list) = data.get("files_list") {
                        let content_html = format!(
                            r#"<div id="solver-files-list" hx-swap-oob="innerHtml">
                                <pre class="content-text">{}</pre>
                            </div>"#,
                            files_list
                        );
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(content_html.clone().into())).await;
                        }
                    }
                    if let Some(files_reasoning) = data.get("files_reasoning") {
                        let reasoning_html = format!(
                            r#"<div id="solver-files-reasoning" hx-swap-oob="innerHtml">
                                <pre class="content-text">{}</pre>
                            </div>"#,
                            files_reasoning
                        );
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(reasoning_html.clone().into())).await;
                        }
                    }
                    if let Some(solution_text) = data.get("solution_text") {
                        let solution_html = format!(
                            r#"<div id="solver-solution" hx-swap-oob="innerHtml">
                                <pre class="content-text">{}</pre>
                            </div>"#,
                            solution_text
                        );
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(solution_html.clone().into())).await;
                        }
                    }
                    if let Some(solution_reasoning) = data.get("solution_reasoning") {
                        let reasoning_html = format!(
                            r#"<div id="solver-solution-reasoning" hx-swap-oob="innerHtml">
                                <pre class="content-text">{}</pre>
                            </div>"#,
                            solution_reasoning
                        );
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(reasoning_html.clone().into())).await;
                        }
                    }
                }

                // Send progress bar update
                for tx in conns.values() {
                    let _ = tx.send(Message::Text(progress_html.clone().into())).await;
                }
            }
            SolverUpdate::Complete { result } => {
                let html = format!(
                    r#"<div id="solver-progress" hx-swap-oob="true">
                        <div class="progress-bar" style="width: 100%">
                            Complete
                        </div>
                    </div>
                    <div id="solver-status" hx-swap-oob="true">
                        Solution complete
                    </div>
                    <div id="solver-final-result" hx-swap-oob="true">
                        <pre class="solution-text">{}</pre>
                    </div>"#,
                    result["solution"]
                );
                for tx in conns.values() {
                    let _ = tx.send(Message::Text(html.clone().into())).await;
                }
            }
            SolverUpdate::Error { message, details } => {
                let html = format!(
                    r#"<div id="solver-error" hx-swap-oob="true">
                        <div class="error">
                            Error: {message}
                            {}</div>
                    </div>"#,
                    details
                        .as_ref()
                        .map(|d| format!("<pre>{d}</pre>"))
                        .unwrap_or_default()
                );
                for tx in conns.values() {
                    let _ = tx.send(Message::Text(html.clone().into())).await;
                }
            }
        }
    }
}

// ... rest of the file stays the same ...
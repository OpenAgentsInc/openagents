use super::MessageHandler;
use crate::server::services::solver::SolverService;
use crate::server::ws::{
    transport::WebSocketState,
    types::{CodeJsonChange, SolverJsonMessage, SolverJsonStatus},
};
use async_trait::async_trait;
use axum::extract::ws::Message;
use chrono::Utc;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::mpsc::{Sender, UnboundedSender};
use tokio::sync::Mutex;
use tracing::{error, info};
use uuid::Uuid;

pub struct SolverJsonHandler {
    ws_state: Arc<WebSocketState>,
    solver_service: Arc<Mutex<SolverService>>,
}

impl SolverJsonHandler {
    pub fn new(ws_state: Arc<WebSocketState>, solver_service: Arc<Mutex<SolverService>>) -> Self {
        Self {
            ws_state,
            solver_service,
        }
    }

    pub async fn handle_message(
        &self,
        message: SolverJsonMessage,
        ws_state: Arc<WebSocketState>,
        conn_id: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        match message {
            SolverJsonMessage::SolveDemoRepo { .. } => {
                info!("Starting demo repo solver process");
                let tx = ws_state.get_tx(&conn_id).await?;
                let unbounded_tx = create_unbounded_sender(tx).await;
                let mut solver = self.solver_service.lock().await;
                if let Err(e) = solver.solve_demo_repo(unbounded_tx).await {
                    error!("Error in demo repo solver process: {}", e);
                    ws_state
                        .send_to(
                            &conn_id,
                            &serde_json::to_string(&SolverJsonMessage::Error {
                                message: format!("Error in demo repo solver process: {}", e),
                                timestamp: Utc::now().to_rfc3339(),
                            })?,
                        )
                        .await?;
                }
            }
            SolverJsonMessage::SolveRepo {
                repository,
                issue_number,
                ..
            } => {
                info!(
                    "Starting repo solver process for {}, issue #{}",
                    repository, issue_number
                );
                let tx = ws_state.get_tx(&conn_id).await?;
                let unbounded_tx = create_unbounded_sender(tx).await;
                let mut solver = self.solver_service.lock().await;
                if let Err(e) = solver
                    .solve_repo(unbounded_tx, repository, issue_number)
                    .await
                {
                    error!("Error in repo solver process: {}", e);
                    ws_state
                        .send_to(
                            &conn_id,
                            &serde_json::to_string(&SolverJsonMessage::Error {
                                message: format!("Error in repo solver process: {}", e),
                                timestamp: Utc::now().to_rfc3339(),
                            })?,
                        )
                        .await?;
                }
            }
            _ => {
                error!("Unknown solver message type: {:?}", message);
            }
        }
        Ok(())
    }

    pub async fn emit_state_update(
        &self,
        conn_id: &str,
        status: SolverJsonStatus,
        current_file: Option<String>,
        progress: Option<f32>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.handle_message(
            SolverJsonMessage::StateUpdate {
                status,
                current_file,
                progress,
                timestamp: Utc::now().to_rfc3339(),
            },
            self.ws_state.clone(),
            conn_id.to_string(),
        )
        .await
    }

    pub async fn emit_file_analysis(
        &self,
        conn_id: &str,
        file_path: String,
        analysis: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.handle_message(
            SolverJsonMessage::FileAnalysis {
                file_path,
                analysis,
                timestamp: Utc::now().to_rfc3339(),
            },
            self.ws_state.clone(),
            conn_id.to_string(),
        )
        .await
    }

    pub async fn emit_change_generated(
        &self,
        conn_id: &str,
        file_path: String,
        search: String,
        replace: String,
        description: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        let change = CodeJsonChange {
            id: Uuid::new_v4().to_string(),
            search,
            replace,
            description,
        };

        self.handle_message(
            SolverJsonMessage::ChangeGenerated {
                file_path,
                changes: vec![change],
                timestamp: Utc::now().to_rfc3339(),
            },
            self.ws_state.clone(),
            conn_id.to_string(),
        )
        .await
    }

    pub async fn emit_change_applied(
        &self,
        conn_id: &str,
        file_path: String,
        success: bool,
        error: Option<String>,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.handle_message(
            SolverJsonMessage::ChangeApplied {
                file_path,
                success,
                error,
                timestamp: Utc::now().to_rfc3339(),
            },
            self.ws_state.clone(),
            conn_id.to_string(),
        )
        .await
    }

    pub async fn emit_error(
        &self,
        conn_id: &str,
        message: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        self.handle_message(
            SolverJsonMessage::Error {
                message,
                timestamp: Utc::now().to_rfc3339(),
            },
            self.ws_state.clone(),
            conn_id.to_string(),
        )
        .await
    }
}

#[async_trait]
impl MessageHandler for SolverJsonHandler {
    type Message = SolverJsonMessage;

    async fn handle_message(
        &self,
        msg: Self::Message,
        conn_id: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Handling solver JSON message: {:?}", msg);
        self.handle_message(msg, self.ws_state.clone(), conn_id)
            .await
    }

    async fn broadcast(&self, msg: Self::Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        let json = serde_json::to_string(&msg)?;
        self.ws_state.broadcast(&json).await
    }
}

async fn create_unbounded_sender(tx: Sender<String>) -> UnboundedSender<Message> {
    let (unbounded_tx, mut unbounded_rx) = tokio::sync::mpsc::unbounded_channel();

    tokio::spawn(async move {
        while let Some(msg) = unbounded_rx.recv().await {
            if let Message::Text(text) = msg {
                if tx.send(text).await.is_err() {
                    break;
                }
            }
        }
    });

    unbounded_tx
}

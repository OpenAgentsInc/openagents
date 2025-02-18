use super::MessageHandler;
use crate::server::services::solver::SolverService;
use crate::server::ws::{transport::WebSocketState, types::SolverMessage};
use async_trait::async_trait;
use chrono::Utc;
use std::error::Error;
use std::sync::Arc;
use tracing::{error, info};

pub struct SolverHandler {
    ws_state: Arc<WebSocketState>,
    solver_service: Arc<SolverService>,
}

impl SolverHandler {
    pub fn new(ws_state: Arc<WebSocketState>, solver_service: Arc<SolverService>) -> Self {
        Self {
            ws_state,
            solver_service,
        }
    }

    pub async fn handle_solver_event(&self, event: SolverMessage, conn_id: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Add timestamp if not present
        let event = match event {
            SolverMessage::StateUpdate { status, current_file, progress, timestamp: _ } => {
                SolverMessage::StateUpdate {
                    status,
                    current_file,
                    progress,
                    timestamp: Utc::now().to_rfc3339(),
                }
            }
            SolverMessage::FileAnalysis { file_path, analysis, timestamp: _ } => {
                SolverMessage::FileAnalysis {
                    file_path,
                    analysis,
                    timestamp: Utc::now().to_rfc3339(),
                }
            }
            SolverMessage::ChangeGenerated { file_path, changes, timestamp: _ } => {
                SolverMessage::ChangeGenerated {
                    file_path,
                    changes,
                    timestamp: Utc::now().to_rfc3339(),
                }
            }
            SolverMessage::ChangeApplied { file_path, success, error, timestamp: _ } => {
                SolverMessage::ChangeApplied {
                    file_path,
                    success,
                    error,
                    timestamp: Utc::now().to_rfc3339(),
                }
            }
            SolverMessage::Error { message, timestamp: _ } => {
                SolverMessage::Error {
                    message,
                    timestamp: Utc::now().to_rfc3339(),
                }
            }
        };

        // Convert to JSON and send
        let json = serde_json::to_string(&event)?;
        self.ws_state.send_to(conn_id, &json).await?;
        Ok(())
    }
}

#[async_trait]
impl MessageHandler for SolverHandler {
    type Message = SolverMessage;

    async fn handle_message(
        &self,
        msg: Self::Message,
        conn_id: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Handling solver message: {:?}", msg);
        self.handle_solver_event(msg, &conn_id).await
    }

    async fn broadcast(&self, msg: Self::Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        let json = serde_json::to_string(&msg)?;
        self.ws_state.broadcast(&json).await
    }
}

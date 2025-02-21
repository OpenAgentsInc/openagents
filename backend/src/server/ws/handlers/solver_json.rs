use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::mpsc;
use tracing::{error, info};
use uuid::Uuid;

use crate::server::{
    config::AppState,
    services::solver::types::{FileAnalysis, SolverChange},
    ws::transport::WebSocketState,
};

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum SolverMessage {
    Subscribe {
        scope: String,
        solver_id: Option<Uuid>,
    },
    ApproveChange {
        solver_id: Uuid,
        change_id: Uuid,
    },
    RejectChange {
        solver_id: Uuid,
        change_id: Uuid,
    },
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum SolverResponse {
    Subscribed {
        scope: String,
    },
    StateUpdate {
        solver_id: Uuid,
        state: String,
    },
    FileAnalysis {
        solver_id: Uuid,
        analysis: FileAnalysis,
    },
    ChangeGenerated {
        solver_id: Uuid,
        change: SolverChange,
    },
    ChangeApplied {
        solver_id: Uuid,
        change_id: Uuid,
    },
    Error {
        message: String,
    },
}

pub struct SolverJsonHandler {
    tx: mpsc::Sender<String>,
    state: AppState,
    user_id: String,
    ws_state: Arc<WebSocketState>,
}

impl SolverJsonHandler {
    pub fn new(tx: mpsc::Sender<String>, state: AppState, user_id: String, ws_state: Arc<WebSocketState>) -> Self {
        Self {
            tx,
            state,
            user_id,
            ws_state,
        }
    }

    pub async fn handle_message(
        &mut self,
        msg: SolverMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match msg {
            SolverMessage::Subscribe { scope, .. } => {
                info!("Processing solver subscribe message for scope: {}", scope);
                let response = SolverResponse::Subscribed { scope };
                let msg = serde_json::to_string(&response)?;
                self.tx.send(msg).await?;
            }
            SolverMessage::ApproveChange {
                solver_id,
                change_id,
            } => {
                info!(
                    "Processing approve change: solver={}, change={}",
                    solver_id, change_id
                );

                // Get solver service
                let solver_service = self.state.solver_service.clone();

                // Approve change
                solver_service.approve_change(solver_id, change_id).await?;

                // Send response
                let response = SolverResponse::ChangeApplied {
                    solver_id,
                    change_id,
                };
                let msg = serde_json::to_string(&response)?;
                self.tx.send(msg).await?;

                // Check if all changes are reviewed
                if solver_service.check_all_changes_reviewed(solver_id).await? {
                    // Send state update
                    let response = SolverResponse::StateUpdate {
                        solver_id,
                        state: "completed".to_string(),
                    };
                    let msg = serde_json::to_string(&response)?;
                    self.tx.send(msg).await?;
                }
            }
            SolverMessage::RejectChange {
                solver_id,
                change_id,
            } => {
                info!(
                    "Processing reject change: solver={}, change={}",
                    solver_id, change_id
                );

                // Get solver service
                let solver_service = self.state.solver_service.clone();

                // Reject change
                solver_service.reject_change(solver_id, change_id).await?;

                // Send response
                let response = SolverResponse::StateUpdate {
                    solver_id,
                    state: "changes_needed".to_string(),
                };
                let msg = serde_json::to_string(&response)?;
                self.tx.send(msg).await?;
            }
        }

        Ok(())
    }

    pub async fn emit_state_update(
        &self,
        conn_id: &str,
        solver_id: Uuid,
        state: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = SolverResponse::StateUpdate {
            solver_id,
            state: state.to_string(),
        };
        let msg = serde_json::to_string(&response)?;
        self.ws_state.send_to(conn_id, &msg).await?;
        Ok(())
    }

    pub async fn emit_file_analysis(
        &self,
        conn_id: &str,
        solver_id: Uuid,
        analysis: FileAnalysis,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = SolverResponse::FileAnalysis {
            solver_id,
            analysis,
        };
        let msg = serde_json::to_string(&response)?;
        self.ws_state.send_to(conn_id, &msg).await?;
        Ok(())
    }

    pub async fn emit_change_generated(
        &self,
        conn_id: &str,
        solver_id: Uuid,
        change: SolverChange,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = SolverResponse::ChangeGenerated {
            solver_id,
            change,
        };
        let msg = serde_json::to_string(&response)?;
        self.ws_state.send_to(conn_id, &msg).await?;
        Ok(())
    }

    pub async fn emit_change_applied(
        &self,
        conn_id: &str,
        solver_id: Uuid,
        change_id: Uuid,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = SolverResponse::ChangeApplied {
            solver_id,
            change_id,
        };
        let msg = serde_json::to_string(&response)?;
        self.ws_state.send_to(conn_id, &msg).await?;
        Ok(())
    }

    pub async fn emit_error(
        &self,
        conn_id: &str,
        error: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let response = SolverResponse::Error {
            message: error.to_string(),
        };
        let msg = serde_json::to_string(&response)?;
        self.ws_state.send_to(conn_id, &msg).await?;
        Ok(())
    }
}

pub async fn create_unbounded_sender() -> mpsc::UnboundedSender<String> {
    let (tx, _rx) = mpsc::unbounded_channel();
    tx
}
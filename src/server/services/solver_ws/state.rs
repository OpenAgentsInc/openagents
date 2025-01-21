use std::{
    collections::HashMap,
    sync::Arc,
};
use axum::extract::ws::Message;
use tokio::sync::{broadcast, mpsc, RwLock};
use crate::server::services::{
    solver::SolverService,
    solver::ws::types::{SolverStage, SolverUpdate},
};

#[derive(Clone)]
pub struct SolverWsState {
    pub(crate) solver_service: Arc<SolverService>,
    pub(crate) connections: Arc<RwLock<HashMap<String, mpsc::Sender<Message>>>>,
    pub(crate) update_tx: broadcast::Sender<SolverUpdate>,
}

impl SolverWsState {
    pub fn new(solver_service: Arc<SolverService>) -> Self {
        let (update_tx, _) = broadcast::channel(100);
        Self {
            solver_service,
            connections: Arc::new(RwLock::new(HashMap::new())),
            update_tx,
        }
    }

    pub async fn broadcast_update(&self, update: SolverUpdate) {
        let conns = self.connections.read().await;
        
        match &update {
            SolverUpdate::Progress { stage, message, data } => {
                // Send stage update (progress bar)
                let progress_html = super::html::render_progress_bar(stage, message);

                // Send content updates if any
                if let Some(data) = data {
                    if let Some(files_list) = data.get("files_list") {
                        let content_html = super::html::render_files_list(files_list);
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(content_html.clone().into())).await;
                        }
                    }
                    if let Some(files_reasoning) = data.get("files_reasoning") {
                        let reasoning_html = super::html::render_files_reasoning(files_reasoning);
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(reasoning_html.clone().into())).await;
                        }
                    }
                    if let Some(solution_text) = data.get("solution_text") {
                        let solution_html = super::html::render_solution(solution_text);
                        for tx in conns.values() {
                            let _ = tx.send(Message::Text(solution_html.clone().into())).await;
                        }
                    }
                    if let Some(solution_reasoning) = data.get("solution_reasoning") {
                        let reasoning_html = super::html::render_solution_reasoning(solution_reasoning);
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
                let html = super::html::render_complete(result);
                for tx in conns.values() {
                    let _ = tx.send(Message::Text(html.clone().into())).await;
                }
            }
            SolverUpdate::Error { message, details } => {
                let html = super::html::render_error(message, details);
                for tx in conns.values() {
                    let _ = tx.send(Message::Text(html.clone().into())).await;
                }
            }
        }
    }
}
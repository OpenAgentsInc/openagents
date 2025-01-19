use axum::{
    extract::{Form, State},
    response::{Html, IntoResponse},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub mod repomap;
pub mod solver;

pub use repomap::RepomapService;
pub use solver::SolverService;

#[derive(Debug, Serialize, Deserialize)]
pub struct SolverRequest {
    pub issue_url: String,
}

pub async fn handle_solver(
    State(service): State<Arc<SolverService>>,
    Form(req): Form<SolverRequest>,
) -> impl IntoResponse {
    match service.solve_issue(req.issue_url).await {
        Ok(response) => Html(response.solution).into_response(),
        Err(e) => {
            eprintln!("Error solving issue: {}", e);
            Html(format!("Error: {}", e)).into_response()
        }
    }
}

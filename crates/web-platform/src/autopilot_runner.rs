// Autopilot job runner - manages autonomous code execution jobs

use actix_web::{web, HttpResponse, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, error};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

// In-memory job store (use database in production)
type JobStore = Arc<RwLock<HashMap<String, AutopilotJob>>>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutopilotJob {
    pub id: String,
    pub user_id: String,
    pub repo_url: String,
    pub task: String,
    pub status: JobStatus,
    pub created_at: String,
    pub updated_at: String,
    pub result: Option<JobResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobResult {
    pub pr_url: Option<String>,
    pub files_modified: u32,
    pub tests_passed: bool,
    pub execution_time_seconds: u64,
    pub credits_used: u64,
}

#[derive(Debug, Deserialize)]
pub struct StartJobRequest {
    pub repo_url: String,
    pub task: String,
    pub user_id: String, // In production, extract from session
}

#[derive(Debug, Serialize)]
pub struct StartJobResponse {
    pub job_id: String,
    pub status: String,
    pub message: String,
}

pub async fn start_job(
    req: web::Json<StartJobRequest>,
) -> Result<HttpResponse> {
    info!("Starting autopilot job for repo: {}", req.repo_url);

    // Generate job ID
    let job_id = format!("job_{}", rand::random::<u64>());
    let now = chrono::Utc::now().to_rfc3339();

    let _job = AutopilotJob {
        id: job_id.clone(),
        user_id: req.user_id.clone(),
        repo_url: req.repo_url.clone(),
        task: req.task.clone(),
        status: JobStatus::Queued,
        created_at: now.clone(),
        updated_at: now,
        result: None,
    };

    // In production:
    // 1. Check user has sufficient credits
    // 2. Clone repository
    // 3. Spawn autopilot task
    // 4. Stream progress to client via WebSocket
    // 5. Deduct credits based on usage
    // 6. Create PR with results

    info!("Created job {} for user {}", job_id, req.user_id);

    // For now, return success response
    Ok(HttpResponse::Created().json(StartJobResponse {
        job_id,
        status: "queued".to_string(),
        message: "Autopilot job queued. In production, this would spawn a real execution.".to_string(),
    }))
}

#[derive(Debug, Serialize)]
pub struct JobStatusResponse {
    pub job: AutopilotJob,
}

pub async fn job_status(
    job_id: web::Path<String>,
) -> Result<HttpResponse> {
    info!("Checking status for job: {}", job_id);

    // In production, fetch from database
    // For now, return mock response
    let mock_job = AutopilotJob {
        id: job_id.to_string(),
        user_id: "user_123".to_string(),
        repo_url: "https://github.com/example/repo".to_string(),
        task: "Fix all type errors".to_string(),
        status: JobStatus::Running,
        created_at: chrono::Utc::now().to_rfc3339(),
        updated_at: chrono::Utc::now().to_rfc3339(),
        result: None,
    };

    Ok(HttpResponse::Ok().json(JobStatusResponse {
        job: mock_job,
    }))
}

#[derive(Debug, Serialize)]
pub struct CancelJobResponse {
    pub success: bool,
    pub message: String,
}

pub async fn cancel_job(
    job_id: web::Path<String>,
) -> Result<HttpResponse> {
    info!("Cancelling job: {}", job_id);

    // In production:
    // 1. Find running process
    // 2. Send SIGTERM
    // 3. Wait for graceful shutdown
    // 4. Update job status to cancelled
    // 5. Refund unused credits

    Ok(HttpResponse::Ok().json(CancelJobResponse {
        success: true,
        message: format!("Job {} cancelled successfully", job_id),
    }))
}

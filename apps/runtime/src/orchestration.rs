use std::sync::Arc;

use serde::Serialize;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    authority::{AuthorityError, RuntimeAuthority},
    projectors::{ProjectionPipeline, ProjectorError},
    types::{AppendRunEventRequest, ProjectionCheckpoint, RuntimeRun, StartRunRequest},
};

#[derive(Debug, Error)]
pub enum OrchestrationError {
    #[error("authority error: {0}")]
    Authority(#[from] AuthorityError),
    #[error("projector error: {0}")]
    Projector(#[from] ProjectorError),
    #[error("event type cannot be empty")]
    EmptyEventType,
    #[error("run not found: {0}")]
    RunNotFound(Uuid),
}

#[derive(Clone, Debug, Serialize)]
pub struct RuntimeReadiness {
    pub authority_ready: bool,
    pub projector_ready: bool,
}

impl RuntimeReadiness {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.authority_ready && self.projector_ready
    }
}

pub struct RuntimeOrchestrator {
    authority: Arc<dyn RuntimeAuthority>,
    projectors: Arc<dyn ProjectionPipeline>,
}

impl RuntimeOrchestrator {
    #[must_use]
    pub fn new(
        authority: Arc<dyn RuntimeAuthority>,
        projectors: Arc<dyn ProjectionPipeline>,
    ) -> Self {
        Self {
            authority,
            projectors,
        }
    }

    pub async fn start_run(
        &self,
        request: StartRunRequest,
    ) -> Result<RuntimeRun, OrchestrationError> {
        let run = self.authority.create_run(request).await?;
        let started_event = self
            .authority
            .append_event(
                run.id,
                "run.started".to_string(),
                serde_json::json!({"source": "runtime"}),
            )
            .await?;
        self.projectors
            .apply_run_event(run.id, &started_event)
            .await?;
        let refreshed = self
            .authority
            .get_run(run.id)
            .await?
            .ok_or(OrchestrationError::RunNotFound(run.id))?;
        Ok(refreshed)
    }

    pub async fn append_run_event(
        &self,
        run_id: Uuid,
        request: AppendRunEventRequest,
    ) -> Result<RuntimeRun, OrchestrationError> {
        let trimmed = request.event_type.trim();
        if trimmed.is_empty() {
            return Err(OrchestrationError::EmptyEventType);
        }

        let event = self
            .authority
            .append_event(run_id, trimmed.to_string(), request.payload)
            .await?;
        self.projectors.apply_run_event(run_id, &event).await?;
        let refreshed = self
            .authority
            .get_run(run_id)
            .await?
            .ok_or(OrchestrationError::RunNotFound(run_id))?;
        Ok(refreshed)
    }

    pub async fn get_run(&self, run_id: Uuid) -> Result<Option<RuntimeRun>, OrchestrationError> {
        Ok(self.authority.get_run(run_id).await?)
    }

    pub async fn checkpoint_for_run(
        &self,
        run_id: Uuid,
    ) -> Result<Option<ProjectionCheckpoint>, OrchestrationError> {
        Ok(self.projectors.checkpoint_for_run(run_id).await?)
    }

    #[must_use]
    pub fn readiness(&self) -> RuntimeReadiness {
        RuntimeReadiness {
            authority_ready: true,
            projector_ready: self.projectors.is_ready(),
        }
    }
}

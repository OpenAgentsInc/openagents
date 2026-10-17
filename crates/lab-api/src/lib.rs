//! Labs routes with injected authentication, kind admission, and durable storage.
//! Every read authorizes its space again and never starts work.
pub mod contract;
mod handlers;
pub use lab_kit::api::*;

use async_trait::async_trait;
use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use lab_kit::*;
use lab_store::{CheckedDraft, LabStore, Store};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Read,
    Edit,
    Delete,
}
#[derive(Clone, Debug)]
pub struct Access {
    pub account: String,
    pub space: SpaceRef,
    pub experimental: bool,
}
#[async_trait]
pub trait Authorizer: Send + Sync {
    async fn authorize(
        &self,
        headers: &HeaderMap,
        space: &str,
        action: Action,
    ) -> Result<Access, ApiError>;
}
#[derive(Clone, Copy)]
pub enum Gate {
    Cad,
    Experimental,
}
#[derive(Clone)]
pub struct Registration {
    pub kind: Arc<dyn Kind>,
    pub gate: Gate,
}
#[derive(Default)]
pub struct Registry(BTreeMap<String, Registration>);
impl Registry {
    pub fn register(&mut self, kind: Arc<dyn Kind>, gate: Gate) -> Result<(), ApiError> {
        let manifest = kind.manifest();
        if manifest.kind.is_empty() || self.0.contains_key(&manifest.kind) {
            return Err(ApiError::invalid("Choose a unique lab kind."));
        }
        self.0
            .insert(manifest.kind.clone(), Registration { kind, gate });
        Ok(())
    }
    fn get(&self, kind: &str, access: &Access) -> Result<Arc<dyn Kind>, ApiError> {
        let entry = self.0.get(kind).ok_or_else(ApiError::missing)?;
        if matches!(entry.gate, Gate::Experimental) && !access.experimental {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "invite_only",
                "This lab isn't available for this account yet.",
            ));
        }
        Ok(entry.kind.clone())
    }
}

/// Kind-specific tools translate only into these common, checked operations.
#[derive(Clone, Debug)]
pub enum Tool {
    Read {
        document: String,
    },
    Change {
        document: String,
        change: Change,
        summary: String,
    },
    Job {
        document: String,
        version: u64,
        want: Want,
    },
}
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceReference {
    pub space: String,
    pub version: Reference,
}
pub trait Kind: Send + Sync {
    fn manifest(&self) -> KindManifest;
    fn initial(
        &self,
        schema: &str,
        content: Value,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft>;
    fn change(
        &self,
        base: &Version,
        change: &Change,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft>;
    fn jobs(&self, version: &Version, want: &Want) -> Vec<JobSpec>;
    fn card(&self, version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card;
    fn tool(&self, _name: &str, _arguments: &Value) -> Result<Tool, ApiError> {
        Err(ApiError::invalid("This tool isn't supported."))
    }
    fn references(&self, _content: &Value) -> Result<Vec<ResourceReference>, Refusal> {
        Ok(vec![])
    }
}
pub struct KindAdapter<K> {
    tool: fn(&str, &Value) -> Result<Tool, ApiError>,
    marker: std::marker::PhantomData<K>,
}
impl<K> KindAdapter<K> {
    pub fn new(tool: fn(&str, &Value) -> Result<Tool, ApiError>) -> Self {
        Self {
            tool,
            marker: Default::default(),
        }
    }
}
impl<K: LabKind + Send + Sync> Kind for KindAdapter<K> {
    fn manifest(&self) -> KindManifest {
        K::manifest()
    }
    fn initial(
        &self,
        schema: &str,
        content: Value,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft> {
        CheckedDraft::initial::<K>(schema, content, summary, &Value::Null)
    }
    fn change(
        &self,
        base: &Version,
        change: &Change,
        summary: &str,
    ) -> lab_store::Result<CheckedDraft> {
        CheckedDraft::change::<K>(base, change, summary, &Value::Null)
    }
    fn jobs(&self, version: &Version, want: &Want) -> Vec<JobSpec> {
        K::jobs_for(version, want)
    }
    fn card(&self, version: &Version, jobs: &[JobState], artifacts: &[Artifact]) -> Card {
        K::card(version, jobs, artifacts)
    }
    fn tool(&self, name: &str, args: &Value) -> Result<Tool, ApiError> {
        (self.tool)(name, args)
    }
}

pub struct Api {
    pub backend: Arc<dyn lab_store::backend::Backend>,
    pub authorizer: Arc<dyn Authorizer>,
    pub registry: Registry,
}
impl Api {
    pub fn router(self: Arc<Self>) -> Router {
        handlers::routes().with_state(self)
    }
    async fn scoped(
        &self,
        headers: &HeaderMap,
        space: &str,
        action: Action,
    ) -> Result<(Access, Store), ApiError> {
        if space != "me"
            && !space
                .strip_prefix("project:")
                .is_some_and(|s| !s.is_empty() && s.len() <= 256)
        {
            return Err(ApiError::invalid("Choose your space or a project."));
        }
        let access = self.authorizer.authorize(headers, space, action).await?;
        // The authorizer derives this value from the credential and membership.
        let store = Store::new(self.backend.clone(), &access.space)?;
        Ok((access, store))
    }
    async fn document(
        &self,
        headers: &HeaderMap,
        space: &str,
        action: Action,
        id: &str,
    ) -> Result<(Access, Store, Arc<dyn Kind>), ApiError> {
        let (access, store) = self.scoped(headers, space, action).await?;
        let doc = store.document(id).await?;
        let kind = self.registry.get(&doc.kind, &access)?;
        Ok((access, store, kind))
    }
    async fn references(
        &self,
        headers: &HeaderMap,
        kind: &dyn Kind,
        content: &Value,
    ) -> Result<(), ApiError> {
        let references = kind.references(content).map_err(lab_store::Error::from)?;
        if references.len() > 100 {
            return Err(ApiError::invalid("This document has too many references."));
        }
        for r in references {
            let (access, store) = self.scoped(headers, &r.space, Action::Read).await?;
            let doc = store.document(&r.version.document).await?;
            self.registry.get(&doc.kind, &access)?;
            store
                .version(&r.version.document, r.version.version)
                .await?;
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}
impl ApiError {
    pub fn new(status: StatusCode, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.into(),
            message: message.into(),
        }
    }
    pub fn invalid(message: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }
    pub fn missing() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            "This lab item could not be found.",
        )
    }
}
impl From<lab_store::Error> for ApiError {
    fn from(error: lab_store::Error) -> Self {
        use lab_store::Error;
        match error {
            Error::NotFound => Self::missing(),
            Error::Conflict => Self::new(StatusCode::CONFLICT, "conflict", error.to_string()),
            Error::LeaseLost => Self::new(StatusCode::CONFLICT, "lease_lost", error.to_string()),
            Error::RateLimited => Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                error.to_string(),
            ),
            Error::Refused(r) => Self::new(StatusCode::UNPROCESSABLE_ENTITY, r.code, r.message),
            Error::Invalid(message) => Self::invalid(&message),
            Error::Corrupt | Error::Storage(_) => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Labs aren't available right now. Try again later.",
            ),
        }
    }
}
impl From<axum::extract::rejection::JsonRejection> for ApiError {
    fn from(error: axum::extract::rejection::JsonRejection) -> Self {
        Self::new(
            error.status(),
            "invalid_request",
            "Check the request's fields and size.",
        )
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut r = (
            self.status,
            axum::Json(ErrorBody {
                error: ErrorDetail {
                    code: self.code,
                    message: self.message,
                },
            }),
        )
            .into_response();
        if matches!(
            self.status,
            StatusCode::TOO_MANY_REQUESTS | StatusCode::SERVICE_UNAVAILABLE
        ) {
            r.headers_mut()
                .insert("Retry-After", axum::http::HeaderValue::from_static("60"));
        }
        r.headers_mut().insert(
            "Cache-Control",
            axum::http::HeaderValue::from_static("private, no-store"),
        );
        r
    }
}

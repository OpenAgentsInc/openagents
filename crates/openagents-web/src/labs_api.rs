//! The shared Labs API uses the site's account and project authority.
use crate::App;
use axum::{
    Router,
    http::{HeaderMap, StatusCode},
    response::Response,
};
use lab_api::{Access, Action, Api, ApiError, Authorizer, Gate, KindAdapter, Registry, Tool};
use lab_kit::{Change, SpaceRef, echo::Echo};
use serde_json::{Value, json};
use std::sync::Arc;

struct Accounts(App);
async fn refusal(response: Response) -> ApiError {
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 16384)
        .await
        .unwrap_or_default();
    let value: Value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    ApiError::new(
        status,
        value["error"]["code"].as_str().unwrap_or("unavailable"),
        value["error"]["message"]
            .as_str()
            .unwrap_or("Labs aren't available right now. Try again later."),
    )
}
#[async_trait::async_trait]
impl Authorizer for Accounts {
    async fn authorize(
        &self,
        headers: &HeaderMap,
        space: &str,
        action: Action,
    ) -> Result<Access, ApiError> {
        // Environment- and run-specific computer credentials cannot enter Labs.
        if headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .is_some_and(|h| h.starts_with("Bearer ecj_") || h.starts_with("Bearer scj_"))
        {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "forbidden",
                "This key can't use Labs.",
            ));
        }
        let id = crate::computers_api::identity(&self.0, headers).await;
        let id = match id {
            Ok(id) => id,
            Err(r) => return Err(refusal(r).await),
        };
        let target = if space == "me" {
            SpaceRef::Personal {
                account: id.account.clone(),
            }
        } else {
            let project = space
                .strip_prefix("project:")
                .ok_or_else(|| ApiError::invalid("Choose your space or a project."))?;
            let access = match crate::projects::api::access(&self.0, headers, project).await {
                Ok(a) => a,
                Err(r) => return Err(refusal(r).await),
            };
            let permission = match action {
                Action::Read => Ok(()),
                Action::Edit => access.edits(),
                Action::Delete => access.owns(),
            };
            if let Err(r) = permission {
                return Err(refusal(r).await);
            }
            SpaceRef::Project {
                project: access.project.id,
            }
        };
        Ok(Access {
            account: id.account,
            space: target,
            experimental: id.permitted,
        })
    }
}
fn echo_tool(name: &str, args: &Value) -> Result<Tool, ApiError> {
    if name != "echo_set" {
        return Err(ApiError::missing());
    }
    let document = args["document"]
        .as_str()
        .ok_or_else(|| ApiError::invalid("Choose a document."))?
        .into();
    let text = args["text"]
        .as_str()
        .ok_or_else(|| ApiError::invalid("Enter text."))?;
    Ok(Tool::Change {
        document,
        change: Change::Intent {
            intent: json!({"intent":"set","text":text}),
        },
        summary: "Updated text".into(),
    })
}

pub(crate) fn routes(app: &App) -> Router<App> {
    let backend = app
        .config
        .lab_backend
        .clone()
        .unwrap_or_else(|| Arc::new(Unavailable));
    let mut registry = Registry::default();
    // This small research kind qualifies the gated API before CAD integration.
    if app.config.lab_backend.is_some() {
        registry
            .register(
                Arc::new(KindAdapter::<Echo>::new(echo_tool)),
                Gate::Experimental,
            )
            .expect("Unique fixture kind");
    }
    Arc::new(Api {
        backend,
        authorizer: Arc::new(Accounts(app.clone())),
        registry,
    })
    .router()
    .with_state(())
}
struct Unavailable;
#[async_trait::async_trait]
impl lab_store::backend::Backend for Unavailable {
    async fn load(&self, _space: &str) -> lab_store::Result<(u64, Option<Vec<u8>>)> {
        Err(lab_store::Error::Corrupt)
    }
    async fn compare_and_swap(
        &self,
        _space: &str,
        _generation: u64,
        _bytes: Vec<u8>,
    ) -> lab_store::Result<bool> {
        Err(lab_store::Error::Corrupt)
    }
    async fn put(&self, _space: &str, _hash: &str, _bytes: Vec<u8>) -> lab_store::Result<()> {
        Err(lab_store::Error::Corrupt)
    }
    async fn get(
        &self,
        _space: &str,
        _hash: &str,
        _range: Option<std::ops::Range<u64>>,
    ) -> lab_store::Result<Vec<u8>> {
        Err(lab_store::Error::Corrupt)
    }
}

/// Credentials arrive from the deployment's secret environment; none are logged.
pub async fn backend_from_env() -> Result<Option<Arc<dyn lab_store::backend::Backend>>, &'static str>
{
    let Ok(dsn) = std::env::var("OPENAGENTS_WEB_LABS_DATABASE_URL") else {
        return Ok(None);
    };
    if dsn.trim().is_empty() {
        return Ok(None);
    }
    let bucket =
        std::env::var("OPENAGENTS_WEB_LABS_BUCKET").map_err(|_| "Configure the Labs bucket.")?;
    let pool = sqlx_postgres::PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(std::time::Duration::from_secs(10))
        .connect(&dsn)
        .await
        .map_err(|_| "Labs couldn't connect to its database.")?;
    let token = lab_store::backend::MetadataBearer::new()
        .map_err(|_| "Labs couldn't configure storage access.")?;
    let signing_account = std::env::var("OPENAGENTS_WEB_LABS_SIGNING_ACCOUNT")
        .ok()
        .filter(|s| !s.trim().is_empty());
    let objects = lab_store::backend::GcsObjects::new(&bucket, Arc::new(token), signing_account)
        .map_err(|_| "Labs couldn't configure its bucket.")?;
    let backend = lab_store::backend::CloudBackend::open(pool, Arc::new(objects))
        .await
        .map_err(|_| "Labs couldn't open its storage.")?;
    Ok(Some(Arc::new(backend)))
}

#[cfg(test)]
mod tests;

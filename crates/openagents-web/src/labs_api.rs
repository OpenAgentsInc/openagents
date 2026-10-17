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
pub(crate) async fn bind_account(app: &App, account: &str) -> lab_store::Result<()> {
    if let Some(backend) = &app.config.lab_backend {
        lab_store::Store::new(
            backend.clone(),
            &SpaceRef::Personal {
                account: account.into(),
            },
        )?
        .bind_alias(&crate::chat_store::account_owner(account))
        .await?;
    }
    Ok(())
}
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
    async fn enroll_worker(
        &self,
        headers: &HeaderMap,
        request: &lab_jobs::protocol::Enroll,
    ) -> Result<lab_jobs::authority::Enrolled, ApiError> {
        let access = self
            .authorize(headers, &request.space, Action::Edit)
            .await?;
        if !access.experimental {
            return Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "invite_only",
                "Lab workers aren't available for this account yet.",
            ));
        }
        let service = self
            .0
            .config
            .cloud
            .as_deref()
            .ok_or_else(worker_unavailable)?;
        service
            .enroll_lab_worker(headers, request)
            .await
            .map_err(worker_error)
    }

    async fn worker_access(
        &self,
        headers: &HeaderMap,
    ) -> Result<lab_jobs::protocol::Access, ApiError> {
        let service = self
            .0
            .config
            .cloud
            .as_deref()
            .ok_or_else(worker_unavailable)?;
        let mut access = service
            .lab_worker_access(headers)
            .await
            .map_err(worker_error)?;
        let admin = access.experimental;
        access.experimental |= self
            .0
            .config
            .agent_accounts
            .contains(&access.worker.account);
        access.cad = crate::cad_gate::account_passes(
            &self.0,
            headers,
            &access.worker.account,
            admin,
            &std::env::var(crate::cad_gate::TEST_ACCOUNTS_ENV).unwrap_or_default(),
        );
        bind_account(&self.0, &access.worker.account)
            .await
            .map_err(ApiError::from)?;
        Ok(access)
    }

    async fn revoke_worker(&self, headers: &HeaderMap, worker: &str) -> Result<(), ApiError> {
        let service = self
            .0
            .config
            .cloud
            .as_deref()
            .ok_or_else(worker_unavailable)?;
        service
            .revoke_lab_worker(headers, worker)
            .await
            .map_err(worker_error)
    }

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
        bind_account(&self.0, &id.account)
            .await
            .map_err(ApiError::from)?;
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
        let cad = action != Action::Edit
            || crate::cad_gate::account_passes(
                &self.0,
                headers,
                &id.account,
                id.admin,
                &std::env::var(crate::cad_gate::TEST_ACCOUNTS_ENV).unwrap_or_default(),
            );
        Ok(Access {
            cad,
            account: id.account,
            space: target,
            experimental: id.permitted,
        })
    }
}
fn worker_unavailable() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "worker_unavailable",
        "Lab workers aren't available right now. Try again later.",
    )
}
fn worker_error(error: crate::cloud::session::SessionError) -> ApiError {
    use crate::cloud::session::SessionError;
    match error {
        SessionError::Unauthenticated | SessionError::Forbidden => ApiError::new(
            StatusCode::UNAUTHORIZED,
            "worker_revoked",
            "This sign-in or worker key no longer works. Sign in or add the worker again.",
        ),
        SessionError::InvalidRequest => {
            ApiError::invalid("Check the worker's space, tools, and limits.")
        }
        _ => worker_unavailable(),
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
        if let Some(engine) = crate::cad_web::engine() {
            registry
                .register(Arc::new(crate::cad_web::CadApi::new(engine)), Gate::Cad)
                .expect("Unique CAD kind");
        }
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

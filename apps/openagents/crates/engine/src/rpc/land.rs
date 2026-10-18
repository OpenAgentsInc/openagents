//! Account-backed landing queue reads for every app client.

use openagents_rpc::{RpcError, RpcReply};

use crate::auth::Auth;

pub(super) async fn status(auth: &Auth) -> Result<RpcReply, RpcError> {
    let lease = auth.openagents_lease().ok_or_else(|| {
        RpcError::Failed(
            "We couldn't connect to your account to read the landing queue. Try again.".into(),
        )
    })?;
    let status = lease
        .client()
        .land(lease.session())
        .map_err(|error| RpcError::Failed(error.to_string()))?
        .status(&lease.cancel)
        .await
        .map_err(|error| RpcError::Failed(error.to_string()))?;
    if !lease.is_current() {
        return Err(RpcError::Failed(
            "We couldn't connect to your account. Try again.".into(),
        ));
    }
    RpcReply::value(&status)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use axum::{Json, Router, http::HeaderMap, routing::get};
    use openagents_account::{
        Controller,
        desktop::MemoryStore,
        openagents::{
            Client, Deployment,
            land::{LandEntry, LandState},
        },
    };
    use serde_json::{Value, json};

    use crate::auth::AuthConfig;

    #[tokio::test]
    async fn status_reads_the_queue_with_the_apps_account() {
        async fn account(headers: HeaderMap) -> Json<Value> {
            assert_eq!(
                headers["authorization"],
                format!("Bearer sess_{}", "a".repeat(64))
            );
            Json(json!({"account": {"id": "landing-account", "label": "Test"}, "workspaces": []}))
        }
        async fn queue(headers: HeaderMap) -> Json<Value> {
            assert_eq!(
                headers["authorization"],
                format!("Bearer sess_{}", "a".repeat(64))
            );
            Json(json!({
                "integrator": {"running": false},
                "waiting": [LandEntry { id: "waiting-change".into(), issue: Some(12038), ..Default::default() }],
                "landing": [],
                "failed": [LandEntry { id: "failed-change".into(), state: LandState::Bounced, reason: Some("Tests failed.".into()), ..Default::default() }]
            }))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/account", get(account))
                    .route("/v1/land", get(queue)),
            )
            .await
            .unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let client = Client::new(Deployment::new(&format!("{base}/v1"), &base).unwrap()).unwrap();
        let account = Controller::new(
            client,
            Arc::new(MemoryStore::default()),
            dir.path().join("account"),
        );
        account
            .import_credential(format!("sess_{}", "a".repeat(64)))
            .await;
        let auth = Auth::with_account(AuthConfig::new(&base, dir.path()), account);
        let RpcReply::Value(status) = status(&auth).await.unwrap() else {
            panic!("expected a status");
        };
        assert_eq!(status["integrator"]["running"], false);
        assert_eq!(status["waiting"][0]["issue"], 12038);
        assert_eq!(status["failed"][0]["reason"], "Tests failed.");
        server.abort();
    }

    #[tokio::test]
    async fn status_requires_an_explicit_app_account() {
        let dir = tempfile::tempdir().unwrap();
        let auth = Auth::new(AuthConfig::new("http://127.0.0.1:1", dir.path()));
        let error = match status(&auth).await {
            Err(error) => error,
            Ok(_) => panic!("a development identity is not an app account"),
        };
        assert!(error.to_string().contains("Sign in to OpenAgents"));
    }
}

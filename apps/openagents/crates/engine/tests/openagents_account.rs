//! The engine consumes the existing SDK controller; no OS credential store is touched.
use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};
use openagents_account::{
    Controller,
    desktop::{self, MemoryStore},
    openagents::{Client, Deployment},
};
use serde_json::json;
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};
use openagents_engine::{AuthState, Engine, EngineConfig, EngineProfile, WorkspaceScope};
use openagents_proto::HarnessId;
#[tokio::test]
async fn sdk_identity_drives_synced_rooms_and_logout_removes_authority() {
    let root = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let active = Arc::new(AtomicBool::new(true));
    let identity = Arc::new(Mutex::new(String::from("acct_engine_fixture")));
    let api = Router::new().route("/v1/account", get(|State((active, identity)): State<(Arc<AtomicBool>, Arc<Mutex<String>>)>| async move {
        if !active.load(Ordering::SeqCst) { return Err(StatusCode::UNAUTHORIZED); }
        Ok(Json(json!({"account":{"id":identity.lock().unwrap().clone(),"label":"Engine fixture"},"workspaces":[]})))
    })).route("/v1/device/sign-out", post(||async { Json(json!({"signed_out":true})) })).with_state((active.clone(), identity.clone()));
    let server = tokio::spawn(async move { axum::serve(listener, api).await.unwrap() });
    let deployment = Deployment::new(&format!("{origin}/v1"), &origin).unwrap();
    let controller = Controller::new(
        Client::new(deployment.clone()).unwrap(),
        Arc::new(MemoryStore::default()),
        root.path().join("account/signed-out"),
    );
    let token = format!("sess_{:064x}", 9002);
    controller.import_credential(token.clone()).await;
    assert!(controller.lease().is_some());
    desktop::register(root.path(), &controller);
    let config = EngineConfig {
        data_dir: root.path().into(),
        edge_url: "https://sync.example".into(),
        edge_token: None,
        ipc_port: 0,
        default_harness: HarnessId::Mock,
        org_id: None,
        workos_client_id: None,
    };
    let auth = Engine::build_auth(&config).await;
    assert_eq!(
        Engine::initial_workspace_scope(&auth),
        WorkspaceScope::Synced
    );
    assert_eq!(auth.access_token().await.unwrap(), token);
    assert_eq!(auth.user_id().as_deref(), Some("acct_engine_fixture"));
    let profile = Engine::resolve_profile(&config, &auth, WorkspaceScope::Synced)
        .unwrap()
        .unwrap();
    assert_eq!(profile.org_id(), "acct_engine_fixture");
    assert_eq!(profile.user_id(), "acct_engine_fixture");
    assert!(
        profile
            .store_root()
            .starts_with(root.path().join("profiles"))
    );
    assert_ne!(
        profile.store_root(),
        EngineProfile::local(root.path()).unwrap().store_root()
    );
    let scope = desktop::sync_scope(&deployment, &config.edge_url, "acct_engine_fixture");
    for (edge, account) in [
        ("https://other.example", "acct_engine_fixture"),
        (&config.edge_url[..], "acct_other"),
    ] {
        let other = EngineProfile::openagents(
            root.path(),
            account,
            &desktop::sync_scope(&deployment, edge, account),
        );
        assert_ne!(profile.store_root(), other.store_root());
    }
    assert_eq!(
        scope,
        desktop::sync_scope(&deployment, "https://sync.example/", "acct_engine_fixture")
    );
    let monitor = auth.spawn_refresh_loop();
    let mut status = auth.watch_state();
    controller.sign_out().await;
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            if matches!(*status.borrow_and_update(), AuthState::SignedOut) {
                break;
            }
            status.changed().await.unwrap();
        }
    })
    .await
    .unwrap();
    assert!(auth.access_token().await.is_err());
    assert!(Engine::resolve_profile(&config, &auth, WorkspaceScope::Synced).is_err());
    assert_eq!(
        Engine::initial_workspace_scope(&auth),
        WorkspaceScope::Local
    );
    controller.import_credential(token).await;
    *identity.lock().unwrap() = "acct_replacement".into();
    controller
        .import_credential(format!("sess_{:064x}", 9003))
        .await;
    assert!(controller.lease().is_some());
    assert!(
        auth.access_token().await.is_err(),
        "the old runtime must not use the replacement account's bearer"
    );
    assert_eq!(auth.state(), AuthState::SignedOut);
    assert!(Engine::resolve_profile(&config, &auth, WorkspaceScope::Synced).is_err());
    let replacement = Engine::build_auth(&config).await;
    let next = Engine::resolve_profile(&config, &replacement, WorkspaceScope::Synced)
        .unwrap()
        .unwrap();
    assert_eq!(next.org_id(), "acct_replacement");
    assert_ne!(profile.store_root(), next.store_root());
    active.store(false, Ordering::SeqCst);
    controller.clone().revalidate().await;
    assert!(auth.access_token().await.is_err());
    assert!(replacement.access_token().await.is_err());
    assert!(!root.path().join("session.json").exists());
    monitor.abort();
    server.abort();
}

#[tokio::test]
async fn local_boot_does_not_wait_for_a_blocked_credential_restore() {
    use openagents_account::openagents::{CredentialRecord, CredentialStore, StorageError};
    use std::sync::Condvar;
    struct Store(Arc<(Mutex<bool>, Condvar)>);
    impl CredentialStore for Store {
        fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
            let (lock, wake) = &*self.0;
            let mut ready = lock.lock().unwrap();
            while !*ready {
                ready = wake.wait(ready).unwrap();
            }
            Ok(None)
        }
        fn save(&self, _: &CredentialRecord) -> Result<(), StorageError> {
            Ok(())
        }
        fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
            Ok(())
        }
    }
    let root = tempfile::tempdir().unwrap();
    let gate = Arc::new((Mutex::new(false), Condvar::new()));
    let controller = Controller::new(
        Client::new(Deployment::production()).unwrap(),
        Arc::new(Store(gate.clone())),
        root.path().join("signed-out"),
    );
    desktop::register(root.path(), &controller);
    let restore = tokio::spawn(controller.restore());
    let config = EngineConfig {
        data_dir: root.path().into(),
        edge_url: "https://sync.example".into(),
        edge_token: None,
        ipc_port: 0,
        default_harness: HarnessId::Mock,
        org_id: None,
        workos_client_id: None,
    };
    let result = tokio::time::timeout(Duration::from_secs(1), Engine::build_auth(&config)).await;
    let (lock, wake) = &*gate;
    *lock.lock().unwrap() = true;
    wake.notify_all();
    let auth =
        result.expect("local startup must not wait for credential storage or account requests");
    assert_eq!(
        Engine::initial_workspace_scope(&auth),
        WorkspaceScope::Local
    );
    restore.await.unwrap();
}

//! Signing in with a Nostr key (#11993) end to end: the SDK, the website's
//! `/v1/device/nostr` and `/v1/device/keys` forwarders, and the account
//! service's `oa_auth::nostr` routes over real stores.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::sync::Arc;

use nostr::domain::{RelaySigner, Tag};
use openagents_sdk::{Client, Deployment, Error, EventTemplate, NostrSigner, SignerError};
use serde_json::json;

struct Key(RelaySigner);

impl NostrSigner for Key {
    fn sign_event(&self, template: &EventTemplate) -> Result<String, SignerError> {
        let tags = template.tags.iter().map(|t| Tag::new(t.clone())).collect();
        let event = self.0.sign(
            template.created_at,
            template.kind,
            tags,
            template.content.clone(),
        );
        serde_json::to_string(&event).map_err(|_| SignerError)
    }
}

fn key(seed: u8) -> Key {
    Key(RelaySigner::from_secret_hex(&format!("{seed:02x}").repeat(32)).unwrap())
}

struct Refuse;
impl NostrSigner for Refuse {
    fn sign_event(&self, _: &EventTemplate) -> Result<String, SignerError> {
        Err(SignerError)
    }
}

#[tokio::test]
async fn a_key_signs_in_through_the_website_and_manages_its_keys() {
    use oa_auth::{
        fake::{self, Fake},
        local::LocalService,
    };
    let fixture_root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(fixture_root).unwrap();
    let root = tempfile::Builder::new()
        .prefix("sdk-nostr-")
        .tempdir_in(fixture_root)
        .unwrap();
    let state = root.path().canonicalize().unwrap();
    std::fs::set_permissions(&state, std::fs::Permissions::from_mode(0o700)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let origin = format!("http://{address}");
    let redirect = format!("{origin}/auth/github/callback");
    let fake = Fake::new("client", "secret", &redirect, vec![fake::octo()]);
    let github = fake.spawn().await.unwrap();
    let credentials = fake::credentials(&github, "client", "secret", &redirect).unwrap();
    let accounts_dir = state.join("accounts");
    let service = LocalService::install(
        &accounts_dir,
        oa_auth::Github::new(credentials).unwrap(),
        "signup",
        3600,
    )
    .unwrap();
    let auth = oa_auth::nostr::NostrAuth::open(&accounts_dir, &[origin.clone()]).unwrap();
    let backend_router = service
        .router()
        .merge(oa_auth::nostr::router(auth, "signup"));
    let backend_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let backend = format!("http://{}", backend_listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(backend_listener, backend_router).await.ok() });
    let key_path = state.join("csrf.key");
    std::fs::write(&key_path, [42; 32]).unwrap();
    std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config_path = state.join("cloud.json");
    std::fs::write(
        &config_path,
        serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":origin,"account_service":backend,"csrf_secret":key_path})).unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = openagents_web::Config::development(state.join("tasks"));
    config.port = address.port();
    config.public_hosts.push(address.to_string());
    config.cloud = Some(Arc::new(
        openagents_web::cloud::session::CloudSession::load(&config_path).unwrap(),
    ));
    config.inference = Some(Arc::new(
        openagents_web::upstream::Upstream::new(&backend).unwrap(),
    ));
    let router = openagents_web::router(config);
    tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .unwrap()
    });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();

    let phone = key(1);
    let first = client
        .sign_in_with_key(&phone, "OpenAgents", "Pat's iPhone")
        .await
        .unwrap();
    assert!(first.created);
    assert_eq!(first.pubkey, phone.0.pubkey());
    assert!(first.session.bearer_token().starts_with("sess_"));
    let view = client.account(&first.session).await.unwrap();
    assert_eq!(view.workspaces.len(), 1);
    let again = client
        .sign_in_with_key(&phone, "OpenAgents", "Pat's iPhone")
        .await
        .unwrap();
    assert!(!again.created);
    assert_eq!(again.session.account().id, first.session.account().id);

    let desk = key(2);
    let keys = client.link_key(&first.session, &desk).await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys[1].npub.starts_with("npub1"));
    let desk_in = client
        .sign_in_with_key(&desk, "OpenAgents", "Pat's Mac")
        .await
        .unwrap();
    assert_eq!(desk_in.session.account().id, first.session.account().id);
    assert_eq!(client.account_keys(&desk_in.session).await.unwrap(), keys);

    let left = client
        .remove_key(&first.session, phone.0.pubkey())
        .await
        .unwrap();
    assert_eq!(left.len(), 1);
    assert_eq!(
        client
            .remove_key(&first.session, desk.0.pubkey())
            .await
            .unwrap_err(),
        Error::Refused {
            status: 409,
            code: Some("last_key")
        }
    );
    assert_eq!(
        client
            .sign_in_with_key(&Refuse, "OpenAgents", "x")
            .await
            .unwrap_err(),
        Error::InvalidCredential
    );
    let store = tenancy::Accounts::open(&accounts_dir)
        .unwrap()
        .store()
        .unwrap();
    assert_eq!(store.accounts.len(), 1);
}

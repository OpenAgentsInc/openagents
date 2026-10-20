//! The SDK against the website's real handlers. These tests moved here from
//! `openagents-sdk` (#12199), so the SDK's own test build doesn't compile
//! the website.
#![cfg(unix)]

/// Signing in with a Nostr key (#11993) end to end: the SDK, the website's
/// `/v1/device/nostr` and `/v1/device/keys` forwarders, and the account
/// service's `oa_auth::nostr` routes over real stores.
mod nostr_sign_in {
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
        let client =
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();

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
}

mod device_login {
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    use openagents_sdk::{CancellationToken, Client, CredentialRecord, Deployment, Error};
    use serde_json::json;

    /// Real public website handlers over real tenancy stores, all under a temporary root.
    #[tokio::test]
    async fn sdk_device_login_restore_workspaces_and_sign_out_use_the_public_handlers() {
        use oa_auth::{
            fake::{self, Fake},
            local::LocalService,
        };
        // This dependency uses its production private-file checks. Use Cargo's
        // explicit test root, not a system temp path with writable ancestors.
        let fixture_root = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
        std::fs::create_dir_all(fixture_root).unwrap();
        let root = tempfile::Builder::new()
            .prefix("sdk-auth-")
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
        let accounts = tenancy::Accounts::open(&accounts_dir).unwrap();
        let account = accounts
            .create_account("Existing account", &["key:0000000000000000".into()])
            .unwrap();
        let workspace = accounts
            .create_workspace(
                &account.id,
                "Personal",
                tenancy::WorkspaceKind::Personal,
                "signup",
                None,
            )
            .unwrap();
        let backend = service.spawn().await.unwrap();
        let key_path = state.join("csrf.key");
        std::fs::write(&key_path, [42; 32]).unwrap();
        std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let config_path = state.join("cloud.json");
        std::fs::write(&config_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":origin,"account_service":backend,"csrf_secret":key_path})).unwrap()).unwrap();
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
        let client =
            Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
        let mut login = client
            .start_login("Coder Desktop", "fixture-mac")
            .await
            .unwrap();
        tenancy::sessions::Sessions::open(&accounts_dir)
            .unwrap()
            .mutate(|book, _, now| {
                book.decide_device(
                    login.user_code(),
                    &tenancy::workspaces::UserId::from(account.id.as_str()),
                    true,
                    now,
                )
            })
            .unwrap();
        let session = client
            .wait_for_login(&mut login, &CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(session.account().id, account.id);
        let view = client.account(&session).await.unwrap();
        assert_eq!(view.workspaces[0].id, workspace.id);
        let bytes = serde_json::to_vec(&session.record()).unwrap();
        let record: CredentialRecord = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            client.restore(&record).await.unwrap().account().id,
            account.id
        );
        // Fixture setup uses the existing sync API; the SDK exposes only reads.
        let fixture_credential = serde_json::to_value(&record).unwrap()["credential"]
            .as_str()
            .unwrap()
            .to_owned();
        let seed = reqwest::Client::new().put(format!("{origin}/v1/threads/synced/phone-sdk-fixture"))
            .bearer_auth(&fixture_credential).json(&json!({
                "computer":"Offline fixture", "title":"Long saved chat", "messages":(0..405).map(|n|json!({"role":if n==1 {"thinking"}else{"assistant"},"text":format!("Saved {n}")})).collect::<Vec<_>>()
            })).send().await.unwrap();
        assert!(seed.status().is_success());
        let cancel = CancellationToken::new();
        let mut chats = client.history(&session).unwrap();
        let page = chats
            .next_page(&client, &session, &cancel)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.data[0].source, openagents_sdk::ChatSource::Phone);
        let mut transcript = client
            .transcript(&session, &page.data[0].chat)
            .unwrap()
            .with_page_size(200)
            .unwrap();
        let mut messages = Vec::new();
        while let Some(page) = transcript
            .next_page(&client, &session, &cancel)
            .await
            .unwrap()
        {
            messages.extend(page.data);
        }
        assert_eq!(messages.len(), 405);
        assert_eq!(messages[1].role, openagents_sdk::MessageRole::Thinking);
        assert_eq!(messages[404].text, "Saved 404");
        // A saved credential can be revoked even without a restorable account session.
        let mut saved = serde_json::to_value(session.record()).unwrap();
        saved["expires_at"] = json!(0);
        let saved = serde_json::from_value(saved).unwrap();
        client.sign_out_record(&saved).await.unwrap();
        assert!(matches!(
            client.restore(&record).await,
            Err(Error::Unauthorized)
        ));
        assert_eq!(accounts.store().unwrap().accounts.len(), 1);
    }
}

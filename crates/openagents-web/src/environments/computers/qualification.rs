//! Explicit staging qualification. The scripted model drives the existing
//! setup tools; allocation, commands, image capture, and fresh verification
//! use the real staging Computers service. No inference request is made.
use super::*;
use axum::routing::any;
use coder_environment_operator::{
    Providers,
    studio::{Config as StudioConfig, Studio, Transports},
};
use coder_working_computer::{
    Principal,
    provider::{Images, Outcome, Provider, fake::FakeProvider},
};
use codex_transport::{
    TokenUsage,
    fake::{FakeTransport, call},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

const STAGING: &str = "https://openagents-web-1-staging-ezxz4mgdsq-uc.a.run.app";
const BACKEND: &str = "https://oa-computers-staging-ezxz4mgdsq-uc.a.run.app";
#[derive(serde::Deserialize)]
struct Configuration {
    account: String,
    workspace: String,
    token: String,
    session: Value,
    backend_token: String,
    base: String,
    commit: String,
    evidence: PathBuf,
}
#[derive(Clone)]
struct Native {
    http: reqwest::Client,
    account: String,
    token: String,
    session: Value,
}
// /v1/session is intentionally private to the gateway sidecar. This test-only
// adapter returns the actual session issued by staging after checking its
// bearer at the deployed account endpoint on every read. Other account reads
// go to staging unchanged, including current workspace membership epochs.
async fn native(State(n): State<Native>, request: Request) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .unwrap_or_default();
    if token != n.token {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    let path = request.uri().path();
    if path != "/v1/session" && path != "/v1/account" && !path.starts_with("/v1/workspaces/") {
        return StatusCode::FORBIDDEN.into_response();
    }
    let target = if path == "/v1/session" {
        "/v1/account"
    } else {
        path
    };
    let Ok(r) = n
        .http
        .get(format!("{STAGING}{target}"))
        .bearer_auth(token)
        .send()
        .await
    else {
        return unavailable();
    };
    let status = r.status();
    let Ok(bytes) = r.bytes().await else {
        return unavailable();
    };
    if path == "/v1/session" && status.is_success() {
        let Ok(account) = serde_json::from_slice::<Value>(&bytes) else {
            return unavailable();
        };
        if account["account"]["id"] != n.account {
            return denied();
        }
        // The public exchange returns issuance metadata without standing. A
        // successful authenticated account read establishes that this same
        // bearer is active; the private session route includes that field.
        let mut session = n.session.clone();
        session["state"] = json!("active");
        return Json(json!({"session":session})).into_response();
    }
    (status, [(header::CONTENT_TYPE, "application/json")], bytes).into_response()
}
fn config(state: PathBuf, base: &str) -> StudioConfig {
    StudioConfig {
        schema: coder_environment_operator::studio::SCHEMA.into(),
        state,
        machines: coder_environment_operator::Config {
            schema: coder_environment_operator::SCHEMA.into(),
            provider: coder_environment_operator::ProviderKind::Boat,
            gce: None,
            workdir: "/home/user/workspace".into(),
            template: Some(base.into()),
            credential_names: BTreeSet::new(),
            tick_seconds: 2,
        },
        owner: Principal {
            workspace: "sdk-qualification".into(),
            principal: "staging-test".into(),
        },
        codex_home: None,
        model: None,
        size: None,
        deadline_seconds: Some(1800),
        github_token: None,
        claude_key: None,
        model_api: None,
        home_retention_days: None,
    }
}
fn transports() -> Transports<FakeTransport> {
    Arc::new(|_| {
        Ok(FakeTransport::new(vec![
            call(
                "inspect",
                "run_command",
                &json!({"command":"git status --short; cat deploy/space-host/fixtures/environment/Cargo.toml"}),
                TokenUsage::default(),
            ),
            call(
                "recipe",
                "write_recipe",
                &json!({"install_script":"set -eu\nexport PATH=\"$HOME/.cargo/bin:$PATH\"\nif ! command -v rustup >/dev/null 2>&1; then curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.97.1; fi\nrustup toolchain install 1.97.1 --profile minimal\ncargo build --locked --manifest-path deploy/space-host/fixtures/environment/Cargo.toml\n"}),
                TokenUsage::default(),
            ),
            call(
                "checks",
                "set_checks",
                &json!({"checks":[{"name":"fixture","command":"export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo test --locked --manifest-path deploy/space-host/fixtures/environment/Cargo.toml"}],"offline":false}),
                TokenUsage::default(),
            ),
            call("install", "run_install", &json!({}), TokenUsage::default()),
            call(
                "finish",
                "finish",
                &json!({"summary":"The Rust fixture builds and passes its tests."}),
                TokenUsage::default(),
            ),
        ]))
    })
}
async fn reuse_saved_image(
    api: &openagents_sdk::computers::Computers<'_>,
    id: &str,
    number: u64,
    evidence: &std::path::Path,
) -> Result<String, String> {
    use openagents_sdk::{
        CancellationToken,
        computers::{CommandOutcome, WaitFor},
    };
    let cancel = CancellationToken::new();
    let request=api.create_request(serde_json::from_value(json!({"environment":id,"version":number.to_string(),"size":"small","idempotency_key":format!("reuse-{id}")})).unwrap()).map_err(|_|"Saved-image request failed.")?;
    shared::write(&evidence.with_file_name("reuse-request.json"), &request)?;
    let computer = api
        .create(&request, &cancel)
        .await
        .map_err(|_| "Saved-image allocation was not confirmed.")?;
    shared::write(
        &evidence.with_file_name("reuse-reference.json"),
        computer.reference(),
    )?;
    api.wait(
        computer.reference(),
        WaitFor::Awake,
        Duration::from_secs(600),
        &cancel,
    )
    .await
    .map_err(|_| "Saved image did not wake.")?;
    let request=api.command_request(computer.reference(),serde_json::from_value(json!({"command":r#"test -f "$HOME/repo/deploy/space-host/fixtures/environment/Cargo.toml" && test -f "$HOME/workspace/deploy/space-host/fixtures/environment/Cargo.toml" && printf oa-root-compatible"#,"idempotency_key":format!("reuse-check-{id}"),"timeout_seconds":60})).unwrap()).map_err(|_|"Saved-image check could not be prepared.")?;
    let command = api
        .execute(&request, &cancel)
        .await
        .map_err(|_| "Saved-image check was not confirmed.")?;
    let mut reader = api
        .output(command.reference())
        .map_err(|_| "Saved-image output could not be opened.")?;
    let mut bytes = vec![];
    let end = Instant::now() + Duration::from_secs(120);
    while Instant::now() < end {
        let Some(page) = reader
            .next_page(api, &cancel)
            .await
            .map_err(|_| "Saved-image output could not be read.")?
        else {
            break;
        };
        if page.stdout_gap || page.stderr_gap {
            return Err("Saved-image output was incomplete.".into());
        }
        bytes.extend(page.stdout);
        if page.terminal && !page.has_more {
            if page.command.outcome() != Some(CommandOutcome::Succeeded)
                || bytes != b"oa-root-compatible"
            {
                return Err("The saved image did not preserve both repository paths.".into());
            }
            return Ok(computer.reference().id().into());
        }
    }
    Err("Saved-image check timed out.".into())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires an explicit disposable staging account and real Boat resources."]
async fn a_staging_repository_build_fresh_verifier_and_save_use_shared_computers() {
    use std::os::unix::fs::PermissionsExt;
    let path = PathBuf::from(
        std::env::var_os("OA_ENVIRONMENT_QUALIFY_CONFIG")
            .expect("Set the private staging qualification file."),
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let q: Configuration = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(q.session["account"], q.account);
    assert_eq!(q.session["kind"], "user");
    assert!(q.token.starts_with("sess_"));
    assert_eq!(q.commit.len(), 40);
    assert!(q.commit.bytes().all(|c| c.is_ascii_hexdigit()));
    let dir = q.evidence.parent().unwrap().join(format!(
        "run-state-{}",
        coder_environment_operator::now_ms()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let native_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let native_origin = format!("http://{}", native_listener.local_addr().unwrap());
    let native_router = Router::new().fallback(any(native)).with_state(Native {
        http: reqwest::Client::new(),
        account: q.account.clone(),
        token: q.token.clone(),
        session: q.session.clone(),
    });
    let native_server =
        tokio::spawn(async move { axum::serve(native_listener, native_router).await.unwrap() });
    let csrf = dir.as_path().join("csrf");
    std::fs::write(&csrf, [9; 32]).unwrap();
    std::fs::set_permissions(&csrf, std::fs::Permissions::from_mode(0o600)).unwrap();
    let cloud = dir.as_path().join("cloud.json");
    std::fs::write(&cloud,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":origin,"account_service":native_origin,"csrf_secret":csrf})).unwrap()).unwrap();
    std::fs::set_permissions(&cloud, std::fs::Permissions::from_mode(0o600)).unwrap();
    crate::computers::configure(BACKEND, crate::computers::Token::Value(q.backend_token));
    let root_state = dir.as_path().join("owners");
    let state = root_state.join("public-computers");
    let fake = Arc::new(FakeProvider::new(BTreeMap::new(), true));
    let root = Studio::start(
        config(root_state, &q.base),
        &q.base,
        Providers {
            setup: fake.clone(),
            build: fake.clone(),
            verify: fake.clone(),
        },
        Arc::new(|_| Ok(coder_environment::evidence::Redactor::new())),
        transports(),
        Duration::from_secs(1),
    )
    .unwrap();
    let runtime = Arc::new(shared::Runtime::new(&state, &origin, [7; 32]).unwrap());
    let selected = || shared::Selected {
        shared: true,
        legacy: fake.clone(),
        runtime: Some(runtime.clone()),
        store: Store::under(&state),
    };
    let child = Studio::start(
        config(state.clone(), &q.base),
        &q.base,
        Providers {
            setup: selected(),
            build: selected(),
            verify: selected(),
        },
        Arc::new(|_| Ok(coder_environment::evidence::Redactor::new())),
        transports(),
        Duration::from_secs(1),
    )
    .unwrap();
    root.attach_public(child.clone()).unwrap();
    let mut configuration = crate::Config::development(dir.as_path().join("tasks"));
    configuration.ask_salt = [7; 32];
    configuration.public_hosts = vec![origin.trim_start_matches("http://").to_owned()];
    configuration.agent_accounts = vec![q.account.clone()];
    configuration.cloud = Some(Arc::new(CloudSession::load(&cloud).unwrap()));
    configuration.inference = Some(Arc::new(
        crate::upstream::Upstream::new(&native_origin).unwrap(),
    ));
    configuration.solver_keys = Some(Arc::new(
        oa_seal::Keyring::scratch("staging-environment-qualification")
            .unwrap()
            .0,
    ));
    configuration.environments.set(root.clone());
    let app = App(Arc::new(crate::Inner {
        config: configuration,
    }));
    let public = crate::router(app.config.clone());
    let public_server = tokio::spawn(async move { axum::serve(listener, public).await.unwrap() });
    let resolved = Resolved {
        repository: coder_environment_operator::studio::github::RepoName::parse(
            "OpenAgentsInc/openagents",
        )
        .unwrap(),
        branch: "main".into(),
        commit: q.commit,
        private: false,
    };
    let mut headers = HeaderMap::new();
    headers.insert(
        header::HOST,
        origin.trim_start_matches("http://").parse().unwrap(),
    );
    headers.insert(
        header::COOKIE,
        format!(
            "oa_cloud_session={}; oa_cloud_workspace={}",
            q.token, q.workspace
        )
        .parse()
        .unwrap(),
    );
    let authorization = Authorization::prepare(&app, &headers, &resolved)
        .await
        .unwrap();
    let id = root
        .create_for_bound(&resolved, &q.account, |id| authorization.save(&app, id))
        .unwrap();
    let sdk = openagents_sdk::Client::new(
        openagents_sdk::Deployment::new(&format!("{origin}/v1"), &origin).unwrap(),
    )
    .unwrap();
    let session = sdk.authenticate(&q.token).await.unwrap();
    let computers = sdk.computers(&session).unwrap();
    let begin = Instant::now();
    let mut ready = false;
    let mut reason = String::new();
    // Setup, clean build, and two fresh verification computers have separate
    // native deadlines. Allow their bounded stop/wake cycles to finish.
    while begin.elapsed() < Duration::from_secs(5400) {
        let view = root.view(&id).unwrap();
        if view.candidate.is_some() {
            ready = true;
            break;
        }
        if matches!(
            view.phase,
            coder_environment_operator::agent::Phase::Failed { .. }
                | coder_environment_operator::agent::Phase::Waiting { .. }
        ) {
            reason = format!("{:?}", view.phase);
            break;
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
    let mut result = json!({"schema":"openagents.environment-computers.qualification.v1","account":q.account,"environment":id,"staging_backend":BACKEND,"public_front":"local test of the same server router","native_authority":"current staging account and workspace reads; issued-session adapter","scripted_model":true,"model_calls":0,"candidate_ready":ready,"reason":reason});
    let mut reuse_ok = false;
    if ready {
        let view = root.view(&id).unwrap();
        let candidate = view.candidate.unwrap();
        assert!(root.save(&id, "unverified").await.is_err());
        let number = root.save(&id, &candidate.digest).await.unwrap();
        assert_eq!(number, 1);
        assert!(root.computer_pin("another-account", &id, number).is_err());
        let (_, pin) = root.computer_pin(&q.account, &id, number).unwrap();
        assert_eq!(pin.source.revision, resolved.commit);
        result["saved_version"] = json!(number);
        result["image"] = json!(pin.image.image_id);
        result["candidate_digest"] = json!(candidate.digest);
        match reuse_saved_image(&computers, &id, number, &q.evidence).await {
            Ok(computer) => {
                reuse_ok = true;
                result["reused_computer"] = json!(computer);
                result["legacy_and_public_paths_verified"] = json!(true);
            }
            Err(error) => {
                result["reuse_error"] = json!(error);
            }
        }
        let list = computers
            .list(200, None, &openagents_sdk::CancellationToken::new())
            .await
            .unwrap();
        result["public_computers"] = json!(
            list.computers
                .iter()
                .map(|c| c.reference().id().to_string())
                .collect::<Vec<_>>()
        );
    }
    result["activity"] = serde_json::to_value(root.view(&id).unwrap().records).unwrap();
    // Ended setup computers can be stopped rather than deleted by the original
    // owner. Delete every allocation through its saved SDK binding, even when
    // qualification fails; keep a cleanup receipt outside the temporary root.
    let mut cleanup = vec![];
    // This fresh qualification account owns only this run's resources. Include
    // a regular saved-image allocation whose create reply may have been lost.
    if let Ok(page) = computers
        .list(200, None, &openagents_sdk::CancellationToken::new())
        .await
    {
        for computer in page.computers {
            let cancel = openagents_sdk::CancellationToken::new();
            let _ = computers.delete(computer.reference(), &cancel).await;
            let deleted = computers
                .wait(
                    computer.reference(),
                    openagents_sdk::computers::WaitFor::Deleted,
                    Duration::from_secs(600),
                    &cancel,
                )
                .await
                .is_ok();
            cleanup.push(json!({"computer":computer.reference().id(),"role":"account cleanup","deleted":deleted}));
        }
    }
    let bindings = state.join("environment-computers/attempts");
    if bindings.is_dir() {
        for entry in std::fs::read_dir(bindings).unwrap() {
            let entry = entry.unwrap();
            let b: Binding = serde_json::from_slice(&std::fs::read(entry.path()).unwrap()).unwrap();
            let c = Store::under(&state).computer(&b.native).unwrap();
            if let Some(reference) = &b.computer {
                let deleted = matches!(
                    selected().delete(&c, reference.id()).await,
                    Outcome::Done { .. }
                );
                cleanup.push(
                    json!({"computer":reference.id(),"role":b.attempt.role,"deleted":deleted}),
                );
            }
            if let Some(name) = b.attempt.image_name {
                let deleted = matches!(selected().delete_image(&name).await, Outcome::Done { .. });
                cleanup.push(json!({"image":name,"deleted":deleted}));
            }
        }
    }
    result["cleanup"] = json!(cleanup);
    shared::write(&q.evidence, &result).unwrap();
    public_server.abort();
    native_server.abort();
    assert!(
        ready,
        "The staging environment did not produce a verified candidate; see the private evidence file."
    );
    assert!(
        reuse_ok,
        "The saved-image compatibility check failed; see the private evidence file."
    );
    assert!(
        cleanup.iter().all(|item| item["deleted"] == true),
        "A staging resource still needs cleanup."
    );
}

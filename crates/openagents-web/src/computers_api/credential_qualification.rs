//! Explicit live canary frontend. Real staging account reads authorize the
//! normal router; its Computers backend is a separate disposable deployment.
use super::*;
use axum::{Json, Router, extract::Request, response::IntoResponse, routing::any};
use std::{
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

const STAGING: &str = "https://openagents-web-1-staging-ezxz4mgdsq-uc.a.run.app";
#[derive(Deserialize)]
struct Account {
    id: String,
    token: String,
    session: Value,
}
#[derive(Deserialize)]
struct Configuration {
    accounts: Vec<Account>,
    directory: PathBuf,
    backend: String,
    backend_token: String,
    port: u16,
    workflow: Option<Workflow>,
}
#[derive(Deserialize)]
struct Workflow {
    source: String,
    egress_profile: String,
    command: String,
    evidence: PathBuf,
}
struct Native {
    http: reqwest::Client,
    accounts: Vec<Account>,
}
async fn native(State(n): State<Arc<Native>>, request: Request) -> Response {
    if request.method() != axum::http::Method::GET
        || request
            .headers()
            .get_all(header::AUTHORIZATION)
            .iter()
            .count()
            != 1
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .unwrap_or_default();
    let Some(account) = n.accounts.iter().find(|a| a.token == token) else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    let path = request.uri().path();
    if path != "/v1/session" && path != "/v1/account" && !path.starts_with("/v1/workspaces/") {
        return StatusCode::FORBIDDEN.into_response();
    }
    let target = if path == "/v1/session" {
        "/v1/account"
    } else {
        path
    };
    let Ok(mut reply) = n
        .http
        .get(format!("{STAGING}{target}"))
        .bearer_auth(token)
        .send()
        .await
    else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let status = reply.status();
    let mut bytes = Vec::new();
    loop {
        match reply.chunk().await {
            Ok(Some(chunk)) if bytes.len() + chunk.len() <= 1048576 => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            _ => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
        }
    }
    if path == "/v1/session" && status.is_success() {
        let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
            return StatusCode::SERVICE_UNAVAILABLE.into_response();
        };
        if value["account"]["id"] != account.id
            || value["account"]["email"] == "chris@openagents.com"
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        let mut session = account.session.clone();
        session["state"] = json!("active");
        return Json(json!({"session":session})).into_response();
    }
    (status, [(header::CONTENT_TYPE, "application/json")], bytes).into_response()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires private canary configuration and dedicated staging accounts."]
async fn serve_the_live_credential_canary_frontend() {
    let path = PathBuf::from(
        std::env::var_os("OA_CREDENTIAL_QUALIFY_CONFIG")
            .expect("Set the private canary configuration file."),
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let q: Configuration = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(q.accounts.len(), 2);
    assert_ne!(q.accounts[0].id, q.accounts[1].id);
    for a in &q.accounts {
        assert_eq!(a.session["account"], a.id);
        assert_eq!(a.session["kind"], "user");
        assert!(a.token.starts_with("sess_"));
    }
    std::fs::create_dir_all(&q.directory).unwrap();
    std::fs::set_permissions(&q.directory, std::fs::Permissions::from_mode(0o700)).unwrap();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", q.port))
        .await
        .unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let account_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let account_origin = format!("http://{}", account_listener.local_addr().unwrap());
    let accounts: Vec<_> = q.accounts.iter().map(|a| a.id.clone()).collect();
    let router = Router::new()
        .fallback(any(native))
        .with_state(Arc::new(Native {
            http: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap(),
            accounts: q.accounts,
        }));
    let account_server = tokio::spawn(async move {
        axum::serve(account_listener, router).await.unwrap();
    });
    let csrf = q.directory.join("csrf");
    std::fs::write(&csrf, [13; 32]).unwrap();
    std::fs::set_permissions(&csrf, std::fs::Permissions::from_mode(0o600)).unwrap();
    let cloud = q.directory.join("cloud.json");
    std::fs::write(&cloud,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":origin,"account_service":account_origin,"csrf_secret":csrf})).unwrap()).unwrap();
    std::fs::set_permissions(&cloud, std::fs::Permissions::from_mode(0o600)).unwrap();
    crate::computers::configure(&q.backend, crate::computers::Token::Value(q.backend_token));
    let mut config = crate::Config::development(q.directory.join("tasks"));
    config.public_hosts = vec![origin.trim_start_matches("http://").to_owned()];
    config.agent_accounts = accounts.clone();
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&cloud).unwrap(),
    ));
    config.inference = Some(Arc::new(
        crate::upstream::Upstream::new(&account_origin).unwrap(),
    ));
    let public = crate::router(config);
    let server = tokio::spawn(async move {
        axum::serve(listener, public).await.unwrap();
    });
    std::fs::write(
        q.directory.join("frontend-ready.json"),
        serde_json::to_vec(&json!({"origin":origin,"accounts":accounts,"inference_calls":0}))
            .unwrap(),
    )
    .unwrap();
    let started = Instant::now();
    while !q.directory.join("frontend-done").exists() {
        assert!(
            started.elapsed() < Duration::from_secs(3600),
            "The canary frontend reached its time limit."
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    server.abort();
    account_server.abort();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "Requires the isolated SPACE canary, broker, clients, and dedicated staging accounts."]
async fn the_live_sdk_uses_model_git_and_github_credential_references() {
    use openagents_sdk::computers::{CommandOptions, CommandOutcome, CreateOptions, WaitFor};
    use openagents_sdk::{CancellationToken, Client, Deployment};
    let path = PathBuf::from(
        std::env::var_os("OA_CREDENTIAL_QUALIFY_CONFIG")
            .expect("Set the private canary configuration file."),
    );
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let q: Configuration = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let workflow = q
        .workflow
        .as_ref()
        .expect("Configure the qualified client workflow.");
    let origin = format!("http://127.0.0.1:{}", q.port);
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let primary = client.authenticate(&q.accounts[0].token).await.unwrap();
    let secondary = client.authenticate(&q.accounts[1].token).await.unwrap();
    assert_eq!(primary.account().id, q.accounts[0].id);
    assert_ne!(primary.account().id, secondary.account().id);
    let computers = client
        .computers(&primary)
        .unwrap()
        .with_operation_timeout(Duration::from_secs(30))
        .unwrap();
    let foreign = client.computers(&secondary).unwrap();
    let cancel = CancellationToken::new();
    let request = computers
        .create_request(CreateOptions {
            size: Some("small".into()),
            placement: Some("fast-sandbox".into()),
            egress_profile: Some(workflow.egress_profile.clone()),
            needs: Some(openagents_sdk::generated::computers::AppComputerNeeds {
                profiles: Some(vec!["core".into(), "suspend".into()]),
                credential_proxy: Some(true),
                command_sessions: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        })
        .unwrap();
    let save = |name: &str, value: Value| {
        let path = q.directory.join(name);
        std::fs::write(&path, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    };
    save(
        "create-request.json",
        serde_json::to_value(&request).unwrap(),
    );
    let created = computers.create(&request, &cancel).await.unwrap();
    let reference = created.reference().clone();
    save("computer.json", serde_json::to_value(&reference).unwrap());
    let mut report = json!({"schema":"openagents.computers.credential-client-qualification.v1",
        "computer":reference.id(),"account":primary.account().id,"checks":[],"cleanup":false});
    let result: Result<(), openagents_sdk::computers::Error> = async {
        let ensure = |condition: bool| if condition { Ok(()) } else { Err(openagents_sdk::computers::Error::InvalidResponse) };
        computers.wait(&reference,WaitFor::Awake,Duration::from_secs(180),&cancel).await?;
        let refused = foreign.get(&foreign.reference(reference.id())?,&cancel).await.err()
            .ok_or(openagents_sdk::computers::Error::InvalidResponse)?;
        ensure(matches!(refused.status(),Some(403|404)))?;
        report["checks"].as_array_mut().unwrap().push(json!("foreign account computer read refused"));
        for phase in ["initial","after-wake"] {
            let command = computers.command_request(&reference,CommandOptions {
                command:workflow.command.clone(),timeout_seconds:Some(120),..Default::default()
            })?;
            let mut command = command;
            let mut credentials = Vec::new();
            for (rule,env) in [("messages","OA_PARAM_MODEL_REF"),("git","OA_PARAM_GIT_REF"),("github-api","OA_PARAM_GH_REF")] {
                let request = computers.credential_request(&command,&workflow.source,rule,300)?;
                save(&format!("{phase}-{rule}-request.json"),serde_json::to_value(&request).unwrap());
                let prepared = computers.prepare_credential(&request,&cancel).await?;
                let retry = computers.prepare_credential(&request,&cancel).await?;
                ensure(prepared.reference()==retry.reference())?;
                ensure(prepared.data().expires_unix==retry.data().expires_unix)?;
                command = computers.command_with_credential(&command,&prepared,env)?;
                credentials.push(prepared);
            }
            save(&format!("{phase}-command-request.json"),serde_json::to_value(&command).unwrap());
            let execution = computers.execute(&command,&cancel).await?;
            let retry = computers.execute(&command,&cancel).await?;
            ensure(execution.reference()==retry.reference())?;
            save(&format!("{phase}-command.json"),serde_json::to_value(execution.reference()).unwrap());
            let mut reader = computers.output(execution.reference())?;
            let mut stdout=Vec::new();let mut stderr=Vec::new();let mut outcome=None;
            let deadline=Instant::now()+Duration::from_secs(180);
            while let Some(page)=reader.next_page(&computers,&cancel).await? {
                ensure(Instant::now()<deadline)?;
                ensure(!page.stdout_gap && !page.stderr_gap)?;
                stdout.extend_from_slice(&page.stdout);stderr.extend_from_slice(&page.stderr);
                outcome=page.command.outcome();
            }
            save(&format!("{phase}-output.json"),json!({"stdout":String::from_utf8_lossy(&stdout),"stderr":String::from_utf8_lossy(&stderr)}));
            ensure(outcome==Some(CommandOutcome::Succeeded))?;
            let output=String::from_utf8_lossy(&stdout);
            ensure(output.contains("SDK_PROXY_OK") && output.contains("credential-fixture") && output.contains("GIT_CLONE_OK"))?;
            for credential in credentials {
                let state=computers.get_credential(credential.reference(),&cancel).await?;
                ensure(state.data().state=="revoked")?;
            }
            report["checks"].as_array_mut().unwrap().push(json!({"phase":phase,"command":execution.reference().id(),"model_git_github_clients":true,"stable_retry":true,"completion_revoked":true}));
            if phase=="initial" {
                computers.stop(&reference,&cancel).await?;
                computers.wait(&reference,WaitFor::Stopped,Duration::from_secs(180),&cancel).await?;
                computers.wake(&reference,&cancel).await?;
                computers.wait(&reference,WaitFor::Awake,Duration::from_secs(180),&cancel).await?;
            }
        }
        Ok(())
    }.await;
    let deleted = computers.delete(&reference, &cancel).await.is_ok()
        && computers
            .wait(
                &reference,
                WaitFor::Deleted,
                Duration::from_secs(180),
                &cancel,
            )
            .await
            .is_ok();
    report["failure"] = result
        .as_ref()
        .err()
        .map(|e| json!({"status":e.status(),"code":e.code()}))
        .unwrap_or(Value::Null);
    report["cleanup"] = json!(deleted);
    report["passed"] = json!(result.is_ok());
    std::fs::write(
        &workflow.evidence,
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert!(deleted, "The canary computer still needs cleanup.");
    assert!(
        result.is_ok(),
        "The live SDK credential workflow failed; inspect its scrubbed record."
    );
}

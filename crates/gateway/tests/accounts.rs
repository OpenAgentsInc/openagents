//! The account surface's end-to-end contract: self-serve sign-up,
//! sessions, workspaces and membership, scoped and rotated keys,
//! recovery, and the funded anonymous lane — over real HTTP against
//! the same in-process deployment the other gateway tests use.
//!
//! Every test stands up its own directory and listeners; nothing
//! shares state but the shape of the claims being checked.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::http::StatusCode;
use serde_json::{Value, json};
use tenancy::{Accounts, Registry, Sessions, keys};

use gateway::config::{self, Config, Door, SCHEMA};
use gateway::serve::{self, ServeState};

use common::*;

/// A deployed gateway with the account surface configured.
struct Deployment {
    /// The operator key issued for `acme` — bound to no account.
    tokens: BTreeMap<String, String>,
    dir: tempfile::TempDir,
    address: String,
    _state: Arc<ServeState>,
}

/// The accounts block most tests run: sign-up onto `acme`, the funded
/// lane bounded to a handful of requests.
fn account_config(anonymous: Option<config::Anonymous>) -> config::Accounts {
    config::Accounts {
        signup_tenant: Some("acme".to_string()),
        session_ttl_secs: 28_800,
        recovery_ttl_secs: 3_600,
        anonymous,
    }
}

/// The funded lane at its smallest honest bound.
fn funded(bound: u64, session_cap: u64) -> config::Anonymous {
    config::Anonymous {
        workspace: "public".to_string(),
        bound,
        session_cap,
        ttl_secs: 3_600,
    }
}

/// Stand the registry, the key store, and the service up — the account
/// stores install themselves on first open under the `accounts` block.
async fn deploy(accounts: Option<config::Accounts>, require_membership: bool) -> Deployment {
    let (endpoint, _forwards) = backend(&artifact('b'), StatusCode::OK, answer(), 0).await;
    let dir = tempfile::tempdir().unwrap();
    let manifest = manifest(&artifact('b'), None);
    let registry = Registry::install(dir.path(), manifest.clone()).unwrap();
    let mut tokens = BTreeMap::new();
    for tenant in manifest.tenants.keys() {
        let issued = keys::issue(dir.path(), registry.manifest(), tenant).unwrap();
        tokens.insert(tenant.clone(), issued.token);
    }
    let doors = ["shared-kev", "acme-kev"]
        .into_iter()
        .map(|door| {
            (
                door.to_string(),
                Door {
                    endpoint: endpoint.clone(),
                    classify: None,
                    classify_item_concurrency: 1,
                    batching: None,
                },
            )
        })
        .collect();
    let state = ServeState::open(Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: dir.path().to_path_buf(),
        require_workspace_membership: require_membership,
        accounts,
        money: None,
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        classify_timeout_ms: None,
        max_tenant_classify_in_flight: None,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_classify_inputs: 1024,
        max_classify_inputs_per_tenant: 1024,
        max_questions: 256,
        max_options: 4096,
        doors,
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
        billing: None,
        skills: None,
    })
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        tokens,
        dir,
        address,
        _state: state,
    }
}

/// A decision request body.
fn call(model: &str) -> Value {
    json!({
        "model": model,
        "state": "A member's private text.",
        "questions": {
            "q1": {"type": "noul", "instructions": "Is this about routing?", "criteria": "yes/no"},
        },
    })
}

/// Send a request that takes a body and return `(status, json)`.
async fn exchange(request: reqwest::RequestBuilder) -> (StatusCode, Value) {
    let response = request.send().await.unwrap();
    let status = response.status();
    let body = response.json().await.unwrap_or_default();
    (status, body)
}

/// POST a JSON body to `path`, with or without a bearer credential.
async fn post(
    deployment: &Deployment,
    path: &str,
    token: Option<&str>,
    body: &Value,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .post(format!("{}{path}", deployment.address))
        .json(body);
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    exchange(request).await
}

/// GET `path` with a bearer credential.
async fn get(deployment: &Deployment, path: &str, token: &str) -> (StatusCode, Value) {
    exchange(
        reqwest::Client::new()
            .get(format!("{}{path}", deployment.address))
            .bearer_auth(token),
    )
    .await
}

/// DELETE `path` with a bearer credential.
async fn remove(deployment: &Deployment, path: &str, token: &str) -> (StatusCode, Value) {
    exchange(
        reqwest::Client::new()
            .delete(format!("{}{path}", deployment.address))
            .bearer_auth(token),
    )
    .await
}

/// PATCH `path` with a bearer credential and a JSON body.
async fn patch(
    deployment: &Deployment,
    path: &str,
    token: &str,
    body: &Value,
) -> (StatusCode, Value) {
    exchange(
        reqwest::Client::new()
            .patch(format!("{}{path}", deployment.address))
            .bearer_auth(token)
            .json(body),
    )
    .await
}

/// A decision call under `token` naming `workspace`, when one is given.
async fn decide(
    deployment: &Deployment,
    token: Option<&str>,
    workspace: Option<&str>,
    door: &str,
) -> (StatusCode, Value) {
    let mut request = reqwest::Client::new()
        .post(format!("{}/v1/systemone", deployment.address))
        .json(&call(door));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    if let Some(workspace) = workspace {
        request = request.header("x-workspace-id", workspace);
    }
    exchange(request).await
}

/// Sign an account up and assert the whole onboarding answer.
async fn sign_up(deployment: &Deployment, label: &str) -> Value {
    let (status, body) = post(deployment, "/v1/accounts", None, &json!({"label": label})).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

/// The account id, workspace id, key secret, and session secret of a
/// sign-up answer.
struct Joined {
    account: String,
    workspace: String,
    key_token: String,
    session_token: String,
}

async fn join(deployment: &Deployment, label: &str) -> Joined {
    let body = sign_up(deployment, label).await;
    Joined {
        account: body["account"]["id"].as_str().unwrap().to_string(),
        workspace: body["workspace"]["id"].as_str().unwrap().to_string(),
        key_token: body["key_token"].as_str().unwrap().to_string(),
        session_token: body["session_token"].as_str().unwrap().to_string(),
    }
}

/// The error code a refusal envelope carries.
fn code(body: &Value) -> &str {
    body["error"]["code"].as_str().unwrap_or_default()
}

#[tokio::test]
async fn sign_up_reaches_the_first_call_and_every_secret_leaves_once() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let joined = join(&deployment, "ada").await;
    assert!(joined.key_token.starts_with("oak_"));
    assert!(joined.session_token.starts_with("sess_"));

    // The session the sign-up minted is the onboarding call's
    // credential: one header chooses the workspace, the workspace
    // resolves the tenant, the door answers.
    let (status, body) = decide(
        &deployment,
        Some(&joined.session_token),
        Some(&joined.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["answers"]["q1"].is_object());

    // The account view is the workspace-switching menu.
    let (status, body) = get(&deployment, "/v1/account", &joined.session_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["account"]["id"], json!(joined.account));
    assert_eq!(
        body["workspaces"][0]["id"].as_str().unwrap(),
        joined.workspace
    );

    // Session status describes the record, never the token.
    let (status, body) = get(&deployment, "/v1/session", &joined.session_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["session"]["kind"], json!("user"));
    assert_eq!(body["session"]["state"], json!("active"));
    assert!(!body.to_string().contains(&joined.session_token));
}

#[tokio::test]
async fn sign_in_logout_and_session_standing() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let joined = join(&deployment, "ada").await;

    // A session token is not a sign-in credential.
    let (status, body) = post(
        &deployment,
        "/v1/sessions",
        Some(&joined.session_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(code(&body), "already_signed_in");

    // An unbound operator key is a credential with no account surface.
    let (status, body) = post(
        &deployment,
        "/v1/sessions",
        Some(&deployment.tokens["acme"]),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "no_account");

    // The key signs its account in.
    let (status, body) = post(
        &deployment,
        "/v1/sessions",
        Some(&joined.key_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second = body["token"].as_str().unwrap().to_string();
    assert_ne!(second, joined.session_token);

    // Logout ends the session it names — its next answer is the state.
    let (status, _) = remove(&deployment, "/v1/session", &second).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = get(&deployment, "/v1/session", &second).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(code(&body), "session_closed");
    let (status, body) = decide(
        &deployment,
        Some(&second),
        Some(&joined.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(code(&body), "session_closed");

    // An `oak_` key has no session to describe or end.
    let (status, body) = get(&deployment, "/v1/session", &joined.key_token).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(code(&body), "not_a_session");
    let (status, body) = remove(&deployment, "/v1/session", &joined.key_token).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(code(&body), "not_a_session");

    // A made-up token and a dead key are the same refusal shape.
    let (status, body) = get(&deployment, "/v1/session", "sess_deadbeef").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(code(&body), "unauthenticated");
}

#[tokio::test]
async fn two_users_two_workspaces_and_the_isolation_between_them() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    let grace = join(&deployment, "grace").await;

    // Grace cannot read, rename, or call into Ada's workspace.
    let (status, body) = get(
        &deployment,
        &format!("/v1/workspaces/{}", ada.workspace),
        &grace.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "not_member");
    let (status, body) = patch(
        &deployment,
        &format!("/v1/workspaces/{}", ada.workspace),
        &grace.session_token,
        &json!({"name": "taken"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = decide(
        &deployment,
        Some(&grace.session_token),
        Some(&ada.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "workspace_forbidden");

    // And each reaches their own.
    for joined in [&ada, &grace] {
        let (status, _) = decide(
            &deployment,
            Some(&joined.session_token),
            Some(&joined.workspace),
            "acme-kev",
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    // The header names exactly one workspace.
    let (status, body) = decide(&deployment, Some(&ada.session_token), None, "acme-kev").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(code(&body), "workspace_required");
    let response = reqwest::Client::new()
        .post(format!("{}/v1/systemone", deployment.address))
        .bearer_auth(&ada.session_token)
        .header("x-workspace-id", &ada.workspace)
        .header("x-workspace-id", &grace.workspace)
        .json(&call("acme-kev"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let (status, body) = decide(
        &deployment,
        Some(&ada.session_token),
        Some("ws_nope"),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(code(&body), "unknown_workspace");
}

#[tokio::test]
async fn organization_workspace_invitation_and_membership() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    let grace = join(&deployment, "grace").await;

    let (status, body) = post(
        &deployment,
        "/v1/workspaces",
        Some(&ada.session_token),
        &json!({"name": "team"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let team = body["workspace"]["id"].as_str().unwrap().to_string();
    assert_eq!(body["workspace"]["kind"], json!("organization"));
    assert_eq!(body["workspace"]["role"], json!("owner"));

    // Members cannot mint invitations — only admins and the owner.
    let invite_path = format!("/v1/workspaces/{team}/invitations");
    let (status, body) = post(
        &deployment,
        &invite_path,
        Some(&grace.session_token),
        &json!({"role": "member"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");

    // Ownership never moves through an invitation.
    let (status, body) = post(
        &deployment,
        &invite_path,
        Some(&ada.session_token),
        &json!({"role": "owner"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(code(&body), "owner_by_invitation");

    let (status, body) = post(
        &deployment,
        &invite_path,
        Some(&ada.session_token),
        &json!({"role": "member"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let invitation = body["token"].as_str().unwrap().to_string();
    assert!(invitation.starts_with("inv_"));

    // Grace accepts — the token carries the workspace it joins.
    let (status, body) = post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": invitation}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["membership"]["workspace"], json!(team));
    assert_eq!(body["membership"]["role"], json!("member"));

    // Grace's session reaches the team's door; her account lists it.
    let (status, body) = decide(
        &deployment,
        Some(&grace.session_token),
        Some(&team),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = get(&deployment, "/v1/account", &grace.session_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let workspaces: Vec<&str> = body["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|workspace| workspace["id"].as_str())
        .collect();
    assert!(workspaces.contains(&team.as_str()));
    assert!(workspaces.contains(&grace.workspace.as_str()));

    // The same token cannot be accepted twice.
    let (status, body) = post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": invitation}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // A member reads the workspace but not its invitation list.
    let (status, body) = get(
        &deployment,
        &format!("/v1/workspaces/{team}"),
        &grace.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["invitations"].is_null());
    assert_eq!(body["role"], json!("member"));
    let (status, body) = get(
        &deployment,
        &format!("/v1/workspaces/{team}"),
        &ada.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["invitations"].as_array().unwrap().len(), 1);
}

#[tokio::test]
async fn invitations_expire_and_revoked_ones_close() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    let grace = join(&deployment, "grace").await;
    let team = post(
        &deployment,
        "/v1/workspaces",
        Some(&ada.session_token),
        &json!({"name": "team"}),
    )
    .await
    .1["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let invite_path = format!("/v1/workspaces/{team}/invitations");

    // A one-second invitation dies before it is accepted.
    let (status, body) = post(
        &deployment,
        &invite_path,
        Some(&ada.session_token),
        &json!({"role": "member", "ttl_secs": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let dying = body["token"].as_str().unwrap().to_string();
    tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
    let (status, body) = post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": dying}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "invitation_expired");

    // A revoked invitation closes rather than vanishing.
    let (status, body) = post(
        &deployment,
        &invite_path,
        Some(&ada.session_token),
        &json!({"role": "member"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let withdrawn = body["token"].as_str().unwrap().to_string();
    let id = body["invitation"]["id"].as_str().unwrap().to_string();
    let (status, body) = remove(
        &deployment,
        &format!("{invite_path}/{id}"),
        &ada.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": withdrawn}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // A malformed or foreign token is its own refusal.
    let (status, body) = post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": "inv_forged.token"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(code(&body), "malformed_invitation");
}

/// Mint a `member` invitation on `team` for `token`'s holder.
async fn invite(deployment: &Deployment, team: &str, token: &str) -> (StatusCode, Value) {
    post(
        deployment,
        &format!("/v1/workspaces/{team}/invitations"),
        Some(token),
        &json!({"role": "member"}),
    )
    .await
}

#[tokio::test]
async fn role_matrix_and_seat_enforcement() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    let grace = join(&deployment, "grace").await;
    let (status, body) = post(
        &deployment,
        "/v1/workspaces",
        Some(&ada.session_token),
        &json!({"name": "team", "seats": 2}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let team = body["workspace"]["id"].as_str().unwrap().to_string();

    // Seats count the owner plus live invitations: one seat is left.
    let (status, body) = invite(&deployment, &team, &ada.session_token).await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let first = body["token"].as_str().unwrap().to_string();
    let (status, body) = invite(&deployment, &team, &ada.session_token).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(code(&body), "seat_limit");

    // Widening seats opens the seat again.
    let (status, _) = patch(
        &deployment,
        &format!("/v1/workspaces/{team}"),
        &ada.session_token,
        &json!({"seats": 3}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = invite(&deployment, &team, &ada.session_token).await;
    assert_eq!(status, StatusCode::CREATED);

    // Grace joins as a member — two members are active now, and seats
    // cannot shrink beneath them.
    let (status, _) = post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": first}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = patch(
        &deployment,
        &format!("/v1/workspaces/{team}"),
        &ada.session_token,
        &json!({"seats": 1}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(code(&body), "seats_below_members");

    // Members touch nothing administrative.
    for (status, _) in [
        invite(&deployment, &team, &grace.session_token).await,
        patch(
            &deployment,
            &format!("/v1/workspaces/{team}"),
            &grace.session_token,
            &json!({"name": "mutiny"}),
        )
        .await,
        patch(
            &deployment,
            &format!("/v1/workspaces/{team}/members/{}", ada.account),
            &grace.session_token,
            &json!({"role": "member"}),
        )
        .await,
        post(
            &deployment,
            &format!("/v1/workspaces/{team}/transfer"),
            Some(&grace.session_token),
            &json!({"account": grace.account}),
        )
        .await,
        post(
            &deployment,
            &format!("/v1/workspaces/{team}/recovery"),
            Some(&grace.session_token),
            &json!({"account": ada.account}),
        )
        .await,
    ] {
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    // Promotion to admin opens invitations — role changes are the
    // owner's alone, so the promotion is the owner calling `set_role`.
    let (status, body) = patch(
        &deployment,
        &format!("/v1/workspaces/{team}/members/{}", grace.account),
        &ada.session_token,
        &json!({"role": "admin"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["membership"]["role"], json!("admin"));
    // The second live invitation still holds the third seat — the
    // admin's invite refuses on seats, then lands once the owner
    // widens the bound.
    let (status, body) = invite(&deployment, &team, &grace.session_token).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(code(&body), "seat_limit");
    let (status, _) = patch(
        &deployment,
        &format!("/v1/workspaces/{team}"),
        &ada.session_token,
        &json!({"seats": 4}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = invite(&deployment, &team, &grace.session_token).await;
    assert_eq!(status, StatusCode::CREATED);

    // An admin still cannot change roles — even the owner's — and an
    // admin removes only members, never the owner or another admin.
    let (status, _) = patch(
        &deployment,
        &format!("/v1/workspaces/{team}/members/{}", ada.account),
        &grace.session_token,
        &json!({"role": "member"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = remove(
        &deployment,
        &format!("/v1/workspaces/{team}/members/{}", ada.account),
        &grace.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(code(&body), "last_owner");
}

#[tokio::test]
async fn last_owner_protection_and_ownership_transfer() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    let grace = join(&deployment, "grace").await;
    let team = post(
        &deployment,
        "/v1/workspaces",
        Some(&ada.session_token),
        &json!({"name": "team"}),
    )
    .await
    .1["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let token = post(
        &deployment,
        &format!("/v1/workspaces/{team}/invitations"),
        Some(&ada.session_token),
        &json!({"role": "admin"}),
    )
    .await
    .1["token"]
        .as_str()
        .unwrap()
        .to_string();
    post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": token}),
    )
    .await;

    // The last owner can neither leave nor step down.
    let (status, body) = remove(
        &deployment,
        &format!("/v1/workspaces/{team}/members/{}", ada.account),
        &ada.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(code(&body), "last_owner");
    let (status, body) = patch(
        &deployment,
        &format!("/v1/workspaces/{team}/members/{}", ada.account),
        &ada.session_token,
        &json!({"role": "member"}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");

    // Transfer moves ownership; the previous owner lands as admin.
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{team}/transfer"),
        Some(&ada.session_token),
        &json!({"account": grace.account}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["owner"], json!(grace.account));
    let (status, body) = get(
        &deployment,
        &format!("/v1/workspaces/{team}"),
        &ada.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], json!("admin"));

    // The new owner may now remove the old one — the membership
    // revokes rather than vanishing.
    let (status, body) = remove(
        &deployment,
        &format!("/v1/workspaces/{team}/members/{}", ada.account),
        &grace.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["membership"]["status"], json!("revoked"));
    let (status, body) = decide(
        &deployment,
        Some(&ada.session_token),
        Some(&team),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(code(&body), "session_closed");
}

#[tokio::test]
async fn member_removal_ends_sessions_and_closes_keys_to_the_workspace() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    let grace = join(&deployment, "grace").await;
    let team = post(
        &deployment,
        "/v1/workspaces",
        Some(&ada.session_token),
        &json!({"name": "team"}),
    )
    .await
    .1["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let token = post(
        &deployment,
        &format!("/v1/workspaces/{team}/invitations"),
        Some(&ada.session_token),
        &json!({"role": "member"}),
    )
    .await
    .1["token"]
        .as_str()
        .unwrap()
        .to_string();
    post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": token}),
    )
    .await;
    let (status, _) = decide(
        &deployment,
        Some(&grace.session_token),
        Some(&team),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = remove(
        &deployment,
        &format!("/v1/workspaces/{team}/members/{}", grace.account),
        &ada.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // The session ends in the same call — the very next request sees it.
    let (status, body) = get(&deployment, "/v1/session", &grace.session_token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(code(&body), "session_closed");
    let (status, body) = decide(
        &deployment,
        Some(&grace.session_token),
        Some(&team),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");

    // The key refuses the workspace on its next authentication — the
    // membership read is fresh, never cached.
    let (status, body) = decide(&deployment, Some(&grace.key_token), Some(&team), "acme-kev").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "workspace_forbidden");

    // The account survives — Grace signs back in and reaches her own
    // workspace, never the team's.
    let (status, body) = post(
        &deployment,
        "/v1/sessions",
        Some(&grace.key_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let renewed = body["token"].as_str().unwrap().to_string();
    let (status, _) = decide(
        &deployment,
        Some(&renewed),
        Some(&grace.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = decide(&deployment, Some(&renewed), Some(&team), "acme-kev").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "workspace_forbidden");
}

#[tokio::test]
async fn named_scoped_keys_and_their_lifecycle() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    let grace = join(&deployment, "grace").await;
    let keys_path = format!("/v1/workspaces/{}/keys", ada.workspace);

    // A named, scoped key: one door, one action.
    let (status, body) = post(
        &deployment,
        &keys_path,
        Some(&ada.session_token),
        &json!({
            "name": "ci",
            "scopes": {"models": ["acme-kev"], "actions": ["inference"]},
        }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let scoped_id = body["key"]["id"].as_str().unwrap().to_string();
    let scoped_token = body["key_token"].as_str().unwrap().to_string();
    assert!(scoped_token.starts_with("oak_"));
    assert_eq!(body["key"]["name"], json!("ci"));

    // The scope narrows: the named door answers, everything else refuses.
    let (status, _) = decide(
        &deployment,
        Some(&scoped_token),
        Some(&ada.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = decide(
        &deployment,
        Some(&scoped_token),
        Some(&ada.workspace),
        "shared-kev",
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "out_of_scope");
    let (status, body) = get(&deployment, "/v1/account", &scoped_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "out_of_scope");

    // Pause holds without ending; resume releases it.
    let (status, body) = post(
        &deployment,
        &format!("{keys_path}/{scoped_id}/pause"),
        Some(&ada.session_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["key"]["status"], json!("paused"));
    let (status, body) = decide(
        &deployment,
        Some(&scoped_token),
        Some(&ada.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    let (status, _) = post(
        &deployment,
        &format!("{keys_path}/{scoped_id}/resume"),
        Some(&ada.session_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = decide(
        &deployment,
        Some(&scoped_token),
        Some(&ada.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Rotation mints a new id and secret for the same reach — the old
    // token is dead, the new one resolves to the same account, and the
    // membership never moved.
    let (status, body) = post(
        &deployment,
        &format!("{keys_path}/{scoped_id}/rotate"),
        Some(&ada.session_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let rotated_id = body["key"]["id"].as_str().unwrap().to_string();
    let rotated_token = body["key_token"].as_str().unwrap().to_string();
    assert_ne!(rotated_id, scoped_id);
    assert_eq!(body["key"]["rotated_from"], json!(scoped_id));
    let (status, _) = decide(
        &deployment,
        Some(&scoped_token),
        Some(&ada.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, _) = decide(
        &deployment,
        Some(&rotated_token),
        Some(&ada.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Rotation carried the binding to the same account — the rotated
    // key resolves Ada's membership wherever the workspace asks.
    let (status, body) = get(&deployment, &keys_path, &ada.session_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let record = body["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|key| key["id"] == json!(rotated_id))
        .unwrap()
        .clone();
    assert_eq!(record["account"], json!(ada.account));

    // A member cannot touch another member's key: Grace joins a team
    // workspace as a member, and the rotated key — bound to Ada's
    // account on the shared tenant — still refuses her.
    let team = post(
        &deployment,
        "/v1/workspaces",
        Some(&ada.session_token),
        &json!({"name": "team"}),
    )
    .await
    .1["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let token = invite(&deployment, &team, &ada.session_token).await.1["token"]
        .as_str()
        .unwrap()
        .to_string();
    post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": token}),
    )
    .await;
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{team}/keys/{rotated_id}/pause"),
        Some(&grace.session_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "forbidden");

    // Revocation ends the credential; the record stays.
    let (status, _) = remove(
        &deployment,
        &format!("{keys_path}/{rotated_id}"),
        &ada.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = decide(
        &deployment,
        Some(&rotated_token),
        Some(&ada.workspace),
        "acme-kev",
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = get(&deployment, &keys_path, &ada.session_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let listed: Vec<&Value> = body["keys"].as_array().unwrap().iter().collect();
    let record = listed
        .iter()
        .find(|key| key["id"] == json!(rotated_id))
        .unwrap();
    assert_eq!(record["status"], json!("revoked"));
    assert_eq!(record["account"], json!(ada.account));
    // No secret field exists in any record the list returns.
    assert!(!body.to_string().contains(&rotated_token));
    assert!(!body.to_string().contains(&ada.key_token));
}

#[tokio::test]
async fn recovery_rotates_the_bound_key_and_ends_old_sessions() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    let grace = join(&deployment, "grace").await;
    let team = post(
        &deployment,
        "/v1/workspaces",
        Some(&ada.session_token),
        &json!({"name": "team"}),
    )
    .await
    .1["workspace"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let token = post(
        &deployment,
        &format!("/v1/workspaces/{team}/invitations"),
        Some(&ada.session_token),
        &json!({"role": "member"}),
    )
    .await
    .1["token"]
        .as_str()
        .unwrap()
        .to_string();
    post(
        &deployment,
        "/v1/invitations/accept",
        Some(&grace.session_token),
        &json!({"token": token}),
    )
    .await;

    // An admin — the owner here — issues the recovery token; the
    // member redeems it without any other credential.
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{team}/recovery"),
        Some(&grace.session_token),
        &json!({"account": grace.account}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) = post(
        &deployment,
        &format!("/v1/workspaces/{team}/recovery"),
        Some(&ada.session_token),
        &json!({"account": grace.account}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let recovery = body["token"].as_str().unwrap().to_string();
    assert!(recovery.starts_with("rcv_"));

    let (status, body) = post(
        &deployment,
        "/v1/recovery/redeem",
        None,
        &json!({"token": recovery}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["account"], json!(grace.account));
    let fresh = body["key_token"].as_str().unwrap().to_string();

    // The old key is dead, the sessions it minted ended, the new key
    // resolves the same account — and the token is single-use.
    let (status, _) = post(
        &deployment,
        "/v1/sessions",
        Some(&grace.key_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let (status, body) = get(&deployment, "/v1/session", &grace.session_token).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(code(&body), "session_closed");
    let (status, body) = get(&deployment, "/v1/account", &fresh).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["account"]["id"], json!(grace.account));
    let (status, body) = post(
        &deployment,
        "/v1/recovery/redeem",
        None,
        &json!({"token": recovery}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(code(&body), "recovery_closed");
    let (status, body) = post(
        &deployment,
        "/v1/recovery/redeem",
        None,
        &json!({"token": "rcv_forged"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "invalid_recovery");
}

#[tokio::test]
async fn anonymous_lane_is_bounded_funded_and_counted() {
    let deployment = deploy(Some(account_config(Some(funded(4, 2)))), false).await;

    // No credential at all mints the funded session.
    let (status, body) = post(&deployment, "/v1/sessions", None, &json!({})).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["session"]["kind"], json!("anonymous"));
    let anonymous = body["token"].as_str().unwrap().to_string();

    let (status, body) = get(&deployment, "/v1/session", &anonymous).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["session"]["kind"], json!("anonymous"));
    assert_eq!(body["budget"]["remaining"], json!(4));
    assert_eq!(body["budget"]["session_cap"], json!(2));

    // The funded lane reaches shared doors only — and holds no
    // account surface at all.
    let (status, _) = decide(&deployment, Some(&anonymous), None, "shared-kev").await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = get(&deployment, "/v1/account", &anonymous).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "membership_required");

    // One session draws to its cap, then names the cap.
    let (status, _) = decide(&deployment, Some(&anonymous), None, "shared-kev").await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = decide(&deployment, Some(&anonymous), None, "shared-kev").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "anonymous_session_capped");

    // A second session draws the rest of the bound; the next name it.
    let (_, body) = post(&deployment, "/v1/sessions", None, &json!({})).await;
    let second = body["token"].as_str().unwrap().to_string();
    let (status, _) = decide(&deployment, Some(&second), None, "shared-kev").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = decide(&deployment, Some(&second), None, "shared-kev").await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = decide(&deployment, Some(&second), None, "shared-kev").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "anonymous_budget_exhausted");
}

#[tokio::test]
async fn anonymous_sessions_answer_nothing_under_membership() {
    let deployment = deploy(Some(account_config(Some(funded(4, 2)))), true).await;
    let (_, body) = post(&deployment, "/v1/sessions", None, &json!({})).await;
    let anonymous = body["token"].as_str().unwrap().to_string();
    let (status, body) = decide(&deployment, Some(&anonymous), None, "shared-kev").await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "workspace_required");
}

#[tokio::test]
async fn signup_and_anonymous_off_when_not_configured() {
    let deployment = deploy(
        Some(config::Accounts {
            signup_tenant: None,
            session_ttl_secs: 28_800,
            recovery_ttl_secs: 3_600,
            anonymous: None,
        }),
        false,
    )
    .await;
    let (status, body) = post(&deployment, "/v1/accounts", None, &json!({"label": "ada"})).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "signup_disabled");
    let (status, body) = post(&deployment, "/v1/sessions", None, &json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(code(&body), "anonymous_disabled");
}

#[tokio::test]
async fn access_history_keeps_references_and_redacts_secrets() {
    let deployment = deploy(Some(account_config(None)), true).await;
    let ada = join(&deployment, "ada").await;
    post(
        &deployment,
        "/v1/sessions",
        Some(&ada.key_token),
        &json!({}),
    )
    .await;
    let (status, body) = get(&deployment, "/v1/account/access", &ada.session_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let events = body["access"].as_array().unwrap();
    let actions: Vec<&str> = events
        .iter()
        .filter_map(|event| event["action"].as_str())
        .collect();
    assert!(actions.contains(&"sign-up"), "{actions:?}");
    assert!(actions.contains(&"sign-in"), "{actions:?}");
    // Every event names its actor and never a secret — the history is
    // attribution, not a credential store.
    for event in events {
        assert_eq!(event["actor"], json!(ada.account));
    }
    let text = body.to_string();
    for secret in [&ada.session_token, &ada.key_token] {
        assert!(!text.contains(secret), "access history leaked a secret");
    }

    // A second user sees only their own events.
    let grace = join(&deployment, "grace").await;
    let (status, body) = get(&deployment, "/v1/account/access", &grace.session_token).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for event in body["access"].as_array().unwrap() {
        assert_eq!(event["actor"], json!(grace.account));
    }
}

#[tokio::test]
async fn stores_install_under_accounts_config_and_validate() {
    // The empty genesis installs on first open — both stores, beside
    // the registry, before any route answers.
    let deployment = deploy(Some(account_config(None)), false).await;
    assert!(Accounts::open(deployment.dir.path()).is_ok());
    assert!(Sessions::open(deployment.dir.path()).is_ok());

    // The funded lane's declared bounds are checked, not trusted.
    let path = deployment.dir.path().join("gateway.json");
    let mut config = Config {
        v: SCHEMA.to_string(),
        listen: "127.0.0.1:0".to_string(),
        registry: deployment.dir.path().to_path_buf(),
        require_workspace_membership: false,
        accounts: Some(config::Accounts {
            signup_tenant: Some("acme".to_string()),
            session_ttl_secs: 28_800,
            recovery_ttl_secs: 3_600,
            anonymous: Some(config::Anonymous {
                workspace: "public".to_string(),
                bound: 0,
                session_cap: 1,
                ttl_secs: 3_600,
            }),
        }),
        money: None,
        max_body_bytes: 1_048_576,
        max_response_bytes: 4_194_304,
        forward_timeout_ms: 10_000,
        classify_timeout_ms: None,
        max_tenant_classify_in_flight: None,
        reservation_ttl_secs: 300,
        max_in_flight: 8,
        max_classify_inputs: 1024,
        max_classify_inputs_per_tenant: 1024,
        max_questions: 256,
        max_options: 4096,
        doors: BTreeMap::new(),
        job_retention_ms: 604_800_000,
        job_cursor_ttl_ms: 3_600_000,
        public_origin: None,
        billing: None,
        skills: None,
    };
    assert!(config.check(&path).is_err());
    config
        .accounts
        .as_mut()
        .unwrap()
        .anonymous
        .as_mut()
        .unwrap()
        .bound = 4;
    config
        .accounts
        .as_mut()
        .unwrap()
        .anonymous
        .as_mut()
        .unwrap()
        .session_cap = 8;
    assert!(config.check(&path).is_err());
    config.accounts.as_mut().unwrap().session_ttl_secs = 0;
    config
        .accounts
        .as_mut()
        .unwrap()
        .anonymous
        .as_mut()
        .unwrap()
        .session_cap = 2;
    assert!(config.check(&path).is_err());
}

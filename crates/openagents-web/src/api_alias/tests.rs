use axum::http::Method;

use super::forwards;

/// The INTERNAL routes docs/api/design.md names (section 6, item 1), and
/// FIRST-PARTY ones the website reaches over the loopback.
pub(crate) const REFUSED: &[(&str, &str)] = &[
    ("GET", "/v1/admin/inference/status"),
    ("GET", "/admin/inference"),
    ("POST", "/admin/inference/session"),
    ("GET", "/dashboard"),
    ("GET", "/playground"),
    ("POST", "/v1/accounts"),
    ("POST", "/v1/sessions/github"),
    ("POST", "/v1/account/github/token"),
    ("POST", "/v1/sessions/device/lookup"),
    ("POST", "/v1/sessions/device/decide"),
    ("POST", "/v1/sessions/device/paired"),
    ("POST", "/v1/sessions/device"),
    ("POST", "/v1/sessions/device/poll"),
    ("POST", "/v1/sessions"),
    ("GET", "/v1/session"),
    ("POST", "/v1/account/identities/github"),
    ("POST", "/v1/account/github/broker"),
    ("POST", "/v1/github/git-credential"),
    ("POST", "/v1/projects"),
    ("DELETE", "/v1/account/projects/pr_1"),
    ("POST", "/v1/billing/webhook"),
    ("POST", "/v1/workspaces/ws_1/billing/reconcile"),
    ("POST", "/v1/workspaces/ws_1/recovery"),
    ("POST", "/v1/systemone"),
    ("POST", "/v1/classify"),
    ("GET", "/v1/jobs"),
    ("POST", "/v1/feedback"),
    ("GET", "/v1/docs"),
    ("GET", "/healthz"),
    ("GET", "/v1/earnings"),
    // Tricks that must not reach a public pattern's neighbor.
    ("GET", "/v1/workspaces/ws_1/keys/../../../admin/inference"),
    ("GET", "/v1/models/../admin/inference/status"),
    ("GET", "/v1//account"),
];

pub(crate) const PUBLIC: &[(&str, &str)] = &[
    ("POST", "/v1/responses"),
    ("GET", "/v1/responses"),
    ("POST", "/v1/responses/compact"),
    ("GET", "/v1/responses/resp_1"),
    ("POST", "/v1/chat/completions"),
    ("GET", "/v1/models"),
    ("HEAD", "/v1/models"),
    ("GET", "/v1/rates"),
    ("GET", "/v1/openapi.json"),
    ("GET", "/v1/key"),
    ("GET", "/v1/usage/req_1"),
    ("GET", "/v1/balance"),
    ("GET", "/v1/account"),
    ("GET", "/v1/account/access"),
    ("POST", "/v1/workspaces"),
    ("GET", "/v1/workspaces/ws_1"),
    ("GET", "/v1/workspaces/ws_1/keys"),
    ("POST", "/v1/workspaces/ws_1/keys/key_1/rotate"),
    ("DELETE", "/v1/workspaces/ws_1/keys/key_1"),
    ("PUT", "/v1/workspaces/ws_1/keys/key_1/limits"),
    ("PUT", "/v1/workspaces/ws_1/provider-keys/openrouter"),
    ("POST", "/v1/workspaces/ws_1/invitations"),
    ("PATCH", "/v1/workspaces/ws_1/members/acct_1"),
    ("GET", "/v1/workspaces/ws_1/usage/activity"),
    ("GET", "/v1/workspaces/ws_1/balance"),
    ("POST", "/v1/workspaces/ws_1/topups/decision"),
    ("POST", "/v1/workspaces/ws_1/decision-funding/decision"),
    ("GET", "/v1/workspaces/ws_1/sso/audit"),
    ("POST", "/v1/invitations/accept"),
    ("OPTIONS", "/v1/responses"),
];

fn method(name: &str) -> Method {
    name.parse().unwrap()
}

#[test]
fn only_public_routes_go_through() {
    for (m, path) in PUBLIC {
        assert!(forwards(&method(m), path, false), "{m} {path}");
    }
    for (m, path) in REFUSED {
        assert!(!forwards(&method(m), path, false), "{m} {path}");
    }
}

#[test]
fn a_public_path_with_another_method_is_refused() {
    assert!(!forwards(&Method::DELETE, "/v1/models", false));
    assert!(!forwards(&Method::POST, "/v1/account", false));
    assert!(!forwards(&Method::DELETE, "/v1/workspaces/ws_1", false));
}

#[test]
fn operator_signup_opens_only_post_v1_accounts_and_sessions() {
    assert!(forwards(&Method::POST, "/v1/accounts", true));
    assert!(forwards(&Method::POST, "/v1/sessions", true));
    assert!(!forwards(&Method::POST, "/v1/sessions", false));
    assert!(!forwards(&Method::GET, "/v1/sessions", true));
    assert!(!forwards(&Method::POST, "/v1/sessions/device", true));
    assert!(!forwards(&Method::GET, "/v1/accounts", true));
    assert!(!forwards(&Method::POST, "/v1/sessions/github", true));
    assert!(!forwards(&Method::GET, "/v1/admin/inference/status", true));
}

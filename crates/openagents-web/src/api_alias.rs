//! Which gateway routes `openagents.com/api/v1/...` forwards (#11155).
//!
//! The alias is a public front: it forwards only the routes
//! `docs/api/design.md` marks PUBLIC. FIRST-PARTY and INTERNAL routes (the
//! inference admin page and meter, operator sign-up, the website-only
//! GitHub token, device approval, sessions, the dashboard and playground,
//! the decision service) answer `404` from outside; the website reaches
//! them over the loopback, never through this alias. Anything not listed
//! here, including every gateway route added later, is refused by default.
//!
//! `operator_signup` (staging only, `OPENAGENTS_WEB_API_OPERATOR_SIGNUP`)
//! also forwards `POST /v1/accounts` carrying a bearer, so the staging
//! smoke suite can make its one test account with its operator token, and
//! `POST /v1/sessions` (a session for the bearer key's own account), so it
//! can sign in as its fixed agent-work test account (#11162).

use axum::http::Method;

/// Methods a rule allows.
enum Methods {
    Any,
    Only(&'static [&'static str]),
}

/// One rule: a path pattern, `*` for one segment, and a trailing `**` for
/// any number (zero or more) of further segments.
struct Rule(Methods, &'static str);

const GET: Methods = Methods::Only(&["GET"]);
const POST: Methods = Methods::Only(&["POST"]);

/// The PUBLIC gateway routes (docs/api/design.md, section 4).
const PUBLIC: &[Rule] = &[
    // Inference (4.7).
    Rule(Methods::Only(&["GET", "POST"]), "/v1/responses"),
    Rule(POST, "/v1/responses/compact"),
    Rule(Methods::Only(&["GET", "DELETE"]), "/v1/responses/*"),
    Rule(POST, "/v1/chat/completions"),
    Rule(GET, "/v1/models"),
    Rule(GET, "/v1/models/*"),
    Rule(GET, "/v1/rates"),
    Rule(GET, "/v1/openapi.json"),
    // Usage and money (4.5, 4.6).
    Rule(GET, "/v1/key"),
    Rule(GET, "/v1/usage/*"),
    Rule(GET, "/v1/balance"),
    // Identity and workspaces (4.1, 4.2).
    Rule(GET, "/v1/account"),
    Rule(GET, "/v1/account/access"),
    Rule(POST, "/v1/workspaces"),
    Rule(Methods::Only(&["GET", "PATCH"]), "/v1/workspaces/*"),
    Rule(Methods::Any, "/v1/workspaces/*/keys/**"),
    Rule(Methods::Any, "/v1/workspaces/*/provider-keys/**"),
    Rule(Methods::Any, "/v1/workspaces/*/invitations/**"),
    Rule(
        Methods::Only(&["PATCH", "DELETE"]),
        "/v1/workspaces/*/members/*",
    ),
    Rule(POST, "/v1/workspaces/*/transfer"),
    Rule(GET, "/v1/workspaces/*/access"),
    Rule(
        Methods::Only(&["GET", "PUT"]),
        "/v1/workspaces/*/team-policy",
    ),
    Rule(Methods::Any, "/v1/workspaces/*/sso/**"),
    Rule(GET, "/v1/workspaces/*/usage/**"),
    Rule(GET, "/v1/workspaces/*/balance"),
    Rule(POST, "/v1/workspaces/*/topups/*"),
    Rule(POST, "/v1/workspaces/*/decision-funding/*"),
    Rule(Methods::Only(&["GET", "PUT"]), "/v1/workspaces/*/budgets"),
    Rule(GET, "/v1/workspaces/*/audit"),
    Rule(POST, "/v1/invitations/accept"),
    Rule(POST, "/v1/invitations/accept-reviewed"),
];

fn matches(pattern: &str, path: &str) -> bool {
    let mut want = pattern.split('/');
    let mut have = path.split('/');
    loop {
        match (want.next(), have.next()) {
            (Some("**"), _) => return true,
            (None, None) => return true,
            (Some("*"), Some(segment)) if !segment.is_empty() => {}
            (Some(a), Some(b)) if a == b => {}
            _ => return false,
        }
    }
}

fn allows(methods: &Methods, method: &Method) -> bool {
    match methods {
        Methods::Any => true,
        Methods::Only(list) => {
            let name = if method == Method::HEAD {
                "GET"
            } else {
                method.as_str()
            };
            list.contains(&name)
        }
    }
}

/// Whether the alias forwards `method path` (the gateway path, `/v1/...`,
/// without its query). A CORS preflight (`OPTIONS`) goes through for any
/// public path.
#[must_use]
pub(crate) fn forwards(method: &Method, path: &str, operator_signup: bool) -> bool {
    if path.contains("/../") || path.ends_with("/..") || path.contains("//") {
        return false;
    }
    if operator_signup && method == Method::POST && matches!(path, "/v1/accounts" | "/v1/sessions")
    {
        return true;
    }
    PUBLIC.iter().any(|Rule(methods, pattern)| {
        matches(pattern, path) && (method == Method::OPTIONS || allows(methods, method))
    })
}

#[cfg(test)]
pub(crate) mod tests;

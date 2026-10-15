//! CAD Lab is part of Pro (#11693).
//!
//! Every action that designs, edits, measures, renders or exports a part
//! asks [`plan`] first, on the server: the `/cad` page's tool, a design's
//! edit controls and their routes (`pull`, `import`, `send`,
//! `configurator`, the print and CNC checks), and the chat's `cad_*`
//! tools, which a non-Pro chat turn is simply not offered (the model is
//! told once, in a note, to say CAD Lab is part of Pro).
//!
//! **Who passes.** A signed-in account on Pro (the plan meter's paid or
//! trial month, the same check `/pricing` makes: [`crate::plan::on_pro`]),
//! a site admin, or an account named in [`TEST_ACCOUNTS_ENV`] (the
//! production test account, so checks can use Pro without a payment).
//! A test run (`X-OpenAgents-Test: 1`, [`crate::test_runs`]) that also
//! sends [`TEST_PLAN_HEADER`]`: free` is treated as Free, so the same
//! account can check both sides; it only ever takes Pro away.
//!
//! A server that signs no one in (local development) has no plans: CAD
//! Lab is open there.
//!
//! **What stays open.** A design someone already made stays viewable and
//! downloadable: its page, its 3D view, its files, its versions, and
//! deleting it. Buying a part in the shop, a shared configurator's page,
//! and a Verse world with placed parts need no plan.
//!
//! The API answers a refused action with `402` and
//! `{"error": {"code": "plan_required", "plan": "pro", "upgrade_url", "message"}}`.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use maud::{Markup, html};
use serde_json::json;

use crate::App;
use crate::cloud::session::Viewer;
use crate::ui_page::action_link;

/// Accounts treated as on Pro (comma separated account ids): the
/// production test account (#11601).
pub(crate) const TEST_ACCOUNTS_ENV: &str = "OPENAGENTS_WEB_PRO_TEST_ACCOUNTS";
/// On a test run, `free` makes the request count as Free.
pub(crate) const TEST_PLAN_HEADER: &str = "x-openagents-test-plan";
/// Where Pro and its trial start.
pub(crate) const UPGRADE: &str = "/settings#settings-plan";
/// The error code of a refused action.
pub(crate) const CODE: &str = "plan_required";
/// The upsell's first line.
pub(crate) const TITLE: &str = "CAD Lab is part of Pro.";
/// Its second.
pub(crate) const LINE: &str =
    "Design parts and assemblies by talking, with every change rebuilt, checked and measured.";

/// The visitor's standing for CAD Lab.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Plan {
    /// On Pro, an admin, or the test account: CAD Lab is theirs.
    Pro,
    /// Signed in without Pro.
    Free,
    /// Not signed in (or the sign-in couldn't be checked).
    SignedOut,
}

impl Plan {
    pub(crate) fn pro(self) -> bool {
        self == Self::Pro
    }
}

/// The signed-in viewer: the browser's session, else an app's own token
/// (`Authorization: Bearer sess_…`).
async fn viewer(app: &App, headers: &HeaderMap) -> Option<Viewer> {
    let service = app.config.cloud.as_deref()?;
    if let Ok(viewer) = service.authenticate(headers).await {
        return Some(viewer);
    }
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))?;
    service.app_viewer(token).await.ok()
}

/// The request's standing for CAD Lab. A server that signs no one in (a
/// local development server) has no plans, so CAD Lab is open there.
pub(crate) async fn plan(app: &App, headers: &HeaderMap) -> Plan {
    if app.config.cloud.is_none() {
        return Plan::Pro;
    }
    match viewer(app, headers).await {
        None => Plan::SignedOut,
        Some(viewer) => {
            let test_accounts = std::env::var(TEST_ACCOUNTS_ENV).unwrap_or_default();
            if passes(app, &viewer, headers, &test_accounts) {
                Plan::Pro
            } else {
                Plan::Free
            }
        }
    }
}

/// Whether a signed-in viewer gets CAD Lab.
fn passes(app: &App, viewer: &Viewer, headers: &HeaderMap, test_accounts: &str) -> bool {
    if test_free(headers) {
        return false;
    }
    viewer.admin
        || listed(test_accounts, &viewer.account_id)
        || crate::plan::on_pro(app, &viewer.account_id)
}

/// A test run asking to be treated as Free.
fn test_free(headers: &HeaderMap) -> bool {
    crate::test_runs::marked(headers)
        && headers
            .get(TEST_PLAN_HEADER)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("free"))
}

fn listed(accounts: &str, account: &str) -> bool {
    !account.is_empty() && accounts.split(',').any(|id| id.trim() == account)
}

/// The `402` an API caller or a page's script gets for a refused action.
pub(crate) fn refused() -> Response {
    (
        StatusCode::PAYMENT_REQUIRED,
        axum::Json(json!({"error": {
            "code": CODE,
            "plan": "pro",
            "upgrade_url": UPGRADE,
            "message": format!("{TITLE} Start a 3-day free trial in Settings."),
        }})),
    )
        .into_response()
}

/// The upsell in place of CAD Lab's tools: what it is, Start 3-day free
/// trial, and Log in when signed out (back to `return_to`).
pub(crate) fn upsell(plan: Plan, return_to: &str) -> Markup {
    let login = crate::auth::login_href(return_to, false);
    html! {
        section.oa-cad-upsell aria-labelledby="cad-upsell-title" {
            h2 id="cad-upsell-title" { (TITLE) }
            p { (LINE) }
            div.oa-page-actions {
                (openagents_ui::actions::ButtonLink::new("Start 3-day free trial", trial_href(plan)))
                @if plan == Plan::SignedOut {
                    (action_link("Log in", &login))
                }
            }
        }
    }
}

/// Start 3-day free trial: Settings → Plan, through sign-up when signed
/// out.
fn trial_href(plan: Plan) -> String {
    match plan {
        Plan::SignedOut => crate::auth::login_href(UPGRADE, true),
        Plan::Free | Plan::Pro => UPGRADE.to_owned(),
    }
}

/// CAD Lab for one chat turn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TurnCad {
    /// The sender may use CAD Lab: with their models (or their project's)
    /// on this site, when they have a scope here.
    Offered(Option<crate::cad_web::Scope>),
    /// The sender isn't on Pro: no CAD tools, and the model says so once.
    NeedsPro,
}

impl TurnCad {
    /// The turn's CAD Lab for a sender whose standing is `plan`.
    pub(crate) fn new(plan: Plan, scope: Option<crate::cad_web::Scope>) -> Self {
        if plan.pro() {
            Self::Offered(scope)
        } else {
            Self::NeedsPro
        }
    }
}

/// What a chat's model is told when CAD Lab isn't offered to this turn:
/// say it once, with the trial link, and help without it.
pub(crate) fn chat_note() -> openagents_chat::router::MemoryNote {
    openagents_chat::router::MemoryNote {
        name: "CAD Lab needs Pro".into(),
        kind: "plan".into(),
        description: "This person isn't on Pro, so this chat has no CAD Lab tools".into(),
        body: format!(
            "CAD Lab (designing, editing, checking, rendering or exporting parts and assemblies) is part of Pro, \
and this person isn't on Pro, so this chat can't make or change CAD models. When they ask for a part or a CAD \
change, say once in this chat, plainly, that CAD Lab is part of Pro and that they can start a 3-day free trial at \
{}{UPGRADE}; then help with what you can without it (ideas, dimensions, a description). If an earlier answer \
in this chat already said it, don't say it again. Never claim you made or changed a model.",
            crate::agent_ready::SITE
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn only_a_test_run_can_ask_to_be_free_and_never_for_pro() {
        let mut headers = HeaderMap::new();
        headers.insert(TEST_PLAN_HEADER, HeaderValue::from_static("free"));
        assert!(!test_free(&headers), "needs the test-run header too");
        headers.insert(crate::test_runs::HEADER, HeaderValue::from_static("1"));
        assert!(test_free(&headers));
        headers.insert(TEST_PLAN_HEADER, HeaderValue::from_static("pro"));
        assert!(!test_free(&headers));
    }

    #[test]
    fn the_test_account_list_names_whole_ids() {
        assert!(listed("acct_1, acct_2", "acct_2"));
        assert!(!listed("acct_12", "acct_1"));
        assert!(!listed("", ""));
        assert!(!listed("acct_1", ""));
    }

    #[tokio::test]
    async fn the_refusal_is_a_typed_402() {
        let response = refused();
        assert_eq!(response.status(), StatusCode::PAYMENT_REQUIRED);
        let body = axum::body::to_bytes(response.into_body(), 4096)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(body["error"]["code"], CODE);
        assert_eq!(body["error"]["plan"], "pro");
        assert_eq!(body["error"]["upgrade_url"], UPGRADE);
    }

    #[test]
    fn the_upsell_has_working_buttons_and_plain_words() {
        let out = upsell(Plan::SignedOut, "/cad").into_string();
        assert!(out.contains(TITLE));
        assert!(out.contains("Start 3-day free trial"));
        assert!(out.contains("Log in"));
        assert!(!out.contains("Sign in"));
        assert!(out.contains("href=\"/login?return_to=%2Fcad\""), "{out}");
        assert!(out.contains("/signup?return_to="), "{out}");
        crate::copy_guard::assert_plain("/cad", &out);
        let free = upsell(Plan::Free, "/cad").into_string();
        assert!(free.contains("href=\"/settings#settings-plan\""), "{free}");
        assert!(!free.contains("Log in"));
        crate::copy_guard::assert_plain("/cad", &free);
        let note = chat_note();
        assert!(
            note.body
                .contains("https://openagents.com/settings#settings-plan")
        );
        assert!(note.body.contains("say once"));
    }
}

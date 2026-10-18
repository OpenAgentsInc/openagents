//! Classifies the app's verified HTTPS links before a platform routes them.
//!
//! A matching URL is only a candidate action. The existing pairing and
//! website-login APIs still check the account and server-side grant. Project
//! invitations are previewed and accepted by the active Rust account.

use std::time::{SystemTime, UNIX_EPOCH};

use openagents_account::openagents::ScannedLogin;
use openagents_connect::code::ConnectCode;

const CONNECT_PREFIX: &str = "https://openagents.com/connect#";
const PROJECT_INVITE_PREFIX: &str = "https://openagents.com/projects/join/";
const MAX_APP_LINK_BYTES: usize = 2048;
const MAX_PROJECT_INVITATION_BYTES: usize = 200;

/// A route recognized by the OpenAgents app-link classifier.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidAppLinkKind {
    Connect,
    ProjectInvitation,
}

/// A bounded route and its opaque code, ready for the existing core flow.
/// Treat `code` as a bearer secret and do not log or persist it.
#[derive(Clone, PartialEq, Eq, uniffi::Record)]
pub struct AndroidAppLink {
    pub kind: AndroidAppLinkKind,
    pub code: String,
}

/// A closed route to an existing Rust operation. This selects a flow; it does
/// not authorize it. The account and server APIs still check the operation.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum AndroidAppLinkOperation {
    PairComputer,
    ReviewWebsiteSignIn,
    ReviewProjectInvitation,
}

/// Rust's decision about which existing core flow may receive a scanned link.
/// `code` is absent when the Rust account API consumes the original link.
#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct AndroidAppLinkDispatch {
    pub operation: AndroidAppLinkOperation,
    pub code: Option<String>,
}

/// Classifies an untrusted URL without performing network or account actions.
/// Returns `None` for other hosts, schemes, paths, expired connect codes, and
/// malformed or overlong input.
#[uniffi::export]
pub fn classify_android_app_link(url: String) -> Option<AndroidAppLink> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    classify_android_app_link_at(&url, now)
}

/// Routes a recognized URL to an existing Rust-owned flow.
///
/// Computer links go to `CoreClient::connect_computer`. Website sign-in links
/// go first to `CoreAccount::scanned_login`, which returns the browser for the
/// confirmation step; approval remains a separate explicit call. Project
/// invitations go to the active account's preview and explicit acceptance
/// methods; the opaque code is not returned to the platform.
#[uniffi::export]
pub fn dispatch_android_app_link(url: String) -> Option<AndroidAppLinkDispatch> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    dispatch_android_app_link_at(&url, now)
}

fn dispatch_android_app_link_at(url: &str, now: u64) -> Option<AndroidAppLinkDispatch> {
    if let Some(link) = classify_android_app_link_at(url, now) {
        let (operation, code) = match link.kind {
            AndroidAppLinkKind::Connect => (AndroidAppLinkOperation::PairComputer, Some(link.code)),
            AndroidAppLinkKind::ProjectInvitation => {
                (AndroidAppLinkOperation::ReviewProjectInvitation, None)
            }
        };
        return Some(AndroidAppLinkDispatch { operation, code });
    }

    ScannedLogin::parse(url).map(|_| AndroidAppLinkDispatch {
        operation: AndroidAppLinkOperation::ReviewWebsiteSignIn,
        code: Some(url.to_owned()),
    })
}

fn classify_android_app_link_at(url: &str, now: u64) -> Option<AndroidAppLink> {
    if url.is_empty() || url.len() > MAX_APP_LINK_BYTES || !url.is_ascii() {
        return None;
    }

    if url.starts_with(CONNECT_PREFIX) {
        ConnectCode::parse(url, now).ok()?;
        let encoded = url.strip_prefix(CONNECT_PREFIX)?;
        return Some(AndroidAppLink {
            kind: AndroidAppLinkKind::Connect,
            code: format!("openagents-connect:{encoded}"),
        });
    }

    let code = project_invitation_code(url)?;

    Some(AndroidAppLink {
        kind: AndroidAppLinkKind::ProjectInvitation,
        code,
    })
}

pub(super) fn project_invitation_code(url: &str) -> Option<String> {
    if url.is_empty() || url.len() > MAX_APP_LINK_BYTES || !url.is_ascii() {
        return None;
    }
    let code = url.strip_prefix(PROJECT_INVITE_PREFIX)?;
    let token = code.strip_prefix("prjinv_")?;
    if code.len() > MAX_PROJECT_INVITATION_BYTES
        || token.is_empty()
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        return None;
    }
    Some(code.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        AndroidAppLinkKind, AndroidAppLinkOperation, classify_android_app_link_at,
        dispatch_android_app_link_at,
    };
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        now: u64,
        cases: Vec<Case>,
    }

    #[derive(Deserialize)]
    struct Case {
        url: String,
        kind: Option<String>,
        code: Option<String>,
    }

    #[test]
    fn fixture_routes_are_bounded_and_fail_closed() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../fixtures/android-app-links.json")).unwrap();

        for case in fixture.cases {
            let parsed = classify_android_app_link_at(&case.url, fixture.now);
            match (parsed, case.kind.as_deref()) {
                (Some(link), Some("connect")) => {
                    assert_eq!(link.kind, AndroidAppLinkKind::Connect);
                    assert_eq!(Some(link.code), case.code);
                }
                (Some(link), Some("project_invitation")) => {
                    assert_eq!(link.kind, AndroidAppLinkKind::ProjectInvitation);
                    assert_eq!(Some(link.code), case.code);
                }
                (None, None) => assert!(case.code.is_none()),
                (_, expected) => panic!("unexpected classification for {expected:?}"),
            }
        }
    }

    #[test]
    fn connect_link_expires_before_it_reaches_pairing() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../fixtures/android-app-links.json")).unwrap();
        let link = &fixture.cases[0].url;
        assert!(classify_android_app_link_at(link, 301).is_none());
    }

    #[test]
    fn dispatch_routes_supported_flows_without_authorizing_them() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../fixtures/android-app-links.json")).unwrap();
        let dispatch = dispatch_android_app_link_at(&fixture.cases[0].url, fixture.now).unwrap();
        assert_eq!(dispatch.operation, AndroidAppLinkOperation::PairComputer);
        assert_eq!(dispatch.code.as_deref(), fixture.cases[0].code.as_deref());

        let login = "https://openagents.com/login/scan#request=0123456789abcdef0123456789abcdef&challenge=v1.9.ab.cd";
        let dispatch = dispatch_android_app_link_at(login, fixture.now).unwrap();
        assert_eq!(
            dispatch.operation,
            AndroidAppLinkOperation::ReviewWebsiteSignIn
        );
        assert_eq!(dispatch.code.as_deref(), Some(login));
    }

    #[test]
    fn project_invitation_dispatches_to_account_review_without_returning_code() {
        let dispatch = dispatch_android_app_link_at(
            "https://openagents.com/projects/join/prjinv_fixture-token_01",
            100,
        )
        .unwrap();
        assert_eq!(
            dispatch.operation,
            AndroidAppLinkOperation::ReviewProjectInvitation
        );
        assert_eq!(dispatch.code, None);
    }

    #[test]
    fn expired_malformed_and_wrong_site_links_do_not_dispatch() {
        let fixture: Fixture =
            serde_json::from_str(include_str!("../../fixtures/android-app-links.json")).unwrap();
        let expired = &fixture.cases[0].url;
        assert!(classify_android_app_link_at(expired, 301).is_none());
        assert!(dispatch_android_app_link_at(expired, 301).is_none());
        for url in [
            "https://openagents.com/connect#not-a-code",
            "https://openagents.com/projects/join/not-an-invitation",
            "https://openagents.com.evil/login/scan#request=0123456789abcdef0123456789abcdef&challenge=v1.9.ab.cd",
        ] {
            assert!(
                dispatch_android_app_link_at(url, fixture.now).is_none(),
                "{url}"
            );
        }
    }
}

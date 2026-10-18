//! Classifies the app's verified HTTPS links before a platform routes them.
//!
//! A matching URL is only a candidate action. Pairing and project-invitation
//! APIs still check the capability, expiry, account, and server-side grant.

use std::time::{SystemTime, UNIX_EPOCH};

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

/// Classifies an untrusted URL without performing network or account actions.
/// Returns `None` for other hosts, schemes, paths, expired connect codes, and
/// malformed or overlong input.
#[uniffi::export]
pub fn classify_android_app_link(url: String) -> Option<AndroidAppLink> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).ok()?.as_secs();
    classify_android_app_link_at(&url, now)
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

    Some(AndroidAppLink {
        kind: AndroidAppLinkKind::ProjectInvitation,
        code: code.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::{AndroidAppLinkKind, classify_android_app_link_at};
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
}

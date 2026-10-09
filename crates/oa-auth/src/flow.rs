//! The browser half of a sign-in: `state`, the PKCE verifier, and where to
//! go afterwards, carried in one short-lived HttpOnly cookie.
//!
//! The cookie is `SameSite=Lax` because GitHub returns the browser with a
//! top-level navigation from github.com, and scoped to `/auth/`. Its value
//! is `<state>.<verifier>.<return_to as base64url>`, plus `.repos` or
//! `.repos-public` when the flow connects repositories instead of signing
//! in ([`Purpose`]); nothing in it is useful to anyone but the browser that
//! started the flow, and the callback re-validates `return_to` anyway.
//!
//! Connecting repositories reuses the same OAuth App and callback: a
//! second authorize that asks for more scopes only at that moment. With a
//! GitHub App, the trip authorizes the App's own client instead
//! ([`Purpose::Install`], tag `.install`), on the same callback.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use oauth2::basic::BasicClient;
use oauth2::{AuthUrl, ClientId, CsrfToken, PkceCodeChallenge, RedirectUrl, Scope};
use subtle::ConstantTimeEq;

use crate::config::{GithubApp, PRIVATE_REPO_SCOPES, PUBLIC_REPO_SCOPES, SCOPES};

/// The flow cookie's name.
pub const FLOW_COOKIE: &str = "oa_auth_flow";
/// How long a started sign-in may take, in seconds.
pub const FLOW_SECONDS: u64 = 600;
/// The longest `return_to` kept.
const RETURN_MAX: usize = 512;

/// A same-site path to return to after sign-in, or `/`.
///
/// Accepted: a path starting with exactly one `/`, with no backslash, no
/// control or whitespace characters, and no scheme. Rejected inputs (full
/// URLs, `//host`, `/\host`, encoded tricks that decode to those) become
/// `/`.
#[must_use]
pub fn return_to(raw: Option<&str>) -> String {
    let Some(raw) = raw else {
        return "/".into();
    };
    let safe = raw.len() <= RETURN_MAX
        && raw.starts_with('/')
        && !raw.starts_with("//")
        && !raw.contains('\\')
        && !raw.chars().any(|c| c.is_control() || c.is_whitespace())
        && !raw.to_ascii_lowercase().contains("%5c")
        && !raw[1..].to_ascii_lowercase().starts_with("%2f")
        && !raw.starts_with("/auth/")
        && url::Url::parse("https://same.invalid")
            .and_then(|base| base.join(raw))
            .is_ok_and(|joined| joined.host_str() == Some("same.invalid"));
    if safe { raw.into() } else { "/".into() }
}

/// What a trip to GitHub is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Sign in or sign up (`read:user user:email`).
    SignIn,
    /// Connect repositories for the signed-in account: `private` asks for
    /// `repo` and `read:org`; otherwise nothing beyond sign-in.
    Repos { private: bool },
    /// Authorize the GitHub App for the signed-in account (its own client;
    /// GitHub Apps take no scopes), to find where it is installed.
    Install,
}

impl Purpose {
    fn scopes(self) -> &'static [&'static str] {
        match self {
            Self::SignIn => &SCOPES,
            Self::Repos { private: true } => &PRIVATE_REPO_SCOPES,
            Self::Repos { private: false } => &PUBLIC_REPO_SCOPES,
            Self::Install => &[],
        }
    }

    fn tag(self) -> Option<&'static str> {
        match self {
            Self::SignIn => None,
            Self::Repos { private: true } => Some("repos"),
            Self::Repos { private: false } => Some("repos-public"),
            Self::Install => Some("install"),
        }
    }
}

/// A started sign-in.
#[derive(Clone, PartialEq, Eq)]
pub struct Flow {
    pub state: String,
    verifier: String,
    pub return_to: String,
    pub purpose: Purpose,
}

impl std::fmt::Debug for Flow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Flow")
            .field("return_to", &self.return_to)
            .finish_non_exhaustive()
    }
}

impl Flow {
    /// Start a sign-in: a fresh state and verifier, and the GitHub URL to
    /// send the browser to.
    pub fn start(app: &GithubApp, return_to_raw: Option<&str>) -> Result<(url::Url, Self), String> {
        Self::start_for(app, return_to_raw, Purpose::SignIn)
    }

    /// Start a trip to GitHub for `purpose`.
    pub fn start_for(
        app: &GithubApp,
        return_to_raw: Option<&str>,
        purpose: Purpose,
    ) -> Result<(url::Url, Self), String> {
        let client = BasicClient::new(ClientId::new(app.client_id.clone()))
            .set_auth_uri(
                AuthUrl::new(app.endpoints.authorize_url.clone())
                    .map_err(|_| "The GitHub authorize URL is invalid.")?,
            )
            .set_redirect_uri(
                RedirectUrl::new(app.redirect_url.clone())
                    .map_err(|_| "The GitHub redirect URL is invalid.")?,
            );
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, state) = client
            .authorize_url(CsrfToken::new_random)
            .add_scopes(purpose.scopes().iter().map(|s| Scope::new((*s).into())))
            .set_pkce_challenge(challenge)
            .url();
        Ok((
            url,
            Self {
                state: state.secret().clone(),
                verifier: verifier.secret().clone(),
                return_to: return_to(return_to_raw),
                purpose,
            },
        ))
    }

    /// The PKCE verifier the token exchange proves possession of.
    #[must_use]
    pub fn verifier(&self) -> &str {
        &self.verifier
    }

    /// Whether the callback's `state` is this flow's, in constant time.
    #[must_use]
    pub fn matches(&self, state: &str) -> bool {
        !state.is_empty() && bool::from(self.state.as_bytes().ct_eq(state.as_bytes()))
    }

    /// The cookie value.
    #[must_use]
    pub fn cookie_value(&self) -> String {
        let mut value = format!(
            "{}.{}.{}",
            self.state,
            self.verifier,
            URL_SAFE_NO_PAD.encode(self.return_to.as_bytes())
        );
        if let Some(tag) = self.purpose.tag() {
            value.push('.');
            value.push_str(tag);
        }
        value
    }

    /// Parse a cookie value back into a flow. Malformed values are `None`.
    #[must_use]
    pub fn from_cookie(value: &str) -> Option<Self> {
        if value.len() > 2048 {
            return None;
        }
        let mut parts = value.split('.');
        let (Some(state), Some(verifier), Some(back), tag, None) = (
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
            parts.next(),
        ) else {
            return None;
        };
        let purpose = match tag {
            None => Purpose::SignIn,
            Some("repos") => Purpose::Repos { private: true },
            Some("repos-public") => Purpose::Repos { private: false },
            Some("install") => Purpose::Install,
            Some(_) => return None,
        };
        let token = |s: &str, min: usize| {
            s.len() >= min
                && s.len() <= 256
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        };
        if !token(state, 16) || !token(verifier, 43) {
            return None;
        }
        let back = String::from_utf8(URL_SAFE_NO_PAD.decode(back).ok()?).ok()?;
        Some(Self {
            state: state.into(),
            verifier: verifier.into(),
            return_to: return_to(Some(&back)),
            purpose,
        })
    }

    /// The `Set-Cookie` value that stores this flow.
    #[must_use]
    pub fn set_cookie(&self, secure: bool) -> String {
        flow_cookie(&self.cookie_value(), FLOW_SECONDS, secure)
    }
}

/// The `Set-Cookie` value that clears the flow cookie.
#[must_use]
pub fn clear_cookie(secure: bool) -> String {
    flow_cookie("", 0, secure)
}

fn flow_cookie(value: &str, seconds: u64, secure: bool) -> String {
    format!(
        "{FLOW_COOKIE}={value}; Path=/auth/; HttpOnly; SameSite=Lax; Max-Age={seconds}{}",
        if secure { "; Secure" } else { "" }
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Endpoints;

    #[test]
    fn return_to_keeps_same_site_paths_and_refuses_everything_else() {
        for good in ["/", "/cloud/app", "/chat/abc?x=1#y", "/cloud/app/settings"] {
            assert_eq!(return_to(Some(good)), good);
        }
        for bad in [
            "https://evil.example/",
            "//evil.example",
            "/\\evil.example",
            "\\\\evil.example",
            "/%5cevil.example",
            "/%2F%2Fevil.example",
            "javascript:alert(1)",
            "evil.example",
            "/ok\nSet-Cookie: x",
            "/ space",
            "/auth/github",
            "",
        ] {
            assert_eq!(return_to(Some(bad)), "/", "{bad:?}");
        }
        assert_eq!(return_to(None), "/");
        assert_eq!(return_to(Some(&format!("/{}", "a".repeat(600)))), "/");
    }

    #[test]
    fn a_flow_carries_pkce_state_and_scopes_and_round_trips_through_its_cookie() {
        let app = GithubApp::new(
            "Ov23liAbc",
            "http://127.0.0.1:4301/auth/github/callback",
            Endpoints::default(),
        )
        .unwrap();
        let (url, flow) = Flow::start(&app, Some("/cloud/app")).unwrap();
        let query: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(url.host_str(), Some("github.com"));
        assert_eq!(query["client_id"], "Ov23liAbc");
        assert_eq!(query["redirect_uri"], app.redirect_url);
        assert_eq!(query["scope"], "read:user user:email");
        assert_eq!(query["code_challenge_method"], "S256");
        assert_eq!(query["state"], flow.state);
        assert_eq!(query["response_type"], "code");
        assert!(!url.as_str().contains(flow.verifier()));

        let back = Flow::from_cookie(&flow.cookie_value()).unwrap();
        assert_eq!(back, flow);
        assert!(back.matches(&flow.state) && !back.matches("other") && !back.matches(""));
        assert!(Flow::from_cookie("a.b.c").is_none());
        assert!(flow.set_cookie(true).contains("HttpOnly; SameSite=Lax"));
        assert!(flow.set_cookie(true).ends_with("; Secure"));

        assert_eq!(back.purpose, Purpose::SignIn);

        // Connecting repositories asks for more only then, and says so in
        // its cookie.
        let (url, repos) =
            Flow::start_for(&app, Some("/projects"), Purpose::Repos { private: true }).unwrap();
        let query: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["scope"], "read:user repo read:org");
        assert_eq!(query["redirect_uri"], app.redirect_url);
        let back = Flow::from_cookie(&repos.cookie_value()).unwrap();
        assert_eq!(back.purpose, Purpose::Repos { private: true });
        assert_eq!(back.return_to, "/projects");
        let (url, public) = Flow::start_for(&app, None, Purpose::Repos { private: false }).unwrap();
        let query: std::collections::BTreeMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(query["scope"], "read:user");
        assert_eq!(
            Flow::from_cookie(&public.cookie_value()).unwrap().purpose,
            Purpose::Repos { private: false }
        );
        assert!(Flow::from_cookie(&format!("{}.admin", flow.cookie_value())).is_none());

        // A tampered return_to in the cookie is re-validated.
        let forged = format!(
            "{}.{}.{}",
            flow.state,
            flow.verifier(),
            URL_SAFE_NO_PAD.encode("//evil.example")
        );
        assert_eq!(Flow::from_cookie(&forged).unwrap().return_to, "/");
    }
}

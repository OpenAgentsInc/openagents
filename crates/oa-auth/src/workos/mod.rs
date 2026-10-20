//! WorkOS as the auth object for a person
//! (docs/auth/2026-10-20-workos-source-of-truth.md, #12320 to #12323).
//!
//! WorkOS holds one User per person and one Organization per shared
//! workspace. Our account IDs never change: a User's `external_id` is the
//! `acct_…` it belongs to, and [`tenancy::workos_links`] joins the two.
//! Nostr keys stay ours. In `workos` mode a verified key signs in through
//! WorkOS; `dual` keeps the legacy session path for rollback.
//!
//! - [`Settings`] and [`Mode`]: the per-environment switch
//!   (`OPENAGENTS_AUTH=legacy|dual|workos`) and the sign-in methods the
//!   environment has turned on.
//! - [`client::Workos`]: the calls we make, over the official `workos`
//!   SDK, with errors mapped to [`Refusal`]s that carry no secret.
//! - [`token`]: access-token checks against the environment's JWKS.
//! - [`link`]: which account a WorkOS User signs in to, linking an
//!   existing account by email or GitHub before making a new one.
//! - [`session`]: the account service's sign-in from a WorkOS access
//!   token, shared by the gateway and the local fixture.
//! - [`sync`]: the dual-write and backfill of accounts, workspaces, and
//!   memberships to WorkOS, idempotent by external ID.
//! - `fake` (feature `fake-workos`): an in-process WorkOS API for tests.

pub mod client;
#[cfg(feature = "fake-workos")]
pub mod fake;
pub mod link;
pub mod retire;
pub mod session;
pub mod sync;
pub mod token;

pub use client::{Authenticated, Grant, Refusal, User, Workos};
pub use token::Claims;
/// The official SDK, for its sealed-session helpers.
pub use workos as sdk;

use serde::Deserialize;

/// The environment variable that overrides [`Settings::mode`].
pub const MODE_ENV: &str = "OPENAGENTS_AUTH";

/// Where WorkOS sends a browser back after a provider sign-in.
pub const CALLBACK_PATH: &str = "/auth/callback";

/// The domain key-only accounts' WorkOS email addresses use (owner
/// decision 3). Nothing is ever sent there, and it is never shown.
pub const SYNTHETIC_DOMAIN: &str = "id.openagents.com";

/// How far WorkOS is in the path, per environment. Changing it is the
/// whole rollback: no data moves.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum Mode {
    /// WorkOS is off: sign-in is exactly what it was before WorkOS.
    #[default]
    Legacy,
    /// WorkOS sign-in methods are on, every account change is also
    /// written to WorkOS, and the API accepts WorkOS access tokens. Web
    /// pages still run on our session, minted after WorkOS signs a person
    /// in.
    Dual,
    /// WorkOS signs in browsers and capable apps. Device-code and older
    /// app clients keep linked legacy sessions; keys and anonymous
    /// sessions keep their existing stores.
    Workos,
}

impl Mode {
    /// Parse `legacy`, `dual`, or `workos` (also `off`, `shadow`, and
    /// `primary`, the plan's names).
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "legacy" | "off" | "shadow" => Some(Self::Legacy),
            "dual" => Some(Self::Dual),
            "workos" | "primary" => Some(Self::Workos),
            _ => None,
        }
    }

    /// Whether WorkOS is in the path at all.
    #[must_use]
    pub fn on(self) -> bool {
        self != Self::Legacy
    }

    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Dual => "dual",
            Self::Workos => "workos",
        }
    }
}

/// One way to sign in that WorkOS offers. Only the methods an
/// environment has turned on in WorkOS are listed in its settings, so the
/// sign-in page never shows a button that can't work.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    /// GitHub through WorkOS.
    Github,
    /// Google through WorkOS.
    Google,
    /// Email and password, with sign-up and email verification.
    Password,
    /// An emailed one-time code (WorkOS Magic Auth).
    MagicCode,
    /// Single sign-on for an organization's domain.
    Sso,
}

impl Method {
    /// The WorkOS provider name, for the OAuth methods.
    #[must_use]
    pub fn provider(self) -> Option<&'static str> {
        match self {
            Self::Github => Some("GitHubOAuth"),
            Self::Google => Some("GoogleOAuth"),
            _ => None,
        }
    }

    /// The path segment `/auth/workos/{slug}` uses.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Github => "github",
            Self::Google => "google",
            Self::Password => "password",
            Self::MagicCode => "code",
            Self::Sso => "sso",
        }
    }

    #[must_use]
    pub fn from_slug(slug: &str) -> Option<Self> {
        [
            Self::Github,
            Self::Google,
            Self::Password,
            Self::MagicCode,
            Self::Sso,
        ]
        .into_iter()
        .find(|method| method.slug() == slug)
    }
}

/// The WorkOS block of a deployment's configuration, shared by the
/// account service and the website.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// The rollout switch; [`MODE_ENV`] overrides it.
    #[serde(default)]
    pub mode: Mode,
    /// The environment's client ID (`client_…`), public.
    pub client_id: String,
    /// The environment variable holding the API key.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Or a file holding it, mode 0600.
    #[serde(default)]
    pub api_key_file: Option<std::path::PathBuf>,
    /// The API's address; tests point it at the fake.
    #[serde(default)]
    pub base_url: Option<String>,
    /// The sign-in methods this environment has turned on in WorkOS.
    #[serde(default)]
    pub methods: Vec<Method>,
    /// The organization role slugs for our three roles, in WorkOS. Each
    /// must exist in the environment; a missing one falls back to the
    /// environment's default role.
    #[serde(default)]
    pub roles: Roles,
}

/// WorkOS role slugs for owner, admin, and member.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roles {
    #[serde(default = "owner")]
    pub owner: String,
    #[serde(default = "admin")]
    pub admin: String,
    #[serde(default = "member")]
    pub member: String,
}

impl Default for Roles {
    fn default() -> Self {
        Self {
            owner: owner(),
            admin: admin(),
            member: member(),
        }
    }
}

fn owner() -> String {
    "owner".into()
}
fn admin() -> String {
    "admin".into()
}
fn member() -> String {
    "member".into()
}

impl Roles {
    #[must_use]
    pub fn slug(&self, role: tenancy::Role) -> &str {
        match role {
            tenancy::Role::Owner => &self.owner,
            tenancy::Role::Admin => &self.admin,
            tenancy::Role::Member => &self.member,
        }
    }
}

impl Settings {
    /// The mode in force: [`MODE_ENV`] when it is set to a known value,
    /// else the configured one.
    #[must_use]
    pub fn mode(&self) -> Mode {
        std::env::var(MODE_ENV)
            .ok()
            .and_then(|value| Mode::parse(&value))
            .unwrap_or(self.mode)
    }

    /// Whether `method` is on here.
    #[must_use]
    pub fn offers(&self, method: Method) -> bool {
        self.methods.contains(&method)
    }

    /// Check the block and read the API key.
    pub fn api_key(&self) -> Result<String, String> {
        if !self.client_id.starts_with("client_")
            || self.client_id.len() > 64
            || !self
                .client_id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err("workos.client_id must be a WorkOS client ID".into());
        }
        let key = match (&self.api_key_env, &self.api_key_file) {
            (Some(name), None) => std::env::var(name)
                .map_err(|_| format!("workos.api_key_env names {name}, which is unset"))?,
            (None, Some(path)) => String::from_utf8(
                crate::config::read_private_bytes(path)
                    .map_err(|why| format!("workos.api_key_file: {why}"))?,
            )
            .map_err(|_| "workos.api_key_file isn't text".to_string())?,
            _ => return Err("workos needs exactly one of api_key_env and api_key_file".into()),
        };
        let key = key.trim().to_string();
        if !key.starts_with("sk_")
            || key.len() > 512
            || key.bytes().any(|b| b.is_ascii_whitespace())
        {
            return Err("the WorkOS API key isn't one".into());
        }
        Ok(key)
    }
}

/// The synthetic WorkOS email for a key-only account (owner decision 3).
#[must_use]
pub fn synthetic_email(account: &str) -> String {
    format!("{account}@{SYNTHETIC_DOMAIN}")
}

/// Whether `email` is one of ours from [`synthetic_email`], which is
/// never shown to anyone.
#[must_use]
pub fn is_synthetic(email: &str) -> bool {
    email
        .rsplit_once('@')
        .is_some_and(|(_, domain)| domain.eq_ignore_ascii_case(SYNTHETIC_DOMAIN))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_parse_both_names_and_default_to_legacy() {
        assert_eq!(Mode::parse("dual"), Some(Mode::Dual));
        assert_eq!(Mode::parse(" WorkOS "), Some(Mode::Workos));
        assert_eq!(Mode::parse("primary"), Some(Mode::Workos));
        assert_eq!(Mode::parse("off"), Some(Mode::Legacy));
        assert_eq!(Mode::parse("sometimes"), None);
        assert_eq!(Mode::default(), Mode::Legacy);
        assert!(!Mode::Legacy.on() && Mode::Dual.on());
    }

    #[test]
    fn methods_have_stable_slugs() {
        for method in [
            Method::Github,
            Method::Google,
            Method::Password,
            Method::MagicCode,
            Method::Sso,
        ] {
            assert_eq!(Method::from_slug(method.slug()), Some(method));
        }
        assert_eq!(Method::Github.provider(), Some("GitHubOAuth"));
        assert_eq!(Method::Password.provider(), None);
    }

    #[test]
    fn synthetic_addresses_are_recognized() {
        let email = synthetic_email("acct_0123456789abcdef");
        assert_eq!(email, "acct_0123456789abcdef@id.openagents.com");
        assert!(is_synthetic(&email));
        assert!(!is_synthetic("ada@example.com"));
    }

    #[test]
    fn settings_need_a_client_and_one_key_source() {
        let settings: Settings = serde_json::from_value(serde_json::json!({
            "client_id": "client_01ABC",
            "api_key_env": "OA_AUTH_TEST_UNSET_KEY_ENV",
            "methods": ["github", "password", "magic_code"]
        }))
        .unwrap();
        assert!(settings.offers(Method::Github));
        assert!(!settings.offers(Method::Google));
        assert!(settings.api_key().unwrap_err().contains("unset"));
        let bad: Settings =
            serde_json::from_value(serde_json::json!({"client_id": "nope"})).unwrap();
        assert!(bad.api_key().is_err());
    }
}

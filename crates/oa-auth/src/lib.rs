//! OpenAgents sign-in, owned in Rust. See `docs/auth/README.md`.
//!
//! - [`config`]: the OAuth App settings and the private credentials file.
//! - [`flow`]: the browser half (state, PKCE verifier, `return_to`), used
//!   by the web server's `/auth/github` and `/auth/github/callback`.
//! - [`github`]: the account-service half: code + verifier to profile.
//! - [`repos`]: connected repositories and projects, with the token kept
//!   encrypted.
//! - [`service`]: the account service's GitHub routes over the tenancy
//!   stores (find or create the account, issue the session).
//! - [`device`]: device-code sign-in for apps (RFC 8628 shape) and the
//!   account's signed-in apps list.
//! - `fake` (feature): an in-process fake GitHub for tests and fixtures.
//! - `local` (feature): a small account service for local fixtures.

pub mod cache;
pub mod config;
pub mod device;
#[cfg(feature = "fake")]
pub mod fake;
pub mod flow;
pub mod github;
#[cfg(feature = "local")]
pub mod local;
pub mod repos;
pub mod service;

pub use config::{CALLBACK_PATH, Endpoints, GithubApp, GithubCredentials};
pub use flow::{FLOW_COOKIE, Flow, Purpose, return_to};
pub use github::Github;

/// Why a sign-in did not complete. Carries no secret or provider detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthError {
    /// GitHub refused the code (expired, reused, wrong verifier) or the
    /// person did not consent.
    Denied,
    /// The identity is already linked to another account.
    Taken,
    /// The account already has a different identity from this provider.
    AlreadyLinked,
    /// GitHub or the account stores could not be reached.
    Unavailable,
}

impl AuthError {
    /// A stable code for the HTTP error envelope.
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Self::Denied => "sign_in_denied",
            Self::Taken => "identity_taken",
            Self::AlreadyLinked => "provider_linked",
            Self::Unavailable => "sign_in_unavailable",
        }
    }

    /// The HTTP status an account-service route answers with.
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::Denied => 401,
            Self::Taken | Self::AlreadyLinked => 409,
            Self::Unavailable => 503,
        }
    }

    pub(crate) fn from_refusal(refusal: tenancy::accounts::Refusal) -> Self {
        use tenancy::accounts::Refusal as R;
        match refusal {
            R::PrincipalTaken { .. } => Self::Taken,
            R::ProviderLinked { .. } => Self::AlreadyLinked,
            R::EmptyField(_) => Self::Denied,
            _ => Self::Unavailable,
        }
    }
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Denied => "GitHub sign-in didn't go through. Try again.",
            Self::Taken => "That GitHub account is already used by another OpenAgents account.",
            Self::AlreadyLinked => "This account already has a different GitHub account linked.",
            Self::Unavailable => "Sign-in isn't available right now. Try again in a minute.",
        })
    }
}

impl std::error::Error for AuthError {}

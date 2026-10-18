//! OpenAgents sign-in, owned in Rust. See `docs/auth/README.md`.
//!
//! - [`app`]: the GitHub App: its key and JWT, installation tokens and
//!   their cache (#11056).
//! - [`attest`]: App Attest for key accounts made on the iPhone: Apple's
//!   attestation and assertion checks, and which account each attested key
//!   belongs to (#11994).
//! - [`config`]: the OAuth App settings and the private credentials file.
//! - [`flow`]: the browser half (state, PKCE verifier, `return_to`), used
//!   by the web server's `/auth/github` and `/auth/github/callback`.
//! - [`github`]: the account-service half: code + verifier to profile.
//! - [`invite`]: invite-only sign-in, the GitHub people a deployment lets in.
//! - [`nostr`]: sign in with a Nostr key: a signed NIP-98 event becomes
//!   the same app session a device sign-in issues, and the account's keys
//!   are listed, added, and removed (#11993).
//! - [`repos`]: connected repositories and projects, with the token kept
//!   encrypted.
//! - [`service`]: the account service's GitHub routes over the tenancy
//!   stores (find or create the account, issue the session).
//! - [`device`]: device-code sign-in for apps (RFC 8628 shape) and the
//!   account's signed-in apps list.
//! - [`sign_in_keys`]: the account's sign-in keys (`oa_sk_…`) for agents:
//!   list, make, revoke, and the last-use note.
//! - `fake` (feature): an in-process fake GitHub for tests and fixtures.
//! - `local` (feature): a small account service for local fixtures.

pub mod app;
pub mod attest;
pub mod cache;
pub mod config;
pub mod device;
#[cfg(feature = "fake")]
pub mod fake;
pub mod flow;
pub mod github;
pub mod invite;
#[cfg(feature = "local")]
pub mod local;
pub mod nostr;
pub mod play_integrity;
pub mod repos;
pub mod service;
pub mod sign_in_keys;

pub use app::{AppClient, AppCredentials, AppInstall, TokenCache};
pub use config::{CALLBACK_PATH, Endpoints, GithubApp, GithubCredentials};
pub use flow::{FLOW_COOKIE, Flow, Purpose, return_to};
pub use github::Github;
pub use invite::{InviteOnly, Invited};

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
    /// The deployment is invite-only and this GitHub account isn't
    /// invited ([`invite`]). Nothing was made.
    InviteOnly,
    /// No account holds this GitHub identity, and the deployment makes no
    /// new accounts from GitHub sign-in (#12005). Nothing was made.
    SignupClosed,
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
            Self::InviteOnly => "invite_only",
            Self::SignupClosed => "signup_disabled",
        }
    }

    /// The HTTP status an account-service route answers with.
    #[must_use]
    pub fn status(self) -> u16 {
        match self {
            Self::Denied => 401,
            Self::Taken | Self::AlreadyLinked => 409,
            Self::Unavailable => 503,
            Self::InviteOnly | Self::SignupClosed => 403,
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
            Self::Denied => "GitHub login didn't go through. Try again.",
            Self::Taken => "That GitHub account is already used by another OpenAgents account.",
            Self::AlreadyLinked => "This account already has a different GitHub account linked.",
            Self::Unavailable => "Login isn't available right now. Try again in a minute.",
            Self::InviteOnly => "Login is invite-only for now.",
            Self::SignupClosed => {
                "No OpenAgents account uses that GitHub account. New accounts start in the OpenAgents app."
            }
        })
    }
}

impl std::error::Error for AuthError {}

/// The code `POST /v1/sessions/apple` and `POST /v1/device/apple` answer
/// with `410 Gone` since Sign in with Apple was removed (#12005).
pub const APPLE_REMOVED_CODE: &str = "apple_removed";

/// The message those routes answer with.
pub const APPLE_REMOVED_MESSAGE: &str = "Sign in with Apple has been removed.";

//! Where the pages that show accounts read their rows: profiles, and the
//! homepage's credit line.
//!
//! The production site reads these from its account store. None of that is
//! in this repository, so
//! the pages read through [`Backend`], and a development server runs with
//! [`Development`], which is connected to nothing: every page renders, says
//! that its data needs the production backend, and shows no records. No
//! page ever shows a made-up row as if it were real.
//!
//! A production implementation answers [`Backend::connected`] with `true`;
//! then a missing profile answers `404`.

use futures_util::future::BoxFuture;

/// What a person made public by signing in with GitHub. Nothing private
/// (email, runs, balance, invite codes) is part of it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Profile {
    pub login: String,
    pub name: Option<String>,
    /// "September 2026".
    pub joined: String,
}

/// The site's data source for pages that need one.
pub trait Backend: Send + Sync {
    /// Whether this backend reads real production data.
    fn connected(&self) -> bool;

    /// What a new account starts with, in cents, for the homepage's credit
    /// line. `None` or zero draws no line.
    fn new_account_credit_cents(&self) -> Option<u64> {
        None
    }

    fn profile<'a>(&'a self, login: &'a str) -> BoxFuture<'a, Option<Profile>>;
}

/// The development backend: connected to nothing, answering nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Development;

impl Backend for Development {
    fn connected(&self) -> bool {
        false
    }

    fn profile<'a>(&'a self, _login: &'a str) -> BoxFuture<'a, Option<Profile>> {
        Box::pin(async { None })
    }
}

/// The note a page shows in place of its data on a development server.
pub const NOT_CONNECTED: &str = "This is a test server, so there's nothing to show here. \
     The real page is on openagents.com.";

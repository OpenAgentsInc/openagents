//! Where the pages that show accounts, uploads, and live fleets read their
//! rows.
//!
//! The production site reads these from its account store, forum, trace
//! intake, and fleet coordinator. None of that is in this repository, so
//! the pages read through [`Backend`], and a development server runs with
//! [`Development`], which is connected to nothing: every page renders, says
//! that its data needs the production backend, and shows no records. No
//! page ever shows a made-up row as if it were real.
//!
//! A production implementation answers [`Backend::connected`] with `true`;
//! then an empty answer means there is nothing yet, and a missing profile,
//! topic, board, or trace answers `404`.

use futures_util::future::BoxFuture;

/// A stored trace, as a listing shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TraceListing {
    /// `sha256:` digest of the document as it arrived.
    pub digest: String,
    pub receipt: String,
    pub domain: String,
    /// `glass` (content public), `ledger`, or `pulse`.
    pub visibility: String,
    /// RFC 3339.
    pub received_at: String,
    pub size_bytes: u64,
    pub truncated: bool,
}

/// One trace: its listing, and its ATIF document when the trace's
/// visibility lets anyone read it.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceRecord {
    pub listing: TraceListing,
    pub document: Option<serde_json::Value>,
}

/// A forum board.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumBoard {
    pub slug: String,
    pub title: String,
    pub description: String,
    pub topics: u64,
    pub posts: u64,
    pub locked: bool,
    /// RFC 3339, of the newest post.
    pub last: Option<String>,
}

/// A forum topic, as a board lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumTopic {
    pub id: String,
    pub title: String,
    pub author: String,
    pub posts: u64,
    pub pinned: bool,
    pub closed: bool,
    pub opened: String,
    pub board_slug: String,
    pub board_title: String,
}

/// A post in a topic. `body` is Markdown; raw HTML in it renders as text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForumPost {
    pub seq: u64,
    pub author: String,
    pub agent: bool,
    pub body: String,
    pub created_at: String,
}

/// A read-only live board (Earn, Weights, QA): a sentence saying what it
/// shows and where it came from, and its tables.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dashboard {
    pub summary: String,
    pub tables: Vec<Table>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    pub title: String,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

/// Which live board.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Board {
    Earn,
    Weights,
    Qa,
}

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

    /// An answer to a question typed on the homepage, as Markdown.
    fn answer<'a>(&'a self, question: &'a str) -> BoxFuture<'a, Option<String>>;

    fn traces(&self) -> BoxFuture<'_, Vec<TraceListing>>;
    fn trace<'a>(&'a self, key: &'a str) -> BoxFuture<'a, Option<TraceRecord>>;

    fn forum_boards(&self) -> BoxFuture<'_, Vec<ForumBoard>>;
    fn forum_board<'a>(
        &'a self,
        slug: &'a str,
    ) -> BoxFuture<'a, Option<(ForumBoard, Vec<ForumTopic>)>>;
    fn forum_topic<'a>(
        &'a self,
        id: &'a str,
    ) -> BoxFuture<'a, Option<(ForumTopic, Vec<ForumPost>)>>;

    fn dashboard(&self, board: Board) -> BoxFuture<'_, Option<Dashboard>>;

    fn profile<'a>(&'a self, login: &'a str) -> BoxFuture<'a, Option<Profile>>;
}

/// The development backend: connected to nothing, answering nothing.
#[derive(Clone, Copy, Debug, Default)]
pub struct Development;

impl Backend for Development {
    fn connected(&self) -> bool {
        false
    }

    fn answer<'a>(&'a self, _question: &'a str) -> BoxFuture<'a, Option<String>> {
        Box::pin(async { None })
    }

    fn traces(&self) -> BoxFuture<'_, Vec<TraceListing>> {
        Box::pin(async { Vec::new() })
    }

    fn trace<'a>(&'a self, _key: &'a str) -> BoxFuture<'a, Option<TraceRecord>> {
        Box::pin(async { None })
    }

    fn forum_boards(&self) -> BoxFuture<'_, Vec<ForumBoard>> {
        Box::pin(async { Vec::new() })
    }

    fn forum_board<'a>(
        &'a self,
        _slug: &'a str,
    ) -> BoxFuture<'a, Option<(ForumBoard, Vec<ForumTopic>)>> {
        Box::pin(async { None })
    }

    fn forum_topic<'a>(
        &'a self,
        _id: &'a str,
    ) -> BoxFuture<'a, Option<(ForumTopic, Vec<ForumPost>)>> {
        Box::pin(async { None })
    }

    fn dashboard(&self, _board: Board) -> BoxFuture<'_, Option<Dashboard>> {
        Box::pin(async { None })
    }

    fn profile<'a>(&'a self, _login: &'a str) -> BoxFuture<'a, Option<Profile>> {
        Box::pin(async { None })
    }
}

/// The note a page shows in place of its data on a development server.
pub const NOT_CONNECTED: &str = "This server runs without the production backend, so this page \
     shows no records. openagents.com serves them from its account store, forum, trace intake, \
     and fleet coordinator.";

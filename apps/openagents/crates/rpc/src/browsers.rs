//! Browsers on openagents.com as clients of this computer (#12264): the
//! method names and values the browser, the engine, and the computer's own
//! app share.
//!
//! A browser reaches the computer through its device room as a `web-`
//! connection ([`crate::device_room::BROWSER_CONN_PREFIX`]). It may ask for
//! access ([`BROWSER_REQUEST`]), follow its request
//! ([`BROWSER_REQUEST_STATUS`]), prove its key on its connection
//! ([`BROWSER_HELLO`], `coder_access::browser`), and give its access back
//! ([`BROWSER_FORGET`]). Every other call needs a hello first, and the
//! engine checks the browser's grant again on each one.
//!
//! The person at the computer allows or denies a request in the computer's
//! own app ([`LIST_BROWSER_REQUESTS`], [`ANSWER_BROWSER_REQUEST`]) and sees
//! and removes allowed browsers there ([`LIST_BROWSERS`],
//! [`REVOKE_BROWSER`]). The engine serves those four only on its
//! in-process and localhost transports, never through the relay.

use serde::{Deserialize, Serialize};

/// Ask for access. Params [`AskAccess`]. Returns [`RequestState`].
pub const BROWSER_REQUEST: &str = "BrowserRequest";
/// Params `{request}`. Returns [`RequestState`].
pub const BROWSER_REQUEST_STATUS: &str = "BrowserRequestStatus";
/// Params `{event}`, the signed hello. Returns [`Hello`].
pub const BROWSER_HELLO: &str = "BrowserHello";
/// Remove this browser's own access. Params `{}`. Returns `{}`.
pub const BROWSER_FORGET: &str = "BrowserForget";

/// This computer's own app: the requests waiting. Params `{}`. Returns
/// `Vec<`[`BrowserRequest`]`>`.
pub const LIST_BROWSER_REQUESTS: &str = "ListBrowserRequests";
/// This computer's own app: allow or deny. Params [`AnswerRequest`].
/// Returns `{}`.
pub const ANSWER_BROWSER_REQUEST: &str = "AnswerBrowserRequest";
/// This computer's own app: allowed browsers. Params `{}`. Returns
/// `Vec<`[`Browser`]`>`.
pub const LIST_BROWSERS: &str = "ListBrowsers";
/// This computer's own app: remove a browser. Params `{key}`. Returns `{}`.
pub const REVOKE_BROWSER: &str = "RevokeBrowser";

/// A browser's request for access.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AskAccess {
    /// The browser's key, lowercase hex (x-only secp256k1).
    pub key: String,
    /// What the browser calls itself, such as "Chrome on macOS".
    #[serde(default)]
    pub label: String,
}

/// Where a request stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// Waiting for the person at the computer.
    Asked,
    Allowed,
    Denied,
    /// Not answered in time, or the computer restarted.
    Expired,
}

/// The answer to [`BROWSER_REQUEST`] and [`BROWSER_REQUEST_STATUS`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestState {
    pub request: String,
    pub state: State,
}

/// The answer to a hello: what the browser may do, until when.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hello {
    pub rights: Vec<String>,
    /// Unix seconds.
    pub expires_at: u64,
}

/// A request waiting in the computer's app.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRequest {
    pub id: String,
    pub label: String,
    pub key: String,
    /// Unix seconds.
    pub requested_at: u64,
}

/// [`ANSWER_BROWSER_REQUEST`]'s params.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnswerRequest {
    pub id: String,
    pub allow: bool,
}

/// An allowed browser.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Browser {
    pub key: String,
    pub label: String,
    /// Unix seconds.
    pub expires_at: u64,
    /// Unix seconds, when this computer last saw it.
    #[serde(default)]
    pub last_seen: Option<u64>,
}

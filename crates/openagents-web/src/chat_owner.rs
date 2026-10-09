//! Whose chats a request reads and writes (#11039).
//!
//! Signed in, the chats belong to the account: they show on any browser
//! the person signs in on, and not to whoever uses the browser after they
//! sign out. Signed out, they belong to the browser's `oa_visitor` cookie
//! ([`crate::ask::visitor`]), as before. The two owner values never overlap
//! ([`crate::chat_store::account_owner`]), so a cookie can never open an
//! account's chats.
//!
//! When a browser with signed-out chats signs in, [`claim`] moves those
//! chats to the account, once, at sign-in.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use axum::http::{HeaderMap, header};
use futures_util::stream::{self, StreamExt};
use sha2::{Digest, Sha256};

use crate::App;
use crate::chat_store::{Store, account_owner};
use crate::cloud::session::SessionError;

/// The Cloud session cookie ([`crate::cloud::session`]).
const SESSION_COOKIE: &str = "oa_cloud_session";

/// How long a checked sign-in is reused for chat requests, so a chat's
/// one-second updates and quick clicks don't each ask the account service.
const REMEMBER: Duration = Duration::from_secs(30);

/// Whose chats this request may use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Who {
    /// Signed in: the account's chats, on any browser.
    Account(String),
    /// Signed out: this browser's chats, named by its visitor cookie.
    Browser(String),
    /// A sign-in that couldn't be checked right now (the account service
    /// didn't answer): the browser's own chats can be read, but nothing is
    /// written, so a signed-in person's new chat never lands on the browser.
    Unchecked(Option<String>),
    /// Signed out, with no visitor cookie yet.
    Nobody,
}

impl Who {
    /// The owner whose chats may be shown.
    pub(crate) fn reader(&self) -> Option<&str> {
        match self {
            Self::Account(owner) | Self::Browser(owner) => Some(owner),
            Self::Unchecked(browser) => browser.as_deref(),
            Self::Nobody => None,
        }
    }

    /// The owner whose chats may be changed; `None` when nothing may be.
    pub(crate) fn writer(&self) -> Option<&str> {
        match self {
            Self::Account(owner) | Self::Browser(owner) => Some(owner),
            Self::Unchecked(_) | Self::Nobody => None,
        }
    }

    /// Whether the chats are a signed-in account's.
    pub(crate) fn signed_in(&self) -> bool {
        matches!(self, Self::Account(_))
    }
}

/// Resolve whose chats this request uses.
pub(crate) async fn who(app: &App, headers: &HeaderMap) -> Who {
    let browser = crate::ask::visitor(headers);
    let (Some(service), Some(token)) = (app.config.cloud.as_deref(), session(headers)) else {
        return decide(browser, None);
    };
    let key: [u8; 32] = Sha256::digest(token.as_bytes()).into();
    if let Some(account) = remembered(&key) {
        return Who::Account(account_owner(&account));
    }
    let checked = service
        .authenticate(headers)
        .await
        .map(|viewer| viewer.account_id);
    if let Ok(account) = &checked {
        remember(key, account);
    }
    decide(browser, Some(checked))
}

/// The rule, apart from the network: no sign-in cookie, or one the account
/// service says is not (or no longer) signed in, is signed out; a sign-in
/// that couldn't be checked writes nothing.
fn decide(browser: Option<String>, checked: Option<Result<String, SessionError>>) -> Who {
    match checked {
        Some(Ok(account)) => Who::Account(account_owner(&account)),
        Some(Err(SessionError::Unavailable | SessionError::Conflict | SessionError::Csrf)) => {
            Who::Unchecked(browser)
        }
        None | Some(Err(_)) => browser.map_or(Who::Nobody, Who::Browser),
    }
}

fn session(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, value)| *name == SESSION_COOKIE && !value.is_empty())
        .map(|(_, value)| value.to_owned())
}

/// Sign-ins checked in the last [`REMEMBER`], by session-cookie digest.
fn checked() -> &'static Mutex<HashMap<[u8; 32], (String, Instant)>> {
    static CHECKED: OnceLock<Mutex<HashMap<[u8; 32], (String, Instant)>>> = OnceLock::new();
    CHECKED.get_or_init(Default::default)
}

fn remembered(key: &[u8; 32]) -> Option<String> {
    let kept = checked().lock().ok()?;
    let (account, at) = kept.get(key)?;
    (at.elapsed() < REMEMBER).then(|| account.clone())
}

fn remember(key: [u8; 32], account: &str) {
    if let Ok(mut kept) = checked().lock() {
        kept.retain(|_, (_, at)| at.elapsed() < REMEMBER);
        if kept.len() > 4096 {
            kept.clear();
        }
        kept.insert(key, (account.to_owned(), Instant::now()));
    }
}

/// At sign-in: move this browser's signed-out chats to the account it
/// signed in to. A chat still being answered stays with the browser (its
/// answer is written there); a chat that changes while it moves stays too.
/// Nothing on the account is ever replaced. Returns how many moved.
pub(crate) async fn claim(app: &App, headers: &HeaderMap, account_id: &str) -> usize {
    let Some(browser) = crate::ask::visitor(headers) else {
        return 0;
    };
    let moved = adopt_all(&app.config.chat_store, &browser, &account_owner(account_id)).await;
    if moved > 0 {
        println!("openagents-web: moved {moved} chats to an account at sign-in");
    }
    moved
}

/// Move every idle chat of `browser` to `account`; see [`claim`].
pub(crate) async fn adopt_all(store: &Store, browser: &str, account: &str) -> usize {
    let rows = match store.list(browser).await {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("openagents-web: chat claim: {error}");
            return 0;
        }
    };
    stream::iter(rows.into_iter().filter(|chat| chat.pending.is_none()))
        .map(|chat| async move {
            let Ok(Some(loaded)) = store.load(browser, &chat.id).await else {
                return false;
            };
            if loaded.conversation.pending.is_some() {
                return false;
            }
            match store.adopt(&loaded, account).await {
                Ok(moved) => moved,
                Err(error) => {
                    eprintln!("openagents-web: chat claim: {error}");
                    false
                }
            }
        })
        .buffer_unordered(8)
        .filter(|moved| std::future::ready(*moved))
        .count()
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_store::{Conversation, Message, Outcome, Pending, Request, Role};
    use axum::http::HeaderValue;

    const BROWSER: &str = "0123456789abcdef0123456789abcdef";
    const OTHER: &str = "abcdef0123456789abcdef0123456789";

    #[test]
    fn signed_in_reads_and_writes_the_account_and_an_unchecked_sign_in_writes_nothing() {
        let browser = || Some(BROWSER.to_owned());
        let account = decide(browser(), Some(Ok("acct_1".into())));
        assert_eq!(account, Who::Account(account_owner("acct_1")));
        assert_eq!(account.reader(), account.writer());
        assert!(account.signed_in());
        assert_ne!(account.reader(), Some(BROWSER));

        assert_eq!(decide(browser(), None), Who::Browser(BROWSER.into()));
        assert_eq!(
            decide(browser(), Some(Err(SessionError::Unauthenticated))),
            Who::Browser(BROWSER.into())
        );
        assert_eq!(decide(None, None), Who::Nobody);
        assert_eq!(decide(None, None).reader(), None);

        let unchecked = decide(browser(), Some(Err(SessionError::Unavailable)));
        assert_eq!(unchecked.reader(), Some(BROWSER));
        assert_eq!(unchecked.writer(), None);
        assert!(!unchecked.signed_in());
    }

    #[test]
    fn only_the_session_cookie_is_read() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oa_visitor=x; oa_cloud_session=tok"),
        );
        assert_eq!(session(&headers).as_deref(), Some("tok"));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oa_cloud_session="),
        );
        assert_eq!(session(&headers), None);
    }

    #[test]
    fn a_checked_sign_in_is_reused_only_for_its_own_cookie() {
        let key = [42; 32];
        assert_eq!(remembered(&key), None);
        remember(key, "acct_9");
        assert_eq!(remembered(&key).as_deref(), Some("acct_9"));
        assert_eq!(remembered(&[43; 32]), None);
    }

    fn chat(owner: &str, id: &str, pending: bool) -> Conversation {
        Conversation {
            id: id.into(),
            owner: owner.into(),
            revision: 1,
            title: "A chat".into(),
            messages: vec![Message {
                role: Role::User,
                text: "Hello".into(),
                request_id: Some(id.into()),
            }],
            pending: pending.then(|| Pending {
                request_id: id.into(),
                started_unix: crate::chat_store::now_unix(),
                job_id: None,
            }),
            requests: vec![Request {
                id: id.into(),
                digest: "a".repeat(64),
                outcome: if pending {
                    Outcome::Pending
                } else {
                    Outcome::Answered
                },
                selection: None,
                cloud: None,
                reply: None,
            }],
            selection: None,
            updated_unix: 1,
            pinned_unix: None,
            archived_unix: None,
        }
    }

    #[tokio::test]
    async fn sign_in_moves_the_browsers_idle_chats_and_nobody_elses() {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::local(directory.path().join("chats"));
        let account = account_owner("acct_1");
        let idle = "12345678-1234-4234-8234-123456789abc";
        let busy = "22345678-1234-4234-8234-123456789abc";
        store.create(&chat(BROWSER, idle, false)).await.unwrap();
        store.create(&chat(BROWSER, busy, true)).await.unwrap();
        store.create(&chat(OTHER, idle, false)).await.unwrap();

        assert_eq!(adopt_all(&store, BROWSER, &account).await, 1);
        let mine = store.list(&account).await.unwrap();
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].id, idle);
        // The chat being answered stays with the browser.
        let left = store.list(BROWSER).await.unwrap();
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, busy);
        // Another browser's chat is untouched.
        assert_eq!(store.list(OTHER).await.unwrap().len(), 1);
        // Signing in again moves nothing more.
        assert_eq!(adopt_all(&store, BROWSER, &account).await, 0);
        // A second account never takes the first account's chats.
        let second = account_owner("acct_2");
        assert_eq!(adopt_all(&store, BROWSER, &second).await, 0);
        assert!(store.list(&second).await.unwrap().is_empty());
    }
}

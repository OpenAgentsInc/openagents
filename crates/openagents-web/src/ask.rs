//! `POST /ask`: the homepage terminal's questions (#10106).
//!
//! A visitor's question goes to the same OpenAgents chat worker the phone
//! and the desktop app talk to, as a NIP-CJ conversation job through
//! `relay.openagents.com` ([`openagents_chat::basic_coder::Relay`]), with
//! the website's surface (`web`). The worker answers it with prepared
//! answers, the knowledge base, or the model, under the website's
//! instructions, and never offers Coder, a computer, a command, or a
//! screen (`coder::router::policy::for_web`). Nothing here reaches a
//! computer, an account, or a Coder run.
//!
//! The site signs each visitor's jobs with a key it derives from the
//! visitor's cookie and a secret only the server holds, so the worker's
//! per-key quota counts each visitor apart and no key is ever sent to the
//! browser. On top of the worker's quotas the site holds each visitor to
//! one question at a time and [`PER_MINUTE`] a minute, and every visitor
//! together to [`IN_FLIGHT`] at once, in process memory.
//!
//! The reply streams back as newline-delimited JSON: `{"html": …}` as the
//! answer grows, drawn by [`crate::markdown::render`] (raw HTML shows as
//! text), then `{"done": true, "text": …}` or `{"error": …}`.

use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::{Body, Bytes};
use axum::extract::{DefaultBodyLimit, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use openagents_chat::basic_coder::{self, Door, Reply, Role, Turn};
use openagents_chat::router::{Context, Surface};
use secp256k1::SecretKey;
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::App;

/// The cookie that names a visitor: 32 random hex characters, nothing
/// else.
pub const COOKIE: &str = "oa_visitor";
/// The most turns of the conversation one question sends, newest kept.
pub const MAX_TURNS: usize = 8;
/// The longest turn, in characters.
pub const MAX_TURN_CHARS: usize = 4_000;
/// Questions one visitor may ask in a rolling minute.
pub const PER_MINUTE: usize = 6;
/// Questions every visitor together may have waiting at once.
pub const IN_FLIGHT: usize = 32;
/// The largest request body.
const MAX_BODY_BYTES: usize = 64 * 1024;
/// How often a waiting answer is read for new words.
const POLL: Duration = Duration::from_millis(100);
/// The most visitors the limiter remembers before it forgets idle ones.
const REMEMBERED: usize = 10_000;

/// Where questions are answered: the relay to the chat worker in
/// production, an in-process door in tests.
pub trait Chat: Send + Sync {
    /// A door that signs with `secret`.
    fn door(&self, secret: SecretKey) -> Result<Box<dyn Door>, String>;
}

/// The OpenAgents chat worker, through `relay.openagents.com`.
pub struct Worker;

impl Chat for Worker {
    fn door(&self, secret: SecretKey) -> Result<Box<dyn Door>, String> {
        Ok(Box::new(basic_coder::Relay::new(
            basic_coder::RELAY,
            basic_coder::WORKER,
            secret,
        )?))
    }
}

/// The site's side of the limits.
#[derive(Default)]
pub(crate) struct Limits {
    visitors: Mutex<HashMap<String, Visitor>>,
    waiting: AtomicUsize,
}

#[derive(Default)]
struct Visitor {
    asking: bool,
    asked: VecDeque<Instant>,
}

/// Why a question was not taken.
#[derive(Debug, PartialEq, Eq)]
enum Busy {
    Asking,
    Minute,
    Everyone,
}

impl Busy {
    fn message(&self) -> &'static str {
        match self {
            Busy::Asking => "We're still answering your last question.",
            Busy::Minute => "That's a lot of questions at once; try again in a minute.",
            Busy::Everyone => "We're answering many visitors right now; try again in a moment.",
        }
    }
}

impl Limits {
    fn take(self: &Arc<Self>, visitor: &str, now: Instant) -> Result<Held, Busy> {
        let mut visitors = self
            .visitors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if visitors.len() >= REMEMBERED {
            visitors.retain(|_, seen| {
                seen.asking
                    || seen
                        .asked
                        .back()
                        .is_some_and(|at| now.duration_since(*at) < Duration::from_secs(60))
            });
        }
        let seen = visitors.entry(visitor.to_string()).or_default();
        while seen
            .asked
            .front()
            .is_some_and(|at| now.duration_since(*at) >= Duration::from_secs(60))
        {
            seen.asked.pop_front();
        }
        if seen.asking {
            return Err(Busy::Asking);
        }
        if seen.asked.len() >= PER_MINUTE {
            return Err(Busy::Minute);
        }
        if self
            .waiting
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |waiting| {
                (waiting < IN_FLIGHT).then_some(waiting + 1)
            })
            .is_err()
        {
            return Err(Busy::Everyone);
        }
        seen.asking = true;
        seen.asked.push_back(now);
        Ok(Held {
            limits: self.clone(),
            visitor: visitor.to_string(),
        })
    }
}

/// A question being answered; dropping it frees the visitor's turn.
struct Held {
    limits: Arc<Limits>,
    visitor: String,
}

impl Drop for Held {
    fn drop(&mut self) {
        self.limits.waiting.fetch_sub(1, Ordering::SeqCst);
        if let Some(seen) = self
            .limits
            .visitors
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get_mut(&self.visitor)
        {
            seen.asking = false;
        }
    }
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/ask", post(ask))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
}

#[derive(Deserialize)]
struct Question {
    turns: Vec<Said>,
}

#[derive(Deserialize)]
struct Said {
    role: Role,
    text: String,
}

/// The visitor's cookie, when it is one this site could have set.
fn visitor(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_string())
        .filter(|value| value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

/// A new visitor's cookie value.
fn new_visitor() -> String {
    secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// The key a visitor's jobs are signed with: derived from the server's
/// secret and the cookie, never stored or sent.
pub(crate) fn key(salt: &[u8; 32], visitor: &str) -> SecretKey {
    let mut seed: [u8; 32] = Sha256::new()
        .chain_update(b"openagents-web-visitor-v1")
        .chain_update(salt)
        .chain_update(visitor.as_bytes())
        .finalize()
        .into();
    loop {
        if let Ok(key) = SecretKey::from_byte_array(seed) {
            return key;
        }
        seed = Sha256::digest(seed).into();
    }
}

/// The turns a question sends: the newest [`MAX_TURNS`], each bounded,
/// ending with the visitor's. `None` when there is nothing to ask.
fn turns(question: Question) -> Option<Vec<Turn>> {
    let start = question.turns.len().saturating_sub(MAX_TURNS);
    let turns: Vec<Turn> = question.turns[start..]
        .iter()
        .map(|said| {
            let text: String = said.text.trim().chars().take(MAX_TURN_CHARS).collect();
            match said.role {
                Role::User => Turn::user(text),
                Role::Assistant => Turn::assistant(text, None),
            }
        })
        .collect();
    let last = turns.last()?;
    (last.role == Role::User && !last.text.is_empty()).then_some(turns)
}

fn refusal(status: StatusCode, message: &str) -> Response {
    (status, axum::Json(json!({ "error": message }))).into_response()
}

async fn ask(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(turns) = serde_json::from_slice::<Question>(&body)
        .ok()
        .and_then(turns)
    else {
        return refusal(StatusCode::BAD_REQUEST, "Ask a question.");
    };
    let (visitor, set_cookie) = match visitor(&headers) {
        Some(visitor) => (visitor, None),
        None => {
            let visitor = new_visitor();
            let cookie = format!(
                "{COOKIE}={visitor}; Path=/; Max-Age=31536000; HttpOnly; SameSite=Lax{}",
                if app.config.secure_cookies {
                    "; Secure"
                } else {
                    ""
                }
            );
            (visitor, Some(cookie))
        }
    };
    let held = match app.limits.take(&visitor, Instant::now()) {
        Ok(held) => held,
        Err(busy) => return refusal(StatusCode::TOO_MANY_REQUESTS, busy.message()),
    };
    let door = match app.config.chat.door(key(&app.config.ask_salt, &visitor)) {
        Ok(door) => door,
        Err(why) => {
            eprintln!("openagents-web: the chat door did not open: {why}");
            return refusal(
                StatusCode::SERVICE_UNAVAILABLE,
                "We can't answer right now; try again soon.",
            );
        }
    };
    let (lines, receiver) = tokio::sync::mpsc::channel::<Bytes>(32);
    tokio::spawn(answer(door, turns, lines, held));
    let stream = futures_util::stream::unfold(receiver, |mut receiver| async move {
        receiver
            .recv()
            .await
            .map(|line| (Ok::<_, std::convert::Infallible>(line), receiver))
    });
    let mut response = Response::new(Body::from_stream(stream));
    let response_headers = response.headers_mut();
    response_headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/x-ndjson; charset=utf-8"),
    );
    response_headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Some(cookie) = set_cookie.and_then(|cookie| HeaderValue::from_str(&cookie).ok()) {
        response_headers.insert(header::SET_COOKIE, cookie);
    }
    response
}

fn line(value: &serde_json::Value) -> Bytes {
    Bytes::from(format!("{value}\n"))
}

/// Asks the door and writes the answer as it grows, then how it ended.
async fn answer(
    door: Box<dyn Door>,
    turns: Vec<Turn>,
    lines: tokio::sync::mpsc::Sender<Bytes>,
    held: Held,
) {
    let _held = held;
    let context = Context {
        surface: Surface::Web,
        ..Context::default()
    };
    let reply = Arc::new(Mutex::new(Reply::default()));
    let job = door.ask(turns, context, reply.clone());
    tokio::pin!(job);
    let mut asked = false;
    let mut shown = String::new();
    loop {
        tokio::select! {
            () = &mut job, if !asked => asked = true,
            () = tokio::time::sleep(POLL) => {}
        }
        let (text, done, failure) = {
            let reply = basic_coder::lock(&reply);
            (reply.text.clone(), reply.done, reply.failure.clone())
        };
        if text != shown {
            shown = text;
            let html = crate::markdown::render(&shown);
            if lines.send(line(&json!({ "html": html }))).await.is_err() {
                return;
            }
        }
        let last = if done {
            json!({ "done": true, "text": shown })
        } else if let Some(failure) = failure {
            json!({ "error": failure.describe() })
        } else if asked {
            json!({ "error": "We didn't get an answer; try asking again." })
        } else {
            continue;
        };
        let _ = lines.send(line(&last)).await;
        return;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_visitor_asks_one_question_at_a_time_and_six_a_minute() {
        let limits = Arc::new(Limits::default());
        let start = Instant::now();
        let first = limits.take("a", start).unwrap();
        assert_eq!(limits.take("a", start).err(), Some(Busy::Asking));
        let other = limits.take("b", start).unwrap();
        drop(first);
        for _ in 1..PER_MINUTE {
            drop(limits.take("a", start).unwrap());
        }
        assert_eq!(limits.take("a", start).err(), Some(Busy::Minute));
        drop(limits.take("a", start + Duration::from_secs(61)).unwrap());
        drop(other);
        assert_eq!(limits.waiting.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn everyone_together_waits_on_at_most_the_in_flight_bound() {
        let limits = Arc::new(Limits::default());
        let now = Instant::now();
        let held: Vec<Held> = (0..IN_FLIGHT)
            .map(|n| limits.take(&n.to_string(), now).unwrap())
            .collect();
        assert_eq!(limits.take("late", now).err(), Some(Busy::Everyone));
        drop(held);
        assert!(limits.take("late", now).is_ok());
    }

    #[test]
    fn each_visitor_signs_with_a_key_of_its_own_that_the_salt_hides() {
        let salt = [7; 32];
        assert_eq!(key(&salt, "a"), key(&salt, "a"));
        assert_ne!(key(&salt, "a"), key(&salt, "b"));
        assert_ne!(key(&salt, "a"), key(&[8; 32], "a"));
    }

    #[test]
    fn only_a_cookie_this_site_could_set_names_a_visitor() {
        let mut headers = HeaderMap::new();
        let id = new_visitor();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_str(&format!("x=1; {COOKIE}={id}")).unwrap(),
        );
        assert_eq!(visitor(&headers), Some(id));
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("oa_visitor=../../etc"),
        );
        assert_eq!(visitor(&headers), None);
    }

    #[test]
    fn a_question_keeps_the_newest_turns_and_ends_with_the_visitor() {
        let said = |role, text: &str| Said {
            role,
            text: text.to_string(),
        };
        let mut many: Vec<Said> = (0..12)
            .map(|n| {
                said(
                    if n % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    "x",
                )
            })
            .collect();
        many.push(said(Role::User, &"y".repeat(MAX_TURN_CHARS + 10)));
        let kept = turns(Question { turns: many }).unwrap();
        assert_eq!(kept.len(), MAX_TURNS);
        assert_eq!(kept.last().unwrap().text.chars().count(), MAX_TURN_CHARS);
        assert!(
            turns(Question {
                turns: vec![said(Role::Assistant, "hi")]
            })
            .is_none()
        );
        assert!(
            turns(Question {
                turns: vec![said(Role::User, "  ")]
            })
            .is_none()
        );
        assert!(turns(Question { turns: vec![] }).is_none());
    }
}

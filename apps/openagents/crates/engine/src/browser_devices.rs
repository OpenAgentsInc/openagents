//! Browsers on openagents.com as clients of this computer (#12264).
//!
//! The relay hands every `web-` connection to [`Gate`] instead of the plain
//! engine service. A browser's connection can ask for access, follow its
//! request, prove its key with a hello (`coder_access::browser`), and give
//! its access back. Nothing else reaches the engine until the hello names a
//! key this computer granted, and every call after that checks the grant
//! again, so a removed browser's next call is refused. Its grant
//! ([`coder_access::Rights::browser`]) is issued only when the person at
//! this computer allows the request in the computer's own app
//! ([`Browsers::answer`], served by [`crate::connect_phone::LocalRpc`]).
//!
//! What a browser may call is [`ALLOWED`], reads of the computer's
//! harnesses, models, skills, commands, and workspace files under the
//! grant's observe right, and [`OPERATE`], the changes its run right
//! covers: the chat queue, answers to an agent's questions, and edits to
//! workspace files. Later work widens them method by method.

use async_trait::async_trait;
use coder_access::protocol::DeviceState;
use coder_access::{Right, Rights};
use openagents_rpc::browsers::{
    AskAccess, BROWSER_FORGET, BROWSER_HELLO, BROWSER_REQUEST, BROWSER_REQUEST_STATUS, Browser,
    BrowserRequest, Hello, RequestState, State,
};
use openagents_rpc::{RpcError, RpcReply, RpcService, methods};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// How long a request waits for the person at the computer, in seconds.
pub const REQUEST_TTL: u64 = 10 * 60;
/// At most this many requests wait at once.
pub const MAX_WAITING: usize = 8;

/// The engine methods a browser may call.
pub const ALLOWED: &[&str] = &[
    methods::ENGINE_INFO,
    methods::LIST_HARNESSES,
    methods::LIST_MODELS,
    methods::LIST_SKILLS,
    methods::LIST_COMMANDS,
    methods::LIST_WORKSPACE_DIRECTORY,
    methods::READ_WORKSPACE_FILE,
    methods::READ_WORKSPACE_IMAGE,
    methods::READ_WORKSPACE_BYTES,
    methods::SEARCH_WORKSPACE_FILES,
    methods::SEARCH_WORKSPACE_CONTENT,
    // The composer's @ mentions (#12267).
    methods::SEARCH_FILES,
    // The queue panel's view of a chat's queue (#12268).
    methods::WATCH_QUEUE,
    // The file panel follows changes on the computer (#12272).
    methods::WATCH_WORKSPACE_FILES,
];

/// The engine methods a browser may call under its grant's run right
/// ([`Right::Operate`]).
pub const OPERATE: &[&str] = &[
    // The queue panel and the question wizard (#12268).
    methods::QUEUE_MESSAGE,
    methods::UPDATE_QUEUED_MESSAGE,
    methods::MOVE_QUEUED_MESSAGE,
    methods::REMOVE_QUEUED_MESSAGE,
    methods::SEND_QUEUED_MESSAGE_NOW,
    methods::STEER_QUEUED_MESSAGE_NOW,
    methods::QUEUE_COMMAND,
    // The file panel's edits (#12272).
    methods::WRITE_WORKSPACE_FILE,
    methods::MOVE_WORKSPACE_ENTRY,
    methods::DELETE_WORKSPACE_ENTRY,
];

/// The right `method` needs, or `None` when a browser can't call it.
pub fn required_right(method: &str) -> Option<Right> {
    if ALLOWED.contains(&method) {
        Some(Right::Observe)
    } else if OPERATE.contains(&method) {
        Some(Right::Operate)
    } else {
        None
    }
}

const ALLOW_FIRST: &str = "Allow this browser on the computer first.";
const ENDED: &str = "This browser's access to this computer ended. Ask again.";
const UNAVAILABLE: &str = "This computer can't take browsers right now.";

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now() -> u64 {
    coder_access::unix_time().unwrap_or(0)
}

#[derive(Clone)]
struct Request {
    id: String,
    key: String,
    label: String,
    at: u64,
    state: State,
}

/// The grant store and the relay its grants name, once the device
/// connection runs.
#[derive(Clone)]
struct Host {
    authority: Arc<coder_host::authority::Authority>,
    relay: String,
}

/// This computer's browsers: waiting requests and the grant checks.
pub struct Browsers {
    /// This computer's device ID: the device room a browser's hello names.
    device: String,
    inner: Arc<dyn RpcService>,
    host: Mutex<Option<Host>>,
    requests: Mutex<Vec<Request>>,
    /// What allowed browsers call themselves, while this process runs.
    labels: Mutex<HashMap<String, String>>,
}

impl Browsers {
    pub fn new(device: impl Into<String>, inner: Arc<dyn RpcService>) -> Arc<Self> {
        Arc::new(Self {
            device: device.into(),
            inner,
            host: Mutex::new(None),
            requests: Mutex::new(Vec::new()),
            labels: Mutex::new(HashMap::new()),
        })
    }

    /// Serve browsers from `running`'s grant store. Until then every
    /// request is refused.
    pub fn attach(&self, running: &coder_host::Running) {
        let Some(relay) = running.relay() else {
            return;
        };
        *lock(&self.host) = Some(Host {
            authority: running.authority().clone(),
            relay: relay.to_owned(),
        });
    }

    fn host(&self) -> Result<Host, RpcError> {
        lock(&self.host)
            .clone()
            .ok_or_else(|| RpcError::Failed(UNAVAILABLE.into()))
    }

    /// `key`'s live browser grant: its ID and expiry. A revoked, expired,
    /// or non-browser grant is none.
    fn grant(&self, key: &str) -> Option<(String, u64)> {
        let host = lock(&self.host).clone()?;
        host.authority
            .devices()?
            .into_iter()
            .find(|entry| {
                entry.device == key
                    && entry.scope.is_none()
                    && entry.state == DeviceState::Active
                    && entry.rights == Rights::browser()
                    && entry.expires_at > now()
            })
            .map(|entry| (entry.grant, entry.expires_at))
    }

    fn ask(&self, ask: AskAccess) -> Result<RequestState, RpcError> {
        let key = ask.key.trim().to_owned();
        if key.len() != 64
            || !key
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(RpcError::BadParams("key".into()));
        }
        self.host()?;
        let label: String = ask
            .label
            .chars()
            .filter(|c| !c.is_control())
            .take(80)
            .collect();
        let label = if label.trim().is_empty() {
            "A browser".to_owned()
        } else {
            label.trim().to_owned()
        };
        let at = now();
        let mut requests = lock(&self.requests);
        requests.retain(|r| r.at + REQUEST_TTL > at || r.state == State::Allowed);
        if self.grant(&key).is_some() {
            let id = uuid::Uuid::new_v4().simple().to_string();
            requests.push(Request {
                id: id.clone(),
                key,
                label,
                at,
                state: State::Allowed,
            });
            return Ok(RequestState {
                request: id,
                state: State::Allowed,
            });
        }
        if let Some(waiting) = requests
            .iter()
            .find(|r| r.key == key && r.state == State::Asked)
        {
            return Ok(RequestState {
                request: waiting.id.clone(),
                state: State::Asked,
            });
        }
        if requests.iter().filter(|r| r.state == State::Asked).count() >= MAX_WAITING {
            return Err(RpcError::Failed(
                "Too many browsers are waiting on this computer. Try again later.".into(),
            ));
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        requests.push(Request {
            id: id.clone(),
            key,
            label,
            at,
            state: State::Asked,
        });
        Ok(RequestState {
            request: id,
            state: State::Asked,
        })
    }

    fn status(&self, id: &str) -> RequestState {
        let at = now();
        let state = lock(&self.requests)
            .iter()
            .find(|r| r.id == id)
            .map(|r| match r.state {
                State::Asked if r.at + REQUEST_TTL <= at => State::Expired,
                state => state,
            })
            .unwrap_or(State::Expired);
        RequestState {
            request: id.to_owned(),
            state,
        }
    }

    /// The requests waiting for the person at this computer.
    pub fn waiting(&self) -> Vec<BrowserRequest> {
        let at = now();
        lock(&self.requests)
            .iter()
            .filter(|r| r.state == State::Asked && r.at + REQUEST_TTL > at)
            .map(|r| BrowserRequest {
                id: r.id.clone(),
                label: r.label.clone(),
                key: r.key.clone(),
                requested_at: r.at,
            })
            .collect()
    }

    /// The person at this computer allowed or denied request `id`. Allowing
    /// issues the browser's grant.
    pub fn answer(&self, id: &str, allow: bool) -> Result<(), RpcError> {
        let at = now();
        let (key, label) = {
            let requests = lock(&self.requests);
            let request = requests
                .iter()
                .find(|r| r.id == id && r.state == State::Asked && r.at + REQUEST_TTL > at)
                .ok_or_else(|| RpcError::Failed("That request isn't waiting anymore.".into()))?;
            (request.key.clone(), request.label.clone())
        };
        if allow {
            let host = self.host()?;
            host.authority
                .local(|access, now| access.approve_browser(&key, &host.relay, now))
                .map_err(|error| {
                    tracing::warn!(%error, "browser grant refused");
                    RpcError::Failed("Couldn't allow this browser. Try again.".into())
                })?;
            lock(&self.labels).insert(key.clone(), label);
        }
        if let Some(request) = lock(&self.requests).iter_mut().find(|r| r.id == id) {
            request.state = if allow { State::Allowed } else { State::Denied };
        }
        Ok(())
    }

    /// The browsers allowed on this computer now.
    pub fn allowed(&self) -> Vec<Browser> {
        let Some(host) = lock(&self.host).clone() else {
            return Vec::new();
        };
        let labels = lock(&self.labels).clone();
        let at = now();
        host.authority
            .devices()
            .unwrap_or_default()
            .into_iter()
            .filter(|entry| {
                entry.scope.is_none()
                    && entry.state == DeviceState::Active
                    && entry.rights == Rights::browser()
                    && entry.expires_at > at
            })
            .map(|entry| Browser {
                label: labels
                    .get(&entry.device)
                    .cloned()
                    .unwrap_or_else(|| "A browser".into()),
                key: entry.device,
                expires_at: entry.expires_at,
                last_seen: entry.last_seen,
            })
            .collect()
    }

    /// Remove browser `key`'s access. Its next call is refused.
    pub fn revoke(&self, key: &str) -> Result<(), RpcError> {
        if self.grant(key).is_none() {
            return Ok(());
        }
        let host = self.host()?;
        host.authority
            .local(|access, now| access.revoke(key, now).map(drop))
            .map_err(|error| {
                tracing::warn!(%error, "browser revoke refused");
                RpcError::Failed("Couldn't remove this browser. Try again.".into())
            })
    }
}

/// [`openagents_rpc::BrowserGate`] over [`Browsers`].
pub struct Gate(pub Arc<Browsers>);

impl openagents_rpc::BrowserGate for Gate {
    fn service(&self, conn_id: &str) -> Arc<dyn RpcService> {
        Arc::new(Connection {
            browsers: self.0.clone(),
            conn: conn_id.to_owned(),
            key: Mutex::new(None),
        })
    }
}

/// One browser connection: nothing reaches the engine before its hello.
struct Connection {
    browsers: Arc<Browsers>,
    conn: String,
    key: Mutex<Option<String>>,
}

fn params<T: serde::de::DeserializeOwned>(params: serde_json::Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|error| RpcError::BadParams(error.to_string()))
}

#[async_trait]
impl RpcService for Connection {
    async fn handle(&self, method: &str, value: serde_json::Value) -> Result<RpcReply, RpcError> {
        let browsers = &self.browsers;
        match method {
            BROWSER_REQUEST => {
                let ask: AskAccess = params(value)?;
                RpcReply::value(&browsers.ask(ask)?)
            }
            BROWSER_REQUEST_STATUS => {
                #[derive(serde::Deserialize)]
                struct Status {
                    request: String,
                }
                let Status { request } = params(value)?;
                RpcReply::value(&browsers.status(&request))
            }
            BROWSER_HELLO => {
                #[derive(serde::Deserialize)]
                struct Signed {
                    event: coder_access::browser::Event,
                }
                let Signed { event } = params(value)?;
                let key = coder_access::browser::verify_hello(
                    &event,
                    &browsers.device,
                    &self.conn,
                    now(),
                )
                .map_err(|_| RpcError::Failed(ALLOW_FIRST.into()))?;
                let (grant, expires_at) = browsers
                    .grant(&key)
                    .ok_or_else(|| RpcError::Failed(ALLOW_FIRST.into()))?;
                if let Ok(host) = browsers.host() {
                    host.authority.touch(&key, &grant);
                }
                *lock(&self.key) = Some(key);
                RpcReply::value(&Hello {
                    rights: Rights::browser()
                        .iter()
                        .map(|right| right.as_str().to_owned())
                        .collect(),
                    expires_at,
                })
            }
            BROWSER_FORGET => {
                let key = lock(&self.key)
                    .take()
                    .ok_or_else(|| RpcError::Failed(ALLOW_FIRST.into()))?;
                let browsers = browsers.clone();
                tokio::task::spawn_blocking(move || browsers.revoke(&key))
                    .await
                    .map_err(|_| RpcError::Failed(UNAVAILABLE.into()))??;
                RpcReply::value(&serde_json::json!({}))
            }
            _ => {
                let key = lock(&self.key)
                    .clone()
                    .ok_or_else(|| RpcError::Failed(ALLOW_FIRST.into()))?;
                if browsers.grant(&key).is_none() {
                    *lock(&self.key) = None;
                    return Err(RpcError::Failed(ENDED.into()));
                }
                // `grant` admits only a browser grant, so its rights are
                // `Rights::browser()`.
                if !required_right(method).is_some_and(|right| Rights::browser().contains(right)) {
                    return Err(RpcError::Failed(
                        "A browser can't do that on this computer yet.".into(),
                    ));
                }
                browsers.inner.handle(method, value).await
            }
        }
    }
}

#[cfg(test)]
#[path = "browser_devices_tests.rs"]
mod tests;

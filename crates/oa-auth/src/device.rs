//! The account service's device sign-in and app-session routes, as plain
//! functions over the tenancy stores (RFC 8628 shape; see
//! `tenancy::sessions` and docs/auth). The gateway and the local fixture's
//! account service both call these, so the two answer the same way.
//!
//! | Route | Caller | Function |
//! | --- | --- | --- |
//! | `POST /v1/sessions/device` `{app, computer}` | the app (through the web server) | [`start`] |
//! | `POST /v1/sessions/device/poll` `{device_code}` | the app (through the web server) | [`poll`] |
//! | `POST /v1/sessions/device/lookup` `{user_code}` | a browser session | [`lookup`] |
//! | `POST /v1/sessions/device/decide` `{user_code, approve}` | a browser session | [`decide`] |
//! | `POST /v1/sessions/device/paired` `{pair}` | a browser session | [`paired`] |
//! | `GET /v1/account/sessions` | a session | [`list`] |
//! | `DELETE /v1/account/sessions/{id}` | a session | [`revoke`] |
//!
//! Every answer is the account surface's envelope: a
//! `openagents.accounts.v1` document, or `{"error": {"code", "message"}}`.
//! The device code and the session token appear only in the response
//! that minted them; codes are stored as digests.

use std::path::Path;

use serde_json::{Value, json};
use tenancy::Accounts;
use tenancy::sessions::{
    self, AppLabel, DEVICE_CODE_TTL, DevicePoll, DeviceRefusal, Refusal, SessionId, Sessions,
};
use tenancy::workspaces::UserId;

pub use tenancy::sessions::normalize_user_code;

/// An HTTP answer: status and JSON body.
#[derive(Debug)]
pub struct Answer {
    pub status: u16,
    pub body: Value,
}

fn ok(mut body: Value) -> Answer {
    body["v"] = json!("openagents.accounts.v1");
    Answer { status: 200, body }
}

fn refused(status: u16, code: &str, message: impl Into<String>) -> Answer {
    Answer {
        status,
        body: json!({"error": {"code": code, "message": message.into()}}),
    }
}

fn unavailable() -> Answer {
    refused(
        503,
        "sessions_unavailable",
        "Sign-in isn't available right now. Try again in a minute.",
    )
}

fn from_refusal(refusal: &Refusal) -> Answer {
    match refusal {
        Refusal::Device(device) => {
            let status = match device {
                DeviceRefusal::Busy => 429,
                _ => 400,
            };
            refused(status, device.code(), device.to_string())
        }
        Refusal::UnknownSession(_) | Refusal::SessionClosed { .. } => {
            refused(404, "unknown_session", "That sign-in isn't active.")
        }
        _ => unavailable(),
    }
}

fn text<'a>(body: &'a Value, name: &str) -> Option<&'a str> {
    body.get(name)
        .and_then(Value::as_str)
        .filter(|v| v.len() <= 256)
}

fn store(dir: &Path) -> Result<Sessions, Answer> {
    Sessions::open(dir).map_err(|_| unavailable())
}

fn access(
    actor: &str,
    action: &str,
    session: Option<String>,
    detail: Option<String>,
) -> sessions::Access {
    sessions::Access {
        at: 0,
        actor: actor.to_string(),
        action: action.to_string(),
        workspace: None,
        session,
        detail,
    }
}

fn push(log: &mut Vec<sessions::Access>, now: u64, mut event: sessions::Access) {
    event.at = now;
    sessions::push_access(log, event);
}

/// `POST /v1/sessions/device` — `{app, computer, pair?}`. Answers `device_code`,
/// `user_code`, `expires_in`, and `interval`; the web server adds the
/// verification addresses, which only it knows.
pub fn start(dir: &Path, body: &Value) -> Answer {
    let label = match AppLabel::new(
        text(body, "app").unwrap_or_default(),
        text(body, "computer").unwrap_or_default(),
    ) {
        Ok(label) => label,
        Err(refusal) => return from_refusal(&refusal),
    };
    let sessions = match store(dir) {
        Ok(sessions) => sessions,
        Err(answer) => return answer,
    };
    match sessions.mutate(|book, log, now| {
        let issued = book.start_paired_device(label, text(body, "pair"), now)?;
        push(
            log,
            now,
            access(
                "anonymous",
                "device-start",
                None,
                Some(format!(
                    "{} on {}",
                    issued.grant.label.app, issued.grant.label.computer
                )),
            ),
        );
        Ok(issued)
    }) {
        Ok(issued) => ok(json!({
            "device_code": issued.device_code,
            "user_code": issued.user_code,
            "expires_in": DEVICE_CODE_TTL,
            "interval": issued.grant.interval,
        })),
        Err(refusal) => from_refusal(&refusal),
    }
}

/// `POST /v1/sessions/device/poll` — `{device_code}`. Answers the session
/// once the person approved: `{session, token, account: {id, label}}`.
/// Until then `400` with `authorization_pending` or `slow_down` (with the
/// new `interval`); `expired_token`, `access_denied`, or `invalid_grant`
/// end the flow.
pub fn poll(dir: &Path, body: &Value) -> Answer {
    let Some(code) = text(body, "device_code").filter(|c| c.starts_with("dvc_")) else {
        return refused(400, "invalid_grant", "Send the device_code.");
    };
    let sessions = match store(dir) {
        Ok(sessions) => sessions,
        Err(answer) => return answer,
    };
    let polled = sessions.mutate(|book, log, now| {
        let polled = book.poll_device(code, now)?;
        if let DevicePoll::Issued(issued, account) = &polled {
            push(
                log,
                now,
                access(
                    account.as_str(),
                    "device-sign-in",
                    Some(issued.session.id.as_str().to_string()),
                    issued
                        .session
                        .app
                        .as_ref()
                        .map(|l| format!("{} on {}", l.app, l.computer)),
                ),
            );
        }
        Ok(polled)
    });
    match polled {
        Ok(DevicePoll::Pending) => refused(
            400,
            "authorization_pending",
            "Waiting for you to approve on the website.",
        ),
        Ok(DevicePoll::SlowDown(interval)) => {
            let mut answer = refused(400, "slow_down", "Polling too fast.");
            answer.body["interval"] = json!(interval);
            answer
        }
        Ok(DevicePoll::Issued(issued, account)) => {
            let label = Accounts::open(dir)
                .and_then(|a| a.store())
                .ok()
                .and_then(|store| {
                    store
                        .accounts
                        .get(account.as_str())
                        .map(|a| a.label.clone())
                })
                .unwrap_or_else(|| account.as_str().to_string());
            ok(json!({
                "session": session_json(&issued.session, None),
                "token": issued.once,
                "account": {"id": account.as_str(), "label": label},
            }))
        }
        Err(refusal) => from_refusal(&refusal),
    }
}

/// Approval must come from a browser session: an app's own session, an
/// API key, or an unknown session can't approve another computer.
fn browser_session(dir: &Path, account: &str, session: Option<&str>) -> Result<(), Answer> {
    let denied = || {
        refused(
            403,
            "browser_session_required",
            "Approve sign-ins from the website while signed in.",
        )
    };
    let session = session.ok_or_else(denied)?;
    let store = store(dir)?.store().map_err(|_| unavailable())?;
    let record = store
        .book
        .session(&SessionId::from(session))
        .ok_or_else(denied)?;
    if record.user.as_str() != account || record.app.is_some() {
        return Err(denied());
    }
    Ok(())
}

fn grant_json(grant: &sessions::DeviceGrant) -> Value {
    json!({
        "app": grant.label.app,
        "computer": grant.label.computer,
        "created_at": grant.created_at,
        "expires_at": grant.expires_at,
    })
}

/// `POST /v1/sessions/device/lookup` — `{user_code}` under a browser
/// session: what the approval page shows.
pub fn lookup(dir: &Path, account: &str, session: Option<&str>, body: &Value) -> Answer {
    if let Err(answer) = browser_session(dir, account, session) {
        return answer;
    }
    let code = text(body, "user_code").unwrap_or_default();
    let store = match store(dir).and_then(|s| s.store().map_err(|_| unavailable())) {
        Ok(store) => store,
        Err(answer) => return answer,
    };
    match store.book.device_of_user_code(code, now()) {
        Ok(grant) => ok(json!({"device": grant_json(grant)})),
        Err(refusal) => from_refusal(&refusal),
    }
}

/// `POST /v1/sessions/device/decide` — `{user_code, approve}` under a
/// browser session: the person's Approve or Deny.
pub fn decide(dir: &Path, account: &str, session: Option<&str>, body: &Value) -> Answer {
    if let Err(answer) = browser_session(dir, account, session) {
        return answer;
    }
    let code = text(body, "user_code").unwrap_or_default();
    let Some(approve) = body.get("approve").and_then(Value::as_bool) else {
        return refused(400, "invalid_request", "Send approve: true or false.");
    };
    let sessions = match store(dir) {
        Ok(sessions) => sessions,
        Err(answer) => return answer,
    };
    let user = UserId::from(account);
    match sessions.mutate(|book, log, now| {
        let grant = book.decide_device(code, &user, approve, now)?;
        push(
            log,
            now,
            access(
                account,
                if approve {
                    "device-approve"
                } else {
                    "device-deny"
                },
                session.map(str::to_string),
                Some(format!("{} on {}", grant.label.app, grant.label.computer)),
            ),
        );
        Ok(grant)
    }) {
        Ok(grant) => ok(json!({
            "device": grant_json(&grant),
            "approved": approve,
        })),
        Err(refusal) => from_refusal(&refusal),
    }
}

/// `POST /v1/sessions/device/paired` — `{pair}` under a browser session:
/// the sign-ins waiting that were started with this pair code, newest
/// first, each with the code its app shows (`{devices: [{user_code, app,
/// computer, created_at, expires_at}]}`). The "Connect your terminal" page
/// shows them with Approve right there.
pub fn paired(dir: &Path, account: &str, session: Option<&str>, body: &Value) -> Answer {
    if let Err(answer) = browser_session(dir, account, session) {
        return answer;
    }
    let pair = text(body, "pair").unwrap_or_default();
    let store = match store(dir).and_then(|s| s.store().map_err(|_| unavailable())) {
        Ok(store) => store,
        Err(answer) => return answer,
    };
    let devices: Vec<Value> = store
        .book
        .paired_devices(pair, now())
        .into_iter()
        .take(8)
        .map(|(grant, user_code)| {
            let mut device = grant_json(grant);
            device["user_code"] = json!(user_code);
            device
        })
        .collect();
    ok(json!({ "devices": devices }))
}

fn session_json(session: &sessions::Session, current: Option<&str>) -> Value {
    let mut value = json!({
        "id": session.id.as_str(),
        "kind": "user",
        "account": session.user.as_str(),
        "created_at": session.created_at,
        "expires_at": session.expires_at,
    });
    if let Some(label) = &session.app {
        value["app"] = json!(label.app);
        value["computer"] = json!(label.computer);
    }
    if let Some(current) = current {
        value["current"] = json!(session.id.as_str() == current);
    }
    value
}

/// `GET /v1/account/sessions` — the account's signed-in apps and
/// computers.
pub fn list(dir: &Path, account: &str, session: Option<&str>) -> Answer {
    let store = match store(dir).and_then(|s| s.store().map_err(|_| unavailable())) {
        Ok(store) => store,
        Err(answer) => return answer,
    };
    let sessions: Vec<Value> = store
        .book
        .app_sessions(&UserId::from(account), now())
        .into_iter()
        .map(|s| session_json(s, Some(session.unwrap_or_default())))
        .collect();
    ok(json!({ "sessions": sessions }))
}

/// `DELETE /v1/account/sessions/{id}` — sign one of the account's own
/// sessions out (Settings' Remove).
pub fn revoke(dir: &Path, account: &str, session: Option<&str>, id: &str) -> Answer {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return refused(404, "unknown_session", "That sign-in isn't active.");
    }
    let sessions = match store(dir) {
        Ok(sessions) => sessions,
        Err(answer) => return answer,
    };
    let user = UserId::from(account);
    match sessions.mutate(|book, log, now| {
        let ended = book.revoke_session(&user, &SessionId::from(id), now)?;
        push(
            log,
            now,
            access(
                account,
                "session-revoke",
                session.map(str::to_string),
                Some(ended.id.as_str().to_string()),
            ),
        );
        Ok(ended)
    }) {
        Ok(ended) => ok(json!({"session": {"id": ended.id.as_str(), "state": "revoked"}})),
        Err(refusal) => from_refusal(&refusal),
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

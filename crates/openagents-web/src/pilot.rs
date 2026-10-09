//! The frozen Coder service offer and explicitly configured private intake.
//! Contact content is never echoed, logged, or sent to a model or public event.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use coder::task::sales::{Store, intake};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use maud::PreEscaped;

use crate::App;
use crate::layout::{escape, problem};
use crate::ui_page::{UiPage, prose};

const COOKIE: &str = "oa_pilot";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Configuration {
    /// Explicit existing host task root, never the default owner home.
    pub root: PathBuf,
    /// The create-only token file, outside the private pipeline directory.
    pub credential: PathBuf,
}

/// Reads an owner-provisioned file without accepting a symlink or public mode.
pub fn private_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let metadata =
        std::fs::symlink_metadata(path).map_err(|_| "private configuration is unavailable")?;
    if !metadata.is_file() || metadata.len() > 16 * 1024 {
        return Err("private configuration must be a bounded regular file".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("private configuration must have mode 0600".into());
        }
    }
    let file = std::fs::File::open(path).map_err(|_| "private configuration is unavailable")?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let opened = file
            .metadata()
            .map_err(|_| "private configuration is unavailable")?;
        if opened.dev() != metadata.dev() || opened.ino() != metadata.ino() {
            return Err("private configuration changed while opening".into());
        }
    }
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "private configuration is unavailable")?;
    if bytes.len() > 16 * 1024 {
        return Err("private configuration exceeds bound".into());
    }
    serde_json::from_slice(&bytes).map_err(|_| "private configuration is malformed".into())
}

pub struct Intake {
    root: PathBuf,
    secret: String,
    origin: String,
    limits: Mutex<Limits>,
    slots: Arc<tokio::sync::Semaphore>,
}

impl Intake {
    pub fn load(path: &Path) -> Result<Self, String> {
        let configuration: Configuration = private_json(path)?;
        Self::new(configuration.root, &configuration.credential)
    }

    pub fn new(root: PathBuf, credential: &Path) -> Result<Self, String> {
        if !root.join("sales/state.json").is_file() {
            return Err("private pipeline is unavailable".into());
        }
        let secret = Store::read_credential(credential)?;
        let mut store = Store::open(&root)?;
        let access = store.authenticate_intake(&secret)?;
        let origin = store.intake_policy(&access)?.origin;
        Ok(Self {
            root,
            secret,
            origin,
            limits: Mutex::default(),
            slots: Arc::new(tokio::sync::Semaphore::new(4)),
        })
    }

    fn policy(&self) -> Result<intake::Policy, String> {
        let mut store = Store::open(&self.root)?;
        let access = store.authenticate_intake(&self.secret)?;
        store.intake_policy(&access)
    }

    fn submit(&self, submission: intake::Submission) -> Result<intake::Acknowledgment, String> {
        let mut store = Store::open(&self.root)?;
        let access = store.authenticate_intake(&self.secret)?;
        store.submit_intake(&access, &submission)
    }

    #[cfg(test)]
    fn sign(&self, cookie: &str, payload: &[u8]) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(self.secret.as_bytes()).expect("HMAC accepts this key");
        mac.update(cookie.as_bytes());
        mac.update(b":pilot:");
        mac.update(payload);
        hex(&mac.finalize().into_bytes())
    }

    #[cfg(test)]
    fn ticket(&self, cookie: &str, referral: Option<String>) -> String {
        let ticket = Ticket {
            request: random(),
            issued_at: coder::task::sales::unix_now(),
            referral,
        };
        let bytes = serde_json::to_vec(&ticket).expect("ticket serializes");
        format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(&bytes),
            self.sign(cookie, &bytes)
        )
    }

    fn verify(&self, cookie: &str, value: &str) -> Result<Ticket, ()> {
        if value.len() > 1024 {
            return Err(());
        }
        let (payload, signature) = value.split_once('.').ok_or(())?;
        let bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|_| ())?;
        let signature: Vec<u8> =
            if signature.len() == 64 && signature.bytes().all(|b| b.is_ascii_hexdigit()) {
                (0..64)
                    .step_by(2)
                    .map(|i| u8::from_str_radix(&signature[i..i + 2], 16).map_err(|_| ()))
                    .collect::<Result<_, _>>()?
            } else {
                return Err(());
            };
        let mut mac = Hmac::<Sha256>::new_from_slice(self.secret.as_bytes()).map_err(|_| ())?;
        mac.update(cookie.as_bytes());
        mac.update(b":pilot:");
        mac.update(&bytes);
        mac.verify_slice(&signature).map_err(|_| ())?;
        let ticket: Ticket = serde_json::from_slice(&bytes).map_err(|_| ())?;
        let now = coder::task::sales::unix_now();
        if ticket.issued_at > now || now - ticket.issued_at > 1800 || !opaque(&ticket.request) {
            return Err(());
        }
        Ok(ticket)
    }

    fn allowed(&self, visitor: &str, post: bool) -> bool {
        self.limits.lock().is_ok_and(|mut l| l.admit(visitor, post))
    }
}

#[derive(Default)]
struct Limits {
    minute: u64,
    total: u16,
    visitors: BTreeMap<String, (u8, u8)>,
}
impl Limits {
    fn admit(&mut self, visitor: &str, post: bool) -> bool {
        let minute = coder::task::sales::unix_now() / 60;
        if self.minute != minute {
            self.minute = minute;
            self.total = 0;
            self.visitors.clear();
        }
        if self.total >= 120 || (!self.visitors.contains_key(visitor) && self.visitors.len() >= 256)
        {
            return false;
        }
        let counts = self.visitors.entry(visitor.into()).or_default();
        let (count, cap) = if post {
            (&mut counts.1, 8)
        } else {
            (&mut counts.0, 20)
        };
        if *count >= cap {
            return false;
        }
        *count += 1;
        self.total += 1;
        true
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ticket {
    request: String,
    issued_at: u64,
    referral: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    ticket: String,
    email: String,
    account: String,
    jurisdiction: String,
    workflow: String,
    consent_version: String,
    consent: Option<String>,
    website: String,
}
#[cfg(test)]
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
#[cfg(test)]
fn random() -> String {
    hex(&secp256k1::rand::random::<[u8; 32]>())
}
fn opaque(s: &str) -> bool {
    s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn visitor(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| {
            let (name, value) = part.trim().split_once('=')?;
            (name == COOKIE && opaque(value)).then(|| value.to_owned())
        })
}
fn private(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("strict-origin"),
    );
    response
}
fn failure(status: StatusCode, text: &str) -> Response {
    private(problem(
        status,
        "Pilot request unavailable",
        text,
        ("/", "Home"),
    ))
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/pilot", get(retired).post(submit))
        .route("/pilot/install", get(retired))
        .layer(DefaultBodyLimit::max(8192))
}

async fn retired() -> Response {
    crate::layout::problem(
        StatusCode::NOT_FOUND,
        "Not found",
        "Nothing on this site has that address.",
        ("/", "Home"),
    )
}

#[cfg(test)]
use archived::{ARCHIVED_INSTALL, ARCHIVED_OFFER};

async fn submit(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<Request>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Some(intake) = &app.config.pilot else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "We aren't taking requests right now. Nothing was sent.",
        );
    };
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(intake.origin.as_str())
        || headers.get(header::HOST).and_then(|v| v.to_str().ok())
            != intake.origin.split_once("://").map(|(_, h)| h)
    {
        return failure(StatusCode::FORBIDDEN, "Send this form from our offer page.");
    }
    let Some(cookie) = visitor(&headers) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "Turn on cookies, open the form again, and send it.",
        );
    };
    if !intake.allowed(&cookie, true) {
        return failure(
            StatusCode::TOO_MANY_REQUESTS,
            "That's a lot of tries. Wait a minute, then send it again.",
        );
    }
    let Ok(Form(form)) = form else {
        return failure(
            StatusCode::BAD_REQUEST,
            "Something in the form is missing or too long. Check it and send it again.",
        );
    };
    let Ok(ticket) = intake.verify(&cookie, &form.ticket) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "This form is too old. Open it again and send it.",
        );
    };
    if !form.website.is_empty()
        || form.consent.as_deref() != Some("yes")
        || intake::email(&form.email).is_err()
        || [&form.account, &form.jurisdiction]
            .iter()
            .any(|s| s.trim().is_empty() || s.len() > 128 || s.chars().any(char::is_control))
        || form.workflow.trim().is_empty()
        || form.workflow.len() > 2048
        || form.workflow.chars().any(|c| c.is_control() && c != '\n')
    {
        return failure(
            StatusCode::BAD_REQUEST,
            "Add a valid email, fill in each field, and tick the box that says we may write back. Nothing was sent.",
        );
    }
    let origin = headers
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let request_host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let submission = intake::Submission {
        request: ticket.request,
        issued_at: ticket.issued_at,
        email: form.email,
        account: form.account,
        jurisdiction: form.jurisdiction,
        workflow: form.workflow,
        referral: ticket.referral,
        consent_version: form.consent_version,
        consent: true,
    };
    let Ok(permit) = intake.slots.clone().try_acquire_owned() else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "We're busy. Send the same form again in a moment.",
        );
    };
    let cloned = intake.clone();
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let policy = cloned.policy()?;
        if origin.as_deref() != Some(policy.origin.as_str())
            || request_host.as_deref() != policy.origin.split_once("://").map(|(_, h)| h)
        {
            return Err("intake origin refused".into());
        }
        cloned.submit(submission)
    })
    .await;
    let Ok(Ok(ack)) = result else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "We couldn't confirm we got your request. Send the same form again.",
        );
    };
    private(
        UiPage::new("Pilot request received")
            .path("/")
            .scriptless()
            .content(prose(PreEscaped(format!(
            "<h1>Pilot request received</h1><p>Thanks, we have your request. Its reference is <code>{}</code>; keep it in case you write to us about it.</p><p>A person reads every request and writes back by email if you said we may. Nothing is booked or billed yet.</p><p><a href=\"/\">Home</a></p>",
            escape(&ack.reference)
        ))))
            .respond(&headers),
    )
}

mod archived;
#[cfg(test)]
mod tests;

//! The frozen Coder service offer and explicitly configured private intake.
//! Contact content is never echoed, logged, or sent to a model or public event.

use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{DefaultBodyLimit, Form, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use coder::task::sales::{Store, intake};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::App;
use crate::layout::{escape, page, problem};

const COOKIE: &str = "oa_pilot";
const SOURCE: &str = "https://github.com/OpenAgentsInc/openagents/blob/main/";

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

    fn sign(&self, cookie: &str, payload: &[u8]) -> String {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(self.secret.as_bytes()).expect("HMAC accepts this key");
        mac.update(cookie.as_bytes());
        mac.update(b":pilot:");
        mac.update(payload);
        hex(&mac.finalize().into_bytes())
    }

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
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Referral {
    reference: Option<String>,
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
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
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
        ("/pilot", "Review the pilot offer"),
    ))
}
fn host_matches(headers: &HeaderMap, policy: &intake::Policy) -> bool {
    let host = policy.origin.split_once("://").unwrap().1;
    headers.get(header::HOST).and_then(|v| v.to_str().ok()) == Some(host)
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/pilot", get(offer).post(submit))
        .route("/pilot/install", get(install))
        .layer(DefaultBodyLimit::max(8192))
}

const OFFER_BODY: &str = "<h1>One checked repository change</h1><p class=\"lede\">A bounded Coder pilot for a workflow owner who wants help turning one public-repository task into a checked patch and a repeatable setup.</p><h2>What the pilot covers</h2><p>You name the workflow owner and the person who accepts the result. For example, choose one small public-repository fix with checks that fail before the change and pass after it. The selected client is source-built Coder and its companion OpenAgents CLI on your macOS arm64 computer, with your own supported provider login. The exact clean installed revision needs private qualification before work starts.</p><p>You supply one public HTTPS GitHub repository, its full 40-character commit, a clean isolated worktree without submodules, a task of at most 16 KiB, and one to eight declared checks of at most 1,024 bytes each. You receive a patch, candidate digest, declared check results with bounded output, a run summary, a private trace reference, and a setup and repeat-workflow runbook. You apply or publish the patch.</p><p>A person other than the executor checks the candidate; you accept the checked patch and runbook. An agent reply, exit, or unchecked patch does not count as delivery.</p><h2>Proposed price and limits</h2><p>The proposed service fee is USD 250, invoiced after you accept the checked patch and runbook, due in seven calendar days. Your provider charges remain yours. The private scoped agreement confirms the price before any work; submitting a request creates no invoice, payment, product credits, or customer agreement. An unaccepted result earns no service fee.</p><p>One buyer, one repository, one change, at most one repair attempt, and seven calendar days with a dated review. Discovery, setup, delivery, and support share a three-hour operator cap. Each attempt stops at 30 minutes; each check stops at 15 minutes. These engagement limits are operated by the delivery person. Free discovery is one 30-minute conversation, with zero promotional credits and no provider subsidy.</p><h2>Support, data, and cancellation</h2><p>The private agreement names your delivery person, private support contact, and business hours. They acknowledge requests within one business day during those hours; support ends at the pilot review and stays inside the three-hour cap. There is no availability SLA or continuing maintenance promise.</p><p>Use public source without secrets or unrelated personal data. Approve the named providers and people before disclosure; keep your login on your computer and disable sponsored cloud fallback. Traces stay local unless you separately approve a redacted export. Operator-copied source and trace exports are deleted within 30 days after review under the agreement. Training, public examples, and marketing need separate permission.</p><p>You can stop before acceptance without a service invoice; provider charges remain yours. Unknown writes need inspection before retry. Later refunds or extensions need a new private agreement.</p><p>Paid plugins, hosted execution, subscriptions, and product-money conversion are unavailable through this offer. This page makes no savings, margin, or customer-result claim.</p>";

async fn offer(
    State(app): State<App>,
    headers: HeaderMap,
    query: Result<Query<Referral>, axum::extract::rejection::QueryRejection>,
) -> Response {
    let Ok(Query(query)) = query else {
        return failure(
            StatusCode::BAD_REQUEST,
            "The referral parameter is invalid. Use the offer address without a query.",
        );
    };
    if query.reference.as_ref().is_some_and(|r| {
        r.is_empty()
            || r.len() > 64
            || !r
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    }) {
        return failure(
            StatusCode::BAD_REQUEST,
            "Use an opaque referral identifier without contact details.",
        );
    }
    let mut body = OFFER_BODY.to_owned();
    body.push_str(&format!("<p><a href=\"/pilot/install\">Review the selected installation path</a> · <a href=\"{SOURCE}docs/sales/README.md#first-workflow-offer-v1\">Frozen offer v1</a></p>"));
    let Some(intake) = &app.config.pilot else {
        body.push_str("<h2>Private pilot requests</h2><p>Commercial approval and a responsible private intake path are not recorded by this site. Pilot requests are unavailable here. These proposed terms are for review.</p>");
        return private(page("Coder pilot", None, &body));
    };
    let cookie = visitor(&headers).unwrap_or_else(random);
    if !intake.allowed(&cookie, false) {
        return failure(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many requests. Wait one minute before opening this form again.",
        );
    }
    let Ok(permit) = intake.slots.clone().try_acquire_owned() else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Private intake is busy. Try opening the form again later.",
        );
    };
    let cloned = intake.clone();
    let policy = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        cloned.policy()
    })
    .await;
    let Ok(Ok(policy)) = policy else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Private intake is unavailable. No request has been recorded by this page.",
        );
    };
    if !host_matches(&headers, &policy) {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Intake is unavailable on this site address.",
        );
    }
    let ticket = intake.ticket(&cookie, query.reference);
    body.push_str(&format!("<h2>Request a private pilot review</h2><p>{owner} accepts responsibility for reviewing requests. Contact: <a href=\"mailto:{email}\">{email}</a>. Request content stays in the private host pipeline; no model or public event receives it.</p><form class=\"pilot-form\" method=\"post\" action=\"/pilot\"><input type=\"hidden\" name=\"ticket\" value=\"{ticket}\"><input type=\"hidden\" name=\"consent_version\" value=\"{version}\"><label>Your email <input type=\"email\" name=\"email\" maxlength=\"254\" autocomplete=\"email\" required></label><label>Team or account <input name=\"account\" maxlength=\"128\" required></label><label>Country or jurisdiction <input name=\"jurisdiction\" maxlength=\"128\" required></label><label>One workflow <textarea name=\"workflow\" maxlength=\"2048\" rows=\"4\" required></textarea></label><p>Describe the task briefly. Do not paste credentials, private source, or unrelated personal data.</p><div class=\"pilot-trap\" aria-hidden=\"true\"><label>Leave empty <input name=\"website\" tabindex=\"-1\" autocomplete=\"off\"></label></div><label class=\"pilot-consent\"><input type=\"checkbox\" name=\"consent\" value=\"yes\" required> I can request this review and consent to {owner} retaining these details for {retention} and following up by email about this request only. I can withdraw or request deletion through the contact above. This permits no marketing, model disclosure, training, or public example, and creates no purchase or delivery agreement.</label><p>Consent version: <code>{version}</code>. An optional referral identifier is unverified attribution; it promises no commission.</p><button type=\"submit\">Request private review</button></form>", owner=escape(&policy.public_owner), email=escape(&policy.support_email), version=escape(&policy.consent_version), ticket=escape(&ticket), retention=if policy.retention_seconds == 86400 { "1 day".into() } else { format!("{} days", policy.retention_seconds / 86400) }));
    let mut response = private(page("Coder pilot", None, &body));
    let secure = if app.config.secure_cookies {
        "; Secure"
    } else {
        ""
    };
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&format!(
            "{COOKIE}={cookie}; Path=/pilot; Max-Age=1800; HttpOnly; SameSite=Strict{secure}"
        ))
        .expect("opaque cookie is a valid header"),
    );
    response
}

async fn submit(
    State(app): State<App>,
    headers: HeaderMap,
    form: Result<Form<Request>, axum::extract::rejection::FormRejection>,
) -> Response {
    let Some(intake) = &app.config.pilot else {
        return failure(
            StatusCode::SERVICE_UNAVAILABLE,
            "Private intake is unavailable. No request was recorded.",
        );
    };
    if headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) != Some(intake.origin.as_str())
        || headers.get(header::HOST).and_then(|v| v.to_str().ok())
            != intake.origin.split_once("://").map(|(_, h)| h)
    {
        return failure(
            StatusCode::FORBIDDEN,
            "Submit only from the configured offer page.",
        );
    }
    let Some(cookie) = visitor(&headers) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "Open the offer form with cookies enabled before submitting.",
        );
    };
    if !intake.allowed(&cookie, true) {
        return failure(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many submissions. Wait one minute, then retry the same form.",
        );
    }
    let Ok(Form(form)) = form else {
        return failure(
            StatusCode::BAD_REQUEST,
            "The form is invalid or too large. No request was recorded. Review its bounded fields before retrying.",
        );
    };
    let Ok(ticket) = intake.verify(&cookie, &form.ticket) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "The form has expired or changed. Open a new form; repeated contact for this offer creates no competing lead.",
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
            "The form needs a valid email, bounded details, and explicit consent. No request was recorded.",
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
            "Private intake is busy. Retry the same form to recover its acknowledgment.",
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
            "No confirmation is available. Retry the same unchanged form to recover its acknowledgment. If this persists, use the named support contact on the offer. Do not assume delivery or payment from this request.",
        );
    };
    private(page(
        "Pilot request received",
        None,
        &format!(
            "<h1>Pilot request received</h1><p>Your private request reference is <code class=\"pilot-reference\">{}</code>. Keep this acknowledgment. A repeated request about this offer preserves the first private lead and its source.</p><p>This receipt confirms private intake only. Follow-up needs current recorded email permission and human review. This acknowledgment creates no qualification, invoice, purchase, or delivery commitment.</p><p><a href=\"/pilot\">Review the offer</a></p>",
            escape(&ack.reference)
        ),
    ))
}

async fn install() -> Response {
    private(page(
        "Coder pilot installation",
        None,
        &format!(
            "<h1>Selected pilot installation</h1><p>The selected pilot client is source-built Coder and its companion OpenAgents CLI on macOS arm64, from a recorded clean repository commit using <a href=\"{SOURCE}scripts/install-coder.sh\">scripts/install-coder.sh</a>. The delivery person records the full revision and qualifies this exact installed path before the pilot. After receiving that revision, check it out in a clean clone and run <code>./scripts/install-coder.sh</code>; the installer exit alone does not establish qualification.</p><p>Use your own supported provider login and keep its credentials local. For the selected initial path, set <code>CODER_CLOUD=off</code> and <code>OPENAGENTS_JEV_HOSTED=off</code>. These switches do not disable decision access enabled by existing <code>TYPESAFE_*</code> or <code>CODER_DECISION_*</code> settings, or a TypeSafe key in <code>~/.openagents/jev.json</code>. Disable that decision access in the selected pilot environment without deleting your saved configuration, or separately admit each exact decision recipient and payer before work.</p><p>The private setup covers your declared repository and checks. Review <a href=\"{SOURCE}docs/coder/guides/headless.md\">terminal and headless usage</a> and <a href=\"{SOURCE}docs/sales/README.md#first-workflow-offer-v1\">the frozen offer</a>. The <a href=\"/download\">general downloads</a> are available separately; a release download alone does not qualify this pilot path.</p><p>Qualification and a private scoped agreement precede work. The pilot request cannot install software, run a task, or grant execution authority.</p><p><a href=\"/pilot\">Review or request the pilot</a></p>"
        ),
    ))
}

#[cfg(test)]
mod tests;

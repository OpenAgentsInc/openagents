//! Session notifications through the push gateway (#11853).
//!
//! This server holds a signing key that the push gateway accepts as a
//! delivery caller, the role a relay's NIP-PL executor has. A phone registers
//! its native token with the gateway, asks the gateway for a delivery grant
//! bound to this server's key, and stores only that grant here. When a
//! registry write finishes a run, asks for input, or fails, the server posts
//! the closed delivery request `{v, endpoint_grant, request_id, expires_at}`
//! for each phone that wants that kind. The gateway sends its fixed wake
//! constant, so no session text, title, or chat ID leaves this server.
//!
//! A phone that is open (its registry presence is current) gets nothing; the
//! app already shows the change. A write marked `X-OpenAgents-Test: 1`
//! notifies nobody.
use super::notifications::{Category, PushPrefs, chat_title, notification_for};
use openagents_doc::RegistryRow;
use push_gateway::wire::{self, DeliveryRequest, Transport};
use secp256k1::SecretKey;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap, HashSet, VecDeque},
    sync::Mutex,
    time::Duration,
};

/// The header a test run sends; such writes never notify.
pub const TEST_HEADER: &str = "x-openagents-test";
/// How long a wake stays useful.
pub const WAKE_SECONDS: u64 = 15 * 60;
/// How long a sent notification blocks an identical one.
const RECENT_SECONDS: u64 = 60 * 60;
const RECENT_MAX: usize = 4096;
const LOG_MAX: usize = 20;
const LOG_ACCOUNTS_MAX: usize = 4096;

/// Whether a request is a test run.
pub fn is_test_run(headers: &axum::http::HeaderMap) -> bool {
    headers
        .get(TEST_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.trim() == "1")
}

/// Push settings read from the environment.
#[derive(Clone)]
pub struct Settings {
    /// Base URL of the gateway's private delivery listener.
    pub delivery: String,
    /// Base URL of the gateway's public registration listener, for phones.
    pub registration: String,
    /// The key the gateway accepts deliveries from.
    pub secret: SecretKey,
    /// The gateway's APNs app profile.
    pub apns_profile: Option<String>,
    /// The gateway's FCM app profile.
    pub fcm_profile: Option<String>,
}

impl std::fmt::Debug for Settings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Settings")
            .field("delivery", &self.delivery)
            .field("registration", &self.registration)
            .field("apns_profile", &self.apns_profile)
            .field("fcm_profile", &self.fcm_profile)
            .finish_non_exhaustive()
    }
}

fn http_url(value: &str, private: bool) -> bool {
    let base = value.trim_end_matches('/');
    let loopback = ["http://127.0.0.1", "http://localhost", "http://[::1]"]
        .iter()
        .any(|p| base.starts_with(p));
    (base.starts_with("https://") || (private && base.starts_with("http://")) || loopback)
        && !base.contains(['?', '#', '@'])
}

impl Settings {
    /// Read `CODER_SYNC_PUSH_*`. All unset is `Ok(None)`: push is off.
    ///
    /// - `CODER_SYNC_PUSH_GATEWAY`: the delivery listener (a private
    ///   `http://` address is allowed).
    /// - `CODER_SYNC_PUSH_REGISTRATION`: the registration listener phones use
    ///   (`https://`, or loopback `http://` for local tests).
    /// - `CODER_SYNC_PUSH_KEY_FILE`: 64 hexadecimal characters, mode `0600`.
    /// - `CODER_SYNC_PUSH_APNS_PROFILE` and `CODER_SYNC_PUSH_FCM_PROFILE`: at
    ///   least one.
    pub fn parse(get: &dyn Fn(&str) -> Option<String>) -> Result<Option<Self>, &'static str> {
        let delivery = get("CODER_SYNC_PUSH_GATEWAY").filter(|v| !v.is_empty());
        let registration = get("CODER_SYNC_PUSH_REGISTRATION").filter(|v| !v.is_empty());
        let key_file = get("CODER_SYNC_PUSH_KEY_FILE").filter(|v| !v.is_empty());
        let apns_profile = get("CODER_SYNC_PUSH_APNS_PROFILE").filter(|v| !v.is_empty());
        let fcm_profile = get("CODER_SYNC_PUSH_FCM_PROFILE").filter(|v| !v.is_empty());
        if delivery.is_none()
            && registration.is_none()
            && key_file.is_none()
            && apns_profile.is_none()
            && fcm_profile.is_none()
        {
            return Ok(None);
        }
        let (Some(delivery), Some(registration), Some(key_file)) =
            (delivery, registration, key_file)
        else {
            return Err(
                "Set CODER_SYNC_PUSH_GATEWAY, CODER_SYNC_PUSH_REGISTRATION, and CODER_SYNC_PUSH_KEY_FILE together.",
            );
        };
        if apns_profile.is_none() && fcm_profile.is_none() {
            return Err("Set CODER_SYNC_PUSH_APNS_PROFILE or CODER_SYNC_PUSH_FCM_PROFILE.");
        }
        if !http_url(&delivery, true) || !http_url(&registration, false) {
            return Err("Invalid push gateway address.");
        }
        let secret = read_key(std::path::Path::new(&key_file))?;
        Ok(Some(Self {
            delivery: delivery.trim_end_matches('/').to_owned(),
            registration: registration.trim_end_matches('/').to_owned(),
            secret,
            apns_profile,
            fcm_profile,
        }))
    }

    /// This server's delivery key, as the phone delegates to it.
    pub fn pubkey(&self) -> String {
        let secp = secp256k1::Secp256k1::new();
        secp256k1::Keypair::from_secret_key(&secp, &self.secret)
            .x_only_public_key()
            .0
            .to_string()
    }

    /// The answer to `GET /registry/{org}/push-config`.
    pub fn public(&self) -> Value {
        json!({
            "configured": true,
            "registration": self.registration,
            "delegate": self.pubkey(),
            "profiles": {"apns": self.apns_profile, "fcm": self.fcm_profile},
        })
    }
}

fn read_key(path: &std::path::Path) -> Result<SecretKey, &'static str> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(path)
            .map_err(|_| "Could not read the push key file.")?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err("The push key file must not be readable by other users.");
        }
    }
    let text = std::fs::read_to_string(path).map_err(|_| "Could not read the push key file.")?;
    let text = text.trim();
    if text.len() != 64 || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("The push key file must hold 64 hexadecimal characters.");
    }
    let mut bytes = [0_u8; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&text[index * 2..index * 2 + 2], 16)
            .map_err(|_| "The push key file must hold 64 hexadecimal characters.")?;
    }
    SecretKey::from_byte_array(bytes).map_err(|_| "The push key is not a valid key.")
}

/// One notification a registry write calls for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    pub chat_id: String,
    pub category: Category,
    /// What makes this notification distinct: the completed turn, or the
    /// row's update time for a question or a failure.
    pub marker: String,
}

/// The notifications a batch calls for. `original` holds each `sessions`
/// row as it was before the batch (or `None` when it was new); `rows` holds
/// the rows after it. `chat` finds a chat row the batch didn't touch.
pub fn notes_for(
    original: &BTreeMap<String, Option<RegistryRow>>,
    rows: &BTreeMap<(String, String), RegistryRow>,
    chat: &dyn Fn(&str) -> Option<RegistryRow>,
    now_ms: i64,
) -> Vec<Note> {
    let mut notes = Vec::new();
    for (id, before) in original {
        let Some(after) = rows.get(&("sessions".to_owned(), id.clone())) else {
            continue;
        };
        let Some(category) = notification_for(before.as_ref(), after, now_ms) else {
            continue;
        };
        let chat_row = rows
            .get(&("chats".to_owned(), id.clone()))
            .cloned()
            .or_else(|| chat(id));
        if chat_title(chat_row.as_ref()).is_none() {
            continue;
        }
        let marker = match category {
            Category::Done => after
                .fields
                .get("lastCompletedTurn")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            _ => after
                .fields
                .get("updatedAt")
                .map(Value::to_string)
                .unwrap_or_default(),
        };
        notes.push(Note {
            chat_id: id.clone(),
            category,
            marker,
        });
    }
    notes
}

/// A phone's stored delivery grant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub device: String,
    pub transport: Transport,
    pub grant: String,
    /// Unix seconds when the grant ends.
    pub expires_at: u64,
    pub prefs: PushPrefs,
}

fn wants(prefs: &PushPrefs, category: Category) -> bool {
    match category {
        Category::Done => prefs.done,
        Category::Input => prefs.input,
        Category::Failed => prefs.failed,
    }
}

/// A wake to send.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wake {
    pub device: String,
    pub transport: Transport,
    pub grant: String,
    pub request_id: String,
    pub chat_id: String,
    pub category: Category,
}

/// A line of the recent delivery log on `/stats`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub at: u64,
    pub chat_id: String,
    pub category: Category,
    pub device: String,
    pub result: String,
}

impl Entry {
    fn json(&self) -> Value {
        json!({"at":self.at,"chatId":self.chat_id,"category":self.category,"device":self.device,"result":self.result})
    }
}

/// A stable request ID for one notification to one phone. The gateway
/// replays a finished request ID without sending again, so a retried or
/// repeated decision never wakes a phone twice.
pub fn request_id(account: &str, device: &str, note: &Note) -> String {
    let mut hash = Sha256::new();
    for part in [
        account,
        device,
        &note.chat_id,
        category_word(note.category),
        &note.marker,
    ] {
        hash.update(part.as_bytes());
        hash.update([0]);
    }
    let mut bytes: [u8; 16] = hash.finalize()[..16].try_into().expect("16 bytes");
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex = push_gateway::hex(&bytes);
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

fn category_word(category: Category) -> &'static str {
    match category {
        Category::Done => "done",
        Category::Input => "input",
        Category::Failed => "failed",
    }
}

/// Request IDs sent recently, so the same notification isn't sent twice.
#[derive(Debug, Default)]
pub struct Recent(HashMap<String, u64>);

impl Recent {
    /// Record `id`; `false` when it was already sent within the window.
    pub fn first(&mut self, id: &str, now: u64) -> bool {
        self.0
            .retain(|_, at| now.saturating_sub(*at) < RECENT_SECONDS);
        if self.0.contains_key(id) {
            return false;
        }
        if self.0.len() >= RECENT_MAX {
            if let Some(oldest) = self
                .0
                .iter()
                .min_by_key(|(_, at)| **at)
                .map(|(k, _)| k.clone())
            {
                self.0.remove(&oldest);
            }
        }
        self.0.insert(id.to_owned(), now);
        true
    }
}

/// Decide the wakes for `notes`. Pure apart from `recent`.
///
/// - A phone that turned the kind off gets nothing.
/// - A phone whose app is open (`present`) gets nothing.
/// - A lapsed grant gets nothing.
/// - A notification already sent to that phone isn't sent again.
pub fn plan(
    account: &str,
    notes: &[Note],
    targets: &[Target],
    present: &HashSet<String>,
    now: u64,
    recent: &mut Recent,
) -> (Vec<Wake>, Vec<Entry>) {
    let mut wakes = Vec::new();
    let mut log = Vec::new();
    for note in notes {
        if targets.is_empty() {
            log.push(Entry {
                at: now,
                chat_id: note.chat_id.clone(),
                category: note.category,
                device: String::new(),
                result: "no targets".into(),
            });
        }
        for target in targets {
            if !wants(&target.prefs, note.category) {
                continue;
            }
            let entry = |result: &str| Entry {
                at: now,
                chat_id: note.chat_id.clone(),
                category: note.category,
                device: target.device.clone(),
                result: result.into(),
            };
            if present.contains(&target.device) {
                log.push(entry("app open"));
                continue;
            }
            if target.expires_at <= now {
                log.push(entry("grant expired"));
                continue;
            }
            let id = request_id(account, &target.device, note);
            if !recent.first(&id, now) {
                log.push(entry("duplicate"));
                continue;
            }
            log.push(entry("pending"));
            wakes.push(Wake {
                device: target.device.clone(),
                transport: target.transport,
                grant: target.grant.clone(),
                request_id: id,
                chat_id: note.chat_id.clone(),
                category: note.category,
            });
        }
    }
    (wakes, log)
}

/// What the gateway said about one wake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Accepted,
    /// The token or grant no longer works; forget the phone's grant.
    Gone(String),
    Failed(String),
}

impl Outcome {
    pub fn word(&self) -> String {
        match self {
            Self::Accepted => "accepted".into(),
            Self::Gone(why) | Self::Failed(why) => why.clone(),
        }
    }
}

/// The push sender: settings, the recent-send window, and the log.
pub struct Push {
    pub settings: Settings,
    http: reqwest::Client,
    recent: Mutex<Recent>,
    log: Mutex<HashMap<String, VecDeque<Entry>>>,
}

impl std::fmt::Debug for Push {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Push")
            .field("settings", &self.settings)
            .finish_non_exhaustive()
    }
}

impl Push {
    pub fn new(settings: Settings) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .unwrap_or_default();
        Self {
            settings,
            http,
            recent: Mutex::default(),
            log: Mutex::default(),
        }
    }

    /// Plan the wakes for one account and record the decisions.
    pub fn plan(
        &self,
        account: &str,
        notes: &[Note],
        targets: &[Target],
        present: &HashSet<String>,
        now: u64,
    ) -> Vec<Wake> {
        let (wakes, log) = {
            let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
            plan(account, notes, targets, present, now, &mut recent)
        };
        self.record(account, log);
        wakes
    }

    fn record(&self, account: &str, entries: Vec<Entry>) {
        if entries.is_empty() {
            return;
        }
        let mut log = self.log.lock().unwrap_or_else(|e| e.into_inner());
        if log.len() >= LOG_ACCOUNTS_MAX && !log.contains_key(account) {
            log.clear();
        }
        let lines = log.entry(account.to_owned()).or_default();
        for entry in entries {
            if let Some(line) = lines.iter_mut().rev().find(|line| {
                line.result == "pending"
                    && line.chat_id == entry.chat_id
                    && line.device == entry.device
                    && line.category == entry.category
                    && entry.result != "pending"
            }) {
                line.result = entry.result;
                continue;
            }
            lines.push_back(entry);
            while lines.len() > LOG_MAX {
                lines.pop_front();
            }
        }
    }

    /// The recent delivery log for `/stats`.
    pub fn log(&self, account: &str) -> Vec<Value> {
        self.log
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(account)
            .map(|lines| lines.iter().map(Entry::json).collect())
            .unwrap_or_default()
    }

    /// Send one wake. A `503` or a connection failure is tried once more;
    /// the stable request ID makes that safe.
    pub async fn send(&self, account: &str, wake: &Wake) -> Outcome {
        self.send_kind(account, wake, None).await
    }

    /// Send the registered attention wake through this same gateway and grant.
    pub async fn send_attention(&self, account: &str, wake: &Wake, expires_at: u64) -> Outcome {
        self.send_kind(account, wake, Some(expires_at)).await
    }

    async fn send_kind(
        &self,
        account: &str,
        wake: &Wake,
        attention_expiry: Option<u64>,
    ) -> Outcome {
        let mut outcome = Outcome::Failed("not sent".into());
        for attempt in 0..2 {
            if attempt > 0 {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            let (retry, result) = self.attempt(wake, attention_expiry).await;
            outcome = result;
            if !retry {
                break;
            }
        }
        self.record(
            account,
            vec![Entry {
                at: push_gateway::unix_now(),
                chat_id: wake.chat_id.clone(),
                category: wake.category,
                device: wake.device.clone(),
                result: outcome.word(),
            }],
        );
        outcome
    }

    async fn attempt(&self, wake: &Wake, attention_expiry: Option<u64>) -> (bool, Outcome) {
        let now = push_gateway::unix_now();
        let path = match wake.transport {
            Transport::Apns if attention_expiry.is_some() => wire::APNS_ATTENTION_PATH,
            Transport::Fcm if attention_expiry.is_some() => wire::FCM_ATTENTION_PATH,
            Transport::Apns => wire::APNS_DELIVERY_PATH,
            Transport::Fcm => wire::FCM_DELIVERY_PATH,
        };
        let url = format!("{}{path}", self.settings.delivery);
        let body = serde_json::to_vec(&DeliveryRequest {
            v: wire::WIRE_VERSION,
            endpoint_grant: wake.grant.clone(),
            request_id: wake.request_id.clone(),
            expires_at: attention_expiry.unwrap_or(now + WAKE_SECONDS),
        })
        .expect("delivery request serializes");
        let Ok(authorization) = push_gateway::nip98::sign(&self.settings.secret, &url, &body, now)
        else {
            return (false, Outcome::Failed("signing failed".into()));
        };
        let response = match self
            .http
            .post(&url)
            .header("content-type", "application/json")
            .header("authorization", authorization)
            .body(body)
            .send()
            .await
        {
            Ok(response) => response,
            Err(_) => return (true, Outcome::Failed("gateway unreachable".into())),
        };
        let status = response.status().as_u16();
        let value: Value = response.json().await.unwrap_or_default();
        let word = value["status"]
            .as_str()
            .or_else(|| value["error"].as_str())
            .unwrap_or("unexpected")
            .to_owned();
        match status {
            200 => (false, Outcome::Accepted),
            404 | 410 => (false, Outcome::Gone(format!("{status} {word}"))),
            503 => (true, Outcome::Failed(format!("{status} {word}"))),
            _ => (false, Outcome::Failed(format!("{status} {word}"))),
        }
    }
}

/// Parse a `POST /registry/{org}/push-grant` body:
/// `{"transport":"apns"|"fcm","grant":"…","expiresAt":<unix seconds>,"prefs":{…}}`.
pub fn parse_grant(body: &Value, now: u64) -> Option<(Transport, String, u64, PushPrefs)> {
    let transport = Transport::parse(body["transport"].as_str()?)?;
    let grant = body["grant"]
        .as_str()
        .filter(|g| {
            !g.is_empty()
                && g.len() <= wire::MAX_GRANT_BYTES
                && g.bytes().all(|b| b.is_ascii_graphic())
        })?
        .to_owned();
    let expires_at = body["expiresAt"].as_u64().filter(|at| *at > now)?;
    Some((
        transport,
        grant,
        expires_at,
        super::notifications::parse_prefs(&body["prefs"]),
    ))
}

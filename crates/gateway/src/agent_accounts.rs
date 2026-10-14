//! Agents that register themselves (#11630, docs/agents/self-registration.md).
//!
//! An agent with no person and no email gets an agent account and an API
//! key in two calls:
//!
//! - `GET /v1/agents/challenge`: a proof-of-work challenge, signed by
//!   this gateway, good for ten minutes.
//! - `POST /v1/agents/register` `{name, challenge, solution}`: where
//!   `SHA-256(challenge ":" solution)` starts with the challenge's number
//!   of zero bits. Each address, each network (an IPv4 /24 or an IPv6
//!   /48), and everyone together may register a set number a day. The
//!   answer holds the `oak_` key (shown once) and a claim code a person can
//!   use later to link the agent to their account.
//!
//! The account is marked as an agent ([`tenancy::accounts::agents`]),
//! never a person, and its key is scoped to calling models, listing them,
//! and reading its own usage. It can't turn into a website session, so
//! the website's chats, computers, Solver, and Drive stay out of reach.
//!
//! What an agent may use without paying is small and hard
//! ([`crate::config::AgentFree`]): a few requests a day on a small model,
//! each with a short input and output, under a daily dollar cap per agent
//! and a daily cap on every free agent together; a few decision calls; a
//! few hundred other API calls. Over the free tier a model request answers
//! `402` at once, before anything is sent to a model, with a body that
//! says what ran out and how to pay. Paying is Lightning only: the pay
//! host sells a top-up as a hosted resource and, once it is paid, calls
//! `POST /v1/agents/credit` with a header it signs; the credit lands on the
//! agent's balance, and paid requests hold their worst case from it.
//!
//! Person and admin routes, on a `sess_` session:
//!
//! - `POST /v1/agents/claim` `{claim}`: link an agent to the signed-in
//!   person.
//! - `GET /v1/agents/claimed`: the person's agents.
//! - `POST /v1/agents/{account}/revoke`: the claiming person or a site
//!   admin revokes an agent; its key stops at once.
//! - `GET /v1/admin/agents`: every agent account, for site admins.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::Write;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};

use axum::Json;
use axum::body::Bytes;
use axum::extract::{Path as UrlPath, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get, post};
use hmac::{Hmac, Mac};
use inference::error::{ApiError, ErrorType};
use inference::request::CreateResponse;
use inference::router::{Needs, micros_usd};
use inference::run::{Caller, Prepared};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tenancy::accounts::agents::{self, CLAIM_PREFIX};
use tenancy::keys;

use crate::accounts::{
    Principal, accounts_refusal, accounts_store, answered, refused, session_principal, unix_now,
};
use crate::config::{AgentFree, AgentSignup};
use crate::inference_public::{AgentShort, today};
use crate::serve::ServeState;

pub const CHALLENGE: &str = "/v1/agents/challenge";
pub const REGISTER: &str = "/v1/agents/register";
pub const ME: &str = "/v1/agents/me";
pub const CLAIM: &str = "/v1/agents/claim";
pub const CLAIMED: &str = "/v1/agents/claimed";
pub const REVOKE: &str = "/v1/agents/{account}/revoke";
pub const CREDIT: &str = "/v1/agents/credit";
pub const ADMIN: &str = "/v1/admin/agents";

/// Where agents read how all of this works.
pub const DOCS: &str = "https://openagents.com/auth.md";
/// Where a person enters an agent's claim code.
pub const CLAIM_PAGE: &str = "https://openagents.com/settings/agent-accounts";

/// How long a challenge stands, seconds.
const CHALLENGE_TTL: u64 = 600;
/// How long a claim code stands, seconds (30 days).
const CLAIM_TTL: u64 = 30 * 86_400;
/// The longest proof-of-work solution taken, bytes.
const MAX_SOLUTION: usize = 64;
/// The day's registration count, beside the registry.
const SIGNUPS: &str = "agent-signups.json";
/// Every credit an agent bought, one JSON line each, beside the registry.
const CREDITS: &str = "agent-credit.jsonl";
/// The key challenges are signed with, made once, beside the registry.
const CHALLENGE_KEY: &str = "agent-challenge.key";
/// The scopes an agent's key gets.
const SCOPES: [&str; 3] = ["responses", "models:read", "usage:read"];
/// A balance at or above this lifts the read and decision caps.
const FUNDED_MICROS: u64 = 10_000;

type HmacSha256 = Hmac<Sha256>;

/// The day's registrations: the total persisted, the per-address and
/// per-network counts in memory.
#[derive(Default)]
struct Signups {
    day: u64,
    total: u32,
    per_ip: HashMap<IpAddr, u32>,
    per_net: HashMap<String, u32>,
}

#[derive(Default, Serialize, Deserialize)]
struct SignupsSaved {
    day: u64,
    total: u32,
}

/// The day's reads and decision calls, per agent and for every agent.
#[derive(Default)]
struct Calls {
    day: u64,
    per_agent: HashMap<String, (u32, u32)>,
    decisions_all: u32,
}

/// One credit an agent bought.
#[derive(Debug, Serialize, Deserialize)]
struct CreditLine {
    payment: String,
    account: String,
    micros: u64,
    at: u64,
}

/// The agent surface's state.
pub struct Agents {
    pub(crate) config: AgentSignup,
    tenant: String,
    mac_key: Vec<u8>,
    /// Challenge nonces already used, with their expiry.
    used: Mutex<HashMap<String, u64>>,
    signups: Mutex<Signups>,
    signups_path: PathBuf,
    /// Active agents' key ids to their accounts.
    index: RwLock<HashMap<String, String>>,
    calls: Mutex<Calls>,
    credits_path: PathBuf,
    /// Payment hashes already credited.
    paid: Mutex<HashSet<String>>,
}

impl Agents {
    /// The agent surface over the registry directory `dir`: the challenge
    /// key (made on first open), the day's registration count, the active
    /// agents' keys, and the credits bought. The credit journal is checked
    /// against `book`: a credit the journal holds that the book never
    /// added (a crash between the two writes) is added now.
    ///
    /// # Errors
    ///
    /// A file can't be read or made.
    pub(crate) fn open(
        dir: &Path,
        config: AgentSignup,
        tenant: String,
        book: Option<&mut crate::inference_public::Book>,
    ) -> Result<Self, String> {
        let key_path = dir.join(CHALLENGE_KEY);
        let mac_key = match std::fs::read(&key_path) {
            Ok(bytes) if bytes.len() == 32 => bytes,
            _ => {
                let mut bytes = [0_u8; 32];
                ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
                    .map_err(|_| "no randomness for the challenge key".to_string())?;
                write_private(&key_path, &bytes)?;
                bytes.to_vec()
            }
        };
        let signups_path = dir.join(SIGNUPS);
        let saved: SignupsSaved = std::fs::read(&signups_path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        let mut index = HashMap::new();
        if let Ok(store) = tenancy::Accounts::open(dir).and_then(|accounts| accounts.store()) {
            for account in store.agents() {
                if let Some(agent) = &account.agent
                    && agent.revoked.is_none()
                {
                    index.insert(agent.key.clone(), account.id.clone());
                }
            }
        }
        let credits_path = dir.join(CREDITS);
        let mut paid = HashSet::new();
        let mut sums: BTreeMap<String, u64> = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(&credits_path) {
            for line in text.lines() {
                if let Ok(credit) = serde_json::from_str::<CreditLine>(line) {
                    paid.insert(credit.payment);
                    *sums.entry(credit.account).or_default() += credit.micros;
                }
            }
        }
        if let Some(book) = book {
            for (account, sum) in sums {
                let credited = book.agent_credit(&account).credited;
                if sum > credited {
                    book.add_agent_credit(&account, sum - credited)?;
                }
            }
        }
        Ok(Self {
            config,
            tenant,
            mac_key,
            used: Mutex::new(HashMap::new()),
            signups: Mutex::new(Signups {
                day: saved.day,
                total: saved.total,
                ..Signups::default()
            }),
            signups_path,
            index: RwLock::new(index),
            calls: Mutex::new(Calls::default()),
            credits_path,
            paid: Mutex::new(paid),
        })
    }

    /// The agent account that holds the active key `key_id`.
    #[must_use]
    pub fn account_of_key(&self, key_id: &str) -> Option<String> {
        self.index.read().ok()?.get(key_id).cloned()
    }

    fn mac(&self, text: &str) -> String {
        let mut mac = HmacSha256::new_from_slice(&self.mac_key).expect("any key length");
        mac.update(text.as_bytes());
        hex::encode(&mac.finalize().into_bytes()[..16])
    }

    /// A fresh challenge: `v1.<bits>.<expires>.<nonce>.<mac>`.
    fn challenge(&self, now: u64) -> (String, u64) {
        let mut nonce = [0_u8; 16];
        let _ = ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut nonce);
        let expires = now + CHALLENGE_TTL;
        let body = format!(
            "v1.{}.{expires}.{}",
            self.config.difficulty_bits,
            hex::encode(nonce)
        );
        let mac = self.mac(&body);
        (format!("{body}.{mac}"), expires)
    }

    /// Checks a solved challenge and spends it.
    fn redeem(&self, challenge: &str, solution: &str, now: u64) -> Result<(), &'static str> {
        let parts: Vec<&str> = challenge.split('.').collect();
        let [version, bits, expires, nonce, mac] = parts.as_slice() else {
            return Err("That challenge isn't one we issued. Get a new one.");
        };
        let body = format!("{version}.{bits}.{expires}.{nonce}");
        if *version != "v1" || !constant_eq(self.mac(&body).as_bytes(), mac.as_bytes()) {
            return Err("That challenge isn't one we issued. Get a new one.");
        }
        let expires: u64 = expires.parse().map_err(|_| "That challenge isn't valid.")?;
        let bits: u32 = bits.parse().map_err(|_| "That challenge isn't valid.")?;
        if now > expires {
            return Err("That challenge has expired. Get a new one.");
        }
        if solution.is_empty()
            || solution.len() > MAX_SOLUTION
            || !solution.bytes().all(|b| b.is_ascii_graphic())
        {
            return Err("`solution` must be 1 to 64 printable characters.");
        }
        let digest = Sha256::digest(format!("{challenge}:{solution}").as_bytes());
        if leading_zero_bits(&digest) < bits {
            return Err("That solution doesn't solve the challenge.");
        }
        let mut used = self.used.lock().map_err(|_| "Try again in a moment.")?;
        used.retain(|_, until| *until >= now);
        if used.insert((*nonce).to_string(), expires).is_some() {
            return Err("That challenge was already used. Get a new one.");
        }
        Ok(())
    }

    /// Counts one registration from `ip`, or says which limit stops it.
    fn admit_signup(&self, ip: Option<IpAddr>, day: u64) -> Result<(), String> {
        let mut signups = self.signups.lock().map_err(|_| "Try again in a moment.")?;
        if signups.day != day {
            *signups = Signups {
                day,
                ..Signups::default()
            };
        }
        if signups.total >= self.config.per_day {
            return Err(
                "We've taken all the agent sign-ups we take today. Try again after 00:00 UTC."
                    .into(),
            );
        }
        if let Some(ip) = ip {
            if signups.per_ip.get(&ip).copied().unwrap_or(0) >= self.config.per_ip_per_day {
                return Err("This address has made all the agents it may make today. Try again after 00:00 UTC.".into());
            }
            let net = network(ip);
            if signups.per_net.get(&net).copied().unwrap_or(0) >= self.config.per_network_per_day {
                return Err("Your network has made all the agents it may make today. Try again after 00:00 UTC.".into());
            }
            *signups.per_ip.entry(ip).or_default() += 1;
            *signups.per_net.entry(net).or_default() += 1;
        }
        signups.total += 1;
        let saved = SignupsSaved {
            day,
            total: signups.total,
        };
        let _ = serde_json::to_vec(&saved)
            .map_err(|e| e.to_string())
            .and_then(|bytes| write_private(&self.signups_path, &bytes));
        Ok(())
    }

    /// Counts one call for an agent; `Err` names the cap it hit.
    fn count_call(&self, account: &str, decision: bool, day: u64) -> Result<(), &'static str> {
        let free = &self.config.free;
        let mut calls = self.calls.lock().map_err(|_| "busy")?;
        if calls.day != day {
            *calls = Calls {
                day,
                ..Calls::default()
            };
        }
        if decision && calls.decisions_all >= free.global_decisions_per_day {
            return Err("decisions_all");
        }
        let entry = calls.per_agent.entry(account.to_owned()).or_default();
        if decision {
            if entry.1 >= free.decisions_per_day {
                return Err("decisions");
            }
            entry.1 += 1;
            calls.decisions_all += 1;
        } else {
            if entry.0 >= free.reads_per_day {
                return Err("reads");
            }
            entry.0 += 1;
        }
        Ok(())
    }

    fn calls_today(&self, account: &str, day: u64) -> (u32, u32) {
        self.calls
            .lock()
            .ok()
            .filter(|calls| calls.day == day)
            .and_then(|calls| calls.per_agent.get(account).copied())
            .unwrap_or_default()
    }

    /// How to pay, for a `402` and the registration answer.
    fn pay(&self, account: &str) -> Value {
        match &self.config.credit {
            Some(credit) => json!({
                "available": true,
                "rail": "lightning",
                "methods": [{
                    "type": "lightning",
                    "protocol": "x402",
                    "method": "POST",
                    "url": format!("{}?account={account}", credit.buy_url),
                    "price_sats": credit.price_sats,
                    "credit_usd": micros_usd(credit.credit_usd_micros),
                    "how": "POST the URL with no payment to get a 402 with a Lightning invoice, pay it, and send the same request again with the proof the 402 asks for. The credit is added to this agent's balance.",
                }],
                "docs": DOCS,
            }),
            None => json!({
                "available": false,
                "rail": "lightning",
                "methods": [],
                "message": "Buying credit isn't open for agent accounts yet. The free tier starts over at 00:00 UTC.",
                "docs": DOCS,
            }),
        }
    }

    /// The free tier's terms and what `account` used of it on `day`.
    fn free_view(&self, used: crate::inference_public::AgentDay, calls: (u32, u32)) -> Value {
        let free = &self.config.free;
        json!({
            "models": free.models,
            "requests_per_day": free.requests_per_day,
            "requests_used": used.requests,
            "usd_per_day": micros_usd(free.usd_micros_per_day),
            "usd_used": micros_usd(used.micros.min(free.usd_micros_per_day)),
            "max_input_chars": free.max_input_chars,
            "max_output_tokens": free.max_output_tokens,
            "decisions_per_day": free.decisions_per_day,
            "decisions_used": calls.1,
            "reads_per_day": free.reads_per_day,
            "reads_used": calls.0,
            "resets_at": next_midnight(),
        })
    }

    fn credit_once(&self, payment: &str, account: &str, micros: u64) -> Result<bool, String> {
        let mut paid = self.paid.lock().map_err(|_| "busy".to_string())?;
        if paid.contains(payment) {
            return Ok(false);
        }
        let line = serde_json::to_string(&CreditLine {
            payment: payment.to_owned(),
            account: account.to_owned(),
            micros,
            at: unix_now(),
        })
        .map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.credits_path)
            .map_err(|e| e.to_string())?;
        file.write_all(format!("{line}\n").as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|e| e.to_string())?;
        paid.insert(payment.to_owned());
        Ok(true)
    }
}

fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let temporary = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .map_err(|e| format!("{}: {e}", temporary.display()))?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&temporary, path).map_err(|e| e.to_string())
}

fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0_u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Leading zero bits of a digest.
#[must_use]
pub fn leading_zero_bits(digest: &[u8]) -> u32 {
    let mut bits = 0;
    for byte in digest {
        if *byte == 0 {
            bits += 8;
        } else {
            bits += byte.leading_zeros();
            break;
        }
    }
    bits
}

/// An address's network: an IPv4 /24 or an IPv6 /48.
fn network(ip: IpAddr) -> String {
    match ip {
        IpAddr::V4(v4) => {
            let [a, b, c, _] = v4.octets();
            format!("{a}.{b}.{c}.0/24")
        }
        IpAddr::V6(v6) => {
            let s = v6.segments();
            format!("{:x}:{:x}:{:x}::/48", s[0], s[1], s[2])
        }
    }
}

/// The caller's address: the `X-Forwarded-For` entry `hops` places from
/// the right (each trusted proxy appends the address it saw).
fn caller_ip(headers: &HeaderMap, hops: usize) -> Option<IpAddr> {
    let value = headers.get("x-forwarded-for")?.to_str().ok()?;
    let entries: Vec<&str> = value.split(',').map(str::trim).collect();
    let index = entries.len().checked_sub(hops.max(1))?;
    entries.get(index)?.parse().ok()
}

/// The next 00:00 UTC, RFC 3339.
fn next_midnight() -> String {
    let day = today() + 1;
    let (year, month, date) = civil(day);
    format!("{year:04}-{month:02}-{date:02}T00:00:00Z")
}

/// Howard Hinnant's civil_from_days.
fn civil(day: u64) -> (i64, u64, u64) {
    let z = i64::try_from(day).unwrap_or(i64::MAX) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let date = u64::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u64::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, date)
}

fn agents(state: &ServeState) -> Result<&Agents, Response> {
    state.agents.as_ref().ok_or_else(|| {
        refused(
            StatusCode::NOT_FOUND,
            "agent_signup_off",
            "Agents can't register on this service.",
        )
    })
}

/// The agent account behind an active agent key, for inference admission.
pub(crate) fn agent_of_key(state: &ServeState, key_id: &str) -> Option<String> {
    state.agents.as_ref()?.account_of_key(key_id)
}

/// An agent's request that names no `max_output_tokens` gets the free
/// tier's, so a plain request fits the free tier; one that wants more
/// says so and pays.
pub(crate) fn default_output(state: &ServeState, caller: &Caller, request: &mut CreateResponse) {
    let Some(agents) = state.agents.as_ref() else {
        return;
    };
    if request.max_output_tokens.is_none()
        && caller
            .key_id
            .as_deref()
            .is_some_and(|key| agents.account_of_key(key).is_some())
    {
        request.max_output_tokens = Some(agents.config.free.max_output_tokens);
    }
}

/// Whether the free tier covers `request` as planned: every attempt on a
/// free model, a short text input, and a small output.
fn covered(free: &AgentFree, request: &CreateResponse, prepared: &Prepared) -> bool {
    let needs = Needs::of(request);
    !prepared.attempts().is_empty()
        && prepared
            .attempts()
            .iter()
            .all(|attempt| free.models.contains(&attempt.model))
        && !needs.images
        && !needs.files
        && needs.input_tokens.saturating_mul(4) <= free.max_input_chars
        && request
            .max_output_tokens
            .is_some_and(|max| max <= free.max_output_tokens)
}

/// Admits an agent's model request: on its free tier when the tier covers
/// it and has room, otherwise held from its bought credit, otherwise a
/// `402` that says what ran out and how to pay. Nothing is sent to a
/// model before this answers.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn admit_inference(
    state: &ServeState,
    account: &str,
    request: &CreateResponse,
    prepared: &Prepared,
    request_id: &str,
    tenant: &str,
    worst: u64,
    priced: Vec<(String, crate::money::Priced)>,
) -> Result<crate::inference_public::Kind, ApiError> {
    let Some(agents) = state.agents.as_ref() else {
        return Err(ApiError::new(
            ErrorType::LimitReached,
            "Agent accounts can't call models here.",
        ));
    };
    let Some(book) = &state.inference_book else {
        return Err(ApiError::new(
            ErrorType::ServerError,
            "Your account can't be checked right now. Try again in a minute.",
        ));
    };
    let free = &agents.config.free;
    let day = today();
    let covered = covered(free, request, prepared);
    let mut book = book.lock().await;
    let mut short = None;
    if covered {
        match book.reserve_agent_free(account, worst, free, day) {
            Ok(()) => {
                book.record_charge(
                    request_id,
                    crate::inference_public::Charge {
                        tenant: tenant.to_owned(),
                        free: true,
                        micros: None,
                        settlement: "agent_free",
                    },
                );
                return Ok(crate::inference_public::Kind::agent(
                    account, day, worst, priced, false,
                ));
            }
            Err(why) => short = Some(why),
        }
    }
    if book.reserve_agent_credit(account, worst) {
        book.record_charge(
            request_id,
            crate::inference_public::Charge {
                tenant: tenant.to_owned(),
                free: false,
                micros: None,
                settlement: "agent_credit",
            },
        );
        return Ok(crate::inference_public::Kind::agent(
            account, day, worst, priced, true,
        ));
    }
    let used = book.agent_day(account, day);
    let balance = book.agent_credit(account).balance;
    drop(book);
    let free_text = format!(
        "The free tier covers {} with at most {} input characters and max_output_tokens up to {}, no images or files",
        free.models.join(", "),
        free.max_input_chars,
        free.max_output_tokens
    );
    let (code, message) = match short {
        Some(AgentShort::Requests) => (
            "agent_free_requests_used",
            format!(
                "This agent has used its {} free requests for today. They start over at 00:00 UTC; to keep going now, buy credit.",
                free.requests_per_day
            ),
        ),
        Some(AgentShort::Budget) => (
            "agent_free_budget_used",
            format!(
                "This agent's free ${} for today can't cover this request, which could cost up to ${}. Send a shorter request, or buy credit.",
                micros_usd(free.usd_micros_per_day),
                micros_usd(worst)
            ),
        ),
        Some(AgentShort::Global) => (
            "agent_free_tier_full",
            "Free use for all agents is used up for today. It starts over at 00:00 UTC; to keep going now, buy credit.".to_string(),
        ),
        None if balance > 0 => (
            "agent_balance_too_low",
            format!(
                "This agent's credit (${}) can't cover this request, which could cost up to ${}. Buy more credit, or set a lower max_output_tokens.",
                micros_usd(balance),
                micros_usd(worst)
            ),
        ),
        None => (
            "agent_payment_required",
            format!("{free_text}. This request is outside it, so it needs credit."),
        ),
    };
    let calls = agents.calls_today(account, day);
    let mut error = ApiError::new(ErrorType::InsufficientBalance, message).with_code(code);
    error
        .extra
        .insert("free_tier".into(), agents.free_view(used, calls));
    error
        .extra
        .insert("balance_usd".into(), json!(micros_usd(balance)));
    error
        .extra
        .insert("request_max_cost_usd".into(), json!(micros_usd(worst)));
    error.extra.insert("pay".into(), agents.pay(account));
    Err(error)
}

// ------------------------------------------------------------ middleware

/// The paths an agent's model requests take, admitted by
/// [`admit_inference`] rather than counted here.
fn inference_path(path: &str) -> bool {
    matches!(
        path,
        "/v1/responses" | "/v1/chat/completions" | "/v1/responses/compact"
    )
}

fn decision_path(path: &str) -> bool {
    matches!(
        path,
        "/v1/systemone" | "/v1/decisions" | "/v1/classify" | "/v1/jobs"
    )
}

/// The key id of an `oak_<id>.<secret>` bearer.
fn bearer_key_id(headers: &HeaderMap) -> Option<String> {
    let token = headers
        .get("authorization")?
        .to_str()
        .ok()?
        .strip_prefix("Bearer ")?;
    let (id, _) = token.strip_prefix("oak_")?.split_once('.')?;
    Some(id.to_owned())
}

/// The agent free tier's call caps, around every route: an agent's
/// decision calls and its other API calls are counted per day, and past
/// a cap answer `429` until 00:00 UTC. An agent with credit isn't capped.
/// Model requests are left to their admission.
pub(crate) async fn limits(
    State(state): State<Arc<ServeState>>,
    request: Request,
    next: Next,
) -> Response {
    let Some(agents) = state.agents.as_ref() else {
        return next.run(request).await;
    };
    let path = request.uri().path();
    if request.method() == Method::OPTIONS
        || inference_path(path)
        || path.starts_with("/v1/agents/")
    {
        return next.run(request).await;
    }
    let Some(account) = bearer_key_id(request.headers()).and_then(|id| agents.account_of_key(&id))
    else {
        return next.run(request).await;
    };
    let funded = match &state.inference_book {
        Some(book) => book.lock().await.agent_credit(&account).balance >= FUNDED_MICROS,
        None => false,
    };
    if !funded {
        let decision = request.method() == Method::POST && decision_path(path);
        if let Err(cap) = agents.count_call(&account, decision, today()) {
            let free = &agents.config.free;
            let message = match cap {
                "decisions" => format!(
                    "This agent has made its {} free decision calls for today. They start over at 00:00 UTC.",
                    free.decisions_per_day
                ),
                "decisions_all" => "Free decision calls for all agents are used up for today. They start over at 00:00 UTC.".to_string(),
                "reads" => format!(
                    "This agent has made its {} free API calls for today. They start over at 00:00 UTC.",
                    free.reads_per_day
                ),
                _ => "Try again in a moment.".to_string(),
            };
            let wait = 86_400 - unix_now() % 86_400;
            let mut response = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({"error": {
                    "type": "rate_limited",
                    "code": "agent_daily_limit",
                    "message": message,
                    "resets_at": next_midnight(),
                    "pay": agents.pay(&account),
                }})),
            )
                .into_response();
            if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
                response.headers_mut().insert("retry-after", value);
            }
            return response;
        }
    }
    next.run(request).await
}

// ---------------------------------------------------------------- routes

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![
        (CHALLENGE, get(challenge)),
        (REGISTER, post(register)),
        (ME, get(me)),
        (CLAIM, post(claim)),
        (CLAIMED, get(claimed)),
        (REVOKE, post(revoke)),
        (CREDIT, post(credit)),
        (ADMIN, get(admin_list)),
    ]
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert("cache-control", HeaderValue::from_static("no-store"));
    response
}

/// `GET /v1/agents/challenge`.
async fn challenge(State(state): State<Arc<ServeState>>) -> Response {
    let agents = match agents(&state) {
        Ok(agents) => agents,
        Err(response) => return response,
    };
    let (challenge, expires) = agents.challenge(unix_now());
    let bits = agents.config.difficulty_bits;
    no_store(answered(
        StatusCode::OK,
        json!({
            "challenge": challenge,
            "algorithm": "sha256",
            "difficulty_bits": bits,
            "expires_at": expires,
            "how": format!(
                "Find a solution (1 to 64 printable characters, such as a counter) where SHA-256 of the challenge, a colon, and the solution starts with {bits} zero bits. Then POST {REGISTER} with {{\"name\", \"challenge\", \"solution\"}}."
            ),
            "register": REGISTER,
            "docs": DOCS,
        }),
    ))
}

#[derive(Deserialize)]
struct RegisterBody {
    name: String,
    challenge: String,
    solution: String,
}

/// `POST /v1/agents/register`.
async fn register(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let agents = match agents(&state) {
        Ok(agents) => agents,
        Err(response) => return response,
    };
    let Ok(body) = serde_json::from_slice::<RegisterBody>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send JSON with `name`, `challenge`, and `solution`. Get a challenge at GET /v1/agents/challenge.",
        );
    };
    let name = body.name.trim();
    if !agents::valid_label(name) {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_name",
            "`name` must be 1 to 64 characters with no line breaks.",
        );
    }
    let now = unix_now();
    if let Err(why) = agents.redeem(&body.challenge, &body.solution, now) {
        return refused(StatusCode::BAD_REQUEST, "invalid_proof", why);
    }
    let hops = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .map_or(0, |v| v.split(',').count());
    eprintln!("agent register: forwarded entries {hops}");
    let ip = caller_ip(&headers, agents.config.forwarded_hops);
    if let Err(why) = agents.admit_signup(ip, today()) {
        let wait = 86_400 - now % 86_400;
        let mut response = refused(StatusCode::TOO_MANY_REQUESTS, "signup_limit", why);
        if let Ok(value) = HeaderValue::from_str(&wait.to_string()) {
            response.headers_mut().insert("retry-after", value);
        }
        return response;
    }
    let registry = match crate::accounts::registry(&state) {
        Ok(registry) => registry,
        Err(response) => return response,
    };
    let issued = match keys::issue_scoped(
        &state.dir,
        registry.manifest(),
        &agents.tenant,
        Some("agent"),
        keys::Scopes::public(SCOPES),
    ) {
        Ok(issued) => issued,
        Err(trouble) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "signup_unavailable",
                format!("We couldn't make a key right now. Try again in a minute. ({trouble})"),
            );
        }
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => {
            let _ = keys::revoke(&state.dir, &issued.key.id);
            return response;
        }
    };
    let secret = match agents::claim_secret() {
        Ok(secret) => secret,
        Err(_) => {
            let _ = keys::revoke(&state.dir, &issued.key.id);
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "signup_unavailable",
                "We couldn't finish making the agent. Try again in a minute.",
            );
        }
    };
    let made = match accounts.register_agent(
        name,
        &agents.tenant,
        &issued.key.id,
        &agents::claim_digest(&secret),
        now + CLAIM_TTL,
    ) {
        Ok(made) => made,
        Err(refusal) => {
            let _ = keys::revoke(&state.dir, &issued.key.id);
            return accounts_refusal(refusal);
        }
    };
    if let Ok(mut index) = agents.index.write() {
        index.insert(issued.key.id.clone(), made.account.id.clone());
    }
    let used = crate::inference_public::AgentDay::default();
    no_store(answered(
        StatusCode::CREATED,
        json!({
            "account": {"id": made.account.id, "name": made.account.label, "kind": "agent"},
            "workspace": {"id": made.workspace.id},
            "api_key": issued.token,
            "key": {"id": issued.key.id, "scopes": SCOPES},
            "use": "Send the API key as `Authorization: Bearer <api_key>` to https://openagents.com/v1 (or https://openagents.com/api/v1). It is shown only now.",
            "claim": {
                "code": format!("{CLAIM_PREFIX}{}.{secret}", made.account.id),
                "page": CLAIM_PAGE,
                "expires_at": now + CLAIM_TTL,
                "how": "To link this agent to a person's OpenAgents account, the person signs in, opens the page, and enters the code. It works once.",
            },
            "free_tier": agents.free_view(used, (0, 0)),
            "pay": agents.pay(&made.account.id),
            "docs": DOCS,
        }),
    ))
}

/// The agent account behind an `oak_` bearer.
fn calling_agent(state: &ServeState, headers: &HeaderMap) -> Result<String, Response> {
    let agents = agents(state)?;
    let unknown = || {
        refused(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Send this agent's API key in the `Authorization: Bearer` header.",
        )
    };
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(unknown)?;
    let registry = crate::accounts::registry(state)?;
    let authenticated =
        keys::authenticate(&state.dir, registry.manifest(), token).map_err(|_| unknown())?;
    agents.account_of_key(&authenticated.key_id).ok_or_else(|| {
        refused(
            StatusCode::NOT_FOUND,
            "not_an_agent",
            "This key doesn't belong to an agent account.",
        )
    })
}

/// `GET /v1/agents/me`: the calling agent, its free tier today, its
/// credit, and how to pay.
async fn me(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let account = match calling_agent(&state, &headers) {
        Ok(account) => account,
        Err(response) => return response,
    };
    let Some(agents) = state.agents.as_ref() else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let day = today();
    let (used, balance) = match &state.inference_book {
        Some(book) => {
            let book = book.lock().await;
            (
                book.agent_day(&account, day),
                book.agent_credit(&account).balance,
            )
        }
        None => Default::default(),
    };
    let store = accounts_store(&state).and_then(|a| {
        a.store().map_err(|t| {
            refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts_unavailable",
                t.to_string(),
            )
        })
    });
    let (name, claimed) = match &store {
        Ok(store) => store
            .accounts
            .get(&account)
            .map_or((String::new(), false), |a| {
                (
                    a.label.clone(),
                    a.agent
                        .as_ref()
                        .is_some_and(|agent| agent.claimed_by.is_some()),
                )
            }),
        Err(_) => (String::new(), false),
    };
    no_store(answered(
        StatusCode::OK,
        json!({
            "account": {"id": account, "name": name, "kind": "agent", "claimed": claimed},
            "free_tier": agents.free_view(used, agents.calls_today(&account, day)),
            "balance_usd": micros_usd(balance),
            "pay": agents.pay(&account),
            "docs": DOCS,
        }),
    ))
}

/// The signed-in person behind a `sess_` bearer.
fn person(state: &ServeState, headers: &HeaderMap) -> Result<String, Response> {
    let token = headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .filter(|token| tenancy::sessions::is_session_bearer(token))
        .ok_or_else(|| {
            refused(
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                "Sign in on openagents.com first: this needs a person's session.",
            )
        })?;
    match session_principal(state, token) {
        Ok(Principal::Account { account, .. }) => Ok(account),
        _ => Err(refused(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "Your session isn't recognized. Sign in again.",
        )),
    }
}

fn agent_view(account: &tenancy::accounts::Account, extra: Option<(u64, u64)>) -> Value {
    let agent = account.agent.as_ref();
    let mut view = json!({
        "id": account.id,
        "name": account.label,
        "registered": agent.map(|a| a.registered.clone()),
        "claimed_by": agent.and_then(|a| a.claimed_by.clone()),
        "claimed": agent.and_then(|a| a.claimed.clone()),
        "revoked": agent.and_then(|a| a.revoked.clone()),
    });
    if let Some((used, balance)) = extra {
        view["free_usd_used_today"] = json!(micros_usd(used));
        view["balance_usd"] = json!(micros_usd(balance));
    }
    view
}

#[derive(Deserialize)]
struct ClaimBody {
    claim: String,
}

/// `POST /v1/agents/claim` `{claim}`: link an agent to the signed-in
/// person.
async fn claim(State(state): State<Arc<ServeState>>, headers: HeaderMap, body: Bytes) -> Response {
    if let Err(response) = agents(&state) {
        return response;
    }
    let person = match person(&state, &headers) {
        Ok(person) => person,
        Err(response) => return response,
    };
    let Ok(body) = serde_json::from_slice::<ClaimBody>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send JSON with `claim`, the code the agent got when it registered.",
        );
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    match accounts.claim_agent(&person, body.claim.trim(), unix_now()) {
        Ok(account) => answered(StatusCode::OK, json!({"agent": agent_view(&account, None)})),
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `GET /v1/agents/claimed`: the signed-in person's agents.
async fn claimed(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    if let Err(response) = agents(&state) {
        return response;
    }
    let person = match person(&state, &headers) {
        Ok(person) => person,
        Err(response) => return response,
    };
    let store = match accounts_store(&state).and_then(|a| {
        a.store().map_err(|t| {
            refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts_unavailable",
                t.to_string(),
            )
        })
    }) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let mine: Vec<Value> = store
        .agents()
        .filter(|a| {
            a.agent
                .as_ref()
                .is_some_and(|agent| agent.claimed_by.as_deref() == Some(person.as_str()))
        })
        .map(|a| agent_view(a, None))
        .collect();
    answered(StatusCode::OK, json!({"agents": mine}))
}

/// `POST /v1/agents/{account}/revoke`: the claiming person or a site
/// admin revokes an agent. Its keys stop at once.
async fn revoke(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    UrlPath(target): UrlPath<String>,
) -> Response {
    let agents = match agents(&state) {
        Ok(agents) => agents,
        Err(response) => return response,
    };
    let person = match person(&state, &headers) {
        Ok(person) => person,
        Err(response) => return response,
    };
    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(trouble) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts_unavailable",
                trouble.to_string(),
            );
        }
    };
    let Some(account) = store.accounts.get(&target).filter(|a| a.agent.is_some()) else {
        return refused(
            StatusCode::NOT_FOUND,
            "not_an_agent",
            "No agent has that id.",
        );
    };
    let claimer = account
        .agent
        .as_ref()
        .and_then(|agent| agent.claimed_by.as_deref())
        == Some(person.as_str());
    if !claimer && !crate::accounts::account_is_site_admin(&state, &person) {
        return refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Only the person who claimed this agent, or a site admin, can revoke it.",
        );
    }
    for principal in &account.principals {
        if let Some(key) = principal.strip_prefix("key:") {
            let _ = keys::revoke(&state.dir, key);
            if let Ok(mut index) = agents.index.write() {
                index.remove(key);
            }
        }
    }
    match accounts.revoke_agent(&target, &person) {
        Ok(account) => answered(StatusCode::OK, json!({"agent": agent_view(&account, None)})),
        Err(refusal) => accounts_refusal(refusal),
    }
}

/// `GET /v1/admin/agents`: every agent account, newest first, for site
/// admins.
async fn admin_list(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    if let Err(response) = agents(&state) {
        return response;
    }
    let person = match person(&state, &headers) {
        Ok(person) => person,
        Err(response) => return response,
    };
    if !crate::accounts::account_is_site_admin(&state, &person) {
        return refused(StatusCode::NOT_FOUND, "not_found", "Not found.");
    }
    let store = match accounts_store(&state).and_then(|a| {
        a.store().map_err(|t| {
            refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "accounts_unavailable",
                t.to_string(),
            )
        })
    }) {
        Ok(store) => store,
        Err(response) => return response,
    };
    let day = today();
    let book = match &state.inference_book {
        Some(book) => Some(book.lock().await),
        None => None,
    };
    let mut all: Vec<(&str, Value)> = store
        .agents()
        .map(|a| {
            let extra = book.as_ref().map(|book| {
                (
                    book.agent_day(&a.id, day).micros,
                    book.agent_credit(&a.id).balance,
                )
            });
            (
                a.agent
                    .as_ref()
                    .map_or("", |agent| agent.registered.as_str()),
                agent_view(a, extra),
            )
        })
        .collect();
    all.sort_by(|a, b| b.0.cmp(a.0));
    let (total, revoked) = (
        all.len(),
        all.iter().filter(|(_, v)| !v["revoked"].is_null()).count(),
    );
    answered(
        StatusCode::OK,
        json!({
            "total": total,
            "revoked": revoked,
            "agents": all.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
        }),
    )
}

#[derive(Deserialize)]
struct CreditQuery {
    account: Option<String>,
}

/// `POST /v1/agents/credit?account=acct_…`: the pay host's call once an
/// agent paid for a top-up. Only a request carrying the pay host's
/// signed header for the configured resource, a payment not credited
/// before, adds credit.
async fn credit(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Query(query): Query<CreditQuery>,
    request: axum::http::Uri,
    body: Bytes,
) -> Response {
    let agents = match agents(&state) {
        Ok(agents) => agents,
        Err(response) => return response,
    };
    let Some(config) = &agents.config.credit else {
        return refused(
            StatusCode::NOT_FOUND,
            "credit_off",
            "Buying credit isn't open for agent accounts yet.",
        );
    };
    let Some(header) = headers
        .get(openagents_x402::hosted::PAID_HEADER)
        .and_then(|v| v.to_str().ok())
    else {
        return refused(
            StatusCode::PAYMENT_REQUIRED,
            "payment_required",
            format!("Buy credit at {}.", config.buy_url),
        );
    };
    let url = match request.query() {
        Some(query) => format!("{}?{query}", config.upstream_url),
        None => config.upstream_url.clone(),
    };
    let paid = match openagents_x402::hosted::verify_paid(
        header,
        &config.paid_key,
        "POST",
        &url,
        &body,
        unix_now(),
    ) {
        Ok(paid) => paid,
        Err(why) => return refused(StatusCode::UNAUTHORIZED, "invalid_payment", why),
    };
    let resource = config.buy_url.rsplit('/').next().unwrap_or_default();
    if paid.resource != resource || paid.payment_hash.is_empty() {
        return refused(
            StatusCode::UNAUTHORIZED,
            "invalid_payment",
            "That payment was for something else.",
        );
    }
    let Some(account) = query.account.filter(|a| a.starts_with("acct_")) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Name the agent: ?account=acct_…",
        );
    };
    let is_agent = accounts_store(&state)
        .ok()
        .and_then(|a| a.store().ok())
        .is_some_and(|store| store.agent(&account).is_some());
    if !is_agent {
        return refused(
            StatusCode::NOT_FOUND,
            "not_an_agent",
            "No agent has that id.",
        );
    }
    let Some(book) = &state.inference_book else {
        return refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "credit_unavailable",
            "Credit can't be added right now.",
        );
    };
    match agents.credit_once(&paid.payment_hash, &account, config.credit_usd_micros) {
        Ok(true) => {
            let added = book
                .lock()
                .await
                .add_agent_credit(&account, config.credit_usd_micros);
            match added {
                Ok(credit) => answered(
                    StatusCode::OK,
                    json!({
                        "account": account,
                        "credited_usd": micros_usd(config.credit_usd_micros),
                        "balance_usd": micros_usd(credit.balance),
                    }),
                ),
                Err(why) => refused(StatusCode::SERVICE_UNAVAILABLE, "credit_unavailable", why),
            }
        }
        Ok(false) => {
            let balance = book.lock().await.agent_credit(&account).balance;
            answered(
                StatusCode::OK,
                json!({"account": account, "credited_usd": "0", "balance_usd": micros_usd(balance), "already_credited": true}),
            )
        }
        Err(why) => refused(StatusCode::SERVICE_UNAVAILABLE, "credit_unavailable", why),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_bits_count_from_the_left() {
        assert_eq!(leading_zero_bits(&[0, 0, 0x0f, 0xff]), 20);
        assert_eq!(leading_zero_bits(&[0x80]), 0);
        assert_eq!(leading_zero_bits(&[0, 1]), 15);
    }

    #[test]
    fn networks_and_forwarded_addresses() {
        assert_eq!(network("203.0.113.7".parse().unwrap()), "203.0.113.0/24");
        assert_eq!(
            network("2001:db8:1:2::9".parse().unwrap()),
            "2001:db8:1::/48"
        );
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("1.2.3.4, 203.0.113.7"),
        );
        assert_eq!(caller_ip(&headers, 1), Some("203.0.113.7".parse().unwrap()));
        assert_eq!(caller_ip(&headers, 2), Some("1.2.3.4".parse().unwrap()));
        assert_eq!(caller_ip(&headers, 3), None);
    }

    #[test]
    fn midnight_is_a_date() {
        assert_eq!(civil(0), (1970, 1, 1));
        assert_eq!(civil(20_740), (2026, 10, 14));
    }
}

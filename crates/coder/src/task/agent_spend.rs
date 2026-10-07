//! A workshop agent's spend records (#10805;
//! `docs/verse/agent-identity-and-engrams.md`, "The steering loop", Budget).
//!
//! **Records.** Each completed call that reported usage leaves one NIP-AM
//! `kind:44200` turn metric: her plan and report calls, and each Coder turn
//! she prompts. She signs it with her own key and encrypts it with NIP-44
//! v2 to her owner, the `owner` of the NIP-OA attestation in her record,
//! with exactly one `p` tag (the owner) and one `agent` tag (her key). The
//! payload holds usage numbers and identifiers only, never a prompt, a
//! reply, or a command. A call with no observed usage leaves no record, as
//! NIP-AM says; Jev reports no usage, so a judgment leaves none.
//!
//! **Ordering.** One request is one NIP-AM session: a fresh `sessionId` per
//! request, and `turnSeq` from 1 for each record in it. `cumulative` sums
//! the request's counters; a counter that any turn left unknown is unknown
//! in `cumulative` from then on, never a stand-in zero.
//!
//! **Ledger.** `agents/NAME/spend.jsonl` (mode `0600`) holds the signed
//! events, one per line, append-only. The file is ciphertext: she and the
//! owner each decrypt it with their own key. When relay sync is on, a sync
//! pass publishes each record a relay lacks (`agent_sync`).
//!
//! **Budgets.** `agents/NAME/budget.json` (`openagents.agent-budget.v1`) is
//! the owner's grant: dollars and tokens for one request and for one UTC
//! day. Without the file the provisional defaults below hold. [`Meter`]
//! reads today's records from the ledger before a request starts, refuses
//! to start once the day's budget is used, and after each record says when
//! the request's or the day's budget is used, which stops her loop before
//! the next call. An unreadable budget or ledger refuses the request: she
//! never spends without a grant she can read. Costs are the estimates the
//! provider or Microcoder's list prices give; an unknown cost is counted as
//! unknown, and the token budgets still bound it.
//!
//! **Not payments.** These records account for model usage. Paying anyone
//! is a separate act on the spend protocol (`docs/breez/spend-protocol.md`)
//! that she never takes: her policy refuses it before Coder is asked.
//!
//! **No key.** An agent without a key or an owner attestation can't seal a
//! record. Her meter then tallies the request in memory, so the request
//! budget still holds, and journals once that nothing is recorded.
//!
//! The record shape follows Buzz's NIP-AM publisher, reimplemented here; no
//! code is copied.

use std::io::Write;
use std::path::PathBuf;

use nostr::domain::{AGENT_TURN_METRIC_KIND, Event, RelaySigner, Tag, agent_turn_metric_owner};
use nostr::nip44;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};

use super::agent::{self, Record, Store};

/// Her spend ledger, beside her record.
pub const LEDGER_FILE: &str = "spend.jsonl";
/// The owner's budget for her, beside her record.
pub const BUDGET_FILE: &str = "budget.json";
/// The budget's schema.
pub const BUDGET_SCHEMA: &str = "openagents.agent-budget.v1";
/// The `harness` of her own calls.
pub const HARNESS: &str = "openagents-agent";
/// The `harness` of a Coder turn she prompts.
pub const CODER_HARNESS: &str = "coder-v1";
/// Dollars one request may spend when the owner set no budget.
/// Provisional, as the loop's thresholds.
pub const REQUEST_USD: f64 = 1.0;
/// Dollars one UTC day may spend when the owner set no budget.
pub const DAILY_USD: f64 = 5.0;
/// Tokens one request may use when the owner set no budget.
pub const REQUEST_TOKENS: u64 = 2_000_000;
/// Tokens one UTC day may use when the owner set no budget.
pub const DAILY_TOKENS: u64 = 10_000_000;
/// The most records one request keeps; NIP-AM relays limit an agent to 60
/// a minute, and her call budget is far below this.
const RECORDS_MAX: u64 = 1_000;
const DAY: u64 = 86_400;

// ------------------------------------------------------------- the payload

/// One NIP-AM usage object, `turn` or `cumulative`. `None` is unknown and
/// serializes as `null`; it is never summed as zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Counters {
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    #[serde(default)]
    pub total_tokens: Option<u64>,
    /// Estimated dollars.
    #[serde(default)]
    pub cost_usd: Option<f64>,
}

impl Counters {
    /// Whether no counter is known.
    #[must_use]
    pub fn unknown(&self) -> bool {
        self.input_tokens.is_none()
            && self.output_tokens.is_none()
            && self.total_tokens.is_none()
            && self.cost_usd.is_none()
    }

    /// The tokens a budget counts: the reported total, else input plus
    /// output as a lower bound. The record itself never derives a total.
    #[must_use]
    pub fn tokens(&self) -> u64 {
        self.total_tokens.unwrap_or_else(|| {
            self.input_tokens
                .unwrap_or(0)
                .saturating_add(self.output_tokens.unwrap_or(0))
        })
    }

    fn check(&self) -> Result<(), String> {
        match self.cost_usd {
            Some(cost) if !cost.is_finite() || cost < 0.0 => {
                Err("costUsd must be a finite, non-negative number".into())
            }
            _ => Ok(()),
        }
    }

    /// `self` after `turn`: each counter stays known only while every turn
    /// reported it.
    fn then(self, turn: &Self) -> Self {
        fn add(a: Option<u64>, b: Option<u64>) -> Option<u64> {
            a.zip(b).map(|(a, b)| a.saturating_add(b))
        }
        Self {
            input_tokens: add(self.input_tokens, turn.input_tokens),
            output_tokens: add(self.output_tokens, turn.output_tokens),
            total_tokens: add(self.total_tokens, turn.total_tokens),
            cost_usd: self.cost_usd.zip(turn.cost_usd).map(|(a, b)| a + b),
        }
    }

    /// The starting point of a session's `cumulative`.
    fn zero() -> Self {
        Self {
            input_tokens: Some(0),
            output_tokens: Some(0),
            total_tokens: Some(0),
            cost_usd: Some(0.0),
        }
    }
}

/// The decrypted NIP-AM payload. Unknown fields are ignored when read.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub harness: String,
    #[serde(default)]
    pub model: Option<String>,
    /// Never set: a channel is private usage metadata she doesn't have.
    #[serde(default)]
    pub channel_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    /// Which call: `plan`, `report`, or `coder-N`.
    #[serde(default)]
    pub turn_id: Option<String>,
    #[serde(default)]
    pub turn_seq: Option<u64>,
    /// RFC 3339, the end of the call.
    pub timestamp: String,
    pub turn: Counters,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cumulative: Option<Counters>,
    #[serde(default = "reliable")]
    pub delta_reliable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
}

fn reliable() -> bool {
    true
}

impl Metric {
    /// Checks the rules a reader relies on.
    ///
    /// # Errors
    /// When a required field is missing or a number is out of range.
    pub fn check(&self) -> Result<(), String> {
        if self.harness.is_empty() {
            return Err("a turn metric names its harness".into());
        }
        if self.timestamp.is_empty() {
            return Err("a turn metric carries its timestamp".into());
        }
        if self.cumulative.is_some() && (self.session_id.is_none() || self.turn_seq.is_none()) {
            return Err("a cumulative turn metric names its session and sequence".into());
        }
        self.turn.check()?;
        if let Some(cumulative) = &self.cumulative {
            cumulative.check()?;
        }
        Ok(())
    }
}

// ------------------------------------------------------- sealing and reading

/// Seals `metric` as her `kind:44200` event to `owner` at `created_at`.
///
/// # Errors
/// When the metric breaks a rule or can't be encrypted.
pub fn seal(
    metric: &Metric,
    agent_secret: &SecretKey,
    owner: &XOnlyPublicKey,
    created_at: u64,
) -> Result<Event, String> {
    metric.check()?;
    let plaintext = serde_json::to_string(metric).map_err(|e| e.to_string())?;
    let key = nip44::conversation_key(agent_secret, owner);
    let content = nip44::encrypt(&plaintext, &key, secp256k1::rand::random::<[u8; 32]>())?;
    let hex: String = agent_secret
        .secret_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let signer = RelaySigner::from_secret_hex(&hex).map_err(|e| e.to_string())?;
    let agent = signer.pubkey().to_owned();
    let event = signer.sign(
        created_at,
        AGENT_TURN_METRIC_KIND,
        vec![
            Tag::new(vec!["p".into(), owner.to_string()]),
            Tag::new(vec!["agent".into(), agent]),
        ],
        content,
    );
    agent_turn_metric_owner(&event)?;
    Ok(event)
}

/// Opens `event` with a NIP-44 conversation key: the envelope, the
/// signature, the ciphertext, and the payload's rules.
fn open(event: &Event, key: &[u8; 32]) -> Result<Metric, String> {
    agent_turn_metric_owner(event)?;
    event
        .validate_crypto()
        .map_err(|_| "the turn metric's signature does not verify".to_string())?;
    let plaintext = nip44::decrypt(&event.content, key)?;
    let metric: Metric = serde_json::from_str(&plaintext)
        .map_err(|e| format!("the turn metric is not a NIP-AM payload: {e}"))?;
    metric.check()?;
    Ok(metric)
}

/// Opens one of her records with her own key.
///
/// # Errors
/// When it isn't hers for `owner` or doesn't decrypt.
pub fn open_as_agent(
    event: &Event,
    agent_secret: &SecretKey,
    owner: &XOnlyPublicKey,
) -> Result<Metric, String> {
    if event.pubkey != agent::public_hex(agent_secret) {
        return Err("the turn metric is another agent's".into());
    }
    if agent_turn_metric_owner(event)? != owner.to_string() {
        return Err("the turn metric is for another owner".into());
    }
    open(event, &nip44::conversation_key(agent_secret, owner))
}

/// Opens one of her records with the owner's key; her key comes from the
/// event. The owner never needs hers.
///
/// # Errors
/// When it isn't to this owner or doesn't decrypt.
pub fn open_as_owner(event: &Event, owner_secret: &SecretKey) -> Result<Metric, String> {
    if agent_turn_metric_owner(event)? != agent::public_hex(owner_secret) {
        return Err("the turn metric is for another owner".into());
    }
    let agent = event
        .pubkey
        .parse::<XOnlyPublicKey>()
        .map_err(|_| "the turn metric's author is not a key".to_string())?;
    open(event, &nip44::conversation_key(owner_secret, &agent))
}

fn ledger_path(store: &Store) -> PathBuf {
    store.dir().join(LEDGER_FILE)
}

/// Every event in her ledger, oldest first. A missing ledger holds none.
///
/// # Errors
/// When the ledger can't be read or a line isn't an event.
pub fn ledger(store: &Store) -> Result<Vec<Event>, String> {
    let path = ledger_path(store);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
    };
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .enumerate()
        .map(|(n, line)| {
            serde_json::from_str(line)
                .map_err(|e| format!("{} line {} is not an event: {e}", path.display(), n + 1))
        })
        .collect()
}

fn append(store: &Store, event: &Event) -> Result<(), String> {
    agent::private_dir(store.dir())?;
    let path = ledger_path(store);
    let mut line = serde_json::to_vec(event).map_err(|e| e.to_string())?;
    line.push(b'\n');
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    file.write_all(&line)
        .and_then(|()| file.flush())
        .map_err(|e| format!("cannot append to {}: {e}", path.display()))
}

/// One record as a reader opened it.
#[derive(Clone, Debug)]
pub struct Read {
    pub id: String,
    pub created_at: u64,
    pub metric: Metric,
}

/// What a reader opens from her ledger, with the owner key or hers.
#[derive(Clone, Debug, Default)]
pub struct View {
    /// Every record that verified and decrypted, oldest first, each once.
    pub records: Vec<Read>,
    /// Each line that did not, and why.
    pub problems: Vec<String>,
}

impl View {
    /// The records since `since`, Unix seconds.
    #[must_use]
    pub fn tally(&self, since: u64) -> Tally {
        let mut tally = Tally::default();
        for read in self.records.iter().filter(|r| r.created_at >= since) {
            tally.add(&read.metric.turn);
        }
        tally
    }
}

/// Decrypts her ledger with `owner`, the owner's secret key.
///
/// # Errors
/// When the ledger can't be read.
pub fn owner_read(store: &Store, owner: &SecretKey) -> Result<View, String> {
    read_with(store, |event| open_as_owner(event, owner))
}

/// Decrypts her ledger with her own key, as the host does.
///
/// # Errors
/// When she can't seal records (no key, or no owner attestation), or the
/// ledger can't be read.
pub fn agent_read(store: &Store, record: &Record) -> Result<View, String> {
    let (secret, owner) = keys(store, record)?;
    read_with(store, |event| open_as_agent(event, &secret, &owner))
}

fn read_with(
    store: &Store,
    open: impl Fn(&Event) -> Result<Metric, String>,
) -> Result<View, String> {
    let mut view = View::default();
    let mut seen = std::collections::BTreeSet::new();
    for event in ledger(store)? {
        if !seen.insert(event.id.clone()) {
            continue;
        }
        match open(&event) {
            Ok(metric) => view.records.push(Read {
                id: event.id.clone(),
                created_at: event.created_at,
                metric,
            }),
            Err(why) => view.problems.push(format!("{}: {why}", short(&event.id))),
        }
    }
    Ok(view)
}

fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

// ------------------------------------------------------------ the budget

/// `budget.json`: what the owner lets her spend. A missing limit is the
/// default for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub schema: String,
    pub v: u32,
    #[serde(default = "request_usd")]
    pub request_usd: f64,
    #[serde(default = "daily_usd")]
    pub daily_usd: f64,
    #[serde(default = "request_tokens")]
    pub request_tokens: u64,
    #[serde(default = "daily_tokens")]
    pub daily_tokens: u64,
}

fn request_usd() -> f64 {
    REQUEST_USD
}
fn daily_usd() -> f64 {
    DAILY_USD
}
fn request_tokens() -> u64 {
    REQUEST_TOKENS
}
fn daily_tokens() -> u64 {
    DAILY_TOKENS
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            schema: BUDGET_SCHEMA.into(),
            v: 1,
            request_usd: REQUEST_USD,
            daily_usd: DAILY_USD,
            request_tokens: REQUEST_TOKENS,
            daily_tokens: DAILY_TOKENS,
        }
    }
}

impl Budget {
    /// `store`'s budget; the defaults when there is no file.
    ///
    /// # Errors
    /// When the file exists and isn't a v1 budget with finite,
    /// non-negative dollars.
    pub fn load(store: &Store) -> Result<Self, String> {
        let path = store.dir().join(BUDGET_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("cannot read {}: {e}", path.display())),
        };
        let budget: Self = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a budget: {e}", path.display()))?;
        if budget.schema != BUDGET_SCHEMA || budget.v != 1 {
            return Err(format!("{} is not a v1 budget", path.display()));
        }
        for usd in [budget.request_usd, budget.daily_usd] {
            if !usd.is_finite() || usd < 0.0 {
                return Err(format!(
                    "{} has a dollar limit out of range",
                    path.display()
                ));
            }
        }
        Ok(budget)
    }

    /// Writes `self` as `store`'s budget, mode `0600`.
    ///
    /// # Errors
    /// When it can't be written.
    pub fn save(&self, store: &Store) -> Result<(), String> {
        agent::private_dir(store.dir())?;
        let body = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        agent::write_private(&store.dir().join(BUDGET_FILE), &body)
    }
}

/// What a set of records spent.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Tally {
    pub records: u64,
    /// Known dollars.
    pub usd: f64,
    /// Records whose cost is unknown.
    pub unpriced: u64,
    /// Tokens, as [`Counters::tokens`] counts them.
    pub tokens: u64,
}

impl Tally {
    fn add(&mut self, turn: &Counters) {
        self.records += 1;
        match turn.cost_usd {
            Some(cost) => self.usd += cost,
            None => self.unpriced += 1,
        }
        self.tokens = self.tokens.saturating_add(turn.tokens());
    }

    /// In words, for her journal and `openagents agent show`.
    #[must_use]
    pub fn words(&self) -> String {
        let unpriced = if self.unpriced == 0 {
            String::new()
        } else {
            format!(", {} without a reported cost", self.unpriced)
        };
        format!(
            "${:.4} and {} tokens over {} records{unpriced}",
            self.usd, self.tokens, self.records
        )
    }

    /// Which limit `self` has reached, in words.
    fn reached(&self, usd: f64, tokens: u64, what: &str) -> Option<String> {
        if self.usd >= usd {
            Some(format!("{what} budget of ${usd:.2}"))
        } else if self.tokens >= tokens {
            Some(format!("{what} budget of {tokens} tokens"))
        } else {
            None
        }
    }
}

/// The start of the UTC day `now` falls in.
#[must_use]
pub fn day_of(now: u64) -> u64 {
    now - now % DAY
}

// --------------------------------------------------------------- the meter

/// One call to record: who answered, which call, and its usage.
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub harness: &'static str,
    /// `plan`, `report`, or `coder-N`.
    pub turn_id: String,
    pub model: Option<String>,
    pub usage: Counters,
    /// `end_turn`, `cancelled`, or `error`.
    pub stop: &'static str,
}

/// Her spend for one request: it seals each call's record, keeps the
/// request's and the day's tallies from the records, and says when a
/// budget is used.
#[derive(Debug)]
pub struct Meter {
    store: Store,
    /// Her key and her owner's, when she can seal records.
    keys: Option<(SecretKey, XOnlyPublicKey)>,
    /// Why she can't, for her journal.
    pub unsealed: Option<String>,
    budget: Budget,
    session: String,
    seq: u64,
    cumulative: Counters,
    day: u64,
    request: Tally,
    today: Tally,
}

impl Meter {
    /// The meter for a request `record` starts at `now`: her budget, and
    /// today's records read back with her key.
    ///
    /// # Errors
    /// When her budget or her ledger can't be read, or a record in it
    /// doesn't open with her key: she never spends against a budget she
    /// can't check.
    pub fn open(store: &Store, record: &Record, now: u64) -> Result<Self, String> {
        let budget = Budget::load(store)?;
        let (keys, unsealed) = match keys(store, record) {
            Ok(keys) => (Some(keys), None),
            Err(why) => (None, Some(why)),
        };
        let day = day_of(now);
        let mut today = Tally::default();
        if let Some((secret, owner)) = &keys {
            for event in ledger(store)? {
                if event.created_at < day {
                    continue;
                }
                let metric = open_as_agent(&event, secret, owner)
                    .map_err(|why| format!("spend record {}: {why}", short(&event.id)))?;
                today.add(&metric.turn);
            }
        }
        Ok(Self {
            store: store.clone(),
            keys,
            unsealed,
            budget,
            session: format!(
                "{}-{now}-{:08x}",
                record.name,
                secp256k1::rand::random::<u32>()
            ),
            seq: 0,
            cumulative: Counters::zero(),
            day,
            request: Tally::default(),
            today,
        })
    }

    /// This request's NIP-AM `sessionId`.
    #[must_use]
    pub fn session(&self) -> &str {
        &self.session
    }

    #[must_use]
    pub fn budget(&self) -> &Budget {
        &self.budget
    }

    /// What this request spent so far.
    #[must_use]
    pub fn request(&self) -> Tally {
        self.request
    }

    /// What today's records spent, this request's included.
    #[must_use]
    pub fn today(&self) -> Tally {
        self.today
    }

    /// Whether she may start a request: `Err` names the day's budget she
    /// has used.
    ///
    /// # Errors
    /// When today's records have reached the day's budget.
    pub fn admit(&self) -> Result<(), String> {
        match self
            .today
            .reached(self.budget.daily_usd, self.budget.daily_tokens, "daily")
        {
            Some(limit) => Err(format!(
                "today's records reach the {limit} ({})",
                self.today.words()
            )),
            None => Ok(()),
        }
    }

    /// Records `call` at `now`: seals and appends its record when she has
    /// keys, and adds it to the tallies. `Ok(Some(why))` when a budget is
    /// now used, so she stops before the next call. A call with no
    /// observed usage leaves no record.
    ///
    /// # Errors
    /// When the record can't be sealed or kept; the call still counts.
    pub fn record(&mut self, call: &Call, now: u64) -> Result<Option<String>, String> {
        if call.usage.unknown() {
            return Ok(None);
        }
        if day_of(now) != self.day {
            // A request that runs past midnight counts against the new day
            // from then on.
            self.day = day_of(now);
            self.today = Tally::default();
        }
        self.request.add(&call.usage);
        self.today.add(&call.usage);
        if self.seq < RECORDS_MAX {
            self.seq += 1;
            self.cumulative = self.cumulative.then(&call.usage);
            self.seal(call, now)?;
        }
        Ok(self.stop())
    }

    fn seal(&self, call: &Call, now: u64) -> Result<(), String> {
        let Some((secret, owner)) = &self.keys else {
            return Ok(());
        };
        let metric = Metric {
            harness: call.harness.into(),
            model: call.model.clone().filter(|m| !m.is_empty()),
            channel_id: None,
            session_id: Some(self.session.clone()),
            turn_id: Some(call.turn_id.clone()),
            turn_seq: Some(self.seq),
            timestamp: crate::relay::usage::rfc3339(now.saturating_mul(1_000)),
            turn: call.usage,
            cumulative: Some(self.cumulative),
            delta_reliable: true,
            stop_reason: Some(call.stop.into()),
        };
        let event = seal(&metric, secret, owner, now)?;
        append(&self.store, &event)
    }

    /// The budget this request or today has used, in words.
    #[must_use]
    pub fn stop(&self) -> Option<String> {
        self.request
            .reached(
                self.budget.request_usd,
                self.budget.request_tokens,
                "request",
            )
            .or_else(|| {
                self.today
                    .reached(self.budget.daily_usd, self.budget.daily_tokens, "daily")
            })
    }
}

/// Her key and her owner's, when she can seal records.
fn keys(store: &Store, record: &Record) -> Result<(SecretKey, XOnlyPublicKey), String> {
    let owner = super::agent_engrams::owner_of(record)?
        .ok_or("she has no owner attestation, so her spend is not recorded")?;
    let secret = store
        .key()?
        .ok_or("she has no key, so her spend is not recorded")?;
    if record.pubkey.as_deref() != Some(agent::public_hex(&secret).as_str()) {
        return Err("her key does not match her record, so her spend is not recorded".into());
    }
    Ok((secret, owner))
}

// ------------------------------------------------------------------- sync

/// How far each relay has taken her ledger, beside her record.
pub const SENT_FILE: &str = "spend-sent.json";
/// Its schema.
pub const SENT_SCHEMA: &str = "openagents.agent-spend-sent.v1";

/// `spend-sent.json`: for each relay, how many ledger lines it has taken.
/// A relay serves a `kind:44200` only to the owner, so she can't ask it
/// what it holds; the ledger is append-only, so a count is exact.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sent {
    pub schema: String,
    pub v: u32,
    #[serde(default)]
    pub sent: std::collections::BTreeMap<String, usize>,
}

impl Sent {
    /// `store`'s counts; none when there is no file or it can't be read,
    /// which only sends records again.
    #[must_use]
    pub fn load(store: &Store) -> Self {
        std::fs::read_to_string(store.dir().join(SENT_FILE))
            .ok()
            .and_then(|text| serde_json::from_str::<Self>(&text).ok())
            .filter(|sent| sent.schema == SENT_SCHEMA && sent.v == 1)
            .unwrap_or_else(|| Self {
                schema: SENT_SCHEMA.into(),
                v: 1,
                sent: std::collections::BTreeMap::new(),
            })
    }

    /// # Errors
    /// When it can't be written.
    pub fn save(&self, store: &Store) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        agent::write_private(&store.dir().join(SENT_FILE), &body)
    }
}

/// Her ledger for a sync pass, in order: each record that opens with her
/// key, or `None` for a line that doesn't, which no relay is sent.
///
/// # Errors
/// When the ledger can't be read.
pub fn publishable(
    store: &Store,
    secret: &SecretKey,
    owner: &XOnlyPublicKey,
) -> Result<Vec<Option<Event>>, String> {
    Ok(ledger(store)?
        .into_iter()
        .map(|event| {
            open_as_agent(&event, secret, owner)
                .is_ok()
                .then_some(event)
        })
        .collect())
}

#[cfg(test)]
#[path = "agent_spend_tests.rs"]
mod tests;

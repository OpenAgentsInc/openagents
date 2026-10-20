//! System leases: scarce shared things a member takes with an expiry.
//!
//! A lease is a resource key (`port:41000`, `deploy-slot:staging`), its
//! holder and when it expires. A holder renews it while it works; a
//! finished member releases all of its leases; one that stops renewing
//! loses them when they expire, and one whose Coder process is gone loses
//! them at once.
//!
//! Deadlock is ruled out by order: named resources sort by kind, then name
//! ([`LeaseKind::rank`]), several are taken in one all-or-nothing call in
//! that order, and a member can't take a named resource that sorts before
//! one it already holds. A number or port handed out fresh ([`LeaseKind::
//! allocates`]) is new to everyone, so no one can be waiting on it, and it
//! is exempt.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::{Kind, Record, State};

/// How long a lease runs before it must be renewed, unless asked otherwise.
pub const DEFAULT_TTL_MS: u64 = 10 * 60 * 1000;

/// Where fresh ports are handed out from.
const PORT_FIRST: u16 = 41_000;
const PORT_LAST: u16 = 41_999;

/// The kinds of things leased.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LeaseKind {
    /// The shared cargo build folder, or its lock.
    BuildFolder,
    /// A build host: CoderOS, this Mac.
    BuildHost,
    /// A staging deploy slot.
    DeploySlot,
    /// A database migration number.
    Migration,
    /// A local port.
    Port,
}

impl LeaseKind {
    pub const ALL: [LeaseKind; 5] = [
        Self::BuildFolder,
        Self::BuildHost,
        Self::DeploySlot,
        Self::Migration,
        Self::Port,
    ];

    /// Reads `build-folder`, `build-host`, `deploy-slot`, `migration` or
    /// `port`.
    ///
    /// # Errors
    /// Any other word.
    pub fn parse(word: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == word)
            .ok_or_else(|| {
                "A lease is build-folder, build-host, deploy-slot, migration or port.".into()
            })
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BuildFolder => "build-folder",
            Self::BuildHost => "build-host",
            Self::DeploySlot => "deploy-slot",
            Self::Migration => "migration",
            Self::Port => "port",
        }
    }

    /// Where it sorts among kinds.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::BuildFolder => 0,
            Self::BuildHost => 1,
            Self::DeploySlot => 2,
            Self::Migration => 3,
            Self::Port => 4,
        }
    }

    /// Whether a request without a name gets a fresh value.
    #[must_use]
    pub const fn allocates(self) -> bool {
        matches!(self, Self::Migration | Self::Port)
    }

    /// The name used when a request gives none, for kinds with one.
    fn default_name(self) -> Option<String> {
        match self {
            Self::BuildFolder => Some(
                std::env::var("CARGO_TARGET_DIR")
                    .ok()
                    .filter(|dir| !dir.is_empty())
                    .unwrap_or_else(|| "cargo".into()),
            ),
            Self::BuildHost => Some("this computer".into()),
            Self::DeploySlot => Some("staging".into()),
            Self::Migration | Self::Port => None,
        }
    }
}

/// One thing asked for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LeaseRequest {
    pub kind: LeaseKind,
    /// The folder, host, slot, number or port; `None` for the default, or a
    /// fresh number or port.
    pub name: Option<String>,
}

impl LeaseRequest {
    #[must_use]
    pub fn new(kind: LeaseKind, name: Option<&str>) -> Self {
        Self {
            kind,
            name: name
                .map(str::trim)
                .filter(|name| !name.is_empty())
                .map(str::to_owned),
        }
    }

    /// Reads `KIND` or `KIND:NAME`.
    ///
    /// # Errors
    /// The kind is unknown.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (kind, name) = match text.split_once(':') {
            Some((kind, name)) => (kind, Some(name)),
            None => (text, None),
        };
        Ok(Self::new(LeaseKind::parse(kind.trim())?, name))
    }

    /// The name it stands for, `None` when it gets a fresh value.
    fn resolved(&self) -> Option<String> {
        self.name.clone().or_else(|| self.kind.default_name())
    }
}

/// A held lease.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lease {
    /// `kind:value`, the key leases are found by.
    pub resource: String,
    pub kind: LeaseKind,
    pub value: String,
    pub holder: String,
    pub acquired_ms: u64,
    pub expires_ms: u64,
    #[serde(default)]
    pub renewals: u32,
    /// Handed out fresh rather than asked for by name.
    #[serde(default)]
    pub allocated: bool,
}

/// The key of `kind` named `value`.
#[must_use]
pub fn key(kind: LeaseKind, value: &str) -> String {
    format!("{}:{value}", kind.as_str())
}

/// Where a named resource sorts.
fn order(kind: LeaseKind, value: &str) -> (u8, String) {
    (kind.rank(), value.to_owned())
}

/// Why a lease was not taken.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Another member holds it.
    Held {
        resource: String,
        holder: String,
        expires_ms: u64,
    },
    /// Taking it would break the order: release `held` first, or ask for
    /// both in one call.
    Order { resource: String, held: String },
    /// The request can't be met as asked.
    Invalid(String),
    /// The channel couldn't be used.
    Channel(String),
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Held {
                resource,
                holder,
                expires_ms,
            } => {
                let left = expires_ms.saturating_sub(super::now_ms()) / 1000;
                write!(
                    f,
                    "{holder} holds {resource} for up to {left} more seconds. Ask them on the channel when they'll be done, wait, or work on something else."
                )
            }
            Self::Order { resource, held } => write!(
                f,
                "{resource} comes before {held}, which you hold. Release {held} first, or ask for both in one call."
            ),
            Self::Invalid(why) | Self::Channel(why) => f.write_str(why),
        }
    }
}

impl std::error::Error for Refusal {}

/// Takes every request for `me`, or none (see the module docs).
///
/// # Errors
/// [`Refusal`].
pub(super) fn acquire(
    state: &mut State,
    records: &mut Vec<Record>,
    me: &str,
    requests: &[LeaseRequest],
    ttl_ms: u64,
    now: u64,
) -> Result<Vec<Lease>, Refusal> {
    if requests.is_empty() {
        return Err(Refusal::Invalid("Name at least one lease.".into()));
    }
    if ttl_ms < 1000 {
        return Err(Refusal::Invalid("A lease runs at least one second.".into()));
    }
    let mut named: Vec<(LeaseKind, String)> = Vec::new();
    let mut fresh: Vec<&LeaseRequest> = Vec::new();
    for request in requests {
        match request.resolved() {
            Some(value) => {
                if request.kind == LeaseKind::Port && value.parse::<u16>().is_err() {
                    return Err(Refusal::Invalid(format!("{value} is not a port.")));
                }
                if request.kind == LeaseKind::Migration && value.parse::<u64>().is_err() {
                    return Err(Refusal::Invalid(format!(
                        "{value} is not a migration number."
                    )));
                }
                named.push((request.kind, value));
            }
            None => fresh.push(request),
        }
    }
    named.sort_by_key(|(kind, value)| order(*kind, value));
    named.dedup();
    // The order: nothing named may sort before a named lease already held,
    // apart from the ones being renewed in this call.
    let keys: Vec<String> = named
        .iter()
        .map(|(kind, value)| key(*kind, value))
        .collect();
    let held_max = state
        .leases
        .values()
        .filter(|lease| lease.holder == me && !lease.allocated && !keys.contains(&lease.resource))
        .map(|lease| (order(lease.kind, &lease.value), lease.resource.clone()))
        .max();
    if let Some((held_order, held)) = held_max {
        if let Some((kind, value)) = named
            .iter()
            .find(|(kind, value)| order(*kind, value) < held_order)
        {
            return Err(Refusal::Order {
                resource: key(*kind, value),
                held,
            });
        }
    }
    for resource in &keys {
        if let Some(lease) = state.leases.get(resource) {
            if lease.holder != me {
                return Err(Refusal::Held {
                    resource: resource.clone(),
                    holder: lease.holder.clone(),
                    expires_ms: lease.expires_ms,
                });
            }
        }
    }
    let mut taken = Vec::new();
    for (kind, value) in named {
        let resource = key(kind, &value);
        if kind == LeaseKind::Migration {
            state.migration = state.migration.max(value.parse().unwrap_or(0));
        }
        let lease = match state.leases.get_mut(&resource) {
            Some(lease) => {
                lease.expires_ms = now + ttl_ms;
                lease.renewals += 1;
                lease.clone()
            }
            None => {
                let lease = Lease {
                    resource: resource.clone(),
                    kind,
                    value,
                    holder: me.to_owned(),
                    acquired_ms: now,
                    expires_ms: now + ttl_ms,
                    renewals: 0,
                    allocated: false,
                };
                state.leases.insert(resource, lease.clone());
                records.push(taken_record(&lease, ttl_ms, now));
                lease
            }
        };
        taken.push(lease);
    }
    for request in fresh {
        let value = match request.kind {
            LeaseKind::Migration => {
                let held = state
                    .leases
                    .values()
                    .filter(|lease| lease.kind == LeaseKind::Migration)
                    .filter_map(|lease| lease.value.parse::<u64>().ok())
                    .max()
                    .unwrap_or(0);
                let next = state.migration.max(held) + 1;
                state.migration = next;
                format!("{next:04}")
            }
            LeaseKind::Port => {
                let port = (PORT_FIRST..=PORT_LAST)
                    .find(|port| {
                        !state
                            .leases
                            .contains_key(&key(LeaseKind::Port, &port.to_string()))
                            && std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, *port))
                                .is_ok()
                    })
                    .ok_or_else(|| {
                        Refusal::Invalid("No port is free between 41000 and 41999.".into())
                    })?;
                port.to_string()
            }
            _ => unreachable!("only migrations and ports are handed out fresh"),
        };
        let lease = Lease {
            resource: key(request.kind, &value),
            kind: request.kind,
            value,
            holder: me.to_owned(),
            acquired_ms: now,
            expires_ms: now + ttl_ms,
            renewals: 0,
            allocated: true,
        };
        state.leases.insert(lease.resource.clone(), lease.clone());
        records.push(taken_record(&lease, ttl_ms, now));
        taken.push(lease);
    }
    Ok(taken)
}

fn taken_record(lease: &Lease, ttl_ms: u64, now: u64) -> Record {
    let minutes = ttl_ms.div_ceil(60_000);
    let mut record = Record::new(
        &lease.holder,
        Kind::Lease,
        format!(
            "{} took {} for {minutes} minute{} (renewed while held).",
            lease.holder,
            lease.resource,
            if minutes == 1 { "" } else { "s" }
        ),
    );
    record.at_ms = now;
    record.data = json!({"event":"taken","lease":lease});
    record
}

/// Finds `me`'s lease by key, or by its value alone when that is unique.
fn find<'a>(state: &'a State, me: &str, resource: &str) -> Option<&'a String> {
    if let Some((key, _)) = state.leases.get_key_value(resource) {
        return Some(key);
    }
    let mut mine = state
        .leases
        .iter()
        .filter(|(_, lease)| lease.holder == me && lease.value == resource);
    let found = mine.next().map(|(key, _)| key);
    if mine.next().is_some() { None } else { found }
}

pub(super) fn renew(
    state: &mut State,
    me: &str,
    resource: &str,
    ttl_ms: u64,
    now: u64,
) -> Result<Lease, String> {
    let key = find(state, me, resource).cloned().ok_or_else(|| {
        format!("You hold no lease on {resource}; it may have expired. Take it again.")
    })?;
    let lease = state.leases.get_mut(&key).expect("found above");
    if lease.holder != me {
        return Err(format!("{} holds {key}, not you.", lease.holder));
    }
    lease.expires_ms = now + ttl_ms.max(1000);
    lease.renewals += 1;
    Ok(lease.clone())
}

pub(super) fn release(
    state: &mut State,
    records: &mut Vec<Record>,
    me: &str,
    resource: &str,
) -> bool {
    let Some(key) = find(state, me, resource).cloned() else {
        return false;
    };
    if state
        .leases
        .get(&key)
        .is_none_or(|lease| lease.holder != me)
    {
        return false;
    }
    let lease = state.leases.remove(&key).expect("present");
    let mut record = Record::new(
        me,
        Kind::Lease,
        format!("{me} released {}.", lease.resource),
    );
    record.data = json!({"event":"released","lease":lease});
    records.push(record);
    true
}

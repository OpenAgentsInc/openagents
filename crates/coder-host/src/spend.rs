//! Agent spend requests on this host: phase 1 of agent spending, where an
//! agent asks and the owner approves each payment on the phone
//! (`docs/breez/spend-protocol.md`).
//!
//! An agent (a Coder task's command, or `openagents x402` paying through the
//! phone) records a request here with [`Book::request`] and waits for the
//! phone's receipt with [`Book::wait`]. The phone reads the requests with
//! NIP-HOST `spend.list`, which also hands the host the phone's current spend
//! grant, and answers each with `spend.settle`. Nothing here pays: the phone's
//! wallet does, after the owner's tap.
//!
//! The book is one private JSON file beside the access store
//! (`~/.openagents/coder-access/spend.json`), written whole under a file
//! lock, so the host process and a local command share it.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use coder_access::Code;
use coder_access::spend::{
    Context, Entry, Grant, MAX_LISTED, MAX_REQUEST_LIFETIME, Purpose, REQUEST, Receipt, Refusal,
    SpendRequest,
};
use serde::{Deserialize, Serialize};

const FILE: &str = "spend.json";
const LOCK: &str = "spend.lock";
const VERSION: &str = "coder-host.spend.v1";
/// The most requests the book holds.
const MAX_ENTRIES: usize = 256;
/// Answered requests are kept this long for `coder host spend list`.
const RETENTION: u64 = 7 * 24 * 60 * 60;
/// A request's default lifetime when the asker names none.
pub const DEFAULT_TTL: u64 = 10 * 60;
const MAX_BOOK: u64 = 16 * 1024 * 1024;

/// What an agent asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    /// The BOLT11 invoice to pay.
    pub payment: String,
    /// The most fee the asker accepts, in msat. The request carries the
    /// lower of it and the grant's ceiling for the amount; `None` takes the
    /// grant's.
    pub fee_max_msat: Option<u64>,
    pub purpose: Purpose,
    pub context: Context,
    /// Seconds the request stays open, at most an hour and never past the
    /// invoice's expiry.
    pub ttl: u64,
    /// The request ID. `None` derives it from the invoice, so asking again
    /// for the same invoice is the same request.
    pub id: Option<String>,
}

/// Why the host would not record a request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No phone has given this host a current spend grant.
    NoGrant,
    /// The request fails a check the grant states.
    Refusal(Refusal),
    /// The same ID was used for a different payment.
    Conflict,
    /// The book could not be read or written, or is full.
    Store(String),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGrant => f.write_str(
                "no phone has given this computer a spend grant; open the OpenAgents app on the phone with this computer connected, then ask again",
            ),
            Self::Refusal(code) => write!(f, "{}", code.describe()),
            Self::Conflict => f.write_str("that request ID names a different payment"),
            Self::Store(message) => write!(f, "the spend book is unavailable: {message}"),
        }
    }
}

impl std::error::Error for Refused {}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Stored {
    request: SpendRequest,
    /// The device whose grant the request draws on.
    issuer: String,
    receipt: Option<Receipt>,
    /// Receipts the phone did not sign: a request that expired unanswered.
    #[serde(default)]
    host_refused: bool,
    /// A spend wake went out for it ([`wake`]).
    #[serde(default)]
    woken: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    v: String,
    /// The current grant from each issuing device.
    grants: BTreeMap<String, Grant>,
    entries: BTreeMap<String, Stored>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            v: VERSION.into(),
            grants: BTreeMap::new(),
            entries: BTreeMap::new(),
        }
    }
}

impl State {
    /// Close requests that expired unanswered as `phone_unreachable`, and
    /// forget answered ones past retention.
    fn expire(&mut self, now: u64) {
        for stored in self.entries.values_mut() {
            if stored.receipt.is_none() && now >= stored.request.expires_at {
                stored.receipt = Some(Receipt::refused(
                    &stored.request.request,
                    &stored.request.grant,
                    Refusal::PhoneUnreachable,
                    now,
                ));
                stored.host_refused = true;
            }
        }
        self.entries.retain(|_, stored| {
            stored.receipt.as_ref().is_none_or(|r| !r.is_final())
                || stored.request.expires_at.saturating_add(RETENTION) > now
        });
    }
}

/// The host's spend requests.
#[derive(Clone, Debug)]
pub struct Book {
    directory: PathBuf,
}

impl Book {
    /// The book beside the access store at `access`.
    #[must_use]
    pub fn open(access: &Path) -> Self {
        Self {
            directory: access.to_path_buf(),
        }
    }

    /// Run `change` on the book under its lock, saving when it returns
    /// `true` beside its value.
    fn with<T>(
        &self,
        change: impl FnOnce(&mut State) -> Result<(T, bool), Refused>,
    ) -> Result<T, Refused> {
        let store = |error: std::io::Error| Refused::Store(error.kind().to_string());
        std::fs::create_dir_all(&self.directory).map_err(store)?;
        let lock = private_options(
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false),
        )
        .open(self.directory.join(LOCK))
        .map_err(store)?;
        lock.lock().map_err(store)?;
        let path = self.directory.join(FILE);
        let mut state = match File::open(&path) {
            Ok(file) => {
                use std::io::Read;
                let mut bytes = Vec::new();
                file.take(MAX_BOOK + 1)
                    .read_to_end(&mut bytes)
                    .map_err(store)?;
                if bytes.len() as u64 > MAX_BOOK {
                    return Err(Refused::Store("the book exceeds its bound".into()));
                }
                let state: State = serde_json::from_slice(&bytes)
                    .map_err(|_| Refused::Store("the book is malformed".into()))?;
                if state.v != VERSION {
                    return Err(Refused::Store("the book has another version".into()));
                }
                state
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(error) => return Err(store(error)),
        };
        let (value, save) = change(&mut state)?;
        if save {
            let bytes = serde_json::to_vec(&state)
                .map_err(|_| Refused::Store("the book could not be written".into()))?;
            let pending = self.directory.join(".spend.pending");
            let mut file =
                private_options(OpenOptions::new().write(true).create(true).truncate(true))
                    .open(&pending)
                    .map_err(store)?;
            file.write_all(&bytes)
                .and_then(|()| file.sync_all())
                .map_err(store)?;
            std::fs::rename(&pending, &path).map_err(store)?;
        }
        drop(lock);
        Ok(value)
    }

    /// Record a request from this host (`host`) under the newest current
    /// grant a phone gave it. Checks what the grant states; the phone checks
    /// again, with its ledger, before it asks the owner. Asking again for the
    /// same ID and invoice returns the recorded request.
    pub fn request(&self, host: &str, ask: &Ask, now: u64) -> Result<SpendRequest, Refused> {
        let id = ask
            .id
            .clone()
            .unwrap_or_else(|| SpendRequest::id_for(&ask.payment));
        self.with(|state| {
            state.expire(now);
            if let Some(stored) = state.entries.get(&id) {
                return if stored.request.payment == ask.payment.trim()
                    && stored.request.purpose == ask.purpose
                {
                    Ok((stored.request.clone(), true))
                } else {
                    Err(Refused::Conflict)
                };
            }
            let grant = state
                .grants
                .values()
                .filter(|g| g.grantee == host && g.expires_at > now)
                .max_by_key(|g| g.issued_at)
                .cloned()
                .ok_or(Refused::NoGrant)?;
            let mut request = SpendRequest {
                v: REQUEST.into(),
                requires: vec![],
                request: id.clone(),
                grant: grant.grant.clone(),
                epoch: grant.epoch,
                grantee: host.into(),
                payment: ask.payment.trim().into(),
                amount_msat: 0,
                fee_max_msat: 0,
                purpose: ask.purpose,
                context: ask.context.clone(),
                issued_at: now,
                expires_at: now,
            };
            let invoice = request.invoice().map_err(|error| {
                Refused::Refusal(if error.code == Code::Unsupported {
                    Refusal::RailNotAllowed
                } else {
                    Refusal::Malformed
                })
            })?;
            request.amount_msat = invoice.amount_msat;
            // The lower fee ceiling wins: the asker's or the grant's.
            let ceiling = grant.fee_max.ceiling(request.amount_msat);
            request.fee_max_msat = ask.fee_max_msat.map_or(ceiling, |asked| asked.min(ceiling));
            request.expires_at = now
                .saturating_add(ask.ttl.clamp(1, MAX_REQUEST_LIFETIME))
                .min(invoice.expires_at())
                .min(grant.expires_at);
            if request.expires_at <= now {
                return Err(Refused::Refusal(Refusal::Expired));
            }
            request
                .validate()
                .map_err(|_| Refused::Refusal(Refusal::Malformed))?;
            if !grant.purposes.contains(&request.purpose) {
                return Err(Refused::Refusal(Refusal::PurposeNotAllowed));
            }
            if request.amount_msat.saturating_add(request.fee_max_msat) > grant.per_payment_max {
                return Err(Refused::Refusal(Refusal::OverPaymentCap));
            }
            if state.entries.len() >= MAX_ENTRIES {
                return Err(Refused::Store("too many open requests".into()));
            }
            state.entries.insert(
                id.clone(),
                Stored {
                    request: request.clone(),
                    issuer: grant.issuer.clone(),
                    receipt: None,
                    host_refused: false,
                    woken: false,
                },
            );
            Ok((request, true))
        })
    }

    /// The receipt recorded for `request`, if any.
    pub fn receipt(&self, request: &str, now: u64) -> Result<Option<Receipt>, Refused> {
        self.with(|state| {
            state.expire(now);
            let receipt = state.entries.get(request).and_then(|s| s.receipt.clone());
            Ok((receipt, true))
        })
    }

    /// Wait until `request` has a final receipt, or it expires (it is then
    /// refused as `phone_unreachable`), or `limit` passes. Returns the last
    /// receipt seen, `None` when there was none by `limit`.
    pub fn wait(
        &self,
        request: &str,
        limit: Duration,
        clock: impl Fn() -> u64,
    ) -> Result<Option<Receipt>, Refused> {
        let deadline = std::time::Instant::now() + limit;
        loop {
            let receipt = self.receipt(request, clock())?;
            if receipt.as_ref().is_some_and(Receipt::is_final)
                || std::time::Instant::now() >= deadline
            {
                return Ok(receipt);
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Every request the book holds, newest first, for the host's owner.
    pub fn entries(&self, now: u64) -> Result<Vec<Entry>, Refused> {
        self.with(|state| {
            state.expire(now);
            let mut entries: Vec<&Stored> = state.entries.values().collect();
            entries.sort_by_key(|s| std::cmp::Reverse(s.request.issued_at));
            let listed = entries
                .into_iter()
                .map(|s| Entry {
                    request: s.request.clone(),
                    receipt: s.receipt.clone(),
                })
                .collect();
            Ok((listed, true))
        })
    }

    /// The devices to wake: each that holds an open request no wake went
    /// out for yet. Marks those requests woken, so each request wakes its
    /// phone once; the phone's `spend.list` reads every open one.
    pub fn wakes(&self, now: u64) -> Result<Vec<String>, Refused> {
        self.with(|state| {
            state.expire(now);
            let mut devices = std::collections::BTreeSet::new();
            for stored in state.entries.values_mut() {
                if stored.receipt.is_none() && !stored.woken {
                    stored.woken = true;
                    devices.insert(stored.issuer.clone());
                }
            }
            let save = !devices.is_empty();
            Ok((devices.into_iter().collect(), save))
        })
    }

    /// Whether the book has changed since `seen` (its file's modification
    /// time and length), so the host reads it only when it moved.
    #[must_use]
    pub fn stamp(&self) -> Option<(std::time::SystemTime, u64)> {
        let metadata = std::fs::metadata(self.directory.join(FILE)).ok()?;
        Some((metadata.modified().ok()?, metadata.len()))
    }

    fn list_for(&self, device: &str, grant: &Grant, now: u64) -> Result<Vec<Entry>, Code> {
        self.with(|state| {
            state.expire(now);
            if let Some(held) = state.grants.get(device) {
                if grant.epoch < held.epoch {
                    return Err(Refused::Refusal(Refusal::Stale));
                }
                // A newer epoch revoked the old grant: what waits under an
                // earlier epoch will never be paid.
                if grant.epoch > held.epoch {
                    for stored in state.entries.values_mut() {
                        if stored.issuer == device
                            && stored.request.epoch < grant.epoch
                            && stored.receipt.is_none()
                        {
                            stored.receipt = Some(Receipt::refused(
                                &stored.request.request,
                                &stored.request.grant,
                                Refusal::Stale,
                                now,
                            ));
                            stored.host_refused = true;
                        }
                    }
                }
            }
            // A grant with no life left revokes: the phone sends one at
            // the next epoch when the owner stops this computer's requests.
            if grant.expires_at > now {
                state.grants.insert(device.into(), grant.clone());
            } else {
                state.grants.remove(device);
            }
            let mut open: Vec<&Stored> = state
                .entries
                .values()
                .filter(|s| s.issuer == device)
                .filter(|s| s.receipt.as_ref().is_none_or(|r| !r.is_final()))
                .collect();
            open.sort_by_key(|s| s.request.issued_at);
            let listed = open
                .into_iter()
                .take(MAX_LISTED)
                .map(|s| Entry {
                    request: s.request.clone(),
                    receipt: s.receipt.clone(),
                })
                .collect();
            Ok((listed, true))
        })
        .map_err(|refused| match refused {
            Refused::Refusal(Refusal::Stale) => Code::Stale,
            _ => Code::Unavailable,
        })
    }

    fn settle_for(&self, device: &str, receipt: &Receipt, now: u64) -> Result<Receipt, Code> {
        let mut failure = Code::Unavailable;
        self.with(|state| {
            state.expire(now);
            let Some(stored) = state.entries.get_mut(&receipt.request) else {
                failure = Code::Forbidden;
                return Err(Refused::Conflict);
            };
            if stored.issuer != device || receipt.answers(&stored.request).is_err() {
                failure = Code::Forbidden;
                return Err(Refused::Conflict);
            }
            match &stored.receipt {
                Some(recorded) if recorded == receipt => return Ok((recorded.clone(), false)),
                // The phone's own answer replaces the host's expiry or
                // staleness refusal: a proven payment means the money moved.
                Some(recorded) if recorded.is_final() && !stored.host_refused => {
                    failure = Code::Conflict;
                    return Err(Refused::Conflict);
                }
                _ => {}
            }
            stored.receipt = Some(receipt.clone());
            stored.host_refused = false;
            Ok((receipt.clone(), true))
        })
        .map_err(|_| failure)
    }
}

impl coder_access::host::Spends for Book {
    fn list(&mut self, device: &str, grant: &Grant, now: u64) -> Result<Vec<Entry>, Code> {
        self.list_for(device, grant, now)
    }

    fn settle(&mut self, device: &str, receipt: &Receipt, now: u64) -> Result<Receipt, Code> {
        self.settle_for(device, receipt, now)
    }
}

pub mod cli;
/// A new file here is `0600`; on Windows it inherits the host root's
/// owner-only DACL.
fn private_options(options: &mut OpenOptions) -> &mut OpenOptions {
    #[cfg(unix)]
    options.mode(0o600);
    options
}

#[cfg(test)]
mod tests;
pub mod wake;

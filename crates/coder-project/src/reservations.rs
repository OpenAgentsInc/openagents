//! The reservation book: how the coordinator stops overscheduling the
//! one machine.
//!
//! A [`Book`] answers *may this task proceed* with names and bounds. A
//! request states its holder and its needs; the book grants only what
//! is free. Executor places, CPU units, and memory are counted lanes —
//! each grant sums against the host's stated ceilings. The quiet-host
//! and integration lanes admit one holder at a time, and a refusal says
//! exactly who holds the lane or which ceiling the request would cross.
//!
//! # Guarantees, not isolation
//!
//! A reservation is an admission guarantee, not OS isolation: the book
//! bounds *how many* tasks may proceed, never *what* a task may do once
//! it runs. Enforcement belongs to the execution boundary. A request
//! that asks for hard OS isolation the host cannot enforce — CPU
//! pinning, a kernel memory limit — is a typed [`Refusal::Unsupported`],
//! never a grant that pretends.
//!
//! # Unknown means conservative
//!
//! Requirements the caller did not state are filled, not zeroed: an
//! unstated memory need reserves the book's stated conservative share,
//! because a task that does not say what it needs is cheaper to
//! overestimate than to undercount. [`Book::reconcile`] frees a grant
//! only on a positive report that its holder is gone; a holder whose
//! state cannot be stated marks its grant [`State::Unknown`], where it
//! keeps counting until an operator reconciles it. A slow holder is a
//! live holder — the book never frees on slowness.
//!
//! # Determinism
//!
//! The book reads no clock and keeps no unordered state: the same book
//! plus the same requests yields the same grants, in the same order,
//! every time. Timestamps on grants and transitions are supplied by the
//! caller, and grant ids are minted in request order.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The typed reservation classes a request can hold.
///
/// `Executor`, `CpuBuild`, and `Memory` are counted lanes — grants sum
/// against configured ceilings. `QuietHost` and `Integration` are
/// exclusive lanes — one holder at a time, and a second request refuses
/// by name.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lane {
    /// A delegated-agent place — the lane bounded agent work holds.
    Executor,
    /// Compile and test capacity, in CPU units.
    CpuBuild,
    /// A memory reservation, in MiB.
    Memory,
    /// The exclusive measurement lane — one holder at a time, tagged
    /// with the accelerator the measurement pins.
    QuietHost,
    /// The serial lane — merges and pushes, strictly one holder.
    Integration,
}

impl Lane {
    /// The lane's name in explanations.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Executor => "executor",
            Self::CpuBuild => "cpu-build",
            Self::Memory => "memory",
            Self::QuietHost => "quiet-host",
            Self::Integration => "integration",
        }
    }
}

impl std::fmt::Display for Lane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.name())
    }
}

/// The hard OS isolation a request asks for.
///
/// The book grants admission, not enforcement: every variant past
/// `Admission` is a [`Refusal::Unsupported`], stated rather than
/// approximated.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Isolation {
    /// Admission bounds only — the reservation is a count, not a cage.
    #[default]
    Admission,
    /// Pin the holder to named host CPUs.
    CpuPinning,
    /// A kernel-enforced memory ceiling.
    MemoryLimit,
}

impl Isolation {
    /// The isolation's name in explanations.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Admission => "admission",
            Self::CpuPinning => "cpu-pinning",
            Self::MemoryLimit => "memory-limit",
        }
    }
}

/// What the host offers — the counted lanes' ceilings.
///
/// The exclusive lanes have no configured count: the quiet host and the
/// integration lane are one each by construction, because a second
/// concurrent holder is never safe to admit.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Ceilings {
    /// Concurrent executor places.
    pub executor_slots: u32,
    /// Total build CPU units.
    pub cpu_units: u32,
    /// Total reservable memory, in MiB.
    pub memory_mib: u64,
}

/// What an unstated need reserves — conservative, never zero.
///
/// A task that does not say what it needs is assumed to need a stated
/// share, so silence cannot smuggle a free ride through the book. The
/// defaults are a full build's appetite: one executor place, a build's
/// CPU units, and a build's working set. A host states its own shares
/// through [`Book::with_fills`].
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Fills {
    /// Executor places an unstated request holds.
    pub executor_slots: u32,
    /// CPU units an unstated request holds.
    pub cpu_units: u32,
    /// Memory an unstated request holds, in MiB.
    pub memory_mib: u64,
}

impl Default for Fills {
    fn default() -> Self {
        Self {
            executor_slots: 1,
            cpu_units: 4,
            memory_mib: 4096,
        }
    }
}

/// What a task states it needs — the request `Book::request` reads.
///
/// `Option` fields are the stated requirements: `None` means the caller
/// did not say, and the book fills the conservative share rather than
/// zero. `at_unix` is the request's timestamp — the book reads no
/// clock, so the caller supplies when the request is made.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Needs {
    /// Who would hold the grant — a task or run id, unique per live
    /// grant.
    pub holder: String,
    /// Executor places; unstated fills to the conservative share.
    #[serde(default)]
    pub executor_slots: Option<u32>,
    /// Build CPU units; unstated fills to the conservative share.
    #[serde(default)]
    pub cpu_units: Option<u32>,
    /// Memory in MiB; unstated fills to the conservative share.
    #[serde(default)]
    pub memory_mib: Option<u64>,
    /// The quiet-host lane for the named accelerator — `Some(tag)`
    /// asks for exclusivity of the host, `None` does not ask.
    #[serde(default)]
    pub quiet_accelerator: Option<String>,
    /// Holds the serial integration lane — merges and pushes.
    #[serde(default)]
    pub integration: bool,
    /// The hard OS isolation asked for. Anything past
    /// [`Isolation::Admission`] refuses as [`Refusal::Unsupported`].
    #[serde(default)]
    pub isolation: Isolation,
    /// The grant's stated lifetime in seconds; `Book::expire` ends it
    /// at `granted_unix + for_seconds`. `None` holds until released or
    /// reconciled.
    #[serde(default)]
    pub for_seconds: Option<u64>,
    /// When the request is made — supplied, never read from a clock.
    pub at_unix: u64,
}

/// The lanes and quantities a grant holds — the request's stated needs
/// with every unstated requirement filled conservatively.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Requirements {
    /// Executor places held.
    pub executor_slots: u32,
    /// Build CPU units held.
    pub cpu_units: u32,
    /// Memory held, in MiB.
    pub memory_mib: u64,
    /// The quiet lane's accelerator tag while held.
    pub quiet_accelerator: Option<String>,
    /// The serial integration lane while held.
    pub integration: bool,
    /// The stated lifetime in seconds, when stated.
    pub for_seconds: Option<u64>,
}

/// Where a grant stands.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum State {
    /// Counted — the holder may proceed.
    Held,
    /// Reconcile could not account for the holder. The grant still
    /// counts — `unknown` frees nothing — until an operator reconciles
    /// it or the holder is observed live again.
    Unknown,
    /// The grant ended; the record says how and when.
    Ended(End),
}

/// How a grant stopped counting.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum End {
    /// The holder presented its grant back.
    Released { at_unix: u64 },
    /// The stated lifetime ran out under [`Book::expire`].
    Expired { at_unix: u64 },
    /// Reconcile proved the holder gone — a dead task's reservation
    /// leaks nothing.
    Reclaimed { at_unix: u64 },
}

/// The durable record of one grant: who holds it, which lanes it
/// holds, the filled requirements it was counted under, when it was
/// granted, and its release or expire transition.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Reservation {
    /// The grant's id — `r-000001`, minted in request order.
    pub id: String,
    /// Who holds the grant — the task or run id the request named.
    pub holder: String,
    /// What the book counted — the filled requirements, never the raw
    /// request.
    pub held: Requirements,
    /// When the grant was made — supplied by the caller, never read
    /// from a clock.
    pub granted_unix: u64,
    /// Where the grant stands.
    pub state: State,
}

impl Reservation {
    /// Does the grant still count against the book — held or unknown?
    /// An unknown grant counts because its holder may still be running
    /// somewhere the observation cannot see.
    #[must_use]
    pub fn counts(&self) -> bool {
        matches!(self.state, State::Held | State::Unknown)
    }
}

/// What `Book::request` hands back — the holder's proof of admission.
///
/// The grant carries the filled requirements, so the holder sees the
/// conservative shares it was counted under, not merely what it
/// stated. Present the grant's id to [`Book::release`] when the work
/// is done.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Grant {
    /// The reservation's id.
    pub id: String,
    /// Who holds it.
    pub holder: String,
    /// The filled requirements the book counted.
    pub held: Requirements,
    /// When it was granted — the request's supplied time.
    pub granted_unix: u64,
}

/// Why the book did not grant, or a transition did not apply.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Refusal {
    /// The request is malformed — no usable holder, a quiet request
    /// that does not name its accelerator, or a zero lifetime.
    Malformed {
        /// What failed validation.
        reason: String,
    },
    /// The host cannot enforce the requested OS isolation — the book
    /// refuses rather than grant a promise it cannot keep.
    Unsupported {
        /// The isolation asked for.
        requested: Isolation,
    },
    /// The holder already holds a live grant — one task, one grant.
    Held {
        /// The holder that asked twice.
        holder: String,
        /// The live grant it already holds.
        grant: String,
    },
    /// An exclusive lane is occupied — the holder is named.
    LaneHeld {
        /// The occupied lane.
        lane: Lane,
        /// Who holds it.
        holder: String,
    },
    /// The request needs the quiet lane and the host is occupied — the
    /// quiet lane admits only onto an empty host.
    QuietHostBusy {
        /// The live holders occupying the host.
        holders: Vec<String>,
    },
    /// The request would push a counted lane past its ceiling.
    Ceiling {
        /// The lane that does not fit.
        lane: Lane,
        /// What the lane already holds.
        held: u64,
        /// What the request asked, after conservative fills.
        requested: u64,
        /// The lane's configured ceiling.
        ceiling: u64,
    },
    /// No grant carries the id — transitions name real grants or
    /// refuse.
    UnknownGrant {
        /// The id that names nothing.
        grant: String,
    },
    /// The grant already ended — a second release changes nothing.
    AlreadyEnded {
        /// The ended grant's id.
        grant: String,
    },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed { reason } => write!(f, "malformed request: {reason}"),
            Self::Unsupported { requested } => write!(
                f,
                "the host cannot enforce {requested} — refused, not approximated",
                requested = requested.name()
            ),
            Self::Held { holder, grant } => {
                write!(f, "`{holder}` already holds grant `{grant}`")
            }
            Self::LaneHeld { lane, holder } => {
                write!(f, "the {lane} lane is held by `{holder}`")
            }
            Self::QuietHostBusy { holders } => write!(
                f,
                "the quiet lane admits only onto an empty host — held by {}",
                holders.join(", ")
            ),
            Self::Ceiling {
                lane,
                held,
                requested,
                ceiling,
            } => write!(
                f,
                "the {lane} lane holds {held} of {ceiling} — {requested} does not fit"
            ),
            Self::UnknownGrant { grant } => write!(f, "no grant carries `{grant}`"),
            Self::AlreadyEnded { grant } => write!(f, "grant `{grant}` already ended"),
        }
    }
}

impl std::error::Error for Refusal {}

/// What the caller observed about a holder when reconciling.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Liveness {
    /// Running — a slow holder counts as live and keeps its grant.
    Live,
    /// Provably gone — the reservation frees.
    Dead,
    /// The caller could not tell — the grant marks `unknown` and keeps
    /// counting.
    Ambiguous,
}

/// What one [`Book::reconcile`] pass changed.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Reconciliation {
    /// Grants freed — their holders were reported dead.
    pub freed: Vec<String>,
    /// Grants kept — their holders are live, however slow.
    pub kept: Vec<String>,
    /// Grants marked unknown — the observation could not account for
    /// the holder. They still count.
    pub unknown: Vec<String>,
}

/// The single-writer reservation book.
///
/// One writer — the coordinator — mutates the book, so request order is
/// grant order and two requests never race a lane. The book bounds how
/// many tasks may proceed; it is an admission guarantee, not OS
/// isolation, and what it cannot enforce it refuses rather than
/// approximates.
pub struct Book {
    ceilings: Ceilings,
    fills: Fills,
    next: u64,
    grants: BTreeMap<String, Reservation>,
}

impl Book {
    /// Open a book over the host's stated ceilings, with the
    /// conservative default fills for unstated needs.
    #[must_use]
    pub fn new(ceilings: Ceilings) -> Self {
        Self::with_fills(ceilings, Fills::default())
    }

    /// Open a book with operator-stated fills for unstated needs.
    #[must_use]
    pub fn with_fills(ceilings: Ceilings, fills: Fills) -> Self {
        Self {
            ceilings,
            fills,
            next: 0,
            grants: BTreeMap::new(),
        }
    }

    /// The ceilings the book was opened with.
    #[must_use]
    pub fn ceilings(&self) -> Ceilings {
        self.ceilings
    }

    /// Every recorded reservation, in grant order.
    pub fn reservations(&self) -> impl Iterator<Item = &Reservation> {
        self.grants.values()
    }

    /// One reservation by grant id.
    #[must_use]
    pub fn reservation(&self, grant: &str) -> Option<&Reservation> {
        self.grants.get(grant)
    }

    /// Answer *may this task proceed* — grant only what is free.
    ///
    /// Checks run in a stated order and a request stops at the first
    /// that refuses: the request itself must be well-formed, the host
    /// must be able to enforce what is asked, the holder must not
    /// already hold, the exclusive lanes must be open, and the counted
    /// lanes must fit under their ceilings.
    pub fn request(&mut self, needs: Needs) -> Result<Grant, Refusal> {
        if needs.holder.is_empty()
            || needs.holder.len() > 128
            || !needs
                .holder
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(Refusal::Malformed {
                reason: "a holder is a task or run id: 1-128 ASCII letters, digits, hyphens, or underscores".into(),
            });
        }
        if let Some(tag) = &needs.quiet_accelerator
            && (tag.trim().is_empty() || tag.len() > 64)
        {
            return Err(Refusal::Malformed {
                reason: "a quiet-host request names its accelerator: 1-64 bytes".into(),
            });
        }
        if needs.for_seconds == Some(0) {
            return Err(Refusal::Malformed {
                reason: "a stated lifetime is at least one second".into(),
            });
        }
        if needs.isolation != Isolation::Admission {
            return Err(Refusal::Unsupported {
                requested: needs.isolation,
            });
        }
        if let Some(existing) = self
            .grants
            .values()
            .find(|r| r.counts() && r.holder == needs.holder)
        {
            return Err(Refusal::Held {
                holder: needs.holder,
                grant: existing.id.clone(),
            });
        }
        let quiet_holder = self
            .grants
            .values()
            .find(|r| r.counts() && r.held.quiet_accelerator.is_some())
            .map(|r| r.holder.clone());
        if needs.quiet_accelerator.is_some() {
            if let Some(holder) = quiet_holder {
                return Err(Refusal::LaneHeld {
                    lane: Lane::QuietHost,
                    holder,
                });
            }
            let holders: Vec<String> = self
                .grants
                .values()
                .filter(|r| r.counts())
                .map(|r| r.holder.clone())
                .collect();
            if !holders.is_empty() {
                return Err(Refusal::QuietHostBusy { holders });
            }
        } else if let Some(holder) = quiet_holder {
            return Err(Refusal::LaneHeld {
                lane: Lane::QuietHost,
                holder,
            });
        }
        if needs.integration
            && let Some(holder) = self
                .grants
                .values()
                .find(|r| r.counts() && r.held.integration)
                .map(|r| r.holder.clone())
        {
            return Err(Refusal::LaneHeld {
                lane: Lane::Integration,
                holder,
            });
        }
        let held = Requirements {
            executor_slots: needs.executor_slots.unwrap_or(self.fills.executor_slots),
            cpu_units: needs.cpu_units.unwrap_or(self.fills.cpu_units),
            memory_mib: needs.memory_mib.unwrap_or(self.fills.memory_mib),
            quiet_accelerator: needs.quiet_accelerator,
            integration: needs.integration,
            for_seconds: needs.for_seconds,
        };
        let mut slots = 0_u64;
        let mut cpu = 0_u64;
        let mut memory = 0_u64;
        for reservation in self.grants.values().filter(|r| r.counts()) {
            slots = slots.saturating_add(u64::from(reservation.held.executor_slots));
            cpu = cpu.saturating_add(u64::from(reservation.held.cpu_units));
            memory = memory.saturating_add(reservation.held.memory_mib);
        }
        if slots.saturating_add(u64::from(held.executor_slots))
            > u64::from(self.ceilings.executor_slots)
        {
            return Err(Refusal::Ceiling {
                lane: Lane::Executor,
                held: slots,
                requested: u64::from(held.executor_slots),
                ceiling: u64::from(self.ceilings.executor_slots),
            });
        }
        if cpu.saturating_add(u64::from(held.cpu_units)) > u64::from(self.ceilings.cpu_units) {
            return Err(Refusal::Ceiling {
                lane: Lane::CpuBuild,
                held: cpu,
                requested: u64::from(held.cpu_units),
                ceiling: u64::from(self.ceilings.cpu_units),
            });
        }
        if memory.saturating_add(held.memory_mib) > self.ceilings.memory_mib {
            return Err(Refusal::Ceiling {
                lane: Lane::Memory,
                held: memory,
                requested: held.memory_mib,
                ceiling: self.ceilings.memory_mib,
            });
        }
        self.next += 1;
        let id = format!("r-{:06}", self.next);
        self.grants.insert(
            id.clone(),
            Reservation {
                id: id.clone(),
                holder: needs.holder.clone(),
                held: held.clone(),
                granted_unix: needs.at_unix,
                state: State::Held,
            },
        );
        Ok(Grant {
            id,
            holder: needs.holder,
            held,
            granted_unix: needs.at_unix,
        })
    }

    /// End a grant because its holder presented it back.
    ///
    /// The transition is recorded on the reservation; the grant's lanes
    /// count again from this point. Releasing an id no live grant
    /// carries refuses — the book names real grants or refuses.
    pub fn release(&mut self, grant: &str, at_unix: u64) -> Result<Reservation, Refusal> {
        let reservation = self
            .grants
            .get_mut(grant)
            .ok_or_else(|| Refusal::UnknownGrant {
                grant: grant.to_string(),
            })?;
        if !reservation.counts() {
            return Err(Refusal::AlreadyEnded {
                grant: grant.to_string(),
            });
        }
        reservation.state = State::Ended(End::Released { at_unix });
        Ok(reservation.clone())
    }

    /// End every live grant whose stated lifetime has run out.
    ///
    /// `at_unix` is supplied — the book reads no clock. Returns the
    /// expired grant ids, in grant order.
    pub fn expire(&mut self, at_unix: u64) -> Vec<String> {
        let mut expired = Vec::new();
        for reservation in self.grants.values_mut() {
            if !reservation.counts() {
                continue;
            }
            if let Some(seconds) = reservation.held.for_seconds
                && reservation.granted_unix.saturating_add(seconds) <= at_unix
            {
                reservation.state = State::Ended(End::Expired { at_unix });
                expired.push(reservation.id.clone());
            }
        }
        expired
    }

    /// Reconcile the book against an observation of who is alive.
    ///
    /// `observed` reports a liveness per holder: `Live` keeps the grant
    /// — a slow holder is a live holder, and the book never frees on
    /// slowness; `Dead` frees it, because a dead task's reservation
    /// leaks nothing. Anything else — `Ambiguous`, or a holder the
    /// observation does not mention at all — marks the grant `unknown`
    /// for operator reconciliation rather than freeing it for reuse.
    /// An unknown grant returns to held when its holder is later
    /// observed live: it never stopped counting, so nothing was
    /// promised twice.
    pub fn reconcile(
        &mut self,
        observed: &BTreeMap<String, Liveness>,
        at_unix: u64,
    ) -> Reconciliation {
        let mut reconciliation = Reconciliation::default();
        for reservation in self.grants.values_mut() {
            if !reservation.counts() {
                continue;
            }
            match observed.get(&reservation.holder) {
                Some(Liveness::Live) => {
                    if reservation.state == State::Unknown {
                        reservation.state = State::Held;
                    }
                    reconciliation.kept.push(reservation.id.clone());
                }
                Some(Liveness::Dead) => {
                    reservation.state = State::Ended(End::Reclaimed { at_unix });
                    reconciliation.freed.push(reservation.id.clone());
                }
                Some(Liveness::Ambiguous) | None => {
                    reservation.state = State::Unknown;
                    reconciliation.unknown.push(reservation.id.clone());
                }
            }
        }
        reconciliation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Ceilings {
        Ceilings {
            executor_slots: 2,
            cpu_units: 8,
            memory_mib: 8192,
        }
    }

    fn needs(holder: &str) -> Needs {
        Needs {
            holder: holder.into(),
            executor_slots: None,
            cpu_units: None,
            memory_mib: None,
            quiet_accelerator: None,
            integration: false,
            isolation: Isolation::Admission,
            for_seconds: None,
            at_unix: 1_000,
        }
    }

    #[test]
    fn executor_slots_bound_the_admitted_set() {
        let mut book = Book::new(host());
        book.request(needs("task-a")).unwrap();
        book.request(needs("task-b")).unwrap();
        assert_eq!(
            book.request(needs("task-c")),
            Err(Refusal::Ceiling {
                lane: Lane::Executor,
                held: 2,
                requested: 1,
                ceiling: 2,
            }),
            "two executor places fill the host"
        );
    }

    #[test]
    fn the_quiet_host_refuses_a_second_holder_by_name() {
        let mut book = Book::new(host());
        let mut quiet = needs("measure");
        quiet.quiet_accelerator = Some("metal".into());
        book.request(quiet.clone()).unwrap();
        assert_eq!(
            book.request(quiet),
            Err(Refusal::Held {
                holder: "measure".into(),
                grant: "r-000001".into(),
            }),
            "the same holder asks twice"
        );
        let mut second = needs("other-measure");
        second.quiet_accelerator = Some("metal".into());
        assert_eq!(
            book.request(second),
            Err(Refusal::LaneHeld {
                lane: Lane::QuietHost,
                holder: "measure".into(),
            }),
            "a second quiet holder is refused by name"
        );
        assert_eq!(
            book.request(needs("ordinary")),
            Err(Refusal::LaneHeld {
                lane: Lane::QuietHost,
                holder: "measure".into(),
            }),
            "ordinary work excludes while the quiet lane is held"
        );
    }

    #[test]
    fn the_quiet_lane_admits_only_onto_an_empty_host() {
        let mut book = Book::new(host());
        book.request(needs("task-a")).unwrap();
        let mut quiet = needs("measure");
        quiet.quiet_accelerator = Some("metal".into());
        assert_eq!(
            book.request(quiet),
            Err(Refusal::QuietHostBusy {
                holders: vec!["task-a".into()],
            })
        );
    }

    #[test]
    fn the_integration_lane_is_serial() {
        let mut book = Book::new(host());
        let mut merge = needs("merge-a");
        merge.integration = true;
        book.request(merge).unwrap();
        let mut second = needs("merge-b");
        second.integration = true;
        assert_eq!(
            book.request(second),
            Err(Refusal::LaneHeld {
                lane: Lane::Integration,
                holder: "merge-a".into(),
            }),
            "merges serialize through one lane"
        );
    }

    #[test]
    fn unstated_needs_fill_conservatively_never_free() {
        let mut book = Book::new(host());
        let grant = book.request(needs("quiet-task")).unwrap();
        assert_eq!(
            grant.held,
            Requirements {
                executor_slots: 1,
                cpu_units: 4,
                memory_mib: 4096,
                quiet_accelerator: None,
                integration: false,
                for_seconds: None,
            },
            "unstated needs reserve the conservative share"
        );
        let tight = Ceilings {
            executor_slots: 4,
            cpu_units: 64,
            memory_mib: 2048,
        };
        let mut book = Book::new(tight);
        assert_eq!(
            book.request(needs("unstated")),
            Err(Refusal::Ceiling {
                lane: Lane::Memory,
                held: 0,
                requested: 4096,
                ceiling: 2048,
            }),
            "an unstated memory need never reserves zero"
        );
    }

    #[test]
    fn hard_isolation_refuses_rather_than_approximates() {
        let mut book = Book::new(host());
        for isolation in [Isolation::CpuPinning, Isolation::MemoryLimit] {
            let mut ask = needs("isolated");
            ask.isolation = isolation;
            assert_eq!(
                book.request(ask),
                Err(Refusal::Unsupported {
                    requested: isolation
                })
            );
        }
        assert_eq!(
            book.reservations().count(),
            0,
            "an unsupported request leaves no grant"
        );
    }

    #[test]
    fn a_vanished_holder_reconciles_and_leaks_nothing() {
        let mut book = Book::new(host());
        let grant = book.request(needs("gone")).unwrap();
        let observed = BTreeMap::from([("gone".to_string(), Liveness::Dead)]);
        let reconciliation = book.reconcile(&observed, 2_000);
        assert_eq!(reconciliation.freed, [grant.id.as_str()]);
        assert_eq!(
            book.reservation(&grant.id).unwrap().state,
            State::Ended(End::Reclaimed { at_unix: 2_000 })
        );
        assert!(
            book.request(needs("next")).is_ok(),
            "the freed capacity admits again"
        );
    }

    #[test]
    fn a_slow_holder_keeps_its_grant() {
        let mut book = Book::new(host());
        book.request(needs("slow")).unwrap();
        let observed = BTreeMap::from([("slow".to_string(), Liveness::Live)]);
        let reconciliation = book.reconcile(&observed, 9_999);
        assert_eq!(reconciliation.kept, ["r-000001"]);
        assert!(reconciliation.freed.is_empty());
        assert_eq!(
            book.reservation("r-000001").unwrap().state,
            State::Held,
            "slowness frees nothing"
        );
    }

    #[test]
    fn an_ambiguous_holder_marks_unknown_and_still_counts() {
        let mut book = Book::new(host());
        book.request(needs("unclear")).unwrap();
        let reconciliation = book.reconcile(&BTreeMap::new(), 3_000);
        assert_eq!(reconciliation.unknown, ["r-000001"]);
        assert!(reconciliation.freed.is_empty());
        assert_eq!(
            book.reservation("r-000001").unwrap().state,
            State::Unknown,
            "an unobserved holder is ambiguous, never freed"
        );
        assert_eq!(
            book.request(needs("unclear")),
            Err(Refusal::Held {
                holder: "unclear".into(),
                grant: "r-000001".into(),
            }),
            "an unknown grant still counts"
        );
        let observed = BTreeMap::from([("unclear".to_string(), Liveness::Live)]);
        book.reconcile(&observed, 4_000);
        assert_eq!(
            book.reservation("r-000001").unwrap().state,
            State::Held,
            "a live observation heals the mark — nothing was promised twice"
        );
    }

    #[test]
    fn the_same_requests_grant_the_same_book() {
        let mut first = Book::new(host());
        let mut second = Book::new(host());
        let mut quiet = needs("measure");
        quiet.quiet_accelerator = Some("ane".into());
        let requests = [needs("a"), needs("b"), quiet, needs("c")];
        let a: Vec<_> = requests
            .iter()
            .cloned()
            .map(|need| first.request(need))
            .collect();
        let b: Vec<_> = requests
            .into_iter()
            .map(|need| second.request(need))
            .collect();
        assert_eq!(a, b, "same book, same requests, same grants");
        let ra: Vec<_> = first.reservations().cloned().collect();
        let rb: Vec<_> = second.reservations().cloned().collect();
        assert_eq!(ra, rb);
    }

    #[test]
    fn grant_release_and_regrant_works() {
        let mut book = Book::new(host());
        let grant = book.request(needs("round-one")).unwrap();
        let reservation = book.release(&grant.id, 1_500).unwrap();
        assert_eq!(
            reservation.state,
            State::Ended(End::Released { at_unix: 1_500 })
        );
        assert_eq!(
            book.release(&grant.id, 1_600),
            Err(Refusal::AlreadyEnded {
                grant: grant.id.clone()
            }),
            "a second release changes nothing"
        );
        let regrant = book.request(needs("round-two")).unwrap();
        assert_eq!(regrant.id, "r-000002");
        assert_eq!(
            book.reservations().filter(|r| r.counts()).count(),
            1,
            "the ended grant counts nothing"
        );
    }

    #[test]
    fn a_stated_lifetime_expires_without_a_clock() {
        let mut book = Book::new(host());
        let mut timed = needs("timed");
        timed.for_seconds = Some(60);
        book.request(timed).unwrap();
        assert!(book.expire(1_059).is_empty());
        assert_eq!(book.expire(1_060), ["r-000001"]);
        assert_eq!(
            book.reservation("r-000001").unwrap().state,
            State::Ended(End::Expired { at_unix: 1_060 })
        );
    }
}

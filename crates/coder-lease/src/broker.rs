//! Admission, waiting, release, and receipts.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::table::{Entry, Guard, Priority, State, held_lock, prepare, private_file, write_atomic};
use crate::{Error, Grant, Holder, Limits, Resource, Shape};

/// The variable a wrapped command reads its lease's identifier from.
pub const LEASE_ID_VAR: &str = "OPENAGENTS_LEASE_ID";
/// The variable that lists, comma-separated, the resources a wrapped
/// command runs under, such as `quiet` or `build,gpu`.
pub const LEASES_VAR: &str = "OPENAGENTS_LEASES";
/// The receipt file's schema.
pub const RECEIPT_SCHEMA: &str = "openagents.lease.receipt.v1";
/// How often a waiting request looks at the table again.
pub const POLL: Duration = Duration::from_millis(250);

/// What a request does when it can't be admitted at once.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wait {
    /// Fail at once with [`Error::Busy`].
    No,
    /// Wait in the queue until admitted.
    Forever,
    /// Wait up to this long, then fail with [`Error::TimedOut`].
    Up(Duration),
}

/// A request for a lease.
#[derive(Clone, Debug)]
pub struct Request {
    /// The resource.
    pub resource: Resource,
    /// How much of a counted resource; 1 for an exclusive one and for a
    /// build slot unless set. Memory and disk leases must set it.
    pub amount: Option<u64>,
    /// Who asks.
    pub holder: Holder,
    /// How urgent it is.
    pub priority: Priority,
    /// Whether it waits.
    pub wait: Wait,
    /// The resources the caller already runs under, from `OPENAGENTS_LEASES`.
    pub inherited: Vec<String>,
    /// The lease the caller already runs under, from `OPENAGENTS_LEASE_ID`.
    pub outer_id: Option<String>,
}

impl Request {
    /// A request that waits, at normal priority, under no outer lease.
    #[must_use]
    pub fn new(resource: Resource, holder: Holder) -> Request {
        Request {
            resource,
            amount: None,
            holder,
            priority: Priority::Normal,
            wait: Wait::Forever,
            inherited: Vec::new(),
            outer_id: None,
        }
    }

    /// Sets the amount.
    #[must_use]
    pub fn amount(mut self, amount: u64) -> Request {
        self.amount = Some(amount);
        self
    }

    /// Sets how urgent it is.
    #[must_use]
    pub fn priority(mut self, priority: Priority) -> Request {
        self.priority = priority;
        self
    }

    /// Sets whether it waits.
    #[must_use]
    pub fn wait(mut self, wait: Wait) -> Request {
        self.wait = wait;
        self
    }

    /// Records the leases the caller runs under: `leases` as
    /// `OPENAGENTS_LEASES` lists them, and the outer lease's identifier.
    #[must_use]
    pub fn inherit(mut self, leases: &str, outer_id: Option<String>) -> Request {
        self.inherited = leases
            .split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .collect();
        self.outer_id = outer_id.filter(|id| !id.is_empty());
        self
    }

    /// Records the leases this process runs under, from its environment.
    #[must_use]
    pub fn inherit_env(self) -> Request {
        let leases = std::env::var(LEASES_VAR).unwrap_or_default();
        self.inherit(&leases, std::env::var(LEASE_ID_VAR).ok())
    }

    fn amount_or_default(&self) -> Option<u64> {
        match (&self.resource, self.amount) {
            (_, Some(amount)) => Some(amount),
            (Resource::Memory | Resource::Disk, None) => None,
            _ => Some(1),
        }
    }
}

type FreeDisk = Arc<dyn Fn(&Path) -> std::io::Result<u64> + Send + Sync>;

/// The broker over one lease root.
#[derive(Clone)]
pub struct Broker {
    root: PathBuf,
    scratch: PathBuf,
    limits: Limits,
    poll: Duration,
    aging: Option<Duration>,
    free_disk: FreeDisk,
}

impl std::fmt::Debug for Broker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Broker")
            .field("root", &self.root)
            .field("scratch", &self.scratch)
            .field("limits", &self.limits)
            .field("aging", &self.aging)
            .finish_non_exhaustive()
    }
}

static NEXT_ID: AtomicU64 = AtomicU64::new(0);

/// Why a request waits, for the person or agent watching it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blocked {
    /// A sentence saying why.
    pub reason: String,
    /// The leases it waits on.
    pub by: Vec<Entry>,
}

enum Decision {
    Admit,
    Wait(Blocked),
    Refuse(Error),
}

impl Broker {
    /// The broker at the root `$OPENAGENTS_LEASE_ROOT`, else
    /// `~/.openagents/leases`, with this machine's limits and the aging
    /// step `OPENAGENTS_LEASE_AGING_MINUTES` chooses.
    ///
    /// # Errors
    /// A sentence when the root, a limit, or the aging step can't be
    /// determined.
    pub fn from_env() -> Result<Broker, Error> {
        let root = crate::root_from_env().map_err(Error::Invalid)?;
        let limits = Limits::from_env().map_err(Error::Invalid)?;
        let aging =
            crate::limits::aging_from(&|name| std::env::var(name).ok()).map_err(Error::Invalid)?;
        Ok(Broker::new(root, limits).with_aging(aging))
    }

    /// The broker at `root` with `limits`. Its leases' scratch root is
    /// `$OPENAGENTS_SCRATCH_ROOT`, else `scratch` beside `root`
    /// ([`crate::scratch`]).
    #[must_use]
    pub fn new(root: PathBuf, limits: Limits) -> Broker {
        crate::root::refuse_real_home(&root);
        let scratch = std::env::var_os(crate::scratch::SCRATCH_ROOT_VAR)
            .filter(|root| !root.is_empty())
            .map_or_else(|| crate::scratch::beside(&root), PathBuf::from);
        Broker {
            root,
            scratch,
            limits,
            poll: POLL,
            aging: Some(crate::DEFAULT_AGING),
            free_disk: Arc::new(crate::limits::free_disk),
        }
    }

    /// Looks at the table this often while waiting.
    #[must_use]
    pub fn with_poll(mut self, poll: Duration) -> Broker {
        self.poll = poll;
        self
    }

    /// Raises a waiter one priority level for each `aging` step it waits;
    /// `None` turns aging off.
    #[must_use]
    pub fn with_aging(mut self, aging: Option<Duration>) -> Broker {
        self.aging = aging;
        self
    }

    /// The aging step, or `None` when aging is off.
    #[must_use]
    pub fn aging(&self) -> Option<Duration> {
        self.aging
    }

    /// Reads free space through `free_disk` instead of the volume.
    #[must_use]
    pub fn with_free_disk(
        mut self,
        free_disk: impl Fn(&Path) -> std::io::Result<u64> + Send + Sync + 'static,
    ) -> Broker {
        self.free_disk = Arc::new(free_disk);
        self
    }

    /// The lease root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Gives the leases' sessions their scratch directories under `root`.
    #[must_use]
    pub fn with_scratch_root(mut self, root: PathBuf) -> Broker {
        self.scratch = root;
        self
    }

    /// The root the leases' scratch directories are under.
    #[must_use]
    pub fn scratch_root(&self) -> &Path {
        &self.scratch
    }

    /// The limits counted leases share.
    #[must_use]
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// Every live lease, held and waiting, in queue order. Leases whose
    /// holders are gone are dropped from the table first.
    ///
    /// # Errors
    /// The table can't be read or written.
    pub fn list(&self) -> Result<Vec<Entry>, Error> {
        let mut guard = Guard::open(&self.root)?;
        if guard.prune() > 0 {
            guard.save()?;
        }
        let mut leases = guard.table.leases.clone();
        leases.sort_by_key(|entry| entry.seq);
        Ok(leases)
    }

    /// Every live lease in queue order: by resource, the held leases first
    /// in arrival order, then the waiters in the order they will be
    /// admitted, each with its effective priority, its place in the queue,
    /// and how long it has waited.
    ///
    /// # Errors
    /// The table can't be read or written.
    pub fn queue(&self) -> Result<Vec<Queued>, Error> {
        let now = crate::now_ms();
        Ok(queue_of(self.list()?, now, self.aging))
    }

    /// Takes a lease, waiting in the queue as the request says.
    ///
    /// # Errors
    /// [`Error::Busy`] or [`Error::TimedOut`] when it wasn't admitted in
    /// time, [`Error::NoGrant`] for an ungranted `screen`, and
    /// [`Error::Invalid`] for an amount the resource can never admit.
    pub fn acquire(&self, request: Request) -> Result<Lease, Error> {
        self.acquire_notify(request, &mut |_| {})
    }

    /// [`Broker::acquire`], calling `waiting` once when the request starts
    /// to wait and again whenever the reason changes.
    ///
    /// # Errors
    /// As [`Broker::acquire`].
    pub fn acquire_notify(
        &self,
        request: Request,
        waiting: &mut dyn FnMut(&Blocked),
    ) -> Result<Lease, Error> {
        let requested_at_ms = crate::now_ms();
        let amount = self.validate(&request)?;
        prepare(&self.root)?;
        let name = request.resource.to_string();

        // Nesting: a command already under a live lease of this resource
        // passes through, so a wrapped command that calls the wrapper again
        // can't wait on itself.
        if request.inherited.iter().any(|lease| *lease == name) {
            let leases = self.list()?;
            if let Some(outer) = leases
                .iter()
                .filter(|entry| entry.resource == name && entry.state == State::Held)
                .find(|entry| request.outer_id.as_deref().is_none_or(|id| id == entry.id))
                .or_else(|| {
                    leases
                        .iter()
                        .find(|entry| entry.resource == name && entry.state == State::Held)
                })
            {
                let id = request.outer_id.clone().unwrap_or_else(|| outer.id.clone());
                return Ok(Lease {
                    root: self.root.clone(),
                    scratch: self.scratch.clone(),
                    entry: Entry {
                        id,
                        resource: name,
                        amount,
                        state: State::Held,
                        holder: request.holder,
                        priority: request.priority,
                        seq: outer.seq,
                        requested_at_ms,
                        acquired_at_ms: Some(crate::now_ms()),
                    },
                    inherited: request.inherited,
                    lock: None,
                    released: false,
                });
            }
        }

        if request.resource == Resource::Screen && self.screen_grant_for(&request.holder)?.is_none()
        {
            return Err(Error::NoGrant(no_grant(&request.holder)));
        }

        let id = format!(
            "{}-{}-{}",
            requested_at_ms,
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        );
        // The lock is taken before the file gets its name, so a reader
        // never finds an unlocked holder lock for a live lease.
        let lock_path = held_lock(&self.root, &id);
        let staging = self.root.join("held").join(format!(".{id}.new"));
        let lock = private_file(&staging, true)?;
        lock.lock()?;
        std::fs::rename(&staging, &lock_path)?;
        let mut lease = Lease {
            root: self.root.clone(),
            scratch: self.scratch.clone(),
            entry: Entry {
                id: id.clone(),
                resource: name,
                amount,
                state: State::Waiting,
                holder: request.holder,
                priority: request.priority,
                seq: 0,
                requested_at_ms,
                acquired_at_ms: None,
            },
            inherited: request.inherited,
            lock: Some(lock),
            released: false,
        };

        let deadline = match request.wait {
            Wait::Up(limit) => Some(Instant::now() + limit),
            _ => None,
        };
        let mut told: Option<String> = None;
        let mut first = true;
        loop {
            let mut guard = Guard::open(&self.root)?;
            guard.prune();
            if first {
                lease.entry.seq = guard.table.next_seq;
                guard.table.next_seq += 1;
                guard.table.leases.push(lease.entry.clone());
                first = false;
            }
            let Some(me) = guard.find(&id).cloned() else {
                drop(guard);
                lease.discard();
                return Err(Error::Lost(id));
            };
            match self.decide(&guard, &me, &request.resource) {
                Decision::Admit => {
                    let now = crate::now_ms();
                    if let Some(entry) = guard.table.leases.iter_mut().find(|e| e.id == id) {
                        entry.state = State::Held;
                        entry.acquired_at_ms = Some(now);
                        lease.entry = entry.clone();
                    }
                    guard.save()?;
                    return Ok(lease);
                }
                Decision::Refuse(error) => {
                    guard.remove(&id);
                    guard.save()?;
                    drop(guard);
                    lease.discard();
                    return Err(error);
                }
                Decision::Wait(blocked) => {
                    let out_of_time = match request.wait {
                        Wait::No => true,
                        Wait::Up(_) => deadline.is_some_and(|at| Instant::now() >= at),
                        Wait::Forever => false,
                    };
                    if out_of_time {
                        guard.remove(&id);
                        guard.save()?;
                        drop(guard);
                        lease.discard();
                        return Err(if request.wait == Wait::No {
                            Error::Busy(blocked)
                        } else {
                            Error::TimedOut(blocked)
                        });
                    }
                    guard.save()?;
                    drop(guard);
                    if told.as_deref() != Some(blocked.reason.as_str()) {
                        waiting(&blocked);
                        told = Some(blocked.reason.clone());
                    }
                }
            }
            let pause = deadline.map_or(self.poll, |at| {
                self.poll
                    .min(at.saturating_duration_since(Instant::now()))
                    .max(Duration::from_millis(1))
            });
            std::thread::sleep(pause);
        }
    }

    fn validate(&self, request: &Request) -> Result<u64, Error> {
        let resource = &request.resource;
        let Some(amount) = request.amount_or_default() else {
            return Err(Error::Invalid(format!(
                "a {resource} lease declares its amount in {}; pass --amount N",
                resource.unit()
            )));
        };
        if amount == 0 {
            return Err(Error::Invalid(format!(
                "a {resource} lease needs an amount of at least 1"
            )));
        }
        if resource.shape() == Shape::Exclusive && amount != 1 {
            return Err(Error::Invalid(format!(
                "{resource} is exclusive; its amount is always 1"
            )));
        }
        if let Some(capacity) = self.limits.capacity(resource)
            && amount > capacity
        {
            return Err(Error::Invalid(format!(
                "{amount} {} of {resource} can never be admitted; the capacity is {capacity}",
                resource.unit()
            )));
        }
        Ok(amount)
    }

    fn decide(&self, guard: &Guard, me: &Entry, resource: &Resource) -> Decision {
        let leases = &guard.table.leases;
        let same = |entry: &&Entry| entry.resource == me.resource && entry.id != me.id;
        let held: Vec<Entry> = leases
            .iter()
            .filter(same)
            .filter(|entry| entry.state == State::Held)
            .cloned()
            .collect();
        // The waiters ahead of this one: more urgent, counting aging, or
        // as urgent and earlier.
        let now = crate::now_ms();
        let mine = me.queue_key(now, self.aging);
        let earlier: Vec<Entry> = leases
            .iter()
            .filter(same)
            .filter(|entry| {
                entry.state == State::Waiting && entry.queue_key(now, self.aging) < mine
            })
            .cloned()
            .collect();
        let wait = |reason: String, by: Vec<Entry>| Decision::Wait(Blocked { reason, by });
        match resource.shape() {
            Shape::Exclusive => {
                if *resource == Resource::Screen {
                    match self.screen_grant_for(&me.holder) {
                        Ok(Some(_)) => {}
                        Ok(None) => return Decision::Refuse(Error::NoGrant(no_grant(&me.holder))),
                        Err(error) => return Decision::Refuse(error),
                    }
                }
                if !held.is_empty() {
                    return wait(format!("{resource} is held"), held);
                }
                if !earlier.is_empty() {
                    return wait(
                        format!("more urgent or earlier requests for {resource} go first"),
                        earlier,
                    );
                }
                if *resource == Resource::Quiet {
                    let builds: Vec<Entry> = leases
                        .iter()
                        .filter(|entry| entry.resource == "build" && entry.state == State::Held)
                        .cloned()
                        .collect();
                    if !builds.is_empty() {
                        return wait(
                            format!(
                                "quiet waits for {} running build{} to finish; new builds wait behind it",
                                builds.len(),
                                if builds.len() == 1 { "" } else { "s" }
                            ),
                            builds,
                        );
                    }
                }
                Decision::Admit
            }
            Shape::Counted => {
                if *resource == Resource::Build {
                    let quiet: Vec<Entry> = leases
                        .iter()
                        .filter(|entry| entry.resource == "quiet")
                        .cloned()
                        .collect();
                    if !quiet.is_empty() {
                        return wait(
                            "a quiet lease is held or queued, so new builds wait".to_owned(),
                            quiet,
                        );
                    }
                }
                if !earlier.is_empty() {
                    return wait(
                        format!("more urgent or earlier requests for {resource} go first"),
                        earlier,
                    );
                }
                let used: u64 = held.iter().map(|entry| entry.amount).sum();
                let unit = resource.unit();
                if let Some(capacity) = self.limits.capacity(resource) {
                    if used + me.amount > capacity {
                        return wait(
                            format!(
                                "{used} of {capacity} {unit} of {resource} are held; this needs {}",
                                me.amount
                            ),
                            held,
                        );
                    }
                    return Decision::Admit;
                }
                // Disk: the floor, the budgets already held, and this one
                // must all fit in the free space.
                let free = match (self.free_disk)(&self.root) {
                    Ok(free) => free,
                    Err(error) => return Decision::Refuse(error.into()),
                };
                let need_gb = self.limits.disk_floor_gb + used + me.amount;
                if free < need_gb.saturating_mul(1_000_000_000) {
                    return wait(
                        format!(
                            "{} GB is free; this needs {need_gb} GB: the {} GB floor, {used} GB held, and {} GB asked",
                            free / 1_000_000_000,
                            self.limits.disk_floor_gb,
                            me.amount
                        ),
                        held,
                    );
                }
                Decision::Admit
            }
        }
    }

    /// Grants a resource, replacing any earlier grant of it.
    ///
    /// # Errors
    /// The grant can't be written.
    pub fn grant(&self, grant: &Grant) -> Result<(), Error> {
        prepare(&self.root)?;
        let bytes =
            serde_json::to_vec_pretty(grant).map_err(|error| Error::Corrupt(error.to_string()))?;
        write_atomic(&self.grant_path(&grant.resource), &bytes)?;
        Ok(())
    }

    /// Ends the grant of a resource. Returns whether one existed. A lease
    /// already held keeps running; new requests are refused.
    ///
    /// # Errors
    /// The grant can't be removed.
    pub fn revoke(&self, resource: &Resource) -> Result<bool, Error> {
        match std::fs::remove_file(self.grant_path(&resource.to_string())) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// The grant of a resource, expired or not.
    ///
    /// # Errors
    /// The grant can't be read.
    pub fn grant_of(&self, resource: &Resource) -> Result<Option<Grant>, Error> {
        let path = self.grant_path(&resource.to_string());
        match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| Error::Corrupt(format!("{}: {error}", path.display()))),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    fn screen_grant_for(&self, holder: &Holder) -> Result<Option<Grant>, Error> {
        Ok(self
            .grant_of(&Resource::Screen)?
            .filter(|grant| grant.admits(&holder.session, crate::now_ms())))
    }

    fn grant_path(&self, resource: &str) -> PathBuf {
        self.root
            .join("grants")
            .join(format!("{}.json", resource.replace('/', "_")))
    }
}

/// A lease as `openagents lease list` shows it: the table entry, the
/// priority it competes at now, its place in its resource's queue, and how
/// long it has waited.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Queued {
    /// The lease.
    #[serde(flatten)]
    pub entry: Entry,
    /// The priority it competes at now: its own, raised by aging.
    pub effective_priority: Priority,
    /// Its place among its resource's waiters, from 1; `None` when held.
    pub position: Option<usize>,
    /// How long it waited, or has waited so far, in milliseconds.
    pub wait_ms: u64,
}

/// `leases` in queue order at `now_ms`; see [`Broker::queue`].
#[must_use]
pub fn queue_of(mut leases: Vec<Entry>, now_ms: u64, aging: Option<Duration>) -> Vec<Queued> {
    let rank = |entry: &Entry| match entry.state {
        State::Held => (0, Priority::Owner, entry.seq),
        State::Waiting => {
            let (priority, seq) = entry.queue_key(now_ms, aging);
            (1, priority, seq)
        }
    };
    leases.sort_by(|a, b| (a.resource.as_str(), rank(a)).cmp(&(b.resource.as_str(), rank(b))));
    let mut position = 0;
    let mut resource = String::new();
    leases
        .into_iter()
        .map(|entry| {
            if entry.resource != resource {
                resource.clone_from(&entry.resource);
                position = 0;
            }
            let place = (entry.state == State::Waiting).then(|| {
                position += 1;
                position
            });
            let wait_ms = entry
                .acquired_at_ms
                .unwrap_or(now_ms)
                .saturating_sub(entry.requested_at_ms);
            Queued {
                effective_priority: match entry.state {
                    State::Held => entry.priority,
                    State::Waiting => entry.effective_priority(now_ms, aging),
                },
                position: place,
                wait_ms,
                entry,
            }
        })
        .collect()
}

fn no_grant(holder: &Holder) -> String {
    format!(
        "the screen needs the owner's grant for session {}; the owner runs `openagents lease grant screen` on a terminal",
        holder.session
    )
}

/// A lease this process holds. Dropping it releases it; [`Lease::release`]
/// also returns the receipt.
#[derive(Debug)]
pub struct Lease {
    root: PathBuf,
    scratch: PathBuf,
    entry: Entry,
    inherited: Vec<String>,
    pub(crate) lock: Option<File>,
    released: bool,
}

impl Lease {
    /// The lease's identifier; for a nested lease, the outer lease's.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.entry.id
    }

    /// The lease as the table records it.
    #[must_use]
    pub fn entry(&self) -> &Entry {
        &self.entry
    }

    /// Whether it passed through an outer lease of the same resource
    /// rather than taking one.
    #[must_use]
    pub fn nested(&self) -> bool {
        self.lock.is_none()
    }

    /// The variables a command run under this lease gets:
    /// `OPENAGENTS_LEASE_ID`, `OPENAGENTS_LEASES`, `OPENAGENTS_SESSION`,
    /// and `OPENAGENTS_SCRATCH`, the session's scratch directory, which
    /// this creates when missing. A scratch directory that can't be
    /// created is left out.
    #[must_use]
    pub fn env(&self) -> Vec<(String, String)> {
        let mut leases = self.inherited.clone();
        if !leases.contains(&self.entry.resource) {
            leases.push(self.entry.resource.clone());
        }
        let mut env = vec![
            (LEASE_ID_VAR.to_owned(), self.entry.id.clone()),
            (LEASES_VAR.to_owned(), leases.join(",")),
            (
                crate::SESSION_VAR.to_owned(),
                self.entry.holder.session.clone(),
            ),
        ];
        if let Ok(dir) = crate::scratch::ensure(&self.scratch, &self.entry.holder.session) {
            env.push((
                crate::scratch::SCRATCH_VAR.to_owned(),
                dir.display().to_string(),
            ));
        }
        env
    }

    /// Releases the lease and writes its receipt to `receipts/<id>.json`,
    /// recording `exit`, the wrapped command's exit code, when given. A
    /// nested lease takes nothing from the table and writes no receipt
    /// there, but still returns one.
    ///
    /// # Errors
    /// The table or the receipt can't be written. The lease is released
    /// either way once this process exits.
    pub fn release(mut self, exit: Option<i32>) -> Result<Receipt, Error> {
        self.finish(exit)
    }

    fn finish(&mut self, exit: Option<i32>) -> Result<Receipt, Error> {
        self.released = true;
        let released_at_ms = crate::now_ms();
        let acquired_at_ms = self.entry.acquired_at_ms.unwrap_or(released_at_ms);
        let nested = self.nested();
        let mut whole = true;
        if let Some(lock) = &self.lock {
            let path = held_lock(&self.root, &self.entry.id);
            whole = same_file(lock, &path);
            let mut guard = Guard::open(&self.root)?;
            let present = guard
                .find(&self.entry.id)
                .is_some_and(|entry| entry.state == State::Held);
            whole &= present;
            guard.remove(&self.entry.id);
            guard.save()?;
            drop(guard);
            if whole {
                let _ = std::fs::remove_file(&path);
            }
        }
        self.lock = None;
        let receipt = Receipt {
            schema: RECEIPT_SCHEMA.to_owned(),
            id: self.entry.id.clone(),
            resource: self.entry.resource.clone(),
            amount: self.entry.amount,
            holder: self.entry.holder.clone(),
            priority: self.entry.priority,
            nested,
            requested_at_ms: self.entry.requested_at_ms,
            acquired_at_ms,
            released_at_ms,
            wait_ms: acquired_at_ms.saturating_sub(self.entry.requested_at_ms),
            held_ms: released_at_ms.saturating_sub(acquired_at_ms),
            held_whole_run: whole,
            exit,
        };
        if !nested {
            receipt.write(
                &self
                    .root
                    .join("receipts")
                    .join(format!("{}.json", receipt.id)),
            )?;
        }
        Ok(receipt)
    }

    /// Drops a lease that was never admitted: no receipt, and its lock
    /// file goes.
    fn discard(&mut self) {
        self.released = true;
        if self.lock.take().is_some() {
            let _ = std::fs::remove_file(held_lock(&self.root, &self.entry.id));
        }
    }
}

impl Drop for Lease {
    fn drop(&mut self) {
        if !self.released {
            let _ = self.finish(None);
        }
    }
}

/// Whether the open lock is still the file at `path`: nobody removed or
/// replaced it while the lease was held.
fn same_file(lock: &File, path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        match (lock.metadata(), std::fs::symlink_metadata(path)) {
            (Ok(open), Ok(named)) => open.dev() == named.dev() && open.ino() == named.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = lock;
        path.exists()
    }
}

/// What a lease leaves behind when it ends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    /// The schema, [`RECEIPT_SCHEMA`].
    pub schema: String,
    /// The lease's identifier.
    pub id: String,
    /// The resource.
    pub resource: String,
    /// The amount.
    pub amount: u64,
    /// Who held it.
    pub holder: Holder,
    /// How urgent the request was.
    pub priority: Priority,
    /// Whether it passed through an outer lease of the same resource.
    pub nested: bool,
    /// When it was requested, in Unix milliseconds.
    pub requested_at_ms: u64,
    /// When it was admitted.
    pub acquired_at_ms: u64,
    /// When it was released.
    pub released_at_ms: u64,
    /// How long it waited in the queue, in milliseconds.
    pub wait_ms: u64,
    /// How long it was held, in milliseconds.
    pub held_ms: u64,
    /// Whether the table and the holder lock still named this lease when it
    /// ended, so it held its resource for the whole run.
    pub held_whole_run: bool,
    /// The wrapped command's exit code, when it had one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit: Option<i32>,
}

impl Receipt {
    /// Writes the receipt as JSON to `path`.
    ///
    /// # Errors
    /// The file can't be written.
    pub fn write(&self, path: &Path) -> Result<(), Error> {
        let mut bytes =
            serde_json::to_vec_pretty(self).map_err(|error| Error::Corrupt(error.to_string()))?;
        bytes.push(b'\n');
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        write_atomic(path, &bytes)?;
        Ok(())
    }
}

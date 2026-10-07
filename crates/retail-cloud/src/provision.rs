//! Provisioning one admitted retail computer (#10711).
//!
//! [`Provider`] is the provider seam: create a sandbox from an exact
//! [`CreateSpec`], find one by its provisioning identity, read its state,
//! delete it, and read its billed seconds. The v1 class pins Boat
//! (`docs/cloud/retail-contract.md`): one `large` sandbox per task from the
//! newest ready daily template, started with no account environment.
//! [`crate::fake::FakeProvider`] simulates it for every test; the live Boat
//! binding (`crate::boat`, feature `boat`) runs only in the owner's funded
//! qualification (`NEEDS_OWNER.md`).
//!
//! [`advance`] moves one funded execution's provisioning forward by one
//! observation, and is safe to call again after any crash:
//!
//! 1. It refuses without the provision authorities or a live hold for the
//!    funded request, before any provider call.
//! 2. It records the intent before the first create call.
//! 3. When a create's acknowledgment was lost, it looks the sandbox up by
//!    its provisioning identity rather than create another.
//! 4. A sandbox not ready within 10 minutes, or one whose restore failed, is
//!    deleted and replaced once; the replacement's cost is the operator's.
//!    After that the outcome is `unavailable`.
//! 5. A start refused for plan or capacity limits is `refused`. Nothing
//!    widens the provider or the computer class automatically.

use pay_ledger::Ledger;
use pay_ledger::compute::HoldState;
use rusqlite::{OptionalExtension, params};
use serde::{Deserialize, Serialize};

use crate::authority::{self, Current, Step};
use crate::journal::Journal;
use crate::offer::FundedRequest;
use crate::{Error, Result};

pub(crate) const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS provision (
    execution TEXT PRIMARY KEY,
    account TEXT NOT NULL,
    attempt INTEGER NOT NULL CHECK(attempt IN (1, 2)),
    resource TEXT,
    state TEXT NOT NULL,
    address TEXT,
    detail TEXT,
    intent_at INTEGER NOT NULL,
    attempt_at INTEGER NOT NULL,
    ready_at INTEGER,
    abandoned TEXT
);
";

/// How long a sandbox may take to become reachable before it is replaced.
pub const READY_DEADLINE_SECS: i64 = 600;
/// The Boat size the v1 class pins.
pub const BOAT_SIZE: &str = "large";

/// Exactly what the provider is asked to start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateSpec {
    /// `<execution>#<attempt>`: the label a lost acknowledgment is found by.
    pub provisioning: String,
    /// The customer's account, as an isolation label.
    pub account: String,
    /// `oa-coder-main-<date>`.
    pub template: String,
    pub size: String,
    /// Start with no account environment or credential from the provider.
    pub no_env: bool,
    /// The sandbox's own lifetime bound, past which the provider deletes it.
    pub lifetime_secs: u64,
}

/// A sandbox the provider knows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resource {
    pub id: String,
    pub provisioning: String,
    pub account: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResourceState {
    Starting,
    Ready {
        address: String,
    },
    /// The sandbox's snapshot restore failed.
    RestoreFailed,
    Stopped,
    Deleted,
}

/// Why a start was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartRefusal {
    /// The operator plan's start limit.
    PlanLimit,
    /// The provider has no capacity.
    Capacity,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderError {
    /// The provider refused the start; nothing was created.
    Refused(StartRefusal),
    /// The call failed or its answer was lost; the effect is unknown.
    Unknown(String),
}

/// The provider seam.
pub trait Provider {
    /// Start a sandbox. A lost acknowledgment is [`ProviderError::Unknown`]
    /// even when the sandbox was created.
    fn create(&self, spec: &CreateSpec) -> std::result::Result<Resource, ProviderError>;
    /// The sandbox labeled `provisioning`, if one exists.
    fn find(&self, provisioning: &str) -> std::result::Result<Option<Resource>, ProviderError>;
    /// Reconcile a create whose reply never reached the local index. The
    /// caller must recheck provision authority and the original live hold.
    /// A backend may replay only the identical durably retained request
    /// inside its idempotency window; absence alone never authorizes creation.
    /// The default performs no side effect.
    fn reconcile_creation(
        &self,
        _spec: &CreateSpec,
        _now: i64,
    ) -> std::result::Result<Option<Resource>, ProviderError> {
        Ok(None)
    }
    fn state(&self, id: &str) -> std::result::Result<ResourceState, ProviderError>;
    /// Delete a sandbox. Deleting a deleted one succeeds.
    fn delete(&self, id: &str) -> std::result::Result<(), ProviderError>;
    /// The sandbox's billed seconds, when the provider can say.
    fn usage_seconds(&self, id: &str) -> std::result::Result<Option<u64>, ProviderError>;
}

/// Where one execution's provisioning stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum ProvisionState {
    /// Recorded; no create call yet.
    Intent,
    /// A create call went out and its outcome is not known yet.
    Creating,
    Starting {
        resource: String,
    },
    Ready {
        resource: String,
        address: String,
    },
    /// The provider refused the start; nothing runs and nothing is charged.
    Refused {
        reason: StartRefusal,
    },
    /// Not reachable after the one replacement; nothing is charged.
    Unavailable,
}

/// One execution's provisioning record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provisioning {
    pub execution: String,
    pub account: String,
    pub attempt: u32,
    pub state: ProvisionState,
    pub intent_at: i64,
    pub ready_at: Option<i64>,
    /// Sandboxes abandoned for not becoming usable, comma separated, kept
    /// for teardown.
    pub abandoned: Option<String>,
}

/// The daily template for `date` (`YYYYMMDD`).
#[must_use]
pub fn template(date: &str) -> String {
    format!("oa-coder-main-{date}")
}

/// The create specification for `funded`'s `attempt`.
#[must_use]
pub fn spec(funded: &FundedRequest, attempt: u32, template: &str) -> CreateSpec {
    CreateSpec {
        provisioning: format!("{}#{attempt}", funded.execution),
        account: funded.account.clone(),
        template: template.into(),
        size: BOAT_SIZE.into(),
        no_env: true,
        lifetime_secs: funded.quote.max_seconds + READY_DEADLINE_SECS as u64 * 2,
    }
}

/// Move `funded`'s provisioning forward by one observation.
///
/// # Errors
///
/// [`Error::Denied`] without the provision authorities, [`Error::Invalid`]
/// without a live hold, or a journal failure. Provider failures are
/// recorded as state, not returned.
pub fn advance(
    journal: &mut Journal,
    ledger: &Ledger,
    provider: &impl Provider,
    funded: &FundedRequest,
    current: &Current,
    template: &str,
    now: i64,
) -> Result<Provisioning> {
    if journal.cleanup_requested(&funded.execution)? {
        return Err(Error::Invalid(
            "cleanup prevents new execution or provisioning",
        ));
    }
    if let Some(found) = journal.provisioning(&funded.execution)?
        && matches!(
            found.state,
            ProvisionState::Ready { .. }
                | ProvisionState::Refused { .. }
                | ProvisionState::Unavailable
        )
    {
        return Ok(found);
    }
    authority::check(Step::Provision, &funded.admission, current).map_err(Error::Denied)?;
    let hold = ledger
        .hold(&funded.request)?
        .ok_or(Error::Invalid("no hold for the funded request"))?;
    if hold.state != HoldState::Held || hold.request.execution != funded.execution {
        return Err(Error::Invalid("the funded request's hold is not live"));
    }
    let mut record = match journal.provisioning(&funded.execution)? {
        Some(found) => found,
        None => journal.record_intent(funded, now)?,
    };
    let attempt = record.attempt;
    let spec = spec(funded, attempt, template);
    record.state = match record.state.clone() {
        ProvisionState::Intent | ProvisionState::Creating => {
            // A create whose acknowledgment was lost is found, not repeated.
            match provider.find(&spec.provisioning) {
                Ok(Some(resource)) => ProvisionState::Starting {
                    resource: resource.id,
                },
                Ok(None) => create(journal, provider, &mut record, &spec, now)?,
                // A failed listing is not proof of absence.
                Err(_) => record.state.clone(),
            }
        }
        ProvisionState::Starting { resource } => match provider.state(&resource) {
            Ok(ResourceState::Ready { address }) => {
                record.ready_at = Some(now);
                ProvisionState::Ready { resource, address }
            }
            Ok(ResourceState::Starting)
                if now - record_attempt_at(journal, &record)? < READY_DEADLINE_SECS =>
            {
                ProvisionState::Starting { resource }
            }
            Ok(_) => replace(
                journal,
                provider,
                &mut record,
                funded,
                &resource,
                template,
                now,
            )?,
            Err(_) => ProvisionState::Starting { resource },
        },
        settled => settled,
    };
    journal.save(&record, now)?;
    Ok(record)
}

fn record_attempt_at(journal: &Journal, record: &Provisioning) -> Result<i64> {
    Ok(journal
        .connection
        .query_row(
            "SELECT attempt_at FROM provision WHERE execution=?",
            [&record.execution],
            |r| r.get(0),
        )
        .optional()?
        .unwrap_or(record.intent_at))
}

fn create(
    journal: &mut Journal,
    provider: &impl Provider,
    record: &mut Provisioning,
    spec: &CreateSpec,
    now: i64,
) -> Result<ProvisionState> {
    // The create call's outcome is unknown until it answers.
    record.state = ProvisionState::Creating;
    journal.save(record, now)?;
    Ok(match provider.create(spec) {
        Ok(resource) => ProvisionState::Starting {
            resource: resource.id,
        },
        Err(ProviderError::Refused(reason)) => ProvisionState::Refused { reason },
        Err(ProviderError::Unknown(_)) => ProvisionState::Creating,
    })
}

fn replace(
    journal: &mut Journal,
    provider: &impl Provider,
    record: &mut Provisioning,
    funded: &FundedRequest,
    resource: &str,
    template: &str,
    now: i64,
) -> Result<ProvisionState> {
    // Delete what did not become usable; teardown retries it if this fails.
    let _ = provider.delete(resource);
    journal.add_abandoned(&record.execution, resource)?;
    record.abandoned = journal
        .provisioning(&record.execution)?
        .and_then(|found| found.abandoned);
    if record.attempt >= 2 {
        return Ok(ProvisionState::Unavailable);
    }
    record.attempt = 2;
    journal.start_attempt(&record.execution, 2, now)?;
    let spec = spec(funded, 2, template);
    create(journal, provider, record, &spec, now)
}

impl Journal {
    /// One execution's provisioning record.
    ///
    /// # Errors
    ///
    /// A SQLite or decoding failure.
    pub fn provisioning(&self, execution: &str) -> Result<Option<Provisioning>> {
        let row = self
            .connection
            .query_row(
                "SELECT execution,account,attempt,resource,state,address,detail,intent_at,ready_at,abandoned FROM provision WHERE execution=?",
                [execution],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, u32>(2)?,
                        r.get::<_, Option<String>>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, Option<String>>(5)?,
                        r.get::<_, Option<String>>(6)?,
                        r.get::<_, i64>(7)?,
                        r.get::<_, Option<i64>>(8)?,
                        r.get::<_, Option<String>>(9)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            execution,
            account,
            attempt,
            resource,
            state,
            address,
            detail,
            intent_at,
            ready_at,
            abandoned,
        )) = row
        else {
            return Ok(None);
        };
        let state = match (state.as_str(), resource, address, detail) {
            ("intent", _, _, _) => ProvisionState::Intent,
            ("creating", _, _, _) => ProvisionState::Creating,
            ("starting", Some(resource), _, _) => ProvisionState::Starting { resource },
            ("ready", Some(resource), Some(address), _) => {
                ProvisionState::Ready { resource, address }
            }
            ("refused", _, _, Some(reason)) => ProvisionState::Refused {
                reason: serde_json::from_str(&reason)?,
            },
            ("unavailable", _, _, _) => ProvisionState::Unavailable,
            _ => return Err(Error::Invalid("provision state")),
        };
        Ok(Some(Provisioning {
            execution,
            account,
            attempt,
            state,
            intent_at,
            ready_at,
            abandoned,
        }))
    }

    /// Every provisioning record, oldest first.
    ///
    /// # Errors
    ///
    /// A SQLite or decoding failure.
    pub fn all_provisioning(&self) -> Result<Vec<Provisioning>> {
        let mut statement = self
            .connection
            .prepare("SELECT execution FROM provision ORDER BY intent_at, execution")?;
        let ids = statement
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        ids.iter()
            .filter_map(|id| self.provisioning(id).transpose())
            .collect()
    }

    fn record_intent(&mut self, funded: &FundedRequest, now: i64) -> Result<Provisioning> {
        let tx = self.immediate()?;
        tx.execute(
            "INSERT INTO provision(execution,account,attempt,state,intent_at,attempt_at) VALUES(?,?,1,'intent',?,?) ON CONFLICT(execution) DO NOTHING",
            params![funded.execution, funded.account, now, now],
        )?;
        tx.commit()?;
        self.provisioning(&funded.execution)?
            .ok_or(Error::Invalid("missing provisioning"))
    }

    fn start_attempt(&mut self, execution: &str, attempt: u32, now: i64) -> Result<()> {
        self.check_custody()?;
        self.connection.execute(
            "UPDATE provision SET attempt=?, attempt_at=? WHERE execution=?",
            params![attempt, now, execution],
        )?;
        Ok(())
    }

    fn add_abandoned(&mut self, execution: &str, resource: &str) -> Result<()> {
        // A second abandoned sandbox is kept beside the first, comma
        // separated; teardown deletes each.
        self.check_custody()?;
        self.connection.execute(
            "UPDATE provision SET abandoned = CASE WHEN abandoned IS NULL THEN ? ELSE abandoned || ',' || ? END WHERE execution=?",
            params![resource, resource, execution],
        )?;
        Ok(())
    }

    pub(crate) fn save(&mut self, record: &Provisioning, _now: i64) -> Result<()> {
        let (state, resource, address, detail) = match &record.state {
            ProvisionState::Intent => ("intent", None, None, None),
            ProvisionState::Creating => ("creating", None, None, None),
            ProvisionState::Starting { resource } => {
                ("starting", Some(resource.clone()), None, None)
            }
            ProvisionState::Ready { resource, address } => {
                ("ready", Some(resource.clone()), Some(address.clone()), None)
            }
            ProvisionState::Refused { reason } => {
                ("refused", None, None, Some(serde_json::to_string(reason)?))
            }
            ProvisionState::Unavailable => ("unavailable", None, None, None),
        };
        let tx = self.immediate()?;
        tx.execute(
            "UPDATE provision SET attempt=?, resource=?, state=?, address=?, detail=?, ready_at=? WHERE execution=?",
            params![
                record.attempt,
                resource,
                state,
                address,
                detail,
                record.ready_at,
                record.execution
            ],
        )?;
        tx.commit()?;
        Ok(())
    }
}

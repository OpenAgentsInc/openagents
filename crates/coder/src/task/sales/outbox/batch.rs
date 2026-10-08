//! Level-1 reviewed batches (REV-66). An owner grant binds up to five exact
//! proposed subjects from one template version and approves each exactly as a
//! single decision would. Dispatch stays single-use through `outbox_intent`.
//! Elapsed time, delivered counts, scores, or recommendations qualify nothing by
//! themselves; the grant is the only authority and it is reset by a pause.
use super::*;

pub const GRANT_SCHEMA: &str = "openagents.sales.outbox-batch.v1";
pub const INITIAL_SIZE: u32 = 5;
pub const CEILING: u32 = 20;
pub const READ_ALL_BATCHES: u32 = 5;
pub const MIN_DELIVERED: u32 = 100;
pub const MIN_CONTACTS: usize = 25;
pub const MIN_WEEKS: u32 = 4;
const MAX_GRANTS: usize = 256;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub proposal: String,
    pub subject_sha256: String,
    /// Digest of the owner's read receipt for this item's subject text.
    pub read_sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Grant {
    pub schema: String,
    pub id: String,
    pub mode: Mode,
    pub template: agents::Artifact,
    pub items: Vec<Item>,
    pub expires_at: u64,
    pub qualification_sha256: String,
    pub owner_review_sha256: String,
}
impl Grant {
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "outbox batch serialization failed")?,
        ))
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GrantPhase {
    Active,
    Expired,
    Revoked,
    Reset,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub grant: Grant,
    pub grant_sha256: String,
    pub owner: String,
    pub granted_at: u64,
    pub phase: GrantPhase,
    pub reference_sha256: Option<String>,
}
/// What level-0 operation has measured so far. Meeting it grants nothing.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Qualification {
    pub mode: Mode,
    pub delivered: u32,
    pub permissioned_contacts: usize,
    pub clean_weeks: u32,
    pub open_incidents: usize,
    pub batches_granted: u32,
    pub batch_size: u32,
    pub ceiling: u32,
    pub eligible: bool,
    pub automatic_promotion: bool,
}
impl Qualification {
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(&serde_json::to_vec(self).map_err(
            |_| "outbox qualification serialization failed",
        )?))
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub grants: BTreeMap<String, Record>,
    pub size: u32,
    pub granted: u32,
}
impl Book {
    pub(super) fn check(&self) -> Result<()> {
        if self.grants.len() > MAX_GRANTS
            || !matches!(self.size, 0 | 5 | 10 | 20)
            || (self.granted as usize) < self.grants.len()
        {
            return Err("outbox batch history or size exceeds its bound".into());
        }
        for (id, r) in &self.grants {
            super::super::id(id)?;
            if r.grant.id != *id || r.grant.schema != GRANT_SCHEMA {
                return Err("outbox batch record is inconsistent".into());
            }
            token(&r.grant_sha256)?;
        }
        Ok(())
    }
    pub fn size(&self) -> u32 {
        if self.size == 0 {
            INITIAL_SIZE
        } else {
            self.size
        }
    }
    /// The active grant covering an exact proposal subject, if any.
    pub fn covering(&self, proposal: &str, subject_sha256: &str) -> Option<&Record> {
        self.grants.values().find(|r| {
            r.grant
                .items
                .iter()
                .any(|i| i.proposal == proposal && i.subject_sha256 == subject_sha256)
        })
    }
    pub(super) fn reset(&mut self, now: u64) {
        for r in self.grants.values_mut() {
            if r.phase == GrantPhase::Active {
                r.phase = GrantPhase::Reset;
                r.reference_sha256 = Some(digest(format!("reset:{now}").as_bytes()));
            }
        }
        self.size = 0;
    }
}

fn weeks_between(start: u64, now: u64) -> Result<u32> {
    let mut weeks = 0;
    let mut mark = start;
    while weeks < MIN_WEEKS && clean_week_elapsed(mark, now)? {
        weeks += 1;
        mark = mark.saturating_add(7 * 86_400);
    }
    Ok(weeks)
}

impl Store {
    /// Measures live level-0 operation. The result is informational.
    pub fn outbox_batch_qualification(
        &mut self,
        access: &Access,
        mode: Mode,
    ) -> Result<Qualification> {
        self.refresh()?;
        self.admin(access)?;
        let book = &self.state.outbox;
        let now = (self.clock)();
        let since = book.cap_started_at;
        let delivered: Vec<&super::Record> = book
            .records
            .values()
            .filter(|r| r.mode == mode && r.phase == Phase::Delivered && r.created_at >= since)
            .collect();
        let contacts: std::collections::BTreeSet<&str> = delivered
            .iter()
            .flat_map(|r| r.contact_pins.iter().map(String::as_str))
            .collect();
        let open_incidents = book
            .incidents
            .values()
            .filter(|i| i.resolved_at.is_none())
            .count();
        let clean_weeks = if since == 0 {
            0
        } else {
            weeks_between(since, now)?
        };
        let eligible = !book.paused
            && since != 0
            && clean_weeks >= MIN_WEEKS
            && delivered.len() >= MIN_DELIVERED as usize
            && contacts.len() >= MIN_CONTACTS
            && open_incidents == 0
            && book.incidents.values().all(|i| i.at < since);
        Ok(Qualification {
            mode,
            delivered: delivered.len() as u32,
            permissioned_contacts: contacts.len(),
            clean_weeks,
            open_incidents,
            batches_granted: book.batches.granted,
            batch_size: book.batches.size(),
            ceiling: CEILING,
            eligible,
            automatic_promotion: false,
        })
    }
    pub(super) fn outbox_grant_batch(
        &mut self,
        access: &Access,
        grant: Grant,
        keys: &dyn email::MailboxCredentials,
        now: u64,
    ) -> Result<super::State> {
        super::super::id(&grant.id)?;
        grant.template.check()?;
        token(&grant.qualification_sha256)?;
        token(&grant.owner_review_sha256)?;
        let qualification = self.outbox_batch_qualification(access, grant.mode)?;
        let book = &self.state.outbox;
        if grant.schema != GRANT_SCHEMA
            || book.batches.grants.len() >= MAX_GRANTS
            || book.batches.grants.contains_key(&grant.id)
            || grant.items.is_empty()
            || grant.items.len() > book.batches.size() as usize
            || grant.expires_at <= now
            || grant.expires_at > now + 7 * 86_400
        {
            return Err(
                "outbox batch needs one to batch-size exact items and a bounded expiry".into(),
            );
        }
        if !qualification.eligible || qualification.sha256()? != grant.qualification_sha256 {
            return Err("outbox batch requires the exact current level-0 qualification and an explicit owner grant".into());
        }
        let must_read_all = book.batches.granted < READ_ALL_BATCHES;
        let mut seen = std::collections::BTreeSet::new();
        let mut next = self.state.clone();
        for item in &grant.items {
            token(&item.subject_sha256)?;
            token(&item.read_sha256)?;
            if !seen.insert(item.proposal.as_str()) {
                return Err("outbox batch lists one proposal twice".into());
            }
            let row = self
                .state
                .outbox
                .records
                .get(&item.proposal)
                .ok_or("outbox batch proposal is unavailable")?
                .clone();
            let subject = row
                .subject
                .as_ref()
                .ok_or("outbox original subject was minimized")?;
            let message = &subject.proposal.message;
            if row.subject_sha256 != item.subject_sha256
                || row.phase != Phase::Proposed
                || row.count_consumed
                || row.mode != grant.mode
                || message.template != grant.template
                || !matches!(row.kind, MessageKind::FirstMessage | MessageKind::FollowUp)
                || (grant.mode == Mode::Live && subject.proposal.certification_reference.is_none())
                || row.expires_at > grant.expires_at
            {
                return Err("outbox batch item must be an exact proposed live certified first message or follow-up on the batch template".into());
            }
            if must_read_all
                && item.read_sha256
                    != digest(format!("{}\n{}", item.subject_sha256, message.subject).as_bytes())
            {
                return Err("outbox batch requires the owner's read receipt for every item in the first five batches".into());
            }
            self.outbox_current(access, subject, keys)?;
            let record = next.outbox.records.get_mut(&item.proposal).unwrap();
            record.decision = Some(Decision {
                subject_sha256: item.subject_sha256.clone(),
                owner: access.principal().into(),
                approved: true,
                at: now,
            });
            record.phase = Phase::Approved;
        }
        let grant_sha256 = grant.sha256()?;
        next.outbox.batches.granted += 1;
        next.outbox.batches.grants.insert(
            grant.id.clone(),
            Record {
                grant,
                grant_sha256,
                owner: access.principal().into(),
                granted_at: now,
                phase: GrantPhase::Active,
                reference_sha256: None,
            },
        );
        Ok(next)
    }
    /// A batch item dispatches only while its grant is active and unexpired.
    pub(super) fn outbox_batch_current(
        &self,
        proposal: &str,
        subject_sha256: &str,
        now: u64,
    ) -> Result<()> {
        if let Some(record) = self.state.outbox.batches.covering(proposal, subject_sha256)
            && (record.phase != GrantPhase::Active || record.grant.expires_at <= now)
        {
            return Err("outbox batch grant expired, was revoked, or was reset".into());
        }
        Ok(())
    }
}
pub(super) fn revoke(
    next: &mut super::State,
    id: &str,
    reference_sha256: String,
    now: u64,
) -> Result<()> {
    token(&reference_sha256)?;
    let record = next
        .outbox
        .batches
        .grants
        .get_mut(id)
        .ok_or("outbox batch grant is unavailable")?;
    if record.phase != GrantPhase::Active {
        return Err("outbox batch grant is not active".into());
    }
    record.phase = GrantPhase::Revoked;
    record.reference_sha256 = Some(reference_sha256);
    for item in record.grant.items.clone() {
        if let Some(row) = next.outbox.records.get_mut(&item.proposal)
            && row.phase == Phase::Approved
            && !row.count_consumed
        {
            row.phase = Phase::Invalidated;
        }
    }
    let _ = now;
    Ok(())
}
pub(super) fn raise(
    next: &mut super::State,
    size: u32,
    owner_review_sha256: &str,
    now: u64,
) -> Result<()> {
    token(owner_review_sha256)?;
    let book = &next.outbox;
    let current = book.batches.size();
    let delivered_batch = book.batches.grants.values().any(|g| {
        g.granted_at < now
            && g.grant.items.iter().all(|i| {
                book.records
                    .get(&i.proposal)
                    .is_some_and(|r| r.count_consumed && r.phase == Phase::Delivered)
            })
    });
    if size
        != match current {
            5 => 10,
            10 => 20,
            _ => 0,
        }
        || size > CEILING
        || size > book.cap.max(5)
        || book.paused
        || !delivered_batch
    {
        return Err("outbox batch size rises one step per reviewed grant, needs a fully delivered batch, and never exceeds the floor cap".into());
    }
    next.outbox.batches.size = size;
    Ok(())
}

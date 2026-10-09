//! Verified offboarding for one service sale (#11013).
//!
//! The delivery handoff plans the cleanup; this book records what actually
//! happened to each planned item: removed, kept because something requires
//! it, not needed, still pending, or attempted with an unknown result. The
//! store records only what it can check against private evidence:
//!
//! - **Removed** needs the operation result document
//!   ([`OPERATION_SCHEMA`]: which target, by which operation, with what
//!   result) and a later re-read document ([`REREAD_SCHEMA`]) that observes
//!   the same target absent. Both are reread from the private evidence root
//!   by exact digest; the times recorded come from those documents, never
//!   from the caller.
//! - **Kept** needs an end date in the future, the requirement, and the
//!   requirement's evidence. Once the date passes, the item reads as due for
//!   removal again rather than as kept.
//! - **Not required** needs an explicit reason; the recording owner is the
//!   reviewer.
//! - **Pending** and **Unknown** need a next action and due date; Unknown
//!   also needs the evidence of the attempted effect. Neither is done.
//!
//! An item missing from every report is never done. Records live on the lead
//! next to their sale, so they share its lock, revision, audit, retention,
//! recipients, deletion, and suppression. Only the sales owner records one.
use super::{Access, Lead, Result, Store, service::Reader};
use receipts::service_sale::Reference;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub const OPERATION_SCHEMA: &str = "openagents.sales.cleanup-operation.v1";
pub const REREAD_SCHEMA: &str = "openagents.sales.cleanup-reread.v1";
const HANDOFF_SCHEMA: &str = "openagents.sales.delivery-handoff.v1";
pub const MAX_ITEMS: usize = 32;
const RESULTS: [&str; 4] = ["removed", "revoked", "cancelled", "deleted"];

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Decision {
    Removed,
    Kept,
    NotRequired,
    Pending,
    Unknown,
}

/// One item of an owner's cleanup report, as submitted.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ItemInput {
    pub plan_item: String,
    pub decision: Option<Decision>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_evidence: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reread_evidence: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kept_until: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirement_evidence: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_evidence: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_action_due_at: Option<u64>,
}

/// An owner's cleanup report for one sale's exact handoff.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Report {
    /// Must equal the sale's handoff digest.
    pub handoff_sha256: String,
    pub items: Vec<ItemInput>,
}

/// One planned cleanup item, copied from the handoff when first recorded.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PlanItem {
    pub id: String,
    pub class: String,
    pub target: String,
    pub operation: String,
    pub due_at: Option<u64>,
}

/// What a removal did and the re-read that shows the target gone.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Removal {
    pub target: String,
    pub operation: String,
    pub result: String,
    pub operation_evidence: Reference,
    pub removed_at: u64,
    pub reread_evidence: Reference,
    pub reread_at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Keep {
    pub until: u64,
    pub requirement: String,
    pub evidence: Reference,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Next {
    pub action: String,
    pub due_at: u64,
}

/// One checked decision about one plan item.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub plan_item: String,
    pub decision: Decision,
    pub decided_by: String,
    pub decided_at: u64,
    pub command_digest: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub removal: Option<Removal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<Keep>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attempt_evidence: Option<Reference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next: Option<Next>,
}

/// The offboarding record for one sale. Later reports replace items by plan
/// item; the audit keeps every command.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub handoff_sha256: String,
    pub plan: Vec<PlanItem>,
    pub items: BTreeMap<String, Item>,
    pub updated_at: u64,
}

/// One plan item's current state as a reader sees it.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
pub enum State {
    /// Removed, and a later re-read showed it gone.
    Removed {
        at: u64,
        operation: String,
    },
    /// Kept because something requires it, until this date.
    Kept {
        until: u64,
        requirement: String,
    },
    /// Was kept until this date, which has passed; removal is due again.
    KeepEnded {
        until: u64,
    },
    NotRequired {
        reason: String,
    },
    Pending {
        action: String,
        due_at: u64,
    },
    /// An attempt was made and its result is not known.
    Unknown {
        action: String,
        due_at: u64,
    },
    /// Nothing recorded for this item.
    NotRecorded,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Row {
    pub id: String,
    pub class: String,
    pub due_at: Option<u64>,
    pub state: State,
}

/// A sale's offboarding as a reader sees it: no paths or evidence bodies.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub sale: String,
    pub handoff_sha256: String,
    pub rows: Vec<Row>,
    pub updated_at: u64,
}

/// A UTC calendar date, or the raw seconds when out of range.
pub fn date(at: u64) -> String {
    i64::try_from(at)
        .ok()
        .and_then(|s| jiff::Timestamp::from_second(s).ok())
        .map_or_else(
            || at.to_string(),
            |t| t.to_zoned(jiff::tz::TimeZone::UTC).date().to_string(),
        )
}

impl State {
    /// The state in plain words.
    pub fn plain(&self) -> String {
        match self {
            Self::Removed { at, .. } => format!("Removed {}", date(*at)),
            Self::Kept { until, .. } => format!("Kept until {} (required)", date(*until)),
            Self::KeepEnded { until } => {
                format!("Not yet removed (was kept until {})", date(*until))
            }
            Self::NotRequired { .. } => "Not needed".into(),
            Self::Pending { due_at, .. } => format!("Not yet removed (due {})", date(*due_at)),
            Self::Unknown { due_at, .. } => {
                format!("Removal not confirmed (check by {})", date(*due_at))
            }
            Self::NotRecorded => "Not yet removed".into(),
        }
    }

    /// Whether nothing is left to do for this item.
    pub fn done(&self) -> bool {
        matches!(
            self,
            Self::Removed { .. } | Self::Kept { .. } | Self::NotRequired { .. }
        )
    }
}

impl View {
    /// One line for a list: how many items are removed, kept, or open.
    pub fn summary(&self) -> String {
        let removed = self
            .rows
            .iter()
            .filter(|r| matches!(r.state, State::Removed { .. }))
            .count();
        let kept = self
            .rows
            .iter()
            .filter(|r| matches!(r.state, State::Kept { .. }))
            .count();
        let open = self.rows.iter().filter(|r| !r.state.done()).count();
        let mut parts = vec![format!("{removed} removed")];
        if kept > 0 {
            parts.push(format!("{kept} kept as required"));
        }
        if open > 0 {
            parts.push(format!("{open} not yet removed"));
        } else {
            parts.push("nothing left to remove".into());
        }
        parts.join(", ")
    }
}

impl Record {
    pub fn view(&self, sale: &str, now: u64) -> View {
        let rows = self
            .plan
            .iter()
            .map(|p| Row {
                id: p.id.clone(),
                class: p.class.clone(),
                due_at: p.due_at,
                state: self
                    .items
                    .get(&p.id)
                    .map_or(State::NotRecorded, |i| i.state(now)),
            })
            .collect();
        View {
            sale: sale.into(),
            handoff_sha256: self.handoff_sha256.clone(),
            rows,
            updated_at: self.updated_at,
        }
    }

    pub(super) fn validate(&self) -> Result<()> {
        if self.plan.is_empty() || self.plan.len() > MAX_ITEMS || self.items.len() > MAX_ITEMS {
            return Err("offboarding record exceeds its bounds".into());
        }
        let ids: BTreeSet<&str> = self.plan.iter().map(|p| p.id.as_str()).collect();
        if ids.len() != self.plan.len()
            || self
                .items
                .iter()
                .any(|(k, i)| k != &i.plan_item || !ids.contains(k.as_str()) || !i.complete())
        {
            return Err("offboarding record disagrees with its plan".into());
        }
        Ok(())
    }
}

impl Item {
    fn state(&self, now: u64) -> State {
        match (self.decision, &self.removal, &self.keep, &self.next) {
            (Decision::Removed, Some(r), _, _) => State::Removed {
                at: r.reread_at,
                operation: r.operation.clone(),
            },
            (Decision::Kept, _, Some(k), _) if k.until > now => State::Kept {
                until: k.until,
                requirement: k.requirement.clone(),
            },
            (Decision::Kept, _, Some(k), _) => State::KeepEnded { until: k.until },
            (Decision::NotRequired, ..) => State::NotRequired {
                reason: self.reason.clone().unwrap_or_default(),
            },
            (Decision::Pending, _, _, Some(n)) => State::Pending {
                action: n.action.clone(),
                due_at: n.due_at,
            },
            (Decision::Unknown, _, _, Some(n)) => State::Unknown {
                action: n.action.clone(),
                due_at: n.due_at,
            },
            _ => State::NotRecorded,
        }
    }

    /// Whether the item carries the evidence its decision needs.
    fn complete(&self) -> bool {
        match self.decision {
            Decision::Removed => self.removal.is_some(),
            Decision::Kept => self.keep.is_some(),
            Decision::NotRequired => self.reason.is_some(),
            Decision::Pending => self.next.is_some(),
            Decision::Unknown => self.next.is_some() && self.attempt_evidence.is_some(),
        }
    }
}

fn field(doc: &Value, name: &str) -> String {
    doc[name].as_str().unwrap_or_default().to_owned()
}

fn plan(bytes: &[u8]) -> Result<Vec<PlanItem>> {
    let doc: Value =
        serde_json::from_slice(bytes).map_err(|_| "delivery handoff is not readable")?;
    if doc["schema"] != HANDOFF_SCHEMA {
        return Err("delivery handoff is not readable".into());
    }
    let items = doc["cleanup_plan"].as_array().cloned().unwrap_or_default();
    if items.is_empty() || items.len() > MAX_ITEMS {
        return Err("the delivery handoff has no bounded cleanup plan".into());
    }
    let mut seen = BTreeSet::new();
    let mut plan = Vec::new();
    for item in &items {
        let p = PlanItem {
            id: field(item, "id"),
            class: field(item, "class"),
            target: field(item, "target_reference"),
            operation: field(item, "operation_reference"),
            due_at: item["due_at"].as_u64(),
        };
        super::id(&p.id)?;
        if !seen.insert(p.id.clone()) {
            return Err("the cleanup plan repeats an item".into());
        }
        plan.push(p);
    }
    Ok(plan)
}

fn document(reader: &mut Reader, r: &Reference, schema: &str) -> Result<Value> {
    let doc: Value = serde_json::from_slice(&reader.read(r)?)
        .map_err(|_| "cleanup evidence is not a readable document")?;
    if doc["schema"] != schema {
        return Err("cleanup evidence has the wrong schema".into());
    }
    Ok(doc)
}

fn next(input: &ItemInput, now: u64) -> Result<Next> {
    let (Some(action), Some(due_at)) = (&input.next_action, input.next_action_due_at) else {
        return Err("an open cleanup item needs a next action and due date".into());
    };
    super::text(action, 256)?;
    if due_at <= now {
        return Err("an open cleanup item needs a future due date".into());
    }
    Ok(Next {
        action: action.clone(),
        due_at,
    })
}

/// Check one removal: the operation result names the planned target and
/// operation, and a later re-read observed that same target absent.
fn removal(reader: &mut Reader, p: &PlanItem, input: &ItemInput, now: u64) -> Result<Removal> {
    let (Some(op_ref), Some(reread_ref)) = (&input.operation_evidence, &input.reread_evidence)
    else {
        return Err(
            "a removal needs the operation result and a re-read showing the item gone".into(),
        );
    };
    let op = document(reader, op_ref, OPERATION_SCHEMA)?;
    let reread = document(reader, reread_ref, REREAD_SCHEMA)?;
    let target = field(&op, "target");
    let operation = field(&op, "operation");
    let result = field(&op, "result");
    let removed_at = op["at"].as_u64().ok_or("cleanup operation has no time")?;
    let reread_at = reread["at"].as_u64().ok_or("cleanup re-read has no time")?;
    super::text(&target, 256)?;
    super::text(&operation, 256)?;
    if field(&op, "plan_item") != p.id
        || field(&reread, "plan_item") != p.id
        || (!p.target.is_empty() && target != p.target)
        || (!p.operation.is_empty() && operation != p.operation)
        || field(&reread, "target") != target
    {
        return Err("cleanup evidence names a different item, target, or operation".into());
    }
    if !RESULTS.contains(&result.as_str()) {
        return Err("the cleanup operation did not report a removal".into());
    }
    if reread["observed"] != "absent" {
        return Err("the re-read does not show the item gone".into());
    }
    if removed_at > now || reread_at > now || reread_at < removed_at {
        return Err("the re-read must follow the removal and not be in the future".into());
    }
    Ok(Removal {
        target,
        operation,
        result,
        operation_evidence: op_ref.clone(),
        removed_at,
        reread_evidence: reread_ref.clone(),
        reread_at,
    })
}

impl Store {
    /// Check an owner's cleanup report against the sale's exact handoff and
    /// the private evidence, and return the record it produces.
    pub(super) fn record_offboarding(
        &self,
        access: &Access,
        lead: &Lead,
        sale: &str,
        report: &Report,
        command_digest: &str,
        root: Option<&Path>,
        now: u64,
    ) -> Result<Record> {
        let found = lead
            .service_sales
            .get(sale)
            .ok_or("service sale is unavailable")?;
        if found.retain_until <= now
            || !found
                .admitted_recipients
                .contains(&format!("human:{}", access.principal()))
        {
            return Err("service access exceeds its admitted retention or recipients".into());
        }
        let handoff = &found.admission.sources.handoff;
        if report.handoff_sha256 != handoff.sha256 {
            return Err("the cleanup report names a different handoff".into());
        }
        if report.items.is_empty() || report.items.len() > MAX_ITEMS {
            return Err("a cleanup report needs 1 to 32 items".into());
        }
        let mut reader = Reader::new(root)?;
        let plan = plan(&reader.read(handoff)?)?;
        let mut record = match lead.offboarding.get(sale) {
            Some(prior) if prior.plan == plan => prior.clone(),
            Some(_) => return Err("the recorded cleanup plan changed".into()),
            None => Record {
                handoff_sha256: handoff.sha256.clone(),
                plan: plan.clone(),
                items: BTreeMap::new(),
                updated_at: now,
            },
        };
        let mut seen = BTreeSet::new();
        for input in &report.items {
            let p = plan
                .iter()
                .find(|p| p.id == input.plan_item)
                .ok_or("the cleanup report names an item outside the plan")?;
            if !seen.insert(p.id.clone()) {
                return Err("the cleanup report repeats an item".into());
            }
            let decision = input.decision.ok_or("each cleanup item needs a decision")?;
            let mut item = Item {
                plan_item: p.id.clone(),
                decision,
                decided_by: access.principal().into(),
                decided_at: now,
                command_digest: command_digest.into(),
                removal: None,
                keep: None,
                reason: None,
                attempt_evidence: None,
                next: None,
            };
            match decision {
                Decision::Removed => item.removal = Some(removal(&mut reader, p, input, now)?),
                Decision::Kept => {
                    let (Some(until), Some(requirement), Some(evidence)) = (
                        input.kept_until,
                        &input.requirement,
                        &input.requirement_evidence,
                    ) else {
                        return Err(
                            "keeping an item needs an end date, the requirement, and its evidence"
                                .into(),
                        );
                    };
                    super::text(requirement, 256)?;
                    if until <= now {
                        return Err("a kept item needs a future end date".into());
                    }
                    reader.read(evidence)?;
                    item.keep = Some(Keep {
                        until,
                        requirement: requirement.clone(),
                        evidence: evidence.clone(),
                    });
                }
                Decision::NotRequired => {
                    let reason = input
                        .reason
                        .as_ref()
                        .ok_or("an item that is not needed needs an explicit reason")?;
                    super::text(reason, 256)?;
                    item.reason = Some(reason.clone());
                }
                Decision::Pending => item.next = Some(next(input, now)?),
                Decision::Unknown => {
                    let attempt = input
                        .attempt_evidence
                        .as_ref()
                        .ok_or("an unknown result needs the attempt's evidence")?;
                    reader.read(attempt)?;
                    item.attempt_evidence = Some(attempt.clone());
                    item.next = Some(next(input, now)?);
                }
            }
            record.items.insert(p.id.clone(), item);
        }
        record.updated_at = now;
        record.validate()?;
        Ok(record)
    }

    /// One sale's offboarding view, fenced like any service read.
    pub fn offboarding_show(
        &mut self,
        access: &Access,
        lead: &str,
        sale: &str,
    ) -> Result<Option<View>> {
        let record = self.show(access, lead)?;
        if !record.service_sales.contains_key(sale) {
            return Err("service sale is unavailable".into());
        }
        let now = (self.clock)();
        Ok(record.offboarding.get(sale).map(|r| r.view(sale, now)))
    }
}

#[cfg(test)]
#[path = "offboarding/tests.rs"]
pub(crate) mod tests;

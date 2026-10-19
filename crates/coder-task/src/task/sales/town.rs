//! The sales floor's bodies in Everglade (REV-70): where Paul and the
//! admitted hires stand in the Agora and what each is doing, projected from
//! the canonical sales books.
//!
//! The adapter is Bob's town work for the floor: a pinned station table
//! ([`Table`]) binding the Agora's world-tree objects to the roles that
//! stand at them, a deterministic placement of an admitted roster over it
//! ([`place`]), and a mapping from real work to the node and activity word a
//! body shows ([`Store::town_bodies`]). Every activity cites one canonical
//! source: an assignment, a draft, a lead's due follow-up, a reply awaiting
//! review, a certification, or a hire proposal. A member with no work idles
//! at its station; a paused or retired member shows no work at all. The
//! adapter reads only. It holds no budget, cap, or clock of its own, so a
//! town routine can neither reset a limit nor authorize an effect.

use super::{Access, Result, Store, agents};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use world_tree::{Affordance, Tree};

pub const TABLE_SCHEMA: &str = "openagents.sales-town-table.v1";
pub const BODIES_SCHEMA: &str = "openagents.sales-town-bodies.v1";
/// The pack's desks on the trading floor (`layout/agora.rs` `DESKS`).
pub const DESKS: usize = 18;

/// Who stands at a station.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Leader,
    Hire,
}

/// One station: a world-tree object the floor uses, by its layout source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Station {
    /// The layout source the tree node carries, such as `agora:desk:3`.
    pub source: String,
    pub role: Role,
    /// What work the station must offer.
    pub needs: Affordance,
}

/// The pinned station table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    pub schema: String,
    pub leader: Station,
    pub desks: Vec<Station>,
    pub decision: Station,
    pub standup: Station,
    pub booths: Station,
    pub whiteboard: Station,
}

impl Table {
    /// The Agora as built: Paul's workstation, eighteen desks, the owner's
    /// lectern, the stand-up spot, the booths, and the whiteboard.
    #[must_use]
    pub fn agora() -> Self {
        let station = |source: &str, role, needs| Station {
            source: source.into(),
            role,
            needs,
        };
        Self {
            schema: TABLE_SCHEMA.into(),
            leader: station("agora:paul", Role::Leader, Affordance::Review),
            desks: (0..DESKS)
                .map(|i| station(&format!("agora:desk:{i}"), Role::Hire, Affordance::Work))
                .collect(),
            decision: station("agora:owner", Role::Leader, Affordance::Approve),
            standup: station("agora:standup", Role::Hire, Affordance::Gather),
            booths: station("agora:booths", Role::Hire, Affordance::Practice),
            whiteboard: station("agora:teacher", Role::Leader, Affordance::Teach),
        }
    }

    fn stations(&self) -> impl Iterator<Item = &Station> {
        [
            &self.leader,
            &self.decision,
            &self.standup,
            &self.booths,
            &self.whiteboard,
        ]
        .into_iter()
        .chain(self.desks.iter())
    }

    /// The table's digest over its JSON form.
    #[must_use]
    pub fn digest(&self) -> String {
        let json = serde_json::to_vec(self).unwrap_or_default();
        hex(&Sha256::digest(json))
    }

    /// Every way the table disagrees with `tree`: a station with no node,
    /// a node without the affordance, a desk count other than the pack's,
    /// or two stations on one node. Empty when the table is good.
    #[must_use]
    pub fn validate(&self, tree: &Tree) -> Vec<String> {
        let mut problems = Vec::new();
        if self.schema != TABLE_SCHEMA {
            problems.push(format!("schema is {}, not {TABLE_SCHEMA}", self.schema));
        }
        if self.desks.len() != DESKS {
            problems.push(format!("{} desks, not {DESKS}", self.desks.len()));
        }
        let mut seen = BTreeMap::new();
        for station in self.stations() {
            let Some(node) = tree.by_source(&station.source) else {
                problems.push(format!("{} has no node in the tree", station.source));
                continue;
            };
            if !node.offers(station.needs) {
                problems.push(format!("{} does not offer {:?}", node.id, station.needs));
            }
            if let Some(other) = seen.insert(node.id.clone(), station.source.clone()) {
                problems.push(format!(
                    "{} and {} share {}",
                    other, station.source, node.id
                ));
            }
        }
        problems
    }

    /// The node a station stands at, in `tree`.
    fn node(&self, tree: &Tree, station: &Station) -> Option<String> {
        tree.by_source(&station.source).map(|n| n.id.clone())
    }
}

/// An admitted member's lifecycle, as the crew book has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lifecycle {
    Active,
    Paused,
    Retired,
}

/// A floor member the caller admits: Paul or a confirmed hire. The name
/// and key are the crew's; the adapter never makes one.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    pub pubkey: String,
    pub role: Role,
    pub lifecycle: Lifecycle,
}

/// Where a member stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Placement {
    Placed {
        station: String,
        node: String,
    },
    /// Admitted, but every desk is taken; host work continues unplaced.
    Unplaced,
}

/// Places `roster` over `table` in `tree`: the leader at the leader's
/// station, hires by name over the desks in order, the rest unplaced.
/// Retired members take no station. The same roster, table, and tree
/// always place the same way.
#[must_use]
pub fn place(roster: &[Member], table: &Table, tree: &Tree) -> BTreeMap<String, Placement> {
    let mut out = BTreeMap::new();
    let mut hires: Vec<&Member> = roster
        .iter()
        .filter(|m| m.role == Role::Hire && m.lifecycle != Lifecycle::Retired)
        .collect();
    hires.sort_by(|a, b| a.name.cmp(&b.name).then(a.pubkey.cmp(&b.pubkey)));
    hires.dedup_by(|a, b| a.name == b.name);
    for (i, hire) in hires.iter().enumerate() {
        let placement = match (
            table.desks.get(i),
            table.desks.get(i).and_then(|d| table.node(tree, d)),
        ) {
            (Some(desk), Some(node)) => Placement::Placed {
                station: desk.source.clone(),
                node,
            },
            _ => Placement::Unplaced,
        };
        out.insert(hire.name.clone(), placement);
    }
    for leader in roster
        .iter()
        .filter(|m| m.role == Role::Leader && m.lifecycle != Lifecycle::Retired)
    {
        let placement = match table.node(tree, &table.leader) {
            Some(node) => Placement::Placed {
                station: table.leader.source.clone(),
                node,
            },
            None => Placement::Unplaced,
        };
        out.entry(leader.name.clone()).or_insert(placement);
    }
    out
}

/// One piece of real work a body may show.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Work {
    /// `assignment:REF`, `draft:REF`, `lead-next:ID`, `reply:ID`,
    /// `certification:ID`, or `hire:ID`.
    pub source: String,
    /// The word the body's screens show.
    pub activity: String,
    /// The station the work happens at.
    pub station: String,
    /// When it fell due, for ordering; the earliest shows.
    pub at: u64,
}

/// A body on the floor.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Body {
    pub name: String,
    pub pubkey: String,
    pub role: Role,
    pub lifecycle: Lifecycle,
    pub placement: Placement,
    /// The work under way, or none: an idle body stands at its station.
    pub current: Option<Work>,
    /// Everything else that cites this member, earliest first.
    pub queued: Vec<Work>,
    pub idle: bool,
}

/// The floor's bodies at `generated_at`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bodies {
    pub schema: String,
    pub generated_at: u64,
    pub owner: String,
    pub zone: String,
    pub layout_digest: String,
    pub table_digest: String,
    pub bodies: Vec<Body>,
    /// Work that names no admitted member; it stays in the host queue.
    pub unattributed: Vec<Work>,
}

/// A hire proposal the owner has not decided yet, as the caller reads it
/// from the crew's hiring book.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingHire {
    pub id: String,
    pub proposed_at: u64,
}

/// Paul and the hires the crew's hiring book admits: a confirmed hire is
/// active until a confirmed retirement names it; a pending proposal waits on
/// the owner. The pubkey is the one the decision's outcome recorded, or
/// empty when the crew record alone holds it.
#[must_use]
pub fn roster_from_book(book: &crate::task::agent_hiring::Book) -> (Vec<Member>, Vec<PendingHire>) {
    use crate::task::agent_hiring::Status;
    use coder_host::access::crew::HireAction;
    let mut members: BTreeMap<String, Member> = BTreeMap::new();
    let mut pending = Vec::new();
    let mut entries: Vec<_> = book.entries.values().collect();
    entries.sort_by_key(|e| {
        (
            e.decision.as_ref().map_or(e.proposed_at, |d| d.at),
            e.sha256.clone(),
        )
    });
    for entry in entries {
        match (&entry.status, &entry.proposal.action) {
            (Status::Pending, _) => pending.push(PendingHire {
                id: entry.proposal.id.clone(),
                proposed_at: entry.proposed_at,
            }),
            (Status::Confirmed, HireAction::Hire { name, .. }) => {
                let pubkey = entry
                    .decision
                    .as_ref()
                    .and_then(|d| d.outcome.pointer("/created/pubkey"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                members.insert(
                    name.clone(),
                    Member {
                        name: name.clone(),
                        pubkey,
                        role: Role::Hire,
                        lifecycle: Lifecycle::Active,
                    },
                );
            }
            (Status::Confirmed, HireAction::Retire { name }) => {
                if let Some(member) = members.get_mut(name) {
                    member.lifecycle = Lifecycle::Retired;
                }
            }
            (Status::Rejected | Status::Expired, _) => {}
        }
    }
    let mut roster = vec![Member {
        name: "paul".into(),
        pubkey: String::new(),
        role: Role::Leader,
        lifecycle: Lifecycle::Active,
    }];
    roster.extend(members.into_values());
    (roster, pending)
}

impl Store {
    /// The floor's bodies for the owner: `roster` placed over the Agora,
    /// each with the real work the books attribute to it.
    pub fn town_bodies(
        &mut self,
        owner: &Access,
        roster: &[Member],
        pending_hires: &[PendingHire],
    ) -> Result<Bodies> {
        self.refresh()?;
        self.admin(owner)?;
        let now = (self.clock)();
        let tree = world_tree::everglade();
        let table = Table::agora();
        Ok(self.project_town(owner, roster, pending_hires, &table, tree, now))
    }

    pub(super) fn project_town(
        &self,
        owner: &Access,
        roster: &[Member],
        pending_hires: &[PendingHire],
        table: &Table,
        tree: &Tree,
        now: u64,
    ) -> Bodies {
        let placements = place(roster, table, tree);
        let mut work: BTreeMap<String, Vec<Work>> = BTreeMap::new();
        let mut unattributed = Vec::new();
        let mut push = |name: Option<&str>, item: Work| match name
            .filter(|n| roster.iter().any(|m| &m.name == n))
        {
            Some(name) => work.entry(name.to_string()).or_default().push(item),
            None => unattributed.push(item),
        };
        let desk_of = |name: &str| match placements.get(name) {
            Some(Placement::Placed { station, .. }) => station.clone(),
            _ => table.standup.source.clone(),
        };
        let state = &self.state;
        for lead in state.leads.values() {
            let mut assignee: Option<&agents::Anchor> = None;
            for assignment in lead
                .agent_records
                .assignments
                .values()
                .filter(|a| a.active && a.expires_at > now)
            {
                assignee = Some(&assignment.anchor);
                push(
                    Some(&assignment.anchor.name),
                    Work {
                        source: format!("assignment:{}", assignment.reference),
                        activity: "research".into(),
                        station: desk_of(&assignment.anchor.name),
                        at: assignment.issued_at,
                    },
                );
            }
            if let Some(next) = &lead.details.next {
                if next.due_at <= now {
                    let name = assignee.map(|a| a.name.as_str());
                    push(
                        name,
                        Work {
                            source: format!("lead-next:{}", lead.id),
                            activity: "follow up".into(),
                            station: name
                                .map(desk_of)
                                .unwrap_or_else(|| table.standup.source.clone()),
                            at: next.due_at,
                        },
                    );
                }
            }
            for draft in lead.agent_records.drafts.values() {
                match draft.state {
                    agents::DraftState::Proposed => push(
                        Some(&draft.author.name),
                        Work {
                            source: format!("draft:{}", draft.reference),
                            activity: "draft".into(),
                            station: desk_of(&draft.author.name),
                            at: draft.proposed_at,
                        },
                    ),
                    agents::DraftState::OwnerReviewed | agents::DraftState::Rejected => {}
                }
            }
        }
        for record in state.outbox.records.values() {
            if record.phase == super::outbox::Phase::Proposed {
                push(
                    Some(record.actor.as_str()),
                    Work {
                        source: format!("message:{}", record.id),
                        activity: "await decision".into(),
                        station: table.decision.source.clone(),
                        at: record.created_at,
                    },
                );
            }
        }
        for record in state.replies.records.values() {
            if record.owner_label.is_none()
                && record
                    .model_classification
                    .as_ref()
                    .is_none_or(|c| c.owner_review_required)
            {
                let name = record
                    .lead
                    .as_ref()
                    .and_then(|l| state.leads.get(l))
                    .and_then(|lead| {
                        lead.agent_records
                            .assignments
                            .values()
                            .find(|a| a.active && a.expires_at > now)
                            .map(|a| a.anchor.name.clone())
                    });
                push(
                    name.as_deref(),
                    Work {
                        source: format!("reply:{}", record.id),
                        activity: "classify reply".into(),
                        station: name
                            .as_deref()
                            .map(desk_of)
                            .unwrap_or_else(|| table.standup.source.clone()),
                        at: record.received_at,
                    },
                );
            }
        }
        for record in state.agents.certificates.values() {
            let cert = &record.certification;
            let activity = match cert.state {
                agents::CertState::InTraining => "role-play",
                agents::CertState::Suspended => "retake",
                agents::CertState::OwnerMarked | agents::CertState::Qualified => continue,
            };
            push(
                Some(&cert.agent.name),
                Work {
                    source: format!("certification:{}", cert.id),
                    activity: activity.into(),
                    station: table.booths.source.clone(),
                    at: record.recorded_at,
                },
            );
        }
        for hire in pending_hires {
            let leader = roster.iter().find(|m| m.role == Role::Leader);
            push(
                leader.map(|m| m.name.as_str()),
                Work {
                    source: format!("hire:{}", hire.id),
                    activity: "await decision".into(),
                    station: table.decision.source.clone(),
                    at: hire.proposed_at,
                },
            );
        }
        let mut bodies = Vec::new();
        for member in roster {
            let placement = placements
                .get(&member.name)
                .cloned()
                .unwrap_or(Placement::Unplaced);
            let mut items = work.remove(&member.name).unwrap_or_default();
            items.sort_by(|a, b| a.at.cmp(&b.at).then(a.source.cmp(&b.source)));
            items.dedup_by(|a, b| a.source == b.source);
            let (current, queued) = if member.lifecycle == Lifecycle::Active {
                let mut items = items.into_iter();
                (items.next(), items.collect())
            } else {
                (None, items)
            };
            bodies.push(Body {
                name: member.name.clone(),
                pubkey: member.pubkey.clone(),
                role: member.role,
                lifecycle: member.lifecycle,
                placement,
                idle: current.is_none(),
                current,
                queued,
            });
        }
        unattributed.sort_by(|a, b| a.at.cmp(&b.at).then(a.source.cmp(&b.source)));
        unattributed.dedup_by(|a, b| a.source == b.source);
        Bodies {
            schema: BODIES_SCHEMA.into(),
            generated_at: now,
            owner: owner.principal.clone(),
            zone: tree.zone().to_string(),
            layout_digest: tree.digest().to_string(),
            table_digest: table.digest(),
            bodies,
            unattributed,
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests;

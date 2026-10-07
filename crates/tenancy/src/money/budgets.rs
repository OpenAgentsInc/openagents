//! Cumulative hierarchical caps on the existing monetary reservation journal.
//! Policy versions preserve previous liability and never restart a budget.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Hold, Phase};

pub const SCHEMA: &str = "openagents.money.budgets.v1";
pub const ROUTE: &str = "gateway-monetary-v1";
pub const SCALE: u64 = 1_000_000;
const MAX_PEOPLE: usize = 256;
const MAX_TEAMS: usize = 32;
const MAX_POLICIES: usize = 128;

/// The owner supplies the threshold in the same integer unit as the cap.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Limit {
    pub cap: u64,
    pub alert_at: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Person {
    pub team: String,
    pub limit: Limit,
}

/// An owner-reviewed subdivision of one native workspace, not another payer.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub schema: String,
    pub version: u64,
    pub currency: String,
    pub scale: u64,
    pub route: String,
    pub effective_from: u64,
    pub workspace: Limit,
    pub teams: BTreeMap<String, Limit>,
    pub people: BTreeMap<String, Person>,
}

impl Policy {
    pub fn check(&self) -> Result<(), String> {
        if self.schema != SCHEMA || self.version == 0 || self.scale != SCALE || self.route != ROUTE
        {
            return Err("budget schema, version, scale, or enabled route is invalid".into());
        }
        super::currency(&self.currency)?;
        if self.teams.is_empty()
            || self.teams.len() > MAX_TEAMS
            || self.people.is_empty()
            || self.people.len() > MAX_PEOPLE
        {
            return Err("budget roster exceeds its bound or is empty".into());
        }
        check_limit(&self.workspace)?;
        for (team, limit) in &self.teams {
            if team.len() > 64 {
                return Err("budget team identity exceeds its bound".into());
            }
            super::identity(team)?;
            check_limit(limit)?;
        }
        for (account, person) in &self.people {
            if !account
                .strip_prefix("acct_")
                .is_some_and(|hex| hex.len() == 16 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
            {
                return Err("budget person must be a native Accounts identity".into());
            }
            if !self.teams.contains_key(&person.team) {
                return Err("budget person names an unreviewed team".into());
            }
            check_limit(&person.limit)?;
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<String, String> {
        self.check()?;
        let bytes = serde_json::to_vec(self).map_err(|e| e.to_string())?;
        Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
    }
}

fn check_limit(limit: &Limit) -> Result<(), String> {
    if limit.alert_at > limit.cap {
        return Err("budget alert threshold exceeds its cap".into());
    }
    Ok(())
}

/// Historical native actor and policy pins; recovery never reassigns them.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Admission {
    pub person: String,
    pub team: String,
    pub policy: String,
    pub account_revision: String,
    pub member_epoch: u64,
    pub admitted_at: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    Workspace,
    Team,
    Person,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Alert {
    Clear,
    Threshold,
    Reached,
    Exceeded,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub level: Level,
    pub cap: u64,
    pub alert_at: u64,
    pub reserved: u64,
    pub unknown: u64,
    pub settled_net: u64,
    pub used: u64,
    pub remaining: u64,
    pub alert: Alert,
}

impl Position {
    fn new<'a>(
        level: Level,
        limit: &Limit,
        holds: impl Iterator<Item = &'a Hold>,
    ) -> Result<Self, String> {
        let (mut reserved, mut unknown, mut settled_net) = (0u64, 0u64, 0u64);
        for hold in holds {
            match hold.phase {
                Phase::Held => {
                    reserved = reserved
                        .checked_add(hold.reserved)
                        .ok_or("budget overflow")?
                }
                Phase::Unknown => {
                    unknown = unknown
                        .checked_add(hold.reserved)
                        .ok_or("budget overflow")?
                }
                Phase::Settled => {
                    settled_net = settled_net
                        .checked_add(
                            hold.retail_charge
                                .ok_or("unpriced settled budget")?
                                .checked_sub(hold.refunded)
                                .ok_or("invalid budget refund")?,
                        )
                        .ok_or("budget overflow")?
                }
                Phase::Released => (),
            }
        }
        let used = reserved
            .checked_add(unknown)
            .and_then(|n| n.checked_add(settled_net))
            .ok_or("budget overflow")?;
        Ok(Self {
            level,
            cap: limit.cap,
            alert_at: limit.alert_at,
            reserved,
            unknown,
            settled_net,
            used,
            remaining: limit.cap.saturating_sub(used),
            alert: if used > limit.cap {
                Alert::Exceeded
            } else if used == limit.cap {
                Alert::Reached
            } else if used >= limit.alert_at {
                Alert::Threshold
            } else {
                Alert::Clear
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Blocked {
    pub policy: String,
    pub currency: String,
    pub scale: u64,
    pub requested: u64,
    pub bound: Position,
}

impl std::fmt::Display for Blocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "The {:?} budget has {} millionths of {} remaining; this action requires {}.",
            self.bound.level, self.bound.remaining, self.currency, self.requested
        )
    }
}

/// A member projection contains only their own person and team, plus the payer cap.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct View {
    pub schema: String,
    pub policy: String,
    pub version: u64,
    pub effective_from: u64,
    pub activated_at: u64,
    pub currency: String,
    pub scale: u64,
    pub route: String,
    pub scope: String,
    /// Legacy exposure with no trustworthy native person/team pin counts
    /// conservatively against every child cap as well as the payer cap.
    pub unattributed_used: u64,
    pub workspace: Position,
    pub teams: BTreeMap<String, Position>,
    pub people: BTreeMap<String, Position>,
    pub requested: Option<u64>,
    pub blocked: Option<Blocked>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct Book {
    pub policies: BTreeMap<String, Policy>,
    pub active: String,
    pub activated: BTreeMap<String, u64>,
}

impl Book {
    pub fn install(&mut self, policy: &Policy, currency: &str, at: u64) -> Result<(), String> {
        let digest = policy.digest()?;
        if policy.currency != currency {
            return Err("budget currency differs from workspace currency".into());
        }
        if self.active == digest {
            return Ok(());
        }
        let version = self.policies.get(&self.active).map_or(0, |p| p.version);
        if policy.version != version.checked_add(1).ok_or("budget version overflow")?
            || self.policies.len() >= MAX_POLICIES
        {
            return Err("budget version must advance once within its history bound".into());
        }
        if policy.effective_from > at
            || self
                .policies
                .get(&self.active)
                .is_some_and(|p| policy.effective_from < p.effective_from)
        {
            return Err("budget effective time is future or precedes the retained policy".into());
        }
        self.policies.insert(digest.clone(), policy.clone());
        self.activated.insert(digest.clone(), at);
        self.active = digest;
        Ok(())
    }

    pub fn admission(
        &self,
        person: &str,
        revision: &str,
        member_epoch: u64,
        at: u64,
    ) -> Result<Admission, String> {
        let policy = self.current()?;
        if !revision
            .strip_prefix("sha256:")
            .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("budget membership revision is invalid".into());
        }
        let person_policy = policy
            .people
            .get(person)
            .ok_or("budget person is not in the reviewed roster")?;
        Ok(Admission {
            person: person.into(),
            team: person_policy.team.clone(),
            policy: self.active.clone(),
            account_revision: revision.into(),
            member_epoch,
            admitted_at: at,
        })
    }

    fn current(&self) -> Result<&Policy, String> {
        self.policies
            .get(&self.active)
            .ok_or_else(|| "budget policy is missing".into())
    }

    pub fn check(
        &self,
        holds: &BTreeMap<String, Hold>,
        admission: &Admission,
        amount: u64,
        at: u64,
    ) -> Result<Option<Blocked>, String> {
        let policy = self.current()?;
        if admission.policy != self.active
            || admission.admitted_at > at
            || at < policy.effective_from
            || self.admission(
                &admission.person,
                &admission.account_revision,
                admission.member_epoch,
                admission.admitted_at,
            )? != *admission
        {
            return Err("budget admission no longer matches the current reviewed policy".into());
        }
        let positions = self.positions(holds, &admission.person)?;
        Ok(positions
            .into_iter()
            .find(|p| p.used > p.cap || amount > p.remaining)
            .map(|bound| Blocked {
                policy: self.active.clone(),
                currency: policy.currency.clone(),
                scale: SCALE,
                requested: amount,
                bound,
            }))
    }

    fn positions(
        &self,
        holds: &BTreeMap<String, Hold>,
        person: &str,
    ) -> Result<Vec<Position>, String> {
        let policy = self.current()?;
        let person_policy = policy
            .people
            .get(person)
            .ok_or("budget person is not in the reviewed roster")?;
        Ok(vec![
            Position::new(Level::Workspace, &policy.workspace, holds.values())?,
            Position::new(
                Level::Team,
                &policy.teams[&person_policy.team],
                holds.values().filter(|h| {
                    h.budget.as_ref().is_none_or(|s| {
                        s.team == person_policy.team
                            || policy
                                .people
                                .get(&s.person)
                                .is_some_and(|p| p.team == person_policy.team)
                    })
                }),
            )?,
            Position::new(
                Level::Person,
                &person_policy.limit,
                holds
                    .values()
                    .filter(|h| h.budget.as_ref().is_none_or(|s| s.person == person)),
            )?,
        ])
    }

    pub fn view(
        &self,
        holds: &BTreeMap<String, Hold>,
        person: &str,
        admin: bool,
        requested: Option<u64>,
    ) -> Result<View, String> {
        let policy = self.current()?;
        let positions = self.positions(holds, person)?;
        let person_policy = &policy.people[person];
        let teams = policy
            .teams
            .iter()
            .filter(|(team, _)| admin || *team == &person_policy.team)
            .map(|(team, limit)| {
                Ok((
                    team.clone(),
                    Position::new(
                        Level::Team,
                        limit,
                        holds.values().filter(|h| {
                            h.budget.as_ref().is_none_or(|s| {
                                &s.team == team
                                    || policy
                                        .people
                                        .get(&s.person)
                                        .is_some_and(|p| &p.team == team)
                            })
                        }),
                    )?,
                ))
            })
            .collect::<Result<_, String>>()?;
        let people = policy
            .people
            .iter()
            .filter(|(account, _)| admin || *account == person)
            .map(|(account, p)| {
                Ok((
                    account.clone(),
                    Position::new(
                        Level::Person,
                        &p.limit,
                        holds
                            .values()
                            .filter(|h| h.budget.as_ref().is_none_or(|s| &s.person == account)),
                    )?,
                ))
            })
            .collect::<Result<_, String>>()?;
        let blocked = requested.and_then(|amount| {
            positions
                .into_iter()
                .find(|p| p.used > p.cap || amount > p.remaining)
                .map(|bound| Blocked {
                    policy: self.active.clone(),
                    currency: policy.currency.clone(),
                    scale: SCALE,
                    requested: amount,
                    bound,
                })
        });
        let unattributed_used = Position::new(
            Level::Workspace,
            &policy.workspace,
            holds.values().filter(|h| h.budget.is_none()),
        )?
        .used;
        Ok(View {
            schema: SCHEMA.into(),
            policy: self.active.clone(),
            version: policy.version,
            effective_from: policy.effective_from,
            activated_at: self.activated[&self.active],
            currency: policy.currency.clone(),
            scale: SCALE,
            route: ROUTE.into(),
            scope: if admin { "workspace" } else { "self-and-team" }.into(),
            unattributed_used,
            workspace: Position::new(Level::Workspace, &policy.workspace, holds.values())?,
            teams,
            people,
            requested,
            blocked,
        })
    }
}

#[cfg(test)]
mod tests;

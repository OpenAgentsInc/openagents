//! The GCE pool's compute budget (#12058): a spend cap over the last 24
//! hours and per run, a hard host limit, and the most idle minutes a host
//! may wait before it deletes itself. `openagents cloud budget` sets it,
//! `openagents cloud status` shows spend against it, and every path that
//! starts a host or a run asks [`admit`] first.
//!
//! - **The record.** `~/.openagents/cloud/budget.json`; without one the
//!   defaults hold ($300 over 24 hours, $10 a run, 8 hosts, 10 idle
//!   minutes).
//! - **Agents.** A process that names an agent (`--agent ID`, or
//!   `OPENAGENTS_AGENT_ID`) starts hosts only under a grant for that agent
//!   definition ID, with its own cap inside the pool's
//!   (`openagents cloud budget --agent ID --daily N`). Without one it is
//!   refused and told to ask the owner. Grants are keyed by agent ID, never
//!   by name.
//! - **Spend.** `~/.openagents/cloud/spend.json` holds every host this
//!   computer's pool had: its shape, when GCE created it, when it was last
//!   seen running, and when it was first seen gone. A host's cost is its
//!   list price ([`crate::pool::hourly_usd`]) from creation until it was
//!   seen gone (or now), at most the 12-hour backstop. A host that deletes
//!   itself between two listings counts until the second one, so the
//!   estimate errs high, never low.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::pool::{Host, Pool, hourly_usd, now};

pub const DEFAULT_DAILY_USD: f64 = 300.0;
pub const DEFAULT_PER_RUN_USD: f64 = 10.0;
pub const DEFAULT_HOST_LIMIT: u64 = 8;
/// The most hosts any budget may allow.
pub const HOST_LIMIT_CEILING: u64 = 32;
pub const DEFAULT_IDLE_LIMIT: u64 = 10;
/// The spend window: a rolling day.
pub const WINDOW_SECONDS: u64 = 24 * 3600;
/// The hosts' maximum run duration, the longest any host can bill.
const LONGEST_HOST_SECONDS: u64 = 12 * 3600 + 120;

fn home() -> PathBuf {
    crate::pool::cloud_home()
}

// ---------------------------------------------------------------------------
// The agent this process acts for.

static AGENT: OnceLock<Option<String>> = OnceLock::new();

/// Names the agent this process acts for (`--agent ID`). Only the first
/// call counts.
pub fn act_for(agent: Option<&str>) {
    let _ = AGENT.set(
        agent
            .map(str::trim)
            .filter(|a| !a.is_empty())
            .map(str::to_owned),
    );
}

/// The agent definition ID this process acts for: `--agent`, else
/// `OPENAGENTS_AGENT_ID`, else none (a person at a terminal).
pub fn agent() -> Option<String> {
    AGENT
        .get_or_init(|| {
            std::env::var("OPENAGENTS_AGENT_ID")
                .ok()
                .map(|a| a.trim().to_owned())
                .filter(|a| !a.is_empty())
        })
        .clone()
}

// ---------------------------------------------------------------------------
// The budget record.

/// One agent's grant, by agent definition ID.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AgentGrant {
    /// The most this agent's hosts may cost over 24 hours.
    pub daily_usd: f64,
    pub granted_at: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub schema: String,
    /// The most the pool's hosts may cost over the last 24 hours.
    pub daily_usd: f64,
    /// The most one run may cost (its slot's share of its host).
    pub per_run_usd: f64,
    /// The most hosts the pool may have at once; `--max-hosts` cannot pass it.
    pub max_hosts: u64,
    /// The most idle minutes a host may wait before it deletes itself.
    pub idle_minutes: u64,
    #[serde(default)]
    pub agents: BTreeMap<String, AgentGrant>,
    /// Unix seconds; `None` while the defaults hold.
    #[serde(default)]
    pub set_at: Option<u64>,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            schema: "openagents.cloud.budget.v1".into(),
            daily_usd: DEFAULT_DAILY_USD,
            per_run_usd: DEFAULT_PER_RUN_USD,
            max_hosts: DEFAULT_HOST_LIMIT,
            idle_minutes: DEFAULT_IDLE_LIMIT,
            agents: BTreeMap::new(),
            set_at: None,
        }
    }
}

impl Budget {
    fn path() -> PathBuf {
        home().join("budget.json")
    }

    /// The record on this computer, or the defaults.
    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Checks the limits are sane.
    pub fn check(&self) -> Result<(), String> {
        if !(self.daily_usd.is_finite() && self.daily_usd >= 0.0) {
            return Err("--daily is a dollar amount, 0 or more".into());
        }
        if !(self.per_run_usd.is_finite() && self.per_run_usd > 0.0) {
            return Err("--per-run is a dollar amount above 0".into());
        }
        if !(1..=HOST_LIMIT_CEILING).contains(&self.max_hosts) {
            return Err(format!("--max-hosts is 1 to {HOST_LIMIT_CEILING}"));
        }
        if !(1..=240).contains(&self.idle_minutes) {
            return Err("--idle-minutes is 1 to 240".into());
        }
        for (id, grant) in &self.agents {
            if !(grant.daily_usd.is_finite() && grant.daily_usd >= 0.0) {
                return Err(format!("agent {id}: --daily is a dollar amount, 0 or more"));
            }
        }
        Ok(())
    }

    /// How long one run may go on a host of this shape before it reaches
    /// the per-run cap.
    pub fn run_limit(&self, host: &Host, slots: u64) -> std::time::Duration {
        let per_hour = hourly_usd(&host.machine, host.spot) / slots.max(1) as f64;
        let hours = if per_hour > 0.0 {
            self.per_run_usd / per_hour
        } else {
            f64::from(u32::MAX)
        };
        std::time::Duration::from_secs_f64((hours * 3600.0).clamp(60.0, 1e9))
    }
}

// ---------------------------------------------------------------------------
// The spend ledger.

/// One host this computer's pool had.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HostSpend {
    pub name: String,
    pub pool: String,
    pub zone: String,
    pub machine: String,
    pub spot: bool,
    /// Unix seconds GCE created it.
    pub started: u64,
    /// Unix seconds it was last listed as running.
    pub last_running: u64,
    /// Unix seconds it was first listed as not running, or not listed.
    #[serde(default)]
    pub gone: Option<u64>,
    /// The agent definition ID that started it, if an agent did.
    #[serde(default)]
    pub agent: Option<String>,
}

impl HostSpend {
    /// Its estimated cost between `from` and `to` (Unix seconds).
    pub fn cost_between(&self, from: u64, to: u64) -> f64 {
        let end = self
            .gone
            .unwrap_or(to)
            .min(self.started + LONGEST_HOST_SECONDS);
        let seconds = end.min(to).saturating_sub(self.started.max(from));
        hourly_usd(&self.machine, self.spot) * seconds as f64 / 3600.0
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Ledger {
    #[serde(default)]
    pub hosts: Vec<HostSpend>,
}

impl Ledger {
    fn path() -> PathBuf {
        home().join("spend.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let temp = path.with_extension(format!("writing-{}", std::process::id()));
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&temp, text + "\n").map_err(|e| format!("{}: {e}", temp.display()))?;
        std::fs::rename(&temp, &path).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Changes the ledger under a lock, so concurrent runs don't drop each
    /// other's updates.
    fn update(change: impl FnOnce(&mut Self)) -> Result<(), String> {
        let dir = home();
        std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(dir.join("spend.lock"))
            .map_err(|e| format!("spend lock: {e}"))?;
        lock.lock().map_err(|e| format!("spend lock: {e}"))?;
        let mut ledger = Self::load();
        change(&mut ledger);
        // A host gone for more than a week no longer counts toward anything.
        let cutoff = now().saturating_sub(7 * 24 * 3600);
        ledger.hosts.retain(|h| h.gone.is_none_or(|g| g >= cutoff));
        let saved = ledger.save();
        let _ = lock.unlock();
        saved
    }

    /// The pool's estimated spend over the window ending `at`, for every
    /// host or for one agent's.
    pub fn spent(&self, at: u64, agent: Option<&str>) -> f64 {
        let from = at.saturating_sub(WINDOW_SECONDS);
        self.hosts
            .iter()
            .filter(|h| agent.is_none() || h.agent.as_deref() == agent)
            .map(|h| h.cost_between(from, at))
            // A float sum of nothing is -0.0; spend reads $0.00.
            .fold(0.0, |total, cost| total + cost)
    }

    /// Records what a listing of `pool` showed at `at`: running hosts are
    /// seen, hosts no longer running or no longer listed are gone, and a
    /// host the ledger lacks (started before it, or elsewhere) is added.
    pub fn observe(&mut self, pool: &str, listed: &[Host], at: u64) {
        for host in listed {
            let running = host.status == "RUNNING";
            match self.hosts.iter_mut().find(|h| h.name == host.name) {
                Some(entry) if entry.gone.is_none() => {
                    if running {
                        entry.last_running = at;
                    } else {
                        entry.gone = Some(at);
                    }
                }
                Some(_) => {}
                None if running => self.hosts.push(HostSpend {
                    name: host.name.clone(),
                    pool: pool.to_owned(),
                    zone: host.zone.clone(),
                    machine: host.machine.clone(),
                    spot: host.spot,
                    started: rfc3339_seconds(&host.created).unwrap_or(at),
                    last_running: at,
                    gone: None,
                    agent: None,
                }),
                None => {}
            }
        }
        for entry in self
            .hosts
            .iter_mut()
            .filter(|h| h.pool == pool && h.gone.is_none())
        {
            if !listed.iter().any(|h| h.name == entry.name) {
                entry.gone = Some(at);
            }
        }
    }
}

/// Records a host this process just started.
pub fn started(pool: &Pool, host: &Host) {
    started_for(pool, host, agent().as_deref());
}

/// Records a host for an explicit agent without changing process identity.
pub fn started_for(pool: &Pool, host: &Host, actor: Option<&str>) {
    let at = now();
    let agent = actor.map(str::to_owned);
    let _ = Ledger::update(|ledger| {
        ledger.hosts.retain(|h| h.name != host.name);
        ledger.hosts.push(HostSpend {
            name: host.name.clone(),
            pool: pool.pool.clone(),
            zone: host.zone.clone(),
            machine: host.machine.clone(),
            spot: host.spot,
            started: rfc3339_seconds(&host.created).unwrap_or(at).min(at),
            last_running: at,
            gone: None,
            agent,
        });
    });
}

/// Records a listing of `pool`.
pub fn observe(pool: &str, listed: &[Host]) {
    let at = now();
    let _ = Ledger::update(|ledger| ledger.observe(pool, listed, at));
}

// ---------------------------------------------------------------------------
// Admission.

/// Whether `adding` more hosts of `pool` may start, given the hosts it has
/// now; the refusal says why and what the owner can change.
pub fn admit(pool: &Pool, hosts: &[Host], adding: u64) -> Result<(), String> {
    admit_for(pool, hosts, adding, agent().as_deref())
}

/// Admission for a multi-agent host. Never changes the process-wide CLI actor.
pub fn admit_for(
    pool: &Pool,
    hosts: &[Host],
    adding: u64,
    actor: Option<&str>,
) -> Result<(), String> {
    admit_with(
        &Budget::load(),
        &Ledger::load(),
        pool,
        hosts,
        adding,
        actor,
        now(),
    )
}

pub fn admit_with(
    budget: &Budget,
    ledger: &Ledger,
    pool: &Pool,
    hosts: &[Host],
    adding: u64,
    agent: Option<&str>,
    at: u64,
) -> Result<(), String> {
    let grant = match agent {
        Some(id) => Some(budget.agents.get(id).ok_or_else(|| {
            format!(
                "agent {id} has no compute budget, so it cannot start GCE hosts. Ask the owner \
                 to grant one: `openagents cloud budget --agent {id} --daily 300`"
            )
        })?),
        None => None,
    };
    let live = hosts.iter().filter(|h| h.status != "TERMINATED").count() as u64;
    if live + adding > budget.max_hosts {
        return Err(format!(
            "the budget allows {} host(s) at once and the pool has {live}; `openagents cloud \
             budget --max-hosts N` raises it",
            budget.max_hosts
        ));
    }
    let new_hour = hourly_usd(&pool.machine, pool.spot) * adding as f64;
    let spent = ledger.spent(at, None);
    if spent + new_hour > budget.daily_usd {
        return Err(format!(
            "the pool has spent about ${spent:.2} of its ${:.2} over the last 24 hours, and \
             {adding} more host(s) would pass the cap within an hour; `openagents cloud budget \
             --daily N` raises it",
            budget.daily_usd
        ));
    }
    if let (Some(id), Some(grant)) = (agent, grant) {
        let mine = ledger.spent(at, Some(id));
        if mine + new_hour > grant.daily_usd {
            return Err(format!(
                "agent {id} has spent about ${mine:.2} of its ${:.2} over the last 24 hours; \
                 ask the owner to raise it (`openagents cloud budget --agent {id} --daily N`)",
                grant.daily_usd
            ));
        }
    }
    Ok(())
}

/// Whether a new run may start: refused once the 24-hour cap is spent.
pub fn admit_run() -> Result<(), String> {
    admit_run_for(agent().as_deref())
}

/// Checks a run even on a warm host, under the named agent's grant and cap.
pub fn admit_run_for(actor: Option<&str>) -> Result<(), String> {
    let budget = Budget::load();
    let ledger = Ledger::load();
    admit_run_with(&budget, &ledger, actor, now())
}

/// Checks the same run grant and caps against an explicit budget snapshot.
pub fn admit_run_with(
    budget: &Budget,
    ledger: &Ledger,
    actor: Option<&str>,
    at: u64,
) -> Result<(), String> {
    let spent = ledger.spent(at, None);
    if let Some(actor) = actor {
        let grant = budget.agents.get(actor).ok_or_else(|| format!(
            "Agent {actor} has no compute budget. Ask the owner to grant one: `openagents cloud budget --agent {actor} --daily 300`"
        ))?;
        if ledger.spent(at, Some(actor)) >= grant.daily_usd {
            return Err(format!(
                "Agent {actor} reached its daily compute budget. Ask the owner to raise it: `openagents cloud budget --agent {actor} --daily N`"
            ));
        }
    }
    if spent >= budget.daily_usd {
        return Err(format!(
            "the pool has spent about ${spent:.2} of its ${:.2} over the last 24 hours; no new \
             run starts (`openagents cloud budget --daily N` raises the cap)",
            budget.daily_usd
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Time.

/// Unix seconds of an RFC 3339 time as GCE writes it
/// (`2026-10-17T19:23:45.123-07:00`).
pub fn rfc3339_seconds(text: &str) -> Option<u64> {
    let text = text.trim();
    let number = |range: std::ops::Range<usize>| text.get(range)?.parse::<i64>().ok();
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    let rest = text.get(19..)?;
    let zone = rest.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    let offset = match zone {
        "Z" | "z" | "" => 0,
        z if z.len() == 6 && (z.starts_with('+') || z.starts_with('-')) => {
            let hours: i64 = z.get(1..3)?.parse().ok()?;
            let minutes: i64 = z.get(4..6)?.parse().ok()?;
            let sign = if z.starts_with('-') { -1 } else { 1 };
            sign * (hours * 3600 + minutes * 60)
        }
        _ => return None,
    };
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
    u64::try_from(seconds).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(name: &str, status: &str) -> Host {
        Host {
            name: name.into(),
            zone: "us-central1-a".into(),
            status: status.into(),
            machine: "c3-standard-8".into(),
            spot: true,
            created: "2026-10-18T02:00:00.000-00:00".into(),
            address: None,
        }
    }

    fn pool() -> Pool {
        Pool::grant(None, "c3-standard-8", true, 8, 10)
    }

    #[test]
    fn gce_times_read_as_unix_seconds() {
        assert_eq!(rfc3339_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(
            rfc3339_seconds("2026-10-17T19:00:00.123-07:00"),
            rfc3339_seconds("2026-10-18T02:00:00Z")
        );
        assert_eq!(rfc3339_seconds("2026-10-18T02:00:00Z"), Some(1_792_288_800));
        assert_eq!(rfc3339_seconds("yesterday"), None);
    }

    #[test]
    fn a_host_costs_from_creation_until_it_was_seen_gone() {
        let start = rfc3339_seconds("2026-10-18T02:00:00Z").unwrap();
        let mut ledger = Ledger::default();
        // Nothing spent shows as $0.00, not $-0.00.
        assert_eq!(format!("{:.2}", ledger.spent(start, None)), "0.00");
        ledger.observe("p1", &[host("a", "RUNNING")], start + 600);
        assert_eq!(ledger.hosts[0].started, start);
        // Still running an hour in: an hour of a spot c3-standard-8.
        let hourly = hourly_usd("c3-standard-8", true);
        assert!((ledger.spent(start + 3600, None) - hourly).abs() < 1e-9);
        // Not listed at two hours: gone then, and the cost stops there.
        ledger.observe("p1", &[], start + 7200);
        assert_eq!(ledger.hosts[0].gone, Some(start + 7200));
        assert!((ledger.spent(start + 10 * 3600, None) - 2.0 * hourly).abs() < 1e-9);
        // A day later it is outside the window.
        assert!(ledger.spent(start + 30 * 3600, None) < 1e-9);
        // A host that never reports gone bills at most the 12-hour backstop.
        let mut stuck = Ledger::default();
        stuck.observe("p1", &[host("b", "RUNNING")], start);
        assert!(stuck.spent(start + 20 * 3600, None) < 12.1 * hourly);
    }

    #[test]
    fn hosts_and_spend_stay_inside_the_budget() {
        let budget = Budget {
            daily_usd: 1.0,
            max_hosts: 2,
            ..Budget::default()
        };
        let empty = Ledger::default();
        let at = 1_792_288_800;
        assert!(admit_with(&budget, &empty, &pool(), &[], 2, None, at).is_ok());
        let full = admit_with(
            &budget,
            &empty,
            &pool(),
            &[host("a", "RUNNING")],
            2,
            None,
            at,
        );
        assert!(full.unwrap_err().contains("allows 2 host(s)"));
        // $1 a day: about five hours of one spot host already spent.
        let mut spent = Ledger::default();
        spent.observe("p1", &[host("a", "RUNNING")], at);
        let later = at + 5 * 3600;
        let refused = admit_with(&budget, &spent, &pool(), &[], 1, None, later).unwrap_err();
        assert!(refused.contains("of its $1.00"), "{refused}");
    }

    #[test]
    fn an_agent_starts_hosts_only_under_its_own_grant() {
        let mut budget = Budget::default();
        let ledger = Ledger::default();
        let at = 1_792_288_800;
        let refused = admit_with(&budget, &ledger, &pool(), &[], 1, Some("agt_1"), at);
        let refused = refused.unwrap_err();
        assert!(
            refused.contains("agent agt_1 has no compute budget"),
            "{refused}"
        );
        assert!(refused.contains("openagents cloud budget --agent agt_1"));
        budget.agents.insert(
            "agt_1".into(),
            AgentGrant {
                daily_usd: 300.0,
                granted_at: at,
            },
        );
        assert!(admit_with(&budget, &ledger, &pool(), &[], 1, Some("agt_1"), at).is_ok());
        // Another agent's grant is not this one's.
        assert!(admit_with(&budget, &ledger, &pool(), &[], 1, Some("agt_2"), at).is_err());
        // A person at a terminal needs no grant.
        assert!(admit_with(&budget, &ledger, &pool(), &[], 1, None, at).is_ok());
    }

    #[test]
    fn a_run_stops_at_its_share_of_the_per_run_cap() {
        let budget = Budget {
            per_run_usd: 0.09,
            ..Budget::default()
        };
        let limit = budget.run_limit(&host("a", "RUNNING"), 2);
        let expected = 0.09 / (hourly_usd("c3-standard-8", true) / 2.0) * 3600.0;
        assert!((limit.as_secs_f64() - expected).abs() < 1.0);
        assert!(Budget::default().check().is_ok());
        let bad = Budget {
            max_hosts: 0,
            ..Budget::default()
        };
        assert!(bad.check().is_err());
    }
}

#[cfg(test)]
mod run_admission_tests {
    use super::*;
    #[test]
    fn a_warm_host_never_bypasses_an_explicit_agents_grant_or_cap() {
        let mut budget = Budget::default();
        let ledger = Ledger::default();
        let at = 1_792_288_800;
        let error = admit_run_with(&budget, &ledger, Some("agt_builder"), at).unwrap_err();
        assert!(error.contains("openagents cloud budget --agent agt_builder"));
        budget.agents.insert(
            "agt_builder".into(),
            AgentGrant {
                daily_usd: 10.0,
                granted_at: at,
            },
        );
        assert!(admit_run_with(&budget, &ledger, Some("agt_builder"), at).is_ok());
        assert!(admit_run_with(&budget, &ledger, Some("agt_reviewer"), at).is_err());
        budget.agents.get_mut("agt_builder").unwrap().daily_usd = 0.0;
        assert!(
            admit_run_with(&budget, &ledger, Some("agt_builder"), at)
                .unwrap_err()
                .contains("raise it")
        );
        assert!(admit_run_with(&budget, &ledger, None, at).is_ok());
    }
}

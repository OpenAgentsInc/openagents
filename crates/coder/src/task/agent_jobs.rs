//! A workshop agent's standing jobs (`docs/verse/workshop-agent.md`,
//! "Standing jobs"): at most [`MAX_JOBS`] finite, recurring jobs in
//! `agents/NAME/jobs.json` under the host root, modeled on an NIP-AUTO
//! plan. Every job starts off; only the owner turns one on, renews it,
//! or makes one, at the host.
//!
//! The host's scheduler ([`tick`]) asks each job whether its trigger
//! fired: a daily or weekly time, a labeled issue that is free to pick up,
//! or the default branch moving. Each occurrence is admitted when it
//! fires, against the agent's state, the job's expiry, occurrences, and
//! budget, and the capacity book; a refused occurrence is journaled with
//! the reason and skipped, never queued. A time that passed while the host
//! was down fires once, not once per missed slot. Four templates ship:
//! the nightly check, watch issues, keep it green, and reflect
//! ([`template`]). A reflect occurrence runs the agent's reflection
//! (`agent_reflect`) instead of handing her a request: nightly, and early
//! when the summed importance of her records since the last reflection
//! passes the job's threshold, at most `early_per_day` times a day.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::agent::{Entry, Kind, Record, State, Store};
use super::issue_pick::{Open, Pull};
use coder_host::access::agent::Mode;

/// One job's schema.
pub const SCHEMA: &str = "openagents.standing-job.v1";
/// The most jobs one agent keeps.
pub const MAX_JOBS: usize = 8;
/// The farthest a job's expiry may be: 90 days.
pub const EXPIRY_MAX: u64 = 90 * 24 * 60 * 60;
/// The longest title.
pub const TITLE_MAX: usize = 120;
/// The templates [`template`] makes.
pub const TEMPLATES: [&str; 4] = ["nightly-check", "watch-issues", "keep-green", "reflect"];

/// What makes a job fire.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Trigger {
    /// A daily time, or a weekly one on `weekday` (0 is Monday), in the
    /// owner's time zone at `utc_offset` minutes from UTC.
    Schedule {
        /// `HH:MM`.
        at: String,
        #[serde(default)]
        weekday: Option<u8>,
        #[serde(default)]
        utc_offset: i32,
    },
    /// An open issue in `repository` labeled `label` that the pickup
    /// rules leave free.
    Issues { repository: String, label: String },
    /// The workspace's default branch moved.
    Checks {},
    /// A reflection: daily at `at`, as a schedule, and early when the
    /// summed importance of the agent's records since the last reflection
    /// reaches `threshold`, at most `early_per_day` times a local day.
    Reflect {
        /// `HH:MM`.
        at: String,
        #[serde(default)]
        utc_offset: i32,
        threshold: u32,
        early_per_day: u32,
    },
}

impl Trigger {
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Self::Schedule { .. } => "schedule",
            Self::Issues { .. } => "issues",
            Self::Checks {} => "checks",
            Self::Reflect { .. } => "reflect",
        }
    }
}

/// Model spend, in dollars: per occurrence, for the whole job, and what
/// it spent. An occurrence whose spend no meter reported counts as
/// unknown, never zero.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    pub per_occurrence: f64,
    pub per_job: f64,
    #[serde(default)]
    pub spent: f64,
    #[serde(default)]
    pub unmetered: u32,
}

/// One standing job (`openagents.standing-job.v1`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub schema: String,
    pub v: u32,
    pub requires: Vec<String>,
    /// Its ID, a studio identity.
    pub job: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    pub trigger: Trigger,
    /// The request text an occurrence sends.
    pub action: String,
    pub mode: Mode,
    /// Its workspace: a host label, or the agent's own directory when
    /// empty.
    #[serde(default)]
    pub workspace: String,
    pub max_occurrences: u32,
    #[serde(default)]
    pub occurrences: u32,
    pub expires_at: u64,
    pub budget: Budget,
    /// A successful occurrence reports only when false.
    #[serde(default)]
    pub quiet: bool,
    /// Off until the owner turns it on.
    #[serde(default)]
    pub enabled: bool,
    /// When the owner turned it on; a slot before this never fires.
    #[serde(default)]
    pub enabled_at: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_fired: Option<u64>,
    /// What the trigger saw last: the default branch's commit, or the
    /// issues it already sent, comma-separated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<String>,
    /// The last occurrence or refusal, in words.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last: Option<String>,
}

/// What the scheduler reads about the world.
pub trait Facts {
    /// The commit the default branch of the repository at `path` names.
    fn head(&self, path: &Path) -> Option<String>;
    /// The open issues in `repository` labeled `label`, and the open pull
    /// requests.
    ///
    /// # Errors
    /// When they cannot be read.
    fn issues(&self, repository: &str, label: &str) -> Result<(Vec<Open>, Vec<Pull>), String>;
    /// Whether a provider has capacity for a model call.
    fn capacity(&self) -> bool;
}

/// One occurrence the scheduler admitted: a request to hand the agent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Occurrence {
    pub job: String,
    pub text: String,
    pub mode: Mode,
    pub workspace: String,
    pub quiet: bool,
    /// Keep it green: a failing check becomes a task-mode fix.
    pub fix_on_failure: bool,
    /// Reflect: what triggered the reflection, such as `nightly`; the host
    /// runs a reflection in place of a request.
    pub reflect: Option<String>,
}

/// An agent's jobs file.
#[derive(Clone, Debug)]
pub struct Jobs {
    store: Store,
}

impl Jobs {
    #[must_use]
    pub fn new(store: Store) -> Self {
        Self { store }
    }

    fn path(&self) -> PathBuf {
        self.store.dir().join("jobs.json")
    }

    /// Every job.
    ///
    /// # Errors
    /// When the file exists and cannot be read.
    pub fn load(&self) -> Result<Vec<Job>, String> {
        let text = match std::fs::read_to_string(self.path()) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(format!("cannot read {}: {e}", self.path().display())),
        };
        let jobs: Vec<Job> = serde_json::from_str(&text)
            .map_err(|e| format!("{} is not a jobs file: {e}", self.path().display()))?;
        Ok(jobs
            .into_iter()
            .filter(|job| job.schema == SCHEMA && job.v == 1 && job.requires.is_empty())
            .collect())
    }

    /// Writes `jobs` in place of the stored ones.
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn save(&self, jobs: &[Job]) -> Result<(), String> {
        let body = serde_json::to_vec_pretty(jobs).map_err(|e| e.to_string())?;
        let temp = self.store.dir().join(".jobs.json.tmp");
        std::fs::write(&temp, body).map_err(|e| format!("cannot write {}: {e}", temp.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&temp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&temp, self.path())
            .map_err(|e| format!("cannot write {}: {e}", self.path().display()))
    }

    /// Adds `job`, off, as the owner does at the host.
    ///
    /// # Errors
    /// A malformed job, an ID in use, or [`MAX_JOBS`] already.
    pub fn add(&self, job: Job, now: u64) -> Result<(), String> {
        validate(&job, now)?;
        let mut jobs = self.load()?;
        if jobs.iter().any(|j| j.job == job.job) {
            return Err(format!("a job named `{}` exists", job.job));
        }
        if jobs.len() >= MAX_JOBS {
            return Err(format!("an agent keeps at most {MAX_JOBS} standing jobs"));
        }
        let title = job.title.clone();
        let id = job.job.clone();
        jobs.push(job);
        self.save(&jobs)?;
        self.store.append(&Entry::new(
            now,
            Kind::Job,
            &format!("the owner added job {id} ({title}), off"),
        ))
    }

    /// Turns job `id` on or off, deletes it, or renews it to `renew`, as
    /// the owner asks. Turning on or renewing is the host's own path; a
    /// device may only pause, resume, or delete.
    ///
    /// # Errors
    /// No such job, an expiry out of range, or the file cannot be written.
    pub fn edit(&self, id: &str, edit: Edit, now: u64) -> Result<(), String> {
        let mut jobs = self.load()?;
        let index = jobs
            .iter()
            .position(|j| j.job == id)
            .ok_or_else(|| format!("no job named `{id}`"))?;
        let word = match edit {
            Edit::On => {
                if jobs[index].expires_at <= now {
                    return Err(format!("job `{id}` expired; renew it first"));
                }
                jobs[index].enabled = true;
                jobs[index].enabled_at = now;
                "turned on"
            }
            Edit::Off => {
                jobs[index].enabled = false;
                "turned off"
            }
            Edit::Delete => {
                jobs.remove(index);
                "deleted"
            }
            Edit::Renew(expires_at) => {
                if expires_at <= now || expires_at - now > EXPIRY_MAX {
                    return Err("a job expires within 90 days".into());
                }
                jobs[index].expires_at = expires_at;
                "renewed"
            }
        };
        self.save(&jobs)?;
        self.store
            .append(&Entry::new(now, Kind::Job, &format!("job {id} {word}")))
    }

    /// Records what occurrence spending of job `id` cost: `Some` dollars
    /// moves one occurrence from unmetered to spent; `None` leaves it
    /// unmetered.
    ///
    /// # Errors
    /// When the file cannot be read or written.
    pub fn meter(&self, id: &str, usd: Option<f64>) -> Result<(), String> {
        let Some(usd) = usd else {
            return Ok(());
        };
        let mut jobs = self.load()?;
        let Some(job) = jobs.iter_mut().find(|j| j.job == id) else {
            return Ok(());
        };
        job.budget.spent += usd.max(0.0);
        job.budget.unmetered = job.budget.unmetered.saturating_sub(1);
        self.save(&jobs)
    }

    /// Turns every job off, as the first step of a stop. Returns how many
    /// were on.
    ///
    /// # Errors
    /// When the file cannot be written.
    pub fn disable_all(&self) -> Result<usize, String> {
        let mut jobs = self.load()?;
        let on = jobs.iter().filter(|j| j.enabled).count();
        if on > 0 {
            for job in &mut jobs {
                job.enabled = false;
            }
            self.save(&jobs)?;
        }
        Ok(on)
    }

    /// The jobs as a device reads them.
    ///
    /// # Errors
    /// When the file cannot be read.
    pub fn rows(&self) -> Result<Vec<coder_host::access::agent::JobRow>, String> {
        Ok(self
            .load()?
            .into_iter()
            .map(|job| coder_host::access::agent::JobRow {
                trigger: job.trigger.word().into(),
                job: job.job,
                title: job.title,
                enabled: job.enabled,
                occurrences: job.occurrences,
                max_occurrences: job.max_occurrences,
                expires_at: job.expires_at,
                last: job.last,
            })
            .collect())
    }
}

/// An owner's change to a job.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    On,
    Off,
    Delete,
    Renew(u64),
}

/// Checks a job's fields.
///
/// # Errors
/// Says which field is out of range.
pub fn validate(job: &Job, now: u64) -> Result<(), String> {
    if job.schema != SCHEMA || job.v != 1 || !job.requires.is_empty() {
        return Err("not a standing-job record this host reads".into());
    }
    if coder_host::access::studio::id(&job.job).is_err() {
        return Err("a job ID is letters, digits, dots, hyphens, and underscores".into());
    }
    if job.title.trim().is_empty() || job.title.len() > TITLE_MAX {
        return Err(format!("a job title is 1 to {TITLE_MAX} bytes"));
    }
    if job.action.trim().is_empty() || job.action.len() > coder_host::access::agent::MAX_TEXT {
        return Err("a job's request is 1 to 16 KiB".into());
    }
    if job.max_occurrences == 0 || job.max_occurrences > 1000 {
        return Err("a job runs 1 to 1000 times".into());
    }
    if job.expires_at <= now || job.expires_at - now > EXPIRY_MAX {
        return Err("a job expires within 90 days".into());
    }
    if !(job.budget.per_occurrence >= 0.0
        && job.budget.per_job >= job.budget.per_occurrence
        && job.budget.per_job <= 1000.0)
    {
        return Err("a job's budget is a per-occurrence amount within its total".into());
    }
    if let Trigger::Schedule { at, weekday, .. } = &job.trigger {
        if minutes(at).is_none() || weekday.is_some_and(|d| d > 6) {
            return Err("a schedule is HH:MM, and a weekday 0 (Monday) to 6".into());
        }
    }
    if let Trigger::Reflect {
        at,
        threshold,
        early_per_day,
        ..
    } = &job.trigger
        && (minutes(at).is_none() || *threshold == 0 || *early_per_day > 24)
    {
        return Err(
            "a reflection is HH:MM, a threshold above 0, and at most 24 early ones a day".into(),
        );
    }
    if let Trigger::Issues { repository, label } = &job.trigger
        && (!repository.contains('/') || label.trim().is_empty())
    {
        return Err("watch issues names an OWNER/REPO and a label".into());
    }
    Ok(())
}

fn minutes(at: &str) -> Option<i64> {
    let (h, m) = at.split_once(':')?;
    let (h, m): (i64, i64) = (h.parse().ok()?, m.parse().ok()?);
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(h * 60 + m)
}

/// The newest scheduled time at or before `now`, Unix seconds.
#[must_use]
pub fn last_slot(at: &str, weekday: Option<u8>, utc_offset: i32, now: u64) -> Option<u64> {
    let minute = minutes(at)?;
    let offset = i64::from(utc_offset) * 60;
    let local = now as i64 + offset;
    let day = local.div_euclid(86_400);
    for back in 0..8 {
        let d = day - back;
        // 1970-01-01 was a Thursday: weekday 3 counting from Monday.
        let weekday_of = (d + 3).rem_euclid(7) as u8;
        if weekday.is_some_and(|w| w != weekday_of) {
            continue;
        }
        let slot = d * 86_400 + minute * 60 - offset;
        if slot <= now as i64 {
            return u64::try_from(slot).ok();
        }
    }
    None
}

/// A job made from template `name` for `workspace`, off, expiring in 30
/// days. `repository` and `label` are watch issues' own.
///
/// # Errors
/// An unknown template, or watch issues without a repository.
pub fn template(
    name: &str,
    workspace: &str,
    repository: Option<&str>,
    label: Option<&str>,
    utc_offset: i32,
    now: u64,
) -> Result<Job, String> {
    let base = |job: &str, title: &str, trigger: Trigger, action: &str, mode: Mode| Job {
        schema: SCHEMA.into(),
        v: 1,
        requires: Vec::new(),
        job: job.into(),
        title: title.into(),
        template: Some(name.into()),
        trigger,
        action: action.into(),
        mode,
        workspace: workspace.into(),
        max_occurrences: 30,
        occurrences: 0,
        expires_at: now + 30 * 24 * 60 * 60,
        budget: Budget {
            per_occurrence: 0.5,
            per_job: 10.0,
            spent: 0.0,
            unmetered: 0,
        },
        quiet: false,
        enabled: false,
        enabled_at: 0,
        last_fired: None,
        last_seen: None,
        last: None,
    };
    Ok(match name {
        "nightly-check" => base(
            "nightly-check",
            "Nightly check",
            Trigger::Schedule {
                at: "02:00".into(),
                weekday: None,
                utc_offset,
            },
            "Run this workspace's checks on its default branch, read-only. Report each failing \
             command with its first error, or that every check passed.",
            Mode::Terminal,
        ),
        "watch-issues" => {
            let repository = repository.ok_or("watch issues needs --repository OWNER/REPO")?;
            base(
                "watch-issues",
                "Watch issues",
                Trigger::Issues {
                    repository: repository.into(),
                    label: label.unwrap_or("agent").into(),
                },
                "Work the issue below in your own worktree and bring the change to the Merge \
                 station. Never merge, push, or comment.",
                Mode::Task,
            )
        }
        "keep-green" => base(
            "keep-green",
            "Keep it green",
            Trigger::Checks {},
            "The default branch moved. Run this workspace's checks, read-only, and report each \
             failing command with its first error.",
            Mode::Terminal,
        ),
        "reflect" => {
            let mut job = base(
                "reflect",
                "Reflect",
                Trigger::Reflect {
                    at: "03:00".into(),
                    utc_offset,
                    threshold: super::agent_reflect::EARLY_THRESHOLD,
                    early_per_day: super::agent_reflect::EARLY_PER_DAY,
                },
                "Reflect on your recent records: ask the three most salient questions, write \
                 insights that cite the records they rest on, and keep only those that check.",
                Mode::Terminal,
            );
            // Nightly for 30 days, and up to two early ones a day.
            job.max_occurrences = 90;
            job.budget.per_occurrence = 0.25;
            job
        }
        other => return Err(format!("no job template named `{other}`")),
    })
}

/// Runs the scheduler once for the agent `record` at `now`: each job on
/// whose trigger fired is admitted or refused, and both are journaled.
/// `path` is the workspace's root for the checks trigger. Returns the
/// admitted occurrences, which the caller hands the agent.
///
/// # Errors
/// When the jobs file cannot be read or written.
pub fn tick(
    store: &Store,
    record: &Record,
    path: &Path,
    facts: &dyn Facts,
    now: u64,
) -> Result<Vec<Occurrence>, String> {
    let jobs_file = Jobs::new(store.clone());
    let mut jobs = jobs_file.load()?;
    let mut fired = Vec::new();
    let mut changed = false;
    for job in &mut jobs {
        if !job.enabled {
            continue;
        }
        let Some((detail, seen)) = due(job, store, path, facts, now) else {
            continue;
        };
        changed = true;
        job.last_fired = Some(now);
        if let Some(seen) = seen {
            job.last_seen = Some(seen);
        }
        match admit(job, record, facts, now) {
            Ok(()) => {
                job.occurrences += 1;
                job.budget.unmetered += 1;
                let reflect = matches!(job.trigger, Trigger::Reflect { .. })
                    .then(|| detail.clone().unwrap_or_else(|| "nightly".into()));
                let text = match &detail {
                    Some(detail) => format!("{}\n\n{detail}", job.action),
                    None => job.action.clone(),
                };
                job.last = Some(format!("fired at {now}"));
                let _ = store.append(&Entry::new(
                    now,
                    Kind::Job,
                    &format!(
                        "job {} fired, occurrence {} of {}",
                        job.job, job.occurrences, job.max_occurrences
                    ),
                ));
                fired.push(Occurrence {
                    job: job.job.clone(),
                    text,
                    mode: job.mode,
                    workspace: job.workspace.clone(),
                    quiet: job.quiet,
                    fix_on_failure: job.template.as_deref() == Some("keep-green"),
                    reflect,
                });
            }
            Err(why) => {
                job.last = Some(format!("refused at {now}: {why}"));
                let _ = store.append(&Entry::new(
                    now,
                    Kind::Job,
                    &format!("job {} refused and skipped: {why}", job.job),
                ));
            }
        }
    }
    if changed {
        jobs_file.save(&jobs)?;
    }
    Ok(fired)
}

/// Whether `job`'s trigger fired at `now`: what an occurrence adds to the
/// request, and what the trigger saw, to record.
#[allow(clippy::type_complexity)]
fn due(
    job: &Job,
    store: &Store,
    path: &Path,
    facts: &dyn Facts,
    now: u64,
) -> Option<(Option<String>, Option<String>)> {
    match &job.trigger {
        Trigger::Schedule {
            at,
            weekday,
            utc_offset,
        } => {
            let slot = last_slot(at, *weekday, *utc_offset, now)?;
            let after = job.last_fired.unwrap_or(0).max(job.enabled_at);
            (slot > after).then_some((None, None))
        }
        Trigger::Reflect {
            at,
            utc_offset,
            threshold,
            early_per_day,
        } => {
            let today = (now as i64 + i64::from(*utc_offset) * 60).div_euclid(86_400);
            // `last_seen` is `DAY:EARLY`, the early reflections that day.
            let early = job
                .last_seen
                .as_deref()
                .and_then(|seen| seen.split_once(':'))
                .and_then(|(day, n)| Some((day.parse::<i64>().ok()?, n.parse::<u32>().ok()?)))
                .filter(|(day, _)| *day == today)
                .map_or(0, |(_, n)| n);
            let after = job.last_fired.unwrap_or(0).max(job.enabled_at);
            if last_slot(at, None, *utc_offset, now).is_some_and(|slot| slot > after) {
                return Some((Some("nightly".into()), Some(format!("{today}:{early}"))));
            }
            if early >= *early_per_day {
                return None;
            }
            let summed = super::agent_reflect::pressure(store, job.enabled_at).ok()?;
            (summed >= f64::from(*threshold)).then(|| {
                (
                    Some(format!(
                        "early: importance {summed:.0} since the last reflection reached {threshold}"
                    )),
                    Some(format!("{today}:{}", early + 1)),
                )
            })
        }
        Trigger::Checks {} => {
            let head = facts.head(path)?;
            match &job.last_seen {
                // The first look only records where the branch is
                // ([`observe`]).
                None => None,
                Some(seen) if *seen == head => None,
                Some(_) => Some((
                    Some(format!("The default branch is at {head}.")),
                    Some(head),
                )),
            }
        }
        Trigger::Issues { repository, label } => {
            let (issues, pulls) = facts.issues(repository, label).ok()?;
            let sent: Vec<&str> = job
                .last_seen
                .as_deref()
                .unwrap_or("")
                .split(',')
                .filter(|s| !s.is_empty())
                .collect();
            let fresh: Vec<Open> = issues
                .into_iter()
                .filter(|open| !sent.contains(&open.number.to_string().as_str()))
                .collect();
            let picked = super::issue_pick::choose(&fresh, &pulls, &[], now, 24)
                .ok()?
                .into_iter()
                .next()?;
            let mut seen: Vec<String> = sent.iter().map(|s| (*s).to_string()).collect();
            seen.push(picked.number.to_string());
            Some((
                Some(format!(
                    "Issue #{} in {repository}: {}",
                    picked.number, picked.title
                )),
                Some(seen.join(",")),
            ))
        }
    }
}

/// Records where a checks trigger starts watching from, without firing.
///
/// # Errors
/// When the jobs file cannot be written.
pub fn observe(store: &Store, path: &Path, facts: &dyn Facts) -> Result<(), String> {
    let jobs_file = Jobs::new(store.clone());
    let mut jobs = jobs_file.load()?;
    let mut changed = false;
    for job in &mut jobs {
        if job.enabled
            && matches!(job.trigger, Trigger::Checks {})
            && job.last_seen.is_none()
            && let Some(head) = facts.head(path)
        {
            job.last_seen = Some(head);
            changed = true;
        }
    }
    if changed {
        jobs_file.save(&jobs)?;
    }
    Ok(())
}

/// Whether an occurrence of `job` may run now: the agent active, the job
/// unexpired with occurrences and budget left, and a provider with
/// capacity.
///
/// # Errors
/// Why it may not.
pub fn admit(job: &Job, record: &Record, facts: &dyn Facts, now: u64) -> Result<(), String> {
    if record.state != State::Active {
        return Err(format!("{} is {}", record.name, record.state.word()));
    }
    if job.expires_at <= now {
        return Err("the job expired".into());
    }
    if job.occurrences >= job.max_occurrences {
        return Err("the job used all its occurrences".into());
    }
    if job.budget.spent + job.budget.per_occurrence > job.budget.per_job {
        return Err("the job's budget is spent".into());
    }
    if !facts.capacity() {
        return Err("no provider has capacity".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct World {
        head: Option<String>,
        issues: Vec<Open>,
        capacity: bool,
    }

    impl Facts for World {
        fn head(&self, _path: &Path) -> Option<String> {
            self.head.clone()
        }
        fn issues(&self, _r: &str, _l: &str) -> Result<(Vec<Open>, Vec<Pull>), String> {
            Ok((self.issues.clone(), Vec::new()))
        }
        fn capacity(&self) -> bool {
            self.capacity
        }
    }

    fn agent(dir: &tempfile::TempDir) -> (Store, Record) {
        let store = Store::new(&dir.path().join("host"), "alice").unwrap();
        let record = store.open(dir.path(), 1).unwrap();
        (store, record)
    }

    const DAY: u64 = 86_400;
    // 2026-10-05 00:00 UTC, a Monday.
    const MONDAY: u64 = 1_791_158_400;

    #[test]
    fn slots_fall_on_the_day_and_weekday() {
        let two = last_slot("02:00", None, 0, MONDAY + 3 * 3600).unwrap();
        assert_eq!(two, MONDAY + 2 * 3600);
        let before = last_slot("02:00", None, 0, MONDAY + 3600).unwrap();
        assert_eq!(before, MONDAY - DAY + 2 * 3600);
        // A Monday-only slot seen on Wednesday is Monday's.
        let weekly = last_slot("02:00", Some(0), 0, MONDAY + 2 * DAY + 10).unwrap();
        assert_eq!(weekly, MONDAY + 2 * 3600);
        // Two hours west of UTC, 02:00 local is 04:00 UTC.
        let west = last_slot("02:00", None, -120, MONDAY + 5 * 3600).unwrap();
        assert_eq!(west, MONDAY + 4 * 3600);
    }

    #[test]
    fn jobs_start_off_and_fire_once_per_slot_when_admitted() {
        let dir = tempfile::tempdir().unwrap();
        let (store, record) = agent(&dir);
        let jobs = Jobs::new(store.clone());
        let now = MONDAY + 3600;
        jobs.add(
            template("nightly-check", "", None, None, 0, now).unwrap(),
            now,
        )
        .unwrap();
        let world = World {
            head: None,
            issues: Vec::new(),
            capacity: true,
        };
        // Off: nothing fires.
        assert!(
            tick(&store, &record, dir.path(), &world, now + DAY)
                .unwrap()
                .is_empty()
        );
        jobs.edit("nightly-check", Edit::On, now).unwrap();
        // Days later, the missed slots fire once.
        let later = now + 3 * DAY;
        let fired = tick(&store, &record, dir.path(), &world, later).unwrap();
        assert_eq!(fired.len(), 1);
        assert_eq!(fired[0].mode, Mode::Terminal);
        assert!(
            tick(&store, &record, dir.path(), &world, later + 60)
                .unwrap()
                .is_empty()
        );
        // No capacity: the next slot is refused and skipped, not queued.
        let empty = World {
            capacity: false,
            ..world
        };
        assert!(
            tick(&store, &record, dir.path(), &empty, later + DAY)
                .unwrap()
                .is_empty()
        );
        let journal = store.journal(50).unwrap();
        assert!(
            journal
                .iter()
                .any(|e| e.text.contains("refused and skipped"))
        );
        let full = World {
            capacity: true,
            ..empty
        };
        assert!(
            tick(&store, &record, dir.path(), &full, later + DAY + 60)
                .unwrap()
                .is_empty()
        );
        // A paused agent's occurrence is refused.
        let mut paused = record.clone();
        paused.state = State::Paused;
        assert!(
            tick(&store, &paused, dir.path(), &full, later + 2 * DAY)
                .unwrap()
                .is_empty()
        );
        assert_eq!(jobs.load().unwrap()[0].occurrences, 1);
        // Stop turns every job off.
        assert_eq!(jobs.disable_all().unwrap(), 1);
        assert!(!jobs.load().unwrap()[0].enabled);
    }

    #[test]
    fn keep_green_fires_when_the_branch_moves_and_watch_picks_free_issues() {
        let dir = tempfile::tempdir().unwrap();
        let (store, record) = agent(&dir);
        let jobs = Jobs::new(store.clone());
        let now = MONDAY;
        jobs.add(template("keep-green", "", None, None, 0, now).unwrap(), now)
            .unwrap();
        jobs.add(
            template("watch-issues", "", Some("o/r"), Some("agent"), 0, now).unwrap(),
            now,
        )
        .unwrap();
        jobs.edit("keep-green", Edit::On, now).unwrap();
        jobs.edit("watch-issues", Edit::On, now).unwrap();
        let mut world = World {
            head: Some("aaa".into()),
            issues: vec![
                Open {
                    number: 7,
                    title: "Held".into(),
                    assignees: vec!["someone".into()],
                    ..Open::default()
                },
                Open {
                    number: 9,
                    title: "Free".into(),
                    ..Open::default()
                },
            ],
            capacity: true,
        };
        observe(&store, dir.path(), &world).unwrap();
        let fired = tick(&store, &record, dir.path(), &world, now + 10).unwrap();
        assert_eq!(fired.len(), 1, "the watch fires; the branch has not moved");
        assert!(fired[0].text.contains("Issue #9"));
        assert_eq!(fired[0].mode, Mode::Task);
        assert!(
            tick(&store, &record, dir.path(), &world, now + 20)
                .unwrap()
                .is_empty()
        );
        world.head = Some("bbb".into());
        let fired = tick(&store, &record, dir.path(), &world, now + 30).unwrap();
        assert_eq!(fired.len(), 1);
        assert!(fired[0].fix_on_failure);
    }

    #[test]
    fn expiry_and_budget_are_finite() {
        let now = MONDAY;
        let mut job = template("nightly-check", "", None, None, 0, now).unwrap();
        job.expires_at = now + EXPIRY_MAX + 1;
        assert!(validate(&job, now).is_err());
        job.expires_at = now + 10;
        job.budget.per_job = 0.1;
        assert!(validate(&job, now).is_err());
        assert!(template("nope", "", None, None, 0, now).is_err());
        assert!(template("watch-issues", "", None, None, 0, now).is_err());
    }
}

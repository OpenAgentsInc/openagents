//! Landing a committed change on a shared branch when several machines
//! land at once (#10226).
//!
//! The issue flow serializes landings on one machine with a lock
//! (`issue_run`'s in-process mutex and the task store's lock file), taken
//! through [`Hooks::enter`] and held only for fetch → rebase → push: the
//! checks after a rebase and the wait after a refused push run outside it,
//! so parallel runs on one machine land seconds apart instead of one check
//! run apart (#10391). Across
//! machines (the Mac, CoderOS, GCE spot hosts, Boat sandboxes) there is no
//! shared lock: Git itself is the arbiter, because a plain push (never a
//! forced one) is refused whenever the branch moved past what the pusher
//! last fetched. [`land`] turns that refusal into a loop:
//!
//! 1. fetch the branch;
//! 2. when it moved past the change's base, rebase the change onto it and
//!    decide whether the newly landed commits can affect the change
//!    ([`affects`]): only then run the checks again, so a dozen hosts that
//!    each lose a race to an unrelated commit do not all re-run their tests;
//! 3. push plainly; a refused push waits a jittered, growing delay
//!    ([`Backoff`]) so the losers of one race do not collide again, then
//!    goes back to 1.
//!
//! It gives up after [`Plan::attempts`] tries, or sooner when two pushes in
//! a row are refused while the branch did not move (a hook, permissions, or
//! a dead remote, not a race). Giving up, a conflict, or red checks leave
//! the change committed in the worktree; nothing is ever forced. A rebase
//! conflict gets one engine repair turn with both commit histories and the
//! conflict diff, followed by mandatory checks before landing resumes.
//!
//! These bounds apply only to the landing retry, never to a Coder run.

use std::hash::{BuildHasher, Hasher};
use std::path::Path;
use std::time::{Duration, Instant};

use super::local;

/// How long to wait before the next push after a refused one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Backoff {
    /// The ceiling of the first wait; each later ceiling doubles.
    pub first: Duration,
    /// The largest ceiling.
    pub cap: Duration,
}

impl Backoff {
    /// The issue flow's landing backoff.
    pub const LANDING: Backoff = Backoff {
        first: Duration::from_secs(2),
        cap: Duration::from_secs(60),
    };

    /// The wait before retry `retry` (1 for the first retry): uniformly
    /// random between half the ceiling and the ceiling, the ceiling
    /// doubling from [`Backoff::first`] up to [`Backoff::cap`].
    #[must_use]
    pub fn delay(&self, retry: u32) -> Duration {
        let doublings = retry.saturating_sub(1).min(20);
        let ceiling = self.first.saturating_mul(1 << doublings).min(self.cap);
        let half = ceiling / 2;
        half + half.mul_f64(random_unit())
    }
}

/// A number in `[0, 1)`, from the standard library's randomly keyed hasher
/// and the clock: every process and every call differs, which is all the
/// jitter needs.
fn random_unit() -> f64 {
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    hasher.write_u128(nanos);
    hasher.write_u32(std::process::id());
    (hasher.finish() >> 11) as f64 / (1u64 << 53) as f64
}

/// What to land and how hard to try.
#[derive(Clone, Debug)]
pub struct Plan<'a> {
    /// The worktree whose `HEAD` holds the committed change.
    pub worktree: &'a Path,
    /// The branch on `origin` to land on.
    pub branch: &'a str,
    /// The most pushes to try.
    pub attempts: u32,
    /// The wait between a refused push and the next try.
    pub backoff: Backoff,
}

impl Plan<'_> {
    /// The issue flow's attempts.
    pub const ATTEMPTS: u32 = 12;
}

/// What a landing asks of the flow it lands for.
pub trait Hooks {
    /// Runs the checks on the rebased change at the worktree's `HEAD`;
    /// returns the problems, empty when green. It may recommit the change,
    /// so the landing reads `HEAD` again after it.
    fn check(&mut self) -> Vec<String>;
    /// Gives the engine one turn to resolve the active rebase conflicts.
    /// The host stages the resolution and continues the rebase afterward.
    fn fix_conflict(&mut self, _request: &str) -> Result<(), String> {
        Err("No conflict-repair engine is available.".into())
    }
    /// Reports a step of the landing as it happens.
    fn note(&mut self, text: &str);
    /// Whether the person who started the landing asked it to stop.
    fn stopping(&self) -> bool;
    /// Takes this machine's landing lock, waiting for another run that
    /// holds it; the lock is held until the returned guard drops. The
    /// landing holds it only for fetch → rebase → push. `None` when there
    /// is no lock to take.
    fn enter(&mut self) -> Option<Box<dyn std::any::Any>> {
        None
    }
}

/// Whether the checks ran again after a rebase, and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Recheck {
    /// The branch had not moved, so nothing changed under the change.
    NotMoved,
    /// The newly landed commits cannot affect the change.
    Skipped(String),
    /// The checks ran again, for this reason; `passed` says how they ended.
    Ran { why: String, passed: bool },
}

/// One try at landing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attempt {
    /// Its number, from 1.
    pub number: u32,
    /// How many commits had landed on the branch since the change's last
    /// base.
    pub moved: usize,
    /// Whether the checks ran again.
    pub recheck: Recheck,
    /// Why the push was refused; `None` when it was not tried or landed.
    pub refused: Option<String>,
    /// How long it waited before the next try.
    pub waited: Option<Duration>,
}

impl Attempt {
    /// The attempt in a sentence, for the issue comment.
    #[must_use]
    pub fn describe(&self, branch: &str) -> String {
        let mut text = format!("Attempt {}: ", self.number);
        if self.moved == 0 {
            text.push_str(&format!("`{branch}` had not moved"));
        } else {
            text.push_str(&format!(
                "`{branch}` had moved by {} commit(s), so Coder rebased the change",
                self.moved
            ));
        }
        match &self.recheck {
            Recheck::NotMoved => {}
            Recheck::Skipped(why) => text.push_str(&format!("; checks not re-run ({why})")),
            Recheck::Ran { why, passed } => text.push_str(&format!(
                "; checks re-run ({why}): {}",
                if *passed { "green" } else { "red" }
            )),
        }
        match (&self.refused, self.waited) {
            (Some(why), Some(waited)) => text.push_str(&format!(
                "; push refused ({}), retried after {:.1} s.",
                clip(why, 160),
                waited.as_secs_f64()
            )),
            (Some(why), None) => text.push_str(&format!("; push refused ({}).", clip(why, 160))),
            (None, _) => text.push('.'),
        }
        text
    }
}

/// A change that landed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Landed {
    /// The commit now at the branch's tip.
    pub commit: String,
    /// Every try, the last one the push that landed.
    pub attempts: Vec<Attempt>,
}

/// Why a change did not land.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The person who started it asked it to stop.
    Stopped,
    /// The change conflicts with the newer branch.
    Conflict(String),
    /// The checks failed on the rebased change.
    Red(Vec<String>),
    /// The branch could not be read.
    Unreadable(String),
    /// The pushes kept being refused.
    GaveUp(String),
}

/// A change that did not land, with what was tried.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NotLanded {
    pub failure: Failure,
    pub attempts: Vec<Attempt>,
}

/// The landing lines of an issue comment: how many tries it took and what
/// each one did.
#[must_use]
pub fn summary(attempts: &[Attempt], branch: &str) -> String {
    let refused = attempts.iter().filter(|a| a.refused.is_some()).count();
    let reran = attempts
        .iter()
        .filter(|a| matches!(a.recheck, Recheck::Ran { .. }))
        .count();
    let skipped = attempts
        .iter()
        .filter(|a| matches!(a.recheck, Recheck::Skipped(_)))
        .count();
    let mut text = format!(
        "{} attempt(s), {refused} refused push(es), checks re-run {reran} time(s) and skipped \
         {skipped} time(s) after a rebase.\n",
        attempts.len()
    );
    for attempt in attempts {
        text.push_str(&format!("- {}\n", attempt.describe(branch)));
    }
    text
}

fn git(worktree: &Path, args: &[&str]) -> Result<String, String> {
    local::git_out(worktree, args).map(|out| out.trim().to_owned())
}

/// Serializes fetches across processes and worktrees sharing Git's refs,
/// as `coder_delegate::git_fetch` explains.
pub(super) fn fetch(worktree: &Path, branch: &str) -> Result<(), String> {
    coder_delegate::git_fetch::fetch(worktree, &[branch], git)
}

/// The repository's fetch lock, for callers that wait on it with their own
/// timeout (`freshen`).
pub(super) fn fetch_lock(worktree: &Path) -> Result<std::fs::File, String> {
    coder_delegate::git_fetch::lock_file(worktree, git)
}

/// Lands the change committed at `plan.worktree`'s `HEAD` on
/// `origin/<plan.branch>`, as the module explains.
///
/// # Errors
/// The change did not land: [`NotLanded`] says why and what was tried; the
/// change stays committed in the worktree.
pub fn land(plan: &Plan<'_>, hooks: &mut dyn Hooks) -> Result<Landed, NotLanded> {
    let branch = plan.branch;
    let remote = format!("origin/{branch}");
    let mut attempts: Vec<Attempt> = Vec::new();
    let fail = |failure, attempts| Err(NotLanded { failure, attempts });
    // The base the checks last passed on; the change's parent until a
    // rebase moves it.
    let mut tested_on: Option<String> = None;
    // One engine repair turn for the entire landing.
    let mut repaired = false;
    // Pushes refused in a row while the branch stayed where it was.
    let mut stuck = 0;
    let mut pushed_against: Option<String> = None;
    // This machine's landing lock: taken at the top of each attempt, let go
    // while the checks run and while a refused push waits.
    let mut gate: Option<Box<dyn std::any::Any>> = None;
    for number in 1..=plan.attempts.max(1) {
        if hooks.stopping() {
            return fail(Failure::Stopped, attempts);
        }
        if gate.is_none() {
            gate = hooks.enter();
        }
        if let Err(why) = fetch(plan.worktree, branch) {
            return fail(
                Failure::Unreadable(format!("Git could not fetch {remote}: {why}")),
                attempts,
            );
        }
        let upstream = git(plan.worktree, &["rev-parse", &remote]).unwrap_or_default();
        if upstream.is_empty() {
            return fail(
                Failure::Unreadable(format!("{remote} names no commit.")),
                attempts,
            );
        }
        if pushed_against.as_deref() == Some(upstream.as_str()) {
            stuck += 1;
        } else {
            stuck = 0;
        }
        if stuck >= 2 {
            let why = attempts
                .last()
                .and_then(|a: &Attempt| a.refused.clone())
                .unwrap_or_default();
            return fail(
                Failure::GaveUp(format!(
                    "Git refused the push twice while `{branch}` did not move, so this is not a \
                     race with another landing: {}",
                    clip(&why, 400)
                )),
                attempts,
            );
        }
        let base = match &tested_on {
            Some(base) => base.clone(),
            None => {
                let base = git(plan.worktree, &["merge-base", "HEAD", &upstream])
                    .unwrap_or_else(|_| upstream.clone());
                tested_on = Some(base.clone());
                base
            }
        };
        let mut attempt = Attempt {
            number,
            moved: 0,
            recheck: Recheck::NotMoved,
            refused: None,
            waited: None,
        };
        let contained = git(
            plan.worktree,
            &["merge-base", "--is-ancestor", &upstream, "HEAD"],
        )
        .is_ok();
        if !contained {
            attempt.moved = git(
                plan.worktree,
                &["rev-list", "--count", &format!("{base}..{upstream}")],
            )
            .ok()
            .and_then(|count| count.parse().ok())
            .unwrap_or(0);
            hooks.note(&format!(
                "`{branch}` moved by {} commit(s) to {}; Coder rebases the change onto it.",
                attempt.moved,
                short(&upstream)
            ));
            let mut fixed = false;
            if let Err(why) = git(plan.worktree, &["rebase", "-q", &upstream]) {
                let resolution = if repaired || hooks.stopping() {
                    Err("The landing's one conflict fix turn is already used or stopped.".into())
                } else {
                    repaired = true;
                    let request = conflict_request(plan.worktree, branch, &base, &upstream);
                    hooks.note(
                        "The rebase conflicts; Coder gives the engine one conflict fix turn.",
                    );
                    drop(gate.take());
                    hooks.fix_conflict(&request).and_then(|()| {
                        if hooks.stopping() {
                            return Err("Stopped before continuing the rebase.".into());
                        }
                        git(plan.worktree, &["diff", "--check"])?;
                        git(plan.worktree, &["diff", "--cached", "--check"])?;
                        git(plan.worktree, &["add", "-A"])?;
                        git(
                            plan.worktree,
                            &["-c", "core.editor=true", "rebase", "--continue"],
                        )?;
                        Ok(())
                    })
                };
                if let Err(repair) = resolution {
                    let _ = git(plan.worktree, &["rebase", "--abort"]);
                    attempts.push(attempt);
                    return fail(
                        if hooks.stopping() {
                            Failure::Stopped
                        } else {
                            Failure::Conflict(format!(
                                "The change conflicts with the newer `{branch}` after conflict repair: {} ({})",
                                clip(&why, 400),
                                clip(&repair, 400)
                            ))
                        },
                        attempts,
                    );
                }
                fixed = true;
            }
            match if fixed {
                Some("the engine resolved rebase conflicts".to_owned())
            } else {
                affects(plan.worktree, &base, &upstream)
            } {
                Some(why) => {
                    hooks.note(&format!("Coder runs the checks again: {why}."));
                    // The checks run outside the landing lock, so other runs
                    // here can land meanwhile.
                    drop(gate.take());
                    let problems = hooks.check();
                    if hooks.stopping() {
                        attempts.push(attempt);
                        return fail(Failure::Stopped, attempts);
                    }
                    let passed = problems.is_empty();
                    attempt.recheck = Recheck::Ran { why, passed };
                    if !passed {
                        attempts.push(attempt);
                        return fail(Failure::Red(problems), attempts);
                    }
                    hooks.note("The checks pass on the rebased change.");
                    tested_on = Some(upstream.clone());
                    // Back under the lock: push only if the branch is still
                    // where the checks ran; otherwise the next attempt
                    // rebases again (and re-checks only what it affects).
                    gate = hooks.enter();
                    if let Err(why) = fetch(plan.worktree, branch) {
                        attempts.push(attempt);
                        return fail(
                            Failure::Unreadable(format!("Git could not fetch {remote}: {why}")),
                            attempts,
                        );
                    }
                    let now = git(plan.worktree, &["rev-parse", &remote]).unwrap_or_default();
                    if now != upstream {
                        hooks.note(&format!(
                            "`{branch}` moved again while the checks ran; Coder rebases once \
                             more."
                        ));
                        attempts.push(attempt);
                        continue;
                    }
                }
                None => {
                    let why =
                        "the newly landed commits cannot affect what the change touches".to_owned();
                    hooks.note(&format!("Coder does not run the checks again: {why}."));
                    attempt.recheck = Recheck::Skipped(why);
                }
            }
            tested_on = Some(upstream.clone());
        }
        let head = git(plan.worktree, &["rev-parse", "HEAD"]).unwrap_or_default();
        hooks.note(&format!("Pushing {} to `{branch}`.", short(&head)));
        match git(
            plan.worktree,
            &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
        ) {
            Ok(_) => {
                attempts.push(attempt);
                return Ok(Landed {
                    commit: head,
                    attempts,
                });
            }
            Err(why) => {
                pushed_against = Some(upstream);
                attempt.refused = Some(why.clone());
                if number < plan.attempts {
                    let wait = plan.backoff.delay(number);
                    attempt.waited = Some(wait);
                    hooks.note(&format!(
                        "The push was refused ({}); Coder tries again in {:.1} s.",
                        clip(&why, 200),
                        wait.as_secs_f64()
                    ));
                    attempts.push(attempt);
                    drop(gate.take());
                    if !pause(wait, hooks) {
                        return fail(Failure::Stopped, attempts);
                    }
                } else {
                    attempts.push(attempt);
                    return fail(
                        Failure::GaveUp(format!(
                            "Git refused the push {} time(s), the last time: {}",
                            plan.attempts,
                            clip(&why, 400)
                        )),
                        attempts,
                    );
                }
            }
        }
    }
    fail(
        Failure::GaveUp(format!(
            "`{branch}` kept moving during checks; landing exhausted {} attempt(s).",
            plan.attempts.max(1)
        )),
        attempts,
    )
}

/// Captures the active conflict before the engine changes it.
fn conflict_request(worktree: &Path, branch: &str, base: &str, upstream: &str) -> String {
    let ours = git(
        worktree,
        &["log", "--format=%h %B", &format!("{base}..ORIG_HEAD")],
    )
    .unwrap_or_default();
    let theirs = git(
        worktree,
        &["log", "--format=%h %B", &format!("{base}..{upstream}")],
    )
    .unwrap_or_default();
    let markers = git(worktree, &["diff", "--no-ext-diff", "--cc"]).unwrap_or_default();
    format!(
        "Resolve the active rebase conflicts onto `{branch}` in one fix turn. Preserve the intent \
         of both sides. Remove conflict markers and resolve any deleted files. Do not commit, \
         abort, or continue the rebase, and do not push. The host stages your resolution, \
         continues the rebase, runs the repository checks, and retries landing.\n\n\
         Change commit messages:\n{ours}\n\nNewer {branch} commit messages:\n{theirs}\n\n\
         Conflict markers and both sides:\n{markers}"
    )
}

/// Sleeps `wait`, waking to see whether to stop; false when asked to.
fn pause(wait: Duration, hooks: &dyn Hooks) -> bool {
    let until = Instant::now() + wait;
    loop {
        if hooks.stopping() {
            return false;
        }
        let now = Instant::now();
        if now >= until {
            return true;
        }
        std::thread::sleep((until - now).min(Duration::from_millis(200)));
    }
}

/// Files that make every package build differently.
pub(super) fn workspace_wide(path: &str) -> bool {
    matches!(
        path,
        "Cargo.toml" | "rust-toolchain" | "rust-toolchain.toml" | "build.rs"
    ) || path.starts_with(".cargo/")
}

/// Why the commits from `base` to `upstream` can affect the change now
/// rebased onto `upstream` at `worktree`'s `HEAD`, or `None` when they
/// cannot, so its checks need not run again:
///
/// - they touch a workspace-wide build file → they can;
/// - they change the resolved dependencies of a changed package → they can;
/// - the run changes `Cargo.lock` itself → re-run conservatively;
/// - they touch one of the change's packages, a package those depend on, or one that
///   depends on them (tests and dev-dependencies included) → they can;
/// - the change touches files outside any package (its diff checks read
///   links and paths) and they delete or rename a file → they can;
/// - the package graph cannot be read while they touch some package → they
///   can, since unsure means re-run;
/// - otherwise (docs, or packages unrelated to the change) → they cannot.
#[must_use]
pub fn affects(worktree: &Path, base: &str, upstream: &str) -> Option<String> {
    let landed = git(
        worktree,
        &["diff", "--name-status", "--no-renames", base, upstream],
    )
    .unwrap_or_default();
    let mut landed_files = Vec::new();
    let mut deleted = false;
    for line in landed.lines() {
        let Some((status, path)) = line.split_once('\t') else {
            continue;
        };
        deleted |= status.starts_with('D');
        landed_files.push(path.to_owned());
    }
    let change = git(worktree, &["diff", "--name-only", upstream, "HEAD"]).unwrap_or_default();
    let change_files: Vec<String> = change.lines().map(str::to_owned).collect();
    if change_files.iter().any(|file| file == "Cargo.lock") {
        return Some("the change itself touches `Cargo.lock`".to_owned());
    }
    if let Some(file) = landed_files.iter().find(|f| workspace_wide(f)) {
        return Some(format!(
            "they change `{file}`, which every package builds with"
        ));
    }
    let landed_packages = packages(worktree, &landed_files);
    let change_packages = packages(worktree, &change_files);
    if landed_files.iter().any(|file| file == "Cargo.lock") && !change_packages.is_empty() {
        let unchanged = git(worktree, &["show", &format!("{base}:Cargo.lock")])
            .ok()
            .zip(git(worktree, &["show", &format!("HEAD:Cargo.lock")]).ok())
            .and_then(|(before, after)| {
                Some(
                    lock_resolution(&before, &change_packages)?
                        == lock_resolution(&after, &change_packages)?,
                )
            });
        if unchanged != Some(true) {
            return Some(format!(
                "they change `Cargo.lock`, and the resolved dependencies of {} changed or could not be read",
                names(&change_packages)
            ));
        }
    }
    let outside = change_files
        .iter()
        .any(|file| packages(worktree, std::slice::from_ref(file)).is_empty());
    if outside && deleted {
        return Some(
            "they delete files, and the change's diff checks read links and paths".to_owned(),
        );
    }
    if landed_packages.is_empty() || change_packages.is_empty() {
        return None;
    }
    let Some(graph) = workspace_graph(worktree) else {
        return Some(format!(
            "they touch {}, and the package graph could not be read",
            names(&landed_packages)
        ));
    };
    let related: Vec<String> = landed_packages
        .iter()
        .filter(|landed| {
            change_packages.iter().any(|changed| {
                changed == *landed
                    || reaches(&graph, changed, landed)
                    || reaches(&graph, landed, changed)
            })
        })
        .cloned()
        .collect();
    if related.is_empty() {
        None
    } else {
        Some(format!(
            "they touch {}, related to the change's {}",
            names(&related),
            names(&change_packages)
        ))
    }
}

/// Compares package identities and resolved edges, not lockfile ordering. Include
/// every dependency kind and target; uncertain resolution must not skip checks.
fn lock_resolution(text: &str, roots: &[String]) -> Option<std::collections::BTreeSet<String>> {
    #[derive(serde::Deserialize)]
    struct Lock {
        version: u32,
        package: Vec<Package>,
    }
    #[derive(serde::Deserialize)]
    struct Package {
        name: String,
        version: String,
        source: Option<String>,
        checksum: Option<String>,
        #[serde(default)]
        dependencies: Vec<String>,
    }
    let lock: Lock = toml::from_str(text).ok()?;
    if !(3..=4).contains(&lock.version) {
        return None;
    }
    let identity = |p: &Package| (p.name.clone(), p.version.clone(), p.source.clone());
    let mut identities = std::collections::BTreeSet::new();
    for p in &lock.package {
        if !identities.insert(identity(p)) {
            return None;
        }
    }
    let mut next = Vec::new();
    for root in roots {
        let candidates: Vec<_> = lock
            .package
            .iter()
            .enumerate()
            .filter(|(_, p)| p.name == *root && p.source.is_none())
            .collect();
        if candidates.len() != 1 {
            return None;
        }
        next.push(candidates[0].0);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut resolution = std::collections::BTreeSet::new();
    while let Some(index) = next.pop() {
        if !seen.insert(index) {
            continue;
        }
        let p = &lock.package[index];
        let mut edges = std::collections::BTreeSet::new();
        for dependency in &p.dependencies {
            let mut parts = dependency.splitn(3, ' ');
            let name = parts.next()?;
            let version = parts.next();
            let source = parts.next().map(|s| s.strip_prefix('(')?.strip_suffix(')'));
            let source = match source {
                Some(Some(source)) => Some(source),
                Some(None) => return None,
                None => None,
            };
            let candidates: Vec<_> = lock
                .package
                .iter()
                .enumerate()
                .filter(|(_, candidate)| {
                    candidate.name == name
                        && version.is_none_or(|v| candidate.version == v)
                        && source.is_none_or(|s| candidate.source.as_deref() == Some(s))
                })
                .collect();
            if candidates.len() != 1 {
                return None;
            }
            let (target, candidate) = candidates[0];
            edges.insert(identity(candidate));
            next.push(target);
        }
        resolution.insert(format!("{:?}", (identity(p), &p.checksum, edges)));
    }
    Some(resolution)
}

/// The workspace packages `files` are in.
pub(super) fn packages(worktree: &Path, files: &[String]) -> Vec<String> {
    let diff: String = files.iter().map(|file| format!("+++ b/{file}\n")).collect();
    coder_delegate::issue::changed_packages(worktree, &diff)
}

/// Each workspace package's workspace dependencies, of every kind, from
/// `cargo metadata`; `None` when it cannot run.
pub(super) fn workspace_graph(worktree: &Path) -> Option<Vec<(String, Vec<String>)>> {
    let output = std::process::Command::new("cargo")
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(worktree)
        .stdin(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let listed = metadata["packages"].as_array()?;
    let members: Vec<String> = listed
        .iter()
        .filter_map(|package| package["name"].as_str().map(str::to_owned))
        .collect();
    Some(
        listed
            .iter()
            .filter_map(|package| {
                let name = package["name"].as_str()?.to_owned();
                let depends = package["dependencies"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|dependency| dependency["name"].as_str())
                    .filter(|dependency| members.iter().any(|member| member == dependency))
                    .map(str::to_owned)
                    .collect();
                Some((name, depends))
            })
            .collect(),
    )
}

/// Whether `from` depends on `to`, directly or through other packages.
pub(super) fn reaches(graph: &[(String, Vec<String>)], from: &str, to: &str) -> bool {
    let mut seen: Vec<&str> = Vec::new();
    let mut next = vec![from];
    while let Some(at) = next.pop() {
        if seen.contains(&at) {
            continue;
        }
        seen.push(at);
        for (name, depends) in graph {
            if name == at {
                for dependency in depends {
                    if dependency == to {
                        return true;
                    }
                    next.push(dependency);
                }
            }
        }
    }
    false
}

fn names(packages: &[String]) -> String {
    packages
        .iter()
        .map(|package| format!("`{package}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn short(commit: &str) -> &str {
    &commit[..10.min(commit.len())]
}

fn clip(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.len() <= max {
        return text.to_owned();
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

#[cfg(test)]
#[path = "landing_tests.rs"]
mod tests;

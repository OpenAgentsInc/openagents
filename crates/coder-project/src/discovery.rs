//! External-agent discovery for the project supervisor.
//!
//! The supervisor's `external_owners` and `external_reservation` are
//! operator declarations, and the implementation boundary records that no
//! other agent is discovered automatically. This scan closes the
//! observation half of that gap without crossing into authority: it looks
//! for evidence of other agents working the same repository — `.coder-git`
//! scratch repositories inside worktree directories, the repository's own
//! `git worktree list` registrations, and live `coder`, `devin`, and
//! `coder-worker` processes — and returns suggested `external_owners`
//! records for review.
//!
//! Discovery observes; the operator or a later admission step decides. A
//! finding is evidence, never authority: a discovered worktree does not
//! prove ownership of its paths, and a matching process name is a label,
//! not proof the process is an agent. Nothing here is applied. What the
//! scan cannot determine — unreadable worktree metadata, a registration
//! whose checkout has vanished, a missing `.coder-git` — comes back as an
//! `unknown` finding rather than silence, because an unobserved agent is
//! a blind spot to record, not an assumption of absence.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use coder_scheduler::plan::Exclusion;
use serde::{Deserialize, Serialize};

/// The most worktree candidates one scan inspects.
pub const WORKTREES_MAX: usize = 64;
/// The most process-table rows one scan reads.
pub const PROCESSES_MAX: usize = 512;
/// A worktree whose metadata changed inside this window reads `recent`.
const RECENT: Duration = Duration::from_secs(6 * 60 * 60);
/// One Git observation's wall bound.
const GIT_WALL: Duration = Duration::from_secs(15);
/// The scratch Git directory a delegate's checkout carries.
const SCRATCH_GIT_DIR: &str = ".coder-git";
/// Process names a scan treats as agent evidence.
const AGENT_COMMANDS: &[&str] = &["coder", "devin", "coder-worker"];

/// What a suggested external owner rests on.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Evidence {
    /// A live `coder`, `devin`, or `coder-worker` process row.
    Process,
    /// A `.coder-git` scratch repository the scan could read.
    ScratchGit,
    /// `git worktree list` names the directory.
    Registration,
    /// A directory under the scanned worktree parent.
    Directory,
}

/// Whether the scan saw the evidence or concluded it.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Observation {
    /// Directly observed: scratch history read, a registration listed, a
    /// process row matched, a directory's own metadata seen.
    Observed,
    /// Concluded rather than confirmed: an unregistered directory without
    /// readable scratch, or a blind spot the scan ran into.
    Inferred,
}

/// How fresh a finding's evidence is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Staleness {
    /// A live process backs it.
    Live,
    /// Its metadata changed inside the recent window.
    Recent,
    /// Nothing fresh backs it.
    Stale,
    /// The scan could not tell.
    Unknown,
}

/// Whether the work a finding names looks in flight.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Activity {
    /// A worktree's scratch holds commits ahead of its seeded base or
    /// uncommitted work; a process finding is a running agent.
    Active,
    /// The scratch sits clean at its seeded base.
    Idle,
    /// The scan could not tell.
    Unknown,
}

/// One suggested `external_owners` record and what backs it.
///
/// `owner` and `writes` are the configurable half; the rest is the
/// evidence a reviewer checks before accepting the suggestion.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Finding {
    /// The suggested owner label.
    pub owner: String,
    /// Suggested declared write paths, repository-relative, where the
    /// evidence names any.
    pub writes: Vec<String>,
    /// The kind of evidence behind the suggestion.
    pub evidence: Evidence,
    /// Whether the evidence was observed or inferred.
    pub observation: Observation,
    /// How fresh the evidence is.
    pub staleness: Staleness,
    /// Whether the work looks in flight.
    pub activity: Activity,
    /// The path the finding concerns, when it has one.
    pub path: Option<PathBuf>,
    /// The process the finding names, when it names one.
    pub pid: Option<u32>,
    /// What the scan saw, including what it could not determine.
    pub detail: String,
}

/// What one scan found: a suggestion set, never an admission.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Discovery {
    /// The repository the scan ran against, canonicalized.
    pub repository: PathBuf,
    /// Every finding: worktrees by path, then processes by pid, then what
    /// the scan could not observe.
    pub findings: Vec<Finding>,
}

impl Discovery {
    /// Scan for agents this supervisor did not dispatch.
    ///
    /// `state_dir` is the directory whose immediate children are worktree
    /// candidates — the repository's `.coder/worktrees` or its
    /// equivalent. `process_table` is captured `ps -eo pid,comm,args`
    /// output, supplied so the caller owns when and how it is read.
    /// Everything considered must resolve inside `repository_root`;
    /// anything outside it is skipped and symbolic links are never
    /// followed out.
    #[must_use]
    pub fn scan(repository_root: &Path, state_dir: &Path, process_table: &str) -> Self {
        let mut worktrees = Vec::new();
        let mut running = Vec::new();
        let mut blind = Vec::new();
        let Ok(repository) = repository_root.canonicalize() else {
            blind.push(blind_spot(
                "unobserved-repository",
                Evidence::Directory,
                format!(
                    "the repository root {} cannot be resolved",
                    repository_root.display()
                ),
            ));
            return Self {
                repository: repository_root.to_path_buf(),
                findings: blind,
            };
        };

        let (rows, unreadable_table) = agent_rows(process_table);
        if let Some(finding) = unreadable_table {
            blind.push(finding);
        }
        let live: BTreeSet<u32> = rows.iter().map(|row| row.pid).collect();
        let paths: Vec<BTreeSet<PathBuf>> = rows
            .iter()
            .map(|row| repository_paths(&row.args, &repository, repository_root))
            .collect();

        // Candidates: directories under the scanned parent, then
        // registrations Git still holds.
        let mut candidates: BTreeMap<PathBuf, bool> = BTreeMap::new();
        match state_dir.canonicalize() {
            Err(error) => blind.push(blind_spot(
                "unobserved-worktrees",
                Evidence::Directory,
                format!(
                    "the worktree directory {} cannot be resolved: {error}",
                    state_dir.display()
                ),
            )),
            Ok(directory) if !directory.starts_with(&repository) => blind.push(blind_spot(
                "unobserved-worktrees",
                Evidence::Directory,
                "the scanned worktree directory resolves outside the repository root".into(),
            )),
            Ok(directory) => match std::fs::read_dir(&directory) {
                Err(error) => blind.push(blind_spot(
                    "unobserved-worktrees",
                    Evidence::Directory,
                    format!(
                        "the worktree directory {} cannot be read: {error}",
                        directory.display()
                    ),
                )),
                Ok(entries) => {
                    let mut entries: Vec<PathBuf> =
                        entries.flatten().map(|entry| entry.path()).collect();
                    entries.sort();
                    let overflow = entries.len().saturating_sub(WORKTREES_MAX);
                    if overflow > 0 {
                        blind.push(blind_spot(
                            "unobserved-worktrees",
                            Evidence::Directory,
                            format!(
                                "the scan stopped at {WORKTREES_MAX} entries; {overflow} more are unobserved"
                            ),
                        ));
                    }
                    for path in entries.into_iter().take(WORKTREES_MAX) {
                        match std::fs::symlink_metadata(&path) {
                            Err(error) => worktrees.push(Finding {
                                owner: format!("worktree-{}", name_of(&path)),
                                writes: vec![],
                                evidence: Evidence::Directory,
                                observation: Observation::Inferred,
                                staleness: Staleness::Unknown,
                                activity: Activity::Unknown,
                                path: Some(path),
                                pid: None,
                                detail: format!("worktree metadata cannot be read: {error}"),
                            }),
                            Ok(metadata) if metadata.file_type().is_symlink() => {
                                worktrees.push(Finding {
                                    owner: format!("worktree-{}", name_of(&path)),
                                    writes: vec![],
                                    evidence: Evidence::Directory,
                                    observation: Observation::Observed,
                                    staleness: Staleness::Unknown,
                                    activity: Activity::Unknown,
                                    path: Some(path),
                                    pid: None,
                                    detail: "the entry is a symbolic link; it was not followed"
                                        .into(),
                                });
                            }
                            Ok(metadata) if metadata.is_dir() => match path.canonicalize() {
                                Ok(path) if path.starts_with(&repository) => {
                                    candidates.entry(path).or_insert(false);
                                }
                                Ok(_) => {}
                                Err(error) => worktrees.push(Finding {
                                    owner: format!("worktree-{}", name_of(&path)),
                                    writes: vec![],
                                    evidence: Evidence::Directory,
                                    observation: Observation::Inferred,
                                    staleness: Staleness::Unknown,
                                    activity: Activity::Unknown,
                                    path: Some(path),
                                    pid: None,
                                    detail: format!("the worktree cannot be resolved: {error}"),
                                }),
                            },
                            Ok(_) => {}
                        }
                    }
                }
            },
        }
        match git(&repository, &["worktree", "list", "--porcelain"]) {
            Err(error) => blind.push(blind_spot(
                "unobserved-registrations",
                Evidence::Registration,
                format!("the repository's worktree registrations cannot be read: {error}"),
            )),
            Ok(list) => {
                for line in list
                    .lines()
                    .filter_map(|line| line.strip_prefix("worktree "))
                {
                    let path = PathBuf::from(line.trim());
                    match path.canonicalize() {
                        Ok(path) if path == repository => {}
                        Ok(path) if path.starts_with(&repository) => {
                            candidates
                                .entry(path)
                                .and_modify(|registered| *registered = true)
                                .or_insert(true);
                        }
                        Ok(_) => {}
                        Err(_) => worktrees.push(Finding {
                            owner: format!("registered-{}", name_of(&path)),
                            writes: vec![],
                            evidence: Evidence::Registration,
                            observation: Observation::Observed,
                            staleness: Staleness::Unknown,
                            activity: Activity::Unknown,
                            path: Some(path),
                            pid: None,
                            detail: "a registered worktree has vanished from disk".into(),
                        }),
                    }
                }
            }
        }

        // Correlate process rows to candidates: a row whose arguments
        // name the worktree, or a worktree named for a live pid.
        for (path, registered) in &candidates {
            let registered = *registered;
            let name = name_of(path);
            let mut pids: BTreeSet<u32> = rows
                .iter()
                .zip(&paths)
                .filter(|(_, named)| named.iter().any(|named| named.starts_with(path)))
                .map(|(row, _)| row.pid)
                .collect();
            let mut notes = Vec::new();
            if let Some(pid) = creator_pid(&name) {
                if live.contains(&pid) {
                    pids.insert(pid);
                } else {
                    notes.push(format!(
                        "the directory names creator process {pid}, which is not in the supplied process table"
                    ));
                }
            }
            let (activity, state_note) = scratch_state(path);
            if let Some(note) = state_note {
                notes.push(note);
            }
            for pid in &pids {
                if let Some(row) = rows.iter().find(|row| row.pid == *pid) {
                    notes.push(format!(
                        "live {} process {pid} names this worktree",
                        row.command
                    ));
                }
            }
            let observation = if activity != Activity::Unknown || registered {
                Observation::Observed
            } else {
                Observation::Inferred
            };
            let evidence = if activity != Activity::Unknown {
                Evidence::ScratchGit
            } else if registered {
                Evidence::Registration
            } else {
                Evidence::Directory
            };
            worktrees.push(Finding {
                owner: format!("worktree-{name}"),
                writes: vec![relative(&repository, path)],
                evidence,
                observation,
                staleness: staleness(path, !pids.is_empty()),
                activity,
                path: Some(path.clone()),
                pid: pids.first().copied(),
                detail: if notes.is_empty() {
                    "a worktree of this repository".into()
                } else {
                    notes.join("; ")
                },
            });
        }

        // A row whose named paths all sit inside a candidate is folded
        // into that worktree's finding. A row with no repository path is
        // not evidence about this repository and is ignored.
        for (row, named) in rows.iter().zip(&paths) {
            if named.is_empty()
                || named
                    .iter()
                    .any(|named| candidates.keys().any(|wt| named.starts_with(wt)))
            {
                continue;
            }
            let writes: Vec<String> = named
                .iter()
                .filter(|named| *named != &repository)
                .map(|named| relative(&repository, named))
                .collect();
            running.push(Finding {
                owner: format!("{}-{}", row.command, row.pid),
                writes,
                evidence: Evidence::Process,
                observation: Observation::Observed,
                staleness: Staleness::Live,
                activity: Activity::Active,
                path: named.first().cloned(),
                pid: Some(row.pid),
                detail: format!(
                    "live {} process names repository paths: {}",
                    row.command,
                    named
                        .iter()
                        .map(|path| path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            });
        }

        worktrees.sort_by(|a, b| (&a.path, &a.owner).cmp(&(&b.path, &b.owner)));
        running.sort_by(|a, b| (a.pid, &a.owner).cmp(&(b.pid, &b.owner)));
        worktrees.extend(running);
        worktrees.extend(blind);
        Self {
            repository,
            findings: worktrees,
        }
    }

    /// The findings in the configuration's `external_owners` shape.
    ///
    /// Suggestions without derivable paths come back with an empty
    /// `writes`; the configuration refuses an owner without declared
    /// paths until a host completes them.
    #[must_use]
    pub fn suggestions(&self) -> Vec<Exclusion> {
        self.findings
            .iter()
            .map(|finding| Exclusion {
                owner: finding.owner.clone(),
                writes: finding.writes.clone(),
            })
            .collect()
    }
}

/// A process-table row whose command names an agent.
struct Row {
    pid: u32,
    command: String,
    args: String,
}

/// A blind spot: evidence the scan could not reach, recorded as `unknown`.
fn blind_spot(owner: &str, evidence: Evidence, detail: String) -> Finding {
    Finding {
        owner: owner.into(),
        writes: vec![],
        evidence,
        observation: Observation::Inferred,
        staleness: Staleness::Unknown,
        activity: Activity::Unknown,
        path: None,
        pid: None,
        detail,
    }
}

/// Parse captured `ps -eo pid,comm,args` output, keeping agent-named rows.
/// A table that holds no readable row at all is a blind spot, not silence.
fn agent_rows(table: &str) -> (Vec<Row>, Option<Finding>) {
    if table.trim().is_empty() {
        return (
            vec![],
            Some(blind_spot(
                "unobserved-processes",
                Evidence::Process,
                "no process table was supplied; live agents cannot be observed".into(),
            )),
        );
    }
    let mut rows = Vec::new();
    let mut malformed = 0_u32;
    let mut overflow = false;
    for (index, line) in table.lines().enumerate() {
        if index >= PROCESSES_MAX {
            overflow = true;
            break;
        }
        let mut fields = line.split_whitespace();
        let Some(first) = fields.next() else { continue };
        match first.parse::<u32>() {
            Ok(pid) => {
                let Some(comm) = fields.next() else {
                    malformed += 1;
                    continue;
                };
                let command = Path::new(comm).file_name().map_or_else(
                    || comm.to_string(),
                    |name| name.to_string_lossy().into_owned(),
                );
                if AGENT_COMMANDS.contains(&command.as_str()) {
                    rows.push(Row {
                        pid,
                        command,
                        args: fields.collect::<Vec<_>>().join(" "),
                    });
                }
            }
            // A header or blank line is not a malformed row.
            Err(_) if index == 0 || line.trim().is_empty() => {}
            Err(_) => malformed += 1,
        }
    }
    rows.sort_by_key(|row| row.pid);
    let note = if overflow {
        Some(blind_spot(
            "unobserved-processes",
            Evidence::Process,
            format!("the process table exceeded {PROCESSES_MAX} rows; the remainder was not read"),
        ))
    } else if rows.is_empty() && malformed > 0 {
        Some(blind_spot(
            "unobserved-processes",
            Evidence::Process,
            "the process table could not be parsed; live agents cannot be observed".into(),
        ))
    } else {
        None
    };
    (rows, note)
}

/// The argument tokens that resolve inside the repository, canonicalized.
/// Tokens are untrusted process output; only resolved paths count.
fn repository_paths(args: &str, canonical: &Path, as_typed: &Path) -> BTreeSet<PathBuf> {
    let mut paths = BTreeSet::new();
    for token in args
        .split_whitespace()
        .filter(|token| token.starts_with('/'))
    {
        let token = Path::new(token);
        if let Ok(resolved) = token.canonicalize() {
            if resolved.starts_with(canonical) {
                paths.insert(resolved);
            }
        } else if token.starts_with(canonical) || token.starts_with(as_typed) {
            // A named path under the repository that no longer resolves
            // is still evidence the process works here.
            paths.insert(token.to_path_buf());
        }
    }
    paths
}

/// The pid a worktree directory name encodes: `<pid>-<counter>`, the
/// shape the runtime's own checkouts are reserved under.
fn creator_pid(name: &str) -> Option<u32> {
    let (pid, rest) = name.split_once('-')?;
    if rest.is_empty() || !rest.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    pid.parse().ok()
}

/// A path's last component for labels.
fn name_of(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}

/// A path as a normalized repository-relative suggestion.
fn relative(repository: &Path, path: &Path) -> String {
    path.strip_prefix(repository)
        .unwrap_or(path)
        .display()
        .to_string()
}

/// A worktree's staleness: a correlated live process, else metadata age.
fn staleness(worktree: &Path, live: bool) -> Staleness {
    if live {
        return Staleness::Live;
    }
    let modified = std::fs::symlink_metadata(worktree.join(SCRATCH_GIT_DIR))
        .or_else(|_| std::fs::symlink_metadata(worktree))
        .and_then(|metadata| metadata.modified());
    match modified {
        Ok(time) => match SystemTime::now().duration_since(time) {
            Ok(age) if age <= RECENT => Staleness::Recent,
            Ok(_) => Staleness::Stale,
            // A clock ahead of the metadata still reads fresh.
            Err(_) => Staleness::Recent,
        },
        Err(_) => Staleness::Unknown,
    }
}

/// One worktree's scratch state: commits ahead of the seeded base or
/// uncommitted work is `active`, clean at the base is `idle`, and
/// anything the scan cannot read is `unknown` with the reason named.
fn scratch_state(worktree: &Path) -> (Activity, Option<String>) {
    let metadata = match std::fs::symlink_metadata(worktree.join(SCRATCH_GIT_DIR)) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return (
                Activity::Unknown,
                Some("no .coder-git scratch repository is present".into()),
            );
        }
        Err(error) => {
            return (
                Activity::Unknown,
                Some(format!("scratch metadata cannot be read: {error}")),
            );
        }
    };
    if metadata.file_type().is_symlink() {
        return (
            Activity::Unknown,
            Some("the scratch Git directory is a symbolic link; it was not followed".into()),
        );
    }
    if !metadata.is_dir() {
        return (
            Activity::Unknown,
            Some("the scratch Git path is not a directory".into()),
        );
    }
    let commits = scratch(worktree, &["rev-list", "--count", "HEAD"]);
    let status = scratch(
        worktree,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    );
    match (commits, status) {
        (Ok(commits), Ok(status)) => match commits.trim().parse::<u64>() {
            Ok(count) if count > 1 || !status.trim().is_empty() => (Activity::Active, None),
            Ok(_) => (Activity::Idle, None),
            Err(_) => (
                Activity::Unknown,
                Some("the scratch commit count was not a number".into()),
            ),
        },
        (Err(error), _) => (
            Activity::Unknown,
            Some(format!("scratch history cannot be read: {error}")),
        ),
        (_, Err(error)) => (
            Activity::Unknown,
            Some(format!("scratch cleanliness cannot be read: {error}")),
        ),
    }
}

/// One bounded read of a worktree's scratch Git directory.
fn scratch(worktree: &Path, args: &[&str]) -> Result<String, String> {
    let mut full = vec![
        "--git-dir=.coder-git",
        "--work-tree=.",
        "-c",
        "core.fsmonitor=false",
    ];
    full.extend_from_slice(args);
    git(worktree, &full)
}

/// One bounded Git observation, through the same supervised run a
/// capability probe uses. A nonzero exit or a truncated stream is not an
/// answer.
fn git(directory: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .current_dir(directory)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .env_remove("GIT_OBJECT_DIRECTORY")
        .env_remove("GIT_ALTERNATE_OBJECT_DIRECTORIES")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .args(["--no-pager", "--no-replace-objects"])
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(args);
    let said = coder::capability::bounded::run(command, GIT_WALL)
        .map_err(|stop| format!("bounded Git observation failed: {stop}"))?;
    if said.code != Some(0) || said.truncated {
        return Err(format!(
            "bounded Git observation failed: exit {:?}: {}",
            said.code,
            said.err.trim()
        ));
    }
    Ok(said.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A repository with a `.coder/worktrees` parent and one commit.
    struct Fixture {
        _held: tempfile::TempDir,
        repository: PathBuf,
        worktrees: PathBuf,
    }

    fn run(directory: &Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(directory)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn fixture() -> Fixture {
        let held = tempfile::tempdir().unwrap();
        let repository = held.path().join("repository");
        std::fs::create_dir(&repository).unwrap();
        run(&repository, &["init", "--quiet"]);
        run(
            &repository,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--allow-empty",
                "--quiet",
                "-m",
                "base",
            ],
        );
        let repository = repository.canonicalize().unwrap();
        let worktrees = repository.join(".coder/worktrees");
        std::fs::create_dir_all(&worktrees).unwrap();
        Fixture {
            _held: held,
            repository,
            worktrees,
        }
    }

    /// Seed a worktree's `.coder-git`: one commit at the base, and one
    /// more when `ahead` says the item committed.
    fn seed_scratch(worktree: &Path, ahead: bool) {
        std::fs::write(worktree.join("a.rs"), "a\n").unwrap();
        let scratch = worktree.join(SCRATCH_GIT_DIR);
        let run = |args: &[&str]| {
            let output = Command::new("git")
                .arg("--git-dir")
                .arg(&scratch)
                .arg("--work-tree")
                .arg(worktree)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .args(args)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
        };
        run(&["init", "--quiet"]);
        run(&["config", "user.name", "Test"]);
        run(&["config", "user.email", "test@example.invalid"]);
        std::fs::write(scratch.join("info/exclude"), ".coder-git\n.git\n").unwrap();
        run(&["add", "-A"]);
        run(&["commit", "--quiet", "-m", "Base"]);
        if ahead {
            std::fs::write(worktree.join("b.rs"), "b\n").unwrap();
            run(&["add", "-A"]);
            run(&["commit", "--quiet", "-m", "the item"]);
        }
    }

    /// A captured `ps -eo pid,comm,args` table with the given rows.
    fn table(rows: &[String]) -> String {
        let mut table = "  PID COMM ARGS\n".to_string();
        for row in rows {
            table.push_str(row);
            table.push('\n');
        }
        table
    }

    fn finding<'a>(discovery: &'a Discovery, worktree: &Path) -> &'a Finding {
        discovery
            .findings
            .iter()
            .find(|finding| finding.path.as_deref() == Some(worktree))
            .expect("a finding names the worktree")
    }

    #[test]
    fn a_worktree_ahead_of_base_suggests_an_active_owner() {
        let fixture = fixture();
        let worktree = fixture.worktrees.join("4242-1");
        std::fs::create_dir(&worktree).unwrap();
        seed_scratch(&worktree, true);
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&[]));
        let finding = finding(&discovery, &worktree);
        assert_eq!(finding.owner, "worktree-4242-1");
        assert_eq!(finding.activity, Activity::Active);
        assert_eq!(finding.evidence, Evidence::ScratchGit);
        assert_eq!(finding.observation, Observation::Observed);
        assert_eq!(finding.writes, [".coder/worktrees/4242-1"]);
        assert!(finding.detail.contains("4242"));
    }

    #[test]
    fn a_clean_worktree_at_base_suggests_idle() {
        let fixture = fixture();
        let worktree = fixture.worktrees.join("4242-2");
        std::fs::create_dir(&worktree).unwrap();
        seed_scratch(&worktree, false);
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&[]));
        let finding = finding(&discovery, &worktree);
        assert_eq!(finding.activity, Activity::Idle);
        assert_eq!(finding.staleness, Staleness::Recent);
        assert_eq!(finding.pid, None);
    }

    #[test]
    fn a_process_naming_the_worktree_correlates() {
        let fixture = fixture();
        let worktree = fixture.worktrees.join("wt-alpha");
        std::fs::create_dir(&worktree).unwrap();
        seed_scratch(&worktree, false);
        let rows = [format!("9001 devin devin --workdir {}", worktree.display())];
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&rows));
        let finding = finding(&discovery, &worktree);
        assert_eq!(finding.staleness, Staleness::Live);
        assert_eq!(finding.pid, Some(9001));
        assert!(finding.detail.contains("devin"));
        // The correlated process folds into the worktree's finding.
        assert!(
            discovery
                .findings
                .iter()
                .all(|finding| finding.evidence != Evidence::Process)
        );
    }

    #[test]
    fn a_worktree_named_for_a_live_pid_correlates() {
        let fixture = fixture();
        let worktree = fixture.worktrees.join("9002-7");
        std::fs::create_dir(&worktree).unwrap();
        seed_scratch(&worktree, false);
        let rows = ["9002 coder-worker coder-worker --relay wss://example.invalid".to_string()];
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&rows));
        let finding = finding(&discovery, &worktree);
        assert_eq!(finding.staleness, Staleness::Live);
        assert_eq!(finding.pid, Some(9002));
    }

    #[test]
    fn a_process_outside_the_repository_is_ignored() {
        let fixture = fixture();
        let outside = tempfile::tempdir().unwrap();
        let rows = [
            format!(
                "9003 devin devin --workdir {}",
                outside.path().canonicalize().unwrap().display()
            ),
            "9004 devin devin".to_string(),
        ];
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&rows));
        assert!(
            discovery
                .findings
                .iter()
                .all(|finding| finding.pid.is_none())
        );
    }

    #[test]
    fn a_process_inside_the_repository_suggests_an_owner() {
        let fixture = fixture();
        let rows = [format!(
            "9005 devin devin --workdir {}",
            fixture.repository.display()
        )];
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&rows));
        let finding = discovery
            .findings
            .iter()
            .find(|finding| finding.evidence == Evidence::Process)
            .expect("a process finding names the repository");
        assert_eq!(finding.owner, "devin-9005");
        assert_eq!(finding.staleness, Staleness::Live);
        assert_eq!(finding.activity, Activity::Active);
    }

    #[test]
    fn missing_scratch_reports_unknown() {
        let fixture = fixture();
        let worktree = fixture.worktrees.join("4242-3");
        std::fs::create_dir(&worktree).unwrap();
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&[]));
        let finding = finding(&discovery, &worktree);
        assert_eq!(finding.activity, Activity::Unknown);
        assert!(finding.detail.contains("no .coder-git"));
    }

    #[test]
    fn unreadable_scratch_reports_unknown() {
        let fixture = fixture();
        let worktree = fixture.worktrees.join("4242-4");
        std::fs::create_dir(&worktree).unwrap();
        std::fs::create_dir(worktree.join(SCRATCH_GIT_DIR)).unwrap();
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&[]));
        let finding = finding(&discovery, &worktree);
        assert_eq!(finding.activity, Activity::Unknown);
        assert!(finding.detail.contains("scratch history"));
    }

    #[test]
    fn a_registered_worktree_that_vanished_reports_unknown() {
        let fixture = fixture();
        run(
            &fixture.repository,
            &["worktree", "add", "--detach", "gone", "HEAD"],
        );
        std::fs::remove_dir_all(fixture.repository.join("gone")).unwrap();
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&[]));
        let finding = discovery
            .findings
            .iter()
            .find(|finding| finding.owner == "registered-gone")
            .expect("the vanished registration is reported");
        assert_eq!(finding.activity, Activity::Unknown);
        assert_eq!(finding.evidence, Evidence::Registration);
        assert!(finding.detail.contains("vanished"));
    }

    #[test]
    fn a_symlinked_entry_is_not_followed() {
        let fixture = fixture();
        let outside = tempfile::tempdir().unwrap();
        let link = fixture.worktrees.join("linked");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&[]));
        let finding = finding(&discovery, &link);
        assert_eq!(finding.activity, Activity::Unknown);
        assert!(finding.detail.contains("not followed"));
    }

    #[test]
    fn an_empty_process_table_is_a_blind_spot() {
        let fixture = fixture();
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, "");
        assert!(discovery.findings.iter().any(|finding| {
            finding.evidence == Evidence::Process && finding.observation == Observation::Inferred
        }));
    }

    #[test]
    fn suggestions_carry_the_external_owner_shape() {
        let fixture = fixture();
        let worktree = fixture.worktrees.join("4242-5");
        std::fs::create_dir(&worktree).unwrap();
        seed_scratch(&worktree, true);
        let discovery = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&[]));
        let suggestion = discovery
            .suggestions()
            .into_iter()
            .find(|suggestion| suggestion.owner == "worktree-4242-5")
            .unwrap();
        assert_eq!(suggestion.writes, [".coder/worktrees/4242-5"]);
        assert!(
            fixture
                .repository
                .join(&suggestion.writes[0])
                .canonicalize()
                .is_ok()
        );
    }

    #[test]
    fn the_scan_is_deterministic() {
        let fixture = fixture();
        let worktree = fixture.worktrees.join("4242-6");
        std::fs::create_dir(&worktree).unwrap();
        seed_scratch(&worktree, true);
        let rows = [
            format!("9001 devin devin --workdir {}", worktree.display()),
            "9003 devin devin".to_string(),
        ];
        let first = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&rows));
        let second = Discovery::scan(&fixture.repository, &fixture.worktrees, &table(&rows));
        assert_eq!(first, second);
    }
}

//! Owned scan processes keep a stalled target from holding up the next target.

use super::{Action, Host, Kind, Layout, Policy, Report, System, TargetSize};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

const TARGET_LIMIT: Duration = Duration::from_secs(25);
const PASS_LIMIT: Duration = Duration::from_secs(180);
const GC_LIMIT: Duration = Duration::from_secs(35);
const POLL: Duration = Duration::from_millis(50);
const MAX_JSON: u64 = 64 * 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Serialize, Deserialize)]
enum Work {
    Target(PathBuf),
    CacheGc,
}

#[derive(Serialize, Deserialize)]
struct Job {
    layout: Layout,
    policy: Policy,
    dry_run: bool,
    work: Work,
    result: PathBuf,
}

struct JobDirectory(PathBuf);

impl JobDirectory {
    fn new(layout: &Layout) -> io::Result<Self> {
        let parent = layout.state.parent().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "build sweep state has no folder",
            )
        })?;
        fs::create_dir_all(parent)?;
        loop {
            let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                "build-sweep-worker-{}-{sequence}",
                std::process::id()
            ));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt as _;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
    }
}

impl Drop for JobDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> io::Result<T> {
    let mut bytes = Vec::new();
    File::open(path)?
        .take(MAX_JSON + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_JSON {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "sweep report is too large",
        ));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn write_json(path: &Path, value: &impl Serialize) -> io::Result<()> {
    let staging = path.with_extension(format!(
        "{}.{}.new",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(&staging)?;
    serde_json::to_writer(&mut file, value).map_err(io::Error::other)?;
    file.flush()?;
    fs::rename(staging, path)
}

/// Run one private job from the sweep binary's hidden worker entry point.
///
/// # Errors
/// The job or its result cannot be read or written.
pub fn run_job(path: &Path) -> io::Result<()> {
    let job: Job = read_json(path)?;
    let report = match &job.work {
        Work::Target(target) => {
            super::sweep_target(&job.layout, &job.policy, &System, job.dry_run, target)
        }
        Work::CacheGc => {
            let mut report = empty_report(&job.layout, &job.policy, false, None);
            if let Some(outcome) = System.cache_gc() {
                report.complete = outcome.is_ok();
                report.actions.push(Action {
                    verb: "cache gc".into(),
                    path: "kache".into(),
                    bytes: 0,
                    reason: outcome.map_or_else(
                        |error| error,
                        |()| "kache gc completed below the disk floor".into(),
                    ),
                    at: report.at.clone(),
                });
            }
            report
        }
    };
    write_json(&job.result, &report)
}

struct OwnedChild {
    child: Child,
    reaped: bool,
}

impl OwnedChild {
    fn spawn(command: &mut Command) -> io::Result<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt as _;
            command.process_group(0);
        }
        Ok(Self {
            child: command.spawn()?,
            reaped: false,
        })
    }

    fn stop(&mut self) {
        if self.reaped {
            return;
        }
        #[cfg(unix)]
        if let Ok(pid) = libc::pid_t::try_from(self.child.id()) {
            // SAFETY: spawn placed this owned, unreaped child in its own group.
            unsafe { libc::kill(-pid, libc::SIGKILL) };
        }
        let _ = self.child.kill();
        self.reaped = self.child.wait().is_ok();
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_owned(
    child: &mut OwnedChild,
    limit: Duration,
    mut elapsed: impl FnMut() -> Duration,
    mut pause: impl FnMut(Duration),
) -> io::Result<ExitStatus> {
    loop {
        match child.child.try_wait() {
            Ok(Some(status)) => {
                child.reaped = true;
                return Ok(status);
            }
            Ok(None) => {}
            Err(error) => {
                child.stop();
                return Err(error);
            }
        }
        let elapsed = elapsed();
        if elapsed >= limit {
            child.stop();
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "worker time limit reached",
            ));
        }
        pause(POLL.min(limit - elapsed));
    }
}

struct Outcome {
    report: Option<Report>,
    error: Option<String>,
}

fn execute(program: &Path, directory: &Path, job: Job, limit: Duration) -> Outcome {
    let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
    let path = directory.join(format!("job-{sequence}.json"));
    let stderr_path = directory.join(format!("job-{sequence}.stderr"));
    let outcome = (|| {
        write_json(&path, &job)?;
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let stderr = options.open(&stderr_path)?;
        let mut command = Command::new(program);
        command
            .arg("--worker-job")
            .arg(&path)
            .env("OPENAGENTS_BUILD_SWEEP_PROGRESS", &job.result)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::from(stderr));
        let mut child = OwnedChild::spawn(&mut command)?;
        let started = Instant::now();
        let status = wait_owned(&mut child, limit, || started.elapsed(), std::thread::sleep)?;
        if !status.success() {
            return Err(io::Error::other(format!("worker exited with {status}")));
        }
        Ok(())
    })();
    let report = read_json(&job.result).ok();
    let error = outcome
        .err()
        .map(|error| error.to_string())
        .or_else(|| {
            report
                .is_none()
                .then(|| "worker did not write a readable result".into())
        })
        .map(|error| {
            let mut stderr = Vec::new();
            let _ =
                File::open(stderr_path).and_then(|file| file.take(4096).read_to_end(&mut stderr));
            let detail = String::from_utf8_lossy(&stderr);
            let detail = detail.trim();
            if detail.is_empty() {
                error
            } else {
                format!("{error}: {detail}")
            }
        });
    Outcome { report, error }
}

fn empty_report(
    layout: &Layout,
    policy: &Policy,
    dry_run: bool,
    previous: Option<&Report>,
) -> Report {
    let at = System.stamp(System.now());
    let free = System.free(&layout.work).ok();
    Report {
        at,
        elapsed: Duration::ZERO,
        dry_run,
        free_before: free,
        free_after: free,
        freed: 0,
        watching_since: previous
            .map_or_else(|| "unknown".into(), |report| report.watching_since.clone()),
        policy: policy.clone(),
        actions: Vec::new(),
        targets: Vec::new(),
        complete: true,
    }
}

fn incomplete(
    report: &mut Report,
    path: &Path,
    kind: Kind,
    previous: Option<&Report>,
    reason: &str,
) {
    report.complete = false;
    if let Some(target) = report.targets.iter_mut().find(|target| target.path == path) {
        target.complete = false;
    } else {
        let bytes = previous
            .and_then(|old| old.targets.iter().find(|target| target.path == path))
            .map_or(0, |target| target.bytes);
        report.targets.push(TargetSize {
            path: path.into(),
            kind,
            bytes,
            holder: None,
            complete: false,
        });
    }
    report.actions.push(Action {
        verb: "skipped".into(),
        path: path.into(),
        bytes: 0,
        reason: format!(
            "incomplete result: {reason}; previous size is advisory; the next pass will try again"
        ),
        at: report.at.clone(),
    });
}

fn merge(report: &mut Report, mut current: Report) {
    report.complete &= current.complete;
    report.freed = report.freed.saturating_add(current.freed);
    if report.watching_since == "unknown" {
        report.watching_since = current.watching_since;
    }
    report.actions.append(&mut current.actions);
    report.targets.append(&mut current.targets);
}

fn collect_targets(
    report: &mut Report,
    targets: &[(PathBuf, Kind)],
    previous: Option<&Report>,
    mut remaining: impl FnMut() -> Duration,
    mut run: impl FnMut(&Path, Duration) -> Outcome,
) {
    for (index, (path, kind)) in targets.iter().enumerate() {
        let count = u32::try_from(targets.len() - index).unwrap_or(u32::MAX);
        let limit = TARGET_LIMIT.min(remaining() / count);
        if limit.is_zero() {
            incomplete(report, path, *kind, previous, "pass time limit reached");
            continue;
        }
        let outcome = run(path, limit);
        let accounted = outcome.report.as_ref().is_some_and(|current| {
            current.targets.iter().any(|target| target.path == *path)
                || current.actions.iter().any(|action| {
                    action.path == *path
                        && matches!(action.verb.as_str(), "removed" | "would remove")
                })
        });
        if let Some(current) = outcome.report {
            merge(report, current);
        }
        if let Some(error) = outcome.error {
            incomplete(report, path, *kind, previous, &error);
        } else if !accounted {
            incomplete(report, path, *kind, previous, "worker omitted this target");
        }
    }
}

/// Sweep each target in an owned process, with a deadline for each job and the pass.
///
/// # Errors
/// The binary path or private job folder cannot be opened.
pub fn run_isolated(layout: &Layout, policy: &Policy, dry_run: bool) -> io::Result<Report> {
    let started = Instant::now();
    let directory = JobDirectory::new(layout)?;
    let program = std::env::current_exe()?;
    let previous: Option<Report> = read_json(&layout.last).ok();
    let mut report = empty_report(layout, policy, dry_run, previous.as_ref());
    let remaining = || PASS_LIMIT.saturating_sub(started.elapsed());
    if !dry_run && report.free_before.is_some_and(|free| free < policy.floor) {
        let outcome = execute(
            &program,
            &directory.0,
            Job {
                layout: layout.clone(),
                policy: policy.clone(),
                dry_run,
                work: Work::CacheGc,
                result: directory.0.join("gc.json"),
            },
            GC_LIMIT.min(remaining()),
        );
        if let Some(current) = outcome.report {
            merge(&mut report, current);
        }
        if let Some(error) = outcome.error {
            report.complete = false;
            report.actions.push(Action {
                verb: "cache gc".into(),
                path: "kache".into(),
                bytes: 0,
                reason: format!("incomplete result: {error}"),
                at: report.at.clone(),
            });
        }
    }
    let processes = System.processes().ok();
    let mut targets = layout.targets();
    targets.sort_by_key(|(path, kind)| {
        (
            processes
                .as_ref()
                .is_none_or(|rows| rows.iter().any(|process| process.holds(path))),
            *kind,
            path.clone(),
        )
    });
    collect_targets(
        &mut report,
        &targets,
        previous.as_ref(),
        remaining,
        |target, limit| {
            execute(
                &program,
                &directory.0,
                Job {
                    layout: layout.clone(),
                    policy: policy.clone(),
                    dry_run,
                    work: Work::Target(target.into()),
                    result: directory.0.join(format!(
                        "target-{}.json",
                        NEXT.fetch_add(1, Ordering::Relaxed)
                    )),
                },
                limit,
            )
        },
    );
    report.free_after = if dry_run {
        report
            .free_before
            .map(|free| free.saturating_add(report.freed))
    } else {
        System.free(&layout.work).ok()
    };
    report.at = System.stamp(System.now());
    report.elapsed = started.elapsed();
    Ok(report)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::os::unix::process::ExitStatusExt as _;

    #[test]
    fn watchdog_kills_its_process_group_and_reaps_the_worker() {
        let mut command = Command::new("/bin/cat");
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        let mut child = OwnedChild::spawn(&mut command).unwrap();
        let pid = child.child.id() as libc::pid_t;
        // SAFETY: this PID belongs to the child retained above.
        assert_eq!(unsafe { libc::getpgid(pid) }, pid);
        let elapsed = Cell::new(Duration::ZERO);
        let error = wait_owned(
            &mut child,
            Duration::from_millis(100),
            || elapsed.get(),
            |pause| {
                assert_eq!(pause, POLL);
                elapsed.set(elapsed.get() + pause);
            },
        )
        .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(elapsed.get(), Duration::from_millis(100));
        let mut status = 0;
        // SAFETY: this is our child's PID and a valid status pointer.
        assert_eq!(
            unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) },
            -1
        );
        assert_eq!(
            io::Error::last_os_error().raw_os_error(),
            Some(libc::ECHILD)
        );
        assert_eq!(child.child.wait().unwrap().signal(), Some(libc::SIGKILL));
    }

    #[test]
    fn stalled_target_keeps_its_advisory_size_and_later_targets_still_run() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::at(dir.path(), &dir.path().join("leases"));
        let policy = Policy::default();
        let mut previous = empty_report(&layout, &policy, false, None);
        let stalled = dir.path().join("stalled");
        let ready = dir.path().join("ready");
        previous.freed = 999;
        previous.targets.push(TargetSize {
            path: stalled.clone(),
            kind: Kind::Shared,
            bytes: 123,
            holder: None,
            complete: true,
        });
        let mut report = empty_report(&layout, &policy, false, Some(&previous));
        let mut visited = Vec::new();
        collect_targets(
            &mut report,
            &[
                (stalled.clone(), Kind::Shared),
                (ready.clone(), Kind::Shared),
            ],
            Some(&previous),
            || Duration::from_secs(40),
            |path, limit| {
                visited.push((path.to_owned(), limit));
                if path == stalled {
                    return Outcome {
                        report: None,
                        error: Some("worker time limit reached".into()),
                    };
                }
                let mut current = empty_report(&layout, &policy, false, None);
                current.freed = 10;
                current.targets.push(TargetSize {
                    path: ready.clone(),
                    kind: Kind::Shared,
                    bytes: 20,
                    holder: None,
                    complete: true,
                });
                Outcome {
                    report: Some(current),
                    error: None,
                }
            },
        );
        assert_eq!(
            visited,
            vec![
                (stalled.clone(), Duration::from_secs(20)),
                (ready.clone(), TARGET_LIMIT)
            ]
        );
        assert!(!report.complete);
        assert_eq!(report.freed, 10);
        assert_eq!(report.targets[0].bytes, 123);
        assert!(!report.targets[0].complete);
        assert!(report.targets[1].complete);
        assert!(report.actions[0].reason.contains("advisory"));
    }

    #[test]
    fn successful_whole_target_removal_does_not_restore_an_advisory_target() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::at(dir.path(), &dir.path().join("leases"));
        let policy = Policy::default();
        let path = dir.path().join("removed");
        for verb in ["removed", "would remove"] {
            let mut report = empty_report(&layout, &policy, verb == "would remove", None);
            collect_targets(
                &mut report,
                &[(path.clone(), Kind::Copy)],
                None,
                || TARGET_LIMIT,
                |_, _| {
                    let mut current = empty_report(&layout, &policy, verb == "would remove", None);
                    current.freed = 100;
                    current.actions.push(Action {
                        verb: verb.into(),
                        path: path.clone(),
                        bytes: 100,
                        reason: "unused target".into(),
                        at: current.at.clone(),
                    });
                    Outcome {
                        report: Some(current),
                        error: None,
                    }
                },
            );
            assert!(report.complete);
            assert!(report.targets.is_empty());
            assert_eq!(report.freed, 100);
            assert_eq!(report.actions.len(), 1);
            assert_eq!(report.actions[0].verb, verb);
        }
    }

    #[test]
    fn timed_out_worker_keeps_its_current_progress_without_claiming_completion() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::at(dir.path(), &dir.path().join("leases"));
        let policy = Policy::default();
        let path = dir.path().join("partial");
        let mut report = empty_report(&layout, &policy, false, None);
        collect_targets(
            &mut report,
            &[(path.clone(), Kind::Shared)],
            None,
            || TARGET_LIMIT,
            |_, _| {
                let mut current = empty_report(&layout, &policy, false, None);
                current.freed = 12;
                current.targets.push(TargetSize {
                    path: path.clone(),
                    kind: Kind::Shared,
                    bytes: 50,
                    holder: None,
                    complete: true,
                });
                Outcome {
                    report: Some(current),
                    error: Some("worker time limit reached".into()),
                }
            },
        );
        assert!(!report.complete);
        assert_eq!(report.freed, 12);
        assert_eq!(report.targets[0].bytes, 50);
        assert!(!report.targets[0].complete);
    }

    #[test]
    fn an_exhausted_pass_reports_targets_without_launching_more_workers() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::at(dir.path(), &dir.path().join("leases"));
        let policy = Policy::default();
        let path = dir.path().join("unvisited");
        let mut report = empty_report(&layout, &policy, false, None);
        collect_targets(
            &mut report,
            &[(path.clone(), Kind::Shared)],
            None,
            || Duration::ZERO,
            |_, _| panic!("an exhausted pass cannot launch a worker"),
        );
        assert!(!report.complete);
        assert_eq!(report.targets[0].path, path);
        assert!(!report.targets[0].complete);
        assert!(report.actions[0].reason.contains("pass time limit"));
    }
}

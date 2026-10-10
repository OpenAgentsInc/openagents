//! `openagents mac serve`: this Mac runs the jobs its account sends it
//! (#11223).
//!
//! It reports what it can do ([`mac_jobs::detect`]) and takes the jobs the
//! website holds for it ([`coder_sync::mac_jobs`]), one at a time. Each job
//! gets its own folder under the jobs root: a git worktree at the ref's
//! commit, its own build folder (`CARGO_TARGET_DIR`, derived data), and an
//! output folder. The recipe's steps run there with no shell
//! ([`mac_jobs::plan`]); their output goes up as log lines, credential
//! shapes redacted, and the whole log and what the recipe made go up as the
//! job's files. The folder, the worktree, and a release gate's fresh
//! simulator are removed when the job ends, whatever happened.
//!
//! A recipe that sends something outside (a TestFlight upload) waits for
//! the owner first: the job asks, the owner answers on the phone or on the
//! web, and this Mac records the answer in its own approvals file
//! ([`coder_new::risk_policy`], ability `mac.upload`) and uses it once for
//! exactly that commit and recipe. Signing identities and the App Store
//! Connect key never leave the Mac; the steps find them here.

use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use coder_new::risk_policy::{self, Decision, Policy, Rule};
use coder_sync::Answer;
use coder_sync::mac_jobs::{self as wire, Ask, End, Heard, Report, Taken};
use mac_jobs::{Capabilities, Places, Recipe, Spec, Step};
use openagents_login::Saved;
use serde_json::Value;

/// How often an idle Mac asks for jobs.
const TAKE_EVERY: Duration = Duration::from_secs(5);
/// How often, against a website without these routes, it asks again.
const QUIET_EVERY: Duration = Duration::from_secs(120);
/// How often new log lines go up.
const REPORT_EVERY: Duration = Duration::from_secs(2);
/// A job with nothing new still reports this often, to hear of a cancel.
const PING_EVERY: Duration = Duration::from_secs(8);
/// How often the capabilities are asked again while idle.
const DETECT_EVERY: Duration = Duration::from_secs(120);
/// How long a job waits for the owner's answer.
const ASK_LIMIT: Duration = Duration::from_secs(2 * 3600);
/// The largest file a job keeps.
const MAX_FILE: u64 = 512 * 1024 * 1024;
/// The most files a job keeps.
const MAX_FILES: usize = 60;

/// Where and how this Mac serves.
#[derive(Clone, Debug)]
pub(crate) struct Settings {
    pub computer: String,
    /// Jobs run under `root/jobs`; repositories without a checkout are
    /// cloned under `root/repos`.
    pub root: PathBuf,
    /// Repositories this Mac builds, each with the checkout its worktrees
    /// come from (`None`: cloned under the root).
    pub repos: Vec<(String, Option<PathBuf>)>,
    /// A job starts only with this much free space, in GB.
    pub min_free_gb: u64,
    /// Take one round of jobs, run them, and stop.
    pub once: bool,
    /// This Mac's approvals file ([`risk_policy::approvals_path`]).
    pub approvals: Option<PathBuf>,
}

/// What a job's channel to the website does: report, and upload parts.
pub(crate) trait Channel {
    fn report(&mut self, report: &Report<'_>) -> Result<Heard, Answer>;
    fn upload(&mut self, name: &str, part: u32, last: bool, bytes: Vec<u8>) -> Result<(), Answer>;
}

/// The website, for one job.
struct Web<'a> {
    runtime: &'a tokio::runtime::Runtime,
    http: &'a reqwest::Client,
    saved: &'a Saved,
    computer: &'a str,
    id: &'a str,
}

impl Channel for Web<'_> {
    fn report(&mut self, report: &Report<'_>) -> Result<Heard, Answer> {
        self.runtime.block_on(wire::report(
            self.http,
            self.saved,
            self.computer,
            self.id,
            report,
        ))
    }

    fn upload(&mut self, name: &str, part: u32, last: bool, bytes: Vec<u8>) -> Result<(), Answer> {
        self.runtime.block_on(wire::upload_part(
            self.http,
            self.saved,
            self.computer,
            self.id,
            name,
            part,
            last,
            bytes,
        ))
    }
}

/// Serve jobs until signed out (or one round, with `once`).
pub(crate) fn serve(saved: &Saved, settings: &Settings) -> Result<(), String> {
    let http = wire::client().ok_or("Couldn't start the web client.")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| format!("Couldn't start: {error}"))?;
    std::fs::create_dir_all(settings.root.join("jobs"))
        .map_err(|error| format!("{}: {error}", settings.root.display()))?;
    eprintln!(
        "Serving Mac jobs as {} for {} ({}).",
        settings.computer, saved.label, saved.origin
    );
    let mut capabilities = mac_jobs::detect(&settings.root);
    let mut detected = Instant::now();
    loop {
        if detected.elapsed() >= DETECT_EVERY {
            capabilities = mac_jobs::detect(&settings.root);
            detected = Instant::now();
        }
        let wait =
            match runtime.block_on(wire::take(&http, saved, &settings.computer, &capabilities)) {
                Ok(jobs) => {
                    for job in jobs {
                        eprintln!("Running {} ({}).", job.spec.title(), job.id);
                        capabilities.busy = true;
                        // Say it's busy while it runs.
                        let _ = runtime.block_on(wire::take(
                            &http,
                            saved,
                            &settings.computer,
                            &capabilities,
                        ));
                        let mut web = Web {
                            runtime: &runtime,
                            http: &http,
                            saved,
                            computer: &settings.computer,
                            id: &job.id,
                        };
                        let end = run_job(&job, settings, &saved.label, &mut web);
                        match &end {
                            End::Done { summary } => eprintln!("Done: {summary}"),
                            End::Failed { why } => eprintln!("Stopped: {why}"),
                        }
                        capabilities = mac_jobs::detect(&settings.root);
                        detected = Instant::now();
                    }
                    if settings.once {
                        return Ok(());
                    }
                    TAKE_EVERY
                }
                Err(Answer::SignedOut) => {
                    return Err(
                        "This Mac's sign-in stopped working. Sign in again with coder login."
                            .into(),
                    );
                }
                Err(Answer::Unknown) => QUIET_EVERY,
                Err(_) => TAKE_EVERY * 2,
            };
        if settings.once {
            return Ok(());
        }
        std::thread::sleep(wait);
    }
}

/// A job's log: the lines waiting to go up, the whole log on disk, and the
/// website's last word.
struct Log<'c, C: Channel> {
    channel: &'c mut C,
    file: Option<File>,
    pending: Vec<String>,
    sent_at: Instant,
    commit: Option<String>,
    commit_sent: bool,
    cancelled: bool,
}

impl<'c, C: Channel> Log<'c, C> {
    fn new(channel: &'c mut C, file: Option<File>) -> Self {
        Self {
            channel,
            file,
            pending: Vec::new(),
            sent_at: Instant::now(),
            commit: None,
            commit_sent: false,
            cancelled: false,
        }
    }

    fn say(&mut self, text: &str) {
        for line in text.lines() {
            let line = mac_jobs::redact_line(line);
            if let Some(file) = &mut self.file {
                let _ = writeln!(file, "{line}");
            }
            if !line.trim().is_empty() {
                self.pending.push(line);
            }
        }
    }

    /// Send what waits (when it is time, or `now`); returns the website's
    /// answer when it was asked.
    fn flush(&mut self, now: bool, ask: Option<&Ask>) -> Option<Heard> {
        let due = now
            || (!self.pending.is_empty() && self.sent_at.elapsed() >= REPORT_EVERY)
            || self.sent_at.elapsed() >= PING_EVERY;
        if !due {
            return None;
        }
        let lines: Vec<String> = std::mem::take(&mut self.pending);
        let commit = if self.commit_sent {
            None
        } else {
            self.commit.as_deref()
        };
        self.sent_at = Instant::now();
        match self.channel.report(&Report {
            lines: &lines,
            commit,
            ask,
            end: None,
        }) {
            Ok(heard) => {
                self.commit_sent |= commit.is_some();
                if heard.cancel {
                    self.cancelled = true;
                }
                Some(heard)
            }
            Err(Answer::Unknown | Answer::SignedOut | Answer::Deleted) => {
                self.cancelled = true;
                None
            }
            Err(_) => {
                // Not reached: keep the lines for the next report.
                let newer = std::mem::take(&mut self.pending);
                self.pending = lines;
                self.pending.extend(newer);
                None
            }
        }
    }

    /// The last report, tried a few times.
    fn finish(&mut self, end: &End) {
        let lines: Vec<String> = std::mem::take(&mut self.pending);
        for attempt in 0..5u32 {
            let sent = self.channel.report(&Report {
                lines: &lines,
                commit: if self.commit_sent {
                    None
                } else {
                    self.commit.as_deref()
                },
                ask: None,
                end: Some(end),
            });
            match sent {
                Ok(_) | Err(Answer::Unknown | Answer::SignedOut | Answer::Refused(_)) => return,
                Err(_) => std::thread::sleep(Duration::from_secs(2u64.pow(attempt))),
            }
        }
    }
}

/// Run one job to its end, cleaning up after it whatever happened.
pub(crate) fn run_job<C: Channel>(
    job: &Taken,
    settings: &Settings,
    by: &str,
    channel: &mut C,
) -> End {
    let folder = settings.root.join("jobs").join(&job.id);
    let places = Places {
        checkout: folder.join("src"),
        target: folder.join("target"),
        out: folder.join("out"),
        simulator: None,
    };
    let _ = std::fs::remove_dir_all(&folder);
    let made =
        std::fs::create_dir_all(&places.out).and_then(|()| std::fs::create_dir_all(&places.target));
    let log_file = made
        .ok()
        .and_then(|()| File::create(places.out.join("log.txt")).ok());
    let mut log = Log::new(channel, log_file);
    let mut cleanup = Cleanup {
        base: None,
        fetched: None,
        worktree: places.checkout.clone(),
        simulator: None,
        folder: folder.clone(),
    };
    let ended = drive(job, settings, by, &mut log, &mut cleanup, places);
    let end = match ended {
        Ok(end) => end,
        Err(why) => End::Failed { why },
    };
    let end = if log.cancelled {
        End::Failed {
            why: "The job was cancelled.".into(),
        }
    } else {
        end
    };
    match &end {
        End::Done { summary } => log.say(&format!("Done. {summary}")),
        End::Failed { why } => log.say(&format!("Stopped: {why}")),
    }
    // The files go up before the end is reported, the log last.
    let out = folder.join("out");
    let plan_collect = mac_jobs::plan(
        &job.spec,
        &Places {
            checkout: folder.join("src"),
            target: folder.join("target"),
            out: out.clone(),
            simulator: None,
        },
    )
    .collect;
    let _ = log.flush(true, None);
    if !log.cancelled {
        let files = collect(&out, &plan_collect);
        upload_all(&mut log, &files);
    }
    log.file = None;
    upload_one(&mut *log.channel, "log.txt", &out.join("log.txt"));
    log.finish(&end);
    cleanup.run();
    end
}

/// What a job leaves on the Mac, removed at its end.
struct Cleanup {
    base: Option<PathBuf>,
    /// The ref the job's commit was fetched to, in `base`.
    fetched: Option<String>,
    worktree: PathBuf,
    simulator: Option<String>,
    folder: PathBuf,
}

impl Cleanup {
    fn run(&mut self) {
        if let Some(udid) = self.simulator.take() {
            let _ = quiet("xcrun", &["simctl", "shutdown", &udid]);
            let _ = quiet("xcrun", &["simctl", "delete", &udid]);
        }
        if let Some(base) = self.base.take() {
            let worktree = self.worktree.to_string_lossy().into_owned();
            let _ = git(&base, &["worktree", "remove", "--force", &worktree]);
            let _ = git(&base, &["worktree", "prune"]);
            if let Some(fetched) = self.fetched.take() {
                let _ = git(&base, &["update-ref", "-d", &fetched]);
            }
        }
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

fn drive<C: Channel>(
    job: &Taken,
    settings: &Settings,
    by: &str,
    log: &mut Log<'_, C>,
    cleanup: &mut Cleanup,
    mut places: Places,
) -> Result<End, String> {
    let spec = &job.spec;
    spec.check()?;
    // Room first: a build here needs space, and a full disk hurts the Mac.
    let free = free_gb(&settings.root);
    if let Some(free) = free
        && free < settings.min_free_gb
    {
        return Err(format!(
            "This Mac has {free} GB free, and jobs start only with {} GB or more.",
            settings.min_free_gb
        ));
    }
    let Some((_, configured)) = settings.repos.iter().find(|(repo, _)| *repo == spec.repo) else {
        return Err(format!("This Mac doesn't build {}.", spec.repo));
    };
    log.say(&format!("Checking out {} at {}.", spec.repo, spec.git_ref));
    let _ = log.flush(true, None);
    let base = match configured {
        Some(path) => path.clone(),
        None => clone_once(&settings.root, &spec.repo)?,
    };
    let fetched = format!("refs/openagents/mac-jobs/{}", job.id);
    cleanup.base = Some(base.clone());
    cleanup.fetched = Some(fetched.clone());
    let commit = checkout(&base, &spec.git_ref, &fetched, &places.checkout)?;
    log.commit = Some(commit.clone());
    log.say(&format!("At commit {commit}."));
    let _ = log.flush(true, None);
    if spec.outward() {
        let approvals = settings
            .approvals
            .as_deref()
            .ok_or("This Mac has no home folder for approvals.")?;
        wait_for_owner(spec, &commit, &places.checkout, by, approvals, log)?;
    }
    if spec.recipe == Recipe::IosReleaseGate {
        let udid = fresh_simulator(spec.simulator(), &job.id, log)?;
        cleanup.simulator = Some(udid.clone());
        places.simulator = Some(udid);
    }
    let plan = mac_jobs::plan(spec, &places);
    let mut failed = None;
    for step in &plan.steps {
        if log.cancelled {
            break;
        }
        log.say(&format!("Step: {}", step.label));
        let _ = log.flush(true, None);
        match run_step(step, &places.checkout, log) {
            Ok(()) => {}
            Err(why) if step.optional => log.say(&format!("Skipped: {why}")),
            Err(why) => {
                failed = Some(why);
                break;
            }
        }
    }
    if log.cancelled {
        return Err("The job was cancelled.".into());
    }
    let summary = mac_jobs::summary_line(&places.out, failed.is_none());
    match failed {
        None => Ok(End::Done { summary }),
        Some(why) if summary.starts_with("Failed") => Err(format!("{summary} {why}")),
        Some(why) => Err(why),
    }
}

/// The question for an outward job, and exactly what an answer approves.
pub(crate) fn question(spec: &Spec, commit: &str) -> Ask {
    let short: String = commit.chars().take(12).collect();
    let args = if spec.args.is_empty() {
        String::new()
    } else {
        format!(" {}", spec.args.join(" "))
    };
    let what = if spec.args.iter().any(|a| a == "--validate-only") {
        "Send a signed build of the iOS app to App Store Connect to validate it (nothing is \
         released)"
    } else {
        "Upload a signed build of the iOS app to TestFlight"
    };
    Ask {
        id: format!("upload-{short}"),
        text: format!(
            "{what}, from {} at {short}?\nThe Mac signs it with its own keys; they never leave it.",
            spec.repo
        ),
        subject: format!("{}@{commit} {}{args}", spec.repo, spec.recipe.name()),
    }
}

/// Ask the owner, wait for the answer, and record it in this Mac's
/// approvals file; an approval is used once, for exactly this subject.
fn wait_for_owner<C: Channel>(
    spec: &Spec,
    commit: &str,
    checkout: &Path,
    by: &str,
    path: &Path,
    log: &mut Log<'_, C>,
) -> Result<(), String> {
    let policy = Policy::load(Some(checkout))?;
    match policy.rule(risk_policy::MAC_UPLOAD) {
        Rule::Allow => return Ok(()),
        Rule::Deny => {
            return Err("The approval policy never lets this Mac upload a build.".into());
        }
        Rule::Ask => {}
    }
    let ask = question(spec, commit);
    log.say("Waiting for the owner's approval, on the phone or on the web.");
    let started = Instant::now();
    loop {
        if let Some(heard) = log.flush(true, Some(&ask))
            && let Some(approval) = heard.approval
            && approval.question == ask.id
        {
            let decision = if approval.decision == "approved" {
                Decision::Approved
            } else {
                Decision::Denied
            };
            let via = if approval.via == "phone" {
                "phone"
            } else {
                "web"
            };
            let record = risk_policy::record(
                path,
                risk_policy::MAC_UPLOAD,
                &ask.subject,
                decision,
                by,
                via,
            )?;
            if decision == Decision::Denied {
                return Err("You denied the upload, so nothing was sent.".into());
            }
            risk_policy::consume(path, &record.id, risk_policy::MAC_UPLOAD, &ask.subject)?;
            log.say(&format!("Approved on the {via}."));
            return Ok(());
        }
        if log.cancelled {
            return Err("The job was cancelled.".into());
        }
        if started.elapsed() >= ASK_LIMIT {
            return Err("Nobody answered within two hours, so nothing was sent.".into());
        }
        std::thread::sleep(Duration::from_secs(3));
    }
}

/// Run one step, its output into the log. `Err` says why it failed.
fn run_step<C: Channel>(step: &Step, cwd: &Path, log: &mut Log<'_, C>) -> Result<(), String> {
    let mut command = Command::new(&step.program);
    command
        .args(&step.args)
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .env("PATH", tool_path(std::env::var("PATH").ok().as_deref()))
        .env_remove("OPENAGENTS_APP_TOKEN");
    for (name, value) in &step.env {
        command.env(name, value);
    }
    let to_file = match &step.stdout_to {
        Some(path) => {
            Some(File::create(path).map_err(|error| format!("{}: {error}", path.display()))?)
        }
        None => None,
    };
    match to_file {
        Some(file) => command.stdout(file),
        None => command.stdout(Stdio::piped()),
    };
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("{} couldn't start: {error}", step.program))?;
    let (send, lines) = mpsc::channel::<String>();
    let mut readers = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        readers.push(read_lines(stdout, send.clone()));
    }
    if let Some(stderr) = child.stderr.take() {
        readers.push(read_lines(stderr, send.clone()));
    }
    drop(send);
    let status = loop {
        for line in lines.try_iter() {
            log.say(&line);
        }
        let _ = log.flush(false, None);
        if log.cancelled {
            stop(&mut child);
            break None;
        }
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(error) => return Err(format!("{}: {error}", step.label)),
        }
    };
    for reader in readers {
        let _ = reader.join();
    }
    for line in lines.try_iter() {
        log.say(&line);
    }
    match status {
        None => Err("The job was cancelled.".into()),
        Some(status) if status.success() => Ok(()),
        Some(status) => Err(match status.code() {
            Some(code) => format!("{} failed (exit {code}).", step.label),
            None => format!("{} stopped.", step.label),
        }),
    }
}

fn read_lines(
    stream: impl std::io::Read + Send + 'static,
    send: mpsc::Sender<String>,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            match reader.read_until(b'\n', &mut buffer) {
                Ok(0) | Err(_) => return,
                Ok(_) => {
                    let text = String::from_utf8_lossy(&buffer);
                    if send.send(text.trim_end().to_owned()).is_err() {
                        return;
                    }
                }
            }
        }
    })
}

/// Stop a step and everything it started.
fn stop(child: &mut Child) {
    #[cfg(unix)]
    {
        if let Ok(pid) = i32::try_from(child.id()) {
            // SAFETY: signals the process group this step leads; no memory
            // is shared.
            unsafe {
                libc::kill(-pid, libc::SIGTERM);
            }
            let until = Instant::now() + Duration::from_secs(10);
            while Instant::now() < until {
                if matches!(child.try_wait(), Ok(Some(_))) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            // SAFETY: as above.
            unsafe {
                libc::kill(-pid, libc::SIGKILL);
            }
        }
    }
    let _ = child.kill();
    let _ = child.wait();
}

/// `PATH` with the folders a build needs (a service started by launchd has
/// a short one): cargo's, Homebrew's, and `/usr/local/bin`.
pub(crate) fn tool_path(current: Option<&str>) -> String {
    let mut parts: Vec<String> = current
        .unwrap_or("/usr/bin:/bin:/usr/sbin:/sbin")
        .split(':')
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect();
    let home = std::env::var("HOME").unwrap_or_default();
    for extra in [
        format!("{home}/.cargo/bin"),
        "/opt/homebrew/bin".to_owned(),
        "/usr/local/bin".to_owned(),
    ] {
        if !parts.contains(&extra) {
            parts.push(extra);
        }
    }
    parts.join(":")
}

fn quiet(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

/// `git -C dir ARGS`, its output, or why it failed.
fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("git couldn't start: {error}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
    } else {
        let said = String::from_utf8_lossy(&output.stderr);
        let said = mac_jobs::redact_line(said.lines().last().unwrap_or("git failed"));
        Err(format!(
            "git {}: {said}",
            args.first().copied().unwrap_or("")
        ))
    }
}

/// The clone under the root that worktrees for `repo` come from, made once.
fn clone_once(root: &Path, repo: &str) -> Result<PathBuf, String> {
    let base = root.join("repos").join(repo.replace('/', "__"));
    if base.join(".git").exists() || base.join("HEAD").exists() {
        return Ok(base);
    }
    std::fs::create_dir_all(root.join("repos"))
        .map_err(|error| format!("{}: {error}", root.display()))?;
    let url = format!("https://github.com/{repo}.git");
    let status = Command::new("git")
        .args([
            "clone",
            "--quiet",
            "--filter=blob:none",
            "--no-checkout",
            &url,
        ])
        .arg(&base)
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .status()
        .map_err(|error| format!("git couldn't start: {error}"))?;
    if !status.success() {
        let _ = std::fs::remove_dir_all(&base);
        return Err(format!("Couldn't clone {repo}."));
    }
    Ok(base)
}

/// Fetch `git_ref` from `base`'s origin to the job's own ref `into` (never
/// `FETCH_HEAD`, which another fetch in the same checkout may move) and
/// add a worktree at its commit at `at`; the commit.
pub(crate) fn checkout(
    base: &Path,
    git_ref: &str,
    into: &str,
    at: &Path,
) -> Result<String, String> {
    // The ref was checked (`Spec::check`): no option can hide in it.
    let refspec = format!("+{git_ref}:{into}");
    git(base, &["fetch", "--quiet", "--no-tags", "origin", &refspec]).map_err(|why| {
        if git_ref.len() < 40 && git_ref.bytes().all(|b| b.is_ascii_hexdigit()) {
            "Name a branch, a tag, or the full 40-character commit: git can't fetch a short \
             commit."
                .to_owned()
        } else {
            why
        }
    })?;
    let commit = git(
        base,
        &["rev-parse", "--verify", &format!("{into}^{{commit}}")],
    )?;
    if !(7..=64).contains(&commit.len()) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("git named no commit for that ref.".into());
    }
    let place = at.to_string_lossy().into_owned();
    git(
        base,
        &["worktree", "add", "--detach", "--force", &place, &commit],
    )?;
    Ok(commit)
}

/// Free space where jobs run, in GB.
fn free_gb(root: &Path) -> Option<u64> {
    let probe = root
        .ancestors()
        .find(|dir| dir.exists())
        .map(Path::to_path_buf)?;
    let text = quiet("df", &["-Pk", &probe.to_string_lossy()])?;
    mac_jobs::parse_df_available_gb(&text)
}

/// The device type and runtime to make a simulator from: the newest iOS
/// runtime, and the iPhone named `wanted` (default an iPhone Pro) it runs.
pub(crate) fn pick_device(runtimes_json: &str, wanted: Option<&str>) -> Option<(String, String)> {
    let value: Value = serde_json::from_str(runtimes_json).ok()?;
    let runtime = value["runtimes"]
        .as_array()?
        .iter()
        .filter(|runtime| {
            runtime["isAvailable"] != Value::Bool(false)
                && runtime["identifier"]
                    .as_str()
                    .is_some_and(|id| id.contains(".iOS-"))
        })
        .max_by(|a, b| {
            let version = |r: &Value| -> Vec<u64> {
                r["version"]
                    .as_str()
                    .unwrap_or_default()
                    .split('.')
                    .filter_map(|part| part.parse().ok())
                    .collect()
            };
            version(a).cmp(&version(b))
        })?;
    let types = runtime["supportedDeviceTypes"].as_array()?;
    let named = |name: &str| {
        types
            .iter()
            .find(|t| t["name"].as_str() == Some(name))
            .and_then(|t| t["identifier"].as_str())
    };
    let device = match wanted {
        Some(name) => named(name)?,
        None => named("iPhone 17 Pro").or_else(|| {
            types
                .iter()
                .filter(|t| t["name"].as_str().is_some_and(|n| n.starts_with("iPhone")))
                .filter_map(|t| t["identifier"].as_str())
                .next_back()
        })?,
    };
    Some((
        device.to_owned(),
        runtime["identifier"].as_str()?.to_owned(),
    ))
}

/// A new simulator for this job, booted; its UDID.
fn fresh_simulator<C: Channel>(
    wanted: Option<&str>,
    id: &str,
    log: &mut Log<'_, C>,
) -> Result<String, String> {
    let runtimes = quiet("xcrun", &["simctl", "list", "runtimes", "-j"])
        .ok_or("Couldn't list the iOS simulators.")?;
    let (device, runtime) = pick_device(&runtimes, wanted).ok_or_else(|| match wanted {
        Some(name) => format!("This Mac has no {name} simulator."),
        None => "This Mac has no iPhone simulator.".into(),
    })?;
    let name = format!("OpenAgents gate {}", &id[id.len().saturating_sub(8)..]);
    let udid = quiet("xcrun", &["simctl", "create", &name, &device, &runtime])
        .filter(|udid| !udid.is_empty())
        .ok_or("Couldn't make a simulator.")?;
    log.say(&format!("Made a fresh simulator ({name})."));
    if quiet("xcrun", &["simctl", "boot", &udid]).is_none()
        || quiet("xcrun", &["simctl", "bootstatus", &udid, "-b"]).is_none()
    {
        let _ = quiet("xcrun", &["simctl", "delete", &udid]);
        return Err("The simulator didn't start.".into());
    }
    Ok(udid)
}

/// The files a job keeps: everything in `out` (not result bundles, not
/// the log, which goes last), then the recipe's own, by name.
pub(crate) fn collect(out: &Path, extra: &[mac_jobs::Collect]) -> Vec<(String, PathBuf)> {
    let mut found: Vec<(String, PathBuf)> = Vec::new();
    let mut walk = vec![out.to_path_buf()];
    while let Some(dir) = walk.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                if path.extension().is_none_or(|ext| ext != "xcresult") {
                    walk.push(path);
                }
                continue;
            }
            if !kind.is_file() || path == out.join("log.txt") {
                continue;
            }
            let Ok(relative) = path.strip_prefix(out) else {
                continue;
            };
            if let Some(name) = mac_jobs::artifact_name(relative) {
                found.push((name, path));
            }
        }
    }
    for collect in extra {
        let Ok(entries) = std::fs::read_dir(&collect.dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let fits = path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| collect.extensions.contains(&ext));
            if fits
                && path.is_file()
                && let Some(name) = path
                    .file_name()
                    .and_then(|n| mac_jobs::artifact_name(Path::new(n)))
            {
                found.push((name, path));
            }
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    found.retain(|(name, _)| seen.insert(name.clone()));
    found.truncate(MAX_FILES);
    found
}

fn upload_all<C: Channel>(log: &mut Log<'_, C>, files: &[(String, PathBuf)]) {
    for (name, path) in files {
        let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
        if size > MAX_FILE {
            log.say(&format!("Kept {name} on the Mac: it is over 512 MB."));
            continue;
        }
        if upload_one(&mut *log.channel, name, path) {
            log.say(&format!("Sent {name}."));
        } else {
            log.say(&format!("Couldn't send {name}."));
        }
        let _ = log.flush(false, None);
    }
}

/// Send one file in parts; whether it all arrived.
fn upload_one<C: Channel + ?Sized>(channel: &mut C, name: &str, path: &Path) -> bool {
    let Ok(bytes) = std::fs::read(path) else {
        return false;
    };
    let parts: Vec<&[u8]> = if bytes.is_empty() {
        vec![&[][..]]
    } else {
        bytes.chunks(wire::PART_BYTES).collect()
    };
    let count = parts.len();
    for (index, part) in parts.into_iter().enumerate() {
        let Ok(number) = u32::try_from(index) else {
            return false;
        };
        let last = index + 1 == count;
        let mut sent = false;
        for attempt in 0..4u32 {
            match channel.upload(name, number, last, part.to_vec()) {
                Ok(()) => {
                    sent = true;
                    break;
                }
                Err(Answer::Retry) => std::thread::sleep(Duration::from_secs(2u64.pow(attempt))),
                Err(_) => return false,
            }
        }
        if !sent {
            return false;
        }
    }
    true
}

/// The repositories this Mac builds: each `--repo OWNER/NAME[=PATH]`, or
/// by default this repository, from the checkout `here` is in when it is
/// one.
pub(crate) fn repos(
    given: &[&str],
    here: Option<&Path>,
) -> Result<Vec<(String, Option<PathBuf>)>, String> {
    if given.is_empty() {
        let checkout = here.and_then(|here| git(here, &["rev-parse", "--show-toplevel"]).ok());
        let ours = checkout.filter(|top| {
            git(Path::new(top), &["remote", "get-url", "origin"])
                .is_ok_and(|url| url.contains("OpenAgentsInc/openagents"))
        });
        return Ok(vec![(
            "OpenAgentsInc/openagents".to_owned(),
            ours.map(PathBuf::from),
        )]);
    }
    given
        .iter()
        .map(|word| {
            let (repo, path) = match word.split_once('=') {
                Some((repo, path)) => (repo, Some(PathBuf::from(path))),
                None => (*word, None),
            };
            let probe = Spec {
                repo: repo.to_owned(),
                git_ref: "main".into(),
                recipe: Recipe::DesktopCapture,
                args: Vec::new(),
            };
            probe
                .check()
                .map(|()| (repo.to_owned(), path))
                .map_err(|why| format!("--repo {word}: {why}"))
        })
        .collect()
}

/// This Mac's capabilities, for `openagents mac capabilities`.
pub(crate) fn capabilities(root: &Path) -> Capabilities {
    mac_jobs::detect(root)
}

#[cfg(test)]
mod tests;

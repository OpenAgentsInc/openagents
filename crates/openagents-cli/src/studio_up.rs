//! `openagents studio up` and `down`: one command that launches the Agent
//! Studio on a repository (docs/verse/agentcraft-parity.md, "Launch"), the
//! way AgentCraft's launcher does, reimplemented over this host.
//!
//! `up --repo PATH` admits the repository as a host workspace
//! (`serve.json`, as `coder host init --workspace` records it), turns the
//! auto-start policy on for it with every seat's route admitted (as
//! `coder host autostart on --route` would), seats a team (a lead and
//! workers on the coding agents signed in here, or `--team`), starts
//! `coder host serve` when no host answers the control socket, and opens
//! Verse straight into Everglade. `up --sim` starts a scratch host whose
//! studio is the simulated team instead (#10572): its access store, root,
//! task store, keys, control socket, and scratch repository all live in
//! one new directory under the system's temporary directory, never under
//! the home directory, and no launch agent or keychain item is made. The
//! host runs `coder host serve --studio-sim`, whose scripted engine ends
//! the studio's turns with no model spend
//! (`coder::task::studio_sim`), and Verse opens on its control socket, so
//! Everglade and `openagents studio --control-socket SOCKET` both act on
//! it. `down` stops that host and removes its directory.
//!
//! `up` records what it changed and started in `ROOT/studio-up.json`, and
//! `down` undoes exactly that: it closes the Verse window it opened, stops
//! the host it started, removes the seats it added and restores the ones it
//! replaced, puts back the auto-start policy it found, and removes the
//! workspace it added. Anything `up` found already there stays.
//!
//! The options `up` was given last are remembered in `ROOT/studio.json`,
//! so a bare `openagents studio up` opens the same studio again.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use coder::task::autostart::{
    self, ClaudeRuns, CodexRuns, DEFAULT_DECISION_MODEL, Engine, MAX_RUNNING, POLICY_FILE,
    POLICY_SCHEMA, Policy, Route,
};
use coder::task::capacity::{self, Provider};
use coder::task::studio::{self, Role, Seat, Studio};
use coder::task::studio_sim;
use coder_host::settings::ServeSettings;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Args, Output};

/// The remembered options' file in the host root.
pub(crate) const DEFAULTS_FILE: &str = "studio.json";
const DEFAULTS_SCHEMA: &str = "openagents.studio-defaults.v1";
/// What `up` changed and started, in the host root.
pub(crate) const RECORD_FILE: &str = "studio-up.json";
const RECORD_SCHEMA: &str = "openagents.studio-up.v1";
/// The coding agents a default team sits on, in preference order.
const TEAM_PROVIDERS: [Provider; 2] = [Provider::Codex, Provider::Claude];
/// How long a started host may take to answer its control socket.
const HOST_WAIT: Duration = Duration::from_secs(30);
/// How long a stopped process may take to exit.
const STOP_WAIT: Duration = Duration::from_secs(10);
/// What Everglade and the terminal say when no seat's coding agent can
/// sign in.
pub(crate) const NO_ENGINE: &str = "No coding agent can sign in, so the studio's seats cannot work. Sign in to Codex or Claude Code, or run `openagents studio up --sim` for the simulated team.";

/// Where the studio's pieces live on this computer.
#[derive(Clone, Debug)]
pub(crate) struct Paths {
    /// The host root: `serve.json`, the auto-start policy, the studio's
    /// worktrees, and this command's own files.
    pub root: PathBuf,
    /// The Coder task store the studio and the host share.
    pub tasks: PathBuf,
    /// The host's access store.
    pub state: PathBuf,
    /// The file key source of a host this command starts.
    pub keys: PathBuf,
    /// The host's control socket.
    pub socket: PathBuf,
}

impl Paths {
    /// The paths for host root `root` and task store `tasks`. Under the
    /// default root the access store, keys, and socket are the ones the
    /// desktop app and `openagents connect` use; under any other root they
    /// sit beside it, so a scratch host never touches this computer's own.
    pub(crate) fn new(root: PathBuf, tasks: PathBuf, socket: Option<PathBuf>, own: bool) -> Self {
        let base = root
            .parent()
            .map_or_else(|| root.clone(), Path::to_path_buf);
        let socket = socket.unwrap_or_else(|| {
            own.then(openagents_connect::control::socket_path)
                .flatten()
                .unwrap_or_else(|| base.join("control.sock"))
        });
        Self {
            state: base.join("coder-access"),
            keys: base.join("connect"),
            root,
            tasks,
            socket,
        }
    }
}

/// One seat of a team: its name, its part, and its route.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Member {
    pub name: String,
    pub role: Role,
    pub route: Route,
}

/// The options `up` remembers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Defaults {
    pub schema: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub sim: bool,
}

impl Defaults {
    /// The remembered options in `root`, or none.
    pub(crate) fn load(root: &Path) -> Self {
        std::fs::read(root.join(DEFAULTS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Self>(&bytes).ok())
            .filter(|defaults| defaults.schema == DEFAULTS_SCHEMA)
            .unwrap_or_default()
    }

    fn save(&self, root: &Path) -> Result<(), String> {
        let mut defaults = self.clone();
        defaults.schema = DEFAULTS_SCHEMA.into();
        let bytes = serde_json::to_vec_pretty(&defaults).map_err(|e| e.to_string())?;
        write_private(&root.join(DEFAULTS_FILE), &bytes)
    }

    /// These options with each one given in `args` in place of the
    /// remembered one. A repository given without `--sim` turns the
    /// remembered simulation off.
    fn merged(mut self, args: &Args) -> Self {
        if let Some(repo) = args.option("repo") {
            self.repo = Some(repo.to_owned());
            self.sim = false;
        }
        if let Some(label) = args.option("workspace") {
            self.workspace = Some(label.to_owned());
        }
        if let Some(team) = args.option("team") {
            self.team = Some(team.to_owned());
        }
        if args.switch("sim") {
            self.sim = true;
        }
        self
    }
}

/// What `up` changed and started, so `down` undoes only that.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Record {
    pub schema: String,
    pub at: u64,
    /// The workspace label `up` opened the studio on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    /// The workspaces `up` added to the host's settings.
    #[serde(default)]
    pub workspaces_added: Vec<String>,
    /// Whether `up` wrote the auto-start policy.
    #[serde(default)]
    pub policy_written: bool,
    /// The policy `up` found, which `down` puts back; none means `down`
    /// removes the policy file.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_policy: Option<Policy>,
    /// Seats `up` added.
    #[serde(default)]
    pub seats_added: Vec<String>,
    /// Seats `up` replaced, as they were.
    #[serde(default)]
    pub seats_replaced: Vec<Seat>,
    /// The host `up` started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_pid: Option<u32>,
    /// The Verse window `up` opened.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub verse_pid: Option<u32>,
    /// The simulated team's scratch host directory `up --sim` made.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sim_dir: Option<PathBuf>,
    /// The simulated team's scratch host `up --sim` started.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sim_host_pid: Option<u32>,
}

impl Record {
    /// Whether `up` changed anything of the host's own files or seats.
    fn changed_host(&self) -> bool {
        self.policy_written
            || !self.workspaces_added.is_empty()
            || !self.seats_added.is_empty()
            || !self.seats_replaced.is_empty()
    }
}

impl Record {
    pub(crate) fn load(root: &Path) -> Option<Self> {
        let bytes = std::fs::read(root.join(RECORD_FILE)).ok()?;
        let record: Self = serde_json::from_slice(&bytes).ok()?;
        (record.schema == RECORD_SCHEMA).then_some(record)
    }

    fn save(&self, root: &Path) -> Result<(), String> {
        let mut record = self.clone();
        record.schema = RECORD_SCHEMA.into();
        let bytes = serde_json::to_vec_pretty(&record).map_err(|e| e.to_string())?;
        write_private(&root.join(RECORD_FILE), &bytes)
    }
}

/// What the setup steps did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct Setup {
    pub workspace: String,
    pub repo: PathBuf,
    pub workspace_added: bool,
    pub policy_written: bool,
    pub routes: Vec<String>,
    pub seats: Vec<Member>,
    /// Whether any seat's coding agent can sign in here.
    pub engines_ready: bool,
}

/// What the setup steps need.
pub(crate) struct Plan<'a> {
    pub repo: PathBuf,
    pub workspace: Option<String>,
    /// `--team`, or none for the default team.
    pub team: Option<String>,
    /// The engine's controller for a new policy; the installed
    /// `microcoder` when none.
    pub controller: Option<PathBuf>,
    /// Whether a provider's coding agent can sign in here.
    pub connected: &'a dyn Fn(Provider) -> bool,
    pub now: u64,
    /// `--full-access`: the policy gives runs the host user's reads and
    /// network, which `session` and `sdk` seats need.
    pub full_access: bool,
}

/// `openagents studio up`. Returns the exit code.
pub(crate) fn up(output: &Output, args: &Args, paths: &Paths) -> u8 {
    match up_inner(output, args, paths) {
        Ok(()) => 0,
        Err(message) => output.fail("studio up", &message),
    }
}

fn up_inner(output: &Output, args: &Args, paths: &Paths) -> Result<(), String> {
    let defaults = Defaults::load(&paths.root).merged(args);
    let open_verse = !args.switch("no-verse");
    let verse = |extra: Vec<String>| -> Result<Option<u32>, String> {
        if !open_verse {
            return Ok(None);
        }
        let program = find_program("verse", args.option("verse"))?;
        launch_verse(&program, &extra, &paths.root).map(Some)
    };
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder
        .create(&paths.root)
        .map_err(|e| format!("cannot create {}: {e}", paths.root.display()))?;
    let mut record = Record::load(&paths.root).unwrap_or_default();
    record.at = autostart::unix_now();
    if defaults.sim {
        defaults.save(&paths.root)?;
        return up_sim(output, args, paths, &mut record, &verse);
    }
    let repo = defaults
        .repo
        .clone()
        .ok_or("give --repo PATH, or --sim for the simulated team")?;
    let probe = |provider: Provider| capacity::probe(provider).is_connected();
    let setup = setup(
        paths,
        &Plan {
            repo: PathBuf::from(&repo),
            workspace: defaults.workspace.clone(),
            team: defaults.team.clone(),
            controller: args.option("controller").map(PathBuf::from),
            connected: &probe,
            now: record.at,
            full_access: args.switch("full-access"),
        },
        &mut record,
    )?;
    defaults.save(&paths.root)?;
    record.save(&paths.root)?;
    if !setup.engines_ready {
        eprintln!("openagents studio up: {NO_ENGINE}");
    }
    let host = ensure_host(args, paths, &mut record, setup.workspace_added);
    record.save(&paths.root)?;
    let host = host?;
    let mut extra: Vec<String> = Vec::new();
    if openagents_connect::control::socket_path().as_deref() != Some(paths.socket.as_path()) {
        extra.extend(["--studio-socket".into(), paths.socket.display().to_string()]);
    }
    if !setup.engines_ready {
        extra.extend(["--studio-notice".into(), NO_ENGINE.into()]);
    }
    stop_verse(&mut record);
    let opened = verse(extra);
    if let Ok(pid) = &opened {
        record.verse_pid = *pid;
    }
    record.save(&paths.root)?;
    let verse_pid = opened?;
    let value = json!({
        "setup": setup,
        "host": host,
        "host_pid": record.host_pid,
        "verse_pid": verse_pid,
        "notice": (!setup.engines_ready).then_some(NO_ENGINE),
    });
    output.emit(&value, |_| {
        let mut lines = vec![format!(
            "Workspace `{}` is {}{}.",
            setup.workspace,
            setup.repo.display(),
            if setup.workspace_added {
                ", newly admitted by the host"
            } else {
                ""
            }
        )];
        lines.push(format!(
            "Auto-start starts its tasks on {}.",
            setup.routes.join(", ")
        ));
        for member in &setup.seats {
            let role = match member.role {
                Role::Lead => "lead",
                Role::Worker => "worker",
            };
            lines.push(format!(
                "Seat {} is a {role} on {}.",
                member.name, member.route
            ));
        }
        lines.push(host.sentence());
        match verse_pid {
            Some(pid) => lines.push(format!("Opened Everglade (Verse pid {pid}).")),
            None => lines.push("Open Everglade with `verse --everglade`.".into()),
        }
        if !setup.engines_ready {
            lines.push(NO_ENGINE.into());
        }
        lines.push("Stop what this started with `openagents studio down`.".into());
        lines.join("\n")
    });
    Ok(())
}

/// `up --sim`: a new scratch host whose studio is the simulated team, its
/// host started with `--studio-sim`, and Verse on its control socket.
/// Whatever an earlier `up --sim` started is stopped and removed first.
fn up_sim(
    output: &Output,
    args: &Args,
    paths: &Paths,
    record: &mut Record,
    verse: &dyn Fn(Vec<String>) -> Result<Option<u32>, String>,
) -> Result<(), String> {
    stop_verse(record);
    for note in stop_sim(record) {
        eprintln!("openagents studio up: {note}");
    }
    let scratch = studio_sim::Scratch::create(&scratch_dir()?)
        .and_then(|scratch| scratch.seat_team().map(|()| scratch))
        .map_err(|error| format!("cannot make the simulated team's scratch host: {error}"))?;
    record.sim_dir = Some(scratch.dir.clone());
    record.save(&paths.root)?;
    let sim = sim_paths(&scratch);
    if !args.switch("no-host") {
        let program = find_program("coder", args.option("coder"))?;
        // The scratch host runs with a home of its own, so nothing it does
        // reads or writes this computer's.
        let home = scratch.dir.join("home");
        std::fs::create_dir_all(&home)
            .map_err(|error| format!("cannot create {}: {error}", home.display()))?;
        record.sim_host_pid = Some(start_host(
            &program,
            &sim,
            &sim_serve_args(&sim),
            Some(&home),
        )?);
        record.save(&paths.root)?;
    }
    let opened = verse(vec![
        "--studio-socket".into(),
        sim.socket.display().to_string(),
    ]);
    if let Ok(pid) = &opened {
        record.verse_pid = *pid;
    }
    record.save(&paths.root)?;
    let verse_pid = opened?;
    let socket = sim.socket.display().to_string();
    let submit = format!(
        "openagents studio goal submit '{}' --workspace {} --control-socket {socket}",
        studio_sim::GOAL,
        studio_sim::WORKSPACE
    );
    let value = json!({
        "sim": true,
        "dir": scratch.dir,
        "workspace": studio_sim::WORKSPACE,
        "repo": scratch.fixture.checkout,
        "root": sim.root,
        "tasks": sim.tasks,
        "socket": sim.socket,
        "host_pid": record.sim_host_pid,
        "verse_pid": verse_pid,
        "goal": studio_sim::GOAL,
    });
    output.emit(&value, |_| {
        let mut lines = vec![match record.sim_host_pid {
            Some(pid) => format!(
                "Started the simulated team's scratch host (pid {pid}) in {}; its scripted engine calls no model.",
                scratch.dir.display()
            ),
            None => format!(
                "Made the simulated team's scratch host in {}; start it with `coder host serve --studio-sim` (--no-host).",
                scratch.dir.display()
            ),
        }];
        lines.push(format!(
            "Workspace `{}` is the scratch repository {}.",
            studio_sim::WORKSPACE,
            scratch.fixture.checkout.display()
        ));
        lines.push(format!("Submit the script's goal: {submit}"));
        lines.push(format!(
            "Act on it with `openagents studio COMMAND --control-socket {socket} --tasks {} --root {}`.",
            sim.tasks.display(),
            sim.root.display()
        ));
        match verse_pid {
            Some(pid) => lines.push(format!("Opened Everglade (Verse pid {pid}).")),
            None => lines.push(format!(
                "Open Everglade with `verse --everglade --studio-socket {socket}`."
            )),
        }
        lines.push("Stop it and remove its directory with `openagents studio down`.".into());
        lines.join("\n")
    });
    Ok(())
}

/// A new directory for a simulated team's scratch host, under the system's
/// temporary directory. Its name stays short: the control socket's path
/// inside it must fit a Unix socket address.
fn scratch_dir() -> Result<PathBuf, String> {
    let base = std::env::temp_dir();
    for attempt in 0..100u32 {
        let dir = base.join(format!("oa-sim-{}-{attempt}", std::process::id()));
        if !dir.exists() {
            return Ok(dir);
        }
    }
    Err(format!(
        "no free scratch directory under {}",
        base.display()
    ))
}

/// The host paths of a simulated team's scratch host.
pub(crate) fn sim_paths(scratch: &studio_sim::Scratch) -> Paths {
    Paths {
        root: scratch.root.clone(),
        tasks: scratch.store.clone(),
        state: scratch.state.clone(),
        keys: scratch.keys.clone(),
        socket: scratch.socket.clone(),
    }
}

/// The `coder host serve` arguments for a simulated team's scratch host:
/// [`serve_args`] without iroh, which the studio does not need, and with
/// `--studio-sim`.
pub(crate) fn sim_serve_args(paths: &Paths) -> Vec<String> {
    let mut args: Vec<String> = serve_args(paths)
        .into_iter()
        .filter(|arg| arg != "--iroh")
        .collect();
    args.push("--studio-sim".into());
    args
}

/// Stop the simulated team's scratch host an earlier `up --sim` started
/// and remove its directory. Returns a sentence for each thing done.
fn stop_sim(record: &mut Record) -> Vec<String> {
    let mut notes = Vec::new();
    if let Some(pid) = record.sim_host_pid.take().filter(|pid| pid_alive(*pid)) {
        match stop(pid) {
            Ok(()) => notes.push(format!(
                "Stopped the simulated team's scratch host (pid {pid})."
            )),
            Err(error) => notes.push(format!(
                "The simulated team's scratch host did not stop: {error}."
            )),
        }
    }
    if let Some(dir) = record.sim_dir.take() {
        // Only a directory a scratch host marked is removed.
        if studio_sim::Scratch::open(&dir).is_ok() {
            match std::fs::remove_dir_all(&dir) {
                Ok(()) => notes.push(format!("Removed {}.", dir.display())),
                Err(error) => notes.push(format!("Could not remove {}: {error}.", dir.display())),
            }
        }
    }
    notes
}

/// The setup steps against the host at `paths`, recording each change in
/// `record`: admit the repository as a workspace, turn the auto-start
/// policy on for it with every seat's route, and seat the team. Starts no
/// process.
///
/// # Errors
/// The repository is not a Git checkout, its label names another
/// repository, the team is malformed, or a file cannot be written.
pub(crate) fn setup(paths: &Paths, plan: &Plan<'_>, record: &mut Record) -> Result<Setup, String> {
    let repo = plan
        .repo
        .canonicalize()
        .map_err(|_| format!("{} does not exist", plan.repo.display()))?;
    if !repo.join(".git").exists() {
        return Err(format!(
            "{} is not the top level of a Git checkout",
            repo.display()
        ));
    }
    let label = match &plan.workspace {
        Some(label) => label.clone(),
        None => label_for(&repo),
    };
    if label.is_empty() || label.len() > 128 || label.chars().any(char::is_control) {
        return Err("a workspace label is 1 to 128 characters with no control character".into());
    }
    let members = match &plan.team {
        Some(text) => parse_team(text)?,
        None => default_team(plan.connected),
    };
    let engines_ready = members
        .iter()
        .any(|member| (plan.connected)(member.route.provider));

    // The workspace, as `coder host init --workspace LABEL=PATH` records it.
    let mut settings = ServeSettings::load(&paths.root).map_err(|e| e.to_string())?;
    let workspace_added = match settings.workspaces.get(&label) {
        Some(path) if *path == repo => false,
        Some(path) => {
            return Err(format!(
                "the host's workspace `{label}` is {}; name this one with --workspace LABEL",
                path.display()
            ));
        }
        None => {
            settings.workspaces.insert(label.clone(), repo.clone());
            settings.save(&paths.root).map_err(|e| e.to_string())?;
            true
        }
    };
    record.workspace = Some(label.clone());
    if workspace_added && !record.workspaces_added.contains(&label) {
        record.workspaces_added.push(label.clone());
    }

    // The policy, as `coder host autostart on --workspace LABEL --route ...`
    // records it. Studio tasks run in worktrees of their own under the host
    // root, so the person's checkout needs no isolated worktree.
    let current = Policy::load(&paths.root)?;
    let wanted = policy_for(current.as_ref(), &label, &members, plan)?;
    let unchanged = current.as_ref().is_some_and(|current| {
        let mut same = wanted.clone();
        same.changed_at = current.changed_at;
        same == *current
    });
    if !unchanged {
        wanted.save(&paths.root)?;
        if !record.policy_written {
            record.policy_written = true;
            record.previous_policy = current;
        }
        autostart::note_policy(&paths.root, plan.now, true, &policy_detail(&wanted))?;
    }

    // The team.
    let mut team = Studio::open(&paths.tasks).map_err(|e| e.to_string())?;
    for member in &members {
        let existing = team.state().seat(&member.name).cloned();
        let seat = Seat {
            name: member.name.clone(),
            role: member.role,
            route: member.route.clone(),
            look: existing
                .as_ref()
                .map_or_else(|| "default".into(), |seat| seat.look.clone()),
            desk: existing
                .as_ref()
                .map_or_else(|| team.free_desk(), |seat| seat.desk),
        };
        if existing.as_ref() == Some(&seat) {
            continue;
        }
        team.set_seat(seat).map_err(|e| e.to_string())?;
        match existing {
            None => {
                if !record.seats_added.contains(&member.name) {
                    record.seats_added.push(member.name.clone());
                }
            }
            Some(seat) => {
                let ours = record.seats_added.contains(&seat.name)
                    || record
                        .seats_replaced
                        .iter()
                        .any(|old| old.name == seat.name);
                if !ours {
                    record.seats_replaced.push(seat);
                }
            }
        }
    }
    Ok(Setup {
        workspace: label,
        repo,
        workspace_added,
        policy_written: !unchanged,
        routes: wanted.routes().iter().map(ToString::to_string).collect(),
        seats: members,
        engines_ready,
    })
}

/// The policy that starts `members`' tasks in `label`: `current` with the
/// label and any missing route added, or a new one.
fn policy_for(
    current: Option<&Policy>,
    label: &str,
    members: &[Member],
    plan: &Plan<'_>,
) -> Result<Policy, String> {
    let seats = u32::try_from(members.len()).unwrap_or(MAX_RUNNING);
    let running = seats.clamp(1, MAX_RUNNING);
    let mut routes: Vec<Route> = current.map(Policy::routes).unwrap_or_default();
    for member in members {
        let admitted = routes.iter().any(|route| {
            route.provider == member.route.provider && route.model == member.route.model
        });
        if !admitted {
            routes.push(member.route.clone());
        }
    }
    let first = routes.first().ok_or("the team has no seat")?;
    let mut policy = match current {
        Some(current) => {
            let mut policy = current.clone();
            if !policy.workspaces.iter().any(|w| w == label) {
                policy.workspaces.push(label.to_owned());
            }
            policy.max_running = policy.max_running.max(running);
            policy
        }
        None => {
            let controller = match &plan.controller {
                Some(path) => path.clone(),
                None => autostart::default_controller()?,
            };
            let controller = controller
                .canonicalize()
                .map_err(|_| format!("the controller {} does not exist", controller.display()))?;
            Policy {
                schema: POLICY_SCHEMA.into(),
                enabled: true,
                workspaces: vec![label.to_owned()],
                max_running: running,
                engine: Engine {
                    adapter: coder::task::adapter::NAME.into(),
                    controller,
                    model: first.model.clone(),
                    effort: Some("medium".into()),
                    max_steps: None,
                    wall_seconds: None,
                    memory_bytes: 4096 * 1024 * 1024,
                    write_workspace: true,
                    decision_endpoint: "https://api.typesafe.ai".into(),
                    decision_model: DEFAULT_DECISION_MODEL.into(),
                    routes: Vec::new(),
                    usage_probe: None,
                    access: coder::task::adapter::Access::Boundary,
                    claude: ClaudeRuns::default(),
                    codex: CodexRuns::default(),
                },
                changed_at: plan.now,
            }
        }
    };
    policy.enabled = true;
    if plan.full_access {
        policy.engine.access = coder::task::adapter::Access::Full;
    }
    policy.engine.model = first.model.clone();
    policy.engine.routes = routes;
    policy.changed_at = plan.now;
    policy.validate()?;
    Ok(policy)
}

fn policy_detail(policy: &Policy) -> String {
    format!(
        "workspaces {} max_running {} routes {} by openagents studio up",
        policy.workspaces.join(","),
        policy.max_running,
        policy
            .routes()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",")
    )
}

/// A workspace label from a repository's directory name: lowercase
/// letters, digits, and hyphens.
pub(crate) fn label_for(repo: &Path) -> String {
    let name = repo
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut label = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            label.push(c);
        } else if !label.ends_with('-') {
            label.push('-');
        }
    }
    let label = label.trim_matches('-');
    if label.is_empty() {
        "repository".into()
    } else {
        label.chars().take(64).collect()
    }
}

/// The default team: a lead and two workers on the coding agents signed in
/// here, Codex first. With none signed in it sits on both, so the studio
/// opens and says why its seats cannot work.
pub(crate) fn default_team(connected: &dyn Fn(Provider) -> bool) -> Vec<Member> {
    let mut providers: Vec<Provider> = TEAM_PROVIDERS
        .into_iter()
        .filter(|provider| connected(*provider))
        .collect();
    if providers.is_empty() {
        providers = TEAM_PROVIDERS.to_vec();
    }
    let route = |provider: Provider| Route {
        provider,
        model: coder::task::settings::default_model(provider)
            .unwrap_or_default()
            .into(),
        effort: None,
        engine: None,
    };
    let last = providers[providers.len() - 1];
    vec![
        Member {
            name: "lead".into(),
            role: Role::Lead,
            route: route(providers[0]),
        },
        Member {
            name: "worker-1".into(),
            role: Role::Worker,
            route: route(last),
        },
        Member {
            name: "worker-2".into(),
            role: Role::Worker,
            route: route(providers[0]),
        },
    ]
}

/// `--team NAME=ROUTE[,NAME=ROUTE]...`: the first seat is the lead, the
/// rest are workers. ROUTE is PROVIDER:MODEL, or a provider alone for its
/// default model.
///
/// # Errors
/// An entry is malformed, a name repeats, or a provider has no default
/// model.
pub(crate) fn parse_team(text: &str) -> Result<Vec<Member>, String> {
    let mut members: Vec<Member> = Vec::new();
    for entry in text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let (name, route) = entry
            .split_once('=')
            .ok_or_else(|| format!("--team takes NAME=ROUTE entries, not `{entry}`"))?;
        let name = name.trim();
        let route = route.trim();
        let route = if route.contains(':') {
            route.to_owned()
        } else {
            let provider = Provider::from_config(route).ok_or_else(|| {
                format!("`{route}` is not codex, claude, devin, opencode, or grok")
            })?;
            let model = coder::task::settings::default_model(provider).ok_or_else(|| {
                format!("{route} has no default model; name one as {route}:MODEL")
            })?;
            format!("{route}:{model}")
        };
        let route = studio::parse_route(&route).map_err(|e| e.to_string())?;
        if members.iter().any(|member| member.name == name) {
            return Err(format!("--team names seat `{name}` twice"));
        }
        members.push(Member {
            name: name.to_owned(),
            role: if members.is_empty() {
                Role::Lead
            } else {
                Role::Worker
            },
            route,
        });
    }
    if members.is_empty() {
        return Err("--team names no seat".into());
    }
    Ok(members)
}

/// How `up` found the host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
pub(crate) enum Host {
    /// A host already answered and serves the workspace.
    Running,
    /// A host already answered but read its workspaces before `up` added
    /// this one.
    NeedsRestart,
    /// `up` started (or restarted) it.
    Started { pid: u32 },
    /// `--no-host`: nothing was checked or started.
    Skipped,
}

impl Host {
    fn sentence(&self) -> String {
        match self {
            Host::Running => "The host on this computer serves the studio.".into(),
            Host::NeedsRestart => "The host on this computer is running but has not read the new workspace: quit and reopen the OpenAgents app, or restart `coder host serve`, so its tasks start.".into(),
            Host::Started { pid } => format!("Started the host (pid {pid})."),
            Host::Skipped => "Left the host alone (--no-host).".into(),
        }
    }
}

/// Start `coder host serve` when no host answers the control socket, and
/// restart a host `up` started earlier when the workspace is new to it.
fn ensure_host(
    args: &Args,
    paths: &Paths,
    record: &mut Record,
    workspace_added: bool,
) -> Result<Host, String> {
    if args.switch("no-host") {
        return Ok(Host::Skipped);
    }
    let ours = record.host_pid.filter(|pid| pid_alive(*pid));
    if crate::host_answers_at(&paths.socket) {
        match (ours, workspace_added) {
            (_, false) => return Ok(Host::Running),
            (None, true) => return Ok(Host::NeedsRestart),
            (Some(pid), true) => {
                stop(pid)?;
                record.host_pid = None;
            }
        }
    }
    // A host set up by the desktop app keeps its keys in the system's key
    // store; this command starts only a host whose keys it holds, or a
    // new one.
    let book = coder_host::access::host::Host::new(
        &paths.state,
        coder_host::access::RelayPolicy::Production,
    )
    .state_path();
    let own_keys = openagents_connect::keys::FileKeySource::new(paths.keys.clone())
        .path(openagents_connect::keys::KeyName::Host)
        .exists();
    if book.exists() && !own_keys {
        return Err(format!(
            "a host is set up in {} but no host answers {}; open the OpenAgents app, or start `coder host serve`, and run `openagents studio up` again",
            paths.state.display(),
            paths.socket.display()
        ));
    }
    let program = find_program("coder", args.option("coder"))?;
    let pid = start_host(&program, paths, &serve_args(paths), None)?;
    record.host_pid = Some(pid);
    Ok(Host::Started { pid })
}

/// The `coder host serve` arguments for a host at `paths`.
pub(crate) fn serve_args(paths: &Paths) -> Vec<String> {
    let path = |path: &Path| path.display().to_string();
    vec![
        "host".into(),
        "serve".into(),
        "--state".into(),
        path(&paths.state),
        "--root".into(),
        path(&paths.root),
        "--tasks".into(),
        path(&paths.tasks),
        "--iroh".into(),
        "--control-socket".into(),
        path(&paths.socket),
        "--keys".into(),
        path(&paths.keys),
    ]
}

/// Start `coder host ARGS` for the host at `paths` and wait for its control
/// socket. `home`, when given, is the `HOME` it runs with.
#[cfg(unix)]
fn start_host(
    program: &Path,
    paths: &Paths,
    args: &[String],
    home: Option<&Path>,
) -> Result<u32, String> {
    let log = paths.root.join("studio-host.log");
    let mut child = spawn_detached(program, args, &log, home)?;
    let pid = child.id();
    let until = Instant::now() + HOST_WAIT;
    loop {
        if crate::host_answers_at(&paths.socket) {
            return Ok(pid);
        }
        if let Ok(Some(status)) = child.try_wait() {
            let text = std::fs::read_to_string(&log).unwrap_or_default();
            return Err(format!(
                "the host exited ({status}): {}",
                text.lines().last().unwrap_or("")
            ));
        }
        if Instant::now() >= until {
            return Err(format!(
                "the host did not open its control socket within {} s; see {}",
                HOST_WAIT.as_secs(),
                log.display()
            ));
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

#[cfg(not(unix))]
fn start_host(
    _program: &Path,
    _paths: &Paths,
    _args: &[String],
    _home: Option<&Path>,
) -> Result<u32, String> {
    Err("the studio starts a host only on macOS and Linux".into())
}

#[cfg(unix)]
fn launch_verse(program: &Path, extra: &[String], root: &Path) -> Result<u32, String> {
    let mut args = vec!["--everglade".to_owned()];
    args.extend(extra.iter().cloned());
    spawn_detached(program, &args, &root.join("studio-verse.log"), None).map(|child| child.id())
}

#[cfg(not(unix))]
fn launch_verse(_program: &Path, _extra: &[String], _root: &Path) -> Result<u32, String> {
    Err("the studio opens Verse only on macOS and Linux".into())
}

/// Run `program` in a session of its own, so it outlives this command,
/// with its output appended to `log` and `home`, when given, as its
/// `HOME`.
#[cfg(unix)]
fn spawn_detached(
    program: &Path,
    args: &[String],
    log: &Path,
    home: Option<&Path>,
) -> Result<std::process::Child, String> {
    use std::os::unix::fs::OpenOptionsExt;
    use std::os::unix::process::CommandExt;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(log)
        .map_err(|e| format!("{}: {e}", log.display()))?;
    let mut command = std::process::Command::new(program);
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log);
    if let Some(home) = home {
        command.env("HOME", home);
    }
    // SAFETY: setsid is async-signal-safe and touches no memory; it runs in
    // the child between fork and exec.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    command
        .spawn()
        .map_err(|e| format!("cannot start {}: {e}", program.display()))
}

/// `name` as `given`, else beside this program, else on `PATH`.
fn find_program(name: &str, given: Option<&str>) -> Result<PathBuf, String> {
    if let Some(path) = given {
        let path = PathBuf::from(path);
        return if path.is_file() {
            Ok(path)
        } else {
            Err(format!("{} does not exist", path.display()))
        };
    }
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(name)));
    let on_path = std::env::var_os("PATH")
        .map(|path| {
            std::env::split_paths(&path)
                .map(|dir| dir.join(name))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    beside
        .into_iter()
        .chain(on_path)
        .find(|path| path.is_file())
        .ok_or_else(|| {
            format!(
                "`{name}` is not beside openagents or on PATH; install it, or pass --{name} PATH"
            )
        })
}

fn pid_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        libc::pid_t::try_from(pid).is_ok_and(|pid| {
            // SAFETY: signal 0 only checks that the process exists.
            pid > 0 && unsafe { libc::kill(pid, 0) } == 0
        })
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// Ask `pid` to stop and wait for it.
fn stop(pid: u32) -> Result<(), String> {
    #[cfg(unix)]
    {
        let target = libc::pid_t::try_from(pid).map_err(|_| "a bad pid")?;
        // SAFETY: kill has no memory preconditions.
        unsafe {
            libc::kill(target, libc::SIGTERM);
        }
    }
    let until = Instant::now() + STOP_WAIT;
    while pid_alive(pid) {
        if Instant::now() >= until {
            return Err(format!("process {pid} did not stop"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

/// Close the Verse window an earlier `up` opened, so one studio window
/// stays open.
fn stop_verse(record: &mut Record) {
    if let Some(pid) = record.verse_pid.take().filter(|pid| pid_alive(*pid)) {
        let _ = stop(pid);
    }
}

/// `openagents studio down`. Returns the exit code.
pub(crate) fn down(output: &Output, paths: &Paths) -> u8 {
    let Some(mut record) = Record::load(&paths.root) else {
        output.emit(&json!({"stopped": false}), |_| {
            "Nothing to stop: `openagents studio up` has not run here.".into()
        });
        return 0;
    };
    stop_verse(&mut record);
    let mut notes = stop_sim(&mut record);
    if record.changed_host() {
        notes.extend(teardown(paths, &record));
    }
    if let Some(pid) = record.host_pid.filter(|pid| pid_alive(*pid)) {
        match stop(pid) {
            Ok(()) => notes.push(format!("Stopped the host (pid {pid}).")),
            Err(error) => notes.push(format!("The host did not stop: {error}.")),
        }
    }
    let _ = std::fs::remove_file(paths.root.join(RECORD_FILE));
    output.emit(&json!({"stopped": true, "notes": notes}), |_| {
        let mut lines = notes.clone();
        lines.push("The studio is down; what `up` found here is as it was.".into());
        lines.join("\n")
    });
    0
}

/// Undo `record`'s changes to the host's files and the studio's seats.
/// Returns a sentence for each change, and for each one that could not
/// be undone.
pub(crate) fn teardown(paths: &Paths, record: &Record) -> Vec<String> {
    let mut notes = Vec::new();
    match Studio::open(&paths.tasks) {
        Ok(mut team) => {
            for name in &record.seats_added {
                match team.remove_seat(name) {
                    Ok(()) => notes.push(format!("Removed seat {name}.")),
                    Err(error) => notes.push(format!("Kept seat {name}: {error}.")),
                }
            }
            for seat in &record.seats_replaced {
                match team.set_seat(seat.clone()) {
                    Ok(()) => notes.push(format!("Restored seat {}.", seat.name)),
                    Err(error) => {
                        notes.push(format!("Could not restore seat {}: {error}.", seat.name))
                    }
                }
            }
        }
        Err(error) => notes.push(format!("Could not open the studio: {error}.")),
    }
    if record.policy_written {
        let now = autostart::unix_now();
        let restored = match &record.previous_policy {
            Some(policy) => policy.save(&paths.root).map(|()| {
                let _ = autostart::note_policy(
                    &paths.root,
                    now,
                    policy.enabled,
                    "restored by openagents studio down",
                );
                "Restored the auto-start policy."
            }),
            None => std::fs::remove_file(paths.root.join(POLICY_FILE))
                .or_else(|error| match error.kind() {
                    std::io::ErrorKind::NotFound => Ok(()),
                    _ => Err(error),
                })
                .map_err(|error| error.to_string())
                .map(|()| {
                    let _ = autostart::note_policy(
                        &paths.root,
                        now,
                        false,
                        "removed by openagents studio down",
                    );
                    "Turned auto-start off."
                }),
        };
        notes.push(match restored {
            Ok(sentence) => sentence.into(),
            Err(error) => format!("Could not restore the auto-start policy: {error}."),
        });
    }
    for label in &record.workspaces_added {
        let removed = ServeSettings::load(&paths.root).and_then(|mut settings| {
            if settings.workspaces.remove(label).is_some() {
                settings.save(&paths.root)?;
            }
            Ok(())
        });
        notes.push(match removed {
            Ok(()) => format!("Removed workspace `{label}` from the host."),
            Err(error) => format!("Could not remove workspace `{label}`: {error}."),
        });
    }
    notes
}

/// Write `bytes` to `path`, mode `0600`, replacing it atomically.
fn write_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    let mut options = std::fs::OpenOptions::new();
    options.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let write = || -> std::io::Result<()> {
        use std::io::Write;
        let mut file = options.open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::rename(&temporary, path)
    };
    write().map_err(|e| format!("cannot write {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch host: its root, task store, and a Git checkout, all under
    /// one temporary directory, with a stand-in controller file.
    struct Scratch {
        _dir: tempfile::TempDir,
        paths: Paths,
        repo: PathBuf,
        controller: PathBuf,
    }

    fn scratch() -> Scratch {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().canonicalize().unwrap();
        let repo = base.join("My Repo");
        std::fs::create_dir_all(repo.join(".git")).unwrap();
        let controller = base.join("microcoder");
        std::fs::write(&controller, b"").unwrap();
        let paths = Paths::new(
            base.join("host"),
            base.join("tasks"),
            Some(base.join("control.sock")),
            false,
        );
        std::fs::create_dir_all(&paths.root).unwrap();
        Scratch {
            _dir: dir,
            paths,
            repo,
            controller,
        }
    }

    fn plan<'a>(scratch: &Scratch, connected: &'a dyn Fn(Provider) -> bool) -> Plan<'a> {
        Plan {
            repo: scratch.repo.clone(),
            workspace: None,
            team: None,
            controller: Some(scratch.controller.clone()),
            connected,
            now: 1_800_000_000,
            full_access: false,
        }
    }

    #[test]
    fn up_admits_the_repository_turns_auto_start_on_and_seats_the_team() {
        let scratch = scratch();
        let paths = &scratch.paths;
        let codex_only = |provider: Provider| provider == Provider::Codex;
        let mut record = Record::default();
        let setup = setup(paths, &plan(&scratch, &codex_only), &mut record).unwrap();

        assert_eq!(setup.workspace, "my-repo");
        assert!(setup.workspace_added && setup.policy_written && setup.engines_ready);
        let settings = ServeSettings::load(&paths.root).unwrap();
        assert_eq!(settings.workspaces["my-repo"], scratch.repo);

        let policy = Policy::load(&paths.root).unwrap().unwrap();
        assert!(policy.enabled && policy.admits("my-repo"));
        assert_eq!(policy.max_running, 3);
        let codex = coder::task::settings::default_model(Provider::Codex).unwrap();
        assert_eq!(policy.routes().len(), 1);
        assert_eq!(policy.routes()[0].to_string(), format!("codex:{codex}"));
        assert!(
            autostart::journal(&paths.root)
                .iter()
                .any(|entry| entry.event == "policy_on")
        );

        let team = Studio::open(&paths.tasks).unwrap();
        let seats = &team.state().seats;
        assert_eq!(
            seats.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
            ["lead", "worker-1", "worker-2"]
        );
        assert_eq!(seats[0].role, Role::Lead);
        assert!(
            seats
                .iter()
                .all(|seat| seat.route.provider == Provider::Codex)
        );
        drop(team);
        assert_eq!(record.seats_added.len(), 3);
        assert!(record.previous_policy.is_none() && record.policy_written);

        // A second run changes nothing and records nothing more.
        let mut again = record.clone();
        let second = setup_again(&scratch, &codex_only, &mut again);
        assert!(!second.workspace_added && !second.policy_written);
        assert_eq!(again, record);
    }

    fn setup_again(
        scratch: &Scratch,
        connected: &dyn Fn(Provider) -> bool,
        record: &mut Record,
    ) -> Setup {
        setup(&scratch.paths, &plan(scratch, connected), record).unwrap()
    }

    #[test]
    fn down_undoes_only_what_up_changed() {
        let scratch = scratch();
        let paths = &scratch.paths;
        // What the person had: another workspace, a policy, and a seat.
        let other = scratch.repo.parent().unwrap().join("other");
        std::fs::create_dir_all(other.join(".git")).unwrap();
        let mut settings = ServeSettings::new(Vec::new(), Default::default());
        settings.workspaces.insert("other".into(), other.clone());
        settings.save(&paths.root).unwrap();
        let none = |_: Provider| false;
        let mut first = Record::default();
        setup(
            paths,
            &Plan {
                repo: other.clone(),
                team: Some("mine=claude".into()),
                ..plan(&scratch, &none)
            },
            &mut first,
        )
        .unwrap();
        let before = Policy::load(&paths.root).unwrap().unwrap();
        let seat_before = Studio::open(&paths.tasks)
            .unwrap()
            .state()
            .seat("mine")
            .cloned()
            .unwrap();

        let mut record = Record::default();
        let setup = setup(
            paths,
            &Plan {
                team: Some("mine=codex,helper=claude:claude-opus-5-5".into()),
                ..plan(&scratch, &none)
            },
            &mut record,
        )
        .unwrap();
        assert!(!setup.engines_ready, "no coding agent signs in");
        let during = Policy::load(&paths.root).unwrap().unwrap();
        assert!(during.admits("other") && during.admits("my-repo"));
        assert_eq!(during.routes().len(), 2);
        assert_eq!(record.seats_added, ["helper"]);
        assert_eq!(record.seats_replaced, [seat_before.clone()]);

        let notes = teardown(paths, &record);
        assert!(
            notes.iter().all(|note| !note.starts_with("Could not")),
            "{notes:?}"
        );
        assert_eq!(Policy::load(&paths.root).unwrap().unwrap(), before);
        let settings = ServeSettings::load(&paths.root).unwrap();
        assert_eq!(settings.workspaces.len(), 1);
        assert_eq!(settings.workspaces["other"], other);
        let team = Studio::open(&paths.tasks).unwrap();
        assert_eq!(team.state().seats, [seat_before]);
    }

    #[test]
    fn down_removes_a_policy_up_created() {
        let scratch = scratch();
        let paths = &scratch.paths;
        let both = |_: Provider| true;
        let mut record = Record::default();
        setup(paths, &plan(&scratch, &both), &mut record).unwrap();
        assert!(paths.root.join(POLICY_FILE).exists());
        teardown(paths, &record);
        assert!(Policy::load(&paths.root).unwrap().is_none());
        assert!(
            ServeSettings::load(&paths.root)
                .unwrap()
                .workspaces
                .is_empty()
        );
        assert!(Studio::open(&paths.tasks).unwrap().state().seats.is_empty());
    }

    #[test]
    fn a_label_that_names_another_repository_is_refused() {
        let scratch = scratch();
        let mut settings = ServeSettings::new(Vec::new(), Default::default());
        settings
            .workspaces
            .insert("my-repo".into(), PathBuf::from("/elsewhere"));
        settings.save(&scratch.paths.root).unwrap();
        let both = |_: Provider| true;
        let error = setup(
            &scratch.paths,
            &plan(&scratch, &both),
            &mut Record::default(),
        )
        .unwrap_err();
        assert!(error.contains("--workspace"), "{error}");
        // A plain directory is no repository.
        let plain = scratch.repo.parent().unwrap().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let error = setup(
            &scratch.paths,
            &Plan {
                repo: plain,
                ..plan(&scratch, &both)
            },
            &mut Record::default(),
        )
        .unwrap_err();
        assert!(error.contains("Git checkout"), "{error}");
    }

    #[test]
    fn the_default_team_sits_on_the_signed_in_agents() {
        let both = default_team(&|_| true);
        let providers: Vec<Provider> = both.iter().map(|m| m.route.provider).collect();
        assert_eq!(
            providers,
            [Provider::Codex, Provider::Claude, Provider::Codex]
        );
        assert_eq!(both[0].role, Role::Lead);
        let claude = default_team(&|provider| provider == Provider::Claude);
        assert!(claude.iter().all(|m| m.route.provider == Provider::Claude));
        // None signed in: both, so the studio opens and says why.
        let none = default_team(&|_| false);
        assert_eq!(none[0].route.provider, Provider::Codex);
        assert_eq!(none[1].route.provider, Provider::Claude);
    }

    #[test]
    fn a_team_names_its_lead_first() {
        let team = parse_team("ada=codex:gpt-6.1-sol, bo=claude").unwrap();
        assert_eq!(team[0].role, Role::Lead);
        assert_eq!(team[1].role, Role::Worker);
        assert_eq!(
            team[1].route.model,
            coder::task::settings::default_model(Provider::Claude).unwrap()
        );
        assert!(parse_team("").is_err());
        assert!(parse_team("ada").is_err());
        assert!(parse_team("ada=codex,ada=claude").is_err());
        assert!(parse_team("ada=opencode").is_err());
        assert!(parse_team("ada=gemini").is_err());
    }

    #[test]
    fn labels_come_from_the_directory_name() {
        assert_eq!(label_for(Path::new("/x/My Repo")), "my-repo");
        assert_eq!(label_for(Path::new("/x/openagents")), "openagents");
        assert_eq!(label_for(Path::new("/x/__")), "repository");
    }

    #[test]
    fn options_are_remembered_and_a_given_one_replaces_its_default() {
        let scratch = scratch();
        let root = &scratch.paths.root;
        assert_eq!(Defaults::load(root), Defaults::default());
        let args = |words: &[&str]| {
            let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
            Args::parse(&words, SWITCHES).unwrap()
        };
        let first = Defaults::default().merged(&args(&["--repo", "/r", "--team", "a=codex"]));
        first.save(root).unwrap();
        let loaded = Defaults::load(root);
        assert_eq!(loaded.repo.as_deref(), Some("/r"));
        assert_eq!(loaded.team.as_deref(), Some("a=codex"));
        let sim = loaded.clone().merged(&args(&["--sim"]));
        assert!(sim.sim && sim.repo.as_deref() == Some("/r"));
        let back = sim.merged(&args(&["--repo", "/s"]));
        assert!(!back.sim);
        assert_eq!(back.team.as_deref(), Some("a=codex"));
    }

    #[test]
    fn the_host_command_names_the_scratch_paths() {
        let scratch = scratch();
        let args = serve_args(&scratch.paths);
        let after = |flag: &str| {
            let at = args.iter().position(|a| a == flag).unwrap();
            PathBuf::from(&args[at + 1])
        };
        assert_eq!(&args[..2], ["host", "serve"]);
        assert_eq!(after("--root"), scratch.paths.root);
        assert_eq!(after("--tasks"), scratch.paths.tasks);
        assert_eq!(after("--control-socket"), scratch.paths.socket);
        let base = scratch.paths.root.parent().unwrap();
        assert_eq!(after("--state"), base.join("coder-access"));
        assert_eq!(after("--keys"), base.join("connect"));
    }

    #[test]
    fn the_record_round_trips() {
        let scratch = scratch();
        let record = Record {
            workspace: Some("w".into()),
            workspaces_added: vec!["w".into()],
            seats_added: vec!["lead".into()],
            host_pid: Some(42),
            ..Record::default()
        };
        record.save(&scratch.paths.root).unwrap();
        let loaded = Record::load(&scratch.paths.root).unwrap();
        assert_eq!(loaded.workspace, record.workspace);
        assert_eq!(loaded.workspaces_added, ["w"]);
        assert_eq!(loaded.host_pid, Some(42));
        assert_eq!(loaded.schema, RECORD_SCHEMA);
    }

    #[test]
    fn the_simulated_host_serves_its_own_scratch_paths_without_iroh() {
        let dir = tempfile::tempdir().unwrap();
        let scratch = studio_sim::Scratch::create(&dir.path().join("sim")).unwrap();
        let paths = sim_paths(&scratch);
        let args = sim_serve_args(&paths);
        let after = |flag: &str| {
            let at = args.iter().position(|a| a == flag).unwrap();
            PathBuf::from(&args[at + 1])
        };
        assert_eq!(&args[..2], ["host", "serve"]);
        assert_eq!(after("--root"), scratch.root);
        assert_eq!(after("--tasks"), scratch.store);
        assert_eq!(after("--state"), scratch.state);
        assert_eq!(after("--keys"), scratch.keys);
        assert_eq!(after("--control-socket"), scratch.socket);
        assert!(args.iter().any(|a| a == "--studio-sim"));
        assert!(!args.iter().any(|a| a == "--iroh" || a == "--keychain"));
        // Every path is inside the scratch directory.
        for path in [
            &paths.root,
            &paths.tasks,
            &paths.state,
            &paths.keys,
            &paths.socket,
        ] {
            assert!(path.starts_with(&scratch.dir), "{}", path.display());
        }
    }

    #[test]
    fn down_stops_the_simulated_host_and_removes_only_a_scratch_directory() {
        let dir = tempfile::tempdir().unwrap();
        let scratch = studio_sim::Scratch::create(&dir.path().join("sim")).unwrap();
        let plain = dir.path().join("plain");
        std::fs::create_dir_all(&plain).unwrap();
        let mut record = Record {
            sim_dir: Some(plain.clone()),
            ..Record::default()
        };
        assert!(stop_sim(&mut record).is_empty());
        assert!(plain.is_dir(), "a directory no scratch host marked stays");
        assert!(!record.changed_host());
        let mut record = Record {
            sim_dir: Some(scratch.dir.clone()),
            ..Record::default()
        };
        let notes = stop_sim(&mut record);
        assert_eq!(notes.len(), 1, "{notes:?}");
        assert!(!scratch.dir.exists());
        assert!(record.sim_dir.is_none());
    }
}

/// The switches `openagents studio` takes; every other `--name` takes a
/// value.
pub(crate) const SWITCHES: &[&str] = &["sim", "no-verse", "no-host", "full-access"];

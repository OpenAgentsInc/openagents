//! `openagents background`: the host's background rules
//! (docs/background/2026-10-02-background-processes.md): the built-in disk
//! cleanup monitor `disk`, and the rules of plugins turned on here
//! (docs/background/2026-10-02-disk-cleanup-plugin.md). Every command reads and writes
//! `~/.openagents/background`; `run` runs in this process under the run
//! lock, so it works with or without a host.

pub(crate) use crate::jev_judge::{JevJudge, NO_JEV};
use std::path::PathBuf;

use background::{Cause, Layout, run, store, view};
use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents background COMMAND [OPTIONS]
  list            Each rule: on or paused, free space, and its last result.
  show ID         The rule's definition, version, and digest.
  add --file PATH Add or replace a rule from a JSON file.
  add --message TEXT [--yes]
                  Make a rule from your words (Jev reads them over the
                  host's built-in triggers, conditions, and actions); show
                  it and its dry run, and save it with --yes or apply.
  edit ID [--message TEXT] [--yes]
                  Change the rule: as your words say (shown as the lines
                  that change, with the dry run), or as JSON in $EDITOR.
  draft TEXT... [--id ID] [--project DIR] [--thread ID]
                  Read words as a new rule, a change, a pause, a resume, or
                  a removal; show it and its dry run and keep it as draft
                  ID, saving nothing. The chat and /background use this.
  apply DRAFT     Save what draft DRAFT shows.
  pause ID [--until TIME]
                  Stop the rule, until TIME (2h, 1d, 2026-10-03, or seconds
                  since the epoch) or until resumed.
  resume ID       Turn the rule on again: a paused rule, or one that ships
                  paused.
  run ID [--dry-run]
                  Run the rule now; --dry-run shows exactly what it would
                  delete and why, and what it keeps, changing nothing.
  log [ID] [--since TIME] [--stats]
                  The audit log; --stats totals bytes freed by week and class.
  undo RUN        Recreate the worktrees that run removed, and put back
                  what it moved to the trash.
  proposals       Folders Jev judged to be caches, waiting for you.
  confirm PATH    Clean that folder from now on (to the trash first, for
                  a day).
  decline PATH    Keep that folder and never ask about it again.
  judge           Ask Jev now about the largest folders no rule covers.
                  Nothing is deleted.
  publish ID [--out DIR]
                  Package the rule as a plugin folder to publish.
  kache [--status]
                  Run kache's own collector on the compile cache, after
                  any collection already running, and report what it freed
                  (docs/background/kache.md); --status only reports the
                  store and who holds its collection lock.
Every command takes --tasks DIR (the Coder task store, default
~/.openagents/tasks). Rules run in the host on their own: the built-in
disk rule, each rule a plugin brings while the plugin is on here
(openagents plugin enable), and each rule made from words and confirmed.
These commands look at them, change them, or run one now. Words become a
rule through Jev (OpenAgents' hosted Jev, or your own TypeSafe key from
`openagents settings provider-key set typesafe`); a rule
only uses the host's own actions (delete build caches and finished
worktrees, notify, fast-forward a clean checkout, start a Coder run, and
the built-in processes), never another command.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("add", Effect::LocalWrite),
    Declared::computer("edit", Effect::LocalWrite),
    // A draft is shown and kept, never applied: it changes no rule.
    Declared::computer("draft", Effect::ReadOnly),
    Declared::computer("apply", Effect::LocalWrite),
    Declared::computer("pause", Effect::LocalWrite),
    Declared::computer("resume", Effect::LocalWrite),
    Declared::computer("run", Effect::LocalWrite),
    Declared::computer("log", Effect::ReadOnly),
    Declared::computer("undo", Effect::LocalWrite),
    Declared::computer("proposals", Effect::ReadOnly),
    Declared::computer("confirm", Effect::LocalWrite),
    Declared::computer("decline", Effect::LocalWrite),
    // Asks Jev and keeps its proposals; deletes nothing.
    Declared::computer("judge", Effect::LocalWrite),
    Declared::computer("publish", Effect::LocalWrite),
    // Runs kache's collector, which drops cache entries; never a rule.
    Declared::computer("kache", Effect::LocalWrite),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("background", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    if rest.first().is_some_and(|word| word == "--help") {
        if let Some(usage) = crate::argv::command_usage("background", command, USAGE) {
            println!("{usage}");
            return 0;
        }
    }
    let args = match Args::parse(rest, &["dry-run", "stats", "status", "yes"]) {
        Ok(args) => args,
        Err(message) => return output.usage("background", &message, USAGE),
    };
    let layout = match layout(&args) {
        Ok(layout) => layout,
        Err(message) => return output.fail("background", &message),
    };
    let id = args.positional().first().cloned();
    let need_id = || {
        id.clone()
            .ok_or_else(|| Failure::Usage(format!("{command} needs a rule ID")))
    };
    let result = match command.as_str() {
        "list" => list(output, &layout),
        "show" => need_id().and_then(|id| show(output, &layout, &id)),
        "add" if args.option("message").is_some() => from_words(output, &layout, &args, None),
        "add" => add(output, &layout, &args),
        "edit" if args.option("message").is_some() => {
            need_id().and_then(|id| from_words(output, &layout, &args, Some(&id)))
        }
        "edit" => need_id().and_then(|id| edit(output, &layout, &id)),
        "draft" => draft(output, &layout, &args),
        "apply" => need_id().and_then(|id| apply(output, &layout, &id)),
        "pause" => need_id().and_then(|id| pause(output, &layout, &id, &args, false)),
        "resume" => need_id().and_then(|id| pause(output, &layout, &id, &args, true)),
        "run" => need_id().and_then(|id| run_now(output, &layout, &id, args.switch("dry-run"))),
        "log" => log(output, &layout, id.as_deref(), &args),
        "undo" => need_id().and_then(|id| undo(output, &layout, &id)),
        "proposals" => proposals(output, &layout),
        "confirm" => need_id().and_then(|path| confirm(output, &layout, &path, true)),
        "decline" => need_id().and_then(|path| confirm(output, &layout, &path, false)),
        "judge" => judge_now(output, &layout),
        "publish" => need_id().and_then(|id| publish(output, &layout, &id, &args)),
        "kache" => kache(output, args.switch("status")),
        other => {
            return output.usage("background", &format!("unknown command `{other}`"), USAGE);
        }
    };
    match result {
        Ok(()) => 0,
        Err(Failure::Usage(message)) => output.usage("background", &message, USAGE),
        Err(Failure::Refused(message)) => output.fail("background", &message),
    }
}

#[derive(Debug)]
enum Failure {
    Usage(String),
    Refused(String),
}

fn layout(args: &Args) -> Result<Layout, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or("HOME must be an absolute path")?;
    Layout::new(&home, args.option("tasks").map(PathBuf::from)).map_err(|error| error.to_string())
}

fn list(output: &Output, layout: &Layout) -> Result<(), Failure> {
    let rows = view::list(layout);
    let host = background::presence::summary(layout);
    output.emit(&json!({ "rules": rows, "host": host }), |_| {
        let mut lines: Vec<String> = rows.iter().map(list_line).collect();
        if let Some(host) = &host {
            lines.push(host.clone());
        }
        if let Some(paused) = rows.iter().find(|row| {
            row.error.is_none()
                && !row.enabled
                && !(row.id == "disk"
                    && background::plugins::enabled(layout)
                        .iter()
                        .any(|plugin| plugin.ends_with(":disk-cleanup")))
        }) {
            lines.push(format!(
                "Turn a paused rule on: openagents background resume {}",
                paused.id
            ));
        }
        lines.join("\n")
    });
    Ok(())
}

/// A rule's line in `background list`: its id, then what it does in words
/// (its name), so a newcomer can tell what they would turn on (#10320).
fn list_line(row: &view::Row) -> String {
    let full = row.line();
    if row.name.is_empty() || row.name == row.id {
        return full;
    }
    match full.strip_prefix(&format!("{} · ", row.id)) {
        Some(rest) => format!("{} ({}) · {rest}", row.id, row.name),
        None => full,
    }
}

fn show(output: &Output, layout: &Layout, id: &str) -> Result<(), Failure> {
    let rule = store::load(layout, id).map_err(Failure::Refused)?;
    let value = json!({ "rule": rule, "digest": rule.digest() });
    output.emit(&value, |_| {
        format!(
            "{} (version {}, {})\n{}",
            rule.name,
            rule.version,
            rule.digest(),
            background::compile::describe(&rule).join("\n")
        )
    });
    Ok(())
}

fn add(output: &Output, layout: &Layout, args: &Args) -> Result<(), Failure> {
    let file = args
        .option("file")
        .ok_or_else(|| Failure::Usage("add needs --file PATH".into()))?;
    let bytes =
        std::fs::read(file).map_err(|error| Failure::Refused(format!("{file}: {error}")))?;
    let rule: background::Rule = serde_json::from_slice(&bytes)
        .map_err(|error| Failure::Refused(format!("{file}: {error}")))?;
    let saved = store::save(layout, &rule).map_err(Failure::Refused)?;
    output.emit(&json!({ "rule": saved }), |_| {
        format!("Saved {} version {}.", saved.id, saved.version)
    });
    Ok(())
}

impl background::engine::Judge for JevJudge {
    fn ask(
        &self,
        state: &str,
        questions: &[(String, background::engine::Question)],
    ) -> Result<std::collections::BTreeMap<String, background::engine::Answer>, String> {
        use background::engine::{Answer, Question};
        let mut asked = jev::Questions::new();
        for (id, question) in questions {
            asked = match question {
                Question::Noul(text) => asked.with(id.clone(), jev::Noul::new(text.clone())),
                Question::Choice {
                    instructions,
                    options,
                } => {
                    let mut choice = jev::Choice::new(instructions.clone(), Default::default());
                    for (name, what) in options {
                        choice = choice.option(name.clone(), what.clone());
                    }
                    asked.with(id.clone(), choice)
                }
            };
        }
        let request = jev::SystemOneRequest::new(state.to_owned(), asked);
        let client = self.client();
        // Its own runtime on its own thread, whatever the caller runs on.
        let response = std::thread::spawn(move || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|error| error.to_string())?
                .block_on(client.system_one(request))
                .map_err(|error| error.to_string())
        })
        .join()
        .map_err(|_| "the Jev request stopped".to_owned())??;
        let mut answers = std::collections::BTreeMap::new();
        for (id, question) in questions {
            let answer = match question {
                Question::Noul(_) => Answer {
                    noul: response.noul(id).ok().map(|answer| answer.noul),
                    choice: Vec::new(),
                },
                Question::Choice { .. } => Answer {
                    noul: None,
                    choice: response
                        .choice(id)
                        .map(|answer| {
                            answer
                                .probabilities
                                .iter()
                                .map(|(name, p)| (name.clone(), *p))
                                .collect()
                        })
                        .unwrap_or_default(),
                },
            };
            answers.insert(id.clone(), answer);
        }
        Ok(answers)
    }
}

/// What only the host can do for a background rule (phase 3): start a
/// Coder run, read and release this computer's stale issue claims,
/// probe the relay and the host and restart the host through the service
/// manager, read what Coder runs did, open or note an issue, and run an
/// installed plugin's background action read-only.
pub(crate) struct HostServices {
    store: PathBuf,
    home: PathBuf,
}

impl HostServices {
    pub(crate) fn at(
        store: &std::path::Path,
    ) -> std::sync::Arc<dyn background::services::Services> {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        std::sync::Arc::new(Self {
            store: store.to_owned(),
            home,
        })
    }

    /// The checkout a run works in when the rule names none: `~/openagents`
    /// (CoderOS) or `~/work/openagents`.
    fn workspace(&self, named: Option<&str>) -> Result<PathBuf, String> {
        if let Some(named) = named {
            return Ok(PathBuf::from(named));
        }
        ["openagents", "work/openagents"]
            .iter()
            .map(|dir| self.home.join(dir))
            .find(|dir| dir.join(".git").exists())
            .ok_or_else(|| "no openagents checkout here; name a workspace in the rule".into())
    }
}

/// The command that posts a scheduled prompt into the existing Coder chat
/// `chat` (#11177): `openagents coder chat`, which continues the saved
/// chat with its history, answers with this computer's tools in `dir`,
/// saves it, and sends it to the account when sync is on. The prompt goes
/// on its stdin.
fn chat_command(program: &std::path::Path, dir: &std::path::Path, chat: &str) -> Vec<String> {
    vec![
        program.display().to_string(),
        "coder".into(),
        "chat".into(),
        "--in".into(),
        dir.display().to_string(),
        "--session".into(),
        chat.into(),
        "--stdin".into(),
    ]
}

impl HostServices {
    /// Post `run`'s prompt into the chat `chat`: start `openagents coder
    /// chat` on its own, its output in a log beside the background rules.
    /// Returns the chat's id.
    fn post_into_chat(
        &self,
        run: &background::services::CoderRun,
        chat: &str,
    ) -> Result<String, String> {
        use std::io::Write;
        if !background::rule::chat_id(chat) {
            return Err(format!("`{chat}` is not a Coder chat"));
        }
        // The rule's folder when it names one, else the default checkout or
        // the home folder: the chat keeps its history either way.
        let dir = match run.workspace.as_deref() {
            Some(named) => PathBuf::from(named),
            None => self.workspace(None).unwrap_or_else(|_| self.home.clone()),
        };
        let program = std::env::current_exe().map_err(|error| error.to_string())?;
        let command = chat_command(&program, &dir, chat);
        let logs = self.home.join(".openagents/background/chat-posts");
        std::fs::create_dir_all(&logs).map_err(|error| error.to_string())?;
        let log = std::fs::File::create(logs.join(format!("{chat}.log")))
            .map_err(|error| error.to_string())?;
        let errors = log.try_clone().map_err(|error| error.to_string())?;
        let mut child = std::process::Command::new(&command[0])
            .args(&command[1..])
            .current_dir(&dir)
            .stdin(std::process::Stdio::piped())
            .stdout(log)
            .stderr(errors)
            .spawn()
            .map_err(|error| format!("couldn't start Coder for the chat: {error}"))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(run.prompt.as_bytes())
                .map_err(|error| error.to_string())?;
        }
        // The chat answers on its own; the runner doesn't wait for it.
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        Ok(chat.to_owned())
    }
}

impl background::services::Services for HostServices {
    fn recalibrate(&self, dry_run: bool) -> Result<String, String> {
        let (report, _) = crate::efficiency::recalibrate(&self.store, !dry_run)?;
        Ok(coder::efficiency::refit::line(&report))
    }

    fn start_coder_run(&self, run: &background::services::CoderRun) -> Result<String, String> {
        if let Some(chat) = &run.chat {
            return self.post_into_chat(run, chat);
        }
        let dir = self.workspace(run.workspace.as_deref())?;
        coder::task::local::Local::here(self.store.clone())
            .start(&dir, &run.title, &run.prompt, None)
            .map(|record| record.task)
    }

    fn stale_claims(
        &self,
        idle_hours: u64,
        now: u64,
    ) -> Result<Vec<background::services::Claim>, String> {
        Ok(coder::task::issue_run::stale_claims(
            &self.store,
            &coder::claim::Gh,
            now,
            idle_hours,
        ))
    }

    fn release_claim(&self, claim: &background::services::Claim) -> Result<(), String> {
        coder::task::issue_run::release_stale(&coder::claim::Gh, claim)
    }

    fn probe(&self, target: background::rule::Watched) -> Result<(), String> {
        use std::net::ToSocketAddrs;
        match target {
            background::rule::Watched::Relay => {
                let addr = "relay.openagents.com:443"
                    .to_socket_addrs()
                    .map_err(|error| error.to_string())?
                    .next()
                    .ok_or("the relay's name did not resolve")?;
                std::net::TcpStream::connect_timeout(&addr, std::time::Duration::from_secs(10))
                    .map(drop)
                    .map_err(|error| error.to_string())
            }
            background::rule::Watched::Host => {
                let layout = Layout::new(&self.home, Some(self.store.clone()))
                    .map_err(|error| error.to_string())?;
                if view::runner_running(&layout) {
                    Ok(())
                } else {
                    Err("no host is running here".into())
                }
            }
        }
    }

    fn restart(&self, _target: background::rule::Watched) -> Result<String, String> {
        use coder_service::launcher::{Config, Layout as HostLayout};
        let layout = HostLayout::new(self.home.join(".openagents/host"));
        let config = Config::load(&layout).map_err(|error| error.to_string())?;
        coder_service::service::restart(&config, &mut coder_service::service::SystemRunner)
            .map_err(|error| error.to_string())?;
        Ok("restarted the host through the service manager.".into())
    }

    fn usage(&self, since: u64) -> Result<background::services::Usage, String> {
        coder::task::recent::usage(&self.store, since)
    }

    fn failures(&self, since: u64) -> Result<Vec<background::services::Failure>, String> {
        coder::task::recent::failures(&self.store, since)
    }

    fn report_issue(
        &self,
        title: &str,
        body: &str,
        existing: Option<&str>,
    ) -> Result<String, String> {
        let dir = self.workspace(None)?;
        match existing {
            Some(issue) => {
                let number = issue.rsplit(['/', '#']).next().unwrap_or(issue);
                coder::claim::gh(Some(&dir), &["issue", "comment", number, "--body", body])?;
                Ok(issue.to_owned())
            }
            None => coder::claim::gh(
                Some(&dir),
                &["issue", "create", "--title", title, "--body", body],
            )
            .map(|url| url.trim().to_owned()),
        }
    }

    fn run_plugin(&self, plugin: &str, input: &str) -> Result<String, String> {
        let layout =
            Layout::new(&self.home, Some(self.store.clone())).map_err(|error| error.to_string())?;
        let installed = background::plugins::find(&layout, plugin)?;
        let workspace = self.workspace(None).unwrap_or_else(|_| self.home.clone());
        let ran = crate::ext_run::execute(&installed.dir, &workspace, input)?;
        Ok(ran["reply"].as_str().unwrap_or_default().to_owned())
    }
}

/// What words compiled to, shown: the card and its dry run, or the
/// question, or why nothing changes. `Some(id)` when a draft waits.
pub(crate) fn compile_words(
    layout: &Layout,
    message: &str,
    thread: &str,
    project: Option<PathBuf>,
    new_only: bool,
) -> Result<(Vec<String>, Option<String>, Value), String> {
    use background::compile::{self, Compiled, Context};
    let judge = JevJudge::from_env().ok_or(NO_JEV)?;
    let clock = background::engine::Clock::here();
    let context = Context {
        thread: thread.to_owned(),
        project,
        clock,
        new_only,
    };
    // An answer to the question this thread was just asked is read with
    // the words that led to it.
    let words = match compile::take_pending(layout, thread, clock.now) {
        Some(pending) => compile::answered(&pending, message),
        None => message.to_owned(),
    };
    let message = words.as_str();
    let compiled = compile::compile(layout, message, &context, &judge)?;
    match compiled {
        Compiled::Draft(draft) => {
            let store_dir = layout.store.clone();
            let facts = move || coder::task::background_facts(&store_dir);
            let env = background::Env {
                layout,
                facts: Some(&facts),
                volumes: &background::volume::Statvfs,
                processes: &background::inuse::System,
                now: clock.now,
                kache: Some(&KACHE),
            };
            let mut lines = compile::card(&draft);
            lines.extend(compile::show_dry_run(&compile::dry_run(
                &env, &draft, clock,
            )));
            compile::save_draft(layout, &draft)?;
            let value = json!({ "draft": *draft, "card": lines });
            Ok((lines, Some(draft.id.clone()), value))
        }
        Compiled::Question { text, readings } => {
            compile::drop_draft(layout, thread);
            compile::save_pending(
                layout,
                thread,
                &compile::Pending {
                    message: message.to_owned(),
                    question: text.clone(),
                    asked: clock.now,
                },
            )?;
            let value = json!({
                "question": text,
                "readings": readings,
            });
            Ok((vec![text], None, value))
        }
        Compiled::Unchanged { text } => {
            compile::drop_draft(layout, thread);
            Ok((vec![text.clone()], None, json!({ "unchanged": text })))
        }
    }
}

/// `add --message` and `edit ID --message`: compile, show, and save with
/// `--yes`; otherwise keep the draft and say how to save it.
fn from_words(
    output: &Output,
    layout: &Layout,
    args: &Args,
    edit: Option<&str>,
) -> Result<(), Failure> {
    let message = args.option("message").unwrap_or_default().trim().to_owned();
    if message.is_empty() {
        return Err(Failure::Usage("--message needs words".into()));
    }
    // `edit ID` names the rule; the words then say what changes.
    let text = match edit {
        Some(id) => {
            let rule = store::load(layout, id).map_err(Failure::Refused)?;
            format!("For the rule {} ({}): {message}", rule.name, rule.id)
        }
        None => message.clone(),
    };
    let id = format!("cli-{}", background::paths::now());
    let (lines, drafted, mut value) = compile_words(
        layout,
        &text,
        &id,
        std::env::current_dir().ok(),
        edit.is_none(),
    )
    .map_err(Failure::Refused)?;
    if let Some(draft) = drafted.as_deref()
        && args.switch("yes")
    {
        let (_, saved) = background::compile::apply(layout, draft, background::paths::now())
            .map_err(Failure::Refused)?;
        value["saved"] = json!(saved);
        output.emit(&value, |_| {
            let mut out = lines.clone();
            out.push(match &saved {
                Some(rule) => format!("Saved {} version {}.", rule.id, rule.version),
                None => "Removed.".into(),
            });
            out.join("\n")
        });
        return Ok(());
    }
    output.emit(&value, |_| {
        let mut out = lines.clone();
        if let Some(draft) = &drafted {
            out.push(format!(
                "Nothing is saved yet. Save it: openagents background apply {draft}"
            ));
        }
        out.join("\n")
    });
    Ok(())
}

/// `draft TEXT...`: compile and show, keep the draft, save nothing.
fn draft(output: &Output, layout: &Layout, args: &Args) -> Result<(), Failure> {
    let message = args.positional().join(" ").trim().to_owned();
    if message.is_empty() {
        return Err(Failure::Usage("draft needs words".into()));
    }
    let id = args.option("id").or(args.option("thread")).map_or_else(
        || format!("cli-{}", background::paths::now()),
        str::to_owned,
    );
    let project = args
        .option("project")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok());
    let (lines, _, value) = compile_words(
        layout,
        &message,
        &background::compile::draft_id(&id),
        project,
        false,
    )
    .map_err(Failure::Refused)?;
    output.emit(&value, |_| lines.join("\n"));
    Ok(())
}

/// `apply DRAFT`: save what the draft shows.
fn apply(output: &Output, layout: &Layout, id: &str) -> Result<(), Failure> {
    let now = background::paths::now();
    let (draft, saved) = background::compile::apply(layout, id, now).map_err(Failure::Refused)?;
    // Whether it will run is what the host here can do, not what was
    // saved (#10349).
    let pickup = saved
        .as_ref()
        .filter(|rule| rule.enabled)
        .map(|rule| background::presence::pickup(layout, &rule.id, now));
    let runs = pickup.as_ref().map(pickup_json);
    output.emit(
        &json!({ "draft": draft, "saved": saved, "runs": runs }),
        |_| match &saved {
            Some(rule) => {
                let head = match draft.kind {
                    background::compile::Kind::Define => {
                        format!("Saved {} ({}).", rule.name, rule.id)
                    }
                    _ => format!("Saved {} version {}.", rule.name, rule.version),
                };
                match &pickup {
                    Some(pickup) => format!(
                        "{head} {} openagents background list shows it.",
                        background::presence::sentence(pickup)
                    ),
                    None => head,
                }
            }
            None => format!("Removed {}.", draft.rule.name),
        },
    );
    Ok(())
}

/// Whether a saved rule runs, for `--json` (#10349).
fn pickup_json(pickup: &background::presence::Pickup) -> serde_json::Value {
    use background::presence::Pickup;
    let state = match pickup {
        Pickup::Running => "running",
        Pickup::Soon => "soon",
        Pickup::Missed(_) => "missed",
        Pickup::OldHost(_) => "old_host",
        Pickup::NoHost => "no_host",
    };
    json!({ "state": state, "says": background::presence::sentence(pickup) })
}

fn edit(output: &Output, layout: &Layout, id: &str) -> Result<(), Failure> {
    let rule = store::load(layout, id).map_err(Failure::Refused)?;
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .unwrap_or_else(|_| "vi".into());
    let dir = tempfile::tempdir().map_err(|error| Failure::Refused(error.to_string()))?;
    let path = dir.path().join(format!("{id}.json"));
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&rule).map_err(|error| Failure::Refused(error.to_string()))?,
    )
    .map_err(|error| Failure::Refused(error.to_string()))?;
    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("{editor} \"$1\""))
        .arg("sh")
        .arg(&path)
        .status()
        .map_err(|error| Failure::Refused(format!("{editor}: {error}")))?;
    if !status.success() {
        return Err(Failure::Refused(format!("{editor} exited with {status}")));
    }
    let bytes = std::fs::read(&path).map_err(|error| Failure::Refused(error.to_string()))?;
    let edited: background::Rule =
        serde_json::from_slice(&bytes).map_err(|error| Failure::Refused(error.to_string()))?;
    if edited == rule {
        output.emit(&json!({ "rule": rule, "changed": false }), |_| {
            "No change.".into()
        });
        return Ok(());
    }
    let saved = store::save(layout, &edited).map_err(Failure::Refused)?;
    if !output.json() {
        println!(
            "Saved {} version {}. Its dry run now:",
            saved.id, saved.version
        );
    }
    run_now(output, layout, &saved.id, true)
}

/// A time: `30m`, `2h`, `1d` from now, `YYYY-MM-DD` (midnight UTC), or
/// seconds since the epoch.
fn time(text: &str) -> Result<u64, Failure> {
    let bad = || {
        Failure::Usage(format!(
            "`{text}` is not a time (2h, 1d, 2026-10-03, or seconds)"
        ))
    };
    let now = background::paths::now();
    if let Some((number, unit)) = text
        .char_indices()
        .last()
        .filter(|(_, unit)| matches!(unit, 'm' | 'h' | 'd'))
        .map(|(at, unit)| (&text[..at], unit))
    {
        let number: u64 = number.parse().map_err(|_| bad())?;
        let unit = match unit {
            'm' => 60,
            'h' => 3600,
            _ => 86_400,
        };
        return Ok(now + number * unit);
    }
    if let [year, month, day] = text.split('-').collect::<Vec<_>>()[..] {
        let (year, month, day): (u64, u64, u64) = (
            year.parse().map_err(|_| bad())?,
            month.parse().map_err(|_| bad())?,
            day.parse().map_err(|_| bad())?,
        );
        // Search the day: dates are a bounded field.
        let guess = (year.saturating_sub(1970)) * 365 * 86_400;
        for offset in 0..800u64 {
            let at = guess + offset * 86_400;
            if background::view::date(at) == format!("{year:04}-{month:02}-{day:02}") {
                return Ok(at);
            }
        }
        return Err(bad());
    }
    text.parse().map_err(|_| bad())
}

fn pause(
    output: &Output,
    layout: &Layout,
    id: &str,
    args: &Args,
    resume: bool,
) -> Result<(), Failure> {
    let until = args.option("until").map(time).transpose()?;
    let now = background::paths::now();
    let rule = view::pause(layout, id, until, resume).map_err(Failure::Refused)?;
    let pickup = resume.then(|| background::presence::pickup(layout, &rule.id, now));
    let runs = pickup.as_ref().map(pickup_json);
    output.emit(&json!({ "rule": rule, "runs": runs }), |_| {
        match (resume, until) {
            (true, _) => match &pickup {
                Some(pickup) => format!(
                    "{} is on. {}",
                    rule.id,
                    background::presence::sentence(pickup)
                ),
                None => format!("{} is on.", rule.id),
            },
            (false, Some(until)) => format!("{} is paused until {}.", rule.id, view::date(until)),
            (false, None) => format!("{} is paused until you resume it.", rule.id),
        }
    });
    Ok(())
}

/// Run `rule` now in this process: a cleanup through the planner (with
/// every safety check and the run lock), any other rule through the
/// engine with Jev and the host's services. A real run is remembered.
pub(crate) fn run_rule(
    layout: &Layout,
    rule: &background::Rule,
    dry_run: bool,
) -> Result<background::Report, String> {
    let store_dir = layout.store.clone();
    let facts = move || coder::task::background_facts(&store_dir);
    let env = background::Env {
        layout,
        facts: Some(&facts),
        volumes: &background::volume::Statvfs,
        processes: &background::inuse::System,
        now: background::paths::now(),
        kache: Some(&KACHE),
    };
    let report = if rule.cleans() {
        run::run(&env, rule, Cause::Manual, dry_run, true)?
    } else {
        let judge = JevJudge::from_env();
        let services = HostServices::at(&layout.store);
        let powers = background::engine::Powers {
            judge: judge
                .as_ref()
                .map(|judge| judge as &dyn background::engine::Judge),
            services: Some(services.as_ref()),
        };
        background::engine::evaluate_with(
            &env,
            rule,
            Cause::Manual,
            &background::engine::Event::default(),
            background::engine::Clock::here(),
            powers,
            dry_run,
        )?
        .unwrap_or_else(background::engine::nothing)
    };
    if !dry_run {
        view::remember(layout, &rule.id, &report);
    }
    Ok(report)
}

/// The lines a dry run of `rule` shows: what it would do now, nothing
/// done.
pub(crate) fn dry_run_lines(
    layout: &Layout,
    rule: &background::Rule,
) -> Result<Vec<String>, String> {
    let report = run_rule(layout, rule, true)?;
    Ok(if rule.cleans() {
        run::describe(&report.plan, &layout.home, true)
    } else {
        step_lines(&report)
    })
}

/// The lines a run of a rule that is not a cleanup shows.
fn step_lines(report: &background::Report) -> Vec<String> {
    let steps = report
        .record
        .as_ref()
        .map_or(&report.steps, |record| &record.steps);
    let mut lines: Vec<String> = steps.iter().map(|step| step.detail.clone()).collect();
    if lines.is_empty() {
        lines.push("Nothing to do now.".into());
    }
    lines
}

fn run_now(output: &Output, layout: &Layout, id: &str, dry_run: bool) -> Result<(), Failure> {
    let rule = store::load(layout, id).map_err(Failure::Refused)?;
    let report = run_rule(layout, &rule, dry_run).map_err(Failure::Refused)?;
    if !rule.cleans() {
        let value = serde_json::to_value(&report).unwrap_or(Value::Null);
        output.emit(&value, |_| step_lines(&report).join("\n"));
        return Ok(());
    }
    let value = serde_json::to_value(&report).unwrap_or(Value::Null);
    output.emit(&value, |_| {
        let mut lines = Vec::new();
        if dry_run {
            lines.push("Dry run: nothing is deleted.".to_owned());
            lines.extend(run::describe(&report.plan, &layout.home, true));
        } else if let Some(record) = &report.record {
            for action in &record.actions {
                lines.push(format!(
                    "  {:?} {} {}  ({})",
                    action.outcome,
                    background::paths::bytes(action.bytes),
                    background::paths::show(&action.path, &layout.home),
                    action.reason
                ));
            }
            lines.push(
                report
                    .notice
                    .clone()
                    .unwrap_or_else(|| "Nothing to clean.".to_owned()),
            );
            lines.push(format!("Run {}.", record.run));
        }
        lines.join("\n")
    });
    Ok(())
}

fn log(output: &Output, layout: &Layout, id: Option<&str>, args: &Args) -> Result<(), Failure> {
    let since = args.option("since").map(time).transpose()?;
    // `--since 1d` means the last day.
    let since = since.map(|at| {
        let now = background::paths::now();
        if at > now { now - (at - now) } else { at }
    });
    let records = view::log(layout, id, since, usize::MAX);
    if args.switch("stats") {
        let stats = view::stats(&records);
        let value = json!({ "stats": stats.iter().map(|(key, bytes)| json!({"bucket": key, "bytes": bytes})).collect::<Vec<_>>() });
        output.emit(&value, |_| {
            if stats.is_empty() {
                return "Nothing freed yet.".into();
            }
            stats
                .iter()
                .map(|(key, bytes)| format!("{key}: {}", background::paths::bytes(*bytes)))
                .collect::<Vec<_>>()
                .join("\n")
        });
        return Ok(());
    }
    output.emit(&json!({ "runs": records }), |_| {
        if records.is_empty() {
            return "No runs yet.".into();
        }
        records
            .iter()
            .map(view::log_line)
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}

fn undo(output: &Output, layout: &Layout, run: &str) -> Result<(), Failure> {
    let restored = run::undo(layout, run).map_err(Failure::Refused)?;
    let value = json!({
        "restored": restored.iter().map(|(path, result)| json!({
            "path": path,
            "ok": result.is_ok(),
            "error": result.as_ref().err(),
        })).collect::<Vec<_>>()
    });
    output.emit(&value, |_| {
        if restored.is_empty() {
            return "That run removed no worktrees.".into();
        }
        restored
            .iter()
            .map(|(path, result)| match result {
                Ok(()) => format!("Restored {}.", path.display()),
                Err(why) => format!("Could not restore {}: {why}", path.display()),
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}

fn proposals(output: &Output, layout: &Layout) -> Result<(), Failure> {
    let all = background::judged::Proposals::load(layout);
    let waiting = all.waiting();
    output.emit(&json!({ "proposals": waiting }), |_| {
        if waiting.is_empty() {
            return "No folders waiting.".into();
        }
        waiting
            .iter()
            .map(|proposal| background::judged::line(proposal))
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}

fn confirm(output: &Output, layout: &Layout, path: &str, yes: bool) -> Result<(), Failure> {
    if yes {
        let rule = background::judged::confirm(layout, path, background::paths::now())
            .map_err(Failure::Refused)?;
        output.emit(&json!({ "rule": rule }), |_| {
            format!("{path} is now cleaned by {} (to the trash first).", rule.id)
        });
    } else {
        background::judged::decline(layout, path).map_err(Failure::Refused)?;
        output.emit(&json!({ "declined": path }), |_| format!("{path} stays."));
    }
    Ok(())
}

/// Judge the largest unknown folders now with Jev; nothing is deleted.
fn judge_now(output: &Output, layout: &Layout) -> Result<(), Failure> {
    let judge = JevJudge::from_env().ok_or_else(|| Failure::Refused(NO_JEV.into()))?;
    let rule = store::load(layout, "disk").map_err(Failure::Refused)?;
    let store_dir = layout.store.clone();
    let facts = move || coder::task::background_facts(&store_dir);
    let env = background::Env {
        layout,
        facts: Some(&facts),
        volumes: &background::volume::Statvfs,
        processes: &background::inuse::System,
        now: background::paths::now(),
        kache: Some(&KACHE),
    };
    let judgments = background::judged::consider(&env, &rule, &judge).map_err(Failure::Refused)?;
    output.emit(&json!({ "judgments": judgments }), |_| {
        if judgments.is_empty() {
            return "No unknown folders to judge.".into();
        }
        judgments
            .iter()
            .map(|j| {
                format!(
                    "{} {} · {} {:.2} · {}",
                    j.path,
                    background::paths::bytes(j.bytes),
                    j.kind.replace('_', " "),
                    j.probability,
                    if j.proposed { "proposed" } else { "kept" }
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}

/// kache's collector as a cleanup run reaches it (the kache class): two
/// attempts 30 seconds apart, so a busy collector holds a run up for at
/// most a minute.
static KACHE: std::sync::LazyLock<background::kache::Kache> =
    std::sync::LazyLock::new(|| background::kache::Kache {
        attempts: 2,
        ..background::kache::Kache::default()
    });

/// Reclaim the kache store through kache's own collector (#10758).
fn kache(output: &Output, status_only: bool) -> Result<(), Failure> {
    use background::kache::{Kache, Lock};
    let bytes = background::paths::bytes;
    let lock_line = |lock: &Lock| match lock {
        Lock::Absent => "no collection has run yet".to_string(),
        Lock::Free {
            last_pid: Some(pid),
            last_alive: false,
        } => format!("no collection running (last collector {pid} has exited)"),
        Lock::Free { .. } => "no collection running".to_string(),
        Lock::Held { pid, .. } => format!(
            "collection running in process {}",
            pid.map_or_else(|| "unknown".into(), |pid| pid.to_string())
        ),
    };
    let disk_line = |disk: &background::kache::Disk| {
        format!(
            "{} of {} cap ({} private, {} shared with target directories)",
            bytes(disk.store_bytes),
            bytes(disk.store_limit_bytes),
            bytes(disk.disk_private_bytes),
            bytes(disk.cloned_into_targets_bytes),
        )
    };
    let kache = Kache::default();
    if status_only {
        let (store_dir, disk, lock) = kache.status().map_err(Failure::Refused)?;
        output.emit(
            &json!({ "store_dir": store_dir, "disk": disk, "lock": lock }),
            |_| format!("kache store: {}\n{}", disk_line(&disk), lock_line(&lock)),
        );
        return Ok(());
    }
    let report = kache.reclaim().map_err(Failure::Refused)?;
    output.emit(&json!(report), |_| {
        let mut lines = vec![
            format!("Before: {}", disk_line(&report.before)),
            format!("After:  {}", disk_line(&report.after)),
        ];
        if report.collected {
            lines.push(format!(
                "Dropped {} entries; {} of store, {} of disk returned.",
                report.entries_dropped,
                bytes(report.store_bytes_removed),
                bytes(report.disk_bytes_reclaimed),
            ));
        } else {
            lines.push(format!(
                "Another collection held the lock through {} tries; it is doing this work.",
                report.skipped
            ));
        }
        if !report.after.under_cap() {
            lines.push(
                "Still over the cap: target directories hold the rest (kache targets).".into(),
            );
        }
        lines.join("\n")
    });
    Ok(())
}

/// Package a rule as a plugin folder `openagents plugin publish` takes.
fn publish(output: &Output, layout: &Layout, id: &str, args: &Args) -> Result<(), Failure> {
    let dir = args.option("out").map_or_else(
        || {
            std::env::current_dir()
                .unwrap_or_default()
                .join(format!("{id}-plugin"))
        },
        PathBuf::from,
    );
    let dir = background::plugins::package(layout, id, &dir).map_err(Failure::Refused)?;
    output.emit(&json!({ "plugin": dir }), |_| {
        format!(
            "{}\nPublish it with: openagents plugin publish {}",
            dir.display(),
            dir.display()
        )
    });
    Ok(())
}

fn home_layout() -> Result<Layout, String> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|home| home.is_absolute())
        .ok_or("HOME must be an absolute path")?;
    Layout::new(&home, None).map_err(|error| error.to_string())
}

/// The rules for the terminal's `/background`.
pub(crate) fn rows() -> Result<Vec<openagents_terminal::BackgroundRow>, String> {
    let layout = home_layout()?;
    Ok(view::list(&layout)
        .into_iter()
        .map(|row| openagents_terminal::BackgroundRow {
            line: terminal_line(&row),
            paused: !row.enabled
                || row
                    .paused_until
                    .is_some_and(|until| until > background::paths::now()),
            id: row.id,
        })
        .collect())
}

/// A rule's row in the watchers view (#10347): its name is the row's
/// label, so the line starts at its state ([`view::Row::line`] already says
/// how long ago its reading and result were).
fn terminal_line(row: &view::Row) -> String {
    let full = row.line();
    full.strip_prefix(&format!("{} · ", row.id))
        .unwrap_or(&full)
        .to_owned()
}

/// What `/background` does to a rule, as the lines of a card.
pub(crate) fn act(
    id: &str,
    act: openagents_terminal::BackgroundAct,
) -> Result<Vec<String>, String> {
    use openagents_terminal::BackgroundAct as Act;
    let layout = home_layout()?;
    match act {
        // The rule in plain words; its full text is `openagents background
        // show` (#10347).
        Act::Show => {
            let rule = store::load(&layout, id)?;
            let mut lines = vec![format!("{} · version {}", rule.name, rule.version)];
            lines.extend(background::compile::describe(&rule));
            lines.push(format!(
                "Its full definition: openagents --json background show {}",
                rule.id
            ));
            Ok(lines)
        }
        Act::DryRun | Act::Run => {
            let rule = store::load(&layout, id)?;
            let dry = act == Act::DryRun;
            let report = run_rule(&layout, &rule, dry)?;
            if !rule.cleans() {
                return Ok(step_lines(&report));
            }
            if dry {
                return Ok(run::describe(&report.plan, &layout.home, false));
            }
            Ok(vec![
                report
                    .notice
                    .unwrap_or_else(|| "Nothing to clean.".to_owned()),
            ])
        }
        Act::Pause | Act::Resume => {
            let rule = view::pause(&layout, id, None, act == Act::Resume)?;
            Ok(vec![if rule.enabled {
                format!("{} is on.", rule.id)
            } else {
                format!("{} is paused until you resume it.", rule.id)
            }])
        }
        Act::Log => {
            let records = view::log(&layout, Some(id), None, 20);
            if records.is_empty() {
                return Ok(vec!["No runs yet.".into()]);
            }
            // Without the run's internal id, which only the CLI's log
            // needs (#10347).
            Ok(records
                .iter()
                .map(|record| view::log_line(record).replacen(&format!(" {}", record.run), "", 1))
                .collect())
        }
    }
}

/// The newest background notification, for the terminal's transcript.
pub(crate) fn notice() -> Option<(u64, String)> {
    let layout = home_layout().ok()?;
    background::store::State::load(&layout)
        .rules
        .values()
        .filter_map(|state| state.notice.clone())
        .max_by_key(|(at, _)| *at)
}

/// The background watchers running on this computer, for the terminal's
/// welcome card.
pub(crate) fn watchers() -> Vec<String> {
    home_layout().map_or_else(|_| Vec::new(), |layout| view::watchers(&layout))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_scheduled_prompt_posts_into_its_chat_through_coder_chat() {
        let command = super::chat_command(
            std::path::Path::new("/usr/local/bin/openagents"),
            std::path::Path::new("/home/me/work/app"),
            "2026-10-10-abc",
        );
        assert_eq!(
            command,
            [
                "/usr/local/bin/openagents",
                "coder",
                "chat",
                "--in",
                "/home/me/work/app",
                "--session",
                "2026-10-10-abc",
                "--stdin",
            ]
        );
    }

    use super::*;

    #[test]
    fn a_list_line_says_what_the_rule_does() {
        let row = view::Row {
            id: "worktrees".into(),
            name: "Stale worktree pruning".into(),
            version: 1,
            digest: String::new(),
            enabled: false,
            paused_until: None,
            state: background::store::RuleState::default(),
            plugin: None,
            error: None,
        };
        assert_eq!(
            list_line(&row),
            "worktrees (Stale worktree pruning) · paused · not run yet"
        );
        let same = view::Row {
            name: "worktrees".into(),
            ..row
        };
        assert_eq!(list_line(&same), "worktrees · paused · not run yet");
    }

    /// The compiler's labeled set (`crates/background/fixtures/compile-v1.json`)
    /// against hosted Jev: run with `BACKGROUND_COMPILE_EVAL=1` and
    /// `TYPESAFE_API_KEY` set, `-- --ignored --nocapture`. Prints each row and
    /// the accuracy of the readings the compiler acts on.
    #[test]
    #[ignore = "asks hosted Jev"]
    fn live_compile_eval() {
        use background::compile::{self, Compiled, Context};
        if std::env::var_os("BACKGROUND_COMPILE_EVAL").is_none() {
            return;
        }
        let judge = JevJudge::from_env().expect("TYPESAFE_API_KEY");
        let set: Value =
            serde_json::from_str(include_str!("../../background/fixtures/compile-v1.json"))
                .unwrap();
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::new(dir.path(), None).unwrap();
        // One rule made in conversation beside the built-in one.
        let mut alert = background::rule::disk();
        alert.id = "low-disk-30gb".into();
        alert.name = "Tell me when free space is below 30 GB".into();
        alert.origin = background::rule::Origin::Conversation {
            thread: "eval".into(),
            message: "tell me when my disk has less than 30 GB free".into(),
        };
        alert.enabled = true;
        alert.triggers = vec![background::rule::Trigger::Interval { every_secs: 300 }];
        alert.conditions = vec![background::rule::Condition::FreeBelow {
            level: background::rule::Level {
                bytes: 30 * background::rule::GB,
                percent: 0,
            },
        }];
        alert.actions = vec![background::rule::Action::Notify {
            text: "Disk space is low: {free} free.".into(),
        }];
        alert.cooldown_secs = 6 * 3600;
        store::save(&layout, &alert).unwrap();
        let context = Context {
            thread: "eval".into(),
            project: Some(dir.path().join("work/openagents")),
            clock: background::engine::Clock::here(),
            new_only: false,
        };
        let (mut rows, mut right, mut kinds) = (0, 0, 0);
        for row in set["rows"].as_array().unwrap() {
            let message = row["message"].as_str().unwrap();
            let rules: Vec<background::Rule> = store::list(&layout)
                .into_iter()
                .filter_map(Result::ok)
                .collect();
            let answers = background::engine::Judge::ask(
                &judge,
                &compile::state(message, &rules),
                &compile::questions(&rules),
            )
            .unwrap_or_else(|why| panic!("{message:?}: {why}"));
            let top = |q: &str| {
                answers
                    .get(q)
                    .and_then(|a| a.top())
                    .map(|(id, p)| (id.to_owned(), p))
            };
            let agrees = |labels: &Value, say: bool| {
                let mut all = true;
                for field in ["intent", "what", "change", "rule"] {
                    if let Some(want) = labels[field].as_str() {
                        let got = top(field);
                        if got.as_ref().map(|(id, _)| id.as_str()) != Some(want) {
                            all = false;
                            if say {
                                println!("  {message:?}: {field} {got:?}, want {want}");
                            }
                        }
                    }
                }
                all
            };
            let ok = agrees(row, false) || (row["also"].is_object() && agrees(&row["also"], false));
            if !ok {
                agrees(row, true);
            }
            let compiled = compile::from_answers(message, &rules, &context, &answers, &layout.home);
            let kind = match &compiled {
                Compiled::Draft(_) => "draft",
                Compiled::Question { .. } => "question",
                Compiled::Unchanged { .. } => "unchanged",
            };
            let expect = row["expect"].as_str().unwrap();
            println!(
                "{} {message:?} -> {kind} (intent {:?})",
                if ok && kind == expect { "ok  " } else { "MISS" },
                top("intent")
            );
            rows += 1;
            right += usize::from(ok);
            kinds += usize::from(kind == expect);
        }
        println!("readings right: {right}/{rows}; compiled kind right: {kinds}/{rows}");
    }

    #[test]
    fn times_read_as_durations_dates_and_seconds() {
        let now = background::paths::now();
        let two_hours = time("2h").unwrap();
        assert!((now + 7200..now + 7300).contains(&two_hours));
        assert_eq!(time("2026-10-02").unwrap(), 1_790_899_200);
        assert_eq!(time("1790899200").unwrap(), 1_790_899_200);
        assert!(time("soon").is_err());
    }
}

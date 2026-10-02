//! `openagents background`: the host's background rules
//! (docs/background/2026-10-02-background-processes.md): the built-in disk
//! cleanup monitor `disk`, and the rules of plugins turned on here
//! (docs/background/2026-10-02-disk-cleanup-plugin.md). Every command reads and writes
//! `~/.openagents/background`; `run` runs in this process under the run
//! lock, so it works with or without a host.

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
  resume ID       Start the rule again.
  run ID [--dry-run]
                  Run the rule now; --dry-run shows exactly what it would
                  delete and why, and what it keeps, changing nothing.
  log [ID] [--since TIME] [--stats]
                  The audit log; --stats totals bytes freed by week and class.
  undo RUN        Recreate the worktrees that run removed.
Every command takes --tasks DIR (the Coder task store, default
~/.openagents/tasks). Rules run in the host on their own: the built-in
disk rule, each rule a plugin brings while the plugin is on here
(openagents plugin enable), and each rule made from words and confirmed.
These commands look at them, change them, or run one now. Words become a
rule through Jev (TYPESAFE_API_KEY or ~/.openagents/jev.json); a rule
only uses the host's own actions (delete build caches and finished
worktrees, notify, fast-forward a clean checkout), never another command.";

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
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("background", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &["dry-run", "stats", "yes"]) {
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
    output.emit(&json!({ "rules": rows }), |_| {
        rows.iter()
            .map(view::Row::line)
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
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
            serde_json::to_string_pretty(&rule).unwrap_or_default()
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

/// Jev through Coder's decision door (`coder::decision::from_env`):
/// `TYPESAFE_API_KEY` or the key in `~/.openagents/jev.json`.
pub(crate) struct JevJudge {
    client: jev::Client,
}

impl JevJudge {
    pub(crate) fn from_env() -> Option<Self> {
        // The same door the rest of Coder decides through: the configured
        // decision profile, `TYPESAFE_API_KEY`, or `~/.openagents/jev.json`.
        let client = coder::decision::from_env().ok().flatten()?;
        Some(Self { client })
    }
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
        let client = self.client.clone();
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

/// What words compiled to, shown: the card and its dry run, or the
/// question, or why nothing changes. `Some(id)` when a draft waits.
pub(crate) fn compile_words(
    layout: &Layout,
    message: &str,
    thread: &str,
    project: Option<PathBuf>,
) -> Result<(Vec<String>, Option<String>, Value), String> {
    use background::compile::{self, Compiled, Context};
    let judge = JevJudge::from_env().ok_or(
        "Jev is not set up here: set TYPESAFE_API_KEY or put the key in ~/.openagents/jev.json.",
    )?;
    let clock = background::engine::Clock::here();
    let context = Context {
        thread: thread.to_owned(),
        project,
        clock,
    };
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
    let (lines, drafted, mut value) =
        compile_words(layout, &text, &id, std::env::current_dir().ok())
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
    )
    .map_err(Failure::Refused)?;
    output.emit(&value, |_| lines.join("\n"));
    Ok(())
}

/// `apply DRAFT`: save what the draft shows.
fn apply(output: &Output, layout: &Layout, id: &str) -> Result<(), Failure> {
    let (draft, saved) = background::compile::apply(layout, id, background::paths::now())
        .map_err(Failure::Refused)?;
    output.emit(&json!({ "draft": draft, "saved": saved }), |_| match &saved {
        Some(rule) => match draft.kind {
            background::compile::Kind::Define => format!(
                "Saved {} ({}). It runs on its own from now on; openagents background list shows it.",
                rule.name, rule.id
            ),
            _ => format!("Saved {} version {}.", rule.name, rule.version),
        },
        None => format!("Removed {}.", draft.rule.name),
    });
    Ok(())
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
    let rule = view::pause(layout, id, until, resume).map_err(Failure::Refused)?;
    output.emit(&json!({ "rule": rule }), |_| match (resume, until) {
        (true, _) => format!("{} is on.", rule.id),
        (false, Some(until)) => format!("{} is paused until {}.", rule.id, view::date(until)),
        (false, None) => format!("{} is paused until you resume it.", rule.id),
    });
    Ok(())
}

fn run_now(output: &Output, layout: &Layout, id: &str, dry_run: bool) -> Result<(), Failure> {
    let rule = store::load(layout, id).map_err(Failure::Refused)?;
    let store_dir = layout.store.clone();
    let facts = move || coder::task::background_facts(&store_dir);
    let env = background::Env {
        layout,
        facts: Some(&facts),
        volumes: &background::volume::Statvfs,
        processes: &background::inuse::System,
        now: background::paths::now(),
    };
    let report = run::run(&env, &rule, Cause::Manual, dry_run, true).map_err(Failure::Refused)?;
    if !dry_run {
        view::remember(layout, id, &report);
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
            line: row.line(),
            paused: !row.enabled
                || row
                    .paused_until
                    .is_some_and(|until| until > background::paths::now()),
            id: row.id,
        })
        .collect())
}

/// What `/background` does to a rule, as the lines of a card.
pub(crate) fn act(
    id: &str,
    act: openagents_terminal::BackgroundAct,
) -> Result<Vec<String>, String> {
    use openagents_terminal::BackgroundAct as Act;
    let layout = home_layout()?;
    match act {
        Act::Show => {
            let rule = store::load(&layout, id)?;
            let mut lines = vec![format!(
                "{} · version {} · {}",
                rule.name,
                rule.version,
                rule.digest()
            )];
            lines.extend(
                serde_json::to_string_pretty(&rule)
                    .unwrap_or_default()
                    .lines()
                    .map(str::to_owned),
            );
            Ok(lines)
        }
        Act::DryRun | Act::Run => {
            let rule = store::load(&layout, id)?;
            let store_dir = layout.store.clone();
            let facts = move || coder::task::background_facts(&store_dir);
            let env = background::Env {
                layout: &layout,
                facts: Some(&facts),
                volumes: &background::volume::Statvfs,
                processes: &background::inuse::System,
                now: background::paths::now(),
            };
            let dry = act == Act::DryRun;
            let report = run::run(&env, &rule, Cause::Manual, dry, true)?;
            if dry {
                return Ok(run::describe(&report.plan, &layout.home, false));
            }
            view::remember(&layout, id, &report);
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
            Ok(records.iter().map(view::log_line).collect())
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
    use super::*;

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

//! `openagents background`: the host's background rules
//! (docs/background/2026-10-02-background-processes.md). Phase 1 has one,
//! the disk cleanup monitor `disk`. Every command reads and writes
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
  edit ID         Edit the rule as JSON in $EDITOR, then show its dry run.
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
~/.openagents/tasks). The disk rule runs in the host on its own; these
commands look at it, change it, or run it now.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("add", Effect::LocalWrite),
    Declared::computer("edit", Effect::LocalWrite),
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
    let args = match Args::parse(rest, &["dry-run", "stats"]) {
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
        "add" => add(output, &layout, &args),
        "edit" => need_id().and_then(|id| edit(output, &layout, &id)),
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

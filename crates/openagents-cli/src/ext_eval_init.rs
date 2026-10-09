//! `openagents plugin test init` (also `openagents ext eval init`): the authoring interview in a terminal.
//!
//! The same typed interview the chat's `eval.author` route runs
//! (`ext_eval::author`, driven by `coder::eval_author`), on the operator's
//! computer: the model door is the operator's own (`CODER_DOOR_*`), every
//! gate is an explicit `y`, and anything else typed at a gate is a change
//! request. The interview reads the extension and never writes into it: a
//! try runs from a temporary copy of the draft, and the only write is the
//! finished test set, at the last step, to the eval directory (or
//! `--out`). It never overwrites a test.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use coder::eval_author::Author;
use coder::generate::{Door, Generate, Message, Role};
use coder::package::Package;
use ext_eval::author::runner::{FULL_RUNS, TRY_RUNS};
use ext_eval::author::{
    Catalog, Event, Interview, Pick, Planned, RunRequest, Runner, Source, Stage, Surface, Tool,
    Turn, files,
};
use nostr::contracts::parse_definition;
use serde_json::{Value, json};

use crate::out::{EXIT_USAGE, Output};

/// The usage line for the interview.
pub const USAGE: &str =
    "usage: openagents plugin test init [<plugin dir>] [--out <dir>] [--eval-dir <dir>]

Writes a test set for the plugin with you, one step at a time: what the
plugin is for, what a good run looks like, the tests, the checks, a one-run
try, and the full run's size. Type y at each step to go on, or type what to
change. The plugin is read, never changed; the finished tests are
written under its eval directory (evals/ by default), or --out.
Uses your model key, or your signed-in Claude Code or Codex when no key
is available.";

/// The package key a local extension's definition names until it is
/// published: no signer yet.
pub const LOCAL_KEY: &str = "0000000000000000000000000000000000000000000000000000000000000000";
/// The most characters of the extension's own words the interview reads.
pub const MAX_WORDS: usize = 6_000;

/// `openagents plugin test init ...`.
pub fn run(output: &Output, words: &[String]) -> u8 {
    let command = "plugin test init";
    let mut root: Option<PathBuf> = None;
    let mut out: Option<PathBuf> = None;
    let mut eval_dir: Option<String> = None;
    let mut rest = words.iter();
    while let Some(word) = rest.next() {
        match word.as_str() {
            "--help" | "-h" => {
                println!("{USAGE}");
                return 0;
            }
            "--out" => match rest.next() {
                Some(dir) => out = Some(PathBuf::from(dir)),
                None => return output.usage(command, "--out needs a directory", USAGE),
            },
            "--eval-dir" => match rest.next() {
                Some(dir) => eval_dir = Some(dir.clone()),
                None => return output.usage(command, "--eval-dir needs a directory", USAGE),
            },
            "--bare" => {
                return output.usage(
                    command,
                    "--bare writes a blank template; it comes with `openagents plugin test` itself (#9934)",
                    USAGE,
                );
            }
            flag if flag.starts_with('-') => {
                return output.usage(command, &format!("unknown flag {flag}"), USAGE);
            }
            path if root.is_none() => root = Some(PathBuf::from(path)),
            extra => return output.usage(command, &format!("unexpected `{extra}`"), USAGE),
        }
    }
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let target = match out {
        Some(out) => out,
        None => match ext_eval::eval_dir(&root, eval_dir.as_deref(), None) {
            Ok(dir) => dir,
            Err(error) => return output.usage(command, &error.to_string(), USAGE),
        },
    };
    let mut bridge = None;
    let door = match Door::from_env() {
        Ok(Door::Stub(_)) => match interview_door() {
            Some(door) => door,
            None => match crate::eval_engine::Bridge::discover() {
                Ok(local) => {
                    let door = Door::Live(coder::generate::ResponsesDoor::new(
                        &local.url,
                        &local.model,
                        &local.key,
                    ));
                    bridge = Some(local);
                    door
                }
                Err(error) => return output.fail(command, &error),
            },
        },
        Ok(door) => door,
        Err(error) => return output.fail(command, &error),
    };
    let name = door.model().to_string();
    let author = Author::new(door, name, None, Catalog::default()).for_person("the operator");
    let stdin = std::io::stdin();
    let result = crate::runtime().block_on(interview(
        &author,
        &root,
        &target,
        &crate::ext_eval::LocalRunner {
            results_base: target.join("results"),
        },
        stdin.lock(),
        std::io::stdout(),
    ));
    drop(bridge);
    match result {
        Ok(written) => {
            if output.json() {
                println!(
                    "{}",
                    json!({"written": written.iter().map(|p| p.display().to_string()).collect::<Vec<_>>()})
                );
            }
            0
        }
        Err(Stop::Usage(message)) => {
            eprintln!("openagents {command}: {message}");
            EXIT_USAGE
        }
        Err(Stop::Failed(message)) => output.fail(command, &message),
    }
}

/// Why the interview stopped without a test set.
#[derive(Debug, PartialEq, Eq)]
pub enum Stop {
    /// The directory isn't an extension.
    Usage(String),
    /// Anything else, in plain words.
    Failed(String),
}

fn program_steps(root: &Path, program: &str) -> Vec<(String, Value)> {
    let Ok(entries) = std::fs::read_dir(root.join("programs")) else {
        return Vec::new();
    };
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    paths.sort();
    for path in paths {
        let Some(value) = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        else {
            continue;
        };
        let claimed = value["definition"]["id"]
            .as_str()
            .and_then(|id| id.rsplit('/').next())
            .or_else(|| value["slug"].as_str());
        if claimed == Some(program) {
            return value["definition"]["steps"]
                .as_array()
                .map(|steps| {
                    steps
                        .iter()
                        .filter_map(|s| Some((s["name"].as_str()?.to_string(), value.clone())))
                        .collect()
                })
                .unwrap_or_default();
        }
    }
    Vec::new()
}

/// Steps 0 and 1's reading: the package record, verified against the files
/// it pins, its program's step names (the operations a trajectory
/// records), and its own words. Reads only.
///
/// # Errors
///
/// Why the directory isn't an extension this host resolves.
pub fn read_extension(root: &Path) -> Result<Tool, String> {
    let record = root.join("package.json");
    let package = Package::load(&record)
        .map_err(|why| format!("{} is not a plugin: {why}", root.display()))?;
    Package::resolve(root, &package)
        .map_err(|refusal| format!("{} does not resolve: {refusal}", root.display()))?;
    let bytes = std::fs::read(&record).map_err(|e| e.to_string())?;
    let program = package
        .program
        .as_ref()
        .map_or(package.slug.as_str(), |program| program.name.as_str());
    let steps = program_steps(root, program);
    let operations: Vec<String> = steps.iter().map(|(name, _)| name.clone()).collect();
    let mut words = String::new();
    if !package.summary.is_empty() {
        words.push_str(&package.summary);
        words.push('\n');
    }
    if let Some((_, program)) = steps.first()
        && let Some(summary) = program["definition"]["summary"].as_str()
    {
        words.push_str(summary);
        words.push('\n');
    }
    if !operations.is_empty() {
        words.push_str(&format!("Its steps: {}.\n", operations.join(", ")));
    }
    for rule in &package.background {
        words.push_str(&format!(
            "It runs in the background when turned on on a computer: rule `{}`.\n",
            rule.name
        ));
    }
    for name in ["README.md", "readme.md"] {
        if let Ok(text) = std::fs::read_to_string(root.join(name)) {
            words.push_str(&text);
            break;
        }
    }
    let words: String = words.chars().take(MAX_WORDS).collect();
    let definition = parse_definition(&json!({
        "id": format!("{LOCAL_KEY}:{}/{program}", package.slug),
        "artifact": {
            "digest": nostr::contracts::digest_bytes(&bytes),
            "size": bytes.len(),
            "media_type": "application/json",
        },
    }))
    .map_err(|e| format!("{}: {e}", record.display()))?;
    let name = if package.name.is_empty() {
        package.slug.clone()
    } else {
        package.name.clone()
    };
    Ok(Tool {
        name,
        summary: package.summary.clone(),
        words,
        source: Source::Existing(definition),
        operations,
    })
}

fn show(out: &mut impl Write, text: &str) {
    let _ = writeln!(out, "\n{text}");
    let _ = out.flush();
}

fn ask_line(input: &mut impl BufRead, out: &mut impl Write) -> Option<String> {
    let _ = write!(out, "> ");
    let _ = out.flush();
    let mut line = String::new();
    match input.read_line(&mut line) {
        Ok(0) | Err(_) => None,
        Ok(_) => Some(line.trim().to_string()),
    }
}

/// Whether a typed answer is the explicit `y`.
fn yes(line: &str) -> bool {
    matches!(line.trim(), "y" | "Y" | "yes")
}

/// Runs the draft once or fully from a temporary copy, never from the
/// extension.
fn try_run(
    runner: &dyn Runner,
    interview: &Interview,
    root: &Path,
    runs: u32,
) -> Result<ext_eval::author::Tried, String> {
    let tool = interview.tool.clone().ok_or("no tool")?;
    let temp = std::env::temp_dir().join(format!(
        "oa-eval-init-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ));
    let evals = temp.join("evals");
    files::write(&evals, &interview.cases).map_err(|e| e.to_string())?;
    let result = runner.run(&RunRequest {
        tool: &tool,
        eval_dir: &evals,
        extension: Some(root),
        runs,
    });
    let _ = std::fs::remove_dir_all(&temp);
    result
}

/// The interview: reads the extension at `root`, asks each step on `out`,
/// reads answers from `input`, and at the end writes the tests to
/// `target`. Returns the files written.
///
/// # Errors
///
/// [`Stop::Usage`] when `root` isn't an extension; [`Stop::Failed`] when
/// the model can't be reached, the answers end early (nothing is written),
/// or the tests can't be written.
pub async fn interview<G: Generate>(
    author: &Author<G>,
    root: &Path,
    target: &Path,
    runner: &dyn Runner,
    mut input: impl BufRead,
    mut out: impl Write,
) -> Result<Vec<PathBuf>, Stop> {
    let tool = read_extension(root).map_err(Stop::Usage)?;
    show(
        &mut out,
        &format!(
            "We read {} at {}. We only read it; we never change it.",
            tool.name,
            root.canonicalize()
                .unwrap_or_else(|_| root.to_path_buf())
                .display()
        ),
    );
    let mut interview = Interview::new(Surface::Terminal, author.catalog().clone());
    // What running this command asks, as the person's first message: a
    // model door refuses a conversation with no message in it.
    let mut transcript: Vec<Message> = vec![Message {
        role: Role::User,
        text: format!("Help me write a test set for my plugin {}.", tool.name),
    }];
    let need = match interview.start(Pick::Existing(tool)) {
        Ok(need) => need,
        Err(turn) => return Err(Stop::Failed(turn.text())),
    };
    let (mut turn, _) = author
        .fulfil(&mut interview, &need, &transcript)
        .await
        .map_err(|e| Stop::Failed(e.to_string()))?;
    let mut tried_this_draft = false;
    loop {
        let text = shown(&turn);
        show(&mut out, &text);
        transcript.push(Message {
            role: Role::Assistant,
            text: text.clone(),
        });
        if interview.stage == Stage::Done {
            break;
        }
        // Offer the try once per version of the tests.
        if interview.stage == Stage::Pilot
            && matches!(turn.offer, Some(Planned::Try(_)))
            && !tried_this_draft
        {
            tried_this_draft = true;
            show(
                &mut out,
                "Try it once now? Type y to run each test one time with and without the plugin, or press Enter to skip.",
            );
            let Some(line) = ask_line(&mut input, &mut out) else {
                return Err(ended());
            };
            if yes(&line) {
                match try_run(runner, &interview, root, TRY_RUNS) {
                    Ok(tried) => {
                        show(&mut out, &tried.headline());
                        let (next, _) = author
                            .advance(&mut interview, Event::Tried(tried), &transcript)
                            .await
                            .map_err(|e| Stop::Failed(e.to_string()))?;
                        turn = next;
                        continue;
                    }
                    Err(why) => show(&mut out, &format!("We couldn't try it: {why}")),
                }
            }
            if let Some(line) = interview.stage.line(Surface::Terminal) {
                show(&mut out, line);
            }
        }
        let Some(line) = ask_line(&mut input, &mut out) else {
            return Err(ended());
        };
        if line.is_empty() {
            turn = interview.again();
            continue;
        }
        transcript.push(Message {
            role: Role::User,
            text: line.clone(),
        });
        let event = if interview.stage.is_gate() {
            if yes(&line) {
                Event::Approve
            } else {
                Event::Change(line)
            }
        } else {
            Event::Answer(line)
        };
        match author.advance(&mut interview, event, &transcript).await {
            Ok((next, _)) => {
                if matches!(next.offer, Some(Planned::Try(_))) {
                    tried_this_draft = false;
                }
                turn = next;
            }
            Err(error) => {
                show(
                    &mut out,
                    &format!("We couldn't do that step ({error}). Try again."),
                );
                turn = interview.again();
            }
        }
    }
    let written = files::write(target, &interview.cases).map_err(|e| {
        Stop::Failed(format!(
            "we couldn't write the tests to {}: {e}; nothing was written into the plugin",
            target.display()
        ))
    })?;
    show(
        &mut out,
        &format!(
            "We wrote {} tests to {}. Run them with `openagents plugin test run {}`.",
            interview.cases.len(),
            target.display(),
            root.display()
        ),
    );
    show(
        &mut out,
        "Run the full test set now? Type y to run it, or press Enter to finish.",
    );
    if let Some(line) = ask_line(&mut input, &mut out)
        && yes(&line)
    {
        match try_run(runner, &interview, root, FULL_RUNS) {
            Ok(full) => {
                show(&mut out, &full.headline());
                let (turn, _) = author
                    .advance(&mut interview, Event::Tried(full), &transcript)
                    .await
                    .map_err(|e| Stop::Failed(e.to_string()))?;
                show(&mut out, &turn.say);
                show(
                    &mut out,
                    "To add it to the Gym, run `openagents plugin test publish` on that report.",
                );
            }
            Err(why) => show(&mut out, &format!("We couldn't run it: {why}")),
        }
    }
    Ok(written)
}

/// A turn as the terminal shows it: the size step also says what the full
/// run costs here.
fn shown(turn: &Turn) -> String {
    let mut text = turn.text();
    if turn.stage == Stage::Size
        && let Some(line) = turn.line
    {
        let head = text.trim_end_matches(line).trim_end().to_string();
        text = format!(
            "{head}\n\nOn this computer each run is one Coder turn through your own door, and each check that asks Jev spends your quota; we can't price that from here.\n\n{line}"
        );
    }
    text
}

fn ended() -> Stop {
    Stop::Failed("the answers ended before the test set was ready; nothing was written".into())
}

/// The interview's model when no door key is set: a key the person stored.
/// The caller falls back to a signed-in coding engine when no key is available.
fn interview_door() -> Option<Door> {
    let theirs = model_access::Access::theirs(model_access::current().keys().clone());
    let model = coder::generate::DEFAULT_MODEL;
    match theirs.chat(model_access::Use::Model(model)) {
        Ok(model_access::Doors::Theirs(doors)) => {
            let first = doors.into_iter().next()?;
            Some(Door::Live(coder::generate::ResponsesDoor::new(
                first.responses_base().to_string(),
                model.to_string(),
                first.key.expose().to_string(),
            )))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;

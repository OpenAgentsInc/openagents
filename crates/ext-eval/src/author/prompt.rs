//! What the model reads: the interview prompt from the specification, the
//! step it is asked for, and the interview's state as data.
//!
//! Both drivers send the same instructions, so the terminal and chat
//! interviews ask the same questions. The model answers with one JSON
//! object ([`super::proposal`]); the machine decides what to keep.

use serde_json::{Value, json};

use crate::grader::{Check, DecisionQuestion, Focus};

use super::catalog::Source;
use super::machine::{Interview, Need};
use super::render;

/// The interview prompt, `docs/extensions/evaluation.md`, *The interview
/// prompt*, with `${person}` and `${tool}` to substitute.
pub const INTERVIEW: &str = r#"# Plugin test-set interview

You are helping ${person} write a test set for the plugin at ${tool}. You
speak as OpenAgents ("we") in plain words: plugin, test, test set, with and
without the plugin.

## Rules

- You may read the plugin. You never edit it.
- One step per turn. At every gate (the plugin's purpose, the test list, the
  checks for each test, the size of the full run) stop and wait for an
  explicit yes. Anything else is a change request: fix and ask again.
- Keep the floor: at least one test where the plugin should stay out of the
  way, at least one check of the outcome per test, three runs per scored
  run, and a run without the plugin to compare against.
- A check describes something we can observe: Coder's last message, the
  files it made, or the steps it took. If it can't be observed, it isn't
  a check.
- Never write a test that tells Coder which plugin to use. A test is a task;
  whether the plugin helps is what we measure.
- You propose; the app shows the draft and the person decides. Never say a
  test ran unless the app gave you its result.

## Steps

0. Confirm which plugin this is, or that we're making one. Stop on errors.
1. Say what the plugin is for, what it does, and what it doesn't.
2. Ask what a good run and a failed run look like.
3. Propose 4 to 6 tests where the plugin should help and 1 or 2 where it
   shouldn't. Wait for yes.
4. Propose the checks for each test. Wait for yes.
5. Offer a one-run try. Read its result with the person and fix the tests.
6. Say how big the full run is. Wait for yes.
7. Finish: the test set is ready to run.
"#;

/// How every answer is written.
const ANSWER: &str = r#"## How to answer

Answer with one JSON object and nothing else. The machine that runs this
interview reads it; the person sees only `say` and the app's cards.

- Write `say` as OpenAgents: "we" and "you". Never write "I", "me", "my",
  or "mine".
- Use plain words: plugin, test, test set, check, with and without the plugin.
- Keep `say` under 600 characters. The app shows the tests and checks on a
  card, so don't list every one in `say`.
- Don't end `say` with a question about approval; the app adds that line.
- Never state a number that isn't in the state you were given."#;

/// A test in the state: its task and each check in a line.
fn case_state(case: &nostr::cj_conversation::DraftCase) -> Value {
    let Ok(parsed) = render::parse(case) else {
        return json!({"test": case.id, "kind": case.kind.word()});
    };
    let checks: Vec<String> = parsed
        .graders
        .iter()
        .map(|grader| {
            let what = match &grader.check {
                Check::Decision {
                    question, focus, ..
                } => {
                    let question = match question {
                        DecisionQuestion::Noul { instructions }
                        | DecisionQuestion::Score { instructions, .. }
                        | DecisionQuestion::Choice { instructions, .. } => instructions,
                    };
                    format!("asks \"{question}\" about {}", focus_words(focus))
                }
                Check::Judge { focus, .. } => format!("judges {}", focus_words(focus)),
                Check::Regex {
                    pattern, target, ..
                } => {
                    format!("looks for /{pattern}/ in {}", focus_words(target))
                }
                Check::FileExists { path, exists } => {
                    if *exists {
                        format!("needs a file matching {path}")
                    } else {
                        format!("needs no file matching {path}")
                    }
                }
                Check::OperationUsed {
                    operation,
                    min,
                    max,
                    ..
                } => match max {
                    Some(0) => format!("{operation} never runs"),
                    Some(max) => format!("{operation} runs {min} to {max} times"),
                    None => format!("{operation} runs at least {min} time(s)"),
                },
                Check::OperationOrder { before, after } => format!("{before} runs before {after}"),
                Check::Receipt { operation } => format!("{operation} replays exactly"),
                Check::Command {
                    command, exit_code, ..
                } => format!("runs `{command}` in the folder and needs exit code {exit_code}"),
            };
            format!("{}: {what}", grader.name)
        })
        .collect();
    let mut value = json!({
        "test": case.id,
        "kind": case.kind.word(),
        "task": parsed.prompt,
        "checks": checks,
    });
    if let Some(workspace) = &parsed.workspace {
        value["workspace"] = json!(workspace.template);
    }
    value
}

fn focus_words(focus: &Focus) -> String {
    match focus {
        Focus::LastMessage => "Coder's last message".into(),
        Focus::Trajectory => "the steps Coder took".into(),
        Focus::Files => "the files Coder made".into(),
        Focus::File(path) => format!("the file {path}"),
        Focus::Changed => "the files Coder changed".into(),
        Focus::Diff => "the diff of Coder's changes".into(),
    }
}

/// The interview's state as the model reads it.
#[must_use]
pub fn state(interview: &Interview) -> Value {
    let tool = interview.tool.as_ref().map(|tool| {
        let mut value = json!({
            "name": tool.name,
            "summary": tool.summary,
            "its_own_words": tool.words,
            "operations": tool.operations,
            "made_in_chat": tool.is_made(),
        });
        if let Source::Made { uses, .. } = &tool.source {
            value["turns_on"] = uses
                .iter()
                .filter_map(|id| interview.catalog.by_id(id))
                .map(|t| t.name.clone())
                .collect();
        }
        value
    });
    let forbidden = interview
        .tool
        .as_ref()
        .map(|tool| tool.forbidden_names(&interview.catalog))
        .unwrap_or_default();
    json!({
        "step": interview.stage.number(),
        "tool": tool,
        "catalog": interview.catalog.tools.iter().map(|t| json!({
            "name": t.name,
            "summary": t.summary,
        })).collect::<Vec<_>>(),
        "what_good_and_failed_runs_look_like": interview.quality,
        "tests": interview.cases.iter().map(case_state).collect::<Vec<_>>(),
        "most_tests": interview.max_cases,
        "tasks_must_not_name": forbidden,
        "result": interview.tried.as_ref().map(super::runner::Tried::state),
    })
}

/// The step the model is asked for, and the JSON it answers with.
#[must_use]
pub fn task(interview: &Interview, need: &Need) -> String {
    let change = |change: &Option<String>| match change {
        Some(change) => format!(
            "\n\nThe person asked for a change: \"{}\". Do what they asked, and keep everything else.",
            change.trim()
        ),
        None => String::new(),
    };
    match need {
        Need::Nothing => String::new(),
        Need::Tool { change: asked } if interview.making => format!(
            "## This turn: step 1, the plugin we make\n\n\
             We are making a plugin with the person. A plugin made in chat is a skill: \
             plain-language guidance Coder follows, which may also turn on plugins from \
             `catalog` in the state. It can't contain new code.{}\n\n\
             If you know enough to propose it, answer:\n\
             {{\"say\": \"Here's what we'd make: ... (two or three sentences: what it's for, what it does, what it leaves to the person)\", \
             \"name\": \"a name of two to four words\", \"summary\": \"one sentence\", \
             \"skill\": \"the guidance Coder follows, as short direct instructions, at most 1500 characters\", \
             \"uses\": [\"exact names of catalog plugins it turns on, or none\"]}}\n\n\
             If you don't know what the plugin should do, ask one short question instead:\n\
             {{\"say\": \"the question\", \"asking\": true}}",
            change(asked)
        ),
        Need::Tool { change: asked } => format!(
            "## This turn: step 1, the plugin\n\n\
             Say what the plugin in the state (`tool`) is for, what it does on its own, and what it \
             leaves to the person, in two to four plain sentences, from its own words.{}\n\n\
             Answer: {{\"say\": \"...\"}}",
            change(asked)
        ),
        Need::Tests { change: asked } => format!(
            "## This turn: step 3, the tests\n\n\
             Propose 4 to 6 tests where the plugin should help (`should-fire`) and 1 or 2 where \
             it should stay out of the way (`should-not-fire`), at most `most_tests` in all, \
             following what the person said a good and a failed run look like.\n\n\
             - Each test is a task a person would give Coder, in their words. Name each test \
             for a real task shape.\n\
             - When the plugin's purpose is to make or change files (code, docs, config), give \
             each test where it should help a `workspace`: the folder the run starts in, one of \
             `empty`, `rust-crate`, `python-package`, or `node-package` (a small project whose \
             test passes). In that folder Coder writes files and may run commands, and the \
             checks read the files afterwards; the task asks for the change, as a person would.\n\
             - A test without a `workspace` starts in an empty folder and is checked on Coder's \
             reply. A task that needs code must include it (paste a short file into the task). \
             Coder may not run commands there, so never ask for shell commands.\n\
             - A task never names the plugin, its operations, or anything in \
             `tasks_must_not_name`. A test is a task; whether the plugin helps is what we measure.\n\
             - A test where the plugin should stay out of the way is an ordinary task the plugin \
             can't help with.{}\n\n\
             Answer: {{\"say\": \"one or two sentences about the tests\", \"tests\": \
             [{{\"id\": \"kebab-case-name\", \"kind\": \"should-fire\", \"task\": \"...\", \
             \"good\": \"one sentence: what a good outcome looks like\", \
             \"workspace\": \"rust-crate, only for a test that changes files\"}}]}}",
            change(asked)
        ),
        Need::Checks { change: asked } => format!(
            "## This turn: step 4, the checks\n\n\
             Propose the checks for every test in the state. Every test needs at least one \
             check of its outcome: Coder's last message, the files it made, or one file's \
             contents.\n\n\
             - Prefer a `decision` check: a yes-or-no question about the outcome that someone \
             could answer by reading it, with an optional `rubric` saying what a good outcome \
             looks like.\n\
             - On tests where the plugin should help, add an `operation_used` check that the \
             plugin ran (its operations are in the state; `min` 1). On tests where it should \
             stay out of the way, add one with `max` 0.\n\
             - Use `regex` (`pattern`, `match`: `contains` or `not_contains`) or `file_exists` \
             (`path`, a glob) only for short, exact things.\n\
             - `focus` is `last_message` (the default), `files`, or {{\"file\": \"path\"}}.\n\
             - A test with a `workspace` is checked on its files, not Coder's reply: \
             `file_exists` for a file it should make, `regex` with `target` {{\"file\": \"path\"}} \
             for what a file should hold, a `decision` with `focus` `diff` (what Coder changed) \
             or a file, and a `command` check (`command`, `exit_code`, default 0) that runs in \
             the folder afterwards, such as `cargo test` in a `rust-crate` folder. `command`, \
             `diff`, and `changed` (the list of changed paths) work only in a test with a \
             `workspace`.{}\n\n\
             Answer: {{\"say\": \"how we'd check the tests, one plain line per kind of check\", \
             \"checks\": [{{\"test\": \"<test id>\", \"graders\": [{{\"type\": \"decision\", \
             \"name\": \"short-name\", \"question\": \"...?\", \"focus\": \"last_message\", \
             \"rubric\": \"...\"}}, {{\"type\": \"operation_used\", \"name\": \"used-tool\", \
             \"operation\": \"...\", \"min\": 1}}]}}]}}",
            change(asked)
        ),
        Need::Read => "## This turn: step 5, reading the try\n\n\
             The app gave us the result of one try: `result` in the state. Read it with the \
             person in two to four sentences: how many tests passed with and without the plugin, \
             and what to fix. A test that passes without the plugin as well as with it may be too \
             easy; a check that fails on a good answer may be the wrong check. Use only the \
             numbers in `result`. Suggest at most one fix and say we can make it.\n\n\
             Answer: {\"say\": \"...\"}"
            .into(),
        Need::Fix { change: asked } => format!(
            "## This turn: fixing the tests\n\n\
             The person asked for a change: \"{}\". Change the tests or the checks to do what \
             they asked, keeping the rules for tests and checks.\n\n\
             Answer: {{\"say\": \"what we changed, in one or two sentences\", \
             \"tests\": [the whole test list, as in step 3, only if tests change], \
             \"checks\": [checks as in step 4, only for tests whose checks change]}}",
            asked.trim()
        ),
        Need::Say { message } => format!(
            "## This turn: the test set is ready\n\n\
             Answer the person's message, \"{}\", in one to three sentences. We can't run \
             anything ourselves; the app's buttons do.\n\n\
             Answer: {{\"say\": \"...\"}}",
            message.trim()
        ),
    }
}

/// The whole instructions for one model call.
#[must_use]
pub fn instructions(interview: &Interview, need: &Need, person: &str) -> String {
    let tool = interview.tool.as_ref().map_or_else(
        || "a plugin we make together".to_string(),
        |t| t.name.clone(),
    );
    let head = INTERVIEW
        .replace("${person}", person)
        .replace("${tool}", &tool);
    format!(
        "{head}\n{ANSWER}\n\n{}\n\n## JSON schema\n{}\n\n## State\n\n{}",
        task(interview, need),
        super::proposal::schema(need),
        serde_json::to_string_pretty(&state(interview)).unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The prompt is the specification's, word for word.
    #[test]
    fn the_interview_prompt_is_the_specs() {
        let spec = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/extensions/evaluation.md"),
        )
        .unwrap();
        let start = spec.find("# Plugin test-set interview").unwrap();
        let end = start + spec[start..].find("```").unwrap();
        assert_eq!(&spec[start..end], INTERVIEW);
    }
}

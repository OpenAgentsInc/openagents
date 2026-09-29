//! The structured wording of the interview's start: which tool the person
//! means, and whether a tool they want made is a skill or needs new code.
//!
//! TypeSafe's System One models read JSON structure in a question's
//! instructions and in each option's criterion
//! (<https://docs.typesafe.ai/primitives/advanced.md>), the way the chat
//! router asks (`crate::router::rubric`): the instructions are
//! `{question, context, focus}`, and each option is `{what, not_for,
//! examples}`. The examples are criteria Jev reads, never strings code
//! matches, and none of them is a request the live check measures
//! ([`LIVE_PICKS`]).
//!
//! What "make a tool" means in chat in v1 (`docs/extensions/evaluation.md`):
//! a skill, plain-language guidance Coder follows, that may turn on catalog
//! tools. Only a tool that needs new code goes to Coder on a computer.

use serde_json::{Value, json};

use crate::router::rubric::option;

/// Who is asking, shared by the start's questions.
pub const CONTEXT: &str = "We are OpenAgents, an assistant in a chat app. We help the person \
make a tool for Coder, our coding agent, or write a test set for one. Coder already reads and \
edits files, searches code, runs the project's commands and tests, uses git, and writes any \
text (code, messages, notes, reviews). A tool made in chat is a skill: plain-language \
instructions Coder follows, which may also turn on catalog tools. A tool that needs new code \
is built with Coder on the person's computer instead.";

/// The `tool` question's instructions.
#[must_use]
pub fn tool_instructions() -> Value {
    json!({
        "question": "Which tool does the person want to write tests for, or do they want to make a new one?",
        "context": CONTEXT,
        "focus": "Read the latest message; earlier turns only resolve what it refers to. A \
                  catalog tool counts only when the person names it or plainly describes it.",
    })
}

/// A catalog tool's rubric.
#[must_use]
pub fn catalog_tool(name: &str, summary: &str) -> Value {
    option(
        &format!("The catalog tool {name}: {summary}"),
        Some("A new tool the person describes that only resembles this one (make)"),
        &[],
    )
}

/// The `make` option's rubric.
#[must_use]
pub fn make() -> Value {
    option(
        "They want to make a new tool, or describe what a tool they want should help Coder \
         do, and it isn't one of the catalog tools",
        Some(
            "Writing tests for a catalog tool they name (that tool); a message that says \
             neither which tool nor what a new one should do (unclear)",
        ),
        &[
            "make a tool that summarizes pull requests",
            "let's make a tool for reading stack traces",
            "help me turn my prompt into a tool coder can use",
        ],
    )
}

/// The `unclear` option's rubric.
#[must_use]
pub fn unclear() -> Value {
    option(
        "They haven't said which tool, or what a new tool should do, and it can't be told \
         from what they wrote",
        Some("A new tool described by what it should help Coder do, however briefly (make)"),
        &["I want to test something", "write tests for my tool"],
    )
}

/// The `build` question's instructions.
#[must_use]
pub fn build_instructions() -> Value {
    json!({
        "question": "If the person wants a new tool, is it a skill we can write in chat, or does it need new code?",
        "context": CONTEXT,
        "focus": "Ask whether Coder, told in plain words what to do and how, could do the job \
                  with what it already has. Producing text, reviewing, checking, explaining, \
                  or following a team's conventions is a skill even when the output is code \
                  or a file. It needs new code only when Coder would have to reach a service \
                  outside the repository, run on its own without a person asking, or become a \
                  new program or plugin.",
    })
}

/// The `skill` option's rubric.
#[must_use]
pub fn skill() -> Value {
    option(
        "A skill: guidance that tells Coder how to do a kind of task with what it already \
         has (reading and editing files, searching code, running the project's commands, \
         git, writing text), maybe with catalog tools turned on. Writing, formatting, \
         reviewing, checking, summarizing, explaining, or following house rules and \
         conventions",
        Some(
            "Sending to or reading from an outside service such as Slack, email, an issue \
             tracker, a dashboard, or a database; running on a schedule or in the background; \
             a new program, plugin, or command (code)",
        ),
        &[
            "make a tool that summarizes pull requests",
            "a tool so Coder follows our naming conventions",
            "a tool that checks a diff for secrets before a commit",
            "a tool that explains failing CI runs from the log",
        ],
    )
}

/// The `code` option's rubric.
#[must_use]
pub fn code() -> Value {
    option(
        "New code: a program, plugin, or script that must be built and shipped with the \
         tool, a connection to a service outside the repository (posting, fetching, or \
         syncing with it), or something that runs on its own on a schedule or in the \
         background",
        Some(
            "Guidance for a task Coder can already do by reading, editing, running the \
             project's commands, and writing text, even when what it writes is code or a \
             file (skill)",
        ),
        &[
            "make a tool that posts my test results to Slack",
            "a tool that files Jira tickets for failing tests",
            "a tool that queries our production database for slow queries",
            "a tool that runs every night and opens a PR with dependency bumps",
        ],
    )
}

/// The live check's "make a tool" requests (`tests/eval_author_live.rs`)
/// and what each one is: `skill` stays in the interview and reaches a
/// draft, `code` goes to Coder on a computer. The first two are the
/// requests from #9945. None is a rubric example.
pub const LIVE_PICKS: [(&str, &str); 11] = [
    ("Help me make a tool that writes changelog entries", "skill"),
    (
        "Help me make a tool that tells Coder how we write commit messages",
        "skill",
    ),
    (
        "make a tool that reviews my Dockerfiles for bad practices",
        "skill",
    ),
    (
        "I want a tool that makes Coder write a failing test before it fixes a bug",
        "skill",
    ),
    (
        "help me make a tool that writes docstrings for new functions",
        "skill",
    ),
    (
        "can we make a tool that explains compiler errors in plain English",
        "skill",
    ),
    (
        "make a tool that sends me a Telegram message when Coder finishes a task",
        "code",
    ),
    (
        "Help me make a tool that pulls my open tickets from Linear",
        "code",
    ),
    (
        "a tool that watches my repo and runs the linter every hour",
        "code",
    ),
    (
        "make a tool that reads error rates from our Grafana dashboard",
        "code",
    ),
    (
        "build a tool that uploads build artifacts to our S3 bucket",
        "code",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_option_is_a_rubric_and_no_example_is_a_live_request() {
        let options = [make(), unclear(), skill(), code(), catalog_tool("A", "B")];
        let mut examples = Vec::new();
        for rubric in &options {
            let object = rubric.as_object().expect("an object");
            assert!(object["what"].as_str().is_some_and(|w| !w.is_empty()));
            assert!(object["not_for"].as_str().is_some_and(|w| !w.is_empty()));
            if let Some(list) = object.get("examples") {
                examples.extend(
                    list.as_array()
                        .unwrap()
                        .iter()
                        .map(|e| e.as_str().unwrap().to_lowercase()),
                );
            }
        }
        for instructions in [tool_instructions(), build_instructions()] {
            for key in ["question", "context", "focus"] {
                assert!(instructions[key].as_str().is_some(), "{key}");
            }
        }
        for (request, _) in LIVE_PICKS {
            assert!(
                !examples.contains(&request.to_lowercase()),
                "{request} is a rubric example"
            );
        }
        assert!(LIVE_PICKS.iter().filter(|(_, b)| *b == "skill").count() >= 5);
        assert!(LIVE_PICKS.iter().filter(|(_, b)| *b == "code").count() >= 5);
    }
}

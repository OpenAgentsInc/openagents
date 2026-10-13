//! Second look's A/B (#11540): replays scripted chat turns through the real
//! hook (the Decisions API for the size/risk line, the real low-cost
//! reviewer) in throwaway repositories, half with a planted bug, and
//! prints what it decided and found for each.
//!
//! `cargo run -p coder-new --example second_look_ab [--haiku] [case ...]`
//!
//! Run it with an isolated `HOME` (and `CODER_DECISIONS_DIR`) so nothing
//! of the owner's settings or logs is touched; the gateway reviewer needs
//! that home's Coder sign-in, else Claude Code's Haiku answers.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use coder_new::second_look;
use coder_sdk::{Budget, Call, Event, HookBackend, Hooks, Registry, Stop};
use serde_json::json;

struct Case {
    name: &'static str,
    request: &'static str,
    file: &'static str,
    before: &'static str,
    after: &'static str,
    test: &'static str,
    bug: bool,
}

const CASES: &[Case] = &[
    Case {
        name: "split-buggy",
        request: "Add split_cents(total, people) to bill.py: split a bill given in cents among people so every part is whole cents and the parts add up exactly to the total. Add a test.",
        file: "bill.py",
        before: "",
        after: "def split_cents(total, people):\n    \"\"\"Split total cents among people.\"\"\"\n    share = total // people\n    return [share] * people\n",
        test: "from bill import split_cents\n\ndef test_even():\n    assert split_cents(300, 3) == [100, 100, 100]\n",
        bug: true,
    },
    Case {
        name: "split-clean",
        request: "Add split_cents(total, people) to bill.py: split a bill given in cents among people so every part is whole cents and the parts add up exactly to the total. Add a test.",
        file: "bill.py",
        before: "",
        after: "def split_cents(total, people):\n    \"\"\"Split total cents among people; the first parts take the remainder.\"\"\"\n    share, extra = divmod(total, people)\n    return [share + 1 if i < extra else share for i in range(people)]\n",
        test: "from bill import split_cents\n\ndef test_even():\n    assert split_cents(300, 3) == [100, 100, 100]\n\ndef test_remainder():\n    assert split_cents(100, 3) == [34, 33, 33]\n    assert sum(split_cents(1001, 7)) == 1001\n",
        bug: false,
    },
    Case {
        name: "page-buggy",
        request: "Add page(items, number, size) to pages.py returning the items on page `number`, where pages are numbered from 1. Add a test.",
        file: "pages.py",
        before: "",
        after: "def page(items, number, size):\n    start = number * size\n    return items[start:start + size]\n",
        test: "from pages import page\n\ndef test_size():\n    assert len(page(list(range(10)), 1, 3)) == 3\n",
        bug: true,
    },
    Case {
        name: "page-clean",
        request: "Add page(items, number, size) to pages.py returning the items on page `number`, where pages are numbered from 1. Add a test.",
        file: "pages.py",
        before: "",
        after: "def page(items, number, size):\n    if number < 1:\n        raise ValueError(\"pages start at 1\")\n    start = (number - 1) * size\n    return items[start:start + size]\n",
        test: "from pages import page\n\ndef test_first_page():\n    assert page(list(range(10)), 1, 3) == [0, 1, 2]\n\ndef test_last_page():\n    assert page(list(range(10)), 4, 3) == [9]\n",
        bug: false,
    },
    Case {
        name: "token-buggy",
        request: "In auth.py, make check_token(given, expected) reject an empty or missing token and compare tokens in constant time. Add a test.",
        file: "auth.py",
        before: "def check_token(given, expected):\n    return given == expected\n",
        after: "import hmac\n\ndef check_token(given, expected):\n    if given is None:\n        return False\n    return hmac.compare_digest(given, expected)\n",
        test: "from auth import check_token\n\ndef test_match():\n    assert check_token(\"abc\", \"abc\")\n    assert not check_token(None, \"abc\")\n",
        bug: true,
    },
    Case {
        name: "token-clean",
        request: "In auth.py, make check_token(given, expected) reject an empty or missing token and compare tokens in constant time. Add a test.",
        file: "auth.py",
        before: "def check_token(given, expected):\n    return given == expected\n",
        after: "import hmac\n\ndef check_token(given, expected):\n    if not given or not expected:\n        return False\n    return hmac.compare_digest(given.encode(), expected.encode())\n",
        test: "from auth import check_token\n\ndef test_match():\n    assert check_token(\"abc\", \"abc\")\n\ndef test_empty():\n    assert not check_token(None, \"abc\")\n    assert not check_token(\"\", \"\")\n",
        bug: false,
    },
    Case {
        name: "typo-clean",
        request: "Fix the typo in the README's first line.",
        file: "README.md",
        before: "# Smal tools\n\nHelpers for bills, pages and tokens.\n",
        after: "# Small tools\n\nHelpers for bills, pages and tokens.\n",
        test: "",
        bug: false,
    },
];

/// A decider that always says the change is worth a look.
struct Always;

impl coder_sdk::decide::Decider for Always {
    fn decide(
        &self,
        _request: jev::decisions::DecisionsRequest,
    ) -> futures::future::BoxFuture<'static, Result<jev::decisions::DecisionsResponse, String>>
    {
        use jev::decisions::{ChoiceProbability, DecisionAnswer, DecisionsResponse, Served};
        Box::pin(async {
            Ok(DecisionsResponse {
                id: "always".into(),
                object: "decision".into(),
                model: "always".into(),
                answers: vec![DecisionAnswer::Choice {
                    name: second_look::QUESTION.into(),
                    choice: "check".into(),
                    probabilities: vec![ChoiceProbability {
                        value: "check".into(),
                        probability: 1.0,
                    }],
                    confidence: 1.0,
                }],
                usage: jev::decisions::DecisionUsage::default(),
                metadata: None,
                openagents: Served {
                    door: None,
                    latency_ms: 0,
                    receipt: None,
                },
            })
        })
    }
}

fn git(dir: &Path, args: &[&str]) {
    let ok = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=ab",
            "-c",
            "user.email=ab@example.com",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .is_ok_and(|output| output.status.success());
    assert!(ok, "git {args:?}");
}

#[tokio::main]
async fn main() {
    let mut only: Vec<String> = std::env::args().skip(1).collect();
    // `--haiku`: the reviewer is Claude Haiku alone, not the gateway first.
    let haiku = only.iter().any(|arg| arg == "--haiku");
    // `--always`: every change is reviewed, to measure the reviewer alone.
    let always = only.iter().any(|arg| arg == "--always");
    only.retain(|arg| arg != "--haiku" && arg != "--always");
    let mut caught = 0;
    let mut bugs = 0;
    let mut false_sends = 0;
    let mut clean = 0;
    for case in CASES
        .iter()
        .filter(|case| only.is_empty() || only.iter().any(|name| name == case.name))
    {
        let repo = tempfile::tempdir().expect("a folder");
        let root = repo.path();
        git(root, &["init", "-q"]);
        std::fs::write(root.join(".gitignore"), "__pycache__/\n").unwrap();
        if !case.before.is_empty() {
            std::fs::write(root.join(case.file), case.before).unwrap();
        }
        git(root, &["add", "-A"]);
        git(root, &["commit", "-qm", "start", "--allow-empty"]);
        let lines: Arc<Mutex<Vec<hook_log::Decision>>> = Arc::default();
        let kept = lines.clone();
        let recorder = coder_sdk::decision::default_recorder();
        let mut registry = Registry::new();
        for hook in second_look::hooks(
            root.to_owned(),
            if always {
                Arc::new(Always) as Arc<dyn coder_sdk::decide::Decider>
            } else {
                Arc::new(coder_sdk::decide::Api)
            },
            if haiku {
                second_look::haiku_reviewer()
            } else {
                second_look::default_reviewer()
            },
            Arc::new(move |line: &hook_log::Decision| {
                recorder(line);
                kept.lock().unwrap().push(line.clone());
            }),
            second_look::default_outcomes(),
            false,
        ) {
            registry.register(hook).unwrap();
        }
        let hooks = Hooks::new(registry, Budget::default(), HookBackend::Scripted).unwrap();
        hooks
            .fire(Call::new(
                Event::UserPromptSubmit,
                json!({"prompt": case.request}),
            ))
            .await;
        // The scripted worker's change.
        std::fs::write(root.join(case.file), case.after).unwrap();
        if !case.test.is_empty() {
            std::fs::write(root.join(format!("test_{}", case.file)), case.test).unwrap();
            let ran = std::process::Command::new("python3")
                .current_dir(root)
                .args(["-m", "pytest", "-q"])
                .output();
            let (exit, out) = ran.map_or((127, String::new()), |output| {
                (
                    output.status.code().unwrap_or(1),
                    String::from_utf8_lossy(&output.stdout).into_owned(),
                )
            });
            hooks
                .fire(Call::tool(
                    Event::PostToolUse,
                    "Bash",
                    json!({"command": "python3 -m pytest -q", "tool_response": {"stdout": out, "exit_code": exit}}),
                ))
                .await;
        }
        let started = Instant::now();
        let stopped = hooks
            .fire(Call::new(
                Event::Stop,
                json!({"last_assistant_message": "Done.", "stop_hook_active": false}),
            ))
            .await;
        let seconds = started.elapsed().as_secs_f64();
        let sent = matches!(stopped.stop, Stop::Block { .. });
        if case.bug {
            bugs += 1;
            caught += usize::from(sent);
        } else {
            clean += 1;
            false_sends += usize::from(sent);
        }
        let lines = lines.lock().unwrap();
        let look = lines
            .iter()
            .find(|line| line.question == second_look::QUESTION);
        let verdict = lines.iter().find(|line| line.question == "verdict");
        println!(
            "{:<12} bug={:<5} look_p={:<6} decision={:<12} verdict={:<10} reviewer={:<40} {:.1}s",
            case.name,
            case.bug,
            look.and_then(|line| line.p)
                .map_or("-".into(), |p| format!("{p:.2}")),
            look.map_or("-", |line| line.action.as_str()),
            verdict.map_or("-", |line| line.action.as_str()),
            verdict
                .and_then(|line| line.model.clone())
                .unwrap_or_else(|| "-".into()),
            seconds,
        );
        if let Stop::Block { reason } = &stopped.stop {
            println!("    sent back: {}", reason.replace('\n', "\n    "));
        }
        if let Some(line) = lines.iter().find(|line| line.action == "fallback") {
            println!("    decision failed: {}", line.detail["error"]);
        }
        if let Some(line) = verdict.filter(|line| line.action == "failed") {
            println!("    review failed: {}", line.detail["error"]);
        }
    }
    println!(
        "planted bugs sent back: {caught}/{bugs}; correct changes sent back: {false_sends}/{clean}"
    );
}

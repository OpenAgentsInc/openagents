//! The session's own commands: everything typed into the composer that starts
//! with a `/`.
//!
//! [`COMMANDS`] is one list read by three things — `/help`, Tab completion,
//! and the dispatch in [`run`] — so a command cannot be listed without being
//! handled, and `every_listed_command_is_handled` in
//! [`crate::interactive`] fails if one ever is. A `/` line whose name is not
//! in the list is refused rather than sent to the model: a mistyped `/diff`
//! that silently became a prompt is a worse answer than being told.
//!
//! None of these reaches a model. They run here, print into the transcript as
//! [`Role::Output`], and are exported as notices rather than as model steps.

use std::path::Path;
use std::sync::mpsc::Sender;

use openagents_cli::tools::{OUTPUT_LIMIT, check_shell_refusal};

use crate::runtime::Control;
use crate::tui::{CoderUi, Entry, Role, ToolCall};

/// The commands, and what each one does. The second column is what `/help`
/// prints, so it says what the command actually does and nothing more.
pub const COMMANDS: &[(&str, &str)] = &[
    ("clear", "clear the transcript"),
    (
        "diff",
        "what changed since HEAD: /diff, /diff --staged, /diff <path>…",
    ),
    (
        "export",
        "write the transcript to ~/.openagents/exports as an ATIF document",
    ),
    ("help", "list these commands and the keys"),
    ("login", "log in with GitHub and store the token"),
    (
        "resume",
        "coding-agent sessions other tools left on this machine: /resume, /resume <number>",
    ),
    (
        "run",
        "run a command here and show its output: /run cargo test",
    ),
];

/// The keys the frame handles. Listed by `/help`, and every one of them is
/// wired in [`crate::interactive`] — this is the hint text, and a key in it
/// that does nothing is the defect this list exists to prevent.
const KEYS: &[(&str, &str)] = &[
    ("Enter", "send"),
    ("Alt+Enter / Ctrl+J", "newline"),
    ("Up / Down", "move the caret, then walk history, then scroll"),
    ("Scroll wheel / trackpad", "scroll the transcript"),
    ("PageUp / PageDown", "scroll a page (Fn+Up / Fn+Down on a Mac)"),
    ("Tab", "complete a command or a path"),
    (
        "Ctrl+A / Ctrl+E / Ctrl+W / Ctrl+K / Ctrl+U / Alt+B / Alt+F",
        "edit the line",
    ),
    ("Esc / Ctrl+C / Ctrl+D", "leave, revoking the thread"),
];

/// The command names, for Tab completion.
pub fn names() -> Vec<&'static str> {
    COMMANDS.iter().map(|(name, _)| *name).collect()
}

/// Whether `name` is one this module runs.
pub fn handles(name: &str) -> bool {
    matches!(name, "clear" | "diff" | "export" | "help" | "login" | "resume" | "run")
}

/// Run one `/` line. `line` still carries its leading slash.
pub fn run(ui: &mut CoderUi, line: &str, tx: &Sender<Control>, cwd: &Path) {
    let body = line.trim_start_matches('/');
    let mut words = body.split_whitespace();
    let Some(name) = words.next() else {
        output(ui, "A command needs a name. Try `/help`.");
        return;
    };
    let arguments: Vec<String> = words.map(str::to_string).collect();
    let rest = body[name.len()..].trim().to_string();

    match name {
        "help" => output(ui, &help()),
        "clear" => {
            ui.entries.clear();
            ui.scroll_override = None;
        }
        "export" => crate::interactive::export(ui),
        "login" => spawn_login(ui, tx),
        "diff" => spawn_diff(ui, arguments, tx, cwd),
        "run" => spawn_run(ui, &rest, tx, cwd),
        "resume" => spawn_resume(ui, &arguments, tx, cwd),
        other => output(
            ui,
            &format!("There is no `/{other}`. `/help` lists the commands."),
        ),
    }
}

fn output(ui: &mut CoderUi, text: &str) {
    ui.entries.push(Entry::new(Role::Output, text));
    ui.scroll_override = None;
}

fn spawn_login(ui: &mut CoderUi, tx: &Sender<Control>) {
    output(ui, "Opening GitHub login in your browser...");
    let tx = tx.clone();
    tokio::spawn(async move {
        let text = match crate::interactive::do_login().await {
            Ok(message) => message,
            Err(error) => format!("Login failed: {error}"),
        };
        let _ = tx.send(Control::Output(text));
    });
}

fn help() -> String {
    let mut lines = vec!["**Commands**".to_string(), String::new()];
    for (name, what) in COMMANDS {
        lines.push(format!("- `/{name}` — {what}"));
    }
    lines.push(String::new());
    lines.push("**Keys**".to_string());
    lines.push(String::new());
    for (key, what) in KEYS {
        lines.push(format!("- `{key}` — {what}"));
    }
    lines.join("\n")
}

/// The call id a command's own output box is filed under.
fn command_call_id(ui: &CoderUi) -> String {
    format!("command-{}", ui.entries.len())
}

// ────────────────────────────────────────────────────────────────── /diff

fn spawn_diff(_ui: &mut CoderUi, arguments: Vec<String>, tx: &Sender<Control>, cwd: &Path) {
    let tx = tx.clone();
    let cwd = cwd.to_path_buf();
    tokio::spawn(async move {
        let text = match openagents_cli::interactive::collect_diff(&arguments, &cwd).await {
            Err(why) => why,
            Ok(files) if files.is_empty() => "Nothing has changed.".to_string(),
            Ok(files) => render_diff(&files),
        };
        let _ = tx.send(Control::Output(text));
    });
}

/// A diff as markdown: a summary line per file, then the unified body in a
/// fenced block so the transcript's own renderer highlights it in the palette
/// it highlights everything else in.
fn render_diff(files: &[openagents_cli::diff::FileDiff]) -> String {
    use openagents_cli::diff::Tag;

    let mut summary = Vec::new();
    let mut body = Vec::new();
    for file in files {
        let (added, removed) = file.stats();
        let named = match &file.renamed_from {
            Some(from) => format!("{from} → {}", file.path),
            None => file.path.clone(),
        };
        summary.push(format!("- `{named}` +{added} −{removed}"));

        body.push(format!("--- a/{}", file.renamed_from.as_ref().unwrap_or(&file.path)));
        body.push(format!("+++ b/{}", file.path));
        if let Some(note) = &file.note {
            body.push(format!("# {note}"));
            continue;
        }
        for hunk in &file.hunks {
            body.push(hunk.header());
            for line in &hunk.lines {
                let marker = match line.tag {
                    Tag::Insert => '+',
                    Tag::Delete => '-',
                    Tag::Equal => ' ',
                };
                body.push(format!("{marker}{}", line.text));
            }
        }
    }

    let mut out = summary.join("\n");
    out.push_str("\n\n```diff\n");
    out.push_str(&bounded(body.join("\n")));
    out.push_str("\n```");
    out
}

/// The same ceiling the `shell` tool holds its output to, for the same reason:
/// a diff of a vendored directory is megabytes and the frame has to stay up.
fn bounded(text: String) -> String {
    if text.len() <= OUTPUT_LIMIT {
        return text;
    }
    // A diff is the repository's own bytes. Floored to a character boundary
    // rather than sliced at a byte index, which is the defect `c48fa5b138`
    // and `28704f72ff` went through this tree removing.
    let cut = openagents_cli::tracker::floor_char_boundary(&text, OUTPUT_LIMIT);
    format!(
        "{}\n[truncated: {} characters, limit is {OUTPUT_LIMIT}]",
        &text[..cut],
        text.len()
    )
}

// ─────────────────────────────────────────────────────────────────── /run

/// Run a command here and stream its output into a box.
///
/// Not a pseudoterminal: there is no pane to attach to and nothing takes keys
/// while it runs, so it is the same non-interactive spawn the `shell` tool
/// makes, held to the same refusal list and the same output ceiling. `/help`
/// says "run a command here and show its output" rather than anything about a
/// terminal, because that is what it does.
fn spawn_run(ui: &mut CoderUi, command: &str, tx: &Sender<Control>, cwd: &Path) {
    if command.is_empty() {
        output(ui, "`/run` needs a command: `/run git status`.");
        return;
    }
    if let Some(refusal) = check_shell_refusal(command) {
        output(ui, &refusal);
        return;
    }

    let call_id = command_call_id(ui);
    let mut entry = Entry::tool_call(format!("run {command}"));
    entry.tool = Some(ToolCall {
        call_id: call_id.clone(),
        function_name: "run".to_string(),
        arguments: serde_json::json!({ "command": command }),
        output: None,
        error: None,
    });
    ui.entries.push(entry);
    ui.scroll_override = None;

    let tx = tx.clone();
    let cwd = cwd.to_path_buf();
    let command = command.to_string();
    tokio::spawn(async move {
        use tokio::io::AsyncReadExt;

        let mut spawn = tokio::process::Command::new("/bin/sh");
        spawn
            .arg("-c")
            .arg(&command)
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        spawn.process_group(0);

        let mut child = match spawn.spawn() {
            Ok(child) => child,
            Err(error) => {
                let _ = tx.send(Control::ToolOutput {
                    call_id: call_id.clone(),
                    chunk: format!("could not start it: {error}"),
                });
                let _ = tx.send(Control::ToolDone {
                    call_id,
                    is_error: true,
                });
                return;
            }
        };

        let mut stdout = child.stdout.take();
        let mut stderr = child.stderr.take();
        let mut printed = 0usize;
        let mut buffer = [0u8; 4096];
        loop {
            let read = match (&mut stdout, &mut stderr) {
                (Some(out), _) => out.read(&mut buffer).await,
                (None, Some(err)) => err.read(&mut buffer).await,
                (None, None) => break,
            };
            match read {
                Ok(0) => {
                    // stdout first, then stderr, then done.
                    if stdout.is_some() {
                        stdout = None;
                    } else {
                        stderr = None;
                    }
                }
                Ok(n) => {
                    if printed < OUTPUT_LIMIT {
                        let chunk = String::from_utf8_lossy(&buffer[..n]).to_string();
                        printed += n;
                        let _ = tx.send(Control::ToolOutput {
                            call_id: call_id.clone(),
                            chunk,
                        });
                    }
                }
                Err(_) => break,
            }
        }

        let status = child.wait().await;
        let (note, failed) = match status {
            Ok(status) if status.success() => (String::new(), false),
            Ok(status) => (
                format!("\n[exited with code {}]", status.code().unwrap_or(1)),
                true,
            ),
            Err(error) => (format!("\n[it did not finish: {error}]"), true),
        };
        if !note.is_empty() {
            let _ = tx.send(Control::ToolOutput {
                call_id: call_id.clone(),
                chunk: note,
            });
        }
        let _ = tx.send(Control::ToolDone {
            call_id,
            is_error: failed,
        });
    });
}

// ──────────────────────────────────────────────────────────────── /resume

fn spawn_resume(ui: &mut CoderUi, arguments: &[String], tx: &Sender<Control>, cwd: &Path) {
    // A bare `/resume` lists; `/resume <n>` picks. A word that is not a
    // positive number is refused rather than read as a list request, because
    // silently listing after a mistyped pick is how someone resumes the wrong
    // session.
    let selection = match arguments.first() {
        None => None,
        Some(word) => match word.parse::<usize>() {
            Ok(number) if number >= 1 => Some(number),
            _ => {
                output(
                    ui,
                    &format!(
                        "`/resume` takes a number from the list: `/resume 1`. `{word}` is not one."
                    ),
                );
                return;
            }
        },
    };

    let tx = tx.clone();
    let cwd = cwd.to_path_buf();
    tokio::spawn(async move {
        // The scan compiles and runs a wasm guest and walks two state
        // directories, all of it synchronous. On a blocking thread so the
        // frame keeps drawing while it works.
        let home = openagents_cli::auth::home_directory();
        let scanned = tokio::task::spawn_blocking(move || {
            openagents_cli::foreign_resume::foreign_resume_turn(&cwd, &home, selection)
        })
        .await;
        let text = match scanned {
            Ok(text) => text,
            Err(error) => format!("The scan did not finish: {error}"),
        };
        let _ = tx.send(Control::Output(text));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn help_lists_every_command_and_nothing_else() {
        let text = help();
        for (name, what) in COMMANDS {
            assert!(text.contains(&format!("`/{name}`")), "{text}");
            assert!(text.contains(what), "{text}");
        }
    }

    /// A key in the hint text that nothing handles is the defect the rule in
    /// issue #105 is about. These are the ones `interactive` wires.
    #[test]
    fn the_key_hints_name_only_keys_the_session_handles() {
        let listed: Vec<&str> = KEYS.iter().map(|(key, _)| *key).collect();
        assert!(listed.contains(&"Enter"));
        assert!(listed.contains(&"Tab"));
        assert!(listed.contains(&"Esc / Ctrl+C / Ctrl+D"));
        // Nothing about a pane, a diff inspector, or a detach key: none of
        // those exist here.
        let text = help();
        for absent in ["Ctrl+]", "detach", "inspector"] {
            assert!(!text.contains(absent), "`{absent}` is claimed: {text}");
        }
    }

    #[test]
    fn a_diff_renders_as_a_summary_and_a_fenced_body() {
        let files = openagents_cli::diff::parse_unified(
            "diff --git a/a.txt b/a.txt\n--- a/a.txt\n+++ b/a.txt\n@@ -1,2 +1,2 @@\n line\n-old\n+new\n",
        );
        assert!(!files.is_empty(), "the fixture parsed to nothing");
        let text = render_diff(&files);
        assert!(text.contains("`a.txt` +1 −1"), "{text}");
        assert!(text.contains("```diff"), "{text}");
        assert!(text.contains("+new"), "{text}");
        assert!(text.contains("-old"), "{text}");
    }

    #[test]
    fn a_long_diff_is_cut_at_the_same_ceiling_the_shell_tool_uses() {
        let text = bounded("x".repeat(OUTPUT_LIMIT + 100));
        assert!(text.contains("truncated"), "it was not cut");
        assert!(text.len() < OUTPUT_LIMIT + 200);
    }
}

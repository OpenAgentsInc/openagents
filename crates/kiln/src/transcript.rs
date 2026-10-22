//! The agent run as the transcript draws it: entries of text, thinking,
//! quiet status lines, and tool cards, sent as JSON to the page's
//! transcript module (`apps/openagents/web/kiln-transcript`), which draws
//! them with the desktop's components. `docs/kiln/transcript.md` has the
//! shape.

use serde_json::{Value, json};

/// The run being drawn.
#[derive(Debug, Clone, Default)]
pub struct Run {
    entries: Vec<Value>,
    working: Option<String>,
    started_ms: Option<f64>,
    next: u64,
}

/// What a tool card shows about its call.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Call {
    /// `read`, `search`, `list`, `exec`, `write`, or `edit`.
    pub tool: &'static str,
    pub path: Option<String>,
    pub pattern: Option<String>,
    pub command: String,
}

/// The card for a command line (tool first) or a file write or patch.
#[must_use]
pub fn call_for(argv: &[String]) -> Call {
    let tool = argv.first().map_or("", String::as_str);
    let command = argv.join(" ");
    let rest: Vec<&String> = argv
        .iter()
        .skip(1)
        .filter(|a| !a.starts_with('-'))
        .collect();
    match tool {
        "cat" | "view" => Call {
            tool: "read",
            path: rest.first().map(|s| (*s).clone()),
            pattern: None,
            command,
        },
        "grep" => Call {
            tool: "search",
            pattern: rest.first().map(|s| (*s).clone()),
            path: rest.get(1).map(|s| (*s).clone()),
            command,
        },
        "write" => Call {
            tool: "write",
            path: rest.first().map(|s| (*s).clone()),
            pattern: None,
            command,
        },
        "patch" => Call {
            tool: "edit",
            path: rest.first().map(|s| (*s).clone()),
            pattern: None,
            command,
        },
        _ => Call {
            tool: "exec",
            path: None,
            pattern: None,
            command,
        },
    }
}

/// Split a model's answer into its `<think>` reasoning and the rest.
#[must_use]
pub fn split_thinking(text: &str) -> (Option<String>, String) {
    let mut thinking = String::new();
    let mut rest = text.to_string();
    while let (Some(start), Some(end)) = (rest.find("<think>"), rest.find("</think>")) {
        if end < start {
            break;
        }
        thinking.push_str(rest[start + "<think>".len()..end].trim());
        thinking.push('\n');
        rest.replace_range(start..end + "</think>".len(), "");
    }
    let thinking = thinking.trim().to_string();
    ((!thinking.is_empty()).then_some(thinking), rest)
}

impl Run {
    fn id(&mut self, prefix: &str) -> String {
        self.next += 1;
        format!("{prefix}{}", self.next)
    }

    /// Start over with a person's task.
    pub fn start(&mut self, task: &str, now_ms: f64) {
        *self = Self::default();
        let id = self.id("u");
        self.entries.push(json!({
            "id": id, "role": "user", "created_ms": now_ms as i64,
            "parts": [{"kind": "text", "text": task}],
        }));
    }

    /// Open a new assistant turn (one agent step), streaming.
    pub fn turn(&mut self, now_ms: f64) {
        self.close();
        let id = self.id("a");
        self.entries.push(json!({
            "id": id, "role": "assistant", "streaming": true,
            "created_ms": now_ms as i64, "parts": [],
        }));
    }

    fn current(&mut self, now_ms: f64) -> &mut Vec<Value> {
        if self.entries.last().is_none_or(|e| e["role"] != "assistant") {
            self.turn(now_ms);
        }
        let last = self.entries.last_mut().expect("a turn is open");
        last["parts"].as_array_mut().expect("parts are an array")
    }

    /// Finish the open turn, stamping its length.
    pub fn close(&mut self) {
        if let Some(last) = self.entries.last_mut()
            && last["role"] == "assistant"
            && last["streaming"] == true
        {
            last["streaming"] = json!(false);
        }
    }

    pub fn status(&mut self, text: &str, now_ms: f64) {
        self.current(now_ms)
            .push(json!({"kind": "status", "text": text}));
    }

    /// Replace the newest status line of the open turn, or add one.
    pub fn update_status(&mut self, text: &str, now_ms: f64) {
        let parts = self.current(now_ms);
        match parts.iter_mut().rev().find(|p| p["kind"] == "status") {
            Some(part) => part["text"] = json!(text),
            None => parts.push(json!({"kind": "status", "text": text})),
        }
    }

    pub fn reasoning(&mut self, text: &str, now_ms: f64) {
        if !text.trim().is_empty() {
            self.current(now_ms)
                .push(json!({"kind": "reasoning", "text": text}));
        }
    }

    pub fn text(&mut self, text: &str, now_ms: f64) {
        if !text.trim().is_empty() {
            self.current(now_ms)
                .push(json!({"kind": "text", "text": text}));
        }
    }

    /// A tool card, running. Returns its id.
    pub fn tool(&mut self, call: &Call, now_ms: f64) -> String {
        let id = self.id("c");
        let mut part = json!({"kind": "tool", "id": id, "tool": call.tool, "command": call.command, "done": false});
        if let Some(path) = &call.path {
            part["path"] = json!(path);
        }
        if let Some(pattern) = &call.pattern {
            part["pattern"] = json!(pattern);
        }
        self.current(now_ms).push(part);
        id
    }

    fn find(&mut self, id: &str) -> Option<&mut Value> {
        self.entries
            .iter_mut()
            .rev()
            .filter_map(|e| e["parts"].as_array_mut())
            .flat_map(|parts| parts.iter_mut())
            .find(|p| p["id"] == id)
    }

    /// Show a running tool's latest output line.
    pub fn progress(&mut self, id: &str, line: &str) {
        if let Some(part) = self.find(id) {
            part["output"] = json!(line);
        }
    }

    /// Finish a tool card: its output, whether it failed, and for a file
    /// change the text before and after.
    pub fn finish(
        &mut self,
        id: &str,
        output: &str,
        failed: bool,
        diff: Option<(Option<String>, String)>,
    ) {
        if let Some(part) = self.find(id) {
            part["done"] = json!(true);
            part["failed"] = json!(failed);
            part["output"] = json!(output);
            if let Some((old, new)) = diff {
                part["old"] = json!(old);
                part["new"] = json!(new);
            }
        }
    }

    /// What the agent is doing now; `None` when it's idle.
    pub fn working(&mut self, label: Option<&str>, now_ms: f64) {
        match label {
            Some(label) => {
                if self.working.is_none() {
                    self.started_ms = Some(now_ms);
                }
                self.working = Some(label.to_string());
            }
            None => {
                self.working = None;
                self.started_ms = None;
                self.close();
            }
        }
    }

    /// The JSON the transcript module takes.
    #[must_use]
    pub fn to_json(&self) -> String {
        json!({
            "entries": self.entries,
            "working": self.working,
            "started_ms": self.started_ms.map(|m| m as i64),
        })
        .to_string()
    }
}

#[must_use]
/// A Pylon note in the words the transcript shows.
pub fn pylon_words(note: &str) -> String {
    if note == "the pylon is working" {
        return "The Pylon is working on it…".into();
    }
    match note.strip_prefix("asking ") {
        Some(rest) => {
            let rest = rest.split(" (").next().unwrap_or(rest);
            match rest.split_once(" for ") {
                Some((pylon, model)) => format!("Asking {}…", who(pylon, model)),
                None => format!("Asking {rest}…"),
            }
        }
        None => note.to_string(),
    }
}

/// A pylon and its model in a few words: the pylon's label when it already
/// names the model (a model group's does), else the model on the pylon.
#[must_use]
pub fn who(label: &str, model: &str) -> String {
    let name = model_words(model);
    let first = name.split_whitespace().next().unwrap_or("").to_lowercase();
    if !first.is_empty() && label.to_lowercase().contains(&first) {
        label.to_string()
    } else {
        format!("{name} on {label}")
    }
}

#[must_use]
/// A model id as people read it.
pub fn model_words(model: &str) -> String {
    let name = model.split('@').next().unwrap_or(model);
    match name {
        "deepseek-v4.1-flash" => "DeepSeek V4.1 Flash · Experimental preview".into(),
        "flint" => "Flint".into(),
        other => other.to_string(),
    }
}

#[must_use]
/// Split a tool's output into what to show and toolchain noise to show
/// quietly: lines a sandboxed linker or `xcrun` prints about caches it
/// can't write, and the `warning:` blocks around them.
pub fn split_noise(output: &str) -> (String, String) {
    let (mut loud, mut quiet) = (String::new(), String::new());
    let mut in_warning = false;
    for line in output.lines() {
        let noisy = line.contains("couldn't create cache file")
            || line.starts_with("warning: output of `xcrun`")
            || line.starts_with("warning: linker stderr")
            || line.contains("generated 1 warning")
            || line.contains("generated 2 warnings")
            || (in_warning && (line.starts_with("  ") || line.trim().is_empty()));
        in_warning = noisy && !line.trim().is_empty();
        let out = if noisy { &mut quiet } else { &mut loud };
        out.push_str(line);
        out.push('\n');
    }
    (loud, quiet)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn commands_become_the_desktops_tool_kinds() {
        assert_eq!(call_for(&words("view src/lib.rs 10 40")).tool, "read");
        assert_eq!(
            call_for(&words("view src/lib.rs 10 40")).path.as_deref(),
            Some("src/lib.rs")
        );
        let grep = call_for(&words("grep -n fib src"));
        assert_eq!(
            (grep.tool, grep.pattern.as_deref(), grep.path.as_deref()),
            ("search", Some("fib"), Some("src"))
        );
        assert_eq!(call_for(&words("patch a.rs")).tool, "edit");
        assert_eq!(call_for(&words("cargo test -p kiln")).tool, "exec");
    }

    #[test]
    fn pylon_notes_and_toolchain_noise_read_plainly() {
        assert_eq!(
            pylon_words(
                "asking DeepSeek-V4.1 w16-g5c for deepseek-v4.1-flash@sha256:74b0 (6 KiB prompt, wait up to 300 s)"
            ),
            "Asking DeepSeek-V4.1 w16-g5c…"
        );
        assert_eq!(
            pylon_words("the pylon is working"),
            "The Pylon is working on it…"
        );
        let (loud, quiet) = split_noise(
            "warning: output of `xcrun` while finding MacOSX.sdk\n  = note: xcrun: error: couldn't create cache file '/var/x'\ntest result: ok. 6 passed\n",
        );
        assert_eq!(loud, "test result: ok. 6 passed\n");
        assert!(quiet.contains("xcrun"));
    }

    #[test]
    fn the_pages_words_pass_the_copy_guard() {
        let strip = |html: &str| {
            let body = html.split("<body>").nth(1).unwrap_or(html);
            let body = body.split("<script").next().unwrap_or(body);
            let mut text = String::new();
            let mut in_tag = false;
            for c in body.chars() {
                match c {
                    '<' => in_tag = true,
                    '>' => {
                        in_tag = false;
                        text.push(' ');
                    }
                    _ if !in_tag => text.push(c),
                    _ => {}
                }
            }
            text
        };
        for page in [
            include_str!("../www/index.html"),
            include_str!("../www/review.html"),
        ] {
            let text = strip(page);
            let hits = oa_copy::violations(&text, &[]);
            assert!(hits.is_empty(), "{hits:?}");
        }
        for line in [
            pylon_words("the pylon is working"),
            pylon_words("asking Mac (network lane) for qwen (1 KiB prompt)"),
            model_words("deepseek-v4.1-flash@sha256:aa"),
            "I couldn't read that reply as commands, so I'll ask again.".into(),
        ] {
            assert!(oa_copy::violations(&line, &[]).is_empty(), "{line}");
        }
    }

    #[test]
    fn thinking_splits_from_the_answer() {
        let (thinking, rest) = split_thinking("<think>\nhmm, read it\n</think>\n{\"a\":1}");
        assert_eq!(thinking.as_deref(), Some("hmm, read it"));
        assert_eq!(rest.trim(), "{\"a\":1}");
        assert_eq!(split_thinking("plain").0, None);
    }

    #[test]
    fn a_run_records_turns_tools_and_status() {
        let mut run = Run::default();
        run.start("make fib", 1.0);
        run.working(Some("Asking DeepSeek"), 2.0);
        run.turn(2.0);
        run.status("Asking DeepSeek V4.1", 2.0);
        run.update_status("DeepSeek V4.1 answered in 24 s", 3.0);
        run.reasoning("think", 3.0);
        run.text("Write it.", 3.0);
        let id = run.tool(&call_for(&words("write fib.rhai")), 3.0);
        run.finish(
            &id,
            "wrote fib.rhai",
            false,
            Some((None, "fn fib() {}".into())),
        );
        run.working(None, 4.0);
        let value: Value = serde_json::from_str(&run.to_json()).unwrap();
        let entries = value["entries"].as_array().unwrap();
        assert_eq!(entries.len(), 2);
        let parts = entries[1]["parts"].as_array().unwrap();
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0]["text"], "DeepSeek V4.1 answered in 24 s");
        assert_eq!(parts[3]["tool"], "write");
        assert_eq!(parts[3]["new"], "fn fib() {}");
        assert_eq!(entries[1]["streaming"], false);
        assert!(value["working"].is_null());
    }
}

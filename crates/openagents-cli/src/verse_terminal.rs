//! `openagents verse terminal`: drive the terminal overlay of the Verse
//! window on this computer over its control socket
//! (`~/.openagents/verse/terminal.sock`, or `VERSE_TERMINAL_SOCKET`).
//!
//! Each command is one JSON request to the socket and one JSON reply;
//! `--json` prints the reply as it came. Nothing here opens a PTY: the
//! panes belong to Verse, which serves the requests between frames.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use verse::terminal_control::{self as control, SOCKET_ENV};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents verse terminal COMMAND [OPTIONS]
  status                    The overlay: open, focused, tabs, and panes with
                            their ids, labels, working directories, and sizes.
  open                      Show the overlay with focus; the first pane starts
                            when none runs (OpenAgents Terminal, else the shell).
  hide                      Hide the overlay; its panes keep running.
  split rows|cols [-- PROGRAM ARGS...]
                            Split the focused pane; the new pane runs PROGRAM
                            (absolute, or found on PATH) or the shell.
  focus left|right|up|down|PANE
                            Focus a neighbor of the focused pane, or pane PANE.
  close                     Close the focused pane, ending its program.
  send TEXT [--enter]       Type TEXT into the focused pane; --enter presses
                            Enter after it.
  key NAME                  Press a key: enter, tab, escape, backspace, up, down,
                            left, right, home, end, pageup, pagedown, delete,
                            space, f1..f12, or a chord such as ctrl-c or alt-x.
  read [--pane PANE] [--wait-for TEXT] [--wait SECONDS]
                            The visible text of the focused pane, or of PANE;
                            --wait-for polls until TEXT appears (default 10 s)
                            and exits 1 when it does not.
  tab new|next|prev         Open a tab, or switch tabs.
  zoom                      Zoom the focused pane to the whole overlay, or back.
Options: --socket PATH (default ~/.openagents/verse/terminal.sock, or
VERSE_TERMINAL_SOCKET). Verse must be running on this computer.";

const COMMANDS: &[&str] = &[
    "status", "open", "hide", "split", "focus", "close", "send", "key", "read", "tab", "zoom",
];

pub(crate) fn run(output: &Output, words: &[String]) -> u8 {
    let group = "verse terminal";
    let Some((command, rest)) = words.split_first() else {
        return output.usage(group, "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    if !COMMANDS.contains(&command.as_str()) {
        return output.usage(group, &format!("unknown command `{command}`"), USAGE);
    }
    let args = match Args::parse(rest, &["enter"]) {
        Ok(args) => args,
        Err(message) => return output.usage(group, &message, USAGE),
    };
    for name in args.option_names() {
        if !["socket", "pane", "wait-for", "wait"].contains(&name) {
            return output.usage(
                group,
                &format!("--{name} isn't an option of {group}"),
                USAGE,
            );
        }
    }
    let socket = match socket(&args) {
        Some(path) => path,
        None => {
            return output.fail(
                group,
                &format!("no home directory; pass --socket PATH or set {SOCKET_ENV}"),
            );
        }
    };
    let request = match request(command, &args) {
        Ok(request) => request,
        Err(message) => return output.usage(group, &message, USAGE),
    };
    if command == "read" && args.option("wait-for").is_some() {
        return read_until(output, &socket, &request, &args);
    }
    let reply = match control::call(&socket, &request) {
        Ok(reply) => reply,
        Err(message) => return output.fail(group, &message),
    };
    if reply["ok"] != true {
        let error = reply["error"].as_str().unwrap_or("the request failed");
        return output.fail(group, error);
    }
    if command == "send" && args.switch("enter") {
        let enter = json!({ "op": "key", "name": "enter" });
        if let Err(message) = control::call(&socket, &enter) {
            return output.fail(group, &message);
        }
    }
    output.emit(&reply, |v| render(command, v));
    0
}

fn socket(args: &Args) -> Option<PathBuf> {
    match args.option("socket") {
        Some(path) => Some(PathBuf::from(path)),
        None => control::default_path(),
    }
}

/// The request `command` and its arguments make.
fn request(command: &str, args: &Args) -> Result<Value, String> {
    let positional = args.positional();
    let need = |n: usize, what: &str| -> Result<(), String> {
        if positional.len() < n {
            return Err(format!("{command} needs {what}"));
        }
        Ok(())
    };
    let only = |n: usize| -> Result<(), String> {
        if positional.len() > n {
            return Err(format!(
                "unexpected argument `{}` for {command}",
                positional[n]
            ));
        }
        Ok(())
    };
    Ok(match command {
        "status" | "open" | "hide" | "close" | "zoom" => {
            only(0)?;
            json!({ "op": command })
        }
        "split" => {
            need(1, "rows or cols")?;
            json!({ "op": "split", "axis": positional[0], "program": &positional[1..] })
        }
        "focus" => {
            need(1, "a direction or a pane id")?;
            only(1)?;
            match positional[0].parse::<u64>() {
                Ok(pane) => json!({ "op": "focus", "pane": pane }),
                Err(_) => json!({ "op": "focus", "direction": positional[0] }),
            }
        }
        "send" => {
            need(1, "text")?;
            json!({ "op": "send", "text": positional.join(" ") })
        }
        "key" => {
            need(1, "a key name")?;
            only(1)?;
            json!({ "op": "key", "name": positional[0] })
        }
        "read" => {
            only(0)?;
            let pane = match args.option("pane") {
                Some(pane) => Some(
                    pane.parse::<u64>()
                        .map_err(|_| format!("--pane needs a pane id, not `{pane}`"))?,
                ),
                None => None,
            };
            json!({ "op": "read", "pane": pane })
        }
        "tab" => {
            need(1, "new, next, or prev")?;
            only(1)?;
            json!({ "op": "tab", "action": positional[0] })
        }
        other => return Err(format!("unknown command `{other}`")),
    })
}

/// Polls `read` until the pane's text contains `--wait-for`, or `--wait`
/// seconds pass.
fn read_until(output: &Output, socket: &std::path::Path, request: &Value, args: &Args) -> u8 {
    let group = "verse terminal";
    let needle = args.option("wait-for").unwrap_or_default().to_owned();
    let seconds = match args.number::<f64>("wait", 10.0) {
        Ok(seconds) => seconds,
        Err(message) => return output.usage(group, &message, USAGE),
    };
    let deadline = Instant::now() + Duration::from_secs_f64(seconds.max(0.0));
    loop {
        let reply = match control::call(socket, request) {
            Ok(reply) => reply,
            Err(message) => return output.fail(group, &message),
        };
        if reply["ok"] != true {
            let error = reply["error"].as_str().unwrap_or("the request failed");
            return output.fail(group, error);
        }
        let found = reply["text"].as_str().is_some_and(|t| t.contains(&needle));
        if found || Instant::now() >= deadline {
            let mut reply = reply;
            if let Some(map) = reply.as_object_mut() {
                map.insert("found".into(), Value::Bool(found));
                map.insert("waited_for".into(), Value::String(needle.clone()));
            }
            output.emit(&reply, |v| render("read", v));
            return u8::from(!found);
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn render(command: &str, value: &Value) -> String {
    match command {
        "read" => {
            let text = value["text"].as_str().unwrap_or_default();
            let trimmed = text.trim_end_matches(['\n', ' ']);
            let mut lines: Vec<String> = trimmed.lines().map(|l| l.trim_end().to_owned()).collect();
            if value["found"] == false {
                lines.push(format!(
                    "(did not see {:?})",
                    value["waited_for"].as_str().unwrap_or_default()
                ));
            }
            lines.join("\n")
        }
        "send" | "key" => String::new(),
        _ => status_text(value),
    }
}

fn status_text(value: &Value) -> String {
    let mut rows = vec![vec![
        "pane".to_owned(),
        "tab".to_owned(),
        "focus".to_owned(),
        "size".to_owned(),
        "label".to_owned(),
        "cwd".to_owned(),
    ]];
    for pane in value["panes"].as_array().into_iter().flatten() {
        rows.push(vec![
            pane["id"].to_string(),
            pane["tab"]
                .as_u64()
                .map_or(String::new(), |t| t.to_string()),
            if pane["focused"] == true { "*" } else { "" }.to_owned(),
            format!(
                "{}x{}",
                pane["cols"].as_u64().unwrap_or(0),
                pane["rows"].as_u64().unwrap_or(0)
            ),
            match pane["exited"].as_str() {
                Some(exit) => format!("{} ({exit})", pane["label"].as_str().unwrap_or_default()),
                None => pane["label"].as_str().unwrap_or_default().to_owned(),
            },
            pane["cwd"].as_str().unwrap_or_default().to_owned(),
        ]);
    }
    let head = format!(
        "overlay {}{}, {} tab(s), {} pane(s)",
        if value["open"] == true {
            "open"
        } else {
            "hidden"
        },
        if value["focused"] == true {
            " with focus"
        } else {
            ""
        },
        value["tabs"].as_array().map_or(0, Vec::len),
        value["panes"].as_array().map_or(0, Vec::len),
    );
    if rows.len() == 1 {
        return head;
    }
    format!("{head}\n{}", crate::out::table(&rows))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Args {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        Args::parse(&words, &["enter"]).unwrap()
    }

    #[test]
    fn commands_become_requests() {
        assert_eq!(
            request("status", &args(&[])).unwrap(),
            json!({ "op": "status" })
        );
        assert_eq!(
            request("split", &args(&["cols", "--", "/bin/sh", "-c", "echo hi"])).unwrap(),
            json!({ "op": "split", "axis": "cols", "program": ["/bin/sh", "-c", "echo hi"] })
        );
        assert_eq!(
            request("focus", &args(&["2"])).unwrap(),
            json!({ "op": "focus", "pane": 2 })
        );
        assert_eq!(
            request("focus", &args(&["left"])).unwrap(),
            json!({ "op": "focus", "direction": "left" })
        );
        assert_eq!(
            request("send", &args(&["echo", "hi", "--enter"])).unwrap(),
            json!({ "op": "send", "text": "echo hi" })
        );
        assert_eq!(
            request("read", &args(&["--pane", "3"])).unwrap(),
            json!({ "op": "read", "pane": 3 })
        );
        assert!(request("read", &args(&["--pane", "x"])).is_err());
        assert!(request("key", &args(&[])).is_err());
        assert!(request("tab", &args(&["new", "extra"])).is_err());
    }

    #[test]
    fn status_renders_panes_as_a_table() {
        let value = json!({
            "open": true, "focused": true, "tabs": [{}],
            "panes": [{ "id": 1, "tab": 0, "focused": true, "rows": 24, "cols": 80,
                        "label": "sh", "cwd": "~/work" }]
        });
        let text = status_text(&value);
        assert!(text.starts_with("overlay open with focus, 1 tab(s), 1 pane(s)"));
        assert!(text.contains("80x24"));
        assert!(text.contains("~/work"));
    }

    #[test]
    fn a_missing_socket_fails_and_wait_for_reports_absence() {
        let dir = tempfile::tempdir().unwrap();
        let socket = dir.path().join("none.sock");
        let output = Output::new(true);
        let words: Vec<String> = ["status", "--socket", socket.to_str().unwrap()]
            .iter()
            .map(|w| (*w).to_owned())
            .collect();
        assert_eq!(run(&output, &words), 1);
        let words: Vec<String> = ["bogus"].iter().map(|w| (*w).to_owned()).collect();
        assert_eq!(run(&output, &words), crate::out::EXIT_USAGE);
    }
}

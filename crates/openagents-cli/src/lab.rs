//! `openagents lab`: Video Lab from the terminal (docs/video-lab/mvp.md).
//! The same tools the web chat and Coder call, on the Video Lab service on
//! this computer (scripts/film/lab.py, `http://127.0.0.1:8792`), which is
//! started from its script when it isn't running.

use std::path::PathBuf;
use std::time::Duration;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents lab COMMAND [DOCUMENT] [options]
  kinds [--space me|project:ID]  Account Labs and their tool names.
  tool KIND NAME [--json OBJECT] [--space me|project:ID]  Call an account Lab tool.
  watch DOCUMENT [--since N] [--count N] [--space me|project:ID]  Follow changes.
  list  The documents.
  open DOCUMENT [--version N]  Its shots, problems and versions.
  show DOCUMENT [--from T] [--to T] [--version N]  A time range.
  frame DOCUMENT AT... [--version N]  One still (or a strip), its file path.
  edit DOCUMENT --patch PATCH --summary TEXT [--version N] [--author NAME]
                 A JSON Patch (or a file with one), checked before it's
                 saved; a refused edit says why.
  versions DOCUMENT  Who made each version and what it changed.
  render DOCUMENT [--quality preview|review|master] [--version N]  Render a version.
  compare DOCUMENT [--at T] [--a N] [--b N]  Two versions side by side.
  notes DOCUMENT [--version N]  The review notes on its videos.
Signed-in commands use account Labs. Kinds not hosted use the local Lab
(docs/video-lab/mvp.md). Every edit is a new version. Times are m:ss or
seconds. Each command prints the Lab player link to watch it. VIDEO_LAB_URL moves the Lab; OPENAGENTS_VIDEO_LAB_SCRIPT
names the script that starts it when it isn't running.";

/// What the command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("kinds", Effect::ReadOnly),
    Declared::computer("tool", Effect::Publishes),
    Declared::computer("watch", Effect::ReadOnly),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("open", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("frame", Effect::ReadOnly),
    Declared::computer("edit", Effect::Publishes),
    Declared::computer("versions", Effect::ReadOnly),
    Declared::computer("render", Effect::Publishes),
    Declared::computer("compare", Effect::ReadOnly),
    Declared::computer("notes", Effect::ReadOnly),
];

fn url() -> String {
    std::env::var("VIDEO_LAB_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8792".into())
        .trim_end_matches('/')
        .to_owned()
}

fn script() -> Option<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    [
        std::env::var_os("OPENAGENTS_VIDEO_LAB_SCRIPT").map(PathBuf::from),
        Some(PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/film/lab.py"
        ))),
        home.map(|h| h.join("work/openagents/scripts/film/lab.py")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

fn up(http: &reqwest::blocking::Client, base: &str) -> bool {
    http.get(format!("{base}/healthz"))
        .timeout(Duration::from_secs(1))
        .send()
        .is_ok_and(|r| r.status().is_success())
}

fn ensure(http: &reqwest::blocking::Client, base: &str) -> Result<(), String> {
    if up(http, base) {
        return Ok(());
    }
    let script = script().ok_or_else(|| {
        format!("Video Lab isn't running at {base}, and its script isn't on this computer")
    })?;
    let dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("work/film-out/lab");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let log = std::fs::File::create(dir.join("serve.log")).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    std::process::Command::new("python3")
        .arg(&script)
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(err)
        .spawn()
        .map_err(|e| format!("Video Lab couldn't start ({e})"))?;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(250));
        if up(http, base) {
            return Ok(());
        }
    }
    Err(format!(
        "Video Lab didn't start; see {}",
        dir.join("serve.log").display()
    ))
}

/// The tool and its arguments for the command line, or the usage problem.
pub(crate) fn request(words: &[String]) -> Result<(String, Value), String> {
    let args = Args::parse(words, &[])?;
    let positional = args.positional();
    let Some(command) = positional.first() else {
        return Err("say which command".into());
    };
    let allowed: &[&str] = match command.as_str() {
        "list" => &[],
        "open" | "frame" => &["version"],
        "show" => &["from", "to", "version"],
        "edit" => &["patch", "summary", "version", "author"],
        "versions" => &[],
        "render" => &["quality", "version"],
        "compare" => &["at", "a", "b"],
        "notes" => &["version"],
        other => return Err(format!("unknown command `{other}`")),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|n| !allowed.contains(n))
    {
        return Err(format!("`lab {command}` takes no --{name}"));
    }
    let mut call = json!({});
    if command != "list" {
        let Some(document) = positional.get(1) else {
            return Err(format!("`lab {command}` needs a document"));
        };
        call["document"] = json!(document);
    }
    if command == "frame" {
        let at: Vec<&String> = positional.iter().skip(2).collect();
        match at.as_slice() {
            [] => return Err("`lab frame` needs a time, like 1:26".into()),
            [one] => call["at"] = json!(one),
            many => call["at"] = json!(many),
        }
    } else if positional.len() > 2 || (command == "list" && positional.len() > 1) {
        return Err(format!("unexpected word `{}`", positional.last().unwrap()));
    }
    for name in allowed {
        if let Some(value) = args.option(name) {
            call[*name] = json!(value);
        }
    }
    if command == "edit" {
        let Some(patch) = args.option("patch") else {
            return Err("`lab edit` needs --patch, a JSON Patch or a file with one".into());
        };
        if args.option("summary").is_none_or(|s| s.trim().is_empty()) {
            return Err("`lab edit` needs --summary, one line on what it changes".into());
        }
        let text = if std::path::Path::new(patch).is_file() {
            std::fs::read_to_string(patch).map_err(|e| format!("{patch}: {e}"))?
        } else {
            patch.to_owned()
        };
        call["patch"] = json!(text);
        if args.option("author").is_none() {
            call["author"] = json!(std::env::var("USER").unwrap_or_else(|_| "owner".into()));
        }
    }
    Ok((format!("lab_{command}"), call))
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|w| matches!(w.as_str(), "kinds" | "tool" | "watch"))
    {
        return crate::hosted_labs::command(output, words);
    }
    if words.is_empty()
        || words
            .first()
            .is_some_and(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let (tool, call) = match request(words) {
        Ok(found) => found,
        Err(message) => return output.usage("lab", &message, USAGE),
    };
    if let Some(code) = crate::hosted_labs::try_run(output, "video", &tool, &call) {
        return code;
    }
    let base = url();
    let http = match reqwest::blocking::Client::builder()
        .default_headers(crate::keys::test_run_headers())
        .timeout(Duration::from_secs(300))
        .build()
    {
        Ok(http) => http,
        Err(e) => return output.fail("lab", &e.to_string()),
    };
    if let Err(message) = ensure(&http, &base) {
        return output.fail("lab", &message);
    }
    let result: Value = match http
        .post(format!("{base}/api/tool/{tool}"))
        .json(&call)
        .send()
        .and_then(|r| r.json())
    {
        Ok(result) => result,
        Err(e) => {
            return output.fail(
                "lab",
                &format!("Video Lab didn't answer ({})", e.without_url()),
            );
        }
    };
    if let Some(error) = result["error"].as_str()
        && result.get("refused").is_none()
    {
        return output.fail("lab", error);
    }
    let refused = result["refused"] == true;
    output.emit(&result, |value| {
        let mut text = value["text"].as_str().unwrap_or_default().to_owned();
        if let Some(image) = value["image"].as_str() {
            text.push_str(&format!("\nFrame: {image}"));
        }
        if let Some(player) = value["card"]["player"].as_str() {
            text.push_str(&format!("\nWatch it: {player}"));
        }
        text
    });
    if refused { crate::EXIT_FAILURE } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn commands_become_lab_tool_calls() {
        let (tool, call) = request(&words("show episode-290 --from 1:20 --to 1:35")).unwrap();
        assert_eq!(tool, "lab_show");
        assert_eq!(
            call,
            json!({"document": "episode-290", "from": "1:20", "to": "1:35"})
        );
        let (tool, call) = request(&words("frame episode-290 1:20 1:26")).unwrap();
        assert_eq!(tool, "lab_frame");
        assert_eq!(call["at"], json!(["1:20", "1:26"]));
        let (tool, call) = request(&words("render episode-290 --quality review")).unwrap();
        assert_eq!(
            (tool.as_str(), call["quality"].as_str()),
            ("lab_render", Some("review"))
        );
    }

    #[test]
    fn mistakes_are_plain() {
        assert!(
            request(&words("open"))
                .unwrap_err()
                .contains("needs a document")
        );
        assert!(
            request(&words("edit episode-290 --patch []"))
                .unwrap_err()
                .contains("--summary")
        );
        assert!(
            request(&words("open episode-290 --quality x"))
                .unwrap_err()
                .contains("takes no --quality")
        );
        assert!(
            request(&words("delete episode-290"))
                .unwrap_err()
                .contains("unknown command")
        );
    }
}

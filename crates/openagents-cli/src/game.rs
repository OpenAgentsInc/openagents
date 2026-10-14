//! `openagents game`: Game Lab from the terminal (docs/game-lab/mvp.md).
//! The same tools the web chat calls, on the Game Lab service on this
//! computer (`game-lab serve`, `http://127.0.0.1:8793`), which is started
//! when it isn't running and its program is here.

use std::path::PathBuf;
use std::time::Duration;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents game COMMAND [GAME] [options]
  list  The games.
  open [GAME] [--version N]  Its settings, bot-run stats and versions.
  play [GAME] [--version N]  A link to play a version.
  edit [GAME] --patch PATCH --summary TEXT [--version N] [--author NAME]
                 A JSON Patch (or a file with one) on its game.v1 document,
                 checked with its rules and 24 seeded bot runs before it's
                 saved; a refused edit says why.
  versions [GAME]  Who made each version and what it changed.
  compare [GAME] [--a N] [--b N]  Two versions: what changed and what the
                 bot runs measured (version 1 is the original).
  notes [GAME] [--version N] [--text TEXT] [--seconds S] [--room ROOM]
                 The notes on it, or save one at a time and place.
Change games on this computer by editing their settings (docs/game-lab/mvp.md).
GAME is the-ice-cream-man (the default). Every edit is a new version, kept on
this computer. GAME_LAB_URL moves the Lab; GAME_LAB_BIN names the program
that starts it when it isn't running.";

/// What the command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("open", Effect::ReadOnly),
    Declared::computer("play", Effect::ReadOnly),
    Declared::computer("edit", Effect::LocalWrite),
    Declared::computer("versions", Effect::ReadOnly),
    Declared::computer("compare", Effect::ReadOnly),
    Declared::computer("notes", Effect::LocalWrite),
];

fn url() -> String {
    std::env::var("GAME_LAB_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8793".into())
        .trim_end_matches('/')
        .to_owned()
}

/// The `game-lab` program: `GAME_LAB_BIN`, beside this one, or on PATH.
fn program() -> Option<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("game-lab")));
    let on_path = std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join("game-lab"))
            .find(|p| p.is_file())
    });
    [
        std::env::var_os("GAME_LAB_BIN").map(PathBuf::from),
        beside,
        on_path,
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
    let program = program().ok_or_else(|| {
        format!(
            "Game Lab isn't running at {base}; start it with: ~/work/openagents/scripts/dev/full-local.sh start --lab"
        )
    })?;
    let dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("work/film-out/lab/games");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let log = std::fs::File::create(dir.join("serve.log")).map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    std::process::Command::new(&program)
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(err)
        .spawn()
        .map_err(|e| format!("Game Lab couldn't start ({e})"))?;
    for _ in 0..40 {
        std::thread::sleep(Duration::from_millis(250));
        if up(http, base) {
            return Ok(());
        }
    }
    Err(format!(
        "Game Lab didn't start; see {}",
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
        "list" | "versions" => &[],
        "open" | "play" => &["version"],
        "edit" => &["patch", "summary", "version", "author"],
        "compare" => &["a", "b"],
        "notes" => &["version", "text", "seconds", "room"],
        other => return Err(format!("unknown command `{other}`")),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|n| !allowed.contains(n))
    {
        return Err(format!("`game {command}` takes no --{name}"));
    }
    if positional.len() > 2 || (command == "list" && positional.len() > 1) {
        return Err(format!(
            "unexpected word `{}`",
            positional.last().map_or("", |w| w.as_str())
        ));
    }
    let mut call = json!({});
    // The Lab's tools take the game they're about; on the command line it
    // may be left out, and the one game is meant.
    if command != "list" {
        call["game"] = json!(
            positional
                .get(1)
                .map_or("the-ice-cream-man", String::as_str)
        );
    }
    for name in allowed {
        if let Some(value) = args.option(name) {
            call[*name] = json!(value);
        }
    }
    if command == "edit" {
        let Some(patch) = args.option("patch") else {
            return Err("`game edit` needs --patch, a JSON Patch or a file with one".into());
        };
        if args.option("summary").is_none_or(|s| s.trim().is_empty()) {
            return Err("`game edit` needs --summary, one line on what it changes".into());
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
    Ok((format!("game_{command}"), call))
}

fn lines(value: &Value, key: &str) -> String {
    value[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(|line| format!("\n  {line}"))
        .collect()
}

/// The result in words.
fn words_for(value: &Value) -> String {
    let mut text = String::new();
    if value["refused"] == true {
        text.push_str(&format!(
            "Refused: {}",
            value["reason"].as_str().unwrap_or_default()
        ));
        return text;
    }
    if let Some(t) = value["text"].as_str() {
        text.push_str(t);
    }
    if let Some(list) = value["documents"].as_array() {
        for doc in list {
            text.push_str(&format!(
                "{} \"{}\" ({} versions)\n",
                doc["id"].as_str().unwrap_or_default(),
                doc["title"].as_str().unwrap_or_default(),
                doc["versions"]
            ));
        }
    }
    if let Some(list) = value["versions"].as_array() {
        for v in list {
            text.push_str(&format!(
                "v{}: {} (by {}, bot escapes {}%)\n",
                v["version"],
                v["summary"].as_str().unwrap_or_default(),
                v["author"].as_str().unwrap_or_default(),
                v["win_rate"]
            ));
        }
    }
    if value["a"].is_object() {
        text.push_str(&format!(
            "v{} ({}) against v{} ({})",
            value["a"]["version"],
            value["a"]["summary"].as_str().unwrap_or_default(),
            value["b"]["version"],
            value["b"]["summary"].as_str().unwrap_or_default()
        ));
    }
    let changes = lines(value, "changes");
    if !changes.is_empty() {
        text.push_str(&format!("\nChanged:{changes}"));
    }
    let measured = lines(value, "differences");
    if !measured.is_empty() {
        text.push_str(&format!("\nThe bot runs measured:{measured}"));
    }
    if let Some(notes) = value["notes"].as_array() {
        if notes.is_empty() {
            text.push_str("No notes yet.");
        }
        for n in notes {
            text.push_str(&format!(
                "v{} {}{}: {}\n",
                n["version"],
                n["seconds"]
                    .as_f64()
                    .map_or(String::new(), |s| format!("at {s:.0}s ")),
                n["room"]
                    .as_str()
                    .map_or(String::new(), |r| format!("in the {r}")),
                n["text"].as_str().unwrap_or_default()
            ));
        }
    }
    if value["saved"] == true && value["note"].is_object() {
        text.push_str("Note saved.");
    }
    if let Some(player) = value["card"]["player"].as_str() {
        text.push_str(&format!("\nPlay it: {player}"));
    }
    text.trim().to_owned()
}

pub fn run(output: &Output, words: &[String]) -> u8 {
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
        Err(message) => return output.usage("game", &message, USAGE),
    };
    let base = url();
    let http = match reqwest::blocking::Client::builder()
        .default_headers(crate::keys::test_run_headers())
        .timeout(Duration::from_secs(300))
        .build()
    {
        Ok(http) => http,
        Err(e) => return output.fail("game", &e.to_string()),
    };
    if let Err(message) = ensure(&http, &base) {
        return output.fail("game", &message);
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
                "game",
                &format!("Game Lab didn't answer ({})", e.without_url()),
            );
        }
    };
    if let Some(error) = result["error"].as_str() {
        return output.fail("game", error);
    }
    let refused = result["refused"] == true;
    output.emit(&result, words_for);
    if refused { crate::EXIT_FAILURE } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> Vec<String> {
        text.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn commands_become_game_tool_calls() {
        let (tool, call) = request(&words("compare --a 1")).unwrap();
        assert_eq!(tool, "game_compare");
        assert_eq!(call, json!({"game": "the-ice-cream-man", "a": "1"}));
        let (tool, call) = request(&words("open the-ice-cream-man --version 2")).unwrap();
        assert_eq!(tool, "game_open");
        assert_eq!(call, json!({"game": "the-ice-cream-man", "version": "2"}));
        let (tool, call) = request(&words("edit --patch [] --summary Slower")).unwrap();
        assert_eq!(tool, "game_edit");
        assert_eq!(call["patch"], "[]");
    }

    #[test]
    fn mistakes_are_plain() {
        assert!(
            request(&words("edit --patch []"))
                .unwrap_err()
                .contains("--summary")
        );
        assert!(
            request(&words("open --quality x"))
                .unwrap_err()
                .contains("takes no --quality")
        );
        assert!(
            request(&words("delete"))
                .unwrap_err()
                .contains("unknown command")
        );
    }

    #[test]
    fn a_refusal_reads_as_one() {
        let text = words_for(&json!({"refused": true, "reason": "nobody could escape"}));
        assert_eq!(text, "Refused: nobody could escape");
        let text = words_for(
            &json!({"text": "Saved version 2.", "changes": ["man.man_chase: 5.6 → 4.48"],
            "card": {"player": "http://127.0.0.1:8793/play?doc=the-ice-cream-man&v=2"}}),
        );
        assert!(text.contains("Changed:\n  man.man_chase") && text.contains("Play it: http"));
    }
}

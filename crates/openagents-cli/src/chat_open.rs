//! `openagents chat open`: hand a terminal session's work to an app chat.
//!
//! A session that coordinates issues must be an app chat, so Foreman, the
//! phone, and the website can see and steer it (`AGENTS.md`;
//! `docs/foreman/lessons-learned.md`, L1). This command starts one on the
//! running OpenAgents app engine through the engine's own `create_chat`
//! tool (`openagents_mcp`), the same path Foreman's `create_chat` takes, so
//! the chat is a standalone session listed in `list_chats` and Sessions.
//! It prints the chat's ID and returns; the chat runs in the app, not here.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use openagents_mcp::{OpenAgents, Origin, Tools};
use openagents_proto::Space;
use serde_json::{Value, json};

use crate::{Output, argv};

/// How long the whole command may take: connecting, resolving the
/// project, creating the chat, and delivering its first message.
const TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    brief: Option<String>,
    project: Option<String>,
    device: Option<String>,
    harness: Option<String>,
    model: Option<String>,
    title: Option<String>,
    port: Option<u16>,
}

fn parse(words: &[String]) -> Result<Options, String> {
    let args = argv::parse_command(
        words,
        "chat open",
        &["project", "device", "harness", "model", "title", "ipc-port"],
        &[],
        0,
        usize::MAX,
    )?;
    let words = args.positional();
    let brief = if words.len() == 1 && words[0] == "-" {
        let mut text = String::new();
        std::io::Read::read_to_string(
            &mut std::io::Read::take(std::io::stdin(), 64 * 1024),
            &mut text,
        )
        .map_err(|error| format!("cannot read the brief from stdin: {error}"))?;
        Some(text)
    } else {
        Some(words.join(" "))
    }
    .map(|text| text.trim().to_owned())
    .filter(|text| !text.is_empty());
    if brief.as_ref().is_some_and(|text| text.len() > 32 * 1024) {
        return Err("the brief is longer than 32 KiB".into());
    }
    let text = |name: &str| {
        args.option(name)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };
    let port = args
        .option("ipc-port")
        .map(|value| {
            value
                .parse::<u16>()
                .ok()
                .filter(|port| *port != 0)
                .ok_or("--ipc-port needs a port from 1 to 65535")
        })
        .transpose()?;
    Ok(Options {
        brief,
        project: text("project"),
        device: text("device"),
        harness: text("harness"),
        model: text("model"),
        title: text("title"),
        port,
    })
}

/// The project on `device` whose folder holds `cwd`, the deepest one when
/// projects nest: a hand-off from a checkout lands in that checkout's
/// project without naming it.
fn project_for(spaces: &[Space], device: &str, cwd: &Path) -> Option<String> {
    spaces
        .iter()
        .filter(|space| space.device_id == device && !space.path.is_empty())
        .filter(|space| cwd.starts_with(Path::new(&space.path)))
        .max_by_key(|space| space.path.len())
        .map(|space| space.id.clone())
}

/// The `create_chat` arguments: the device is always named, so a project
/// that exists on several computers resolves to the one asked for (this
/// computer by default).
fn request(options: &Options, device: &str, project: Option<&str>) -> Value {
    let mut args = json!({ "device": device });
    for (key, value) in [
        ("project", project),
        ("harness", options.harness.as_deref()),
        ("model", options.model.as_deref()),
        ("title", options.title.as_deref()),
        ("prompt", options.brief.as_deref()),
    ] {
        if let Some(value) = value {
            args[key] = json!(value);
        }
    }
    args
}

async fn open(
    engine: Arc<OpenAgents>,
    options: &Options,
    cwd: Option<PathBuf>,
) -> Result<Value, String> {
    let device = engine
        .resolve_device_id(options.device.as_deref())
        .await
        .map_err(|error| error.to_string())?;
    let project = match (&options.project, cwd) {
        (Some(project), _) => Some(project.clone()),
        (None, Some(cwd)) => {
            let spaces = engine.spaces().await.map_err(|error| error.to_string())?;
            project_for(&spaces, &device, &cwd)
        }
        (None, None) => None,
    };
    let tools = Tools::new(engine);
    tools
        .call("create_chat", request(options, &device, project.as_deref()))
        .await
}

pub(super) fn run(output: &Output, words: &[String], usage: &str) -> u8 {
    let options = match parse(words) {
        Ok(options) => options,
        Err(message) => return output.usage("chat", &message, usage),
    };
    let port = match options.port {
        Some(port) => port,
        None => match openagents_brand::env_var("OPENAGENTS_IPC_PORT") {
            Err(_) => openagents_brand::DEFAULT_IPC_PORT,
            Ok(value) => match value.parse::<u16>().ok().filter(|port| *port != 0) {
                Some(port) => port,
                None => {
                    return output.usage(
                        "chat",
                        "OPENAGENTS_IPC_PORT needs a port from 1 to 65535",
                        usage,
                    );
                }
            },
        },
    };
    // A chat opened from inside an app chat records where it came from.
    let engine = Arc::new(OpenAgents::new(
        format!("ws://127.0.0.1:{port}"),
        Origin::from_env(),
    ));
    let cwd = std::env::current_dir().ok();
    let result = crate::runtime().block_on(async {
        tokio::time::timeout(TIMEOUT, open(engine, &options, cwd))
            .await
            .unwrap_or_else(|_| Err("the OpenAgents app did not answer within 60 seconds".into()))
    });
    match result {
        Ok(value) => {
            let computer = options.device.as_deref().unwrap_or("this computer");
            output.emit(&value, |value| render(value, computer));
            0
        }
        Err(message) => output.fail("chat", &format!("couldn't open an app chat: {message}")),
    }
}

fn render(value: &Value, computer: &str) -> String {
    let id = value["chatId"].as_str().unwrap_or_default();
    let place = match value["project"]["name"].as_str() {
        Some(name) => format!("in {name}"),
        None => "with no project".to_owned(),
    };
    let mut lines = vec![
        id.to_owned(),
        format!(
            "Opened an app chat {place} on {computer}. It shows in the app's Sessions, on your phone, and to Foreman."
        ),
    ];
    if value.get("sent").is_some() {
        lines.push("Your brief is its first message; it is working on it now.".into());
    } else {
        lines.push("It has no message yet; send it one from the app or your phone.".into());
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| (*word).to_owned()).collect()
    }

    fn space(id: &str, device: &str, path: &str) -> Space {
        serde_json::from_value(json!({
            "id": id, "deviceId": device, "path": path,
            "gitDetected": true, "createdAt": "2026-10-17T00:00:00Z",
        }))
        .unwrap()
    }

    #[test]
    fn the_brief_is_the_words_and_options_are_trimmed() {
        let options = parse(&words(&[
            "--project",
            "openagents",
            "--harness",
            "claude-code",
            "coordinate",
            "#12028",
        ]))
        .unwrap();
        assert_eq!(options.brief.as_deref(), Some("coordinate #12028"));
        assert_eq!(options.project.as_deref(), Some("openagents"));
        assert_eq!(options.harness.as_deref(), Some("claude-code"));
        assert_eq!(parse(&[]).unwrap(), Options::default());
        assert!(parse(&words(&["--ipc-port", "0"])).is_err());
        assert!(parse(&words(&["--thread", "x"])).is_err());
    }

    #[test]
    fn the_request_always_names_the_device_and_omits_what_was_not_given() {
        let options = Options {
            brief: Some("brief".into()),
            model: Some("m".into()),
            ..Options::default()
        };
        assert_eq!(
            request(&options, "dev-1", Some("space-1")),
            json!({"device": "dev-1", "project": "space-1", "model": "m", "prompt": "brief"})
        );
        assert_eq!(
            request(&Options::default(), "dev-1", None),
            json!({"device": "dev-1"})
        );
    }

    #[test]
    fn the_checkout_picks_the_deepest_project_on_this_computer() {
        let spaces = [
            space("work", "here", "/Users/me/work"),
            space("oa", "here", "/Users/me/work/openagents"),
            space("oa-other", "other", "/Users/me/work/openagents/apps"),
        ];
        let at = |path: &str| project_for(&spaces, "here", Path::new(path));
        assert_eq!(
            at("/Users/me/work/openagents/apps/x").as_deref(),
            Some("oa")
        );
        assert_eq!(at("/Users/me/work/notes").as_deref(), Some("work"));
        assert_eq!(at("/Users/me/work/openagents-wt").as_deref(), Some("work"));
        assert_eq!(at("/tmp"), None);
    }

    #[test]
    fn the_receipt_names_the_chat_first() {
        let value = json!({"chatId": "c-1", "deviceId": "d-1", "project": {"name": "openagents"}, "sent": {}});
        let text = render(&value, "this computer");
        assert!(text.starts_with("c-1\n"));
        assert!(text.contains("in openagents"));
        assert!(oa_copy::violations(&text, &[]).is_empty());
    }
}

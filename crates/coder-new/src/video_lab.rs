//! Video Lab tools in Coder (docs/video-lab/README.md, docs/video-lab/mvp.md).
//!
//! The same typed tools the web chat offers (`lab_open`, `lab_show`,
//! `lab_frame`, `lab_edit`, `lab_render`, `lab_compare`, `lab_notes`,
//! `lab_versions`, `lab_list`), run by the Video Lab service on this Mac
//! (scripts/film/lab.py, `http://127.0.0.1:8792`). Coder in the terminal
//! answers in words and gives the Lab player link for anything to watch;
//! the video plays there or in the web chat's card.
//!
//! The tools are offered only where a Lab is: `VIDEO_LAB_URL` is set, or
//! the Lab script is on this computer (`OPENAGENTS_VIDEO_LAB_SCRIPT`, the
//! checkout this Coder was built from, or ~/work/openagents). A Lab that
//! isn't running is started from that script, so the tools always work.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};

/// The tools, with their parameters: (name, description, [(param, description, required)]).
const TOOLS: &[(&str, &str, &[(&str, &str, bool)])] = &[
    (
        "lab_list",
        "List the Video Lab documents (films, shots, scenes) on this computer.",
        &[],
    ),
    (
        "lab_open",
        "Open a Video Lab document: its latest version (or the one asked for), its shots, any problems, the versions, and the Lab player link.",
        &[(DOC, DOC_HELP, true), ("version", VERSION_HELP, false)],
    ),
    (
        "lab_show",
        "Show a time range of a document: which shots and captions are there, and a preview rendered for the Lab player starting there. Times are m:ss or seconds.",
        &[
            (DOC, DOC_HELP, true),
            ("from", "start time, m:ss or seconds", false),
            ("to", "end time, m:ss or seconds", false),
            ("version", VERSION_HELP, false),
        ],
    ),
    (
        "lab_frame",
        "One still at a time, for looking closely; its file path comes back.",
        &[
            (DOC, DOC_HELP, true),
            ("at", "a time, m:ss or seconds", true),
            ("version", VERSION_HELP, false),
        ],
    ),
    (
        "lab_edit",
        "Change a document by a JSON Patch (RFC 6902) on its JSON, making a new version. Address a cut's shot by name: /shots/@s4-delegation/dur. In a cut, captions move with their shots; shot lengths are whole beats (multiples of 0.5 s at 120 BPM). The edit is checked first and refused with the reason if it adds a problem; then fix the patch. A preview renders.",
        &[
            (DOC, DOC_HELP, true),
            (
                "patch",
                "the JSON Patch as a JSON string, e.g. [{\"op\":\"replace\",\"path\":\"/shots/@s4-delegation/dur\",\"value\":12.5}]",
                true,
            ),
            (
                "summary",
                "one line in plain words saying what the edit changes",
                true,
            ),
            ("version", VERSION_HELP, false),
        ],
    ),
    (
        "lab_render",
        "Render a version: preview (quick, 960x540), review (1080p) or master (full size).",
        &[
            (DOC, DOC_HELP, true),
            ("quality", "preview, review or master", false),
            ("version", VERSION_HELP, false),
        ],
    ),
    (
        "lab_compare",
        "Two versions side by side at a time (default: the latest and the one before it).",
        &[
            (DOC, DOC_HELP, true),
            ("at", "a time, m:ss or seconds", false),
            ("a", "the first version (default: the one before b)", false),
            ("b", "the second version (default: the latest)", false),
        ],
    ),
    (
        "lab_notes",
        "Read the owner's review notes on a document's videos.",
        &[(DOC, DOC_HELP, true), ("version", VERSION_HELP, false)],
    ),
    (
        "lab_versions",
        "List a document's versions: who made each and what it changed.",
        &[(DOC, DOC_HELP, true)],
    ),
];

const DOC: &str = "document";
const DOC_HELP: &str = "the document's id or title, e.g. episode-290";
const VERSION_HELP: &str = "a version number like 3 (default: the latest)";

/// What Coder's model is told when the tools are on.
pub const INSTRUCTIONS: &str = "The lab_* tools see and edit the owner's films, shots and scenes in Video Lab on this computer: open a document, show a time range, look at a frame, edit it with a JSON Patch (read it first; a refused edit says why, so fix the patch), render, compare versions, and read the review notes. The terminal can't play video: end your answer with the Lab player link the tool returned (`player`), so the owner can watch it.\n";

/// Whether `name` is a Video Lab tool.
#[must_use]
pub fn is_tool(name: &str) -> bool {
    TOOLS.iter().any(|(tool, ..)| *tool == name)
}

/// The Lab's address.
fn url() -> String {
    std::env::var("VIDEO_LAB_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8792".into())
        .trim_end_matches('/')
        .to_owned()
}

/// The Lab script on this computer, if there is one.
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

/// Whether the tools are offered here.
#[must_use]
pub fn available() -> bool {
    std::env::var("VIDEO_LAB_URL").is_ok_and(|u| !u.trim().is_empty()) || script().is_some()
}

/// The declarations, in Coder's tool format.
#[must_use]
pub fn definitions() -> Vec<Value> {
    TOOLS
        .iter()
        .map(|(name, description, params)| {
            let mut properties = serde_json::Map::new();
            let mut required = Vec::new();
            for (param, help, needed) in *params {
                properties.insert((*param).into(), json!({"type": "string", "description": help}));
                if *needed {
                    required.push(*param);
                }
            }
            json!({"type": "function", "function": {
                "name": name,
                "description": description,
                "parameters": {"type": "object", "properties": properties, "required": required, "additionalProperties": false},
            }})
        })
        .collect()
}

async fn up(http: &reqwest::Client, base: &str) -> bool {
    http.get(format!("{base}/healthz"))
        .timeout(Duration::from_secs(1))
        .send()
        .await
        .is_ok_and(|r| r.status().is_success())
}

/// Starts the Lab from its script when it isn't answering.
async fn ensure(http: &reqwest::Client, base: &str) -> Result<(), String> {
    if up(http, base).await {
        return Ok(());
    }
    let Some(script) = script() else {
        return Err(format!(
            "Video Lab isn't running at {base}, and its script isn't on this computer."
        ));
    };
    let log_dir = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("work/film-out/lab");
    std::fs::create_dir_all(&log_dir).map_err(|e| e.to_string())?;
    let log = std::fs::File::create(log_dir.join("serve.log")).map_err(|e| e.to_string())?;
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
        tokio::time::sleep(Duration::from_millis(250)).await;
        if up(http, base).await {
            return Ok(());
        }
    }
    Err(format!(
        "Video Lab didn't start; see {}",
        log_dir.join("serve.log").display()
    ))
}

/// Run one Lab tool.
///
/// # Errors
/// The Lab not answering or refusing the call (with its reason).
pub async fn execute(name: &str, arguments: Value) -> Result<Value, String> {
    if !is_tool(name) {
        return Err(format!("{name} isn't a Video Lab tool"));
    }
    let base = url();
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;
    ensure(&http, &base).await?;
    let mut result: Value = http
        .post(format!("{base}/api/tool/{name}"))
        .json(&arguments)
        .send()
        .await
        .map_err(|e| format!("Video Lab didn't answer ({})", e.without_url()))?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(error) = result.get("error").and_then(Value::as_str)
        && result.get("refused").is_none()
    {
        return Err(error.to_owned());
    }
    // The card is for screens that draw it; the terminal gets its link.
    if let Some(card) = result.get("card").cloned()
        && let Some(object) = result.as_object_mut()
    {
        object.remove("card");
        object.insert("player".into(), card["player"].clone());
        object.insert("version".into(), card["version"].clone());
        if let Some(state) = card["job"]["state"].as_str() {
            object.insert("render".into(), json!(state));
        }
    }
    if let Some(object) = result.as_object_mut() {
        object.remove("documents");
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tool_is_declared_with_its_required_document() {
        let defs = definitions();
        assert_eq!(defs.len(), TOOLS.len());
        for def in &defs {
            let name = def["function"]["name"].as_str().unwrap();
            assert!(name.starts_with("lab_") && is_tool(name));
            if name != "lab_list" {
                assert!(
                    def["function"]["parameters"]["required"]
                        .as_array()
                        .unwrap()
                        .contains(&json!("document")),
                    "{name}"
                );
            }
        }
        let edit = defs
            .iter()
            .find(|d| d["function"]["name"] == "lab_edit")
            .unwrap();
        let required = edit["function"]["parameters"]["required"]
            .as_array()
            .unwrap();
        assert!(required.contains(&json!("patch")) && required.contains(&json!("summary")));
    }

    #[test]
    fn the_tools_match_the_lab_script() {
        // The Lab (scripts/film/lab.py) serves the same tools to the web chat.
        let script = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/film/lab.py"
        ))
        .expect("the Lab script");
        for (name, ..) in TOOLS {
            assert!(
                script.contains(&format!("\"{name}\": (tool_")),
                "{name} is not in lab.py"
            );
        }
        assert!(!is_tool("lab_delete"));
    }
}

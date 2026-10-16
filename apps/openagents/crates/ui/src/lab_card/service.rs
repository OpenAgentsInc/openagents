//! Starting the Video Lab on this computer when a card needs it.
//!
//! The Lab is `scripts/film/lab.py serve` in the OpenAgents repository. A
//! card's Play and "Open in the Lab player" first make sure it answers, and
//! start it when it doesn't, the same way the chat's Lab tools do
//! (`zeron_mcp::video_lab`).

use std::path::PathBuf;
use std::time::Duration;

/// The Lab script on this computer: `OPENAGENTS_VIDEO_LAB_SCRIPT`, the
/// repository this app was built from, then `~/work/openagents`.
pub fn script() -> Option<PathBuf> {
    let from_exe = std::env::current_exe().ok().and_then(|exe| {
        exe.ancestors()
            .map(|dir| dir.join("scripts/film/lab.py"))
            .find(|path| path.is_file())
    });
    [
        std::env::var_os("OPENAGENTS_VIDEO_LAB_SCRIPT").map(PathBuf::from),
        from_exe,
        std::env::var_os("HOME").map(|h| PathBuf::from(h).join("work/openagents/scripts/film/lab.py")),
    ]
    .into_iter()
    .flatten()
    .find(|path| path.is_file())
}

fn healthy(base: &str) -> bool {
    super::get(&format!("{base}/healthz")).is_some()
}

/// Make sure the Lab at `base` answers, starting it when needed. The error
/// is a sentence for the card.
pub fn ensure(base: &str) -> Result<(), String> {
    if healthy(base) {
        return Ok(());
    }
    let script = script().ok_or(
        "Video Lab isn't running on this Mac, and its script (scripts/film/lab.py) isn't here.",
    )?;
    let logs = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join("work/film-out/lab");
    std::fs::create_dir_all(&logs).map_err(|e| e.to_string())?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("serve.log"))
        .map_err(|e| e.to_string())?;
    let err = log.try_clone().map_err(|e| e.to_string())?;
    let mut command = std::process::Command::new("python3");
    command
        .arg(&script)
        .arg("serve")
        .stdin(std::process::Stdio::null())
        .stdout(log)
        .stderr(err);
    if let Some(repo) = script.ancestors().nth(3) {
        command.current_dir(repo);
    }
    // Its own process group: the Lab keeps running after the app, a chat's
    // tool server, or the shell that started it exits.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    command
        .spawn()
        .map_err(|e| format!("Video Lab couldn't start ({e})."))?;
    for _ in 0..60 {
        std::thread::sleep(Duration::from_millis(250));
        if healthy(base) {
            return Ok(());
        }
    }
    Err(format!(
        "Video Lab didn't start; see {}.",
        logs.join("serve.log").display()
    ))
}

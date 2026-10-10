//! Screenshots and files asked for on openagents.com (#11185).
//!
//! A Coder chat's page on the website offers Screenshot and Copy a file
//! while Coder here is online. The ask waits on the website; Coder takes
//! it with the chat's replies ([`coder_sync::Event::Asks`]), and this runs
//! it through this computer's own host, exactly as `openagents computer
//! screenshot` and `openagents computer pull` would here, then sends back
//! the bytes or why it couldn't ([`coder_sync::Job::Capture`]). The
//! website never reaches this computer; it only answers what Coder asks,
//! and the host checks this device's grant as for any other call.
//!
//! The host is the one serving on this computer: the paired host whose
//! route is loopback (`--same-machine`), else the one labelled with this
//! computer's name, and it must grant `terminal`.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

use coder_sync::{Ask, AskAction, Job, MAX_CAPTURE_BYTES};
use serde_json::Value;

/// What Coder says when no host here can take the ask.
pub const NO_HOST: &str = "No host on this computer lets Coder take screenshots or copy files. Start one with `openagents host serve`, then pair this computer's command line with it: `openagents computer link CODE`.";

/// Run `asks` from chat `session` in the background and queue what came of
/// each on `jobs`.
pub(crate) fn run(session: String, asks: Vec<Ask>, computer: String, jobs: mpsc::Sender<Job>) {
    std::thread::spawn(move || {
        let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        else {
            return;
        };
        for ask in asks {
            let result = runtime.block_on(capture(&ask.action, &computer));
            let _ = jobs.send(Job::Capture {
                session: session.clone(),
                ask: ask.id,
                result,
            });
        }
    });
}

/// One ask, through this computer's host: the bytes, or why not.
async fn capture(action: &AskAction, computer: &str) -> Result<Vec<u8>, String> {
    let program = crate::bundled_runtime::cli_binary().ok_or_else(|| {
        format!(
            "The OpenAgents command line is missing here. Reinstall Coder with `{}`.",
            crate::account::INSTALL_COMMAND
        )
    })?;
    let scratch = std::env::temp_dir().join("openagents-web-asks");
    std::fs::create_dir_all(&scratch).map_err(|_| "Couldn't make a scratch folder.".to_owned())?;
    let host = local_host(&program, &scratch, computer).await?;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos());
    let out = scratch.join(format!("capture-{nanos}"));
    let mut words: Vec<String> = vec!["computer".into()];
    match action {
        AskAction::Screenshot => words.extend([
            "screenshot".into(),
            host,
            "--out".into(),
            out.display().to_string(),
        ]),
        AskAction::Pull { path } => words.extend([
            "pull".into(),
            host,
            path.clone(),
            out.display().to_string(),
            "--max-bytes".into(),
            MAX_CAPTURE_BYTES.to_string(),
        ]),
    }
    words.push("--same-machine".into());
    let ran = cli(&program, &words, &scratch).await;
    let bytes = match ran {
        Ok(_) => std::fs::read(&out).map_err(|_| "The computer sent nothing back.".to_owned()),
        Err(message) => Err(message),
    };
    let _ = std::fs::remove_file(&out);
    bytes
}

/// Run the CLI; its parsed last JSON line when it exits 0, else its words.
async fn cli(program: &Path, words: &[String], cwd: &Path) -> Result<Value, String> {
    let cancel = Arc::new(AtomicBool::new(false));
    let result =
        crate::bundled_runtime::cli_checked(program, words, cwd, &cancel, &mut |_| {}).await?;
    let stdout = result["stdout"].as_str().unwrap_or_default();
    let last: Option<Value> = stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str(line.trim()).ok());
    if result["exit"].as_i64() == Some(0) {
        return Ok(last.unwrap_or(Value::Null));
    }
    Err(last
        .as_ref()
        .and_then(|value| value["error"].as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| "The computer couldn't do this.".to_owned()))
}

/// The host serving on this computer that grants this device `terminal`.
async fn local_host(program: &Path, cwd: &Path, computer: &str) -> Result<String, String> {
    let words: Vec<String> = ["computer", "list", "--wait", "5", "--same-machine"]
        .into_iter()
        .map(str::to_owned)
        .collect();
    let listed = cli(program, &words, cwd)
        .await
        .map_err(|_| NO_HOST.to_owned())?;
    pick(&listed, computer).ok_or_else(|| NO_HOST.to_owned())
}

/// From `openagents computer list`: the loopback host, else the one
/// labelled `computer`, among those granting `terminal`.
fn pick(listed: &Value, computer: &str) -> Option<String> {
    let usable: Vec<&Value> = listed["hosts"]
        .as_array()?
        .iter()
        .filter(|host| {
            host["rights_now"]
                .as_str()
                .is_some_and(|rights| rights.split(',').any(|right| right == "terminal"))
        })
        .collect();
    usable
        .iter()
        .find(|host| host["route"] == "loopback")
        .or_else(|| {
            usable
                .iter()
                .find(|host| !computer.is_empty() && host["label"] == computer)
        })
        .and_then(|host| host["key"].as_str())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_host_on_this_computer_is_the_loopback_one_that_grants_terminal() {
        let listed = json!({"hosts": [
            {"key": "a", "label": "Studio", "route": "relay", "rights_now": "observe,terminal"},
            {"key": "b", "label": "Other", "route": "loopback", "rights_now": "observe"},
            {"key": "c", "label": "Here", "route": "loopback", "rights_now": "observe,terminal"},
        ]});
        assert_eq!(pick(&listed, "Studio").as_deref(), Some("c"));
        let no_loopback = json!({"hosts": [
            {"key": "a", "label": "Studio", "route": "relay", "rights_now": "observe,terminal"},
        ]});
        assert_eq!(pick(&no_loopback, "Studio").as_deref(), Some("a"));
        assert_eq!(pick(&no_loopback, "Laptop"), None);
        assert_eq!(pick(&json!({"hosts": []}), "Studio"), None);
    }
}

//! Earn on a paired computer (#12294): the `Pylon*` methods, each one run of
//! this computer's `openagents pylon` command, so the Earn page in a browser
//! reads and changes what the desktop's own Earn page does.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use openagents_mcp::pylon::{cli, run_args};
use openagents_rpc::pylon::{
    PYLON_DOWN, PYLON_EARNINGS, PYLON_SET_PAUSE, PYLON_SETTINGS, PYLON_STATUS, PYLON_UP, SetPause,
};
use openagents_rpc::{RpcError, RpcReply, parse_params};
use serde_json::{Value, json};

/// How long a read or a change may take.
const TIMEOUT: Duration = Duration::from_secs(60);
/// How long `up` is awaited before it answers that it still runs.
const UP_WAIT: Duration = Duration::from_secs(15);
/// How long `up` may set up in the background: the model download.
const UP_LIMIT: Duration = Duration::from_secs(6 * 60 * 60);

/// Whether an `up` is still running in the background.
static STARTING: AtomicBool = AtomicBool::new(false);

pub(super) use openagents_rpc::pylon::handles;

pub(super) async fn handle(method: &str, params: Value) -> Result<RpcReply, RpcError> {
    let exe = cli().ok_or_else(|| {
        RpcError::Failed("The OpenAgents command line isn't installed on this computer.".into())
    })?;
    RpcReply::value(&answer(&exe, method, params, UP_WAIT).await?)
}

async fn answer(
    exe: &Path,
    method: &str,
    params: Value,
    up_wait: Duration,
) -> Result<Value, RpcError> {
    let failed = |e: String| RpcError::Failed(e);
    match method {
        PYLON_STATUS => run_args(exe, &["status"], TIMEOUT).await.map_err(failed),
        PYLON_EARNINGS => run_args(exe, &["earnings"], TIMEOUT).await.map_err(failed),
        PYLON_SETTINGS => run_args(exe, &["settings"], TIMEOUT).await.map_err(failed),
        PYLON_DOWN => run_args(exe, &["down"], TIMEOUT).await.map_err(failed),
        PYLON_SET_PAUSE => {
            let set: SetPause = parse_params(params)?;
            run_args(exe, &set.args(), TIMEOUT).await.map_err(failed)
        }
        PYLON_UP => up(exe, up_wait).await,
        _ => Err(RpcError::Failed("Choose an Earn operation.".into())),
    }
}

/// Start `up` in the background and wait `wait` for it: it answers then,
/// or `{running: true}` while it still sets up. One runs at a time.
async fn up(exe: &Path, wait: Duration) -> Result<Value, RpcError> {
    if STARTING.swap(true, Ordering::SeqCst) {
        return Ok(json!({ "running": true }));
    }
    let (tx, rx) = tokio::sync::oneshot::channel();
    let exe = exe.to_path_buf();
    tokio::spawn(async move {
        let result = run_args(&exe, &["up", "--yes"], UP_LIMIT).await;
        STARTING.store(false, Ordering::SeqCst);
        let _ = tx.send(result);
    });
    match tokio::time::timeout(wait, rx).await {
        Ok(Ok(result)) => result.map_err(RpcError::Failed),
        Ok(Err(_)) => Err(RpcError::Failed("Sharing didn't start.".into())),
        Err(_) => Ok(json!({ "running": true })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in `openagents` that answers with its arguments, and sleeps
    /// on `up`.
    fn fake(dir: &Path) -> std::path::PathBuf {
        let exe = dir.join("openagents");
        std::fs::write(
            &exe,
            "#!/bin/sh\nif [ \"$3\" = up ]; then echo '{\"step\":\"model\"}'; [ -f \"$0.slow\" ] && sleep 3; fi\n\
             printf '{\"args\":\"%s\"}\\n' \"$*\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        exe
    }

    #[tokio::test]
    async fn each_method_runs_its_pylon_command() {
        let dir = tempfile::tempdir().unwrap();
        let exe = fake(dir.path());
        let read = answer(&exe, PYLON_EARNINGS, Value::Null, UP_WAIT)
            .await
            .unwrap();
        assert_eq!(read["document"]["args"], "--json pylon earnings");
        assert_eq!(read["exitCode"], 0);
        let set = answer(
            &exe,
            PYLON_SET_PAUSE,
            json!({"pause": "battery", "on": true}),
            UP_WAIT,
        )
        .await
        .unwrap();
        assert_eq!(
            set["document"]["args"],
            "--json pylon settings --pause-on-battery on"
        );
        assert!(
            answer(&exe, PYLON_SET_PAUSE, json!({"pause": "cold"}), UP_WAIT)
                .await
                .is_err()
        );
        // A quick `up` answers with its last document, after its progress.
        let up = answer(&exe, PYLON_UP, Value::Null, UP_WAIT).await.unwrap();
        assert_eq!(up["document"]["args"], "--json pylon up --yes");
        // A slow one answers that it still runs, and a second waits on it.
        std::fs::write(dir.path().join("openagents.slow"), "").unwrap();
        let wait = Duration::from_millis(300);
        let first = answer(&exe, PYLON_UP, Value::Null, wait).await.unwrap();
        assert_eq!(first, json!({"running": true}));
        let second = answer(&exe, PYLON_UP, Value::Null, wait).await.unwrap();
        assert_eq!(second, json!({"running": true}));
    }
}

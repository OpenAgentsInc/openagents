//! An optional harness checkpoint between sessions. The harness collects
//! task-declared artifacts while the executor waits; it never returns grades.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub const ENV: &str = "CODER_ONE_CANDIDATE_CHECKPOINT";
pub const VERSION: &str = "candidate-checkpoint-v1";

pub async fn capture(root: &Path, dispatch: &str, session: u32) -> Result<Value, String> {
    capture_with_timeout(root, dispatch, session, Duration::from_secs(180)).await
}

async fn capture_with_timeout(
    root: &Path,
    dispatch: &str,
    session: u32,
    timeout: Duration,
) -> Result<Value, String> {
    if !root.is_absolute()
        || !dispatch.starts_with("lean-")
        || !dispatch[5..].parse::<u32>().is_ok_and(|number| number > 0)
        || session == 0
    {
        return Err("invalid candidate checkpoint identity".into());
    }
    let id = format!("{dispatch}-session-{session}");
    let request = root.join(format!("{id}.request.json"));
    let response = root.join(format!("{id}.response.json"));
    if request.exists() || response.exists() {
        return Err("candidate checkpoint identity was already used".into());
    }
    let bytes = serde_json::to_vec(&json!({"schema": VERSION, "id": id}))
        .map_err(|error| error.to_string())?;
    crate::record::write_atomic(&request, &bytes).map_err(|error| error.to_string())?;
    let started = Instant::now();
    loop {
        if let Ok(bytes) = std::fs::read(&response) {
            let value: Value = serde_json::from_slice(&bytes).map_err(|error| error.to_string())?;
            let sealed = value["receipt_digest"].as_str().is_some_and(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            });
            if value["schema"] != VERSION
                || value["id"] != id
                || value["complete"] != true
                || !sealed
            {
                return Err(
                    "candidate collection did not complete; see the harness receipt".into(),
                );
            }
            return Ok(value);
        }
        if started.elapsed() >= timeout {
            return Err("candidate collection timed out; stop before another session".into());
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn acknowledgement_is_required_and_cannot_be_reused() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_path_buf();
        let writer = root.clone();
        let worker = tokio::spawn(async move {
            while !writer.join("lean-1-session-1.request.json").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            crate::record::write_atomic(
                &writer.join("lean-1-session-1.response.json"),
                br#"{"schema":"candidate-checkpoint-v1","id":"lean-1-session-1","complete":true,"receipt_digest":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}"#,
            )
            .unwrap();
        });
        assert!(capture(&root, "lean-1", 1).await.is_ok());
        worker.await.unwrap();
        assert!(capture(&root, "lean-1", 1).await.is_err());
        assert!(
            capture_with_timeout(&root, "lean-1", 2, Duration::ZERO)
                .await
                .is_err()
        );
        assert!(capture(&root, "../outside", 1).await.is_err());
        assert!(capture(&root, "lean-0", 1).await.is_err());
    }

    #[tokio::test]
    async fn an_incomplete_capture_stops_before_the_next_session() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().to_path_buf();
        let writer = root.clone();
        let worker = tokio::spawn(async move {
            while !writer.join("lean-1-session-1.request.json").exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            crate::record::write_atomic(
                &writer.join("lean-1-session-1.response.json"),
                br#"{"schema":"candidate-checkpoint-v1","id":"lean-1-session-1","complete":false}"#,
            )
            .unwrap();
        });
        assert!(
            capture(&root, "lean-1", 1)
                .await
                .unwrap_err()
                .contains("did not complete")
        );
        worker.await.unwrap();
    }
}

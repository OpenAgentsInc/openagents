//! One bounded JSON-line session per supervised process.
mod actions;
mod session;
mod state;
use anyhow::{Result, bail, ensure};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{BufRead, Write};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

pub const MAX_LINE: usize = 1 << 20;
#[derive(Deserialize)]
struct Request {
    id: u64,
    op: String,
    #[serde(default)]
    args: Value,
}

pub fn emit(value: Value) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{value}");
    let _ = out.flush();
}

fn bounded_seconds(args: &Value, default: u64) -> Result<u64> {
    let n = match args.get("seconds") {
        Some(v) => v
            .as_u64()
            .ok_or_else(|| anyhow::anyhow!("seconds must be an integer"))?,
        None => default,
    };
    ensure!(n <= 120, "seconds exceeds 120");
    Ok(n)
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("--version") {
        println!(
            "wow-bridge {} benilla cf891dc3756a",
            env!("CARGO_PKG_VERSION")
        );
        return;
    }
    let (tx, rx) = mpsc::sync_channel::<Result<Request>>(16);
    let closed = Arc::new(AtomicBool::new(false));
    let flag = closed.clone();
    std::thread::spawn(move || {
        let mut input = std::io::stdin().lock();
        loop {
            let mut line = Vec::new();
            use std::io::Read;
            let read = input
                .by_ref()
                .take(MAX_LINE as u64 + 1)
                .read_until(b'\n', &mut line);
            match read {
                Ok(0) | Err(_) => break,
                Ok(_) if line.len() > MAX_LINE => {
                    let _ = tx.send(Err(anyhow::anyhow!("request exceeds one megabyte")));
                    break;
                }
                _ => {
                    let value = serde_json::from_slice(&line)
                        .map_err(|_| anyhow::anyhow!("invalid request"));
                    if tx.send(value).is_err() {
                        break;
                    }
                }
            }
        }
        flag.store(true, Ordering::Relaxed);
    });
    let mut live: Option<session::Live> = None;
    let mut joined = false;
    loop {
        let req = match rx.recv_timeout(Duration::from_millis(50)) {
            Ok(Ok(r)) => r,
            Ok(Err(e)) => {
                emit(json!({"id":0,"ok":false,"code":"invalid_request","error":e.to_string()}));
                break;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(s) = &mut live {
                    if let Err(e) = s.pump(Duration::ZERO) {
                        emit(json!({"event":"server_exit","reason":e.to_string()}));
                        break;
                    }
                }
                continue;
            }
            Err(_) => break,
        };
        let result = (|| -> Result<Value> {
            match req.op.as_str() {
                "join" => {
                    ensure!(!joined, "one session per helper; start a new process");
                    joined = true;
                    live = Some(session::Live::join(&req.args, closed.clone())?);
                    Ok(live.as_ref().unwrap().state.observation(40.0))
                }
                "cleanup" => live
                    .as_mut()
                    .ok_or_else(|| anyhow::anyhow!("join first"))?
                    .cleanup(),
                "shutdown" | "disconnect" => {
                    if let Some(s) = &mut live {
                        s.disconnect()?;
                    }
                    Ok(json!({"disconnected":true}))
                }
                op => {
                    let s = live.as_mut().ok_or_else(|| anyhow::anyhow!("join first"))?;
                    ensure!(s.connected(), "session is disconnected");
                    match op {
                        "state" => {
                            s.pump(Duration::from_millis(100))?;
                            let radius = req
                                .args
                                .get("radius")
                                .and_then(Value::as_f64)
                                .unwrap_or(40.0);
                            ensure!(
                                radius.is_finite() && (0.0..=100.0).contains(&radius),
                                "radius must be between 0 and 100"
                            );
                            Ok(s.state.observation(radius as f32))
                        }
                        "say" => {
                            let text = req.args["text"]
                                .as_str()
                                .ok_or_else(|| anyhow::anyhow!("text required"))?;
                            ensure!(text.len() <= 255, "chat exceeds 255 bytes");
                            ensure!(
                                !text.trim_start().starts_with(['.', '!']),
                                "GM commands are setup-only"
                            );
                            s.writer.send_chat(text)?;
                            s.pump(Duration::from_millis(100))?;
                            Ok(json!({"sent":true}))
                        }
                        "wait" => {
                            let seconds = bounded_seconds(&req.args, 2)?;
                            s.pump(Duration::from_secs(seconds))?;
                            Ok(s.state.observation(40.0))
                        }
                        "target" | "attack" | "cast" | "loot" | "quest" | "use" | "vendor" => {
                            s.action(op, &req.args, bounded_seconds(&req.args, 30)?)
                        }
                        "goto" => s.goto(&req.args, bounded_seconds(&req.args, 60)?),
                        "gm" => {
                            ensure!(s.setup, "GM commands are setup-only");
                            let text = req.args["command"]
                                .as_str()
                                .ok_or_else(|| anyhow::anyhow!("command required"))?;
                            ensure!(
                                text.starts_with('.') && text.len() <= 255,
                                "invalid setup command"
                            );
                            s.setup_feedback.clear();
                            s.writer.send_chat(text)?;
                            s.pump(Duration::from_millis(500))?;
                            ensure!(
                                !s.setup_feedback.iter().any(|line| {
                                    let line = line.to_ascii_lowercase();
                                    line.contains("command is not available")
                                        || line.contains("there is no such command")
                                        || line.starts_with("syntax")
                                        || line.starts_with("you cannot")
                                        || line.starts_with("failed")
                                }),
                                "setup command refused: {:?}",
                                s.setup_feedback
                            );
                            Ok(json!({"sent":true,"feedback":s.setup_feedback}))
                        }
                        _ => bail!("unknown operation"),
                    }
                }
            }
        })();
        match result {
            Ok(v) => emit(json!({"id":req.id,"ok":true,"result":v})),
            Err(e) => emit(json!({"id":req.id,"ok":false,"code":"refused","error":e.to_string()})),
        }
        if req.op == "shutdown" {
            break;
        }
    }
    if let Some(s) = &mut live {
        let _ = s.disconnect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn operation_time_is_bounded() {
        assert!(bounded_seconds(&json!({"seconds":121}), 2).is_err());
        assert!(bounded_seconds(&json!({"seconds":-1}), 2).is_err());
        assert!(bounded_seconds(&json!({"seconds":1.5}), 2).is_err());
    }
}

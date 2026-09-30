//! A stand-in agent that replays a recorded ACP conversation, for tests.
//!
//! A recording is the agent's side of a real session, one JSON-RPC frame
//! per line, as `fixtures/devin-3000.11.3-turn.jsonl` holds Devin's. It is
//! cut into blocks, each ending with the reply to one client request: the
//! first block answers request 1, the second request 2, and so on, which is
//! how [`crate::client::Client`] numbers its requests. The stand-in is a
//! `/bin/sh` script: each time it reads a client request, it writes the
//! next block. A block without a reply leaves the request open until the
//! client sends `session/cancel`, which the script answers with a
//! `cancelled` turn.

use std::path::Path;
#[cfg(unix)]
use std::path::PathBuf;

use serde_json::Value;

/// Cut a recording into blocks, each through the reply that ends it. A
/// trailing block with no reply is kept, so a test can leave a turn open.
///
/// # Panics
/// A line that is not JSON.
#[must_use]
pub fn blocks(recording: &str) -> Vec<Vec<Value>> {
    let mut blocks = Vec::new();
    let mut block = Vec::new();
    for line in recording.lines().filter(|line| !line.trim().is_empty()) {
        let frame: Value = serde_json::from_str(line).expect("a recorded frame");
        let reply = frame.get("method").is_none() && frame.get("id").is_some();
        block.push(frame);
        if reply {
            blocks.push(std::mem::take(&mut block));
        }
    }
    if !block.is_empty() {
        blocks.push(block);
    }
    blocks
}

/// Write the stand-in for `blocks` into `dir` and return its path. The
/// script writes its arguments to `dir/arguments`, one a line, and appends
/// each line it reads to `dir/received.jsonl`.
///
/// # Panics
/// The directory cannot be written.
#[cfg(unix)]
#[must_use]
pub fn script(dir: &Path, blocks: &[Vec<Value>]) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    for (index, block) in blocks.iter().enumerate() {
        let text: String = block.iter().map(|frame| format!("{frame}\n")).collect();
        std::fs::write(dir.join(format!("{}.jsonl", index + 1)), text).expect("block");
    }
    let path = dir.join("agent.sh");
    let script = format!(
        r#"#!/bin/sh
dir='{dir}'
n=0
printf '%s\n' "$@" > "$dir/arguments"
while IFS= read -r line; do
  printf '%s\n' "$line" >> "$dir/received.jsonl"
  case "$line" in
    *'"method":"session/cancel"'*)
      printf '{{"jsonrpc":"2.0","id":%s,"result":{{"stopReason":"cancelled"}}}}\n' "$n"
      ;;
    *'"method":'*)
      case "$line" in
        *'"id":'*)
          n=$((n+1))
          if [ -f "$dir/$n.jsonl" ]; then cat "$dir/$n.jsonl"; fi
          ;;
      esac
      ;;
  esac
done
"#,
        dir = dir.display()
    );
    std::fs::write(&path, script).expect("script");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("mode");
    path
}

/// The arguments the stand-in in `dir` was started with.
#[must_use]
pub fn arguments(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("arguments"))
        .unwrap_or_default()
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The lines the stand-in in `dir` received, parsed.
#[must_use]
pub fn received(dir: &Path) -> Vec<Value> {
    std::fs::read_to_string(dir.join("received.jsonl"))
        .unwrap_or_default()
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// The recorded Devin 3000.11.3 turn: initialize, a new session in
/// `/workspace`, `bypass` mode, and one prompt that runs `echo` and `ls`.
pub const DEVIN_TURN: &str = include_str!("../fixtures/devin-3000.11.3-turn.jsonl");

/// The recorded OpenCode 1.18.26 turn: initialize, a new session on
/// `google/gemini-3.6-flash`, and one prompt that runs `cat` through the
/// bash tool and answers `done`.
pub const OPENCODE_TURN: &str = include_str!("../fixtures/opencode-1.18.26-turn.jsonl");

/// The same, on a model the provider refused with HTTP 403.
pub const OPENCODE_REFUSED: &str = include_str!("../fixtures/opencode-1.18.26-refused.jsonl");

/// A synthetic Grok Build turn shaped like `grok agent stdio`: initialize,
/// a new session reporting `grok-4.6`, one completed read, and `done`.
/// This is a stand-in for tests, not a captured live session.
pub const GROK_TURN: &str = include_str!("../fixtures/grok-agent-stdio-turn.jsonl");

# Briefing: #10301 Task store v2 still refuses with "another process holds the task store lock"

## The issue

From #10300 (07d8378aae, docs/worktrees/2026-10-03-git-lock-contention.md): task-store lock refusals still happen on store v2 (one file per task) — seen on coderos (`Coder did not start: another process holds the task store lock; retry after it exits` for #10239/#10248 in parallel chat work). Find which lock is still global under v2, make per-task operations not contend (or wait briefly instead of refusing), regression test with parallel starts.

## Change plan

1. Goal: Task store v2 still refuses with "another process holds the task store lock"
2. Change the behavior where it lives, most likely in `crates/coder/src/task.rs`.
3. Add or update a test that pins the new behavior, beside the code's existing tests.
4. Run `check:coder`, then `test:coder`, then `fmt`; stop when they pass.

## Files to change

### `crates/coder/src/task.rs`

Why: has `another process holds the task store lock`

```
--- lines 507-537 of 1845 ---
  507              Self::Conflict => {
  508                  formatter.write_str("the command identity already names different bytes")
  509              }
  510              Self::RevisionMismatch => {
  511                  formatter.write_str("the expected task revision does not match")
  512              }
  513              Self::NotFound => formatter.write_str("the task does not exist"),
  514              Self::InvalidTransition => formatter.write_str("the task cannot make this transition"),
  515              Self::LimitExceeded => formatter.write_str("the task inbox capacity is exhausted"),
  516              Self::UnsafePath => formatter
  517                  .write_str("the task store requires private regular files and a real directory"),
  518              Self::Busy => formatter
  519                  .write_str("another process holds the task store lock; retry after it exits"),
  520              Self::UnsupportedPlatform => {
  521                  formatter.write_str("the task store requires Unix filesystem protections")
  522              }
  523              Self::ReopenRequired => formatter
  524                  .write_str("a write failed; reopen the store before retrying the exact command"),
  525              Self::WorkspaceBusy => {
  526                  formatter.write_str("another Coder task is still running in this project")
  527              }
  528          }
  529      }
  530  }
  531  
  532  impl std::error::Error for Error {}
  533  impl From<std::io::Error> for Error {
  534      fn from(error: std::io::Error) -> Self {
  535          Self::Io(error)
  536      }
  537  }
```

## Similar past changes

### 5a7b318a53 coder: fix two flaky task tests, one a real spare race (#10286)

```diff
diff --git a/crates/coder/src/task.rs b/crates/coder/src/task.rs
index b0424ccb64..d94461c2cd 100644
--- a/crates/coder/src/task.rs
+++ b/crates/coder/src/task.rs
@@ -927,4 +927,12 @@ impl Store {
         let path = self.dir.join(TASK_DIR).join(format!("{id}.lock"));
         let lock = open_lock(&path)?;
+        #[cfg(test)]
+        match lock.try_lock() {
+            Ok(()) => lock.unlock()?,
+            Err(std::fs::TryLockError::WouldBlock) => {
+                TASK_LOCK_WAITS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
+            }
+            Err(std::fs::TryLockError::Error(_)) => {}
+        }
         take_lock(&lock, self.wait)?;
         verify_same_file(&path, &lock)?;
@@ -1059,4 +1067,10 @@ pub fn present(dir: &Path) -> bool {
 }
 
+/// How many times this process found a task's write lock held by another
+/// writer: what writers of different tasks must never do.
+#[cfg(test)]
+pub(crate) static TASK_LOCK_WAITS: std::sync::atomic::AtomicU64 =
+    std::sync::atomic::AtomicU64::new(0);
+
 /// Wait up to `wait` for `lock`'s exclusive OS lock.
 fn take_lock(lock: &File, wait: Duration) -> Result<(), Error> {
```

## Checks (run them with the `run_check` tool)

- `check:coder`: `cargo check -p coder --tests --message-format short` (compile coder and its tests)
- `test:coder`: `cargo test -p coder [FILTER]` (run coder's tests (pass a test-name filter to run fewer))
- `fmt:coder`: `cargo fmt -p coder` (format coder)

## Repo rules

- Product code is Rust. Do not add TypeScript.
- The check for a change is `cargo test -p` for the crates you edited plus `cargo fmt`. Do not run clippy, release gates, or other crates' tests.
- Never put machine talk (internal words like retained, projection, canonical, digest, lane) in text a user sees; say what happened in plain words. Each surface's tests run the `oa-copy` guard over user-visible text.
- When a test fails only because a checked-in generated file is stale, regenerate it.
- No new INVARIANTS rows, design notes, or long docs for a small change.
- Fix stale or false user-facing copy you touch in the same change.
- - `crates/coder` — the agent: `classify` routes each turn through Jev, `generate` answers through an Open Responses door, and the `coder` binary draws the conversation in the terminal or, with `-p`, runs one turn from a script. Both modes run the same turn, `coder::turn::run`; keep it that way. `permit` is the host's answer to whether a turn runs commands at all, built from the route and the operator's setting before anything generates and narrowing from there; a reply becomes an executable plan only under a permit that runs one, so keep execution policy there rather than in what the model is told. `delegate_door` answers a turn through Microcoder's loop in process, on the first connected provider with capacity in the capacity book (the Codex login, then Claude Code's login, then, always last, Vertex through the OpenAgents cloud, which needs no token on the host; read `docs/coder/runtime

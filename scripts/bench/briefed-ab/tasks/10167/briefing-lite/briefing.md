# Briefing: #10167 Disk cleanup runs on triggers the rule doesn't name

## The issue

Found by a Codex review on CoderOS (thread 700865e6, 2026-10-02) of 5ae8d27af8 (#10156).

`crates/background/src/runner.rs:99-120`: the runner evaluates cleanup at host start, when tasks end, and every interval regardless of `rule.triggers`. Removing `HostStart` or `TaskEnded` has no effect; removing `Interval` still schedules checks every 300 s through a fallback; an enabled rule with an empty trigger list can still delete files when disk is low.

Fix: gate each automatic evaluation on the configured trigger; schedule no interval checks when no interval is configured; keep explicit manual runs (`openagents background run`, `/background`) separate. Tests for each trigger removed.

## Change plan

1. Goal: Disk cleanup runs on triggers the rule doesn't name
2. Change the behavior where it lives, most likely in `crates/background/src/runner.rs`, `crates/background/src/run.rs`, `crates/background/src/rule.rs`.
3. Add or update a test that pins the new behavior, beside the code's existing tests.
4. Run `check:background`, then `test:background`, then `fmt`; stop when they pass.

## Files to change

### `crates/background/src/runner.rs`

Why: the issue names `/background`; the issue names `crates/background/src/runner.rs`; has `HostStart`; has `TaskEnded`; has `Interval`

```
--- lines 11-60 of 256 ---
   11  use std::time::Duration;
   12  
   13  use crate::inuse::System;
   14  use crate::paths::{self, Layout};
   15  use crate::plan::{Env, Facts, observe};
   16  use crate::run::{self, Cause, Report};
   17  use crate::store::{self, State};
   18  use crate::volume::Statvfs;
   19  
   20  /// A request to the runner.
   21  enum Request {
   22      Run { rule: String },
   23      TaskEnded,
   24  }
   25  
   26  /// Talks to a running runner.
   27  #[derive(Clone)]
   28  pub struct Handle {
   29      sender: Sender<Request>,
   30  }
   31  
   32  impl Handle {
   33      /// Run `rule` now on the runner's thread. The result goes to the log
   34      /// and the rule's state. (A dry run is computed where it is asked for:
   35      /// it changes nothing, so it needs no runner.)
   36      pub fn run(&self, rule: &str) {
   37          let _ = self.sender.send(Request::Run { rule: rule.into() });
   38      }
   39  
   40      /// A Coder task ended: check now.
   41      pub fn task_ended(&self) {
   42          let _ = self.sender.send(Request::TaskEnded);
   43      }
   44  }
   45  
   46  /// How the runner says what happened: the host prints it to its log.
   47  pub type Say = Box<dyn Fn(&str) + Send>;
   48  
   49  /// Start the runner on its own thread.
   50  #[must_use]
   51  pub fn start(layout: Layout, facts: Option<Arc<dyn Facts>>, say: Say) -> Handle {
   52      let (sender, receiver) = channel();
   53      let _ = std::thread::Builder::new()
   54          .name("background".into())
   55          .spawn(move || Runner::new(layout, facts, say).serve(&receiver));
   56      Handle { sender }
   57  }
   58  
   59  struct Runner {
   60      layout: Layout,
--- lines 88-138 of 256 ---
   88                  Err(RecvTimeoutError::Disconnected) => return,
   89                  Ok(Request::Run { rule }) => {
   90                      // Another process is the runner; a request made here
   91                      // still runs, under the run lock.
   92                      self.manual(&rule);
   93                  }
   94                  _ => {}
   95              }
   96          };
   97          let pid = std::process::id();
   98          State::update(&self.layout, "disk", |state| state.runner = Some(pid));
   99          std::thread::sleep(START_DELAY);
  100          self.check(Cause::HostStart);
  101          let mut next = paths::now() + self.interval();
  102          let mut next_tasks = paths::now() + TASK_POLL.as_secs();
  103          loop {
  104              let wait = next.min(next_tasks).saturating_sub(paths::now()).max(1);
  105              match requests.recv_timeout(Duration::from_secs(wait)) {
  106                  Ok(Request::Run { rule }) => self.manual(&rule),
  107                  Ok(Request::TaskEnded) => self.check(Cause::TaskEnded),
  108                  Err(RecvTimeoutError::Timeout) => {}
  109                  Err(RecvTimeoutError::Disconnected) => return,
  110              }
  111              let now = paths::now();
  112              if now >= next_tasks {
  113                  next_tasks = now + TASK_POLL.as_secs();
  114                  if self.task_ended() {
  115                      self.check(Cause::TaskEnded);
  116                  }
  117              }
  118              if now >= next {
  119                  next = now + self.interval();
  120                  self.check(Cause::Interval);
  121              }
  122          }
  123      }
  124  
  125      fn runner_lock(&self) -> Option<File> {
  126          std::fs::create_dir_all(self.layout.background()).ok()?;
  127          let file = File::options()
  128              .read(true)
  129              .write(true)
  130              .create(true)
  131              .truncate(false)
  132              .open(self.layout.runner_lock())
  133              .ok()?;
  134          file.try_lock().ok().map(|()| file)
  135      }
  136  
  137      fn interval(&self) -> u64 {
  138          store::load(&self.layout, "disk")
--- lines 197-227 of 256 ---
  197              state.last_check = Some(now);
  198              state.next_check = Some(now + rule.interval().unwrap_or(300));
  199              state.free = fullest.map(|volume| volume.space.free);
  200              state.total = fullest.map(|volume| volume.space.total);
  201          });
  202          let spaces: Vec<(u64, u64)> = volumes
  203              .iter()
  204              .map(|volume| (volume.space.free, volume.space.total))
  205              .collect();
  206          if decide(&rule, &spaces, state.last_run, now).is_none() {
  207              return;
  208          }
  209          let cause = if cause == Cause::Interval {
  210              Cause::Threshold
  211          } else {
  212              cause
  213          };
  214          let result = run::run(&env, &rule, cause, false, false);
  215          self.finish(&rule.id, result);
  216      }
  217  
  218      fn finish(&self, id: &str, result: Result<Report, String>) {
  219          let report = match result {
  220              Ok(report) => report,
  221              Err(why) => {
  222                  (self.say)(&format!("background {id}: {why}"));
  223                  return;
  224              }
  225          };
  226          crate::view::remember(&self.layout, id, &report);
  227          let notice = report.notice.clone();
```

### `crates/background/src/run.rs`

Why: the issue names `/background`; has `openagents background run`; has `HostStart`; has `TaskEnded`; has `Interval`; has `/background`

```
--- lines 7-41 of 452 ---
    7  use serde::{Deserialize, Serialize};
    8  
    9  use crate::git::{self, Undo};
   10  use crate::paths::{self, Layout, bytes, show};
   11  use crate::plan::{self, Env, Evidence, Item, Kept, Plan, View};
   12  use crate::rule::{Class, Rule};
   13  use crate::store;
   14  
   15  /// What started a run.
   16  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
   17  #[serde(rename_all = "snake_case")]
   18  pub enum Cause {
   19      Interval,
   20      Threshold,
   21      TaskEnded,
   22      HostStart,
   23      /// Someone asked: `openagents background run`, `/background`, or
   24      /// `background.run`.
   25      Manual,
   26  }
   27  
   28  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
   29  #[serde(rename_all = "snake_case")]
   30  pub enum Outcome {
   31      Deleted,
   32      /// A worktree removed with `git worktree remove`.
   33      Removed,
   34      /// A check failed right before deletion.
   35      Skipped,
   36      Failed,
   37  }
   38  
   39  /// One action of a run.
   40  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   41  pub struct Action {
```

### `crates/background/src/rule.rs`

Why: the issue names `/background`; has `HostStart`; has `TaskEnded`; has `Interval`; has `/background`

```
--- lines 1-20 of 457 ---
    1  //! The rule type: one JSON document per rule at
    2  //! `~/.openagents/background/rules/<id>.json`, versioned and digested.
    3  //!
    4  //! Phase 1 ships one built-in rule, [`disk`]. A rule file that is absent
    5  //! means the built-in defaults; editing it (`openagents background edit
    6  //! disk`) writes the file, which then wins.
    7  
    8  use std::path::{Path, PathBuf};
    9  
   10  use serde::{Deserialize, Serialize};
   11  use sha2::{Digest, Sha256};
   12  
   13  /// The schema every rule document carries.
   14  pub const SCHEMA: &str = "openagents.background.rule.v1";
   15  
   16  /// One gigabyte, as the policy counts it (10^9 bytes).
   17  pub const GB: u64 = 1_000_000_000;
   18  
   19  /// A durable, user-defined rule the host runs.
   20  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
--- lines 41-78 of 457 ---
   41  
   42  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   43  #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
   44  pub enum Origin {
   45      BuiltIn,
   46      File { path: String },
   47  }
   48  
   49  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   50  #[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
   51  pub enum Trigger {
   52      /// A check every `every_secs`.
   53      Interval { every_secs: u64 },
   54      /// Free space falls below the goal's start threshold, checked on the
   55      /// interval.
   56      Threshold,
   57      /// A Coder task ends.
   58      TaskEnded,
   59      /// The host starts.
   60      HostStart,
   61  }
   62  
   63  /// `max(bytes, percent of the volume)`.
   64  #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
   65  #[serde(deny_unknown_fields)]
   66  pub struct Level {
   67      pub bytes: u64,
   68      pub percent: u8,
   69  }
   70  
   71  impl Level {
   72      /// The bytes this level means on a volume of `total` bytes.
   73      #[must_use]
   74      pub fn of(self, total: u64) -> u64 {
   75          let share = u128::from(total) * u128::from(self.percent) / 100;
   76          self.bytes.max(u64::try_from(share).unwrap_or(u64::MAX))
   77      }
   78  }
--- lines 222-255 of 457 ---
  222  /// The built-in disk cleanup rule with the spec's default policy.
  223  #[must_use]
  224  pub fn disk() -> Rule {
  225      Rule {
  226          schema: SCHEMA.into(),
  227          id: "disk".into(),
  228          name: "Disk cleanup".into(),
  229          version: 1,
  230          origin: Origin::BuiltIn,
  231          enabled: true,
  232          paused_until: None,
  233          triggers: vec![
  234              Trigger::Interval { every_secs: 300 },
  235              Trigger::Threshold,
  236              Trigger::TaskEnded,
  237              Trigger::HostStart,
  238          ],
  239          goal: Goal {
  240              start: Level {
  241                  bytes: 30 * GB,
  242                  percent: 5,
  243              },
  244              stop: Level {
  245                  bytes: 60 * GB,
  246                  percent: 15,
  247              },
  248              emergency: Level {
  249                  bytes: 10 * GB,
  250                  percent: 1,
  251              },
  252              max_freed: 100 * GB,
  253          },
  254          actions: vec![
  255              Action::DeleteCaches {
--- lines 269-299 of 457 ---
  269              checkout_days: 7,
  270              keep: 0,
  271              orphan_worktree_days: 7,
  272              gate_idle_hours: 1,
  273              report_only: Vec::new(),
  274          },
  275          safety: Safety {
  276              allow: vec![
  277                  "~/.openagents/targets".into(),
  278                  "~/.openagents/coder-one/target".into(),
  279                  "~/.openagents/worktrees".into(),
  280                  "~/.openagents/gate".into(),
  281                  "~/.openagents/background/trash".into(),
  282                  "~/work".into(),
  283              ],
  284              deny: Vec::new(),
  285              report: vec![
  286                  "~/.openagents/pylon".into(),
  287                  "~/Library/Caches".into(),
  288                  "/private/var/folders".into(),
  289              ],
  290          },
  291          cooldown_secs: 600,
  292      }
  293  }
  294  
  295  /// The built-in rules, by id.
  296  #[must_use]
  297  pub fn built_in(id: &str) -> Option<Rule> {
  298      (id == "disk").then(disk)
  299  }
--- lines 312-328 of 457 ---
  312      }
  313  
  314      /// Whether the rule runs on its triggers at `now`.
  315      #[must_use]
  316      pub fn active(&self, now: u64) -> bool {
  317          self.enabled && self.paused_until.is_none_or(|until| until <= now)
  318      }
  319  
  320      /// The check interval.
  321      #[must_use]
  322      pub fn interval(&self) -> Option<u64> {
  323          self.triggers.iter().find_map(|trigger| match trigger {
  324              Trigger::Interval { every_secs } => Some(*every_secs),
  325              _ => None,
  326          })
  327      }
  328  
```

### `crates/background/Cargo.toml`

Why: the issue names `/background`; has `/background`

```
--- lines 1-19 of 19 ---
    1  [package]
    2  name = "background"
    3  version.workspace = true
    4  edition.workspace = true
    5  rust-version.workspace = true
    6  publish.workspace = true
    7  description = "Background processes: durable rules the host runs without a conversation, starting with the disk cleanup monitor (docs/background)."
    8  
    9  [dependencies]
   10  libc = "0.2"
   11  serde = { version = "1", features = ["derive"] }
   12  serde_json = "1"
   13  sha2 = "0.10"
   14  
   15  [dev-dependencies]
   16  tempfile = "3"
   17  
   18  [lints]
   19  workspace = true
```

### `crates/background/src/lib.rs`

Why: the issue names `/background`; has `/background`

```
--- lines 1-33 of 33 ---
    1  //! Background processes: durable rules the host runs without a
    2  //! conversation (docs/background/2026-10-02-background-processes.md).
    3  //!
    4  //! Phase 1 is the disk cleanup monitor, the built-in rule `disk`: when a
    5  //! watched volume's free space falls below its start level, it deletes
    6  //! disposable build caches and finished worktrees, in class order and least
    7  //! recently used first, until the stop level, and never anything in use,
    8  //! unsaved, denied, or reached through a link. No model is called.
    9  //!
   10  //! The crate has no host dependency: the host gives it the task store
   11  //! ([`Facts`]) and starts [`runner::start`]; `openagents background` calls
   12  //! [`run::run`] directly.
   13  
   14  #![cfg(unix)]
   15  
   16  pub mod git;
   17  pub mod inuse;
   18  pub mod paths;
   19  pub mod plan;
   20  pub mod rule;
   21  pub mod run;
   22  pub mod runner;
   23  pub mod store;
   24  pub mod view;
   25  pub mod volume;
   26  
   27  pub use paths::{Layout, SLOTS};
   28  pub use plan::{Env, Facts, Plan, TaskFact};
   29  pub use rule::{Class, Rule};
   30  pub use run::{Cause, Record, Report};
   31  
   32  #[cfg(test)]
   33  mod tests;
```

### `crates/background/src/git.rs`

Why: the issue names `/background`

```
--- lines 1-60 of 170 ---
    1  //! The Git checks a worktree passes before it is removed, the removal
    2  //! itself, and its undo.
    3  
    4  use std::path::{Path, PathBuf};
    5  use std::process::Command;
    6  
    7  use serde::{Deserialize, Serialize};
    8  
    9  /// What recreates a removed worktree: its repository, branch, and commit.
   10  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
   11  #[serde(deny_unknown_fields)]
   12  pub struct Undo {
   13      pub repo: PathBuf,
   14      pub path: PathBuf,
   15      #[serde(default, skip_serializing_if = "Option::is_none")]
   16      pub branch: Option<String>,
   17      pub commit: String,
   18  }
   19  
   20  fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
   21      let output = Command::new("git")
   22          .arg("-C")
   23          .arg(dir)
   24          .args(args)
   25          .env("GIT_TERMINAL_PROMPT", "0")
   26          .env("GIT_OPTIONAL_LOCKS", "0")
   27          .stdin(std::process::Stdio::null())
   28          .output()
   29          .map_err(|error| format!("git: {error}"))?;
   30      if output.status.success() {
   31          Ok(String::from_utf8_lossy(&output.stdout)
   32              .trim_end()
   33              .to_owned())
   34      } else {
   35          Err(format!(
   36              "git {}: {}",
   37              args.first().copied().unwrap_or_default(),
   38              String::from_utf8_lossy(&output.stderr).trim()
   39          ))
   40      }
   41  }
   42  
   43  /// Whether Git ignores `path` inside the checkout `top`.
   44  #[must_use]
   45  pub fn ignored(top: &Path, path: &Path) -> bool {
   46      let Ok(relative) = path.strip_prefix(top) else {
   47          return false;
   48      };
   49      Command::new("git")
   50          .arg("-C")
   51          .arg(top)
   52          .args(["check-ignore", "-q", "--no-index"])
   53          .arg(relative)
   54          .stdin(std::process::Stdio::null())
   55          .stdout(std::process::Stdio::null())
   56          .stderr(std::process::Stdio::null())
   57          .status()
   58          .is_ok_and(|status| status.success())
   59  }
   60  
```

## Checks (run them with the `run_check` tool)

- `check:background`: `cargo check -p background --tests --message-format short` (compile background and its tests)
- `test:background`: `cargo test -p background [FILTER]` (run background's tests (pass a test-name filter to run fewer))
- `fmt:background`: `cargo fmt -p background` (format background)

## Repo rules

- Product code is Rust. Do not add TypeScript.
- The check for a change is `cargo test -p` for the crates you edited plus `cargo fmt`. Do not run clippy, release gates, or other crates' tests.
- Never put machine talk (internal words like retained, projection, canonical, digest, lane) in text a user sees; say what happened in plain words. Each surface's tests run the `oa-copy` guard over user-visible text.
- When a test fails only because a checked-in generated file is stale, regenerate it.
- No new INVARIANTS rows, design notes, or long docs for a small change.
- Fix stale or false user-facing copy you touch in the same change.

# Briefing: #10074 openagents chat local run refused: "the execution grant has an invalid shape"

## The issue

## What happened (2026-10-01)

An agent ran `openagents chat` (CLI) in a scratch git worktree of this repo against the live chat worker **without** `--no-run`. The router replied `work.dispatch`, the CLI started Coder locally (the #10032 local-run path), and the launch was refused:

> the execution grant has an invalid shape

## Evidence so far

- The message is `Grant::parse` in `crates/coder/src/task/owner.rs` failing serde deserialization. `Grant` and `adapter::Configuration` are `deny_unknown_fields`, and `adapter::Access` is a closed enum.
- #10045 (b4d0acc8ea) added `Access::Toolchains`; every local run (`coder::task::local`) now writes `"access": "toolchains"` into the grant's adapter configuration.
- The local run's engine comes from `coder::task::local::controller()` → `autostart::default_controller()`: `$OPENAGENTS_CODER_CONTROLLER`, else `microcoder` **beside the running exe**, else `~/.openagents/bin/microcoder`.
- On the owner's machine, `~/.openagents/bin/microcoder` is a Sep 28 build (coder beside it is `358975bdbd`), from before #10045, so it does not know `toolchains`.
- The shipping Mac app (`scripts/desktop/package-macos.sh`) puts the CLI at `Contents/Helpers/openagents` but `microcoder` at `Contents/MacOS/microcoder`. So the app's own CLI never finds the microcoder it ships with: it falls back to `~/.openagents/bin/microcoder` (stale or missing).

Other suspects: recent changes #10066 (3e13917001), #10070 (1ebd5e20f7), #10067/68 (ebdec8d004), #10073 (92aef353b7).

## Plan

Reproduce with a temp HOME and fresh `openagents` + `coder` + `microcoder` builds from origin/main, print which binaries each run uses, and decide between version skew and a grant-shape bug. Fix the resolution so a bundled CLI uses the engine that shipped with it, and add a regression test.

## Change plan

1. Goal: openagents chat local run refused: "the execution grant has an invalid shape"
2. Required: The message is `Grant::parse` in `crates/coder/src/task/owner.rs` failing serde deserialization. `Grant` and `adapter::Configuration` are `deny_unknown_fields`, and `adapter::Access` is a closed enum.
3. Required: #10045 (b4d0acc8ea) added `Access::Toolchains`; every local run (`coder::task::local`) now writes `"access": "toolchains"` into the grant's adapter configuration.
4. Required: The local run's engine comes from `coder::task::local::controller()` → `autostart::default_controller()`: `$OPENAGENTS_CODER_CONTROLLER`, else `microcoder` **beside the running exe**, else `~/.openagents/bin/microcoder`.
5. Required: On the owner's machine, `~/.openagents/bin/microcoder` is a Sep 28 build (coder beside it is `358975bdbd`), from before #10045, so it does not know `toolchains`.
6. Required: The shipping Mac app (`scripts/desktop/package-macos.sh`) puts the CLI at `Contents/Helpers/openagents` but `microcoder` at `Contents/MacOS/microcoder`. So the app's own CLI never finds the microcoder it ships with: it f
7. Change the behavior where it lives, most likely in `crates/coder/src/task/owner.rs`, `crates/coder/src/task/local.rs`, `crates/coder/src/task/autostart.rs`.
8. Add or update a test that pins the new behavior, next to the existing ones in `crates/microcoder/src/repository/tests.rs`.
9. Run `check:coder`, then `test:coder`, then `check:microcoder`, then `test:microcoder`, then `fmt`; stop when they pass.

## Files to change

### `crates/coder/src/task/owner.rs`

Why: the issue names `crates/coder/src/task/owner.rs`; has `the execution grant has an invalid shape`; has `Grant::parse`; has `adapter::Configuration`; has `adapter::Access`; has `Access::Toolchains`

```
--- lines 131-170 of 925 ---
  131      pub expected_revision: u64,
  132      #[serde(default)]
  133      pub expected_source_snapshot: Option<String>,
  134      pub program: PathBuf,
  135      pub arguments: Vec<String>,
  136      pub write_workspace: bool,
  137      pub wall_seconds: u64,
  138      pub stream_bytes: usize,
  139      pub memory_bytes: u64,
  140      #[serde(default)]
  141      pub requirements: Option<checks::Requirements>,
  142      #[serde(default)]
  143      pub adapter_configuration: Option<super::adapter::Configuration>,
  144  }
  145  
  146  impl Grant {
  147      pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
  148          let value = parse_strict_bounded(bytes, MAX_COMMAND_BYTES).map_err(|_| {
  149              Error::InvalidCommand("the execution grant must be bounded strict JSON")
  150          })?;
  151          let grant: Self = serde_json::from_value(value)
  152              .map_err(|_| Error::InvalidCommand("the execution grant has an invalid shape"))?;
  153          grant.validate()?;
  154          Ok(grant)
  155      }
  156  
  157      fn validate(&self) -> Result<(), Error> {
  158          if self.schema != GRANT_SCHEMA
  159              || !identifier(&self.task_id, false)
  160              || !hex_digest(&self.intent_digest)
  161              || self
  162                  .expected_source_snapshot
  163                  .as_ref()
  164                  .is_some_and(|digest| !hex_digest(digest))
  165              || !self.program.is_absolute()
  166              || self.arguments.len() > 128
  167              || self.arguments.iter().any(|arg| arg.contains('\0'))
  168              || !(1..=3600).contains(&self.wall_seconds)
  169              || !(1024..=1024 * 1024).contains(&self.stream_bytes)
  170              || !(64 * 1024 * 1024..=8 * 1024 * 1024 * 1024).contains(&self.memory_bytes)
--- lines 319-377 of 925 ---
  319                  || if admission
  320                      .grant
  321                      .adapter_configuration
  322                      .as_ref()
  323                      .is_some_and(|config| config.container.is_some())
  324                  {
  325                      admission.network != "container_network_none"
  326                          || admission.read_scope != "workspace_host_reads_and_pinned_container_image"
  327                  } else if admission
  328                      .grant
  329                      .adapter_configuration
  330                      .as_ref()
  331                      .is_some_and(|config| config.access == super::adapter::Access::Full)
  332                  {
  333                      // The owner's full access: the host user's reads and
  334                      // network, and nothing narrower claimed.
  335                      admission.network != "host_network" || admission.read_scope != "host_user"
  336                  } else if admission
  337                      .grant
  338                      .adapter_configuration
  339                      .as_ref()
  340                      .is_some_and(|config| config.access == super::adapter::Access::Toolchains)
  341                  {
  342                      // This computer's tools: the network, and reads of the
  343                      // workspace, the system, and the derived toolchains.
  344                      admission.network != "host_network"
  345                          || admission.read_scope != "workspace_system_and_toolchains"
  346                  } else {
  347                      !matches!(
  348                          admission.network.as_str(),
  349                          "external_ip_denied_localhost_allowed" | "network_namespace_isolated"
  350                      ) || admission.read_scope != "workspace_and_system"
  351                  }
  352                  || admission.authority != "local_os_user"
  353                  || !admission
  354                      .context
  355                      .valid(task, admission.grant.requirements.as_ref())
  356                  || admission.trace_file != task.trace_file(task.turn())
  357                  || !hex_digest(&admission.source_snapshot)
  358                  || !hex_digest(&admission.program_digest)
  359                  || Grant::parse(admission.grant_request.as_bytes())? != admission.grant
  360                  || digest_bytes(admission.grant_request.as_bytes()) != admission.grant_digest
  361                  || !hex_digest(&admission.grant_digest)
  362              {
  363                  return Err(Error::InvalidTransition);
  364              }
  365              task.run = Some(Run {
  366                  epoch: 1,
  367                  admission: (**admission).clone(),
  368                  effect_id: None,
  369                  result: None,
  370                  process_id: None,
  371                  recovery_reason: None,
  372                  check_report: None,
  373              });
  374              task.status = Status::Running;
  375              task.execution = Execution::Running;
  376          }
  377          Event::EffectIntent { effect_id } => {
--- lines 636-666 of 925 ---
  636      Ok(ended.stdout.text.trim().into())
  637  }
  638  
  639  /// The one effect a turn's run records before dispatch:
  640  /// `<task>:<turn>:command`.
  641  pub(super) fn effect_id_for(task: &Task) -> String {
  642      format!("{}:{}:command", task.task_id, task.turn())
  643  }
  644  
  645  /// Execute a single explicitly granted bounded command. The calling process is
  646  /// the owner, not a client connection. Use the detached CLI to outlive a client.
  647  pub async fn execute(directory: &Path, bytes: &[u8]) -> Result<Task, Error> {
  648      let grant = Grant::parse(bytes)?;
  649      if grant.adapter_configuration.is_some() {
  650          return Err(Error::InvalidCommand(
  651              "use the explicitly configured adapter entry point",
  652          ));
  653      }
  654      let (owner, task) = {
  655          let store = Store::open_for_owner(directory)?;
  656          let owner = Owner::acquire(&store, &grant.task_id)?;
  657          let task = store.show(&grant.task_id)?;
  658          if task.run.is_some() || task.status != Status::Queued {
  659              return Err(Error::InvalidTransition);
  660          }
  661          if grant.intent_digest != task.intent_digest || grant.expected_revision != task.revision {
  662              return Err(Error::RevisionMismatch);
  663          }
  664          (owner, task)
  665      };
  666      let workspace = Path::new(&task.intent.workspace.path).canonicalize()?;
```

### `crates/coder/src/task/local.rs`

Why: has `openagents chat`; has `adapter::Access`; has `Access::Toolchains`; has `autostart::default_controller`; has `$OPENAGENTS_CODER_CONTROLLER`; has `Configuration`

```
--- lines 1-21 of 2293 ---
    1  //! Coder on this computer, for the person sitting at it.
    2  //!
    3  //! When a person asks for coding from a terminal (`openagents chat`), or
    4  //! from an app, on the computer they are using, nothing has to be paired or
    5  //! registered first: the project is the Git checkout they are in, and the
    6  //! providers are the coding agents signed in here. This module starts that
    7  //! run and follows it, through exactly what a host's auto-start uses:
    8  //!
    9  //! - **The same start.** A task is submitted to a task store (by default
   10  //!   `~/.openagents/tasks`, the store a host on this computer serves; see
   11  //!   [`default_store`]) and started with an execution grant through
   12  //!   [`Policy::launch`], so the same engine (`microcoder repository`), the
   13  //!   same provider failover, and the same ATIF recording run it.
   14  //! - **Its own worktree.** Coder never writes in the person's checkout.
   15  //!   Each task gets a detached worktree of the checkout's `HEAD` under
   16  //!   `worktrees/` beside the store; the checkout changes only by Git's
   17  //!   record of the worktree. Uncommitted changes in the checkout are not
   18  //!   carried over.
   19  //! - **Providers detected here.** Codex, then Claude Code, each only when
   20  //!   [`capacity::probe`] finds its login on this computer (no network, no
   21  //!   credential read), skipping one with a refusal that still holds in the
--- lines 38-71 of 2293 ---
   38  //! so no device grant is involved; see the INVARIANTS row on local runs.
   39  //! Nothing here reads or prints a credential.
   40  
   41  use std::collections::BTreeMap;
   42  use std::path::{Path, PathBuf};
   43  
   44  use openagents_chat::coder_events::{
   45      self, CoderEvent, FileChange, Line, Mapper, Passed, PassedOver, Runner, Started,
   46  };
   47  use serde::{Deserialize, Serialize};
   48  use serde_json::{Value, json};
   49  
   50  use super::autostart::{self, Choice, Engine, Launch, Policy, Route, UsageProbe};
   51  use super::capacity::{self, Connection, Provider};
   52  use super::{
   53      Action, COMMAND_SCHEMA, Command, RequestedConfiguration, Status, Store, TaskIntent, Workspace,
   54      adapter, owner, settings, usage,
   55  };
   56  
   57  /// The routes a local run admits by default, in preference order: Codex,
   58  /// then Claude Code, with the models the desktop's auto-start switch
   59  /// admits. The settings' `coder.providers` replaces them.
   60  pub const ROUTES: [(Provider, &str); 2] = [
   61      (Provider::Codex, "gpt-6-luna"),
   62      (Provider::Claude, "claude-opus-5-5"),
   63  ];
   64  /// Names another task store than [`default_store`].
   65  pub const STORE_VAR: &str = "OPENAGENTS_TASKS";
   66  /// Names the engine (`microcoder`) instead of the one beside the running
   67  /// program or in `~/.openagents/bin`.
   68  pub const CONTROLLER_VAR: &str = "OPENAGENTS_CODER_CONTROLLER";
   69  /// The record a local run keeps beside its task.
   70  pub const RECORD_SCHEMA: &str = "openagents.coder.local-run.v1";
   71  /// The host name a thread's binding gives a local run.
--- lines 82-123 of 2293 ---
   82  /// `~/.openagents/tasks`, the store `coder task` and a host on this
   83  /// computer use by default.
   84  #[must_use]
   85  pub fn default_store() -> PathBuf {
   86      if let Some(dir) = std::env::var_os(STORE_VAR).filter(|v| !v.is_empty()) {
   87          return PathBuf::from(dir);
   88      }
   89      std::env::var_os("HOME")
   90          .map_or_else(|| PathBuf::from("."), PathBuf::from)
   91          .join(".openagents/tasks")
   92  }
   93  
   94  /// The engine that runs a turn: `$OPENAGENTS_CODER_CONTROLLER`, else the
   95  /// `microcoder` beside the running program or in `~/.openagents/bin`.
   96  ///
   97  /// # Errors
   98  /// Names where it looked.
   99  pub fn controller() -> Result<PathBuf, String> {
  100      if let Some(path) = std::env::var_os(CONTROLLER_VAR).filter(|v| !v.is_empty()) {
  101          return PathBuf::from(path)
  102              .canonicalize()
  103              .map_err(|_| format!("{CONTROLLER_VAR} names no file"));
  104      }
  105      autostart::default_controller().and_then(|path| {
  106          path.canonicalize()
  107              .map_err(|_| "the microcoder engine is missing".into())
  108      })
  109  }
  110  
  111  /// A person's Git checkout.
  112  #[derive(Clone, Debug, PartialEq, Eq)]
  113  pub struct Checkout {
  114      /// Its top level.
  115      pub top: PathBuf,
  116      /// Its folder's name, which names the project.
  117      pub name: String,
  118      /// The commit `HEAD` names.
  119      pub head: String,
  120  }
  121  
  122  pub(crate) fn git() -> std::process::Command {
  123      let program = owner::GIT_PATHS
--- lines 288-318 of 2293 ---
  288  
  289  /// The record of `task` in `store`, if a local run started it.
  290  #[must_use]
  291  pub fn record(store: &Path, task: &str) -> Option<Record> {
  292      let bytes = std::fs::read(record_path(store, task)).ok()?;
  293      serde_json::from_slice::<Record>(&bytes)
  294          .ok()
  295          .filter(|record| record.schema == RECORD_SCHEMA && record.task == task)
  296  }
  297  
  298  fn save(store: &Path, record: &Record) -> Result<(), String> {
  299      let bytes = serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?;
  300      autostart::write_private(&record_path(store, &record.task), &bytes)
  301  }
  302  
  303  /// Starts, answers, and stops Coder runs on this computer.
  304  pub struct Local {
  305      store: PathBuf,
  306      worktrees: PathBuf,
  307      launcher: Box<dyn Launch>,
  308      probe: fn(Provider) -> Connection,
  309      now: fn() -> u64,
  310      controller: Option<PathBuf>,
  311      /// The person's settings, or why they could not be read: a run then
  312      /// refuses rather than falling back to the defaults.
  313      settings: Result<settings::Coder, String>,
  314      max_steps: std::sync::atomic::AtomicUsize,
  315  }
  316  
  317  impl std::fmt::Debug for Local {
  318      fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
--- lines 325-336 of 2293 ---
  325  impl Local {
  326      /// Runs over the task store `store`, with worktrees beside it and the
  327      /// engine started as a detached process.
  328      #[must_use]
  329      pub fn new(store: PathBuf) -> Self {
  330          let worktrees = store.parent().map_or_else(
  331              || store.join("worktrees"),
  332              |parent| parent.join("worktrees"),
  333          );
  334          Local {
  335              store,
  336              worktrees,
```

### `crates/coder/src/task/autostart.rs`

Why: has `Grant::parse`; has `adapter::Configuration`; has `adapter::Access`; has `Access::Toolchains`; has `toolchains`; has `Configuration`

```
--- lines 1-77 of 3524 ---
    1  //! The owner's auto-start policy: start tasks that enrolled devices create.
    2  //!
    3  //! `task.create` from an enrolled device is an inert submission. This policy
    4  //! is the one exception, and only the host's owner can turn it on, locally,
    5  //! with `coder host autostart on`. It is off by default and off whenever its
    6  //! file is missing or unreadable. While it is on, a task a device creates in
    7  //! an allowlisted workspace is recorded as *eligible*, and the host starts it
    8  //! through the configured engine with an execution grant the policy bounds:
    9  //!
   10  //! - **Workspaces**: only the labels the policy lists, each also admitted by
   11  //!   the host's own settings. A device still never sends a path.
   12  //! - **Concurrency**: at most `max_running` auto-started tasks run at once;
   13  //!   the rest wait queued and start as earlier ones finish.
   14  //! - **Engine**: one adapter (`microcoder-repository`), one controller
   15  //!   executable, the admitted models in the owner's preference order, step
   16  //!   and wall-clock limits, and the same filesystem boundary and supervisor
   17  //!   as a hand-written grant.
   18  //! - **Routes**: each start names the first admitted route whose provider
   19  //!   has a local login and no recorded usage-limit refusal (see
   20  //!   [`super::capacity`]); the other connected routes follow as the grant's
   21  //!   fallbacks. When none has capacity, the task ends as `no_capacity` with
   22  //!   the earliest reset instead of starting a run that cannot succeed.
   23  //! - **Usage probes** (off unless the owner passes `--probe-usage`): the
   24  //!   host also reads each admitted provider's usage windows (see
   25  //!   [`super::usage`]) and prefers a route whose provider is below the
   26  //!   policy's threshold. A probe is advisory: it never adds a route and
   27  //!   never overrides a recorded refusal, and any probe failure leaves the
   28  //!   refusal-only choice above.
   29  //!
   30  //! Every decision is appended to `autostart.jsonl` beside the policy:
   31  //! eligible, started (with the grant digest and owner process), skipped, and
   32  //! refused, plus each policy change. Turning the policy off stops new starts
   33  //! at once; tasks already started keep running under their grants and can
   34  //! be cancelled as usual. Read `docs/coder/runtime/host-autostart.md`.
   35  
   36  use std::collections::{BTreeMap, BTreeSet};
   37  use std::io::Write;
   38  use std::path::{Path, PathBuf};
   39  use std::sync::{Arc, Mutex};
   40  use std::time::Duration;
   41  
   42  use serde::{Deserialize, Serialize};
   43  
   44  use openagents_connect::control::{
   45      EngineAccount, EngineReport, EngineRoute, RouteUsage, UsageWindow,
   46  };
   47  
   48  use super::capacity::{self, Connection, Provider};
   49  use super::usage;
   50  use super::{Action, COMMAND_SCHEMA, Command, Status, Store, adapter, owner};
   51  
   52  pub use coder_host::StartCause;
   53  
   54  /// The policy file in the host root.
   55  pub const POLICY_FILE: &str = "autostart.json";
   56  /// The append-only record in the host root.
   57  pub const JOURNAL_FILE: &str = "autostart.jsonl";
   58  pub const POLICY_SCHEMA: &str = "openagents.coder.host-autostart.v1";
   59  pub const ENTRY_SCHEMA: &str = "openagents.coder.host-autostart-entry.v1";
   60  /// The most tasks a policy may run at once.
   61  pub const MAX_RUNNING: u32 = 8;
   62  /// The decision model a new policy names. The engine refuses a Jev reply
   63  /// whose model differs from the admitted one, so this is an exact version,
   64  /// never an alias such as `jev-latest`.
   65  pub const DEFAULT_DECISION_MODEL: &str = "jev-1.13.0";
   66  /// How long a started task may stay queued, waiting for its owner process
   67  /// to admit it, before it stops counting against the concurrency bound.
   68  const PENDING_GRACE: u64 = 120;
   69  /// How long a started task may stay queued while its owner process still
   70  /// runs before the host ends it as never started. An owner that has exited
   71  /// without admitting the task ends it at the next sweep instead. It matches
   72  /// [`StartCause::Timeout`]'s sentence.
   73  const ADMISSION_DEADLINE: u64 = 600;
   74  /// How many times the host launches an owner for one turn when the owner
   75  /// stops without admitting it for a cause that may be transient.
   76  const MAX_ATTEMPTS: usize = 2;
   77  /// The most of a launch diagnostic the host reads, from its end.
--- lines 121-153 of 3524 ---
  121      /// every policy had before routes existed: the Codex login with `model`
  122      /// and `effort`. When set, the first route's model is `model`.
  123      #[serde(default, skip_serializing_if = "Vec::is_empty")]
  124      pub routes: Vec<Route>,
  125      /// Probe each admitted provider's usage windows before routing. Absent
  126      /// means off: no credential is read for a probe and routing uses
  127      /// recorded refusals only.
  128      #[serde(default, skip_serializing_if = "Option::is_none")]
  129      pub usage_probe: Option<UsageProbe>,
  130      /// What each started task's commands may reach. Absent means the
  131      /// filesystem boundary, so a policy written before this field keeps
  132      /// its meaning; `full` is the owner's full access
  133      /// (`coder host autostart on --full-access`).
  134      #[serde(default, skip_serializing_if = "adapter::Access::is_boundary")]
  135      pub access: adapter::Access,
  136  }
  137  
  138  /// The owner's usage-probe setting.
  139  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
  140  #[serde(deny_unknown_fields)]
  141  pub struct UsageProbe {
  142      /// Utilization, in percent (1 to 100), at or above which routing
  143      /// prefers another admitted route with capacity.
  144      pub threshold_percent: u8,
  145  }
  146  
  147  /// One admitted provider and model, in a policy's preference order.
  148  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
  149  #[serde(deny_unknown_fields)]
  150  pub struct Route {
  151      pub provider: Provider,
  152      /// The exact model identity the provider reports.
  153      pub model: String,
--- lines 358-387 of 3524 ---
  358              expected_revision: revision,
  359              expected_source_snapshot: None,
  360              program,
  361              arguments: Vec::new(),
  362              write_workspace: self.engine.write_workspace,
  363              wall_seconds: self.engine.wall_seconds,
  364              stream_bytes: 64 * 1024,
  365              memory_bytes: self.engine.memory_bytes,
  366              requirements: None,
  367              adapter_configuration: Some(self.configuration(order)),
  368          };
  369          let bytes = serde_json::to_vec_pretty(&grant).map_err(|e| e.to_string())?;
  370          owner::Grant::parse(&bytes).map_err(|e| format!("the grant is invalid: {e}"))?;
  371          let path = grants.join(format!("{task}-{revision}.grant.json"));
  372          write_private(&path, &bytes)?;
  373          launcher.launch(&self.engine, &path, store)
  374      }
  375  
  376      /// The grant configuration that starts on `order[0]` and falls back to
  377      /// the rest. `order` must not be empty.
  378      fn configuration(&self, order: &[Route]) -> adapter::Configuration {
  379          let engine = &self.engine;
  380          let route = |route: &Route| adapter::Route {
  381              provider: route.provider.as_str().into(),
  382              model: route.model.clone(),
  383              // Devin, OpenCode, and Grok Build take no effort: their model
  384              // names carry their own.
  385              effort: (!matches!(
  386                  route.provider,
  387                  Provider::Devin | Provider::OpenCode | Provider::Grok
```

### `scripts/desktop/package-macos.sh`

Why: the issue names `scripts/desktop/package-macos.sh`; has `scripts/desktop/package-macos.sh`; has `Contents/Helpers/openagents`; has `Contents/MacOS/microcoder`

```
--- lines 1-29 of 419 ---
    1  #!/usr/bin/env bash
    2  # Packages OpenAgents for Mac as a signed, notarized, stapled .dmg.
    3  #
    4  #   scripts/desktop/package-macos.sh [options]
    5  #
    6  # Steps (docs/desktop/release.md has the full runbook):
    7  #   1. Build universal (arm64 + x86_64) release binaries of the app
    8  #      (`openagents-desktop`), `coder`, `microcoder`, and `openagents`, and glue each pair
    9  #      with `lipo`.
   10  #   2. Assemble OpenAgents.app: Contents/MacOS/{OpenAgents,coder,microcoder},
   11  #      Contents/Helpers/openagents,
   12  #      Info.plist, icon, and the host's launchd plist in
   13  #      Contents/Library/LaunchAgents/.
   14  #   3. Sign every executable, inner ones first, with the Developer ID
   15  #      Application identity, the hardened runtime, a secure timestamp, and
   16  #      entitlements (the app's and the embedded host's).
   17  #   4. Notarize the app with `xcrun notarytool submit --wait` and staple it.
   18  #   5. Build the .dmg (the app plus an Applications symlink to drag it onto),
   19  #      sign it, notarize it, staple it.
   20  #   6. Check: `codesign --verify --strict`, `spctl --assess` on the app and
   21  #      the .dmg, and `stapler validate` on both.
   22  #
   23  # Options:
   24  #   --app PATH          Package an already assembled .app instead of building
   25  #                       one (skips steps 1-2). Any macOS .app works, e.g. the
   26  #                       deck from scripts/bundle-openagents-deck.sh.
   27  #   --out DIR           Where the .app and .dmg go
   28  #                       (default: $CARGO_TARGET_DIR/desktop-release, or
   29  #                       target/desktop-release).
--- lines 259-291 of 419 ---
  259    <key>LSMinimumSystemVersion</key><string>${MACOSX_DEPLOYMENT_TARGET:-13.0}</string>
  260    <key>LSApplicationCategoryType</key><string>public.app-category.developer-tools</string>
  261    <key>NSHighResolutionCapable</key><true/>
  262    <key>NSLocalNetworkUsageDescription</key><string>OpenAgents lets your phone reach this Mac.</string>
  263  </dict>
  264  </plist>
  265  PLIST
  266    fi
  267    exe="$(plist_get "$app/Contents/Info.plist" CFBundleExecutable)"
  268    exe="${exe:-OpenAgents}"
  269    cp "$app_bin" "$app/Contents/MacOS/$exe"
  270    cp "$coder_bin" "$app/Contents/MacOS/coder"
  271    cp "$micro_bin" "$app/Contents/MacOS/microcoder"
  272    mkdir -p "$app/Contents/Helpers"
  273    cp "$cli_bin" "$app/Contents/Helpers/openagents"
  274  
  275    if [[ -f "$macos_dir/com.openagents.desktop.host.plist" ]]; then
  276      cp "$macos_dir/com.openagents.desktop.host.plist" "$app/Contents/Library/LaunchAgents/"
  277    else
  278      cat >"$app/Contents/Library/LaunchAgents/com.openagents.desktop.host.plist" <<'PLIST'
  279  <?xml version="1.0" encoding="UTF-8"?>
  280  <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
  281  <plist version="1.0">
  282  <dict>
  283    <key>Label</key><string>com.openagents.desktop.host</string>
  284    <key>BundleProgram</key><string>Contents/MacOS/coder</string>
  285    <key>ProgramArguments</key>
  286    <array><string>coder</string><string>host</string><string>serve</string><string>--keychain</string><string>--iroh</string><string>--control</string></array>
  287    <key>RunAtLoad</key><true/>
  288    <key>KeepAlive</key><true/>
  289    <key>ProcessType</key><string>Interactive</string>
  290  </dict>
  291  </plist>
```

### `crates/coder/src/task/settings.rs`

Why: has `openagents chat`; has `"access": "toolchains"`; has `adapter::Access`; has `Access::Toolchains`; has `coder::task::local`; has `toolchains`

```
--- lines 1-60 of 781 ---
    1  //! Local capability settings: what Coder may use on this computer when the
    2  //! person at it asks for coding (`coder::task::local`).
    3  //!
    4  //! One file, read by `openagents chat`, the desktop, and a host on this
    5  //! computer: `$OPENAGENTS_SETTINGS`, else `~/.openagents/settings.json`
    6  //! ([`path`]). A missing file, or a missing field, means the default, and
    7  //! the defaults are exactly what a computer does with no file at all
    8  //! (#10032, #10045): Codex then Claude Code, a coding request runs at once,
    9  //! a fresh usage reading at or above 90% passes a provider over, any Git
   10  //! checkout is a project, and commands run in the filesystem boundary with
   11  //! this computer's toolchains.
   12  //!
   13  //! ```json
   14  //! {
   15  //!   "schema": "openagents.settings.v1",
   16  //!   "coder": {
   17  //!     "providers": ["codex", "claude"],
   18  //!     "start": "at_once",
   19  //!     "usage_threshold_percent": 90,
   20  //!     "projects": [],
   21  //!     "access": "toolchains"
   22  //!   }
   23  //! }
   24  //! ```
   25  //!
   26  //! A file that does not parse, or names a value outside its closed set, is
   27  //! never read as the defaults: a local run then refuses and names the file,
   28  //! so a broken opt-out never quietly becomes an opt-in. Other top-level
   29  //! sections are kept as they are when the file is saved, so other settings
   30  //! (the desktop's, #10021) can live beside these.
   31  //!
   32  //! [`Settings`] is the typed API; [`keys`], [`Settings::get`],
   33  //! [`Settings::set`], and [`Settings::unset`] are the flat `coder.*` keys
   34  //! `openagents settings` and a settings screen edit.
   35  
   36  use std::path::{Path, PathBuf};
   37  
   38  use serde::{Deserialize, Serialize};
   39  use serde_json::{Map, Value, json};
   40  
   41  use super::adapter::Access;
   42  use super::autostart::Route;
   43  use super::capacity::Provider;
   44  use super::usage;
   45  
   46  /// The file's schema.
   47  pub const SCHEMA: &str = "openagents.settings.v1";
   48  /// Names another settings file than `~/.openagents/settings.json`.
   49  pub const PATH_VAR: &str = "OPENAGENTS_SETTINGS";
   50  /// The most routes a local run admits: a first route and its fallbacks.
   51  pub const MAX_PROVIDERS: usize = 1 + super::adapter::MAX_FALLBACKS;
   52  /// The providers a local run can use, in their default order.
   53  pub const PROVIDERS: [Provider; 5] = [
   54      Provider::Codex,
   55      Provider::Claude,
   56      Provider::Grok,
   57      Provider::OpenCode,
   58      Provider::Devin,
   59  ];
   60  
--- lines 213-290 of 781 ---
  213  }
  214  
  215  /// Whether a coding request from a chat starts Coder at once or waits for
  216  /// the person to accept the offer.
  217  #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
  218  #[serde(rename_all = "snake_case")]
  219  pub enum Start {
  220      /// Coder starts as soon as the router judges the message is coding
  221      /// work (#10032).
  222      #[default]
  223      AtOnce,
  224      /// The reply offers Coder; it starts only when the person accepts
  225      /// (`openagents chat run-coder`, or **Run Coder** in the app).
  226      AskFirst,
  227  }
  228  
  229  impl Start {
  230      const fn as_str(self) -> &'static str {
  231          match self {
  232              Start::AtOnce => "at_once",
  233              Start::AskFirst => "ask_first",
  234          }
  235      }
  236  }
  237  
  238  fn default_providers() -> Vec<Choice> {
  239      vec![Choice::new(Provider::Codex), Choice::new(Provider::Claude)]
  240  }
  241  
  242  #[allow(clippy::unnecessary_wraps)]
  243  fn default_threshold() -> Option<u8> {
  244      Some(usage::DEFAULT_THRESHOLD_PERCENT)
  245  }
  246  
  247  fn default_access() -> Access {
  248      Access::Toolchains
  249  }
  250  
  251  /// What Coder may use on this computer for a person's own runs.
  252  #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
  253  #[serde(deny_unknown_fields)]
  254  pub struct Coder {
  255      /// The providers a run may use, first preferred; each only when it is
  256      /// signed in here and has capacity. At least one, at most
  257      /// [`MAX_PROVIDERS`].
  258      #[serde(default = "default_providers")]
  259      pub providers: Vec<Choice>,
  260      /// Whether a coding request runs at once or asks first.
  261      #[serde(default)]
  262      pub start: Start,
  263      /// Utilization (1 to 100 percent) at or above which a fresh usage
  264      /// reading passes a provider over for a later one below it; `null`
  265      /// ignores readings, so only a recorded refusal passes one over.
  266      #[serde(default = "default_threshold")]
  267      pub usage_threshold_percent: Option<u8>,
  268      /// The folders whose Git checkouts count as projects, as absolute
  269      /// paths. Empty: any Git checkout.
  270      #[serde(default)]
  271      pub projects: Vec<PathBuf>,
  272      /// What a run's commands may reach: `toolchains` (the filesystem
  273      /// boundary with this computer's developer tools), `full` (no sandbox,
  274      /// as the person's own user), or `boundary` (the plain boundary).
  275      #[serde(default = "default_access")]
  276      pub access: Access,
  277  }
  278  
  279  impl Default for Coder {
  280      fn default() -> Self {
  281          Coder {
  282              providers: default_providers(),
  283              start: Start::default(),
  284              usage_threshold_percent: default_threshold(),
  285              projects: Vec::new(),
  286              access: default_access(),
  287          }
  288      }
  289  }
  290  
--- lines 428-429 of 781 ---
  428              .map_err(|why| format!("the settings in {} are not valid: {why}", file.display()))?;
  429          Ok(settings)
```

### `crates/microcoder/src/repository/tests.rs`

Why: has `openagents chat`; has `Grant::parse`; has `adapter::Access`; has `Access::Toolchains`; has `coder::task::local`; has `toolchains`

```
--- lines 1-54 of 2955 ---
    1  use super::*;
    2  use crate::models::{Basis, NextAction};
    3  use coder::task::adapter::Route as GrantRoute;
    4  use coder::task::adapter::{CONFIG_SCHEMA, Configuration, NAME};
    5  use coder::task::capacity::{self, Provider, Refusal};
    6  use coder::task::{Action, Command, RequestedConfiguration, Store, TaskIntent, Workspace};
    7  use std::cell::{Cell, RefCell};
    8  use std::collections::VecDeque;
    9  
   10  /// The canonical system shell the owner admits: `/bin/bash`, or `/bin/sh`
   11  /// where there is no `/bin/bash`, as on NixOS.
   12  fn system_shell() -> std::path::PathBuf {
   13      ["/bin/bash", "/bin/sh"]
   14          .iter()
   15          .find_map(|path| Path::new(path).canonicalize().ok())
   16          .expect("a system shell")
   17  }
   18  
   19  pub(super) fn fixture() -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>) {
   20      fixture_with("fixture-model", |_| {})
   21  }
   22  
   23  /// A fixture task that requests `model`, with its grant's configuration
   24  /// changed by `change`.
   25  pub(super) fn fixture_with(
   26      model: &str,
   27      change: impl FnOnce(&mut Configuration),
   28  ) -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>) {
   29      fixture_images(model, change, &[], "Write result.txt containing output.")
   30  }
   31  
   32  /// [`fixture_with`], with `images` attached to the task as a device's
   33  /// upload binds them: kept in the store's task media, named by the intent.
   34  pub(super) fn fixture_images(
   35      model: &str,
   36      change: impl FnOnce(&mut Configuration),
   37      images: &[coder::task::media::wire::Upload],
   38      prompt: &str,
   39  ) -> (tempfile::TempDir, std::path::PathBuf, Vec<u8>) {
   40      let root = tempfile::tempdir().unwrap();
   41      let repo = root.path().join("repo");
   42      let checkout = root.path().join("checkout");
   43      std::fs::create_dir(&repo).unwrap();
   44      for args in [
   45          vec!["init", "-q"],
   46          vec![
   47              "-c",
   48              "user.name=Fixture",
   49              "-c",
   50              "user.email=fixture@example.invalid",
   51              "commit",
   52              "--allow-empty",
   53              "-qm",
   54              "Fixture",
--- lines 77-156 of 2955 ---
   77          schema: task::COMMAND_SCHEMA.into(),
   78          command_id: "submit-fixture".into(),
   79          task_id: "fixture".into(),
   80          expected_revision: None,
   81          action: Action::Submit {
   82              intent: TaskIntent {
   83                  title: "Repository fixture".into(),
   84                  prompt: prompt.into(),
   85                  workspace: Workspace {
   86                      path: checkout.canonicalize().unwrap().display().to_string(),
   87                      source_revision: None,
   88                  },
   89                  configuration: RequestedConfiguration {
   90                      adapter: NAME.into(),
   91                      model: Some(model.into()),
   92                  },
   93                  images: images.iter().map(|image| image.reference.clone()).collect(),
   94              },
   95          },
   96      };
   97      let mut inbox = Store::open(&store).unwrap();
   98      // As a device sends them: chunk by chunk to the host's uploads, then
   99      // bound to the task its `task.create` names.
  100      let device = "d".repeat(64);
  101      for image in images {
  102          for chunk in image.chunks(0) {
  103              coder::task::media::put(&store, &device, &chunk).unwrap();
  104          }
  105          coder::task::media::adopt(&store, &device, "fixture", &image.reference).unwrap();
  106      }
  107      inbox.apply(&serde_json::to_vec(&command).unwrap()).unwrap();
  108      let task = inbox.show("fixture").unwrap();
  109      let grant = task::owner::Grant {
  110          schema: task::owner::GRANT_SCHEMA.into(),
  111          task_id: task.task_id,
  112          intent_digest: task.intent_digest,
  113          expected_revision: 1,
  114          expected_source_snapshot: None,
  115          program: system_shell(),
  116          arguments: Vec::new(),
  117          write_workspace: true,
  118          wall_seconds: 8,
  119          stream_bytes: 4096,
  120          memory_bytes: 256 * 1024 * 1024,
  121          requirements: None,
  122          adapter_configuration: Some(Configuration {
  123              schema: CONFIG_SCHEMA.into(),
  124              provider: "synthetic".into(),
  125              model: "fixture-model".into(),
  126              effort: Some("medium".into()),
  127              generation_endpoint: "in-process".into(),
  128              decision_endpoint: "in-process".into(),
  129              decision_model: "fixture-judge".into(),
  130              max_steps: 4,
  131              acceptance: false,
  132              route: "never".into(),
  133              knowledge: "off".into(),
  134              dollar_limit_micros: None,
  135              expected_controller_digest: None,
  136              container: None,
  137              fallbacks: Vec::new(),
  138              access: coder::task::adapter::Access::Boundary,
  139          }),
  140      };
  141      let mut grant = grant;
  142      if let Some(configuration) = grant.adapter_configuration.as_mut() {
  143          change(configuration);
  144      }
  145      (root, store, serde_json::to_vec(&grant).unwrap())
  146  }
  147  
  148  struct Generator {
  149      actions: RefCell<VecDeque<NextAction>>,
  150      model: &'static str,
  151      calls: Cell<usize>,
  152  }
  153  fn generator(command: &str) -> Generator {
  154      Generator {
  155          actions: RefCell::new(VecDeque::from([
  156              NextAction {
--- lines 432-437 of 2955 ---
  432          &JudgeFixture,
  433      )
  434      .await
  435      .unwrap();
  436      assert_ne!(result.execution, task::Execution::Finished);
  437      assert!(!root.path().join("checkout/result.txt").exists());
```

## Similar past changes

### d70415ef8f Let a task's own process wait out a busy task store

```diff
diff --git a/crates/coder/src/task/owner.rs b/crates/coder/src/task/owner.rs
index 8b60bdbca9..0fb7ecb3c2 100644
--- a/crates/coder/src/task/owner.rs
+++ b/crates/coder/src/task/owner.rs
@@ -471,5 +471,5 @@ impl Owner {
 
     pub(super) fn record(&self, event: Event) -> Result<Task, Error> {
-        Store::open(&self.dir)?.record(self, event, 1)
+        Store::open_for_owner(&self.dir)?.record(self, event, 1)
     }
 }
@@ -499,5 +499,5 @@ pub async fn check(
 ) -> Result<Task, Error> {
     let (owner, task) = {
-        let mut store = Store::open(directory)?;
+        let mut store = Store::open_for_owner(directory)?;
         let owner = Owner::acquire(&store, id)?;
         let task = store.record(&owner, Event::CheckIntent, 1)?;
@@ -551,5 +551,5 @@ pub async fn execute(directory: &Path, bytes: &[u8]) -> Result<Task, Error> {
     }
     let (owner, task) = {
-        let store = Store::open(directory)?;
+        let store = Store::open_for_owner(directory)?;
         let owner = Owner::acquire(&store, &grant.task_id)?;
         let task = store.show(&grant.task_id)?;
@@ -650,5 +650,5 @@ pub async fn execute(directory: &Path, bytes: &[u8]) -> Result<Task, Error> {
     )?;
     let effect_id = effect_id_for(&task);
-    let state = Store::open(&owner.dir)?.show(&task.task_id)?;
+    let state = Store::open_for_owner(&owner.dir)?.show(&task.task_id)?;
     // A cancellation accepted before dispatch permits no effect.
     let mut output_incomplete = false;
@@ -680,5 +680,5 @@ pub async fn execute(directory: &Path, bytes: &[u8]) -> Result<Task, Error> {
             .env("PATH", SYSTEM_PATH);
         let live = {
-            let mut dispatch = Store::open(&owner.dir)?;
+            let mut dispatch = Store::open_for_owner(&owner.dir)?;
             if dispatch.show(&task.task_id)?.status == Status::CancelRequested {
                 None
@@ -714,5 +714,5 @@ pub async fn execute(directory: &Path, bytes: &[u8]) -> Result<Task, Error> {
                     break Some(live.wait().await);
                 }
-                let state = Store::open(&owner.dir)?.show(&task.task_id)?;
+                let state = Store::open_for_owner(&owner.dir)?.show(&task.task_id)?;
                 if state.status == Status::CancelRequested {
                     break Some(live.stop().await);
```

## Checks (run them with the `run_check` tool)

- `check:coder`: `cargo check -p coder --tests --message-format short` (compile coder and its tests)
- `test:coder`: `cargo test -p coder [FILTER]` (run coder's tests (pass a test-name filter to run fewer))
- `fmt:coder`: `cargo fmt -p coder` (format coder)
- `check:microcoder`: `cargo check -p microcoder --tests --message-format short` (compile microcoder and its tests)
- `test:microcoder`: `cargo test -p microcoder [FILTER]` (run microcoder's tests (pass a test-name filter to run fewer))
- `fmt:microcoder`: `cargo fmt -p microcoder` (format microcoder)

## Repo rules

- Product code is Rust. Do not add TypeScript.
- The check for a change is `cargo test -p` for the crates you edited plus `cargo fmt`. Do not run clippy, release gates, or other crates' tests.
- Never put machine talk (internal words like retained, projection, canonical, digest, lane) in text a user sees; say what happened in plain words. Each surface's tests run the `oa-copy` guard over user-visible text.
- When a test fails only because a checked-in generated file is stale, regenerate it.
- No new INVARIANTS rows, design notes, or long docs for a small change.
- Fix stale or false user-facing copy you touch in the same change.
- - `crates/coder` — the agent: `classify` routes each turn through Jev, `generate` answers through an Open Responses door, and the `coder` binary draws the conversation in the terminal or, with `-p`, runs one turn from a script. Both modes run the same turn, `coder::turn::run`; keep it that way. `permit` is the host's answer to whether a turn runs commands at all, built from the route and the operator's setting before anything generates and narrowing from there; a reply becomes an executable plan only under a permit that runs one, so keep execution policy there rather than in what the model is told. `delegate_door` answers a turn through Microcoder's loop in process, on the first connected provider with capacity in the capacity book (the Codex login, then Claude Code's login, then, always last, Vertex through the OpenAgents cloud, which needs no token on the host; read `docs/coder/runtime

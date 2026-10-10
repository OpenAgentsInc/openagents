# Briefing: #10228 Engine lookup falls back to an old ~/.openagents/bin/microcoder when the running binary was rebuilt (Linux "(deleted)" path)

## The issue

On CoderOS a `chat work` batch's fix turns failed: "the Coder engine at ~/.openagents/bin/microcoder is older than this program". `default_controller` (crates/coder/src/task/autostart.rs ~2352) canonicalizes `current_exe()`; when the running binary was replaced by a rebuild (agents ran `oa-terminal-dev --build-only` during the batch), Linux reports `/path/microcoder (deleted)`, so `dir.join(MICROCODER)` is still right but canonicalize/the exe check drops it and the lookup falls back to the old installed copy.

Fix: strip a trailing " (deleted)" from `current_exe()` (and/or use the uncanonicalized path's parent) so the engine beside the program — the freshly rebuilt one — is found; only fall back to `~/.openagents/bin` when nothing is beside it. Test with a fake exe path ending in " (deleted)".

## Change plan

1. Goal: Engine lookup falls back to an old ~/.openagents/bin/microcoder when the running binary was rebuilt (Linux "(deleted)" path)
2. Change the behavior where it lives, most likely in `crates/coder/src/task/autostart.rs`, `crates/coder/src/task/local.rs`, `INVARIANTS.md`.
3. Add or update a test that pins the new behavior, next to the existing ones in `crates/microcoder/src/repository/tests.rs`.
4. Run `check:coder`, then `test:coder`, then `check:microcoder`, then `test:microcoder`, then `fmt`; stop when they pass.

## Files to change

### `crates/coder/src/task/autostart.rs`

Why: the issue names `crates/coder/src/task/autostart.rs`; has `default_controller`; has `current_exe`; has `MICROCODER`; has `~/.openagents/bin`

```
--- lines 2097-2127 of 4469 ---
 2097                  }
 2098                  engine
 2099              } else {
 2100                  // Runs have no step or time limit. The old options are
 2101                  // still accepted, so an older script keeps working, and do
 2102                  // nothing.
 2103                  take_one(&mut values, "--max-steps")?;
 2104                  take_one(&mut values, "--wall-seconds")?;
 2105                  let memory_mib =
 2106                      number(take_one(&mut values, "--memory-mib")?, "--memory-mib", 4096)?;
 2107                  let controller = match take_one(&mut values, "--controller")? {
 2108                      Some(path) => PathBuf::from(path),
 2109                      None => default_controller()?,
 2110                  };
 2111                  let controller = controller.canonicalize().map_err(|_| {
 2112                      format!("the controller {} does not exist", controller.display())
 2113                  })?;
 2114                  let routes = values
 2115                      .remove("--route")
 2116                      .unwrap_or_default()
 2117                      .iter()
 2118                      .map(|route| parse_route(route))
 2119                      .collect::<std::result::Result<Vec<Route>, String>>()?;
 2120                  let model = take_one(&mut values, "--model")?;
 2121                  let model = match (routes.first(), model) {
 2122                      (Some(_), Some(_)) => {
 2123                          return Err(
 2124                              "usage: give --model or --route, not both; name the Codex model as --route codex:MODEL"
 2125                                  .into(),
 2126                          );
 2127                      }
--- lines 2330-2403 of 4469 ---
 2330      let common = common.canonicalize().unwrap_or(common);
 2331      if common.starts_with(&root) {
 2332          return Err(format!(
 2333              "{} holds its own Git directory; give the host an isolated worktree \
 2334               (git worktree add) or pass --read-only",
 2335              root.display()
 2336          ));
 2337      }
 2338      Ok(())
 2339  }
 2340  
 2341  /// The engine's file name on this platform.
 2342  const MICROCODER: &str = if cfg!(windows) {
 2343      "microcoder.exe"
 2344  } else {
 2345      "microcoder"
 2346  };
 2347  
 2348  /// The engine beside the running program, else the one its macOS app
 2349  /// bundle ships, else the installed one.
 2350  ///
 2351  /// # Errors
 2352  /// Names where it looked.
 2353  pub fn default_controller() -> std::result::Result<PathBuf, String> {
 2354      let exe = std::env::current_exe()
 2355          .ok()
 2356          .and_then(|exe| exe.canonicalize().ok());
 2357      let home = std::env::var_os("HOME").map(PathBuf::from);
 2358      controller_candidates(exe.as_deref(), home.as_deref())
 2359          .into_iter()
 2360          .find(|path| path.is_file())
 2361          .ok_or_else(|| {
 2362              "no microcoder beside coder or in ~/.openagents/bin; pass --controller".into()
 2363          })
 2364  }
 2365  
 2366  /// Where [`default_controller`] looks, in order: beside `exe`; when `exe`
 2367  /// is the `openagents` CLI in an app bundle's `Contents/Helpers`, the
 2368  /// bundle's `Contents/MacOS` (where `scripts/desktop/package-macos.sh`
 2369  /// puts `coder` and `microcoder`, so the CLI runs the engine it shipped
 2370  /// with rather than an older `~/.openagents/bin` copy that refuses a
 2371  /// newer grant's shape); then `~/.openagents/bin`.
 2372  fn controller_candidates(exe: Option<&Path>, home: Option<&Path>) -> Vec<PathBuf> {
 2373      let mut candidates = Vec::new();
 2374      if let Some(dir) = exe.and_then(Path::parent) {
 2375          candidates.push(dir.join(MICROCODER));
 2376          if dir.file_name().is_some_and(|name| name == "Helpers")
 2377              && let Some(contents) = dir
 2378                  .parent()
 2379                  .filter(|contents| contents.file_name().is_some_and(|name| name == "Contents"))
 2380          {
 2381              candidates.push(contents.join("MacOS").join(MICROCODER));
 2382          }
 2383      }
 2384      if let Some(home) = home {
 2385          candidates.push(home.join(".openagents/bin").join(MICROCODER));
 2386      }
 2387      candidates
 2388  }
 2389  
 2390  #[cfg(test)]
 2391  mod tests {
 2392      use super::*;
 2393  
 2394      /// No login identity: tests never read the person's own files.
 2395      fn no_login(_: Provider) -> Option<String> {
 2396          None
 2397      }
 2398      use crate::task::remote::Inbox;
 2399      use coder_host::{Code, TaskCreate, Tasks};
 2400      use std::cell::Cell;
 2401  
 2402      thread_local! {
 2403          // Each test runs on its own thread and sweeps in the foreground, so
--- lines 2979-3009 of 4469 ---
 2979      #[test]
 2980      fn the_command_line_turns_the_policy_on_and_off() {
 2981          let dir = tempfile::tempdir().unwrap();
 2982          let root = dir.path().join("host");
 2983          let workspace = dir.path().join("checkout");
 2984          std::fs::create_dir_all(&workspace).unwrap();
 2985          coder_host::settings::ServeSettings::new(
 2986              vec!["wss://relay.example/".into()],
 2987              BTreeMap::from([("checkout".into(), workspace.canonicalize().unwrap())]),
 2988          )
 2989          .save(&root)
 2990          .unwrap();
 2991          let controller = std::env::current_exe().unwrap();
 2992          let args = |list: &[&str]| -> Vec<String> {
 2993              let mut args: Vec<String> = list.iter().map(|a| (*a).to_owned()).collect();
 2994              args.extend(["--root".into(), root.to_string_lossy().into_owned()]);
 2995              args
 2996          };
 2997          let controller = controller.to_string_lossy().into_owned();
 2998          // An unknown label refuses; so does a writing policy on a directory
 2999          // that is not an isolated worktree.
 3000          let on = |label: &str, extra: &[&str]| {
 3001              let mut list = vec!["on", "--workspace", label, "--controller", &controller];
 3002              list.extend_from_slice(extra);
 3003              cli(&args(&list))
 3004          };
 3005          assert_eq!(on("nope", &["--read-only"]), 1);
 3006          assert_eq!(on("checkout", &[]), 1);
 3007          assert_eq!(on("checkout", &["--read-only", "--max-running", "2"]), 0);
 3008          let policy = Policy::load(&root).unwrap().unwrap();
 3009          assert!(policy.enabled && !policy.engine.write_workspace);
--- lines 3025-3028 of 4469 ---
 3025          let (one, two) = (dir.path().join("one"), dir.path().join("two"));
 3026          std::fs::create_dir_all(&one).unwrap();
 3027          std::fs::create_dir_all(&two).unwrap();
 3028          coder_host::settings::ServeSettings::new(
```

### `crates/coder/src/task/local.rs`

Why: has `default_controller`; has `current_exe`; has `~/.openagents/bin`

```
--- lines 60-129 of 3594 ---
   60  /// then Claude Code, then Grok Build (#10091), with the models the
   61  /// desktop's auto-start switch admits. Since #10184 the settings admit
   62  /// every coding agent not turned off ([`settings::Coder::routes`]); these
   63  /// remain the first routes of that default order.
   64  pub const ROUTES: [(Provider, &str); 3] = [
   65      (Provider::Codex, "gpt-6.1-sol"),
   66      (Provider::Claude, "claude-opus-5-5"),
   67      (Provider::Grok, acp_client::grok::DEFAULT_MODEL),
   68  ];
   69  /// Names another task store than [`default_store`].
   70  pub const STORE_VAR: &str = "OPENAGENTS_TASKS";
   71  /// Names the engine (`microcoder`) instead of the one beside the running
   72  /// program or in `~/.openagents/bin`.
   73  pub const CONTROLLER_VAR: &str = "OPENAGENTS_CODER_CONTROLLER";
   74  /// The record a local run keeps beside its task.
   75  pub const RECORD_SCHEMA: &str = "openagents.coder.local-run.v1";
   76  /// The host name a thread's binding gives a local run.
   77  pub const LOCAL_HOST: &str = "local";
   78  // A turn has no step or time limit: it ends when Coder finishes or asks,
   79  // when the person stops it, or when the loop's stuck guard finds it
   80  // repeating a failed approach without progress (#10103).
   81  const MEMORY_BYTES: u64 = 4096 * 1024 * 1024;
   82  /// How long a started turn may wait for its owner before a missing
   83  /// admission counts as a failure, when the owner left no diagnostic.
   84  const ADMISSION_WAIT: u64 = 120;
   85  
   86  /// The task store local runs use: `$OPENAGENTS_TASKS`, else
   87  /// `~/.openagents/tasks`, the store `coder task` and a host on this
   88  /// computer use by default.
   89  #[must_use]
   90  pub fn default_store() -> PathBuf {
   91      if let Some(dir) = std::env::var_os(STORE_VAR).filter(|v| !v.is_empty()) {
   92          return PathBuf::from(dir);
   93      }
   94      std::env::var_os("HOME")
   95          .map_or_else(|| PathBuf::from("."), PathBuf::from)
   96          .join(".openagents/tasks")
   97  }
   98  
   99  /// The engine that runs a turn: `$OPENAGENTS_CODER_CONTROLLER`, else the
  100  /// `microcoder` beside the running program, in its app bundle, or in
  101  /// `~/.openagents/bin` ([`autostart::default_controller`]).
  102  ///
  103  /// # Errors
  104  /// Names where it looked.
  105  pub fn controller() -> Result<PathBuf, String> {
  106      if let Some(path) = std::env::var_os(CONTROLLER_VAR).filter(|v| !v.is_empty()) {
  107          return PathBuf::from(path)
  108              .canonicalize()
  109              .map_err(|_| format!("{CONTROLLER_VAR} names no file"));
  110      }
  111      autostart::default_controller().and_then(|path| {
  112          path.canonicalize()
  113              .map_err(|_| "the microcoder engine is missing".into())
  114      })
  115  }
  116  
  117  /// A person's Git checkout.
  118  #[derive(Clone, Debug, PartialEq, Eq)]
  119  pub struct Checkout {
  120      /// Its top level.
  121      pub top: PathBuf,
  122      /// Its folder's name, which names the project.
  123      pub name: String,
  124      /// The commit `HEAD` names.
  125      pub head: String,
  126  }
  127  
  128  pub(crate) fn git() -> std::process::Command {
  129      let program = owner::GIT_PATHS
--- lines 2207-2237 of 3594 ---
 2207              Provider::Claude => Connection::Connected,
 2208              _ => Connection::Missing("no login".into()),
 2209          }
 2210      }
 2211  
 2212      fn nobody(_: Provider) -> Connection {
 2213          Connection::Missing("no login".into())
 2214      }
 2215  
 2216      fn local(dir: &Path, probe: fn(Provider) -> Connection) -> Local {
 2217          Local::new(dir.join("tasks"))
 2218              .with_probe(probe)
 2219              .with_controller(std::env::current_exe().unwrap())
 2220              .with_identify(|_| None)
 2221              .with_opencode_model(|| None)
 2222      }
 2223  
 2224      /// The owner's Mac on 2026-10-01 (#10113): Codex, Claude Code, and
 2225      /// Grok Build signed in, Devin signed in, and OpenCode installed. The
 2226      /// context and the welcome card name every one of them with its own
 2227      /// state, in the order Coder tries them, not only the one a run would
 2228      /// start on. Since #10184 nothing has to be enabled: Devin signed in is
 2229      /// ready with no settings at all, and only a turn-off lists an agent
 2230      /// as not enabled.
 2231      #[test]
 2232      fn every_coding_agent_here_is_listed_with_its_state() {
 2233          use openagents_chat::router::{Engine as Agent, EngineState as S};
 2234          fn all_but_opencode(provider: Provider) -> Connection {
 2235              match provider {
 2236                  Provider::OpenCode => Connection::Missing("no opencode".into()),
 2237                  _ => Connection::Connected,
```

### `INVARIANTS.md`

Why: has `crates/coder/src/task/autostart.rs`

```
--- lines 1-46 of 599 ---
    1  # Invariants
    2  
    3  This ledger records invariants of this repository whose change is a policy
    4  change. The workspace-level guide is `INVARIANTS.md` at the workspace root.
    5  When a change adds, removes, relaxes, or reinterprets an invariant here,
    6  update this file in the same change and name the test that checks it.
    7  
    8  ## Remote task creation
    9  
   10  | Invariant | Status | Checked by |
   11  | --- | --- | --- |
   12  | NIP-HOST `task.create` from an enrolled device is an inert inbox submission: it records intent and grants no execution authority. | Relaxed on 2026-09-27 by the owner's auto-start policy ([#9735](https://github.com/OpenAgentsInc/openagents/issues/9735)). Holds whenever the policy is absent, unreadable, or off. | `without_a_policy_creation_is_inert_and_unchanged` and `creation_is_inert_idempotent_and_bound_to_labels` in `crates/coder` |
   13  | Only the host's owner, with a command on the host, turns auto-start on or widens it. A device sends only a workspace label, a title, a prompt, its own images, and at most the engine the person asked for, and cannot choose the routes, model, or limits: the engine only reorders the routes the owner's policy already admits. | New on 2026-09-27. Reinterpreted on 2026-09-29 ([#9969](https://github.com/OpenAgentsInc/openagents/issues/9969)): a request on the host's local control socket (the desktop app's switch or `openagents connect`) is a command on the host, and runs the host's own `coder host autostart`; a device still cannot. The switch changes only whether the policy is on, its projects, and how many run (`on --keep-engine`); the engine the owner set up (controller, routes, full access, usage probes) stays, and removing a project takes it off the policy (2026-09-29). Reinterpreted on 2026-09-30 ([#10076](https://github.com/OpenAgentsInc/openagents/issues/10076)): a start from the host's own chat (`thread.run`, the desktop handoff) puts first the engine that chat's typed `run_coder` offer named, through the host-only `Tasks::prefer`, and only among the routes the owner's policy admits; it never sets a model, a limit, or a route the policy does not admit, and a device's `task.create` still carries no engine. Reinterpreted on 2026-09-30 ([#10081](https://github.com/OpenAgentsInc/openagents/issues/10081)): a device's `task.create` may carry `engine`, the typed engine of its chat's `run_coder` offer, and the host reads it exactly as `Tasks::prefer` (the host's own preference wins when both are present): first only among the policy's admitted routes, never adding a route, a model, or a limit, and the task's summary says plainly when it does not run. A device sends it only to a host whose presence advertises `task-engine`. | `the_command_line_turns_the_policy_on_and_off`, `policies_outside_their_bounds_refuse`, `the_desktop_switch_keeps_the_owners_engine_settings`; `auto_start_changes_run_the_hosts_own_command` in `crates/coder-host/tests/control.rs`; `a_requested_engine_starts_first_when_the_policy_admits_it`, `a_requested_engine_without_capacity_falls_back`, `a_devices_requested_engine_starts_first_when_the_policy_admits_it`, `a_devices_requested_engine_outside_the_policy_falls_back_and_says_why`, `a_devices_requested_engine_at_its_limit_says_until_when` in `crates/coder/src/task/autostart.rs`; `a_phone_runs_coder_from_a_host_threads_offer` in `crates/coder-host/tests/threads.rs`; `a_phone_asks_the_computer_for_the_engine_the_person_named` in `crates/coder/tests/phone_engine.rs` |
   14  | An auto-started task runs only in a workspace the policy lists and the host admits, under a normal operator execution grant that the task owner admits with every usual check. | New on 2026-09-27. | `a_policy_starts_admitted_tasks_within_its_bounds_and_records_each` |
   15  | At most `max_running` auto-started tasks run at once, and every eligible, started, skipped, refused, and no-capacity decision is appended to the host's `autostart.jsonl` before the next. | New on 2026-09-27; `no_capacity` added on 2026-09-28. | `a_policy_starts_admitted_tasks_within_its_bounds_and_records_each`, `turning_the_policy_off_stops_new_starts_and_cancelled_tasks_are_skipped`, `without_capacity_a_task_ends_as_no_capacity_with_the_reset` |
   16  | An auto-started task generates only through a route (provider and model) the owner's policy admits. The host picks the first admitted route that is connected and has capacity; failover during a run picks only among the grant's admitted routes. Neither adds a model, widens a limit, or relaxes a spend bound. | New on 2026-09-28 ([#9831](https://github.com/OpenAgentsInc/openagents/issues/9831)). It reinterprets one admission check: the task's requested model must be one of the grant's admitted routes, not only its primary one. A grant without `fallbacks` admits exactly as before, and a policy without `routes` means exactly what it meant. | `a_task_starts_on_the_first_connected_route_with_capacity`, `an_existing_policy_file_keeps_its_meaning`, `admission_accepts_the_task_model_on_any_admitted_route`, `a_capacity_refusal_fails_over_to_the_next_admitted_route_and_is_recorded` |
   17  | A task that no admitted, connected provider has capacity for does not start a run; it ends as `no_capacity` with the earliest recorded reset, and a device's summary headline says so from typed host state. | New on 2026-09-28. | `without_capacity_a_task_ends_as_no_capacity_with_the_reset`, `with_every_route_exhausted_the_run_ends_as_no_capacity_with_the_earliest_reset` |
   18  | An auto-started task never stays queued forever waiting for its owner process. When the owner exits without admitting it, or has not admitted it within 10 minutes, the host records `unadmitted` and either starts the turn once more (only for an unexplained stop or a refused admission, at most two starts per turn) or cancels it and records `not_started` with a typed cause. A device's summary headline says why from that typed cause, never from the owner's output. | New on 2026-09-29 ([#9985](https://github.com/OpenAgentsInc/openagents/issues/9985)). Before, an unadmitted task was recorded once and left queued. | `an_owner_that_exits_without_admitting_ends_the_task_with_its_reason`, `an_owner_that_stops_without_a_reason_is_started_once_more_then_ends`, `an_owner_that_runs_but_never_admits_ends_the_task_at_the_deadline`, `a_task_an_earlier_host_left_unadmitted_ends_at_the_next_sweep` in `crates/coder`; `every_not_started_cause_survives_the_summary_disclosure_rules` in `crates/coder-host` |
   19  | An auto-started task's commands run inside the filesystem boundary (writes only to the workspace and scratch, no external network, a cleared environment with the system `PATH`) unless the host's owner turned on full access with a command on the host (`coder host autostart on --full-access`). With full access, commands run as the host's user with no sandbox, network access, and the owner's login-shell environment and real `HOME`, `USER`, and `LOGNAME`, less variables named `*_API_KEY`, `*_TOKEN`, or `*_SECRET`; a Claude route's call adds `--permission-mode bypassPermissions` and still has every tool off. A device cannot ask for it, and every grant, right, workspace, route, limit, and concurrency check still applies. A container grant refuses it. | New on 2026-09-28, owner-directed: owner-host Coder tasks need network access and the owner's installed tools. It relaxes the boundary only on the owner's own hosts, for tasks the owner's admitted devices create; the task store and Git directory lose their protection from commands there. Off by default; a policy or grant without `access` keeps the boundary. | `the_owner_turns_on_full_access_on_the_host`, `an_existing_policy_file_keeps_its_meaning` in `crates/coder`; `a_login_listing_keeps_the_path_and_leaves_out_credentials`, `a_shell_that_cannot_answer_leaves_the_fallback_path`; `full_access_runs_as_the_owner_with_no_sandbox`, `full_access_is_refused_for_container_commands`, `a_full_access_call_bypasses_permissions_and_still_runs_no_tool` in `crates/microcoder` |
```

### `crates/microcoder/src/kbstudy.rs`

Why: has `current_exe`; has `MICROCODER`

```
--- lines 64-94 of 582 ---
   64          Some("report") if args.len() == 2 => {
   65              let store = Store::open(Path::new(&args[1]))?;
   66              let report = report(&store)?;
   67              println!("{}", serde_json::to_string_pretty(&report).map_err(io)?);
   68              Ok(())
   69          }
   70          _ => Err(USAGE.into()),
   71      }
   72  }
   73  fn draft(snapshot: &Path, tasks: &Path, output: &Path, names: &[String]) -> Result<(), String> {
   74      let verified = knowledge::snapshot::read(snapshot)?;
   75      let candidate = std::fs::read(snapshot).map_err(io)?;
   76      let binary = std::env::current_exe()
   77          .map_err(io)?
   78          .canonicalize()
   79          .map_err(io)?;
   80      let tasks_root = tasks.canonicalize().map_err(io)?;
   81      let mut random = [0_u8; 16];
   82      std::fs::File::open("/dev/urandom")
   83          .map_err(io)?
   84          .read_exact(&mut random)
   85          .map_err(io)?;
   86      let study = format!(
   87          "study-{}",
   88          random
   89              .iter()
   90              .map(|b| format!("{b:02x}"))
   91              .collect::<String>()
   92      );
   93      let mut cases = Vec::new();
   94      for task in names {
--- lines 259-289 of 582 ---
  259              assignment.id, assignment.task, assignment.arm
  260          );
  261          let task = crate::tbench::find(&store.plan.tasks_root, &assignment.task)?;
  262          let deadline = store
  263              .plan
  264              .configuration
  265              .max_seconds
  266              .saturating_add(task.verifier_seconds)
  267              .saturating_add(600);
  268          let mut command = std::process::Command::new(&store.plan.binary);
  269          command
  270              .args(&args)
  271              .env("MICROCODER_TASKS", &store.plan.tasks_root)
  272              .env(
  273                  jev::env::BASE_URL,
  274                  &store.plan.configuration.decision_base_url,
  275              )
  276              .env(
  277                  jev::env::DEFAULT_MODEL,
  278                  &store.plan.configuration.decision_model,
  279              );
  280          let clock = Instant::now();
  281          let ended = supervise::Job::from_command(command)
  282              .bounded(supervise::Limits::within(Duration::from_secs(deadline)).keeping(1024 * 1024))
  283              .run()
  284              .await;
  285          std::fs::write(
  286              store.root.join(format!("{}.stdout.txt", assignment.id)),
  287              &ended.stdout.text,
  288          )
  289          .map_err(io)?;
```

### `crates/microcoder/src/repository/tests.rs`

Why: has `current_exe`; has `MICROCODER`

```
--- lines 231-261 of 2983 ---
  231      assert!(result.engine_microusd.is_some() && result.jev_microusd.is_some());
  232      let text = serde_json::to_string(&view).unwrap();
  233      assert!(
  234          text.contains("full command output")
  235              && text.contains("generation")
  236              && text.contains("decision")
  237      );
  238      assert!(text.contains("fixture-model") && text.contains("medium"));
  239      assert_eq!(
  240          std::fs::read(root.path().join("checkout/result.txt")).unwrap(),
  241          b"output"
  242      );
  243      if let Some(destination) = std::env::var_os("MICROCODER_REPOSITORY_ACCEPTANCE_DIR") {
  244          let destination = std::path::PathBuf::from(destination);
  245          std::fs::create_dir(&destination).unwrap();
  246          std::fs::write(destination.join("grant.json"), &grant).unwrap();
  247          std::fs::write(
  248              destination.join("task.json"),
  249              serde_json::to_vec_pretty(&result).unwrap(),
  250          )
  251          .unwrap();
  252          std::fs::write(
  253              destination.join("view.json"),
  254              serde_json::to_vec_pretty(&view).unwrap(),
  255          )
  256          .unwrap();
  257          std::fs::copy(
  258              store.join("fixture.1.atif.jsonl"),
  259              destination.join("trace.atif.jsonl"),
  260          )
  261          .unwrap();
--- lines 753-804 of 2983 ---
  753      let generator = ContextGenerator {
  754          prompt: RefCell::new(String::new()),
  755      };
  756      run(host, &generator, &JudgeFixture).await.unwrap();
  757      let prompt = generator.prompt.borrow();
  758      assert!(prompt.contains("Exact frozen reference bytes."));
  759      assert!(prompt.contains("fixture.reference") && prompt.contains("independent-reference"));
  760  }
  761  
  762  fn docker_grant(bytes: &[u8]) -> Vec<u8> {
  763      let mut grant = task::owner::Grant::parse(bytes).unwrap();
  764      let program = std::path::PathBuf::from(
  765          std::env::var_os("MICROCODER_REPOSITORY_DOCKER_PROGRAM").expect("explicit Docker program"),
  766      )
  767      .canonicalize()
  768      .unwrap();
  769      grant.wall_seconds = 60;
  770      grant.adapter_configuration.as_mut().unwrap().container =
  771          Some(task::adapter::container::Profile {
  772              schema: "openagents.microcoder.container.v1".into(),
  773              docker_digest: nostr::contracts::digest_bytes(&std::fs::read(&program).unwrap()),
  774              docker_program: program,
  775              socket: std::path::PathBuf::from(
  776                  std::env::var_os("MICROCODER_REPOSITORY_DOCKER_SOCKET")
  777                      .expect("explicit Docker socket"),
  778              )
  779              .canonicalize()
  780              .unwrap(),
  781              image: std::env::var("MICROCODER_REPOSITORY_DOCKER_IMAGE").expect("explicit image ID"),
  782              uid: std::env::var("MICROCODER_REPOSITORY_DOCKER_UID")
  783                  .unwrap()
  784                  .parse()
  785                  .unwrap(),
  786              gid: std::env::var("MICROCODER_REPOSITORY_DOCKER_GID")
  787                  .unwrap()
  788                  .parse()
  789                  .unwrap(),
  790          });
  791      serde_json::to_vec(&grant).unwrap()
  792  }
  793  
  794  #[tokio::test]
  795  #[ignore = "requires an explicitly admitted local Docker socket, executable, image, and user"]
  796  async fn docker_repository_loop_retains_outputs_and_reconciles_whole_containers() {
  797      let (root, store, grant) = fixture();
  798      let grant = docker_grant(&grant);
  799      let outside = root.path().join("outside");
  800      std::fs::write(&outside, "unchanged").unwrap();
  801      let generator = generator(
  802          "set -e; test -z \"$TYPESAFE_API_KEY\"; test -z \"$OPENAI_API_KEY\"; test -z \"$HOME_SECRET\"; test ! -e /var/run/docker.sock; if printf denied > /outside-write; then exit 10; fi; if printf changed > .git; then exit 11; fi; printf output > result.txt; printf 'container output'",
  803      );
  804      let host = Host::admit(&store, &grant).await.unwrap();
--- lines 812-842 of 2983 ---
  812              .result
  813              .as_ref()
  814              .unwrap()
  815              .group_clear
  816      );
  817      assert_eq!(
  818          task::artifact::read(&store, "fixture", Path::new("result.txt")).unwrap(),
  819          b"output"
  820      );
  821      assert_eq!(std::fs::read(outside).unwrap(), b"unchanged");
  822      let trace = std::fs::read_to_string(store.join("fixture.1.atif.jsonl")).unwrap();
  823      assert!(trace.contains("container output") && trace.contains("\"container_removed\":true"));
  824      if let Some(destination) = std::env::var_os("MICROCODER_REPOSITORY_CONTAINER_PROOF_DIR") {
  825          let destination = std::path::PathBuf::from(destination);
  826          std::fs::create_dir(&destination).unwrap();
  827          std::fs::write(destination.join("grant.json"), grant).unwrap();
  828          std::fs::write(
  829              destination.join("task.json"),
  830              serde_json::to_vec_pretty(&result).unwrap(),
  831          )
  832          .unwrap();
  833          std::fs::write(destination.join("trace.atif.jsonl"), trace).unwrap();
  834          std::fs::write(
  835              destination.join("view.json"),
  836              serde_json::to_vec_pretty(&task::view::read(&store, "fixture", None, 200).unwrap())
  837                  .unwrap(),
  838          )
  839          .unwrap();
  840      }
  841  }
  842  
--- lines 1926-1951 of 2983 ---
 1926              // Turn 2, after the answer: Codex is still refused in the
 1927              // book, so Claude Code starts, and finishes.
 1928              (
 1929                  vec![],
 1930                  vec![
 1931                      Ok(write("printf 'x = 1\\n' >> test_slugs.py")),
 1932                      Ok(finished("I added test_slugs.py.")),
 1933                  ],
 1934              ),
 1935          ]);
 1936          let local = Local::new(store.clone())
 1937              .with_probe(signed_in)
 1938              .with_controller(std::env::current_exe().unwrap())
 1939              .with_launcher(Box::new(Scripted(Mutex::new(script), None)));
 1940          let record = local
 1941              .start(
 1942                  &top,
 1943                  "add a unit test for slugify",
 1944                  "add a unit test for slugify",
 1945                  Some(&"a".repeat(32)),
 1946              )
 1947              .unwrap();
 1948          let (turn_one, state) = drain(&local, &record.task);
 1949          assert_eq!(state, State::Waiting);
 1950          let seen = names(&turn_one);
 1951          for name in [
```

### `docs/coder/runtime/host-autostart.md`

Why: has `crates/coder/src/task/autostart.rs`; has `~/.openagents/bin`

```
--- lines 1-56 of 409 ---
    1  # Host auto-start policy
    2  
    3  A task that an enrolled device creates with NIP-HOST `task.create` is an
    4  inert submission: the durable inbox records it, and nothing runs until the
    5  host's local operator starts it with an execution grant. The auto-start
    6  policy lets the host's owner say, once and locally, that tasks created in
    7  chosen workspaces start on their own within stated bounds. It is off by
    8  default.
    9  [Issue #9735](https://github.com/OpenAgentsInc/openagents/issues/9735)
   10  delivers it as part of the
   11  [linked devices program](https://github.com/OpenAgentsInc/openagents/issues/9736).
   12  The implementation is `coder::task::autostart` in
   13  [`crates/coder`](../../../crates/coder/src/task/autostart.rs).
   14  
   15  ## Turn it on
   16  
   17  The policy lives in the host root, `~/.openagents/host/autostart.json`
   18  (mode `0600`), and only a command on the host itself changes it. No device
   19  operation, relay message, or grant can.
   20  
   21  ```sh
   22  coder host autostart on --workspace openagents --max-running 1
   23  ```
   24  
   25  | Option | Default | Meaning |
   26  | --- | --- | --- |
   27  | `--workspace LABEL` | Required | A workspace label the host admits, from `coder host init --workspace`. Repeat for more. |
   28  | `--max-running N` | `1` | At most N auto-started tasks run at once, 1 to 8. The rest wait queued. |
   29  | `--model ID` | `gpt-6-luna` | The Codex model each eligible task records and its grant admits, when no `--route` is given. |
   30  | `--route PROVIDER:MODEL` | None | An admitted provider (`codex`, `claude`, `devin`, `opencode`, or `grok`; repository runs don't generate through `vertex`, the OpenAgents cloud fallback, which answers terminal and phone turns only) and model, in preference order. A `devin` route hands the whole turn to the local Devin CLI; see [the Devin route](devin.md). An `opencode` route names OpenCode's `PROVIDER/MODEL` and hands the whole turn to OpenCode; see [the OpenCode route](opencode.md). A `grok` route (`grok:default` or `grok:MODEL`) hands the whole turn to Grok Build; see [the Grok Build route](grok.md). Repeat for more, up to five. The first route's model is the one each task records. Use instead of `--model`. See [Routes and capacity](#routes-and-capacity). |
   31  | `--probe-usage` | Off | Read each admitted provider's usage windows before routing, with its local login, and prefer a route below the threshold. See [Usage probes](#usage-probes). |
   32  | `--usage-threshold PERCENT` | `90` | The utilization, 1 to 100, at or above which a probed provider is passed over. Implies `--probe-usage`. |
   33  | `--effort LEVEL` | `medium` | `low`, `medium`, `high`, or `xhigh`, for every route. |
   34  | `--max-steps N`, `--wall-seconds N` | None | Accepted for older scripts and ignored. A run has no step or time limit ([#10103](https://github.com/OpenAgentsInc/openagents/issues/10103)): it ends when Coder finishes or asks, when the task is stopped, or when the loop's stuck guard finds it repeating a failed approach without progress for eight judged steps in a row ([how a run ends](../../cli/chat.md#coder-on-this-computer)). An older `autostart.json` that carries `max_steps` or `wall_seconds` still loads and keeps them, and they are ignored; grants written now carry neither. |
   35  | `--memory-mib N` | `4096` | Each command's memory limit, 64 MiB to 8 GiB. |
   36  | `--read-only` | Off | Grant a read-only workspace. Without it, the workspace must be an isolated Git worktree whose common Git directory is outside it. |
   37  | `--full-access` | Off | Run each task's commands as you, with network access and your login-shell environment. On macOS the only sandbox keeps them out of the folders macOS guards with a privacy prompt ([privacy prompts](privacy-prompts.md)). For your own computer only. See [Full access](#full-access). |
   38  | `--controller PATH` | `microcoder` beside `coder`, else `~/.openagents/bin/microcoder` | The engine executable. |
   39  | `--decision-endpoint URL`, `--decision-model ID` | `https://api.typesafe.ai`, `jev-1.13.0` | The Jev client the engine's grant names. Use an exact version: the engine refuses a reply whose model differs from the admitted one, so an alias such as `jev-latest` fails at the first judgment. |
   40  | `--keep-engine` | Off | Keep an existing policy's engine (controller, routes, access, and usage probes) and change only the workspaces and `--max-running`; the engine options then set up a first policy only. The desktop app's "Let my phone start Coder here" switch runs `on --keep-engine`, so it never undoes `--full-access`, `--probe-usage`, or a controller you set. Removing a project from the desktop app also takes its label off the policy, and turns the policy off when no project is left. |
   41  
   42  Every command also takes `--root DIR` for a host root other than
   43  `~/.openagents/host`. `on` refuses a label the host does not admit, and a
   44  writing policy on a checkout that holds its own Git directory; create a
   45  worktree for the host instead:
   46  
   47  ```sh
   48  git -C ~/work/openagents worktree add --detach ~/work/openagents-host-tasks origin/main
   49  coder host init --owner OWNER_PUBKEY --relay wss://relay.openagents.com/ \
   50    --workspace openagents=$HOME/work/openagents-host-tasks
   51  ```
   52  
   53  ### The task workspace
   54  
   55  Tasks run in the directory the workspace label names in the host's
   56  `serve.json`, not in your own checkout. The recommended default is one
```

## Similar past changes

### 8d4599b152 Coder: say plainly when the engine is older than this program (#10113)

```diff
diff --git a/crates/coder/src/task/autostart.rs b/crates/coder/src/task/autostart.rs
index 12b884dea8..162044d0d7 100644
--- a/crates/coder/src/task/autostart.rs
+++ b/crates/coder/src/task/autostart.rs
@@ -690,14 +690,7 @@ impl Launch for Process {
             .unwrap_or_default();
         if !output.status.success() {
-            let error = String::from_utf8_lossy(&output.stderr);
-            return Err(format!(
-                "the controller refused the launch: {}",
-                error
-                    .lines()
-                    .next()
-                    .unwrap_or("")
-                    .chars()
-                    .take(200)
-                    .collect::<String>()
+            return Err(refused(
+                &engine.controller,
+                &String::from_utf8_lossy(&output.stderr),
             ));
         }
@@ -712,4 +705,37 @@ impl Launch for Process {
 }
 
+/// Why the engine at `controller` refused a launch, in plain words, from
+/// the first line it wrote to standard error (#10113). The engine writes
+/// `{"error": ...}`; the person sees its sentence, never the JSON. An
+/// engine that cannot read the grant's shape is older than this program,
+/// and the sentence says so and how to update it.
+fn refused(controller: &Path, stderr: &str) -> String {
+    let line = stderr
+        .lines()
+        .map(str::trim)
+        .find(|line| !line.is_empty())
+        .unwrap_or("");
+    let said = match serde_json::from_str::<serde_json::Value>(line) {
+        Ok(value) => value["error"].as_str().map(str::to_owned),
+        Err(_) => Some(line.to_owned()).filter(|line| !line.is_empty()),
+    };
+    match said {
+        Some(error) if error == owner::GRANT_SHAPE => format!(
+            "the Coder engine at {} is older than this program; reinstall the OpenAgents app \
+             or rebuild microcoder",
+            controller.display()
+        ),
+        Some(error) => format!(
+            "the controller refused the launch: {}",
+            error
+                .chars()
+                .filter(|ch| !ch.is_control())
+                .take(200)
+                .collect::<String>()
+        ),
+        None => "the controller refused the launch without saying why".to_owned(),
+    }
+}
+
 /// The `claude` binary the engine runs: `CLAUDE_BIN` when it names a file,
 /// else the one [`microcoder_loop::claude::locate`] finds (which asks the
@@ -4269,4 +4295,74 @@ mod tests {
     }
 
+    /// #10113: a dev build ran an older `~/.openagents/bin/microcoder`
+    /// that could not read the newer grant, and the person saw the
+    /// engine's raw JSON. A refusal is a plain sentence: an older engine
+    /// is named with how to update it, and any other refusal shows the
+    /// engine's own words, never its JSON.
+    #[test]
+    #[cfg(unix)]
```

### 409ad8771b Users still see "usage limit" wording when Coder switches engines

```diff
diff --git a/crates/coder/src/task/local.rs b/crates/coder/src/task/local.rs
index f4073d800f..c1c9a3eed3 100644
--- a/crates/coder/src/task/local.rs
+++ b/crates/coder/src/task/local.rs
@@ -2583,4 +2583,9 @@ mod tests {
         // (#10120): the run fails over to Claude Code silently.
         assert_eq!(why, "Claude Code is signed in and has capacity.");
+        assert_eq!(runner.started("is running."), "Claude Code is running.");
+        let json = serde_json::to_value(&runner).unwrap();
+        assert_eq!(json["passed"][0]["why"], "refused");
+        assert_eq!(json["passed"][0]["kind"], "usage_limit");
+        assert_eq!(json["passed"][0]["until"], now + 3600);
         assert!(matches!(
             &runner,
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

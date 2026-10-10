# Background processes: user-defined, reliable, System One

Status: phase 1 (the disk monitor) implemented, 2026-10-02: `crates/background`,
`coder host serve`, `openagents background`, the terminal's `/background`,
and NIP-HOST `background.*`; see "Phase 1 as built" at the end. Plugins
can now bring background rules that run only while turned on
([Disk cleanup as a plugin](2026-10-02-disk-cleanup-plugin.md), #10165).
Phase 2 (rules from conversation and the general rule engine) implemented
the same day; see "Phase 2 as built" at the end. Phase 3 (Jev judgment of
unknown folders, escalation to Coder, the other built-ins, desktop and
phone, plugins) implemented the same day; see "Phase 3 as built" at the
end. Issues:
umbrella [#10155](https://github.com/OpenAgentsInc/openagents/issues/10155),
phase 1 [#10156](https://github.com/OpenAgentsInc/openagents/issues/10156),
phase 2 [#10157](https://github.com/OpenAgentsInc/openagents/issues/10157),
phase 3 [#10158](https://github.com/OpenAgentsInc/openagents/issues/10158).

## Why

On 2026-10-02 the owner's Mac (1.8 TB) filled to 100% twice. Coder runs
failed with `No space left on device`. Each time, the orchestrating
conversational agent noticed only after the failures and then spent turns
deleting directories by hand. What it found:

| Path | Size | What it is |
| --- | --- | --- |
| `~/.openagents/targets/<project>-<task>-<hash>` | 9–38 GB each, 131 GB after one day | One Cargo target directory per Coder task, never deleted. [#10148](https://github.com/OpenAgentsInc/openagents/issues/10148) replaced these with reusable slots and a cleanup on task end. |
| `~/.openagents/coder-one/target` | 85 GB | Stale Coder One build output. |
| `~/work/openagents-target-agentN` | 17–72 GB each | Agent target directories left behind by finished conversation-agent work. |
| `~/work/openagents/target` | 66 GB | The checkout's own build directory. |
| `~/.openagents/worktrees` | 35 GB, 29 worktrees | Coder task worktrees, many for ended tasks. |
| `~/.openagents/gate/*` | 55 GB | Release-gate pools and their builds. |
| `~/.openagents/pylon` | 47 GB | Pylon state. Not known to be disposable. |
| `~/Library/Caches`, `/private/var/folders` | 75 GB | Application and operating-system caches. |

The rules the agent applied by hand were simple: never delete a directory
that a running task or process uses, never touch the user's source checkouts
or uncommitted work, and treat Cargo target directories and ended tasks'
worktrees as disposable caches. Simple, repeated, and mechanical work like
this should not wait for a conversation. The owner's words: "There just
should be some processes that do automatic cleanup, user-defined, defined in
conversation but reliable."

## The System One framing

The TypeSafe material (`docs/research/typesafe/`) and episodes 285–287
(`docs/transcripts/`) make one argument that applies directly here:

- Today's models are trained for assistance, with a person in the loop.
  Automation needs "simple little bits of work that just need to be done
  reliably" running "in the background in a server that you never even look
  at" (Diogo Almeida, AI Engineer, 2026-07-31).
- The answer is not a smarter conversation. It is software that owns the
  workflow and calls a System One model, Jev, only for a bounded, typed
  judgment, with probabilities that code compares to a named threshold.
  "Code owns control. Jev judges. Executors generate." (episode 287).
- Episode 286 lists background processing as a first-class category for a
  System One coding agent: work that runs beside the normal workflow and
  reads shared state.

A background process is that idea made concrete. A conversation defines it
once. Code then runs it deterministically, every time, without a model in the
loop for routine work. Jev appears only where a judgment needs semantic
understanding, such as "is this unknown directory a disposable cache?", and
even then its answer informs a decision that code makes. Escalation to a
Coder run (System Two) happens only when the rule cannot handle the
situation.

## What a background process is

A background process is a durable, user-defined **rule**: a trigger, a
condition, and an ordered list of actions. The host runs it. A conversation
does not.

```text
trigger  ->  observe  ->  condition  ->  plan (dry run)  ->  act  ->  record  ->  notify
```

- **Defined in conversation.** The user says "keep my disk above 50 GB free;
  clear old build caches first." The chat router selects the
  `background.define` route, and a compiler turns the message into a typed
  rule (phase 2). The user sees the rule and a dry run of what it would do
  now, and confirms.
- **Compiled once.** The result is a typed definition, not a prompt. It is
  inspectable (`openagents background show`), editable in conversation or as
  a file, versioned, and digested. Re-running it never re-asks a model what
  the user meant.
- **Executed deterministically.** The host evaluates triggers and conditions,
  builds a plan, applies the safety checks, acts, and records every action.
- **Jev for bounded judgments only.** A rule may contain a judgment step: a
  Jev question set with a named `Setting` threshold, in the same pattern as
  `crates/coder-delegate/src/decision.rs` (`ISSUE_TURN_PLAIN` and the
  others). There is never an open-ended agent loop in a routine run.
- **Escalation is explicit.** When a rule cannot reach its goal, it notifies,
  and if the rule allows it, starts a Coder run with a briefing (phase 3).

There are no usage limits or quotas anywhere in this design. The disk
monitor is about the machine's disk, not about how much anyone uses
OpenAgents.

### The rule type

A rule is one JSON document, held at
`~/.openagents/background/rules/<id>.json`:

```text
Rule {
  id, name, version, digest,
  origin: BuiltIn | Conversation { thread, message } | File,
  enabled: bool,
  triggers: [Trigger],          // any one starts an evaluation
  conditions: [Condition],      // all must hold
  actions: [Action],            // run in order until the goal is met
  goal: Option<Goal>,           // for example: free >= 15% of the volume
  safety: Safety,               // allow and deny lists, trash window, dry run
  notify: Notify,               // when and where to tell the user
  escalate: Option<Escalation>, // what to do when the actions fall short
  cooldown: Duration,
}
```

Built-in rules ship with the host and the user edits their parameters. A
plugin can contribute a built-in rule, an action, or a candidate class
(phase 3); it declares them in its manifest with the same types.

### Triggers

| Trigger | Fires when | Notes |
| --- | --- | --- |
| `Interval { every }` | A timer elapses. | Jittered by up to 10% so several rules do not run at once. |
| `Daily { at }` | A local wall-clock time passes. | Missed runs (computer asleep) run once at wake. |
| `Threshold { metric, below \| above }` | A measured value crosses a bound. | Checked on the rule's interval. Metrics in phase 1: free bytes and free percent of a volume. |
| `TaskEnded { workspace? }` | A Coder task reaches `Finished` or `Cancelled`. | From the task store, the same signal `targets::cleanup` uses. |
| `HostStart` | The host starts. | Runs after the host is serving. |
| `FsEvent { paths, kinds }` | A watched path changes. | FSEvents on macOS, inotify on Linux. Phase 2. |

### Conditions

Conditions are typed predicates over an observation that code takes before
deciding: free space on a volume, a path's existence, size, or age, the state
of a task, the time of day, whether any Coder task is running, and whether
the host is on battery. One more kind, `Judgment`, asks Jev a question set
over a typed state and compares the answer to a named `Setting`. A judgment
condition is the only place a model appears in evaluation.

### Actions

Every action is built in and typed, and every action can produce a dry-run
plan before it changes anything.

| Action | Does |
| --- | --- |
| `DeleteCaches { classes, keep }` | Deletes candidate directories from the listed classes, in class order, until the goal is met. |
| `PruneWorktrees { ended_only, require_pushed }` | Removes Coder worktrees of ended tasks with `git worktree remove`, then `git worktree prune`. |
| `CargoCleanPartial { what }` | Removes part of an idle Cargo target directory: `debug/incremental`, `release/incremental`, or `doc`. Compiled dependencies stay. |
| `EmptyTrash { older_than }` | Empties the background trash. |
| `Notify { text }` | Sends a short notification. |
| `StartCoderRun { prompt, workspace, briefing }` | Starts a Coder task with a prompt and a code-built briefing (phase 3). |
| `RunPlugin { plugin, input }` | Runs an installed plugin's declared background action (phase 3). |

There is no "run any shell command" action. A user who needs one writes a
plugin, which declares what it reads and writes.

### Safety

Safety checks run on every candidate, in code, after any judgment:

1. **Allow and deny lists.** A candidate must be under an allowed root and
   not under a denied path. The deny list always contains the host's own
   state (`~/.openagents/host`, the task store, keys, the wallet, the
   background log and rules), `~/.claude`, `~/.codex`, and the user's
   documents folders. The user adds to it in conversation.
2. **No symbolic links, no other volumes.** A candidate that is a symbolic
   link, or whose device ID differs from its parent's (a mount point), is
   skipped. Walks never follow links.
3. **In-use detection.** A candidate is skipped when a running Coder task
   names it, when its lock is held (a target slot's `.lock`, Cargo's
   `.cargo-lock` in `debug/` or `release/`), or when any process has its
   working directory or an open file under it (`lsof` on macOS,
   `/proc/*/cwd` and `/proc/*/fd` on Linux). The check runs immediately
   before deletion, not only when planning.
4. **Never source or unsaved work.** A Git work tree is never deleted unless
   it is a Coder worktree of an ended task with a clean `git status` and no
   commits missing from every remote (`git rev-list HEAD --not --remotes` is
   empty). Git-ignored files count too: `git worktree remove` deletes them
   and undo only recreates the checkout, so a worktree that holds any
   ignored path outside a disposable cache (`.env`, `private/`, a dataset)
   is kept, and the dry run says why ("holds ignored files: .env,
   private/"). Disposable caches are paths with a component named
   `target`, `node_modules`, `dist`, `build`, `.build`, `.next`, `.turbo`,
   `.cache`, `.parcel-cache`, `.svelte-kit`, `.gradle`, `DerivedData`,
   `__pycache__`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, or
   `.DS_Store`, or starting with `.cargo-target` (#10166). Inside any other checkout, only the build directory itself is
   eligible, and only when it carries `CACHEDIR.TAG` (Cargo writes one) and
   Git ignores it.
5. **Dry run first.** `openagents background run ID --dry-run` and every
   rule edit show the exact plan: paths, classes, sizes, and why each one
   qualifies or is skipped. A new rule's first real run waits for the user
   to confirm its dry run.
6. **Trash and undo for anything that is not a known cache.** Known caches
   (classes 1, 2, and 5 below) are deleted directly; they rebuild. A removed
   worktree records its repository, branch, and commit, so
   `openagents background undo RUN` recreates it from the remote. A
   directory that Jev judged disposable (phase 3) moves to
   `~/.openagents/background/trash/<run>/` and stays for 24 hours. Moving to
   trash on the same volume frees nothing until the trash empties, so under
   an emergency (below) the trash empties oldest first, and that is recorded.
7. **Audit log.** Every action is recorded (see Records).

## The disk cleanup monitor

The first built-in rule, `disk`. Since #10165 it is off on a new host: the same rule ships as the Disk cleanup plugin (`plugins/disk-cleanup`), which a person turns on per computer ([Disk cleanup as a plugin](2026-10-02-disk-cleanup-plugin.md)). A saved `rules/disk.json` keeps its own setting.

### Default policy

| Setting | Default | On the owner's 1.8 TB disk |
| --- | --- | --- |
| Volumes | Each distinct volume holding `~/.openagents`, a Coder workspace, or an agent target directory, by device ID. | One volume. |
| Check | Every 5 minutes, and every minute while the last check found free space below the start level (`goal.pressure_secs`, 60); on every task end; and at host start. A check is one `statvfs` call per volume and costs nothing. | |
| Start cleaning when | Free space is below `max(200 GB, 15% of the volume)`. | Below 299 GB. |
| Stop cleaning when | Free space reaches `max(300 GB, 20% of the volume)`, or this run has freed 100 GB, whichever comes first. | At 399 GB free, or after 100 GB. |
| Emergency when | Free space is below `max(10 GB, 1% of the volume)`. Every class runs, the trash empties, and the notification is immediate. | Below 20 GB. |
| Cooldown | 10 minutes after a run, unless free space falls into emergency. | |
| Free space measure | `f_bavail`, the space available to the user. | |

The monitor measures sizes as allocated blocks (`st_blocks × 512`), not file
lengths, and caches directory sizes between runs so a check does not walk
the disk. A file hard-linked twice inside a folder counts once, and so does
an APFS clone family: on macOS, a file that shares blocks (its private size,
`ATTR_CMNEXT_PRIVATESIZE`, is below its allocation) counts in full the first
time its clone ID is seen and only its private bytes after that. It reports freed space two ways: the sum of what it deleted and
the change in free space. On macOS, when the change is much smaller than the
sum, local Time Machine snapshots probably hold the space; the monitor says
so and never deletes snapshots.

### Candidate classes, in order

The monitor works down this list and stops as soon as the goal is met. Within
a class, it takes the least recently used candidate first.

| # | Class | Paths | Qualifies when | In use when |
| --- | --- | --- | --- | --- |
| 1 | Ended tasks' target directories | `~/.openagents/targets/<project>-<task>-<hash>` (legacy per-task), any slot past the configured slot count, and any slot whose `build` lease ended with its session (`<slot>.lease.json`, #10760) | The task store lists the task as ended (`Finished` or `Cancelled`, checks not running, group clear: the `ended` test in `crates/coder/src/task/targets.rs`). For a slot with a lease record: no lease in its table names the session, and neither the session's process nor its agent process runs ([Leases](../coder/runtime/leases.md#slots-of-ended-sessions)). | A live task maps to the directory, or its lock is held. A slot used again since its lease ended, or whose session still lives, stays for class 2. |
| 2 | Stale target directories | Idle slots `~/.openagents/targets/*-slot-N`; `~/.openagents/coder-one/target`; agent target directories (`~/work/openagents-target-agent*`, configurable); a checkout's `target/` with `CACHEDIR.TAG` | Untouched for 6 hours (agent directories, `classes.agent_idle_hours`), 3 days (slots and Coder One, `classes.idle_days`), or 7 days (a checkout's `target/`). "Touched" is the newest of the lock file's mtime, `.cargo-lock`'s mtime, and the `.fingerprint` directory's mtime. | `.cargo-lock` or the slot lock is held, or a process has a working directory or open file inside. |
| 3 | Ended tasks' worktrees | `~/.openagents/worktrees/*` | The task is ended, `git status --porcelain` is empty, it holds no ignored file outside a disposable cache (Safety 4), and no commit is missing from every remote. A worktree with no task record qualifies only when it is also older than 7 days. | A process has a working directory or open file inside, or a task names it. |
| 4 | Gate pools | Build directories under `~/.openagents/gate/` that carry `CACHEDIR.TAG` | No gate run holds the gate's lock and none ran in the last hour. Checkouts in the gate pool follow class 3's rules. | The gate lock is held, or a gate or `verify-rust` process runs. |
| 5 | Incremental caches of live target directories | `debug/incremental` and `release/incremental` inside slots and agent target directories | Its Cargo lock is free. Compiled dependencies stay, so the next build is warm. | `.cargo-lock` is held. |
| 6 | Background trash (emergency only) | `~/.openagents/background/trash/*` | Oldest first. | Never. |
| 8 | Claude Code worktrees | `<checkout>/.claude/worktrees/*` in each checkout `classes.claude_checkouts` names (`~/code/*`, `~/work/*`) | Class 3's Git checks (clean, no ignored file outside a disposable cache, every commit on some remote, no stash made on it), it is not locked (Claude Code runs `git worktree lock` for a running agent), and its Git state (`HEAD`, index, and `HEAD` log) has not changed for 2 hours (`classes.claude_worktree_hours`). `git worktree remove` removes it; `undo` recreates it on its branch. | A process has a working directory or open file inside, or a task names it. |
| 9 | The kache store | kache's store (`kache stats --json` names it) | The store is over its cap. The planned size is the overage times the share of the store no target directory also holds. The run calls `background::kache::Kache::reclaim`, which runs `kache gc`; nothing under the store is deleted here. | kache's collector holds `gc.lock`. |
| 10 | Agent scratch | `~/.openagents/scratch/*`, one directory a session (`openagents scratch`, [Durable scratch](../coder/guides/scratch.md)) | The session ended: no lease in `~/.openagents/leases` names it, and for a session that names a process (`codex:4242`), that process is gone. Nothing in the directory changed for 7 days (`classes.scratch_days`). | A process has a working directory or open file inside, or the lease table can't be read. |

The default rule runs the classes in this order: 1 and 2, 3, 8, 10, 4 and
9, 5, then 6. Classes 8 and 9 came from the low-disk episodes of 2026-10-04 and
2026-10-05 (#10759): the runner was live through both, but with only class 5
eligible it freed less each run, down to nothing, while Claude Code
worktrees, kache, and agent target directories younger than three days held
the space. A rule file saved before these settings existed gets the 6-hour
and 2-hour defaults; it gains classes 8 and 9, the new levels, and the
one-minute check only by naming them, as the Disk cleanup plugin's 0.2.0
rule does. Class 10 (#10766) arrived with durable agent scratch; a saved rule
gains it the same way, as the plugin's 0.3.0 rule does.

[#10148](https://github.com/OpenAgentsInc/openagents/issues/10148) and this
monitor work together. Slot reuse stops the per-task growth at its source,
and its cleanup trims the slot pool when a task ends. The monitor cleans
everything slot reuse does not cover (agent target directories, Coder One,
worktrees, gate pools, a checkout's own `target/`) and acts on the disk's
actual free space, not on one directory's budget.

### What it never touches

- Any path outside the allowed roots, or under the deny list.
- `~/.openagents/pylon`, `~/Library/Caches`, `/private/var/folders`, and
  anything else not in a class above. These are measured and named in the
  report ("not cleaned: not a known cache") so the user can add a rule.
  `/private/var/folders` belongs to the operating system and is never a
  candidate.
- Source files, uncommitted changes, unpushed commits, and stashes.
- Anything a running task or process uses at the moment of deletion.
- Symbolic links and other volumes.
- The host's state, keys, wallet, and logs.

### How it decides ownership

Code answers "whose is this, and is it finished?" from evidence, in this
order:

1. **The task store.** Coder's task records name each task's workspace,
   worktree, target directory, and status.
2. **Locks.** The slot lock and Cargo's `.cargo-lock` are advisory file
   locks. A lock the monitor can take is not held by a build. The monitor
   takes it for the duration of the deletion, so a build cannot start
   mid-delete.
3. **Processes.** Open files and working directories, checked just before
   the deletion.
4. **Markers.** `CACHEDIR.TAG` and Git's ignore rules mark a build
   directory.
5. **Age.** Lock and fingerprint modification times.

Phase 1 needs no model. Phase 3 adds one judgment for directories no class
covers (below).

### What the user sees

Notifications are one or two plain lines, sent only when something happens:

- `Freed 84 GB: 3 build caches from ended tasks.`
- `Freed 41 GB: 2 old agent build folders, 6 finished worktrees. 312 GB free.`
- `Disk still low: 22 GB free. Largest not cleaned: ~/.openagents/pylon (47 GB), not a known cache.`
- `Disk almost full (9 GB free). Cleaned everything allowed: 12 GB.`

A check that finds enough free space sends nothing. The terminal status line
shows `disk ok · 312 GB free` only in `/background`.

### Changing it in conversation

Each request becomes a typed edit to the `disk` rule. The user sees the
change and the dry run before it applies.

| The user says | The edit |
| --- | --- |
| "only keep 2 agent target dirs" | Class 2, agent target directories: `keep: 2` (the two most recently used stay, regardless of age). |
| "never touch ~/.openagents/pylon" | Add `~/.openagents/pylon` to the deny list. |
| "keep 200 GB free" | Start threshold `200 GB`; stop threshold raised to stay above it. |
| "clean old build caches first" | No change: that is already the order. The reply says so. |
| "don't delete worktrees, just tell me" | Class 3 set to report only. |
| "pause disk cleanup until tomorrow" | `enabled: false` with a resume time. |

## Surfaces

**CLI.** `openagents background` with:

| Command | Does |
| --- | --- |
| `list` | Each rule: enabled or paused, last run, last result, next check. |
| `show ID` | The full definition, its version and digest, and its origin. |
| `add --file PATH` | Adds a rule from a JSON file (phase 1); from a message in phase 2. |
| `edit ID` | Opens the rule as JSON in `$EDITOR`, validates it, and shows the dry run. |
| `pause ID [--until TIME]`, `resume ID` | Stops or restarts a rule. |
| `run ID [--dry-run]` | Runs a rule now. Without a host, it runs in this process. |
| `log [ID] [--since TIME] [--stats]` | The audit log; `--stats` totals bytes freed by class and week. |
| `undo RUN` | Recreates removed worktrees and restores trashed directories from that run. |

Every command takes `--json`. Each is declared in the chat router's command
tree (`coder::cli_route::tree`) with its effect, as `service` is.

**Terminal.** `/background` lists the rules with their status. Enter shows a
rule, `r` runs it (dry run first), `p` pauses or resumes, and `l` shows its
log. A notification from a rule appears as one line in the transcript area.

**Host protocol.** NIP-HOST methods `background.list`, `background.show`,
`background.run`, `background.pause`, and `background.log`, so the desktop
app and the phone can render the same list later (phase 3). A run's result is
also an activity summary, like a task change.

**Desktop and phone.** Phase 3: a Background section in settings and a
notification for each non-empty run.

## Where it runs

- **The host.** `openagents host serve` runs the rule runner. On a Mac the
  desktop app runs the host. On a computer without the app,
  `openagents service install` already registers the host as a launchd agent
  (macOS) or a systemd user unit (Linux); the background runner comes with
  it. There is no second daemon.
- **One runner per machine.** The runner holds
  `~/.openagents/background/runner.lock`. A second host, or a
  `background run` while the host runs, defers to the holder.
- **What runs, said truthfully (#10349).** The runner rereads the rules on
  every wake (at most 30 s apart), so a rule saved while the host runs needs
  no restart. On every wake it also writes
  `~/.openagents/background/runner.json`: its process, program, what it can
  run (`conversation-rules`, `reload`), and the rules it saw. `background
  apply` and `resume` read it with the lock and say whether the rule runs,
  runs within 90 seconds, waits for a host to start, or waits for an older
  host (one that writes no `runner.json`) to be updated and restarted;
  `background list` adds a line when rules made from words can't run yet.
  `--json` carries this as `runs` and `host`.
- **Without a host.** `openagents background run disk` performs one run in
  the calling process, so a user can schedule it with cron.
- **Low disk resilience.** The runner keeps a small preallocated log file and
  buffers records in memory when a write fails, then flushes after it frees
  space.
- **Windows.** Not supported in phase 1, like the host itself.

## Records

Every run and every action is appended to
`~/.openagents/background/runs.jsonl`:

```text
{ run, rule, rule_version, rule_digest, trigger, started, ended,
  observation: { volume, free_before, free_after, total },
  actions: [ { kind, class, path, bytes, outcome: deleted | trashed | removed | skipped,
               reason, evidence: { task, task_status, lock, processes, age, markers,
                                   judgment: { set, setting, probability } } } ],
  freed_sum, freed_measured, notified, escalated }
```

The log rotates at 10 MB and keeps 10 files. `log --stats` reads it to
answer "how much did cleanup free this week, and from where?" Judgment
records keep the question set, the `Setting` name, and the probability, so
the Gym can calibrate thresholds the way it does for the other Jev settings.

## Other background processes worth having

In priority order. "Rule" means pure code; "Jev" means it needs a bounded
judgment.

1. **Disk cleanup monitor** (rule; Jev for unknown directories in phase 3).
2. **Stale worktree pruning** (rule): remove ended tasks' clean, pushed
   worktrees after 7 days even when the disk is fine. Daily at 03:30. Ships
   on for CoderOS (`/etc/coderos`) and cloud pool hosts
   (`/etc/openagents/pool-host`, written by `scripts/cloud/coder-host-setup.sh`
   and the GCE pool host agent); off on a person's own computer (#10292).
   Disk cleanup stays opt-in everywhere.
3. **Stale issue-claim release** (rule): release a Coder issue claim whose
   task ended or has not run for 6 hours, and comment why.
4. **Keep `~/openagents` on `main` on CoderOS** (rule): fast-forward when the
   checkout is clean; notify when it is dirty or diverged.
5. **Relay and host health watch** (rule): probe the relay and the host;
   after three failures, restart through the service manager and notify.
6. **Flaky-test watch** (Jev): judge whether a new test failure matches a
   known flake's signature or is a new failure, then update or open an issue.
7. **Nightly simulated-user QA run** (rule trigger, Coder run action):
   start the playtest suite at night and post its result.
8. **Daily usage summary** (rule): one short line of what ran, what it cost,
   and what finished. A summary, never a limit.
9. **Log and trace rotation** (rule): compress and age out traces, gate
   logs, and run artifacts by the same safety rules as the disk monitor.

## Phased plan

### Phase 1: the disk monitor, end to end

- **Crates.** A new `crates/background` holds the rule types, the volume
  observation (behind a trait, so tests inject free space), the candidate
  classes, the ownership checks, the planner, the executor, undo, and the
  audit log. It has no host dependency. `crates/coder` shares the task
  store's `ended` test (moved out of `task/targets.rs`) and emits a
  task-ended signal. `crates/coder-host` starts the runner in `serve`, wires
  the `Interval`, `Threshold`, `TaskEnded`, and `HostStart` triggers, and
  adds the NIP-HOST methods. `crates/openagents-cli` adds
  `openagents background` and the terminal's `/background`.
- **Scope.** The `disk` rule only, with the default policy, classes 1–6,
  every safety check, dry run, undo for worktrees, notifications to the
  terminal and the host's activity stream, and the audit log. Rules can be
  edited as JSON; conversation editing is phase 2. No model calls.
- **Size.** Large: about 2,500 lines with tests.
- **Tests.** All under a temporary `HOME` with a scratch task store; none
  reach the real home. Ended and running tasks' target directories; a held
  slot lock and a held `.cargo-lock` keep a directory; a child process with
  an open file inside keeps it; a worktree with an unpushed commit, a dirty
  worktree, and a stash are kept; a clean, pushed worktree is removed and
  `undo` recreates it; symbolic links are not followed and other devices are
  skipped; the deny list wins over every class; a checkout's `target/`
  without `CACHEDIR.TAG` is kept; the dry-run plan equals the executed plan;
  class order and the stop goal; threshold arithmetic for small and large
  volumes; cooldown and emergency; the record's bytes match what was
  deleted.

### Phase 2: rules from conversation

- **Crates.** `crates/coder` router: a `background.define` and a
  `background.edit` route in the next chat-router question set, chosen by
  Jev like every other route. `crates/background`: a compiler that asks Jev
  Choice questions over the typed catalogs (trigger kinds, condition kinds,
  action kinds, candidate classes, the existing rules for an edit) and fills
  bounded fields (sizes, durations, paths, times) by deterministic parsing
  only after the route is chosen. The general engine: every trigger,
  including `Daily` and `FsEvent`, every condition including `Judgment`, and
  multiple rules. Terminal and CLI: a rule card showing the compiled rule and
  its dry run, with confirm and cancel.
- **Rules.** No keyword or string matching selects a route, an action, or a
  class. When the compiler's confidence is below its `Setting`
  (`background.compile`), it asks one clarifying question instead of
  guessing. A compiled rule is shown, never applied silently.
- **Size.** Medium to large.
- **Tests.** A labeled set of requests (define, edit, pause, and off-topic)
  in `crates/gym/questions/` for the route and the compiler, scored like the
  router's evaluation; golden compiled rules for the examples in this
  document; a request below the threshold produces a question, not a rule;
  engine tests for each trigger and condition with injected clocks and file
  events.

### Phase 3: judgment, escalation, more built-ins, more surfaces

- **Unknown-directory judgment.** For the largest directories no class
  covers, Jev answers a Noul ("Is this directory output a program will
  regenerate, or a download cache, holding nothing a person made?") and a
  Choice of kind (build output, package cache, application cache, user data,
  source, unknown) over a code-built state: path, size, age, top entries,
  marker files, and the processes that write it. Setting
  `background.cache_dir`, default 0.9. A yes never deletes on its own: it
  proposes a new class to the user, and once confirmed, the class is a plain
  rule, so later runs need no model.
- **Escalation.** When a rule falls short, it may start a Coder run with a
  briefing that code assembles in the Jev-probe pattern from episode 287
  (sizes, ownership evidence, what was skipped and why). The run proposes
  rule changes; it does not delete. At most one escalation per rule per day.
- **Built-ins.** The other processes listed above, in order, each its own
  small change.
- **Plugins.** A plugin can contribute rules, actions, and candidate classes
  through its manifest, and a user's rule can be published as a plugin.
- **Surfaces.** The desktop Background section, phone notifications, and the
  NIP-HOST methods rendered on both.
- **Crates.** `background`, `coder-host`, `coder`, `jev`, the desktop and
  mobile app crates, and the plugin catalog.
- **Size.** Large, delivered as separate issues per built-in.
- **Tests.** Judgment question sets in the Gym with labeled directories;
  escalation briefing contents; each built-in's rule tests in the phase 1
  style.

## Phase 1 as built

Where the implementation differs from the design above, and why:

- **Where it runs.** `coder host serve` starts the runner only for this
  user's own host (the default root `~/.openagents/host`), so a test's or a
  scratch host never cleans the person's disk. `OPENAGENTS_BACKGROUND=off`
  in the host's environment turns it off. The desktop app's host, the dev
  host LaunchAgent (`scripts/desktop/dev-host.sh`), and a host
  `openagents service install` registers all run it.
- **Task ended.** The runner reads the task store every 30 seconds and runs
  a `TaskEnded` check when a task newly ended (the store's own `Task::ended`
  test, shared with `targets::cleanup`). That poll is the task-ended signal;
  no new event channel was needed.
- **Triggers gate checks.** The runner checks only on the triggers the
  rule names: `HostStart` at host start, `TaskEnded` when a task ends, and
  `Interval { every_secs }` on its own period. Without an `Interval` trigger
  no interval checks are scheduled (there is no default period), so
  `Threshold` alone does nothing: it is checked on the interval, and an
  interval check that runs is recorded as `threshold` when the rule names it
  and `interval` otherwise. A rule with no triggers never runs by itself.
  The rule is re-read after every wake, so an edit to its triggers takes
  effect without restarting the host. A run someone asks for
  (`openagents background run`, `/background`, `background.run`) ignores
  triggers and always runs.
- **Runs in other processes.** `openagents background run` and `/background`
  run in their own process and hold `~/.openagents/background/run.lock` while
  deleting; the host's runner takes the same lock, so two runs never
  overlap. The runner lock (`runner.lock`) only decides which host process
  runs the triggers. A dry run takes no lock.
- **A run someone asks for** plans for every watched volume below its stop
  level, not only those below the start level; triggers plan only below the
  start level. Above the stop level a run has nothing to do.
- **Dry runs are not recorded**: they change nothing. Checks that find
  enough free space are not recorded either. Every real run that acted or
  notified is.
- **Notifications** go to the host's log (`coder host: Freed …`), to the
  rule's state (`~/.openagents/background/state.json`), which
  `background.list` and `openagents background list` show, and to the
  terminal, which shows a new one as a transcript line. Publishing it as a
  Nostr activity summary comes with the desktop and phone surfaces
  (phase 3).
- **NIP-HOST.** `background.list`, `background.show`, `background.log`
  (`observe`), `background.run`, and `background.pause` (`operate`). Their
  answer is the `background` crate's JSON in one `background` outcome, at
  most 256 KB, so `coder-access` does not depend on the rule types.
  `background.run` queues a real run on the host's runner; a dry run is
  asked where it is shown (the CLI, the terminal). No reply is retained.
- **Gate pools.** No gate lock file exists in this repository, so a gate
  build directory qualifies when its Cargo locks are free, no process uses
  it, and nothing in it changed in the last hour.
- **Sizes.** Hard-linked files (Cargo links each output into `deps/`) count
  once. Sizes are cached in `sizes.json` for six hours while the
  directory's last-use time is unchanged.
- **Spare worktrees** (`*.spare-*`, #10115) are never candidates: they are
  the next task's worktree.
- **Ignored files.** One `git status --porcelain --untracked-files=all
  --ignored=matching` lists both unsaved changes and ignored paths (an
  ignored directory once, without walking it). A worktree whose ignored
  paths are not all disposable caches is kept: "holds ignored files: .env,
  private/" (first three, then "and N more"). Keeping it was chosen over
  moving the files to trash, so nothing a person made is ever moved
  (#10166).
- **Stashes.** A worktree is kept when any stash was made on its branch (or,
  detached, on its commit), since stashes belong to the repository.
- **Low-disk log writes** are buffered in memory and flushed with the next
  record; there is no preallocated file.

## Phase 2 as built

Rules from conversation (#10157). Where the implementation differs from the
design above, and why:

- **One route, not two.** The chat router has one `standing.rule` route
  (`chat-router-v5`), described to Jev as "something to keep happening on
  its own on their computer, or a change, pause, resume, or removal of such
  a rule; not something to do once now". Whether a message defines, edits,
  pauses, resumes, or removes a rule is the compiler's first question, over
  the rules this computer actually has, which the worker cannot see. Policy
  rule 2b serves `standing.rule` ("Drafting a background rule for this
  computer. Nothing is saved until you confirm it.") in a terminal and
  `standing.elsewhere` (where rules are made) on the phone, desktop, and
  web. The labeled set `routes-v5.json` adds 42 standing rows and 14 near
  misses (doing it once now, a one-time reminder, email, cron in general,
  how rules work, listing rules); see the measurement in
  `docs/coder/measurements/2026-10-02-standing-rule-route.md`.
- **The compiler** (`background::compile`) asks one Jev request with eight
  Choice questions over typed catalogs: `intent` (define, edit, pause,
  resume, remove, none), `rule` (the rules there are, or new), `what` (keep
  free space, prune worktrees, warn on low disk, tell me when a run fails,
  tell me when a run ends, keep a checkout up to date, unsupported),
  `change` (free level, keep count, a folder to never touch, report only,
  delete again, idle days, interval, time of day, already does it, other),
  `when`, `part_of_day`, `class`, and `pause_for`. Each choice it acts on
  must read at or above `background.compile` (0.6); otherwise it asks one
  question. Only then does code read bounded fields from the words
  (`compile::fields`: sizes, percents, counts, durations, times of day,
  dates, paths); a missing one is a question too ("How much free space
  should it keep, in GB?"). Nothing selects by keywords.
- **Drafts.** A compiled rule is a draft (`background/drafts/<id>.json`,
  one per chat thread), shown as a card: the rule in plain lines (When, If,
  Does, Keeps, Never touches, Status), or for an edit only the lines that
  change (`- ` before, `+ ` after), then its dry run now. `openagents
  background apply ID` (Enter in the terminal, `openagents chat
  run-command` after `chat send`) saves it; a draft is good for a day, and
  an edit applies only to the rule as it was when drafted. A rule made in
  conversation is on once confirmed (only plugin rules start off), records
  `origin: conversation { thread, message }`, and only uses the host's
  built-in actions; deleting stays inside the host's own roots, as for a
  plugin. Removing the built-in rule or a plugin's turns it off instead.
  Changing how a rule works ("keep 200 GB free") turns an off rule on, and
  the card's diff shows it; a pause stays a pause. When the intent reading
  is unsure, it is settled only by readings that agree and are sure on their
  own: a sure action over a sure "no listed rule" is a new rule, a sure
  change of a sure listed rule is an edit.
- **Surfaces.** The chat client runs `openagents background draft --id
  THREAD -- WORDS` on the computer when the worker served `standing.rule`
  (read-only, so it runs at once), then offers `background apply THREAD`
  as a command to confirm. `/background WORDS` in the terminal does the
  same without the worker. `openagents background add --message TEXT
  [--yes]` and `edit ID --message TEXT [--yes]` use the same compiler;
  without `--yes` they keep the draft and print the apply command.
- **New actions.** `Notify { text }` (with `{task}`, `{outcome}`, and
  `{free}` filled by code) and `GitFastForward { repo }`: `git fetch`, then
  `git merge --ff-only` only when the checkout is clean and has no commit
  its upstream lacks; otherwise it changes nothing and says why. A dry run
  fetches nothing. A plugin's rule may notify only with `needs.notify` and
  may never update a checkout. There is still no shell action.
- **The engine** (`background::engine`, the runner). Triggers: `Interval`
  per rule, jittered by up to 10%; `Daily { at }` in local time, where the
  first sight of a rule only sets a baseline and a time missed while the
  computer slept runs once at the next look; `TaskEnded`, evaluated once
  per newly ended task for a rule that reads the outcome; `HostStart`; and
  `FsEvent { paths }`. Conditions: `FreeBelow`, `TaskOutcome`,
  `NoTaskRunning`, `PathExists`, `TimeBetween`, `Weekdays { days }` (the
  local day of the week, 0 Sunday to 6 Saturday; Coder's `/schedule
  weekdays 9am PROMPT` uses it, #11177), and `Judgment` (a Jev Noul
  read at the rule's percent, setting `background.judgment`, 0.8 by
  default, with the host's judge; without one it never holds). Each rule
  has its cooldown. Disk cleanup actions still run through the planner
  with every safety check, the run lock, the log, and undo; the other
  actions are recorded in the same log as `steps`.
- **File triggers poll.** `FsEvent` compares each watched path's existence,
  size, and modification time every 30 seconds (`background/watched.json`)
  instead of using FSEvents or inotify: the runner already wakes on that
  period for ended tasks, it needs no new dependency, and a rule that
  watches a file does not need sub-second reaction.

## Phase 3 as built

Judgment, escalation, the other built-ins, desktop and phone, and plugins
(#10158). Where the implementation differs from the design above, and why:

- **Unknown folders** (`background::judged`). When a cleanup run ends with
  a volume still below its start level, the runner looks at the children
  of `~/.openagents`, `~/work`, and `~/.cache` that no class covers, takes
  the five largest of at least 1 GB, and asks Jev, per folder, the Noul
  and a Choice of kind over a code-built state (path, size, days since it
  changed, its eight largest entries, marker files, processes using it
  now). Code never asks about a Git checkout, anything on the deny list or
  privacy-protected, a link or another volume, or a folder that holds or
  sits inside one a class covers. A folder is proposed only when the Noul
  reads at or above `background.cache_dir` (0.9), the likeliest kind is
  build output, a package cache, or an application cache, and nothing
  uses it now. Proposals wait in `background/proposals.json`;
  `openagents background proposals`, `confirm PATH`, and `decline PATH`
  answer them, and `judge` asks now. A folder judged once is not asked
  about again for 30 days; a declined one never. Each judgment (question
  set, setting, probability, kind) is in the log's `judgments`.
- **Confirmed folders** become `classes.judged` entries of the rule that
  fell short and class 7 (`judged`), with a `DeleteCaches` step for it
  before the trash step. Later runs need no model. Class 7 is never
  deleted outright: the folder moves to `background/trash/<run>/`
  (outcome `trashed`, which frees nothing yet and is not counted as
  freed), `undo RUN` moves it back, and any real cleanup run deletes trash
  older than 24 hours first. A confirmed folder that has become a Git
  checkout is kept.
- **Escalation** (`background::escalate`). A rule with `escalate:
  {workspace?}` that falls short starts one Coder run a day (state
  `last_escalation`) through the host's services. The prompt asks for
  proposed rule changes and forbids deleting, moving, or applying
  anything; the briefing code assembles holds the rule and its digest,
  each volume's free space before and after against its levels, what the
  run removed with its evidence, what was skipped at deletion, what was
  kept and why (40 at most, then a count), the measured folders that are
  not a known cache, the judgments and waiting proposals, and the deny
  list. The record has `escalated: true`. No built-in rule escalates by
  default.
- **Host services** (`background::services::Services`). What only the
  host can do goes through one trait the program that starts the host
  supplies (`coder_host::background::set_services`; `openagents host`
  sets it, `coder host` does not): start a Coder run, list and release
  stale claims, probe and restart, read usage and failed checks, open or
  comment on an issue, and run a plugin. Without it those steps say the
  host cannot do them here; everything else still runs.
- **Built-ins**, all off on a new computer except `checkout` on CoderOS
  (`/etc/coderos` exists); each is turned on with `resume ID`, the
  terminal, or the desktop:

  | Rule | Trigger | Does |
  | --- | --- | --- |
  | `worktrees` | daily 03:30 | Prunes ended tasks' clean, pushed worktrees at least 7 days after their last use (`classes.worktree_days`), whatever the free space, through the planner and every class 3 check. |
  | `claims` | hourly | `ReleaseStaleClaims { idle_hours: 6 }`: an issue flow in this store whose latest claim comment names its task, on an open issue, that neither landed nor opened a pull request, and whose task ended or whose flow has not changed for 6 hours with no live process, is released with a comment saying why (`coder::task::issue_run::stale_claims`). Claims made elsewhere are never touched. |
  | `checkout` | hourly | `GitFastForward { repo: "~/openagents", branch: "main" }`. On another branch, with uncommitted work, or with commits the upstream lacks it changes nothing and says so once (a `blocked` step; repeated identical ones are not said again). |
  | `health` | every 5 min | `HealthWatch` for the relay (a TCP connection to `relay.openagents.com:443`) and the host (a runner holds `runner.lock`); after three failures in a row it restarts the host through the service manager (`coder_service::service::restart`), which reconnects to the relay. |
  | `flakes` | every 30 min | `FlakeWatch`: reads failed checks from the task store's check reports since the last look. A test failing in a second run is shown to Jev with both failures (`background.flake`, 0.8); the same failure opens an issue (`gh issue create`), and later ones comment on it. Memory in `background/flakes.json`. |
  | `qa` | daily 02:00 | `StartCoderRun` with the simulated-user QA prompt (`docs/qa/simulated-users.md`). |
  | `usage` | daily 21:00 | `UsageSummary`: "Today: 12 Coder runs ended (10 finished, 2 failed), $3.40 (1 unpriced); background rules ran 3 times and freed 41 GB." A task's time is its store file's last change. Never a limit. |
  | `rotate` | daily 04:00 | `RotateLogs { compress_days: 7, keep_days: 30 }` over `~/.openagents/traces`, `logs`, `log`, `run-artifacts`, and `gate/logs`: log, trace, and text files only, never through a link or into another volume, never one a process has open; compressed with gzip keeping the modification time, removed after `keep_days`. The task store's own traces are on the deny list and stay. |
  | `calibration` | daily 04:30 | `Recalibrate`: `openagents efficiency refit --write` — refit each delegation threshold from decision readings joined to run outcomes and adopt only fits that beat the default on held-out runs (#10387). On by default on CoderOS and cloud pool hosts; off elsewhere. |

- **Manual runs** of a rule that is not a cleanup (`background run ID`,
  `/background`) now go through the engine with Jev and the host's
  services, so `run usage` or `run qa --dry-run` do what the rule does.
- **Plugins.** A rule may name `RunPlugin { plugin, input }`, which runs
  the plugin's workflow read-only (`openagents plugin run`'s executor); a
  plugin's rule may run only its own plugin. `StartCoderRun` and
  `escalate` need `needs.coder`. The host's own processes (claims,
  health, flakes, usage, rotation) and checkout updates stay the host's.
  A plugin's record may propose folders as caches (`"classes": [{"path",
  "kind"}]`, `~/` paths and disposable kinds only); turning it on adds
  them to the proposals, and the person confirms each like Jev's. A
  package can never bring confirmed folders (`classes.judged`) or class 7.
  `openagents background publish ID [--out DIR]` packages a rule as a
  plugin folder (its record pinning the rule by digest, `needs` computed
  from its actions, confirmed folders left on this computer, paused until
  a dry run) for `openagents plugin publish`.
- **Desktop.** Settings has a Background page listing every rule with
  On or Paused, its last result, and how long ago, and an On switch per
  rule. It reads and writes `~/.openagents/background` directly, as the
  sidebar's watcher line does, rather than through `background.*`: the
  local control socket admits task operations only. The newest notice is
  a desktop notification (title "Background") when it is newer than the
  last one seen and the window is not in front; the first look only
  records.
- **Phone.** Each online computer on Computers shows, under its watchers,
  the newest notice from its `background.list` answer ("Freed 41 GB: 2
  old build folders."). The phone has no system notifications of its own
  yet, so this line is the phone's notice.
- **Gym.** `crates/gym/suites/background-cache-dir-v1.json` (24 labeled
  folders, two families) and its question set
  `background-cache-dir-v1`, generated from
  `crates/background/fixtures/cache-dir-v1.json` by
  `crates/gym/suites/build_background_cache_dir_v1.py`; a test in the
  background crate fails when the states or the questions drift from
  production.

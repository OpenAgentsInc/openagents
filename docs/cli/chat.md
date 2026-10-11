# `openagents chat`: OpenAgents from the terminal

`openagents chat` sends a message to OpenAgents, the chat router the phone
and the desktop talk to, and prints the reply as it streams. People, scripts,
and agents get the same answers, the same knowledge answers, and the same
Coder handoff. When the router judges a message is coding work, Coder runs
on this computer at once and every one of its events streams here
([below](#coder-on-this-computer)). Tracking:
[#10031](https://github.com/OpenAgentsInc/openagents/issues/10031),
[#10032](https://github.com/OpenAgentsInc/openagents/issues/10032).

The command adds no chat logic. Every operation is one of the shared chat
service's commands (`openagents_chat::service::Command`: create, send, read,
stop, list, run Coder) and every answer is its `Snapshot`, the types the phone
and the desktop's host already use. The chat worker's router decides every
route; the command only shows what it said.

The backends, their selection, the move of threads into the host, and the
Coder handoff live in the shared chat client, `openagents_chat::client`
([#10108](https://github.com/OpenAgentsInc/openagents/issues/10108)), which
OpenAgents Terminal ([docs/terminal](../terminal/README.md)) uses too. Each
operation reports typed events (`client::Event`) to a sink, or on a channel
(`Client::stream`); this command prints them as text or as the NDJSON below.
Coder runs on this computer through `coder::task::chat_client::Here`, and the
host is reached through `coder::task::chat_client::Control`.

The unit is the **thread** ([glossary](../glossary.md)): one conversation
with OpenAgents, one row of the chat list. Every run prints the thread ID it
used, so a script can continue it.

## Commands

```text
openagents chat MESSAGE                        same as `chat send MESSAGE`
openagents chat send MESSAGE [--thread ID] [--no-run] [--timeout SECONDS]
openagents chat follow --thread ID
openagents chat stop --thread ID
openagents chat answer --thread ID TEXT
openagents chat threads [--all] [--limit N]
openagents chat read --thread ID
openagents chat export --thread ID
openagents chat run-coder --thread ID
openagents chat run-command --thread ID
openagents chat apply --thread ID
openagents chat work --issues NUMBERS|LABEL [--parallel N] [--land queue|main|pr|none] [--on boat|gce] [--engine briefed|bare]
```

Every command also takes `--scratch`, `--local`, and `--socket PATH`, and
`--json` before or after the group.

- `send` (or a bare message) starts a new thread, or continues `--thread ID`
  with its context. `MESSAGE` may be several words; `-` reads it from stdin
  (`echo "What is the Gym?" | openagents chat -`). A message that is itself
  a command word (`threads`, `read`) needs `send`.
- `threads` lists threads newest first; `--all` includes archived ones.
- `read` prints a thread's turns, every page of them, then how each Coder
  turn ended and its answer (`--json`: `coder_turns`).
- `export` prints the thread as its `ATIF-v1.8` trajectory
  ([below](#threads-as-atif)). The command checks the document with
  `crates/atif` before it prints it.
- `follow` replays the thread's Coder task from its first event and keeps
  streaming until it ends or asks. Ctrl-C stops following, not the task.
- `stop` stops the thread's running Coder task.
- `answer` answers the question (or approval) Coder asked, and follows the
  turn the answer starts.
- `run-coder` runs Coder for the thread's last offer, as `send` does
  without `--no-run` (useful after `--no-run`).
- `run-command` runs the `openagents` command the thread's last reply
  proposed (#10170). `send` already runs a command this build's command
  tree declares read-only, such as `wallet info` for "check my wallet
  balance", and prints its output as the answer; a command that changes
  something on this computer waits for `run-command` (in OpenAgents
  Terminal, Enter); a command that moves money or shows a secret never runs
  from the chat.
- `apply` brings the thread's Coder change into the checkout it was made
  from, uncommitted, for you to review and commit there: the worktree's
  whole change from the task's base, new files included, applied with a
  three-way merge. It refuses a checkout with changes of its own and
  never commits or pushes (#10343).
- `work` hands several GitHub issues to Coder, one issue flow each
  ([below](#working-a-github-issue)).

Thread IDs are 32 lowercase hex characters, the IDs the phone and the
desktop use.

## Where threads live

1. **This computer's host.** When the host runs (the OpenAgents app, or
   `openagents host serve --control`), the command speaks the host's
   same-user control socket, the one the desktop app uses. Threads are the
   desktop's threads: one sent from the terminal shows in the app's chat
   list, and one started in the app is readable here. The host holds the
   device key and the encrypted store (`<host root>/basic-chats`).
   `--socket PATH` names another host's socket and fails if nothing answers.
   A phone paired with this computer lists these threads beside its own,
   labelled with the computer, and can continue one: its follow-up and the
   reply show here and in the app (NIP-HOST `thread.*`,
   [#10035](https://github.com/OpenAgentsInc/openagents/issues/10035)).
2. **In process.** Without a host, or with `--local`, the chat service runs
   inside the command with its own device key, `~/.openagents/chat/device.key`
   (created on first use, mode `0600`, never printed), and its own encrypted
   store, `~/.openagents/chat/threads/`. `OPENAGENTS_CHAT_HOME` moves both.
3. **Scratch.** `--scratch` uses a throwaway key and store for one thread,
   in `$TMPDIR/openagents-chat-scratch/<thread>/`, and never touches the host
   or real history. Continue, read, or export that thread with `--scratch
   --thread ID`. It is for tests and smokes.

### Threads kept without a host join the host

When a host runs on this computer, threads kept in process join the host's
store, so the desktop app (and a paired phone) shows them. It happens once,
at two moments: when the host starts (only this user's own host, whose root
is `~/.openagents/host`, or one started with `OPENAGENTS_CHAT_HOME` set),
and when `openagents chat` finds a running host (control operation
`chat_migrate`, on a connection of its own, so an older host that doesn't
know it changes nothing). The host reads `~/.openagents/chat/threads` with
the chat home's device key and re-encrypts each thread under its own key in
`<host root>/basic-chats`. Every thread keeps its ID, title, times, turns
with the router's metadata and send IDs, lane, archived and pinned state,
and Coder link, so `openagents chat read` and `export` print the same thread
and the same ATIF trajectory as before.

The move is crash-safe: each thread is a durable record in the host's store
before anything else changes, a thread the host already holds is left as it
is, then the chat home gets a marker (`threads-migrated.json`: the host, the
time, and every thread moved), and last the old store is renamed
`threads-moved-<time>`, so it is never read again. Nothing is deleted: the
renamed store keeps its encrypted files under the same device key. A move
that stops halfway leaves `threads` where it was, and the next one skips
the threads already moved. Moving twice is a no-op. After a move, `--local`
starts an empty store; its new threads join the host the next time. A home
another user owns is refused, and a scratch store (`--scratch`) is never
moved.

`openagents doctor` shows which one a run would use, the host socket, the
chat home, and the command's public identity (never the key).

The in-process and scratch modes talk to the public chat worker on
`wss://relay.openagents.com` as a NIP-CJ client signed by their own key, so
they are answered like a fresh phone, with no usage limit, and recorded in
the worker's usage log.
`OPENAGENTS_CHAT_RELAY` and `OPENAGENTS_CHAT_WORKER` point them at another
relay and worker (the fixture tests use a local relay). The request says
`surface: "terminal"` and `client: "openagents-cli"`, and so it does through
the host: the control socket's `chat` operation carries the caller, and the
host puts it on the turn
([#10108](https://github.com/OpenAgentsInc/openagents/issues/10108)). A host
older than that refuses the field, and the command asks again without it, so
the turn says `desktop`. OpenAgents Terminal sends the same surface with
`client: "openagents-terminal"`. No model key is needed.

Each turn also tells the worker that this computer is where Coder runs
(`context.computer`, with its coding agents' readiness, under `--no-run`
too, so the printed offer is the one a run would take) and names the project folder: the Git checkout the command
runs in, by name and path (`context.project`). Through the host, the project
is the one the thread's Coder task used, else the host's first project. So
"what's your working dir" is answered with that folder, and the chat never
asks you to connect a computer
([#10077](https://github.com/OpenAgentsInc/openagents/issues/10077)). The
folder's name and path go only to our chat worker and its chat model.

## Coder on this computer

The router may judge that a message is work for a computer: a Coder offer
(`run_coder`) or the computer lane on the `work.dispatch` route; a reply
that answered the message on another route starts nothing
([#10079](https://github.com/OpenAgentsInc/openagents/issues/10079)). The command then runs Coder right here,
with no host to attach, nothing to pair, no project to register, and no
separate accept step. `--no-run` keeps the old behavior: the offer is
printed, with `openagents chat run-coder --thread ID` to accept it later,
and, when the person named a coding engine ("do a test delegation to
claude"), the engine it asks for (`engine` on the offer and route events;
`asked for: Claude Code` on stderr). The run puts that engine first and
falls back only when it is not signed in, at its limit, or not allowed by
the settings, and its start card says why
([#10076](https://github.com/OpenAgentsInc/openagents/issues/10076)).
Nothing matches keywords; only the router's judgment decides.

Everything below is the default. The local capability settings
([settings.md](settings.md), `openagents settings`) choose the providers and
their order, whether a coding reply runs at once or only offers
(`coder.start: ask_first`), the usage threshold, which folders count as
projects, and what the run's commands may reach.

- **Project.** The Git checkout the command runs in. Coder works in its own
  detached worktree of the checkout's `HEAD`, never in the checkout, so the
  result is in the worktree the `result` event names. A start takes the
  project's spare worktree when one is ready (made in the background when
  `openagents chat` or OpenAgents Terminal opens in the project, and after
  each start), moved to that exact commit, so it does not wait seconds on
  Git in a large repository; a spare with any change is removed, never
  used. Each turn's record (`<store>/local/<task>.json`) keeps how long
  each stage of its start took (`timings`). Uncommitted changes
  in the checkout are not carried over. Outside a checkout, or in one with
  no commit, nothing runs and the command says so and exits 1. (With a host
  running and no checkout here, the host's own handoff still runs Coder in
  the host's project, as before.)
- **Providers.** Codex, then Claude Code, then Grok Build (#10091), each
  only when it is signed in on
  this computer (the same local check the host uses, reading no credential
  and asking no network). A provider with a usage or rate-limit refusal that
  still holds in the task store's capacity book is passed over, and so is
  one a fresh usage reading in the store shows near its limit. The
  `coder_started` JSON keeps the precise passed-over reasons. The display
  names the running engine without naming provider usage windows. Without
  `--json` a start prints two short lines, "Starting Grok Build…" at once
  and "Grok Build is working." when the run starts, and the reason only
  when another engine runs than the one asked for or one was passed over;
  the task ID and worktree stay in `--json` (#10115). During the
  run, a provider that refuses is recorded and the run switches
  (`provider_switched`).
- **The same run as the host's.** The task is started through the host
  auto-start's own start (`coder::task::autostart::Policy::launch`): the
  `microcoder repository` engine beside `openagents`, else the one the Mac
  app bundles in `Contents/MacOS` when `openagents` is the app's
  `Contents/Helpers/openagents`, else `~/.openagents/bin`'s
  (`OPENAGENTS_CODER_CONTROLLER` names another). An older engine than the
  CLI can refuse a newer grant with "the execution grant has an invalid
  shape" (#10074); build or install `microcoder` with `openagents`. The
  run is under an execution grant, reaching what `coder.access` allows
  (`full` by default: no sandbox, and Coder runs every step without asking, #10104), with the
  same failover and the same ATIF trajectory per turn. The shared code is
  [`coder::task::local`](../../crates/coder/src/task/local.rs).
- **No step or time budget.** A run ends only when Coder finishes (or asks
  a question), when the person stops it, or when the loop's **stuck guard**
  ends it: Jev, in the judgment it makes before every step, judged eight
  steps in a row (`microcoder_loop::run::STUCK_STEPS`; a smaller window is
  raised to four) repeating an approach that already failed (`repeating`
  at least 0.5) without moving the work forward (`progress` under 0.5).
  Without Jev's answers the rule decides: every command of the last step
  had already run with the same exit and output. The run then fails with
  "Coder stopped before finishing: stuck (it repeated an approach that
  already failed, without progress, for 8 steps in a row)." The loop's
  other guards stay: replies it cannot use and replies that run nothing,
  three in a row each, and, for a whole coding agent (Devin, OpenCode,
  Grok Build), twenty minutes with no output. Settings and policies written
  before #10103 may carry `max_steps` or `wall_seconds` (the host's
  `autostart.json`, an execution grant, `.openagents/coder-issues.json`);
  they still read without error and are ignored, and nothing writes them
  now. `coder host autostart on` still accepts `--max-steps` and
  `--wall-seconds` so older scripts work; they do nothing.
- **Where tasks live.** `~/.openagents/tasks`, the store `coder task` and a
  host on this computer use by default, so the host's devices see these
  tasks in their history. `OPENAGENTS_TASKS` names another store; worktrees
  go in `worktrees/` beside it. A `--scratch` thread keeps its tasks in its
  own scratch directory.
- **The thread records it.** The task is bound to the thread
  (`service::Command::BindCoder`, host `local`), so `follow`, `stop`,
  `answer`, and `export` find it, and the phone and the desktop can show it.
  When this computer's host runs, the thread lives there, and the run is the
  same.
- **A follow-up after the run.** Once the task's turn has ended (a result,
  a failure, or a stop), `openagents chat --thread ID "..."` sends the
  message to the router with what the run did (`context.coder_run`: how it
  ended, its engine, its summary, the files it changed, and its commands),
  so a question such as "summarize what happened" is answered in chat. A
  reply that hands the message to Coder continues the same task with it as
  its next turn, in the same worktree, and follows that turn (#10094).
  The next turn carries the earlier turns' conversation into the engine,
  so it continues the same session. While the run still works, a message
  the router hands to Coder goes to that run, read at its next step (or
  the turn it starts, for an agent that reads only at a turn's start), as
  the run view's composer sends it, and the client follows it
  ([#10171](https://github.com/OpenAgentsInc/openagents/issues/10171)).
  Whether a follow-up is more work or a question is the router's Jev
  reading, never the message's words.

Text mode shows a compact live view on stderr (the provider and why, each
step's thinking, commands, their exit and the last lines of output, progress,
provider switches) and the final result on stdout: Coder's reply, the files
changed with lines added and removed, and the worktree. A question is printed
on stdout with the `answer` command on stderr. Ctrl-C stops following and
prints how to follow or stop the task; the task keeps running.

Exit codes after a run: `0` when the turn finished or asked, `1` when it
failed, was stopped, or could not start (not a checkout, no provider signed
in, no capacity).

### Several runs at once

A message that asks for the same work on several coding engines ("do 3
readonly delegations, 1 per agent", "ask all three agents", "have codex
and claude both look") gets a dispatch plan from the router's typed
readings (`fanout`, `read_only`, `summarize`; #10183), only from a
terminal: the reply says what starts, verb first ("Exploring the repo with
Codex, Claude Code, and Grok Build."), and the client starts one run per
engine in parallel ("Running Codex, Claude Code, and Grok Build,
read-only."), each in its own worktree, pinned to its engine with no fallback.
A read-only plan's runs start with a grant that writes nothing in the
worktree and seals Git, and a full-access setting runs them under this
computer's toolchains instead, so the boundary holds whatever the engine
does. The `coder` event's `task` is then an array, one `{task, engine,
worktree, read_only}` per run; every run's events stream as usual (the
terminal shows one rail row per run). When all end, each run's result is
added to the thread, and, when the message asked for a summary, the chat
model writes one combined summary of them (the request carries
`context.runs`; no further run starts). The first run is the thread's
bound task; each run's own record names the thread.

### Working a GitHub issue

A message that asks Coder to work a GitHub issue of this checkout's
repository, such as `openagents chat "work on #10051"` or "take
OpenAgentsInc/openagents#10051", runs the **issue flow** instead of an
ordinary run ([#10049](https://github.com/OpenAgentsInc/openagents/issues/10049)).
The router judges the message is coding work, as for any run; Jev then
chooses the issue among the references the message and the thread name, or
answers none. The reference is a bounded field read after that judgment;
no keyword decides. The shared code is
[`coder::task::issue_run`](../../crates/coder/src/task/issue_run.rs), and
the desktop's chat runs the same flow.

1. **Claim.** It reads the issue, its comments, and up to three issues it
   links, and claims it ([below](#claims)): a claim comment (`Claimed:
   Coder is working on this…`), the signed-in GitHub user as assignee, and
   "In progress" on each project the issue is on.
2. **Work.** It fetches the default branch and starts a local run, as
   above, in Coder's own worktree of `origin/main` (not the checkout's
   `HEAD`), with the issue as the prompt. A turn has no step or time limit
   ([above](#coder-on-this-computer)).
3. **Check.** It runs the repository's checks for what the change touched:
   each touched Rust package's tests inside a write boundary with
   credentials withheld, the issue flow's diff checks (style, figures with
   no source, broken links, code that depends on what changed, plain
   wording), and, when the policy asks, `cargo fmt --check` and Clippy with
   warnings denied. When they find problems, a fix turn continues the same
   task with the problems and the diff, up to `fix_rounds` times. A flow
   runs at most `1 + fix_rounds` turns. (#10063's continuation turns,
   which continued a turn that ran out of its step or time limit while
   progressing, are gone with the limits,
   [#10103](https://github.com/OpenAgentsInc/openagents/issues/10103).)
4. **Land.** It commits (the issue's title, Coder's summary, and the issue
   link) and lands as the repository's policy says. `main`: fetch, rebase
   onto the newer `main` when it moved, run the checks again only when the
   newly landed commits can affect the change (a workspace build file, or a
   package the change's packages depend on or that depends on them; docs and
   unrelated packages do not), then push plainly, never forced. Git refuses
   a push when `main` moved, so a refused push waits a random, growing delay
   (2 s doubling to 60 s) and retries on the newer `main`, up to 12 tries,
   giving up sooner when two pushes are refused while `main` did not move.
   This makes landing from several machines at once safe (#10226); flows on
   one machine still land one at a time. Each try is listed in the issue
   comment. `pull_request`: push a `coder/issue-N-…` branch and open a pull
   request that closes the issue.
5. **Close.** It comments the commit, the files, the checks that ran, and
   the run (task, turns, provider and model) on the issue, closes it, and
   moves it to "Done" on each project it is on.

It never pushes a red change. When the checks still fail after the fix
turns, a rebase conflicts, the run fails (such as the stuck guard ending
it), Coder asks a question instead of finishing, nothing
changed, or the person stops it, the flow comments what it tried and the
failing output, releases its claim, and leaves the issue open with the
change in Coder's worktree. The comment says how far the run got (`git diff
--stat` of the worktree against the branch it started on) and the run's
turns and fix turns, so a person can pick up there.

The repository's policy is `.openagents/coder-issues.json` at its top
level; without one a flow opens a pull request and runs only the tests and
diff checks. This repository's:

```json
{ "land": "main", "claim_hours": 6, "fix_rounds": 3, "fmt": true, "clippy": true }
```

An older policy's `max_steps` and `continue_turns` still read and are
ignored: turns have no step or time limit, so there is nothing to continue
from.

`branch` names another branch than `origin/HEAD`'s, and `trailer` adds a
line to each commit message.

**The stream.** Every step is in the task's events: the flow's notes
(`step` events of kind `note`: the issue, the claim, the worktree, the
checks, fix turns, the rebase, the push, and the ending) come between the
turns' events, and the last turn's ending waits for the flow, so it carries
the outcome. A landed flow ends in a `result` whose `summary` ends with what
the flow did and whose `issue` names the issue: `repository`, `number`,
`url`, `title`, `outcome` (`landed`, `pull_request`, `unchanged`, `failed`,
`stopped`), `commits`, `pull_request`, and `closed`. A flow that did not
land ends in a `failure` (or `stopped`) with the same `issue`. The flow
keeps its notes beside the task (`<store>/local/<task>.issue.json`), so
`follow`, the desktop, and the phone show the same run.

**Stopping.** Ctrl-C while the flow runs (or `chat stop`, or the apps' stop)
stops the running turn, or, between turns, stops the flow before its next
step; it says so on the issue. The flow runs in a process of its own
(`microcoder issue-flow`), like a run's engine, so closing the terminal,
the shell, or the screen that started it does not end it; a chat or the
thread list follows it again. With an engine older than this program it
runs in the process that started it, as before.

**A queue.** `openagents chat work --issues 10052,10053` (or `--issues
LABEL`, a label's open issues) works several issues, one at a time or
`--parallel N` (up to 4) at once, each in its own worktree and its own
thread titled with the issue. It skips a closed issue and a claimed one
([below](#claims)). A label's issues come in the repository's project order
when it has one (Ready or Todo, not blocked), else oldest first. Each flow's
events stream with an `issue` field (text mode prefixes `#N`); each issue
ends with an `issue` line (`outcome`: `landed`, `pull_request`, `failed`,
`stopped`, `unchanged`, `skipped`, `closed`, or `not_started`, and
`message`, `thread`, `task`, `commits`), and the queue with `queue_done`.
It exits 0 when every issue landed or was skipped. `--land main|pr`
overrides the policy.

`--on boat` runs each issue on a Boat sandbox of its own instead of this
computer (`--parallel` up to 16); `--template NAME` and `--engine-logins
api-keys|boat` go with it; under `--json` the extra events are
`boat_sandbox`, `boat_seed` and `route_record`, and the final `issue` event
adds `sandbox`, `wall_seconds`, `machine_seconds` and `cost_usd`; see
[docs/cloud/boat-chat-work.md](../cloud/boat-chat-work.md).

### Claims

One claim record, which every path writes and reads
([#10203](https://github.com/OpenAgentsInc/openagents/issues/10203);
[`coder::claim`](../../crates/coder/src/claim.rs)): the chat issue flow
and its queues, `coder-project`'s supervisor, and other agents through
`openagents issue claim|release N`.

- **Claim**: a comment carrying `<!-- openagents-coder-claim … -->`, the
  signed-in GitHub user as assignee, and, on each open GitHub Project the
  issue is on, its Status set to "In progress".
- **Release**: a comment carrying `<!-- openagents-coder-release -->`, that
  assignee removed, and Status back to "Ready" (or "Todo", whichever the
  project has). **Landed**: Status "Done"; the assignee stays.
- **Claimed** means a claim comment (one with the marker, or starting
  "Claimed") from the last `claim_hours` hours that no later release
  answered, or Status "In progress" set within `claim_hours` and after the
  latest release. A queue and `coder-project` leave a claimed issue alone;
  a person naming one issue is told and Coder works it anyway.
- **Session**: a claim is held for the agent session that took it
  ([#10764](https://github.com/OpenAgentsInc/openagents/issues/10764)): a
  record under the lease root
  ([Issue claims](../coder/runtime/leases.md#issue-claims)) and a
  `session=` field in the marker. `claim` refuses while another live
  session holds the issue or a claim comment from another session is
  younger than `claim_hours`, and says which session and how long ago. The
  same session claims again; a session that ended is taken over. `release`
  drops the hold and is refused for another live session's claim. `--force`
  overrides either refusal. Markers without a session still count as
  claims.
- **Without Projects** the claim is the comment and the assignee; nothing
  else changes. A step that fails (no project access, say) is said in the
  flow's notes and the rest still happen.

Field and value names match without case and are set under `project` in
`.openagents/coder-issues.json` (defaults shown):

```json
{ "project": { "field": "Status", "in_progress": "In progress",
               "ready": ["Ready", "Todo"], "done": "Done", "number": null } }
```

**Pickup** (`openagents issue pickup`, a queue's label): with a project —
`number`, or the one open project linked to the repository — its item order,
a `ready` Status, and no open `blockedBy`; without one, the `coder-sized`
label (or the label given), oldest first.

```text
openagents issue claim 10203 [--note TEXT] [--force]   # hold + comment + assignee + In progress
openagents issue release 10203 [--force]               # drop hold, release comment, unassign, Ready
openagents issue status 10203                # claimed? and each project's Status
openagents issue pickup [--label L]          # what to pick up next, in order
```

`coder-project` claims an admitted task's issue when its attempt starts
(marker `project=ATTEMPT`), skips an issue another claim holds, releases
it when the attempt ran nothing (no capacity) or failed, and keeps it while
a finished attempt waits for review.

**Picking up an issue from a chat.** A message that asks Coder to choose an
open issue itself, such as "pick one of the open issues nobody is working on
and take it", names no issue. After the router judges it coding work, Jev's
issue question answers `pick` ([#10206](https://github.com/OpenAgentsInc/openagents/issues/10206);
labeled asks in `crates/coder/fixtures/issue-pick/asks-v1.json`, scored by
`cargo test -p coder --test issue_pick_eval -- --ignored`). Coder then reads
the open issues and pull requests
([`coder::task::issue_pick`](../../crates/coder/src/task/issue_pick.rs)) and
passes over any issue with an assignee, a claim (above), an open pull
request that closes it, names it, or is on a branch named for it, or a
holding label (`blocked`, `umbrella`, `epic`, `needs-owner`, `question`,
`wontfix`, `duplicate`). It takes the first free issue in the pickup order
above; when none of those is free, the open issue whose body names no other
open issue, then the shortest, then the oldest. It runs the issue flow on
it and says "Picking up #N: title."; when no issue is free it says why and
starts nothing. An engine run never claims or delegates on its own.

### Coder events

Under `--json`, every event of the task follows the chat's own events as one
NDJSON line. The stream is `openagents_chat::coder_events`
([source](../../crates/openagents-chat/src/coder_events.rs)): the desktop
([#10033](https://github.com/OpenAgentsInc/openagents/issues/10033),
[how it draws each event](../desktop/local-coder.md#coder-on-this-computer-from-a-chat)) and the
phone ([#10035](https://github.com/OpenAgentsInc/openagents/issues/10035))
consume the same types, mapped from the same trajectories by the same
`Mapper`.

Every line carries `seq` (from 1, without gaps; a replay produces the same
numbers), `task`, `thread`, `event`, and the event's fields. Every event but
the envelope has `turn` (from 1; an answer starts the next turn).

| Event | Fields |
| --- | --- |
| `coder_started` | `project`, `checkout`, `worktree`, `base` (the commit the worktree started from), `provider` (`codex`, `claude`), `model`, `reason` (why this provider, a sentence), `fallbacks` (`provider:model`, in order), `via` (`local`) |
| `step` | `step_id` (the ATIF step in the turn's trajectory), `kind`, `source` (`user`, `agent`, `system`), `text` (at most 2 KiB). `kind` is `message` (the person's request), `thinking`, `command`, `tool_call` (an agent's tool, for Devin, OpenCode, and Grok Build routes), `observation` (a command's exit and time), `reply` (Coder's reply as it is written, in pieces), or `note` (such as running without Jev). A `command` or `tool_call` step also has `call`: `verb` (`read`, `search`, `list`, `fetch`, `edit`, `delete`, `move`, `run`, `other`), `target` (the path, pattern, URL, or command; for `other`, the agent's words), `about` (a command's description, when the agent gave one), and `failed` (when the agent said it failed). Every surface groups tool calls from `call` ([#10117](https://github.com/OpenAgentsInc/openagents/issues/10117)); the text view prints `◈ Read 3 files, Searched 2 patterns` once a group ends and `◆ Run cargo test · exit 101` once a call has its result |
| `output` | `step_id`, `command`, `exit` (null when a signal or the deadline ended it), `timed_out`, `seconds`, `text` (at most 4 KiB), `truncated` |
| `provider_switched` | `step_id`, `from`, `to` (null when no admitted route had capacity), `reason`, `resets_at` (Unix seconds) |
| `progress` | `step`, `seconds`, `done` (Jev's probability that the task is done, or null), `complete` (Jev's estimate, 0 to 1, of how much of the task is complete; left out until Jev gave one). No step budget: an older line's `max_steps` is ignored. The desktop and the phone show it as "Coder is working · step 5 · ≈40% done · 9s", never "of N" |
| `question` | `text`, `answer` (the command that answers it) |
| `approval` | `text`, `answer` |
| `result` | `summary` (Coder's reply), `files_changed` (`path`, `status`, `added`, `removed`), `insertions`, `deletions`, `worktree`, `trajectory` (the turn's ATIF file), and, for the issue flow, `issue` ([above](#working-a-github-issue)) |
| `failure` | `message`, `ending` (such as `no_capacity`, `loop_incomplete`, `not_started`, `issue_failed`), `resets_at`, and, for the issue flow, `issue` |
| `stopped` | `message` |

A turn ends with exactly one of `result`, `question`, `approval`,
`failure`, or `stopped`. `follow` replays a finished task identically: what a
turn changed is computed once, when it ends, and kept in the run's record
(`<store>/local/<task>.json`).

```sh
$ cd ~/code/slugs && openagents --json chat "add a unit test for slugify that covers an empty string"
{"event":"accepted","thread":"1742…","backend":"in_process",…}
{"event":"result","thread":"1742…","text":"Working on adding a unit test for slugify that covers an empty string.",…}
{"event":"starting","thread":"1742…","engine":"claude","text":"Starting Claude Code…"}
{"event":"coder","thread":"1742…","accepted":true,"message":"Coder started.","task":{"host":"local","task":"4d0d…","project":"slugs","worktree":"…/worktrees/slugs-4d0d730cb9be"}}
{"seq":1,"task":"4d0d…","thread":"1742…","event":"coder_started","turn":1,"project":"slugs",…,"provider":"claude","model":"claude-opus-5-5","reason":"Codex reached its usage limit until 2026-09-30 17:39 UTC; using Claude Code.","fallbacks":["codex:gpt-6-luna"],"via":"local"}
{"seq":2,…,"event":"step","turn":1,"step_id":1,"kind":"message","source":"user","text":"add a unit test for slugify that covers an empty string…"}
{"seq":3,…,"event":"progress","turn":1,"step":1,"seconds":6.65,"done":0.04,"complete":0.0}
{"seq":5,…,"event":"step","turn":1,"step_id":12,"kind":"command","source":"agent","text":"git status --short | head; ls -la; …"}
{"seq":9,…,"event":"output","turn":1,"step_id":17,"command":"git status --short | head; …","exit":0,"timed_out":false,"seconds":0.1,"text":"…","truncated":false}
{"seq":36,…,"event":"result","turn":1,"summary":"I added `test_slugs.py` …","files_changed":[{"path":"test_slugs.py","status":"added","added":13,"removed":0}],"insertions":13,"deletions":0,"worktree":"…/worktrees/slugs-4d0d730cb9be","trajectory":"…/tasks/4d0d….1.atif.jsonl"}
```

## Stopping

`--timeout SECONDS` (default 120, the worker's own limit) and Ctrl-C stop
receiving the reply, with the apps' words: "Stopped receiving this reply.
The hosted worker may still finish." What streamed is kept as a stopped
reply. Stopping does not cancel the remote work. The run exits 1.

## Output

Without `--json`, the reply streams to stdout and everything else goes to
stderr: the served knowledge answer, offers, suggestions, and `thread ID`.
`reply=$(openagents chat "…")` captures only the reply.

With `--json`, `send` prints NDJSON, one event per line, in this order:

| Event | Fields |
| --- | --- |
| `accepted` | `thread`, `request` (the send ID), `new`, `backend` (`host`, `in_process`, `scratch`), `at` (socket or store) |
| `partial` | `thread`, `text` (the reply so far), `delta` (what was added, or null when the preview was rewritten) |
| `offline` | `thread`, `retry_in` (seconds): the relay, or this computer's host, has not been reached for 3 s (a shorter blip, such as the host restarting, is not reported); the reply is asked for again then, with a pause that doubles up to 30 s, until it goes through or Ctrl-C stops it |
| `online` | `thread`: reached again after an `offline`; the reply streams on |
| `route` | `thread`, `tier`, `route`, `bank`, `served_answer` (a knowledge entry `id@version`, when the reply is one), `judgment` (the router's typed judgment as it arrived), `computer` (the judgment placed it on a computer), `family` (the shared route policy's route family for the reply: `answer`, `local_command`, `plugin`, `coder`, `standing_rule`, `missing_capability`, `clarification`, or `refusal`; see [Route records](#route-records)), `followups`, `cards` |
| `offer` | `thread`, `offer` (the typed offer, such as `{"offer": "run_coder"}`), `accept` (the command that accepts it, when there is one) |
| `result` | `thread`, `text`, `model` (the model the worker named), `served_answer` |
| `command` | `thread`, `argv` (without `openagents`), `confirm` (false: it runs now; true: it waits for `run-command`) |
| `ran` | `thread`, `argv`, `ok`, `output` (what it printed, at most 16 KiB) |
| `starting` | `thread`, `engine` (the provider's word), `text` ("Starting Grok Build…"): a start on this computer began; its worktree and launch follow (#10115) |
| `coder` | `thread`, `accepted`, `message`, `task` (`{host, task, project, worktree}` when Coder started) |
| `stop` | `thread`, `task`, `requested`, `message` (from `chat stop`) |
| `failure` | `thread`, `message`, `stopped`, `partial` |

The Coder events ([above](#coder-events)) follow `result` when Coder runs.
`threads`, `read`, and `export` print one JSON document each. Exit codes are
the CLI's: `0` success, `1` refused, failed, stopped, or a coding request
whose Coder run did not finish, `64` invalid usage.

```sh
$ openagents --json chat --scratch "Write a haiku about rain"
{"event":"accepted","thread":"ef9f…","request":"3a2d…","new":true,"backend":"scratch","at":"…/openagents-chat-scratch/ef9f…"}
{"event":"partial","thread":"ef9f…","text":"Here's a draft.\n\n","delta":"Here's a draft.\n\n"}
{"event":"partial","thread":"ef9f…","text":"Here's a draft.\n\nSoft drops kiss the earth,  \nPuddles","delta":"Soft drops kiss the earth,  \nPuddles"}
{"event":"route","thread":"ef9f…","tier":"opener","route":"general","bank":"chat-answers-v1@a04859020455","served_answer":null,"judgment":{…},"computer":false,"followups":[],"cards":[]}
{"event":"result","thread":"ef9f…","text":"Here's a draft.\n\nSoft drops kiss the earth, …","model":"google/gemini-3.8-flash","served_answer":null}
```

`openagents mcp serve` exposes the group as the `chat` tool: its `args` are
the words after `openagents chat`, and its result carries the events as
`{"events": [...]}`.

## Route records

Every message `openagents chat` and OpenAgents Terminal send goes through
one route policy, `openagents_chat::route` (`route-policy-v1`; router plan
phase 1, #10207). It reads the worker's typed judgment and offers into the
router contract's route result (`crates/route-contract`), builds the
immutable admission snapshot, and keeps a route record
(`openagents.route.record.v1`) in the thread's journal on this computer,
`~/.openagents/routes/<thread>.jsonl` beside the task store (a scratch
thread's beside its own). Each line is one write; the latest line of a
request is its record. A record holds:

- the route result and the admission snapshot, named by its digest;
- the router's own moves (`received`, then `proposed` for an offer or a
  command waiting for Enter, `admitted` for work that starts now, or
  `completed`/`failed` for an answer or refusal);
- each Coder task the route started (one, or a dispatch plan's N), with its
  lifecycle projected from the task owner's `(status, execution, checks)`,
  the run's cost in micro-dollars when known, its wall time, its
  retained artifact and trace digests, and who paid for its model calls
  (`payer`: `ours` or `theirs`, and `payer_keys`, each key's provider and
  fingerprint, from the run's result record);
- the route's own wall time, received to settled;
- under BYOK `mine`, `payer_keys`: the person's keys the message went with,
  by provider and fingerprint, never a key. The snapshot's `money` then
  says `byok: mine` and names their providers as the payers of routing,
  decisions, and the chat model (#10176).

Cost is recorded, never shown. A request whose record already names its
tasks is followed by `run-coder`, never started again, and a confirmed
command runs once per message. The snapshot names the checkout's path, so
the journal stays on this computer.

## Threads as ATIF

`openagents chat export --thread ID` renders the thread through
`openagents_chat::thread::trajectory`, shared code the phone and the desktop
can call too:

- `schema_version` is `ATIF-v1.8`; `session_id` and `trajectory_id` are the
  thread ID; `agent.name` is `openagents-chat`.
- One step per turn: the person's messages are `user` steps, OpenAgents'
  replies `agent` steps with the model the worker named, stamped with when
  this device saved them. Turns saved before times were kept take the time
  of the turn before them.
- The router's judgment of a reply is a decision call on its step
  (`chat_router`, `openagents.decision-call.v1`, the judgment as its
  answers and the route as its `route`).
- What the router said beside the text is the step's
  `extra["openagents.chat.router"]`; a knowledge answer names its entry as
  `extra.served_answer`.
- A Coder task the thread started is linked from the reply before it: an
  observation result whose `subagent_trajectory_ref` names the task and its
  first attempt's trajectory file (`<task>.1.atif.jsonl` on that host).
- When this computer's task store holds that task, each of its turns'
  trajectories travels inside the export, in `subagent_trajectories`, and
  the reference names the first by its `trajectory_id`
  (`openagents_chat::thread::trajectory_with`). The desktop and the phone
  can then render every step the task took from the thread alone.

`cargo run -p atif --example read -- thread.json` reads an export back.
The document also validates against Harbor's `Trajectory` model
(`bench/terminal-bench/tbench/atif.py`). To carry it as
[NIP-ATIF](../../nips/openagents/NIP-ATIF.md), treat it as the document
form.

## Tests

- `crates/openagents-cli/tests/chat.rs` runs the command against a local
  NIP-42 relay with a scripted chat worker: a knowledge answer with an old
  citation stripped, streamed partials, a follow-up carrying context, the
  NDJSON events, `threads`, `read`, `export` read back by `crates/atif`, an
  offer that cannot be accepted without a host, text mode, and exit codes.
- `live_chat_answers_from_product_knowledge` in the same file is the live
  check against the public worker, ignored by default:
  `cargo test -p openagents-cli --test chat -- --ignored --nocapture`.
- `live_chat_runs_coder_on_this_computer`, also ignored, runs a coding
  request in a scratch Python checkout with this computer's own Codex or
  Claude Code login; build the engine first (`cargo build -p microcoder`).
- `crates/microcoder/src/repository/tests.rs` (`local_run`) runs the shared
  local start with scripted provider output (a Codex refusal, Claude Code's
  steps, a question, an answer, an approval, no capacity, a stop) and checks
  every event type, the reason in `coder_started`, the worktree, and an
  identical replay. `crates/openagents-chat/src/coder_events.rs` tests the
  step mapping; `crates/coder/src/task/local.rs` tests checkout detection,
  provider choice with its reason, each local capability setting's
  effect, and that the defaults change nothing.
- `settings_change_the_local_run_and_the_defaults_change_nothing` in
  `crates/openagents-cli/tests/chat.rs` runs `openagents settings` and the
  local run each setting changes that a terminal can observe.
- `crates/openagents-chat/src/thread.rs` tests the ATIF mapping and the
  paged reader.
- `crates/openagents-chat/src/client_tests.rs` runs the client's event
  stream against the in-process service with a scripted door and against a
  fake host: the turn's surface and client word, a coding reply starting
  Coder at once and its events, `ask_first`, a stop, and backend selection.
  `a_terminal_turn_through_the_host_says_terminal` in
  `crates/coder-host/tests/threads.rs` checks that a terminal's turn through
  a real host reaches the worker as `terminal`.

## Not supported yet

- Rename, pin, archive, restore, and retry have service commands but no
  `chat` subcommands.
- A scratch thread is never moved into the host.
- Local runs reach what `coder.access` allows: `full` by default (no
  sandbox, Coder runs every step without asking, #10104). Under `boundary` or `toolchains`,
  on macOS the boundary cannot load Xcode's `xcrun`, so `/usr/bin/python3`
  (the Xcode shim) fails inside it; Coder finds another interpreter, such as
  `/Library/Developer/CommandLineTools/usr/bin/python3`, or says it could
  not run the tests.
- Eval offers (`start_eval`, `publish_eval`) and screen offers are printed;
  accepting them needs the app.
- Suggestion ranking for a new chat (a rank job) is a phone feature and is
  not sent from the terminal.

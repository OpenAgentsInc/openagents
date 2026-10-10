# Alice runbook

This runbook is for you, Alice's owner. It covers how to hand her work, how
to let her code with fewer interruptions, and how to watch, stop, and audit
her. Every command here exists in the host build as of 2026-10-07
(`openagents 1.0.0-rc.5`, commit `78d31a8168`). [Workshop
agent](workshop-agent.md) is the design reference, and [Alice on Codex:
handoff](alice-codex-handoff.md) records how the Codex engine landed.

## Contents

- [Overview](#overview)
- [Setup checklist](#setup-checklist)
- [Delegate work](#delegate-work)
- [Put her on autopilot](#put-her-on-autopilot)
- [Supervise, stop, and audit](#supervise-stop-and-audit)
- [Troubleshooting](#troubleshooting)
- [Quick reference](#quick-reference)

## Overview

Alice is the workshop agent: a persistent agent you own, with her own Nostr
key, which your key attests. The resident Coder host is her only authority.
It plans, checks, and journals each request, and Verse, `openagents agent`,
the smart terminal's `@alice`, and the phone are its clients.

Work passes down a chain:

```text
you ──▶ Alice ──▶ Coder V1 ──▶ Codex or Devin (optional)
       plans,     runs the       edits files in her worktree
       judges,    commands,      under its own sandbox
       reports    checks the delegate
```

1. **You** send a request from her desk in Verse, the CLI, `@alice`, or the
   phone.
1. **Alice** recalls her memory, plans the request with her own model, and
   sends each step to Coder as a plain prompt. Jev judges each Coder turn, and
   she reports back in at most three sentences.
1. **Coder V1** (`openagents coder`) runs the commands. In terminal mode it
   runs in her session `alice-coder`; in task mode it runs in a per-task
   session, `task-ID`, in a worktree of her own.
1. **Codex or Devin** does the file edits in task mode when her engine is
   `codex` or `devin`. Coder stays the driver and checks the delegate's
   result.

She lives in the owner's house at the east end of Library Way in Everglade.
She stands at her workstation, a walnut standing desk in the great room. While
a command runs she stands at the console by the east wall, and while she waits
for you she stands behind the lectern. `verse --owners-house` starts you just
inside the front door, facing her.

She has two modes:

- **Terminal mode** answers questions and runs commands in her workspace.
  Read-only commands run without asking. Any other command goes to her
  approval policy, and what the policy doesn't confirm waits for your
  CONFIRM or REJECT.
- **Task mode** makes a code change in her own worktree on branch
  `studio/alice/TASK-SLUG`, runs the checks, and leaves the change at the
  Merge station. Task mode asks nobody: it runs under the host's auto-start
  policy and its sandbox.

## Setup checklist

Work through this list once, and again after a host reinstall.

1. **The host runs.** `openagents studio status` prints her seat when the
   host answers. On this Mac the development host runs it; check it with
   `scripts/desktop/dev-host.sh status`. When no host answers,
   `openagents studio host` starts one, and so does CONFIRM at her desk.
1. **The `openagents` on your `PATH` is the host's build.** Run
   `which -a openagents` and `openagents --version`. The commit must match
   the host's, which `readlink ~/.openagents/dev-host/current` names. See
   [An old `openagents` on `PATH`](#an-old-openagents-on-path).
1. **Her record exists.** `openagents agent show alice` prints her state, key,
   attestation, workspace, charter, transcript, and budget. Her files are in
   `~/.openagents/host/agents/alice/`.
1. **She's attested.** `show` prints `attested until` and `authorized by`.
   She was attested on 2026-10-07 for 365 days with the owner key in
   `~/work/.secrets/coder-owner.key`, so the attestation expires on
   2027-10-07. Her panel and `show` warn 14 days ahead. To renew, run:

    ```sh
    openagents agent renew alice --owner-key ~/work/.secrets/coder-owner.key --days 365
    ```

    Without an attestation she can't seal NIP-AM spend records; her request
    budget still holds in memory.
1. **Her engine is set.** `openagents agent engine alice codex` has Codex do
   her coding, `openagents agent engine alice devin` has the Devin CLI do it,
   and `openagents agent engine alice coder` returns it to Coder's own
   model. `openagents agent engine alice devin:MODEL` pins a model `devin
   models list` names. The record's `engine` field in
   `~/.openagents/host/agents/alice/agent.json` holds the choice.
1. **The delegate is reachable.** For the `codex` engine, `openagents coder
   agents list` must show the `codex` agent as enabled (`openagents coder
   agents enable codex` turns it on), and `coder host autostart show` must
   report `codex: connected`. For the `devin` engine, `openagents agent show
   alice` prints `Devin connected` when a `devin` binary and its stored CLI
   login are present, and the `devin-cli` agent must be enabled in
   `openagents coder agents list`.
1. **Task mode has a workspace and a route.** Task mode runs in a host
   workspace, a label in `~/.openagents/host/serve.json`. The host picks the
   workspace whose path matches her record's workspace, else the first one.
   Today that's `openagents`, which is `~/code/openagents`. Her studio
   seat takes its route from the auto-start policy's first route, so the
   policy must be on: `coder host autostart show` prints it.
1. **The workspace checkout is on a branch near `main`.** Studio worktrees
   start from the checkout's `HEAD`, and a merge needs a branch to land on.
   It's on `main` now. Before you hand her coding work, check it against
   `origin/main`:

    ```sh
    git -C ~/code/openagents fetch origin
    git -C ~/code/openagents status -sb
    ```

   When the checkout is clean and has no local commits, update it with
   `git -C ~/code/openagents merge --ff-only origin/main`. Preserve any
   work in progress before updating the checkout.
1. **Her policy and budget are what you want.** Without
   `agents/alice/policy.json` and `agents/alice/budget.json`, the defaults
   hold. See [The approval policy](#the-approval-policy) and
   [The budget](#the-budget).
1. **Providers have capacity.** `openagents capacity list` shows each
   provider, and `openagents capacity check codex` exits `1` while a recorded
   limit holds.

## Delegate work

### At her desk in Verse

1. Walk into the owner's house and up to her workstation, and press **F**.
1. Type a request and press **Enter**.
1. When she proposes a command, the panel shows a `PROPOSED:` line. Press
   **Enter** on an empty input line to CONFIRM it, or **Esc** to REJECT it.
   Coder continues either way. A rejected command ends the request; she never
   works around it.
1. While she works, **`` Ctrl+` ``** shows her pane, titled `driven by alice`,
   running Coder's terminal on her session.

The key strip reads `ENTER SEND  ESC CLOSE  F2 MEMORY  F3 PLAN  F4 JOURNAL
F7 STOP  F8 PAUSE  PGUP PGDN`. **F7** and **F8** each ask for CONFIRM first.

### From the CLI

```sh
openagents agent ask alice "TEXT" [--mode auto|task|terminal] [--workspace LABEL] [--from DIR] [--wait]
```

- **`--mode`.** `auto`, the default, chooses by the first word of the request:
  `fix`, `add`, `change`, `implement`, `update`, `remove`, `write`, and the
  other change verbs choose task mode; `run`, `check`, `what`, `show`, and
  every other word choose terminal mode. "Work issue #N" starts with `work`,
  so `auto` reads it as terminal mode. Pass `--mode task` for every coding
  request and `--mode terminal` for every question.
- **`--wait`.** Follows her transcript until she reports, and prints each
  `PROPOSED: CMD (WHY)` line as it appears. Answer from another shell with
  `openagents agent answer alice confirm` or `openagents agent answer alice
  reject`. `--wait` gives up after an hour without a report; the work goes on.
- **`--workspace`.** Names a host workspace label. Without it, the host
  picks as the [setup checklist](#setup-checklist) describes.
- **`--from`.** The directory you asked from, which she receives as context.
  It defaults to your current directory.
- `@alice TEXT` in the smart terminal sends the same request.

Read her state with `openagents agent show alice` and her journal with
`openagents agent log alice`, newest last. `openagents agent list` shows
every agent with a service record: requests, finished, and merged.

She runs one request at a time. Up to four more wait in her queue, and the
host refuses a request beyond that. A request Jev reads as a note to keep
("remember that mobile is its own workspace") becomes a memory note; one
that asks her to do something ("remember to fix the login bug") is work.

### Delegate a GitHub issue

This pattern worked for issue #10893, which she did end to end on Codex in
about 32 minutes. Coder's turn, Codex's three delegations included, took
about 6 minutes; the rest was the studio's checks waiting for a build lease.

1. Claim the issue yourself. She never comments on issues.

    ```sh
    openagents issue claim 10893
    scripts/project-status.sh 10893 in-progress
    ```

1. Bring the workspace checkout up to date, as in the
   [setup checklist](#setup-checklist).
1. Ask in task mode with a summary, the files touched, exact acceptance
   criteria, the check command, and a scope limit:

    ```sh
    openagents agent ask alice --mode task --wait "Work issue #10893: drop the command hint from the agent's PROPOSED line. In crates/openagents-cli/src/agent.rs, both 'openagents agent show' and 'openagents agent ask --wait' print 'PROPOSED: CMD (WHY) -- openagents agent answer NAME confirm|reject'. Acceptance: both print exactly 'PROPOSED: CMD (WHY)' with no hint; one function in that file formats the line and both places use it; a unit test pins the format; cargo test -p openagents-cli --bin openagents -- agent passes. Only crates/openagents-cli/src/agent.rs changes."
    ```

1. When she reports that her change waits at the Merge station, find the
   task with `openagents studio tasks`, review it, and merge it. See
   [The Merge station](#the-merge-station).
1. Push the merged commit from your identity, then close the issue:

    ```sh
    git -C ~/code/openagents fetch origin
    git -C ~/code/openagents rebase origin/main
    git -C ~/code/openagents push origin HEAD:main
    ```

Keep the word "merge" out of a work request. A request that names merging
and asks her to do it, for example with "merge it", "for me", or "do it",
goes to her own merge instead of the work.

## Put her on autopilot

Each lever below widens what she does without you. They're listed from the
narrowest to the widest. None of them lets her push, publish, pay, install
software, or read credentials.

### The approval policy

**What it changes.** Which terminal-mode approvals she confirms herself
instead of asking you. It doesn't affect task mode, which asks nobody.

**File.** `~/.openagents/host/agents/alice/policy.json`
(`openagents.agent-policy.v1`). You edit it; she never does. She reads it at
the start of each request, so no restart is needed. Without the file, the defaults are:

```json
{
  "schema": "openagents.agent-policy.v1",
  "rules": [
    {"tool": "run", "command": "", "directory": "worktree"},
    {"tool": "run", "command": "", "directory": "scratch"},
    {"tool": "run", "command": "cargo fmt", "directory": "workspace"}
  ]
}
```

Each rule is a tool, a command prefix matched word by word (empty matches any
command), and a directory the command must stay in: `workspace`, `worktree`
(the studio worktrees), `scratch` (the system's temporary directories), or an
absolute path. To let her fetch and run leased builds in her workspace
without asking, add rules such as:

```json
{"tool": "run", "command": "git fetch", "directory": "workspace"},
{"tool": "run", "command": "openagents lease build", "directory": "workspace"}
```

**Risks.** A rule with an empty command in `workspace` confirms every command
in your checkout, including deletions. Keep prefixes exact. Her never list
wins over every rule: `git push`, `gh pr create`, `gh pr merge`, `cargo
publish`, payments, installs, `sudo`, and credential reads are refused, and
so are the host's deny-listed commands. A policy file that doesn't parse
sends every approval to you.

### The budget

**What it changes.** How much model usage she may spend in terminal mode
before she stops.

**File.** `~/.openagents/host/agents/alice/budget.json`
(`openagents.agent-budget.v1`). She reads it before each request. Without the
file the defaults are $1 and 2,000,000 tokens a request, and $5 and
10,000,000 tokens a UTC day. A missing field takes its default:

```json
{
  "schema": "openagents.agent-budget.v1",
  "v": 1,
  "request_usd": 2.0,
  "daily_usd": 10.0,
  "request_tokens": 4000000,
  "daily_tokens": 20000000
}
```

**Risks and limits.** An unreadable budget refuses every request. The budget
meters her plan and report calls and the Coder turns she prompts in terminal
mode. Task-mode runs don't pass through her meter: the auto-start policy and
provider capacity bound them. Codex on the ChatGPT login reports tokens but no
dollar price, so only the token limits bound it. Each standing job has its own
budget in `jobs.json`.

### Her merge, when you ask

**What it changes.** You don't have to run the merge yourself.

**How.** Ask her, for example `openagents agent ask alice "Can you merge your
change instead of me?" --wait`. She merges her newest change that waits at the
Merge station, whose checks passed to get there, through the station's own
**Merge**: into the workspace checkout's branch. It calls no model and pushes
nothing. Her charter states this: "the owner merges at the Merge station, or
she does when the owner asks her to."

**Risks.** She merges the newest waiting change of hers without showing you
the diff. Review it first with `openagents studio review TASK --diff`. The
checkout must be on a branch; see [A detached `HEAD`](#a-detached-head-refuses-the-merge).

### The auto-start policy

**What it changes.** Whether her task-mode tasks start at all, with what
access, and how many studio tasks run at once.

**Command.** `coder host autostart on`, which writes
`~/.openagents/host/autostart.json`. The current policy admits workspace
`openagents`, runs one task at a time, routes through `codex:gpt-6.1-sol`,
then `claude:claude-opus-5-5`, then `grok:default`, probes usage at 90%, and
runs with full access. To change only the workspaces and the bound and keep
the engine settings, run:

```sh
coder host autostart on --workspace openagents --max-running 2 --keep-engine
coder host autostart show
```

`coder host autostart off` stops new starts; running tasks keep running.
[Host auto-start policy](../coder/runtime/host-autostart.md) lists every
option.

**Risks.** `--full-access` runs each task's commands as you, with network
access and your login environment, limited only by macOS privacy prompts.
Her own task counts against `--max-running` with every other studio task, so
with a bound of 1 her task waits behind any other. Without a policy, task mode
refuses with "Task mode needs the host's auto-start policy to admit a route".

### The Codex engine

**What it changes.** Codex, on your ChatGPT login, makes the file edits in
task mode, and Coder on its own model drives and checks it.

**Command.** `openagents agent engine alice codex`. Undo it with
`openagents agent engine alice coder`.

**How it runs.** Her task brief ends with a line that asks Coder to delegate
the edits to Codex and to check the result. Codex edits her worktree under its
own `workspace-write` sandbox, with no network, and asks nobody. Questions and
terminal-mode work never go to Codex. Codex's tokens become a spend record of
their own (harness `codex-cli`).

**Risks.** Codex usage counts against your ChatGPT plan's limits. When Codex
hits a usage or rate limit, the limit goes in the capacity book
(`~/.openagents/tasks/capacity.json`), she says once that Codex is out of
capacity until its reset, and Coder codes on its own model until then.

### The Devin engine

**What it changes.** The Devin CLI on this computer, on its own stored
login, makes the file edits in task mode, and Coder on its own model drives
and checks it. While Devin is free it costs nothing; her spend still records
its tokens.

**Command.** `openagents agent engine alice devin`, or `devin:MODEL` for one
of the models `devin models list` shows, such as `devin:swe-2-high`. Undo it
with `openagents agent engine alice coder`. `openagents agent show alice`
prints `engine: devin (Devin connected)` while a `devin` binary and its
stored login are present.

**How it runs.** Her task brief asks Coder to delegate the edits to the
`devin-cli` subagent and to check the result, the same shape as Codex's. The
task's grant is full access, so Devin runs its `bypass` mode in her
worktree; a gated chat runs `accept-edits` under `devin --sandbox`, and
every permission request is refused. Questions and read-only lookups stay
with Coder. Devin's pane shows as a Devin child chat in her transcript, and
steering and stop reach it as `session/cancel`. Devin's tokens become a
spend record of their own (harness `devin-cli`).

**Failover.** When Devin is out of capacity — a recorded Devin refusal in
the capacity book (`~/.openagents/tasks/capacity.json`), which holds 30
minutes since Devin reports no reset — she says so once, and Coder falls
back to Codex, then to its own model.

**Risks.** Devin's tool calls bypass the host's command boundary under full
access: in `bypass` it runs its exec tool without asking, inside her
worktree. Her policy says Devin edits only her worktree; the boundary form
(`accept-edits` under `devin --sandbox`) holds when a turn is gated. A Devin
login expires; `devin auth login` restores it.

### A connected computer

**What it changes.** Her task-mode coding can run on another computer you
own — a second Mac or a Linux box — through the device grant `openagents
computer enroll` gives the host. The remote host's own auto-start policy
runs the task on the Devin route, directly in the checkout at `--path`,
which the host first resets to the commit yours had. When the remote task
ends, the checkout's whole diff comes back and lands in her task worktree
at the Merge station, where you review and merge it like a local task.

**Command.** `openagents agent computers alice add coderos-4080 --max 1
--path ~/work/openagents-alice` lets her place work on the computer your
`openagents computer list` calls `coderos-4080`, at most that many tasks at
once, in its checkout at `--path` (default `~/work/<workspace label>`).
Give her a checkout of her own there; see the risks below. The remote
host must admit that checkout as a workspace, and its auto-start policy
must admit a `devin:` route.
`openagents agent computers alice list` shows the set; `remove` takes one
back. One request names its computer with `openagents agent ask alice
--mode task --computer coderos-4080 TEXT`, `local` pins it here, and `auto`
(the default when her policy names computers) picks the first with a free
slot, falling back to this host when none answers.

**How it runs.** Before each task, the host runs `git reset --hard BASE`
and `git clean -fd` in the remote checkout, cloning it first when it's
absent. The brief asks the remote worker to leave the change in the
checkout — committed or not — and to stage new files so the diff is
whole; it never pushes, merges, or opens a pull request. A commit that is
not pushed to the checkout's `origin` is refused before anything starts.
`agent stop` cancels the remote task through the same grant, so its Devin
session stops too. A task that fails, or whose patch cannot land, closes
with its reason in the studio note instead of a merge decision.

**Risks.** Every task discards uncommitted changes and untracked files
in the remote checkout, so never point `--path` at a checkout anyone
edits by hand, including the default `~/work/openagents`. Keep build
output outside it: a target directory inside the checkout counts against
the remote host's [workspace snapshot bound](#the-workspace-snapshot-bound).
The computer sees her request text and its checkout; its own
policy still decides what its tasks may do. The grant is the owner's, held
by the host — she holds no credential on the computer. An offline computer
is reported once and the work falls back to this host or stays with `auto`
until one has a slot.

### The work queue

**What it changes.** `openagents agent queue alice add TEXT` files a
task-mode request away instead of asking her now. Every time a request of
hers ends she takes the oldest waiting entry — through the night, not only
while you watch. The file lives beside her record, so a restart keeps it,
and an entry interrupted mid-run waits again instead of being lost.

**Command.** `queue alice add "Work issue 10932 in openagents"` queues the
text, `--computer` places it like `ask` does (`auto` picks the first
computer with a free slot, `local` keeps it here, a name pins it), and
`--workspace` names its workspace. `queue alice list` shows waiting,
running, and recent finished entries; `queue alice remove ID` takes a
waiting one back; `queue alice clear` empties it. `agent stop` clears the
queue along with everything else of hers.

**How it runs.** She is still one request at a time, so entries start in
the order you added them and their changes reach the Merge station in that
order — the queue is the merge order. Each entry is placed when it starts,
not when it was queued: a box that comes up mid-queue takes the entries
that waited for it, and one that stays down falls back to this host with a
line she says. A failed entry is marked and the next one starts; nothing
stalls the queue on one bad task.

**Risks.** Every entry runs with the same permissions a task-mode request
has — her policy, her worktree, your merge decision. The queue holds at
most 32 waiting entries; its file is `agents/NAME/queue.json` and nothing
else reads it, so never edit it by hand while she runs.

### Standing jobs

**What it changes.** Work starts on a schedule or a trigger instead of a
request from you. Every job starts off, expires within 90 days, and has a
budget per occurrence and per job. Each occurrence is admitted when it fires;
a refused one is journaled and skipped, never queued.

```sh
openagents agent jobs alice add watch-issues --repository OpenAgentsInc/openagents --label agent --workspace openagents
openagents agent jobs alice on watch-issues
openagents agent jobs alice list
```

| Template | Trigger | What she does |
| --- | --- | --- |
| `nightly-check` | Daily at 02:00 | Runs the workspace's checks read-only and reports failures. |
| `watch-issues` | An open issue with the label that the pickup rules leave free | Works the issue in task mode to the Merge station. Never merges, pushes, or comments. |
| `keep-green` | The default branch moved | Runs the checks; when one fails, queues a task-mode fix that goes to the Merge station. |
| `reflect` | Daily at 03:00, and early on a busy day | Writes insights that cite her records; see [Generative agents](generative-agents.md#2-reflection-with-checked-citations). |
| `plan` | Daily at 07:00 | Drafts her day plan. |

Renew a job with `openagents agent jobs alice renew JOB --days 30` (at most
90). Turn one off with `openagents agent jobs alice off JOB`.

**Risks.** `watch-issues` doesn't claim the issue, so claim each labeled issue
yourself, or label only issues no other agent works. Each occurrence costs
model usage up to its budget, $0.50 by default.

### Day plans from real work

**What it changes.** What she shows and where she walks, not what she does.
The `plan` job drafts 5 to 8 blocks each morning from real sources only:
scheduled jobs, issues `watch-issues` would pick, your queued requests, and
her accepted insights. F3 at her desk and the board on the great room's west
wall show it. It costs about $0.10 a day.

```sh
openagents agent jobs alice add plan
openagents agent jobs alice on plan
```

A day plan never starts work. The jobs and your requests do.

### Several tasks at once

Alice herself is serial: one request runs, and up to four wait. To run
several changes in parallel, use the Agent Studio's seats beside her, each
on a route the auto-start policy admits:

```sh
openagents studio seat set bob --route codex:gpt-6.1-sol
openagents studio goal submit "TEXT" --workspace openagents
coder host autostart on --workspace openagents --max-running 3 --keep-engine
```

A studio seat's approvals arrive as decisions: `openagents studio decisions`
lists them, and `openagents studio answer DECISION allow --always` keeps a
standing rule for that seat's exact tool, command, and directory. See
[Agent Studio](agent-studio.md).

**Risks.** Parallel tasks contend for build leases, and #10893 already spent
most of its time waiting for one. Each running task spends against its
provider's limits.

### What isn't possible today

- **Pushing.** She can't push to `main` or any remote. `git push` is on her
  never list, and the Merge station's merge pushes nothing. You push.
- **Unattended merges.** She merges only when a request from you asks her to.
  No setting makes her merge on her own, and the standing jobs' requests tell
  her never to merge.
- **Pull requests and issue comments.** `gh pr create` is refused, and the
  job templates forbid comments. Claim issues yourself.
- **Raising her own limits.** She can't change her policy, budget, charter,
  jobs, or the auto-start policy.
- **Budgeting task mode.** `budget.json` meters terminal mode only.
- **Parallel requests to her.** She runs one request at a time.
- **Other computers.** Her task-mode coding can run on a connected
  computer her policy names (`agent computers`), but her planning,
  questions, and checks stay on her home host.

## Supervise, stop, and audit

### Watch her

- **Her pane.** In Verse, **`` Ctrl+` ``** shows her pane while she works. From any
  terminal on the host, `~/.openagents/dev-host/current/coder-new --follow
  alice-coder` follows her terminal-mode session.
- **The studio.** `openagents studio status` shows her seat's activity and
  task, `openagents studio watch` prints each change as it happens, and
  `openagents studio log alice` prints her engine's last actions.

### Take over

Press any key in her pane. Her turn stops, Coder saves the session, and the
session is yours to type in. To hand it back, quit Coder in her pane with
**Ctrl+C**; she takes the session back before her next prompt. Until you
quit, she answers each request with "You have my Coder session open".

### Stop, pause, and reject

| Action | Verse | CLI |
| --- | --- | --- |
| Reject a proposed command | **Esc** | `openagents agent answer alice reject` |
| Pause: she keeps everything and starts nothing new | **F8**, then CONFIRM | `openagents agent pause alice` |
| Resume | **F8**, then CONFIRM | `openagents agent resume alice` |
| Stop: jobs off, panes released with Ctrl+C, work cancelled | **F7**, then CONFIRM | `openagents agent stop alice --reason TEXT` |
| Cancel one studio task | None | `openagents studio task cancel TASK` |

Stop journals each step and can't prove that an effect stopped; an uncertain
effect stays marked unknown. After a stop, she starts nothing until you run
`resume`.

### The Merge station

The Merge station is the strongroom in the Everglade workshop yard. Every
task-mode change waits there:

```sh
openagents studio tasks                       # find the task ID
openagents studio review TASK --diff          # revisions, files, line counts, and the diff
openagents studio merge TASK --head REV       # merge into the checkout's branch; pushes nothing
openagents studio request-changes TASK "TEXT" # send it back to her seat
openagents studio reject TASK "REASON"        # close it; the worktree stays until archived
```

A unique prefix of the task ID works. `--head` refuses the merge when the
change moved past the revision you reviewed. Her journal redacts task IDs,
so read them from `studio tasks` or `studio status`.

### Spend

`openagents agent show alice --owner-key ~/work/.secrets/coder-owner.key`
decrypts her NIP-AM records and prints today's spend and the total against
her budget. Without `--owner-key`, `show` may print that spend isn't read.
The records are in `agents/alice/spend.jsonl`, signed with her key and
encrypted to yours: one per call that reported usage, including Coder turns
(harness `coder-v1`) and Codex (harness `codex-cli`).

### Audit trail

- **Journal.** `openagents agent log alice [--after N]` prints every request,
  plan, prompt, proposal and its answer, task, merge, memory write, job
  occurrence, stop, and engine change, with times. It holds no secrets and no
  terminal output. The file is `agents/alice/journal.jsonl`, append-only.
- **Transcripts.** `openagents coder sessions read alice-coder` reads her
  terminal-mode session as ATIF, and `openagents coder export alice-coder
  --output FILE` exports it. A task's session, Codex's child chats included,
  is `~/.openagents/coder-new/sessions/task-ID.atif.json`.
- **Signed commits.** `openagents agent signing alice on` signs the tip of
  her merged worktree changes with her key (NIP-GS), with your attestation
  embedded. It's off by default.

### Memory and engrams

- **F2** at her desk shows her memory; type there to add a note.
- `openagents agent memory alice list` lists projects, preferences, outcomes,
  notes, insights, and her knowledge-entry drafts.
- `openagents agent memory alice note TEXT`, `forget ID`, `accept ID`, and
  `reject ID` edit it. A preference she proposes shapes nothing until you
  accept it.
- `openagents agent memory alice engrams --owner-key FILE` prints her NIP-AE
  engram heads, decrypted. `--orphans` lists memories her core doesn't link.
- Relay sync is off until you run `openagents agent memory alice sync on`.
  See [Agent identity and engrams](agent-identity-and-engrams.md).

## Troubleshooting

These problems all happened during the week of 2026-10-05.

### The workspace checkout is stale

**Symptom.** A task fails because a file the request names doesn't exist, or
the change is built on old code. **Cause.** Studio worktrees start from the
workspace checkout's `HEAD`, not from `origin/main`. **Fix.** Bring
Alice's workspace up to date as the [setup
checklist](#setup-checklist) shows, then ask again.

### A detached `HEAD` refuses the merge

**Symptom.** "The checkout at /Users/christopherdavid/work/openagents-phone is
not on a branch. Check out the branch to merge into, then merge again."
**Fix.** Put the affected checkout on a branch, then merge again. For the
older phone checkout:

```sh
git -C ~/work/openagents-phone switch -c alice-landing
openagents studio merge TASK
```

### The workspace snapshot bound

**Symptom.** Admission refuses every task with "the workspace snapshot is
incomplete: the snapshot stopped at its limit". **Cause.** Admission hashes
the whole workspace. The bound is 16 GiB since commit `203a8cac4c`; the
repository's tracked files are 4.28 GiB. **Fix.** Keep Cargo target
directories and other build output outside the workspace checkout, and make
sure the host runs a build from after that commit.

### An old `openagents` on `PATH`

**Symptom.** A studio Coder turn fails on an unknown flag, such as
`--codex-writes`. **Cause.** The host runs the `openagents` beside its own
program or the first one on `PATH` (`OPENAGENTS_CODER_CLI` overrides it).
**Fix.** `which -a openagents` and `openagents --version` show which one
runs. `scripts/desktop/dev-host.sh install` puts the host's build in
`~/.openagents/bin/openagents`; remove or update any older copy ahead of it on
`PATH`.

### The development host's install and follow job

**Symptom.** `main` moves, but the host stays on an old commit.
**Causes and fixes.**

- The follow job (`scripts/desktop/dev-host.sh follow-on`) installs a new
  build only while no task or turn runs, and the build itself can wait a long
  time for a lease. Read `~/.openagents/dev-host/follow.log`, or install now
  with `scripts/desktop/dev-host.sh install`.
- After a crash, `~/Library/LaunchAgents/com.openagents.dev.host.plist` can be
  missing. Restore it from the newest backup,
  `~/.openagents/dev-host/com.openagents.dev.host.plist.bak.TIMESTAMP`, and
  load it with `launchctl bootstrap gui/$(id -u)
  ~/Library/LaunchAgents/com.openagents.dev.host.plist`.
- A crash can also leave the lock directory
  `~/.openagents/dev-host/follow.lock`, which stops every follow pass. When no
  pass runs, remove it with `rmdir ~/.openagents/dev-host/follow.lock`.

`readlink ~/.openagents/dev-host/current` names the installed commit, and
`~/.openagents/dev-host/host.log` is the host's log.

### Codex usage limits

**Symptom.** She says Codex is out of capacity until a time, and Coder codes
on its own model. **Fix.** Nothing is broken. `openagents capacity list` shows
the recorded limit, and `coder host autostart show` shows each provider's
usage windows. She uses Codex again after the reset. If a limit was recorded
in error, it lapses at its reset time.

### A question goes to Codex

**Symptom.** A read-only question proposes `codex exec --sandbox
workspace-write`. **Cause.** The host predates commit `655a57b3c2`, after
which terminal mode sends no Codex directive and a gated Codex is always
read-only. **Fix.** Update the host.

### "Invalid native Verse configuration"

This error comes from the phone apps' Verse configuration
(`crates/openagents-mobile` and `crates/coder-mobile`). It has nothing to do
with Alice or the host.

### Other refusals

- "Task mode needs the host's auto-start policy to admit a route": turn the
  policy on with `coder host autostart on`.
- "alice is paused, so she starts nothing new": run
  `openagents agent resume alice`.
- "can't reach her key": her key in the host's keychain is unreadable. She
  runs nothing until you restore it; the host never makes her a new one.

## Quick reference

| Task | Command |
| --- | --- |
| Show her state, attestation, and transcript | `openagents agent show alice` |
| Show her spend | `openagents agent show alice --owner-key ~/work/.secrets/coder-owner.key` |
| Read her journal | `openagents agent log alice` |
| Ask a question | `openagents agent ask alice "TEXT" --mode terminal --wait` |
| Hand her coding work | `openagents agent ask alice "TEXT" --mode task --wait` |
| Answer a proposal | `openagents agent answer alice confirm` or `reject` |
| Code on Codex, Devin, or Coder | `openagents agent engine alice codex`, `devin[:MODEL]`, or `coder` |
| Find her task | `openagents studio tasks` |
| Review a change | `openagents studio review TASK --diff` |
| Merge a change | `openagents studio merge TASK --head REV` |
| Ask her to merge | `openagents agent ask alice "Can you merge your change instead of me?" --wait` |
| Send a change back | `openagents studio request-changes TASK "TEXT"` |
| Pause or resume | `openagents agent pause alice`, `openagents agent resume alice` |
| Stop her | `openagents agent stop alice --reason TEXT` |
| Renew her attestation | `openagents agent renew alice --owner-key FILE --days 365` |
| List, add, or switch on jobs | `openagents agent jobs alice list`, `add TEMPLATE`, `on JOB` |
| Show or change auto-start | `coder host autostart show`, `coder host autostart on ... --keep-engine` |
| Check provider capacity | `openagents capacity list`, `openagents capacity check codex` |
| Read her memory | `openagents agent memory alice list` |
| Follow her session | `~/.openagents/dev-host/current/coder-new --follow alice-coder` |
| Watch the studio | `openagents studio watch` |
| Check the development host | `scripts/desktop/dev-host.sh status` |

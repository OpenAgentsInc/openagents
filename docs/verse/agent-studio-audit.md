# Agent Studio audit

Status: audit, October 4, 2026, against `main` at `76c6f86aa4`. The owner
asked whether the [Agent Studio](agent-studio.md) in
[Everglade](everglade.md), its Claude Agent SDK integration, and the
[AgentCraft parity](agentcraft-parity.md) work run end to end, and whether
all of it is reachable from the `openagents` command and Verse's flags,
so that day-to-day coding can move from Claude Code and Codex into the
studio.

## Summary

- The host half works. A scratch host built from current code accepted a
  goal, raised a decision when its lead ended without a plan, released a
  two-task plan with its dependency held, gave the released task its own
  worktree and branch, served a review of a commit in that worktree, and
  merged it locally into the checkout's branch without pushing. Every
  steering intent returned a receipt.
- No real engine has ever driven the studio. The mixed-engine acceptance
  run (#10477) is still an owner step in `NEEDS_OWNER.md`. Checks, the
  lead's review, conflict handling, questions, and approvals are covered
  only by unit and scratch-host tests with engines stood in.
- The Claude Agent SDK port (`crates/claude_agent_sdk`) is not used. No
  crate depends on it, and no studio seat can run on it. A Claude seat runs
  either a lean Claude Code session (`claude -p` with
  `--permission-mode bypassPermissions`) or Microcoder's loop with Claude
  as the model.
- The command line covers about a third of what the Everglade panels do.
  `openagents studio` can set up seats, submit goals, list goals and plans,
  message seats, and keep shared memory. It cannot list decisions, answer
  them, open a review, merge, request changes, reject, pause, resume, stop,
  cancel, retry, reassign, or prioritize. Those steps need Verse on the
  desktop today.
- No installed `openagents` binary has the `studio` command. The one on
  `PATH` is an unrelated August build, and the newest workspace build
  predates the studio.

## Binaries found

No new builds ran for this audit. Each binary was used read-only.

| Binary | Path | Commit | Usable for the studio |
| --- | --- | --- | --- |
| `openagents` | `~/work/openagents-target-agent1/debug/openagents` | `105824cae4` dirty, October 3, 04:56; 396 commits behind `main` | No: it predates the studio coordinator (`4daa195d73`, October 4, 01:17) and has no `studio` command |
| `openagents` on `PATH` | `~/.openagents/bin/openagents`, a link to `~/.openagents/downloads/openagents-macos-aarch64` (August 29) | Unknown; it has no `version` command | No: a different, older program |
| `coder` | `~/work/openagents-target-agent1/release/coder` | `c3a2b76149` dirty | Yes: no studio-related source changed after it |
| `coder` | `~/.openagents/dev-host/current/coder` | `02058d61ed` clean | Yes: the owner's running host (`serve --keychain --iroh --control`) runs this build |
| `verse` | `~/work/openagents-target-agent1/release/verse` (October 4, 15:01) | Not reported; `verse` has no `--version` | Yes: built after the panels learned to act (`a8456d03ed`); the owner runs it as `verse --everglade` |
| `verse` | `~/work/openagents-target-agent1/debug/verse` (October 3, 20:33) | Not reported | No: it predates the live studio source |

Four scratch `coder host serve --loopback-test` processes from earlier test
runs (started September 30 through October 2, under
`/var/folders/.../T/.tmp*`) are still running on the owner's Mac. This
audit did not stop them.

## Scratch-host smoke

Because no `openagents` binary has the `studio` command, the smoke drove
the host directly over its control socket, the way Everglade does: a
30-line Python client in the session's scratch directory (not committed)
sent `openagents.control.v1` frames carrying NIP-HOST `studio.*`
operations. Seats were written into the scratch task store's
`studio/state.json` as a stand-in for `openagents studio seat set`.
Auto-start stayed off, so no engine ran. Everything lived under a
temporary directory with a temporary `HOME`; every task was archived and
the directory was deleted afterward.

```sh
T=$(mktemp -d .../studio-smoke.XXXX)        # scratch repo at $T/repo, 2 commits
env -i PATH=/usr/bin:/bin HOME=$T/home \
  ~/.openagents/dev-host/current/coder host serve \
  --state $T/state --root $T/root --tasks $T/tasks --keys $T/keys \
  --control-socket /private/tmp/claude-501/oas6Z.sock \
  --relay ws://127.0.0.1:9 --loopback-test \
  --workspace demo=$T/repo --no-telemetry --no-runtime
```

| Step | Operation | Result |
| --- | --- | --- |
| Empty studio | `studio.snapshot` | Works: stream `18db6e1a…`, sequence 1, empty view |
| Goal with no seats | `studio.goal.submit` | Refused `conflict`, message "host refused `studio.goal.submit`"; the reason (no lead seat) is lost |
| Goal with a lead and a worker | `studio.goal.submit` | Works: goal `g1-1ed832be`, status `planning`, lead task queued |
| Message, pause, resume | `studio.seat.message`, `.pause`, `.resume` | Works: `dispatched` receipts |
| Message to an unknown seat | `studio.seat.message` | Refused `forbidden` with no reason |
| Cancel the lead | `studio.task.cancel` | Works: within about six seconds the goal showed a `lead_failed` decision, with auto-start off |
| Answer with a plan | `studio.decision.answer` carrying an `openagents.coder.studio-plan.v1` plan of two tasks | Works: goal `running`, first task `queued`, dependent task `held` |
| Worktree | (released task) | Works: `root/studio-worktrees/<task>` on branch `studio/ada/f2ec5b80-write-contributing-md` |
| Prioritize, reassign | `studio.task.prioritize`, `.reassign` | Works: receipts |
| Review | `studio.review.open` after a stand-in commit as `Studio Ada` | Works: base, head commit, head, one file, one line added |
| Merge | `studio.merge.decide` with verdict `merge` at the reviewed revisions | Works: merge commit `2cd1df1` on `main` in the scratch checkout; note "nothing was pushed" |
| Merge of an unfinished task | (same call) | Defect: the task was still `queued` and never ran, yet the merge landed; the task stayed `queued`, its dependent stayed `held`, and a later `studio.seat.stop` cancelled it |
| Updates | `studio.update` from sequence 2 | Works: delta to sequence 6; an unknown stream refuses `malformed` |
| Stop, retry | `studio.seat.stop`, `studio.task.retry` | Works: retry minted a new lead task |

The merge commit carried Git's fallback identity, because the temporary
`HOME` had no Git configuration. On the owner's computer it carries the
owner's configured identity.

Not exercised, because each needs a running engine or a window: questions
and approvals raised by an engine, **Always allow for this seat**, the
independent check and its fix round, the lead's review, conflict
resolution, **Request changes**, **Reject**, spend, and the Everglade
panels. `cargo test -p verse --test studio_host` covers the panels against
a scratch host with engines stood in.

## Capability status

Legend: **Works** was observed in this audit's smoke; **Tested** has unit or
scratch-host tests but was not observed with a real engine; **Partial**
works with a named limit; **Broken** misbehaves; **Missing** does not
exist.

| Capability | Status | Evidence |
| --- | --- | --- |
| Goal submission, lead task, plan validation, dependencies, release | Works | Smoke; `crates/coder/src/task/studio.rs`, `studio_tests.rs` |
| Coordinator reconciliation with auto-start off | Works | Smoke: the lead's failure became a decision within about six seconds |
| NIP-HOST `studio.*` over the control socket, snapshot and sequenced updates | Works | Smoke; `crates/coder/src/task/remote.rs`, `crates/coder-host/src/serve/dispatch.rs` |
| Steering: message, pause, resume, stop, cancel, retry, prioritize, reassign | Works at the protocol | Smoke receipts; effect on a running engine untested |
| Worktree and branch per task, Git transports refused | Works (worktree), Tested (transport refusal) | Smoke; `studio_git.rs`, `studio_git_tests.rs` |
| Review at exact revisions | Works | Smoke |
| Local merge, nothing pushed | Works | Smoke |
| Merge only of a finished task | Broken | Smoke: a `queued` task merged; `studio_merge` in `dispatch.rs` checks revisions, not task status |
| Refusal reasons | Partial | Smoke: only the code (`conflict`, `forbidden`) reaches the client; `studio_refusal` in `remote.rs` drops the coordinator's sentence |
| Questions and approvals from engines | Partial | Only Microcoder's loop ends a turn with a question or approval (`crates/microcoder/src/repository.rs`, `finish`); the Claude Code and Codex sessions run with permission bypass flags (`crates/coder-delegate/src/delegate.rs`) and never ask |
| Approver binding and standing rules | Tested | `studio_approvals.rs`, `studio_rules.rs` |
| Independent check, one fix round, lead review, conflicts back to the worker | Tested | `studio_flow.rs`, `studio_flow_tests.rs` |
| Codex seat | Tested | Route `codex:MODEL`; Microcoder's loop by default (`coder.codex` is `loop`), a `codex exec` session under full access with `coder.codex session` |
| Claude Code seat | Tested | Route `claude:MODEL`; a lean Claude Code session under full access (`coder.claude` defaults to `session`), the loop otherwise |
| Microcoder seat | Partial | Not a route; it is the loop mode of a Codex or Claude route, chosen for the whole host, not per seat |
| Devin, OpenCode, Grok Build seats | Tested | Routes accepted by `autostart::parse_route`; their ACP permission requests are answered by policy, not raised as studio approvals |
| Claude Agent SDK seat | Missing | `crates/claude_agent_sdk` has no dependents (`grep` over every `Cargo.toml`); its README still tells users to export `ANTHROPIC_API_KEY` |
| `openagents studio up --repo PATH` and `down` | Tested | `studio_up.rs` tests; not run here (no current binary, and it changes the host's policy). A policy it creates uses `boundary` access, so Claude seats then run the loop, not a Claude Code session |
| `openagents studio up --sim`, `verse --studio-sim` | Tested | #10572: `up --sim` starts a scratch host (`coder host serve --studio-sim`) whose scripted engine ends the studio's turns through `owner::scripted`; `crates/openagents-cli/tests/studio_host.rs` drives its question, approval, conflict, and merges through `openagents studio`. `verse --studio-sim` is still the recorded replay |
| Everglade live view, panels, intents (desktop) | Tested | `crates/verse/tests/studio_host.rs`; the owner's running `verse --everglade` has the code |
| Phone studio panels | Partial | `crates/coder-mobile/src/studio_panel.rs` observes only; "Everglade does not open" a host connection with `operate` or `review` |
| Seat configuration from Verse or NIP-HOST | Missing | No `studio.seat.set` operation; seats change only through the local `openagents studio seat` commands |
| Lead review on or off | Missing from every surface | `Studio::set_lead_review` has no command, operation, or panel |
| Real-engine end-to-end run | Missing | `NEEDS_OWNER.md`, "Agent Studio mixed-engine run (#10477)" |
| Documentation status lines | Broken | `agent-studio.md` says "Nothing in this document is implemented yet"; `everglade.md` says sending intents is not implemented; `docs/cli/README.md` omits `studio up` and `down`; `NEEDS_OWNER.md` says a landing merge needs a forge remote and lists a Microcoder worker as a route |

## Command-line gaps

What a person can do in Everglade's panels, and whether `openagents
studio` does it without Verse:

| Panel action | Everglade | `openagents studio` |
| --- | --- | --- |
| Create a goal | Console | Yes: `goal submit TEXT --workspace LABEL [--lead SEAT]` |
| See goals and progress | Atrium, HUD bar | Yes: `goal list` |
| See the plan and task graph | Task Wall, library | Partial: `plan list GOAL` shows entries, seats, progress, and dependencies, but not task identities, checks, branches, or the lead's task |
| See seats and what they do | Desks, seat panel | Partial: `seat list` shows route, desk, spend, and task, but not activity, station, or log tail |
| See a seat's log | Monitor, seat panel | Missing |
| List open decisions | Podium, `J` | Partial: `goal list` prints goal decisions only; task questions and approvals are missing |
| Answer a question or approval | Podium | Missing; `plan accept GOAL FILE` answers only a goal's plan decision |
| **Always allow for this seat** | Podium | Missing |
| Open a review and read the diff | Merge station | Missing |
| **Merge**, **Request changes**, **Reject** | Merge station | Missing |
| Message a seat | Console `@seat` | Yes: `message SEAT TEXT` |
| Pause, resume, stop a seat | Console, seat card | Missing |
| Cancel, retry, prioritize, reassign a task | Task card | Missing |
| Configure seats and routes | None | Yes: `seat set NAME --route PROVIDER:MODEL [--role] [--look] [--desk]`, `seat remove` |
| Choose Claude Code session, Codex session, or Microcoder's loop per seat | None | Missing: the policy's engine setting (`coder.claude` and `coder.codex` in `openagents settings`) chooses for the whole host |
| Run a seat on the Claude Agent SDK | None | Missing |
| Shared memory | Library | Yes: `memory add`, `memory list` |
| Watch changes as they happen | The world | Missing: no `watch` or NDJSON stream |
| Launch and tear down | n/a | Yes: `up`, `down` |

`openagents studio` also writes the task store directly instead of asking
the running host, so its commands get no NIP-HOST receipt or refusal code.
The host's control socket already brokers every `studio.*` operation
(`openagents_connect::control::Op::Task`), so the missing commands need
only a client, not new host code.

## Start using it today

This path uses what is on the owner's Mac now: the dev host at `02058d61ed`
and the running Verse build. The steps are the owner's to run; the audit
ran none of them against the owner's host.

### Prerequisites

- Codex is signed in (`~/.codex/auth.json`), and Claude Code is signed in.
  `coder host autostart status` reports both.
- The host runs with auto-start on. On this Mac the policy is enabled with
  `full` access, the workspace `openagents`, and the routes
  `codex:gpt-6.1-sol`, `claude:claude-opus-5-5`, and `grok:default`. A
  seat's route must match one of those exactly.
- A current `openagents` binary. Build it once into the agent slot's target
  directory and call it by path, because `openagents` on `PATH` is the
  August program:

  ```sh
  CARGO_TARGET_DIR=~/work/openagents-target-agent1 \
    cargo build --release -p openagents-cli
  OA=~/work/openagents-target-agent1/release/openagents
  $OA version
  ```

- A repository the merge can land in. A merge fast-forwards the branch
  checked out in the workspace's checkout and refuses a dirty one. The
  `openagents` workspace is the checkout other agents edit, so merges there
  will often refuse. A dedicated clone admitted as its own workspace avoids
  that; admitting it changes `serve.json`, and the desktop host must restart
  to read it.

### Steps

1. Seat a team on the admitted routes. Under the owner's `full` access
   policy, a `claude` seat runs a lean Claude Code session and a `codex`
   seat runs Microcoder's loop on Codex:

   ```sh
   $OA studio seat set lead --role lead --route codex:gpt-6.1-sol
   $OA studio seat set ada --route claude:claude-opus-5-5
   $OA studio seat set bob --route codex:gpt-6.1-sol
   $OA studio seat list
   ```

2. Submit a goal, from the console at Everglade's notice board or:

   ```sh
   $OA studio goal submit "Add a --verbose flag to openagents studio seat list" \
     --workspace openagents
   ```

3. Follow it: `$OA --json studio goal list`, `$OA studio plan list GOAL`,
   and `$OA studio seat list`, or walk the glade.
4. Answer questions and approvals at the podium, and open each finished
   task at the merge station to choose **Merge**, **Request changes**, or
   **Reject**. These steps have no command yet.
5. Push the merged branch from the checkout yourself. The studio pushes
   nothing.
6. Archive the tasks of a trial run: `$OA task archive TASK_ID --reason
   "studio trial"`.

`openagents studio up --repo PATH --no-verse` does steps 1 and part of the
setup in one command, but it rewrites the auto-start policy (and restores
it on `down`), and with the desktop's host running it reports that the host
needs a restart when it admits a new workspace.

### Blockers, most severe first

1. No real engine has run a studio goal. The first real run is the
   acceptance test; expect defects in the lead's plan reply, steering, and
   the check round.
2. Answering, reviewing, and merging need Verse on the desktop. The
   command line cannot do them, and phones only observe.
3. No current `openagents` binary is installed.
4. Claude Code seats never raise approvals, because the session bypasses
   permissions. Only Microcoder's loop asks. The Claude Agent SDK, whose
   permission callback could carry Claude's requests to the podium, is
   unused.
5. The host merges a task that has not finished, and does not mark a
   merged task done.
6. Refusals reach the person as a bare code.
7. The only admitted workspace is the shared main checkout, so merges
   refuse while it is dirty.

## Proposed issues

Each issue lists the files it owns. Issues in the same wave own disjoint
files and can run in parallel; a later wave waits for the issue it names.

### Wave 1

**1. Act on the host's studio from `openagents studio`.**

- Scope: a control-socket client for every studio intent and read:
  `status` (seats with activity and station, decisions, goals), `tasks
  [GOAL]` (identities, status, branch, check), `log SEAT`, `decisions`,
  `answer DECISION TEXT` and `--always RULE`, `review TASK [--diff]`,
  `merge TASK`, `request-changes TASK TEXT`, `reject TASK [REASON]`, `seat
  pause|resume|stop SEAT`, `task cancel|retry|prioritize|reassign`, and
  `watch` (NDJSON updates). Take `--control-socket`. Print the host's
  refusal code and message. Send `goal submit` and `message` through the
  host when one answers.
- Files: `crates/openagents-cli/src/studio.rs`, a new
  `crates/openagents-cli/src/studio_host.rs`, a new scratch-host test under
  `crates/openagents-cli/tests/`, `docs/cli/README.md` (including `up` and
  `down`), and `crates/coder/src/cli_route/tree.json` and
  `labeled-v1.json` if the command-tree test asks for them.
- Acceptance: a scratch-host test takes a goal from submission through a
  plan answer, a review, and a merge with only `openagents studio`
  commands, and archives its tasks; every command has `--json` output.

**2. Merge only finished studio tasks, and say why an intent was refused.**

- Scope: refuse **Merge** and **Request changes** for a task that is not
  waiting for review or done; mark a merged task done so its dependents
  release; carry the coordinator's sentence in every studio refusal.
- Files: `crates/coder-host/src/serve/dispatch.rs`,
  `crates/coder-host/src/serve/studio_tests.rs`,
  `crates/coder/src/task/remote.rs`, `crates/coder/src/task/studio_flow.rs`,
  `crates/coder/src/task/studio_flow_tests.rs`.
- Acceptance: tests show a merge of a queued task refused as `conflict`
  with a reason, a merged task `done` with its dependent released, and a
  goal with no lead seat refused with a sentence that names the missing
  lead seat.

**3. Let each seat choose its engine.**

- Scope: a seat route may name its engine,
  `PROVIDER[/ENGINE]:MODEL` with `session` or `loop` for `claude` and
  `codex`, overriding `coder.claude` and `coder.codex` for that seat's
  tasks; a `session` seat under `boundary` access is refused with the
  reason; the snapshot's route shows the engine so nameplates tell a
  Claude Code seat from Microcoder on Claude. `openagents studio seat set
  --route` and `up --team` pass the route through unchanged.
- Files: `crates/coder/src/task/studio.rs`,
  `crates/coder/src/task/studio_tests.rs`,
  `crates/coder/src/task/autostart.rs`.
- Acceptance: tests show two seats on `claude/session` and `claude/loop`
  producing grants with the session endpoint and the provider endpoint.

**4. Correct the studio's documentation.**

- Scope: replace the stale status lines; mark the simulated team as a
  read-only replay; record the command-line gaps until issue 1 lands; fix
  the #10477 steps (a merge needs no forge remote; Microcoder is an engine
  mode, not a route); state in the SDK README that a subscription login
  works and that nothing uses the crate yet.
- Files: `docs/verse/agent-studio.md`, `docs/verse/everglade.md`,
  `docs/verse/agentcraft-parity.md`, `NEEDS_OWNER.md`,
  `crates/claude_agent_sdk/README.md`.
- Acceptance: documentation only; links resolve.

**5. Act on the studio from the phone panels.**

- Scope: open a host connection with the device's grant for Everglade's
  studio, and offer answer, merge, and steering actions the grant allows.
- Files: `crates/coder-mobile/src/studio_panel.rs`,
  `crates/coder-mobile/src/studio_tests.rs`, and a new studio module in
  `crates/openagents-mobile/src/`.
- Acceptance: Rust tests against a scratch host answer a decision and
  merge a task from the phone panel's intents.

### Wave 2

**6. Run a Claude seat on the Claude Agent SDK. Blocked by issue 3.**

- Scope: a `claude/sdk` engine that runs a seat's turn through
  `crates/claude_agent_sdk` on the owner's Claude Code login, with
  `can_use_tool` mapped to the host's interaction path so each tool
  request outside the worktree becomes an approval with an
  `openagents.coder.approval-step.v1` step; session resume across turns;
  cost from the result message; cancellation that stops the process.
- Files: `crates/microcoder/src/repository.rs`, a new
  `crates/microcoder/src/repository/claude_sdk.rs`,
  `crates/microcoder/Cargo.toml`, `crates/microcoder-loop/src/capacity.rs`
  (the engine's endpoint constant), `crates/coder/src/task/autostart.rs`
  (the `sdk` mapping), and `crates/claude_agent_sdk/` as needed.
- Acceptance: a test with a scripted `claude` binary shows a `Bash` request
  become a studio approval, **Allow once** continue the turn, and **Deny**
  refuse that step; no API key is read.

**7. Make the simulated team interactive on a scratch host. Blocked by
issue 1.**

- Scope: `openagents studio up --sim` starts a scratch host whose studio is
  the scripted team, so Verse and the command line both act on it, with
  no model spend.
- Files: `crates/coder/src/task/studio_sim.rs`,
  `crates/coder/src/task/studio_sim_tests.rs`,
  `crates/openagents-cli/src/studio_up.rs`, `crates/coder-host/src/cli.rs`.
- Acceptance: a test drives the script's question, approval, conflict, and
  merge through `openagents studio` against the simulated host.

### Owner step

The real-engine acceptance run stays an owner step (#10477). After issues 1
and 2, it can run from the command line alone: a scratch host under a
temporary `HOME`, a Codex lead with Claude Code and Codex workers on a
scratch clone, one goal with two independent tasks, one answered question,
one requested change, one merge, a host restart mid-run, and every task
archived at the end.

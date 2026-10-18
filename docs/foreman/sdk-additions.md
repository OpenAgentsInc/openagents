# SDK additions for Foreman and outside harnesses

Foreman today reaches most of what it needs through two side doors: the
desktop engine's app tools (MCP over the engine RPC, local to one computer)
and shell commands (`gh`, `openagents`, `df`, `pgrep`). Neither works for a
phone, the website, a cloud host, or a harness outside the app. This page
lists the additions to `openagents-sdk`, its HTTP routes, and the CLI that
would let any client see and steer agent work.

Everything under **Proposal** is a sketch: names, fields, and routes are
suggestions to settle in review. **Today** cites what exists. Rust sketches
follow the SDK's existing shape: a borrowed accessor on `Client` taking a
`Session`, async methods taking a `CancellationToken`, and request types
with caller-chosen request ids for safe retries (as in
`crates/openagents-sdk/src/agents.rs`).

## Summary

| # | Addition | Today | Gap |
| --- | --- | --- | --- |
| S1 | Run presence and heartbeats | Host session rows (`sessions::publish`); local run records | No progress time; terminal and cloud runs not published |
| S2 | Steer, nudge, or stop any session | `chats::send`/`stop` for account threads; assignment commands; local app tools | No account route to reach a session on another computer |
| S3 | Attention event stream | Foreman's engine-local `Event` enum | Local, managed chats only, no stall events |
| S4 | Landing queue client | `openagents land` (CLI over a GCS queue) | No SDK or HTTP |
| S5 | Work and cloud pool client | `POST /v1/work`, `openagents cloud` | No SDK module |
| S6 | Issue claims and boards | `openagents issue claim`, `issue-board.sh`, `coder-lease` claims | No SDK; claims are local files plus comments |
| S7 | Decision log | `foreman/decisions.jsonl`, engine RPC | Local to one computer |
| S8 | Schedules | `schedules` module (list, sync) | No missed-tick signal; no kind |
| S9 | Machine and capacity facts | `openagents capacity`, leases, `df` | Not published; no route |
| S10 | Owner notifications | `push-gateway` wake constants | No "tell the owner" call |
| S11 | Typed judgments for coordination | `POST /v1/systemone`, `crates/jev` | No question sets for coordination |

## S1. Run presence and heartbeats

**Today.**

- Desktop and phone hosts publish one summary row per session to the
  account: `sessions::publish` → `PUT /v1/history/sessions/computers/{device}`
  with `SessionSummary { status: Idle | Working | AwaitingInput | Errored, … }`
  (`crates/openagents-sdk/src/sessions.rs`). It carries status and last
  activity, not progress.
- Terminal Claude Code and Codex sessions on this repository record a local
  run (`scripts/dev/agent-run.py`, `~/.openagents/runs/<id>/run.json`) with
  `kind`, `host`, `root_pid`, `parent`, and the harness `sessions`. Nothing
  leaves the computer, and there is no last-activity field.
- Coder writes `~/.openagents/activity/<pid>.json` while a turn runs
  ([`docs/coder/guides/activity.md`](../coder/guides/activity.md)).

**Gap.** No client off the computer can tell a live, progressing session
from a live, silent one, and terminal or cloud runs are invisible.

**Proposal.**

```rust
// crates/openagents-sdk/src/runs.rs
pub struct RunHeartbeat {
    pub run: String,                 // stable id, for example "cc-20261017003912-98355"
    pub kind: RunKind,               // AppChat, Terminal, CloudWork, Landing
    pub harness: String,             // "claude-code", "codex", ...
    pub harness_session: Option<String>, // joins claim markers
    pub computer: String,
    pub issues: Vec<u64>,            // what it holds
    pub status: SessionStatus,       // reuse sessions::SessionStatus
    pub last_output_at: i64,         // Unix ms of the last model or tool output
    pub last_progress_at: Option<i64>, // last commit, push, landing step, or test result
    pub note: Option<String>,        // one plain line, under 200 bytes
}

impl Client {
    pub fn runs<'a>(&'a self, session: &'a Session) -> Result<Runs<'a>, Error>;
}
impl Runs<'_> {
    pub async fn heartbeat(&self, beat: &RunHeartbeat, cancel: &CancellationToken) -> Result<(), Error>;
    pub async fn list(&self, filter: RunFilter, cancel: &CancellationToken) -> Result<Vec<RunHeartbeat>, Error>;
    pub async fn end(&self, run: &str, cancel: &CancellationToken) -> Result<(), Error>;
}
```

Routes: `PUT /v1/runs/{id}/heartbeat`, `GET /v1/runs?live=true&issue=N`,
`POST /v1/runs/{id}/end`. A host sends a heartbeat at most once a minute and
on each status change. `agent-run.py` and the desktop engine would both
send them; `openagents run heartbeat` covers shells.

## S2. Steer, nudge, or stop any session

**Today.**

- Account threads: `chats::send` → `POST /v1/threads/{id}/messages` and
  `chats::stop` → `POST /v1/threads/{id}/stop`
  (`crates/openagents-sdk/src/chats.rs`).
- Saved-agent assignments: `POST /v1/agent-assignments/{id}/commands` with
  `AssignmentAction::{FollowUp, Stop, Archive}`
  (`crates/openagents-sdk/src/agents/assignments.rs`). An assignment holds
  at most 128 commands, which is why #11965 didn't use it for check-ins.
- Desktop engine chats: `send_message`, `interrupt_chat`,
  `respond_to_input`, and `create_chat` app tools, reachable only from a
  chat inside the same engine (`apps/openagents/crates/mcp/src/tools.rs`),
  gated by Foreman's standing authority on check-in turns
  (`doc_host/foreman.rs`, `foreman_admit`).
- The device channel between paired devices is NIP-HOST/REACH
  (`crates/coder-host`, `crates/coder-reach`).

**Gap.** Foreman on one computer, or the owner on the phone, can't send a
message to, steer, or stop a session that runs on another computer or in a
terminal. On 2026-10-17 the owner couldn't nudge the coordinator from the
phone.

**Proposal.**

```rust
pub enum SessionCommand {
    Message { text: String },
    Steer { text: String },          // delivered into a running turn
    Stop,
    Restart { brief: Option<String> }, // stop, then start a new turn with this brief
}

impl Runs<'_> {
    pub async fn command(
        &self,
        run: &str,
        request_id: &str,            // idempotent retries
        command: SessionCommand,
        cancel: &CancellationToken,
    ) -> Result<CommandReceipt, Error>;
}
```

Route: `POST /v1/runs/{id}/commands`. The server relays to the run's host
over the existing device channel; the host checks the NIP-HOST grant and,
for Foreman, the standing authority. A terminal run that can't accept input
answers `refused: not_steerable`, which is itself useful: Foreman then knows
to use the stall ladder's release-and-redispatch step.

## S3. Attention event stream

**Today.** Foreman's events are an engine-local enum: `ChatFinished`,
`ChatErrored`, `ChatAwaitingInput`, `IssueOpened`, `IssueClosed`,
`StaleClaim` (`apps/openagents/crates/engine/src/foreman.rs`). They cover
only chats Foreman manages on that computer, and coalesce into a check-in
prompt.

**Gap.** No other client can subscribe, and there are no stall, disk,
landing, or work-run events.

**Proposal.**

```rust
pub enum Attention {
    RunStalled { run: String, since: i64 },
    RunAwaitingInput { run: String, question: String },
    RunErrored { run: String, reason: String },
    ClaimNoProgress { issue: u64, holder: String, since: i64 },
    LandingFailed { entry: String, issue: Option<u64>, reason: String },
    WorkFinished { work: String, issue: u64, outcome: String },
    MachineLow { computer: String, resource: Resource, value: u64 },
}

impl Client {
    pub async fn attention(&self, session: &Session, after: u64, cancel: &CancellationToken)
        -> Result<AttentionStream, Error>;
}
```

Route: `GET /v1/attention?after=N`, server-sent events with a cursor, the
same shape as `GET /v1/work/{id}/events`. Foreman's host turns each event
into a check-in reason; the phone turns the owner-facing ones into a push
(S10).

## S4. Landing queue client

**Today.** `openagents land submit | status | show | withdraw`
(`crates/openagents-cli/src/land.rs`) over a GCS queue
(`gs://openagentsgemini-coder-artifacts/land-queue/openagents`) and the
integrator on `oa-dev-env-1` ([`docs/cloud/land-queue.md`](../cloud/land-queue.md)).

**Gap.** An app chat or a cloud harness can only use it by shelling out to
the CLI with Google credentials. There is no HTTP route.

**Proposal.**

```rust
pub struct LandSubmit {
    pub repo: String,
    pub branch: String,       // already on origin
    pub issue: Option<u64>,   // closes it and moves the board when it lands
    pub summary: Option<String>,
}

impl Land<'_> {
    pub async fn submit(&self, request_id: &str, entry: LandSubmit, cancel: &CancellationToken) -> Result<LandEntry, Error>;
    pub async fn status(&self, cancel: &CancellationToken) -> Result<LandStatus, Error>;
    pub async fn show(&self, id: &str, cancel: &CancellationToken) -> Result<LandEntry, Error>;
    pub async fn withdraw(&self, id: &str, cancel: &CancellationToken) -> Result<(), Error>;
}
```

Routes: `POST /v1/land`, `GET /v1/land`, `GET /v1/land/{id}`,
`DELETE /v1/land/{id}`. The server writes the same queue objects the CLI
writes, so both paths share one queue.

## S5. Work and cloud pool client

**Today.** `POST /v1/work {repo, issue, land, engine}`, `GET /v1/work`,
`GET /v1/work/{id}`, `GET /v1/work/{id}/events`, `POST /v1/work/{id}/cancel`
(`crates/openagents-web/src/work_runs.rs`,
[`docs/cloud/work-on-issues.md`](../cloud/work-on-issues.md)); the doc says
these accept agent-work accounts only. `openagents cloud up | down |
status` manages the pool (`crates/openagents-cli/src/cloud.rs`,
[`docs/cloud/gce-pool.md`](../cloud/gce-pool.md)). The SDK has
`dashboard::Home::work`, which reads `GET /v1/home/work`, but no client for
`/v1/work` or the pool.

**Gap.** Foreman can't dispatch briefed agents or start pool hosts except
through the CLI.

**Proposal.**

```rust
pub struct WorkRequest {
    pub repo: String,
    pub issue: u64,
    pub land: LandMode,             // Queue (default), Pr, None
    pub engine: Option<String>,     // "briefed", "briefed:codex", "bare", "solver"
    pub on: Option<Placement>,      // Gce, Boat, Computer(name)
}

impl Work<'_> {
    pub async fn start(&self, request_id: &str, req: WorkRequest, cancel: &CancellationToken) -> Result<WorkRun, Error>;
    pub async fn list(&self, cancel: &CancellationToken) -> Result<Vec<WorkRun>, Error>;
    pub async fn events(&self, id: &str, after: u64, cancel: &CancellationToken) -> Result<WorkEvents, Error>;
    pub async fn cancel(&self, id: &str, cancel: &CancellationToken) -> Result<(), Error>;
}

impl Pool<'_> {
    pub async fn status(&self, cancel: &CancellationToken) -> Result<PoolStatus, Error>;
    pub async fn up(&self, hosts: u32, cancel: &CancellationToken) -> Result<PoolStatus, Error>; // spends money
    pub async fn down(&self, cancel: &CancellationToken) -> Result<(), Error>;
}
```

`Pool::up` spends money, so for Foreman it stays behind the owner's approval
unless the owner grants a daily budget (a new authority scope).

## S6. Issue claims and boards

**Today.** `openagents issue claim N` / `release N` / `status N` write a
local claim record (`crates/coder-lease/src/claims.rs`, under the lease
root), a GitHub comment with `<!-- openagents-coder-claim cli
session=HARNESS:ID -->`, the assignee, and the board status
([`docs/coder/runtime/leases.md`](../coder/runtime/leases.md), "Issue
claims"). `scripts/dev/issue-board.sh` moves boards 19 and 22. Foreman reads
claims with one GraphQL search (`CLAIMS_QUERY` in `engine/src/foreman.rs`).

**Gap.** A claim's liveness ("its agent process still runs") is only
checkable on the claiming computer, so another computer sees only the
comment and its age. There is no SDK call.

**Proposal.**

```rust
pub struct Claim {
    pub repo: String,
    pub issue: u64,
    pub holder: String,          // "claude-code:9456b274"
    pub run: Option<String>,     // S1 run id, which carries liveness and progress
    pub claimed_at: i64,
    pub last_progress_at: Option<i64>,
}

impl Issues<'_> {
    pub async fn claim(&self, repo: &str, issue: u64, holder: &str, cancel: &CancellationToken) -> Result<Claim, Error>;
    pub async fn release(&self, repo: &str, issue: u64, note: Option<&str>, cancel: &CancellationToken) -> Result<(), Error>;
    pub async fn claims(&self, repo: &str, cancel: &CancellationToken) -> Result<Vec<Claim>, Error>;
    pub async fn board(&self, repo: &str, issue: u64, status: BoardStatus, cancel: &CancellationToken) -> Result<(), Error>;
}
```

Routes: `GET /v1/repos/{owner}/{name}/claims`,
`PUT|DELETE /v1/repos/{owner}/{name}/issues/{n}/claim`,
`PUT /v1/repos/{owner}/{name}/issues/{n}/board`. The server still writes the
GitHub comment, so `gh` readers keep working; the record links the claim to
an S1 run, which answers "alive and progressing" from anywhere.

## S7. Decision log

**Today.** `DecisionEntry::{Wake, Decision, Outcome, Authority}`
(`apps/openagents/crates/proto/src/foreman.rs`), appended to
`foreman/decisions.jsonl` beside the profile's `agent-lab/` folder and read
by the settings card over the engine RPC. Foreman writes through the
`record_decision` app tool.

**Gap.** Local to one computer; the phone and the website can't read it,
and a replacement host starts with no memory.

**Proposal.**

```rust
impl Agents<'_> {
    pub async fn record_decision(&self, agent: &str, request_id: &str, entry: &DecisionEntry, cancel: &CancellationToken) -> Result<(), Error>;
    pub async fn decisions(&self, agent: &str, after: Option<&str>, cancel: &CancellationToken) -> Result<DecisionPage, Error>;
}
```

Routes: `POST /v1/agent-definitions/{id}/decisions`,
`GET /v1/agent-definitions/{id}/decisions?after=CURSOR`. Append-only; the
host mirrors its local file and keeps the file as its cache.

## S8. Schedules

**Today.** `Schedules::list` and `Schedules::sync` over `GET /v1/schedules`
and `POST /v1/schedules/sync` (`crates/openagents-sdk/src/schedules.rs`).
"Each computer runs its own entries, so no server clock starts work." The
server keeps one list per account (`crates/openagents-web/src/account_schedules.rs`).

**Gap.** When the computer is off, or the host is stuck, the schedule is
silent and nobody is told. Foreman's schedule is recognizable only by its
prompt text.

**Proposal.**

- Add `kind: Option<String>` (for example `"foreman_check_in"`) and
  `last_ran_at: Option<i64>` to `Schedule`, written by the host on each tick.
- `GET /v1/schedules/{id}/health` answers `on_time | late { missed: u32 } |
  never_ran`, and two missed intervals raise an S3 attention event.

## S9. Machine and capacity facts

**Today.** The capacity book (`openagents capacity`, `check`, `record`,
[`docs/coder/runtime/capacity.md`](../coder/runtime/capacity.md)), the
host broker's build and disk leases (`openagents lease`,
[`docs/coder/runtime/leases.md`](../coder/runtime/leases.md)), the build
sweep's last run (`~/.openagents/logs/build-sweep.last.json`), and `df`.
Foreman reached all of these through shell commands it wrote itself.

**Gap.** No one tool returns them, so each agent writes its own probe, and
on 2026-10-17 two of them were wrong (L10 in
[lessons-learned.md](lessons-learned.md)).

**Proposal.** A `machine_status` app tool and a field on host presence:

```rust
pub struct MachineFacts {
    pub computer: String,
    pub disk_free_gib: u64,
    pub build_leases: (u32, u32),     // held, limit
    pub cargo_jobs: u32,              // `pgrep -x cargo`
    pub sweep_last_run: Option<i64>,
    pub capacity: Vec<(String, CapacityState)>, // provider → capacity | limited { reset }
}
```

Published with the S1 heartbeat of the computer's host, at most once a
minute.

## S10. Owner notifications

**Today.** `crates/push-gateway` sends only registered wake constants to
APNs and FCM, on relay-presented delivery grants
([`docs/deployment/push-gateway.md`](../deployment/push-gateway.md)).

**Gap.** Foreman has no way to say "tell the owner now".

**Proposal.** A new wake constant `attention` and one call:

```rust
impl Client {
    pub async fn notify_owner(&self, session: &Session, request_id: &str, note: OwnerNote, cancel: &CancellationToken) -> Result<(), Error>;
}
pub struct OwnerNote { pub chat: String, pub title: String } // plain words, under 120 bytes
```

Route: `POST /v1/notify`. Rate-limited per agent (for example 6 an hour), so
a confused agent can't flood the phone. The push carries only the constant;
the phone fetches the note.

## S11. Typed judgments for coordination

**Today.** `POST /v1/systemone` and the Rust SDK in `crates/jev`; the caller
contract is `.agents/skills/decision-api/SKILL.md`.

**Gap.** No question sets for the judgments Foreman makes every check-in.

**Proposal.** Question sets in `questions/`, each with a baseline written
under `docs/decision-models/measurements/` before use:

- `foreman-progress`: over a run's last outputs, commits, and build events,
  "has this run made progress since the last check-in?"
- `foreman-failure-on-main`: over a branch's failing tests and `main`'s,
  "is each failure also on `main`?"
- `foreman-overlap`: over two entries' changed paths and packages, "do
  these overlap?"
- `foreman-ready-to-close`: over an issue's acceptance text and its landed
  commits, "is the code complete?"

Foreman records each judgment's receipt with the decision that used it.

## Order

S1, S2, and S3 come first: they turn "outside the app" from a dead end into
a run that Foreman can read and steer. S5 and S4 next, so Foreman dispatches
and lands without shelling out. S6, S7, and S9 make the state shared. S8,
S10, and S11 follow the roadmap's Phase 2.

# Programmatic Autopilot Work Runbook Audit

Date: 2026-06-04

Scope: this document records the experiment where a Codex coding-agent session
was asked to administer an Autopilot goal/run instead of directly implementing
the requested Gemini / Imagen ImageGen work itself. The aim is to separate the
mechanics of programmatically kicking off and supervising Autopilot work from
the product implementation work that Autopilot performs inside its own runner
workspace.

Canonical launch/runbook packet: use
`docs/autopilot-tasks/2026-06-04-programmatic-autopilot-operator-runbook.md`
for current Autopilot task dispatch mechanics. This audit remains the
historical background document.

## Intended Operating Model

The intended end state is that a foreground coding agent can act as an
Autopilot operator. It should translate a user request into a durable
OpenAgents goal, launch the right Autopilot runner against the right repository
and project scope, monitor progress through Cloudflare-backed state and SHC
callbacks, fix only the Autopilot infrastructure when the run cannot proceed,
and leave the product implementation itself to the Autopilot run.

This is intentionally different from normal Codex implementation work. In the
normal path, the foreground coding agent edits the repo, runs tests, commits,
pushes, and deploys the requested feature. In the Autopilot-admin path, the
foreground coding agent mostly administers goal state, runner dispatch,
provider account health, callback ingestion, sync visibility, and recovery
from infrastructure failures. Direct code changes should be limited to defects
that prevent Autopilot from receiving, running, reporting, resuming, or showing
the work.

## What Happened In This Experiment

The user asked for Gemini / Imagen ImageGen support to be built through
Autopilot. The foreground agent created a Gemini implementation spec in
`docs/gemini.md`, provisioned the needed production D1 rows for a team project
and agent, created a durable project goal, and launched an SHC-backed
Autopilot run.

The first launch failed because the ChatGPT/Codex provider account token had
been invalidated by OpenAI. The system correctly recorded a failure, but the
user experience was too reactive: the user saw a failed run transcript rather
than a clearer preflight message that the account needed reconnection before a
run should be dispatched. After the provider account was reconnected, a second
Autopilot run launched and began implementing ImageGen.

The second run exposed several Autopilot infrastructure issues that were
unrelated to the ImageGen feature:

- Production D1 migrations for goals were not applied before use.
- The SHC control daemon needed a restart before its control API responded
  reliably.
- SHC callback batches used job-event envelopes that the Worker callback
  normalizer did not accept.
- SHC control events sometimes had no top-level sequence or timestamp, relying
  on the receiver's fallback sequence and current time.
- A shared `/t/<run>` route showed the viewer's name instead of the run owner's
  name.
- Team-visible runs were accessible by route but did not stay visible in a
  teammate's sidebar because mission sync was owner-workspace-only.

The foreground agent had to fix those infrastructure issues before the
Autopilot run could be trusted as a durable execution path. Those fixes are
administration and platform work, not Gemini implementation work.

## What Was Actual Product Implementation

Actual ImageGen implementation work belongs to the Autopilot run. The SHC
workspace showed ImageGen-related files such as `docs/image-generation.md` and
app code under the logged-in image surface. That is the work Autopilot should
own.

The foreground coding agent should not manually finish that implementation
unless the user explicitly switches back to direct Codex implementation or the
Autopilot path is blocked in a way that cannot be repaired without changing the
product code. In this experiment, the foreground agent's direct commits were
for Autopilot platform defects: callback ingestion, timestamp/runtime
normalization, shared-thread attribution, and team mission sync.

## Programmatic Runbook

### 1. Confirm The Work Should Be Autopilot-Owned

Before launching anything, classify the user's request:

- Use Autopilot when the user wants a durable agent to pursue an objective,
  especially when the point is to stress-test long-running agents.
- Use direct foreground implementation when the user asks for immediate code
  edits and does not ask to exercise Autopilot.
- Use direct foreground implementation only for Autopilot infrastructure
  defects that stop the run from proceeding or reporting honestly.

The operator should write down the distinction in the run notes: "Autopilot is
responsible for feature X; foreground agent is responsible for goal/run
administration and platform repair."

### 2. Prepare The Repository Context

Make sure the Autopilot runner can see the instructions it needs:

- Put implementation specs in tracked repo files, not only in the chat
  transcript.
- Commit and push those specs before launching the run.
- Keep the spec focused enough that the runner can execute without needing
  hidden chat context.
- Avoid including secrets or local-only state in the spec.

In this experiment, `docs/gemini.md` was committed first so the SHC runner
could read the exact Gemini / Imagen implementation plan from the remote clone.

### 3. Preflight Production State

Before creating a goal or run, validate production state:

- Required D1 migrations are applied to production.
- The team, project, and agent records exist.
- The provider account is connected and healthy.
- The SHC control API health endpoint responds.
- The callback ingest URL and callback auth token are configured.
- The target repo, branch policy, write connection, and delivery contract are
  valid.

The current system did not enforce all of these as a single preflight. That is
why the first run failed on provider token invalidation and the second run
required manual SHC control recovery.

### 4. Create Or Reuse The Durable Goal

The durable goal should be the product objective, not an operational note.
It should include:

- The objective.
- The agent and team/project scope.
- Visibility.
- A bounded token budget or explicit reason no token budget is set.
- The current run ID once dispatch starts.

The foreground agent should avoid creating duplicate goals for retries. If the
first run fails due to infrastructure, create a continuation or follow-up run
against the same goal when possible.

### 5. Dispatch The Run

Dispatch should use the programmatic API path, not a one-off manual shell
recipe. The request should include:

- Repository and base ref.
- Runner ID and runtime.
- Provider account reference.
- Goal context.
- Required artifacts.
- GitHub writeback settings.
- Callback URL and callback token reference.

The runner prompt should make the product implementation objective clear while
keeping private delivery mechanics out of user-visible messages.

### 6. Monitor Through Cloudflare State First

The operator should use Cloudflare-backed run state as the source of truth:

- Run status.
- Event cursor.
- Event count.
- Goal status and token usage.
- Provider reconnect requirement.
- Team-visible mission projection.

SHC process inspection is a fallback for diagnosing runner/control failures,
not the primary product status surface.

### 7. Recover Infrastructure Failures

If the run fails or stalls, classify the failure:

- Provider auth failure: show reconnect-required state and do not keep
  dispatching doomed runs.
- SHC control failure: restart or repair control service, then continue the
  same run/goal where possible.
- Callback ingestion failure: fix Worker normalizer or SHC payload shape, then
  backfill or retry pending callbacks.
- Sync visibility failure: fix Cloudflare sync scopes and projections.
- Product implementation failure: leave it to the Autopilot run unless it is
  blocked by missing instructions or unavailable credentials.

This experiment found that the callback receiver must accept both
`openagents.runner_event.v1` event payloads and SHC job-event envelopes. The
receiver also needs fallback sequencing and timestamping for sparse control
events.

### 8. Resume Or Continue The Run

After fixing infrastructure, continue the same goal:

- If the run is still running, queue a follow-up turn through the SHC control
  API instead of launching a new unrelated run.
- If the run failed due to provider auth or callback ingest, reconnect/fix,
  then launch a new run attached to the same goal.
- If pending callback files exist, retry ingestion so Cloudflare catches up.
- Preserve the same public/team thread where possible so observers see a
  continuous narrative.

The current SHC control service supports follow-up turns for running jobs. If a
job is `running`, a follow-up request moves it to `waiting_for_input` and queues
the turn rather than spawning a second worker process.

### 9. Close Out

When the Autopilot run completes, the operator should verify:

- Required artifacts exist.
- GitHub writeback happened.
- Commits or PRs are visible.
- Tests and deploy receipts are attached to the run.
- Adjutant Site runs also have usage receipts for generation, build, hosting,
  storage, and adjustment categories, with public beta rows charged at `$0.00`.
- Goal accounting is finalized.
- Public/team thread attribution and sidebar visibility are correct.

The foreground agent should summarize the Autopilot outcome and any platform
repairs separately.

## Current Blockers To Repeatable Autopilot Work

### Missing End-To-End Preflight

There should be a single API-visible preflight that verifies D1 migrations,
team/project/agent rows, provider-account health, SHC control health, callback
auth, and GitHub writeback readiness. Without this, the operator discovers
failures after dispatch.

### Provider Reconnect UX Is Too Late

The first failed run correctly identified token invalidation, but the user saw
it as a failed work transcript. The better behavior is to detect provider
invalidity before dispatch and show "Reconnect ChatGPT/Codex to run Autopilot"
as a first-class run-preflight state.

### Callback Payload Contracts Are Not Unified

The Worker originally accepted one callback dialect but rejected SHC job-event
batches. Future agents need one documented callback schema, plus compatibility
tests for:

- Top-level runner events.
- SHC job-event envelopes.
- Sparse control events with fallback sequence and timestamp.
- Tool calls emitted separately from events.
- Terminal status updates.

### Team Visibility Was Split Across Route Access And Sync

Route access allowed teammates to open `/t/<run>`, but sync projection did not
publish team-owned mission rows into the team scope. This caused the thread to
appear briefly and then disappear from the sidebar. Access control and sync
projection need to be treated as one product contract.

### Attribution Was Viewer-Derived

The shared thread rendered the goal message with `model.session.name`, so Ben
saw Ben's name on Christopher's run. Shared run pages must render run-owner
identity from the run record and team membership data, not from the current
viewer.

### Manual SHC Recovery Is Still Too Common

The operator had to inspect SHC processes and restart `oa-codex-control`. That
should become automated health monitoring with a clear recovery action, or at
least a documented operator command that records the recovery into the run
events.

### Callback Backfill Is Ad Hoc

Pending SHC callbacks were retried manually by reconstructing request bodies
and using the callback token. The control service should expose a safe
operator endpoint for "retry pending callbacks for run X" and the Worker should
surface why a batch failed.

### Goal Continuation Needs A Cleaner Public Contract

The current system can launch and continue runs, but the UI/API should make the
goal/run relationship explicit: current run, previous failed run, continuation
run, budget, and public/team visibility. Otherwise a user sees multiple
threads and cannot tell which run is the live continuation.

## Recommended Follow-Up Issues

1. Add `POST /api/operator/autopilot/preflight` for programmatic run readiness.
   It should verify migrations, agent/project presence, provider health, SHC
   health, callback config, and GitHub writeback config.

2. Add provider reconnect gating before dispatch. The launch API should return
   a typed reconnect-required response instead of creating a doomed failed run
   when the provider account is invalid.

3. Publish a canonical SHC callback contract and keep Worker tests for every
   supported payload dialect.

4. Add an authenticated operator endpoint to retry pending SHC callbacks by
   run ID without reconstructing callback bodies manually.

5. Add a run-continuation API that attaches new runs or follow-up turns to the
   same durable goal and records the continuation relationship.

6. Add a public/team goal observer page that shows objective, current run,
   status, budget, and live stream without leaking private delivery mechanics.

7. Add an Autopilot operator checklist command for foreground coding agents.
   It should print preflight state, current run, callback lag, provider health,
   SHC health, and next safe action.

8. Add tests that prove team-visible `/t/<run>` pages use run-owner attribution
   and maintain sidebar visibility for non-owner team members.

## 2026-06-04 Implementation Pass

The recommendations above are now represented by concrete OpenAgents product surface surfaces for
foreground coding agents:

- `GET` or `POST /api/operator/autopilot/preflight`
- `GET` or `POST /api/omni/operator/autopilot/preflight`
- `GET` or `POST /api/omni/operator/autopilot/checklist`
- `GET` or `POST /api/omni/operator/agent-runs/:runId/checklist`
- `POST /api/omni/operator/agent-runs/:runId/callbacks/retry`
- `POST /api/omni/operator/agent-runs/:runId/continue`
- `scripts/autopilot-operator-checklist.mjs`

All operator endpoints require the admin API bearer token. The checklist is the
recommended foreground-agent entrypoint because it combines preflight readiness
with the current run, callback lag, and the next safe action. Example:

```sh
OPENAGENTS_ADMIN_API_TOKEN=... \
  node scripts/autopilot-operator-checklist.mjs \
  --email chris@openagents.com \
  --teamId team_openagents_core \
  --projectId project_imagegen_support \
  --runId 11a4ff12-601b-48f3-b596-34f947bfc4bb
```

The preflight response reports these checks:

- `database_migrations`
- `team_project_agent`
- `provider_account`
- `github_write`
- `shc_control`
- `runner_callback`
- `runner_backends`
- `callback_lag` when the checklist path or `includeCallbackLag` is used

The provider launch gate now returns a typed
`provider_reconnect_required` response with `requiresReconnect: true` instead
of letting the operator create a doomed run when no connected healthy
ChatGPT/Codex provider account is available.

The callback retry endpoint asks SHC for events after the Cloudflare
`agent_runs.event_cursor`, decodes them through the Worker runner-event
normalizer, writes them to D1, updates goal runtime accounting, and publishes
the same sync scopes as normal callback ingestion. Operators no longer need to
reconstruct callback JSON or use the runner callback token manually.

The run detail projection now separates runner state from callback delivery
state. `operationalState.runner.status` is the product runner status.
`operationalState.callbackDelivery.status=failed` means callback delivery or
post-ingest accounting degraded and should be retried or inspected; it should
not mask a completed retained SHC run as `runner_failed`.

The continuation endpoint uses two modes. If the run is `queued`, `running`, or
`waiting_for_input`, it sends a follow-up turn to SHC through
`/v1/codex-runs/:runId/turns` or the compatible control action route. If the
run has stopped and is attached to an active durable goal, it requests a
policy-gated goal continuation so the next run remains attached to the same
goal.

The public/team observer recommendation is partially implemented through the
existing goal and run surfaces rather than a new standalone page:

- public goal snapshots: `/api/public/agents/:agentId/current-goal`,
  `/api/public/goals/:goalId`, and `/api/public/goals/:goalId/snapshot`;
- team-visible live run stream: `/t/:runId` and
  `/api/omni/agent-runs/:runId/events`;
- sidebar visibility for team-owned runs is backed by team-scope mission sync;
- shared run attribution is owner-derived from the run/team record, not the
  viewer session.

The remaining product-level choice is whether to add a polished browser page
that stitches the public goal snapshot and the live `/t/:runId` stream into one
explicit "goal observer" view. The backend contracts needed by future coding
agents are now present.

## Canonical SHC Callback Contract

The Worker callback receiver accepts these dialects:

1. Canonical `openagents.runner_event.v1` payloads with `sequence`, `source`,
   `type`, `summary`, optional `status`, optional `artifactRefs`, and
   millisecond or ISO timestamps.
2. SHC job-event envelopes with top-level `sequence`, `source`, `type`,
   `summary`, `createdAtMs`, and optional `dataJson`.
3. Sparse SHC control events that omit sequence or timestamp; Cloudflare
   supplies the fallback sequence and current timestamp.
4. Tool/status events where richer details are nested under `dataJson`.
5. Redacted runner events. If callback payloads contain credential-shaped
   material such as OAuth tokens, the normalizer persists only safe event
   metadata and drops the sensitive body.
6. Artanis bootstrap callback batches containing safe
   `artanis.settlement_intent.attached`, `runner.auth_grant_resolved`,
   `runner.failed`, and `runner.cleanup` events.

Regression coverage lives in `workers/api/src/omni-services.test.ts` and
`workers/api/src/omni-runs.test.ts`.

## Future Coding-Agent Session Contract

A future foreground coding agent should follow this discipline:

1. Write or locate a tracked spec for the requested work.
2. Commit and push the spec.
3. Run the Autopilot preflight.
4. Create or update the durable goal.
5. Launch the run or queue a continuation.
6. Watch Cloudflare run/goal state.
7. Repair only Autopilot infrastructure defects.
8. Avoid manually implementing the product feature unless explicitly asked.
9. Commit, push, and deploy Autopilot platform fixes.
10. Continue monitoring until the Autopilot run has a real completion,
    handoff, or typed blocker.

That contract is the point of this experiment: the foreground agent becomes an
operator for Autopilot, and Autopilot becomes the default implementation engine
for durable coding work.

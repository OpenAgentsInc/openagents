# Codex Control API

Status: SHC async MVP control surface

`oa-codex-control` is the narrow HTTP control API that lets Vortex start the
existing `oa-workroomd codex run` path on a managed VM without SSH. The primary
`/v1/codex-runs` path is asynchronous: it persists a local job record, returns
`202 Accepted`, then runs Codex in a background worker while status/events are
recoverable from the control daemon and optionally mirrored back to Vortex.

## Endpoints

```text
GET /healthz
POST /v1/codex-runs
POST /v1/codex-runs/start
POST /v1/queue
POST /v1/queue/start
POST /v1/placement
POST /v1/placement/start
POST /v1/training-runs
POST /v1/training-runs/start
POST /v1/artanis/bootstrap
POST /v1/artanis/bootstrap/start
GET /v1/codex-runs/{runId}
GET /v1/codex-runs/{runId}/events?cursor=0
GET /v1/codex-runs/{runId}/stream?cursor=0
POST /v1/codex-runs/{runId}/turns
POST /v1/codex-runs/{runId}/cancel
POST /v1/codex-runs/continue
POST /v1/codex-runs/steer
POST /v1/codex-runs/cancel
POST /v1/workrooms/codex/start
```

Requests must include:

```text
Authorization: Bearer <OA_CODEX_CONTROL_TOKEN>
Content-Type: application/json
```

The POST body is the Vortex Codex VM envelope:

```json
{
  "authGrantRef": "codex-auth-grant_...",
  "backend": "gcp_vm_codex",
  "goal": "Create the requested artifact.",
  "providerAccountRef": "provider-account_...",
  "repository": "OpenAgentsInc/vortex",
  "runnerId": "oa-shc-katy-01",
  "runId": "workroom_codex_vm_0",
  "sandboxMode": "danger_full_access",
  "timeoutMs": 300000,
  "walletAuthority": false
}
```

For retained Terminal-Bench/training runs, Vortex should post
`openagents.training_run_assignment.v1` to `/v1/training-runs/start` instead.
That path validates dataset/task/package/signature/artifact fields, writes the
assignment into the local job directory, emits `training.assignment.validated`,
`benchmark.package.validated`, `training.artifact_policy.attached`, and
optional `signature.context.loaded` events, then runs the existing account-
backed Codex workroom path. The first implementation accepts exactly one
variant per request and only supports `terminal-bench` task refs.

See `docs/contracts/openagents.training_run_assignment.v1.md`.

For Artanis/Pylon launch bootstrap runs, Vortex or an approved operator should
post `openagents.artanis_bootstrap_assignment.v1` to
`/v1/artanis/bootstrap/start`. That path validates sanitized Artanis source
refs, Pylon capability labels, Blueprint signature ids, artifact policy, and
no-wallet SHC execution boundaries, then runs the existing account-backed Codex
workroom path. It persists `artanis-bootstrap-assignment.json` in the job
directory and emits `artanis.bootstrap.validated`,
`artanis.capability_context.loaded`, and `artanis.artifact_policy.attached`
events before the Codex worker starts.

See `docs/contracts/openagents.artanis_bootstrap_assignment.v1.md`.

The async response is the Vortex runner acknowledgement shape:

```json
{
  "externalRunId": "shc-codex:oa-shc-katy-01:workroom_codex_vm_0",
  "status": "queued",
  "events": [
    {
      "kind": "queued",
      "summary": "Codex run queued on SHC control daemon.",
      "artifactRefs": [],
      "receiptRefs": [],
      "redacted": false
    }
  ]
}
```

## Durable Unattended Queue (`/v1/queue`, cloud#97)

`POST /v1/queue` (or `/v1/queue/start`) accepts the same Codex VM envelope as
`/v1/codex-runs` but does **not** start a worker inline. It persists the job in
the local registry with `status: "queued"` and a durable `queue.pending` marker,
then returns `202`. An internal tick worker drains pending jobs with **no
external driver**, so coding work proceeds while the owner is offline.

```text
$OA_CODEX_CONTROL_STATE_ROOT/jobs/<safe-run-id>/queue.pending
```

The tick worker (enabled with `OA_CODEX_QUEUE_ENABLED=true`):

- scans `jobs/*/queue.pending` oldest-first each `OA_CODEX_QUEUE_TICK_MS`;
- dispatches up to `OA_CODEX_QUEUE_MAX_CONCURRENCY` jobs at a time (bounded
  concurrency, tracked by an in-flight counter);
- applies `OA_CODEX_QUEUE_LANE` (default `cloud-gcp`) to a dequeued job that did
  not pin a `lane`, so the run drives the configured lane unattended;
- claims a job by removing its durable marker **before** spawning the worker, so
  a daemon restart never double-dispatches it;
- emits `cloud.run.enqueued` on enqueue and `cloud.run.dequeued` on dispatch.

Because the `queue.pending` markers live on disk under the job registry, a
restarted daemon re-scans them and resumes draining. A job that was already
mid-run when the daemon was killed keeps its last durable status (the existing
no-process-reattach behavior); only jobs still marked pending are re-dispatched.

`/v1/codex-runs` remains the inline path that spawns a worker immediately;
`/v1/queue` is the enqueue surface for fully unattended draining.

## Lane-Agnostic Placement (`/v1/placement`)

`POST /v1/placement` (or `/v1/placement/start`) accepts a Vortex-independent,
lane-agnostic coding-run assignment and binds it to a concrete runner per
`openagents.compute_quota_routing.v1`. This is the coordinator/placement layer
a generic control front door (Pylon) can call without being a Vortex-shaped
caller (cloud#86).

Request body (`openagents.codex_placement_assignment.v1`):

```json
{
  "contract_version": "openagents.codex_placement_assignment.v1",
  "run_id": "agent_run_42",
  "owner_ref": "owner://sha256/...",
  "provider_account_ref": "provider-account_...",
  "auth_grant_ref": "codex-auth-grant_...",
  "goal": "Create the requested artifact.",
  "lane": "auto",
  "repository": "OpenAgentsInc/openagents",
  "sandbox_mode": "danger_full_access",
  "wallet_authority": false,
  "created_at_ms": 0
}
```

`lane` is one of `auto`, `local`, `cloud-gcp`, `cloud-shc` and defaults to
`auto`. Policy (owner direction 2026-06-14): **Google GCE is primary, SHC is
secondary/fallback.** `auto` and `cloud-gcp` bind to the GCE ephemeral-per-
session VM lane (capacity class `gce.ephemeral.standard.v1`, the commercial
C-5 class) when GCE is selectable; otherwise they fall back to SHC. `cloud-shc`
pins `oa-shc-katy-01`. `local` is resolved by the caller's own Pylon, not by
cloud placement, and is rejected here. The GCE lane uses `danger_full_access`
by default inside the no-wallet VM boundary (CND-041/CND-055, cloud#88).

Until CND-042 receipt comparison lands, placement is **policy-driven (Google
default), not cost-driven**; the returned binding records `costDriven: false`.

The response carries the runner binding plus the same async run acknowledgement
that `/v1/codex-runs/start` returns (the placed run executes through the
existing async Codex run path):

```json
{
  "binding": {
    "contractVersion": "openagents.codex_placement_assignment.v1",
    "runId": "agent_run_42",
    "externalRunId": "shc-codex:oa-gce-ephemeral-agent_run_42:agent_run_42",
    "lane": "cloud-gcp",
    "providerLane": "gcp",
    "runnerId": "oa-gce-ephemeral-agent_run_42",
    "capacityClassId": "gce.ephemeral.standard.v1",
    "sandboxMode": "danger_full_access",
    "reason": "policy_default_gce",
    "costDriven": false,
    "caps": { "sessionTtlMs": 28800000, "idleTimeoutMs": 1800000, "...": "..." }
  },
  "externalRunId": "shc-codex:oa-gce-ephemeral-agent_run_42:agent_run_42",
  "status": "queued",
  "events": [{ "kind": "placement.bound", "...": "..." }]
}
```

Full GCE warm-pool optimization is deferred to the density phase; placement
just selects GCE by default and keeps cold-start reasonable. The runner binding
and quota caps are refs-and-limits only and carry no raw owner identity, cost,
GCP project id, instance name, IP, credentials, or topology.

A `lane` (and optional `owner_ref`) field may also be set directly on a
`/v1/codex-runs` request body when a caller wants placement policy applied to a
run assignment instead of trusting a caller-supplied `runner_id`.

`POST /v1/workrooms/codex/start` remains the compatibility path for the old
blocking run-to-completion behavior. Vortex should prefer
`POST /v1/codex-runs/start`, which is the action-expanded form produced when
`VORTEX_CODEX_VM_CONTROL_URL` points at `/v1/codex-runs`.

## Local Job Registry

`OA_CODEX_CONTROL_STATE_ROOT` now contains a durable local registry:

```text
$OA_CODEX_CONTROL_STATE_ROOT/jobs/<safe-run-id>/job.json
$OA_CODEX_CONTROL_STATE_ROOT/jobs/<safe-run-id>/events.jsonl
$OA_CODEX_CONTROL_STATE_ROOT/jobs/<safe-run-id>/cancel.requested
$OA_CODEX_CONTROL_STATE_ROOT/jobs/<safe-run-id>/callbacks/<sequence>.sent
```

`job.json` stores the run envelope, status, last local event sequence, cancel
flag, and external runner id. `events.jsonl` stores local normalized events.
This lets a restarted daemon answer status/event queries from disk even if the
active process is gone. The current implementation does not resume a killed
Codex process; a restarted daemon can honestly report the last durable local
state and future work should add process reattach or explicit stale-run
recovery.

`job.json` writes are committed through a temp file plus rename so local status
readers and the live callback mirror do not observe partial JSON during worker
updates.

When `oa-workroomd` returns `runner_events`, `oa-codex-control` preserves their
typed event names in its local `events.jsonl` and Vortex callbacks. That is the
preferred product event channel for messages, shell commands, tool calls,
artifacts, receipts, usage availability, and terminal state. The older
`codex-run-events.jsonl` compatibility events are still consumed as a fallback.
For async runs, `oa-codex-control` also mirrors
`openagents-runner-events.jsonl` from the active workroom state directory while
`oa-workroomd codex run` is still running. New runner events are appended to
the local job registry and posted through the Vortex callback path before final
closeout, so Vortex can show live tool and shell activity instead of waiting
for the blocking workroom process to exit.

Vortex callback delivery is best-effort relative to local workroom execution.
Callback failures must be logged and retried from local state, but they must
not prevent `oa-workroomd` from starting or closing the Codex workroom. A
production failure on 2026-06-02 showed why: a Vortex ingest 502 after
`cloud.run.started` left the local job stuck in `running` before the workroom
started. The daemon now treats callback delivery as an observability/export
path, not a precondition for running the job.
Current runner event types include `run.queued`, `run.started`,
`run.heartbeat`, `turn.started`, `message.delta`, `message.completed`,
`tool.call.started`, `tool.call.delta`, `tool.call.completed`,
`shell.command.started`, `shell.output.delta`, `shell.command.completed`,
`file.edit`, `artifact.created`, `receipt.created`,
`turn.completed`, `ThreadTokenUsageUpdated`, `opencode.step-finish`,
`opencode.session.next.step.ended`, `resource.usage.captured`,
`usage.unavailable`, `run.waiting_for_input`, `run.failed`, `run.timed_out`,
`run.cancelled`, and `run.completed`. For subscription-backed Codex,
`oa-workroomd` forwards SDK `turn.completed.usage` or app-server
`thread/tokenUsage/updated` / `ThreadTokenUsageUpdated` payloads when Codex
exposes them. For OpenCode-backed models, it forwards `step-finish` /
`session.next.step.ended` token payloads, including provider/model labels and
cache read/write token counts in the raw event payload. Only the true
no-usage-observed path emits `usage.unavailable`, and that event must cite the
same `openagents.resource_usage_receipt.v1` digest as
`resource.usage.captured`.

`GET /v1/codex-runs/{runId}` returns the current local status and normalized
event summary. `GET /events?cursor=N` returns only local events after that
cursor. `GET /stream?cursor=N` returns a one-shot SSE snapshot plus heartbeat;
it is meant for internal polling/recovery, not public browser exposure.

`POST /v1/codex-runs/{runId}/cancel` and `POST /v1/codex-runs/cancel` mark the
job canceled and persist a cancel marker. If the current Codex turn is already
inside the blocking `oa-workroomd codex run` process, the first supervisor
version records the cancellation and suppresses a later success when the
worker returns. A later process-supervisor pass should track child PIDs and
terminate the active turn immediately.

`POST /v1/codex-runs/{runId}/turns` and `POST /v1/codex-runs/continue` append a
continuation request. If the run is not currently inside an active worker, the
daemon starts another background Codex turn from the stored run envelope with
the new prompt and optional auth grant. If the worker is already running, the
daemon records the continuation as `waiting_for_input`; the next supervisor
pass should attach the turn to a live Codex thread/process rather than starting
a separate command.

## Runtime Env

```text
OA_CODEX_CONTROL_BIND=0.0.0.0:8787
OA_CODEX_CONTROL_TOKEN=<server-to-server bearer token>
OA_CODEX_AUTH_JSON_ROOT=/home/ubuntu/.openagents-codex-accounts
OA_CODEX_CONTROL_STATE_ROOT=/var/lib/openagents/codex-control
OA_WORKROOMD_BIN=/home/ubuntu/openagents-cloud/target/release/oa-workroomd
OA_CODEX_BIN=/usr/local/bin/codex
OA_OPENCODE_BIN=/usr/local/bin/opencode
OA_OPENCODE_CODEX_MODEL=openai/gpt-5.5

# Neutral (preferred), Vortex-codebase-independent grant/ingest config (cloud#87).
# Each neutral var falls back to its legacy OA_VORTEX_* equivalent when unset,
# so existing deployments keep working. The Vortex credential/endpoint may be
# reused as the URL; the daemon no longer depends on the deprecated Vortex code.
OA_CODEX_GRANT_RESOLVE_URL=https://openagents.com/api/provider-accounts/chatgpt-codex/grants/resolve
OA_CODEX_RUNNER_GRANT_TOKEN=<grant resolver bearer token>
OA_CODEX_EVENT_INGEST_URL=https://openagents.com/api/workrooms/codex-runs
OA_CODEX_EVENT_INGEST_TOKEN=<optional; defaults to OA_CODEX_CONTROL_TOKEN>

# Placement policy (cloud#86/#88). GCE primary; SHC secondary.
OA_CODEX_PLACEMENT_GCE_AVAILABLE=true   # set false to force SHC
OA_CODEX_PLACEMENT_SHC_RUNNER_ID=oa-shc-katy-01

# Git writeback fallback token (cloud#96). Used by oa-workroomd to commit and
# push a coding run's workspace changes back to the target repo/branch before
# teardown when no per-run github_write_grant_ref is supplied. Process-env only;
# never logged, never embedded in commits/remotes. Preferred path is a
# run-scoped GitHub write grant; this is the operator fallback.
OA_CODEX_GITHUB_TOKEN=<github write token>

# Durable unattended queue (cloud#97). When enabled, the daemon runs an internal
# tick worker that drains queued coding jobs on the configured lane with no
# external driver. Queue state persists under the job registry, so a restart
# resumes draining.
OA_CODEX_QUEUE_ENABLED=true             # default false
OA_CODEX_QUEUE_LANE=cloud-gcp          # default lane for dequeued jobs
OA_CODEX_QUEUE_MAX_CONCURRENCY=1       # bounded in-flight jobs (default 1)
OA_CODEX_QUEUE_TICK_MS=2000            # worker poll interval (default 2000)

# Legacy aliases (still honored as fallback; do not add new dependencies on
# the deprecated Vortex codebase):
OA_VORTEX_GRANT_RESOLVE_URL=<- OA_CODEX_GRANT_RESOLVE_URL
OA_VORTEX_CLOUD_RUNNER_GRANT_TOKEN=<- OA_CODEX_RUNNER_GRANT_TOKEN
OA_VORTEX_CODEX_INGEST_URL=<- OA_CODEX_EVENT_INGEST_URL
OA_VORTEX_CODEX_INGEST_TOKEN=<- OA_CODEX_EVENT_INGEST_TOKEN
```

Env var resolution prefers the neutral `OA_CODEX_*` names and falls back to the
legacy `OA_VORTEX_*` names only when the neutral var is unset. A generic
Pylon-originated coding-run assignment resolves Codex/ChatGPT grants through the
neutral `OA_CODEX_GRANT_RESOLVE_URL` endpoint and posts
`openagents.codex_workroom_event.v1` callbacks to the neutral
`OA_CODEX_EVENT_INGEST_URL`.

`agentRuntime` on a control request selects the execution binary. Missing
values default to `opencode_codex`, which runs OpenCode with Codex
connected-account auth through OpenCode's OpenAI provider and an explicit
OpenAI GPT-5 model selector. Set `agentRuntime: "codex"` only for raw Codex CLI
fallback runs.

`oa-codex-control` must resolve the per-run Vortex `authGrantRef` before it
materializes VM-side Codex auth. The resolver response contains only the
provider-account ref, provider-secret ref, expiry, and status. It does not
return raw Codex credentials.

`OA_CODEX_AUTH_JSON_ROOT` is the VM-local account store for connected
ChatGPT/Codex accounts. Each account gets a separate directory:

```text
$OA_CODEX_AUTH_JSON_ROOT/<provider-account-ref>/auth.json
```

The control API selects that file only after Vortex grant resolution, only
through `oa-workroomd codex auth materialize`, and only into a session-scoped
`CODEX_HOME`; `oa-workroomd codex run` then scrubs that session auth directory
after closeout. Do not print or commit the auth JSON content. The legacy
`OA_CODEX_AUTH_JSON_FILE` single-account bridge still exists for local
development and migration, but production runners should prefer
`OA_CODEX_AUTH_JSON_ROOT` so multiple user accounts cannot overwrite each
other.

The control daemon rejects auth cache files that look like OpenAI API-key
material. `OPENAI_API_KEY`/`CODEX_API_KEY` fallback is not an accepted
production path for user workrooms; reconnect the user's ChatGPT/Codex account
through Vortex instead.

`oa-shc-katy-01` also runs a Codex App Server broker for the Vortex
device-code login flow:

```text
codex app-server \
  --listen ws://0.0.0.0:8788 \
  --ws-auth capability-token \
  --ws-token-file /home/ubuntu/.openagents-secrets/codex-app-server-token
```

Vortex production uses:

```text
VORTEX_CODEX_APP_SERVER_WS_URL=ws://23.182.128.195:8788
VORTEX_CODEX_APP_SERVER_WS_BEARER_TOKEN=<capability token>
```

When the user completes that device-code login, Codex persists the VM-side
auth cache under the selected `CODEX_HOME`. For the connected-account runner,
that `CODEX_HOME` must be the account-scoped home, not the VM user's default
`~/.codex`, for example:

```bash
CODEX_HOME=/home/ubuntu/.openagents-codex-accounts/provider-account_... \
  codex login --device-auth
```

If `codex exec`, `codex login status`, or the compact endpoint returns
`401 token_revoked`, treat the provider account as stale and run the Vortex
ChatGPT/Codex login flow again for that account slot; do not retry with API
keys.

## Vortex Callbacks

When `OA_VORTEX_CODEX_INGEST_URL` is configured, the background worker posts
normalized events and status back to:

```text
$OA_VORTEX_CODEX_INGEST_URL/{runId}/events/ingest
```

The URL may also contain a `{runId}` placeholder for deployments that need an
exact route template. Callback event sequence numbers are local to the SHC
daemon; Vortex assigns canonical Convex sequences. The daemon writes
`callbacks/<sequence>.sent` markers after successful posts so normal retry
loops do not resend already-acknowledged local events. If Vortex receives a
request but the daemon loses the response before writing the marker, duplicate
delivery is still possible; the next hardening pass should include stable
runner event IDs in the Vortex ingest contract.

Callback errors are non-fatal to local execution. Operators should recover by
fixing the Vortex ingest path and replaying unsent events from the local job
registry, not by restarting the Codex workroom blindly.

Callbacks never include raw Codex auth material. Event details are redacted
when they contain token-like markers, and local-only Vortex runs still enforce
their Convex-side retention policy.

Direct Rust Convex ingestion from SHC is not part of the MVP default. See
`docs/control/CODEX_CONVEX_BRIDGE_EVALUATION.md`: the bridge is only acceptable
after Vortex owns runner-scoped Convex functions and a dedicated runner service
identity. Do not put broad Convex admin authority on SHC.

## SHC Profile

The SHC Katy node currently uses `sandboxMode: "danger_full_access"` for real
Codex account-backed runs. This is explicit: the VM/workroom boundary is the
sandbox, there is no wallet authority, and the run receives no broad cloud or
host credentials. `workspace_write` was attempted first and failed at Codex's
Linux bubblewrap/loopback layer on this nested VPS.

## Deployment Shape

For the MVP, run `oa-codex-control` as a systemd service on
`oa-shc-katy-01`, listening on a locked-down public or tunnel endpoint. Vortex
uses:

```text
VORTEX_CODEX_VM_CONTROL_URL=http://23.182.128.195:8787/v1/codex-runs
VORTEX_CODEX_VM_CONTROL_TOKEN=<same bearer token>
VORTEX_CODEX_VM_RUNNER_ID=oa-shc-katy-01
VORTEX_CODEX_VM_SANDBOX_MODE=danger_full_access
```

Longer term this should move behind a private tunnel, mTLS, or Cloudflare
Access. Do not expose the endpoint without the bearer token.

# SHC Agent Deployment Runbook

Date: 2026-06-02

Status: updated operator runbook for OpenAgents Autopilot. This folds the SHC
deployment and connection material from Vortex docs, root `docs/omni`, and root
cloud runbooks into this repo so the SHC path is no longer scattered across
historical audits.

Audience: OpenAgents operators and agents deploying or supervising OpenCode,
Codex, and OpenAgents Autopilot agent workloads on SHC.

## Source Material Reviewed

Root workspace:

- `../../docs/omni/agent-cloud-edge-synthesis.md`
- `../../docs/omni/coding-on-autopilot-wedge-spec.md`
- `../../docs/omni/vortex-coding-agent-cockpit-synthesis.md`
- `../../docs/cloud/backend-strategy.md`
- `../../docs/cloud/codex-vm-workroom.md`
- `../../docs/cloud/shc-pilot.md`
- `../../docs/cloud/gcp-shc-fallback-node.md`
- `../../TAILSCALE_SSH_RUNBOOK.md`

Vortex:

- `../../vortex/docs/workroom-runner-service.md`
- `../../vortex/docs/chatgpt-codex-provider-accounts.md`
- `../../vortex/docs/2026-06-01-long-running-shc-codex-delegation-audit.md`
- `../../vortex/docs/2026-06-01-account-backed-shc-training-loop-audit.md`
- `../../vortex/docs/omni/2026-06-02-effect-first-openauth-opencode-codex-cloudflare-audit.md`

Local repo:

- `../README.md`
- `../AGENTS.md`
- `2026-06-02-cloudflare-only-openagents-sync-audit.md`
- `2026-06-02-chatgpt-codex-account-connection-opencode-openauth-audit.md`

## Executive Summary

SHC is the primary early execution substrate for OpenAgents Autopilot workrooms
and deploy automation. GCP remains the reference, fallback, sensitive-work, and
comparison lane. Cloudflare remains the product edge, auth/sync/control
surface, artifact store candidate, and future coordination layer. It is not the
first place to run OpenCode or Codex process workloads.

The current working split is:

```text
OpenAgents Autopilot / Vortex API
  -> provider-account grant
  -> structured agent or deploy assignment
  -> SHC control API / OpenCode server / oa-workroomd
  -> session-scoped Codex or OpenCode provider auth
  -> private no-wallet workspace
  -> normalized events, artifacts, receipts
  -> OpenAgents product ledger, review, acceptance, and billing policy
```

For this repo, the target split is:

```text
Foldkit webapp
  -> Effect commands and OpenAgents Sync client
  -> Cloudflare Worker API
  -> D1 / Durable Objects / R2 / Queues / Workflows
  -> SHC OpenCode runner fleet
  -> GCloud/GCP fallback runner fleet
```

The product path must use APIs and durable event ingestion. SSH is only an
operator break-glass path. The browser should never send shell commands, raw
Codex credentials, OpenCode server passwords, SSH commands, or deployment
scripts to SHC.

## Current SHC Facts

The current primary SHC node recorded in the root cloud docs is:

| Field                           | Value                                                                                 |
| ------------------------------- | ------------------------------------------------------------------------------------- |
| Runner id                       | `oa-shc-katy-01`                                                                      |
| Public IPv4                     | `23.182.128.195`                                                                      |
| SSH user                        | `ubuntu`                                                                              |
| Product                         | NVMe VPS Enterprise                                                                   |
| Location                        | Katy, Texas                                                                           |
| OS                              | Ubuntu 24.04 LTS                                                                      |
| vCPU                            | 16                                                                                    |
| RAM                             | 64 GB nominal, about 62 GiB usable                                                    |
| Disk                            | 256 GB SSD, about 247 GiB root filesystem                                             |
| Current sandbox posture         | `danger_full_access` inside an externally isolated VM/workroom boundary for Codex MVP |
| Current control API             | `http://23.182.128.195:8787/v1/codex-runs`                                            |
| Current Codex App Server broker | `ws://23.182.128.195:8788`                                                            |

The current GCP reference/fallback node is:

| Field        | Value                                                                    |
| ------------ | ------------------------------------------------------------------------ |
| Runner id    | `oa-gcp-shc-katy-01`                                                     |
| GCP project  | `openagentsgemini`                                                       |
| Zone         | `us-central1-a`                                                          |
| Machine type | `n2-standard-16`                                                         |
| SSH user     | `christopherdavid`                                                       |
| Role         | reference, fallback, canonical grader/storage path, sensitive rerun lane |

The GCP node external IPv4 has been documented as ephemeral. Always confirm it
with GCP before use.

## Current Readiness State

The root cloud docs record that `oa-shc-katy-01` has passed:

- host inventory;
- Rust/Node/Codex bootstrap;
- Cloud bootstrap verification;
- fake-Codex `oa-workroomd` runner test;
- `oa-node` lifecycle and quarantine smoke;
- KVM probe after provider-side configuration change;
- manual Firecracker guest boot smoke;
- real account-backed Codex run through `oa-workroomd codex run`;
- control API and Codex App Server broker installation.

Important qualification: Firecracker has only manual smoke evidence. The first
production agent path should still use the container/no-wallet workroom path
until Firecracker has jailer setup, rootfs/kernel digests, TAP/firewall
receipts, metrics/log collection, artifact closeout, and idempotent cleanup
receipts.

The current Codex sandbox finding is also important. `workspace_write` failed
on SHC because the nested VPS could not initialize loopback through bubblewrap,
and legacy Landlock did not fit the current permission profile. For the current
Codex MVP, use:

```text
VORTEX_CODEX_VM_SANDBOX_MODE=danger_full_access
```

This is acceptable only because the work is inside the no-wallet SHC
VM/workroom boundary. It is not a general instruction to run untrusted code
with host authority.

## Backend Policy

Dispatch policy from the source docs:

```text
Workspace-shaped, bursty, mostly-idle, low-to-medium trust tasks go to SHC.
Sensitive, fallback, canonical grading, durable storage, and reference runs go
to GCP.
```

For this repo:

| Backend    | Use                                                                                                               |
| ---------- | ----------------------------------------------------------------------------------------------------------------- |
| SHC        | Primary OpenCode/Codex workrooms, OpenAgents Autopilot deploy assignments, low-to-medium trust internal runs.     |
| GCP        | Reference/fallback execution, sensitive or canonical reruns, fallback graders, durable backup path.               |
| Cloudflare | OpenAuth, Worker API, D1/Durable Object sync, R2 artifacts, Queues/Workflows, edge routing, future agent gateway. |

Do not route customer-sensitive, wallet-bearing, broad-cloud-credential, or
private-production data to SHC until the trust tier and isolation profile prove
that workload class. SHC should prove measured operator economics first, not
enterprise readiness.

## Product Control Path

The normal path is product API to runner API, not SSH:

```text
User or internal API
  -> OpenAgents Auth/OpenAuth session
  -> ProviderAccountService grant, if Codex/ChatGPT account is needed
  -> AgentRunService or AppDeployService
  -> RunnerGatewayService
  -> SHC OpenCode/Codex control API
  -> event callback ingestion
  -> OpenAgents ledger and UI projection
```

Current Vortex routes that define the historical product path:

```text
GET  /api/autopilot/fleet
POST /api/autopilot/missions
GET  /api/autopilot/missions/:runId
POST /api/autopilot/missions/:runId/command
```

Lower-level Codex routes used for runner ingestion, raw events, and tests:

```text
POST /api/workrooms/codex-runs
GET  /api/workrooms/codex-runs/:runId
GET  /api/workrooms/codex-runs/:runId/events
GET  /api/workrooms/codex-runs/:runId/stream
POST /api/workrooms/codex-runs/:runId/continue
POST /api/workrooms/codex-runs/:runId/steer
POST /api/workrooms/codex-runs/:runId/cancel
GET  /api/workrooms/codex-runs/:runId/artifacts
POST /api/workrooms/codex-runs/:runId/events/ingest
```

Training-run ingestion path already defined in Vortex:

```text
POST /api/training-runs/events
```

For this repo, the new Cloudflare-only names should converge on:

```text
POST /api/omni/agent-runs
GET  /api/omni/agent-runs/:runId
GET  /api/omni/agent-runs/:runId/events
GET  /api/omni/agent-runs/:runId/stream
POST /api/omni/agent-runs/:runId/continue
POST /api/omni/agent-runs/:runId/steer
POST /api/omni/agent-runs/:runId/cancel
POST /api/omni/agent-runs/:runId/events/ingest

POST /api/omni/deployments
GET  /api/omni/deployments/:deployId
GET  /api/omni/deployments/:deployId/events
POST /api/omni/deployments/:deployId/events/ingest
POST /api/omni/deployments/:deployId/rollback
```

The compatibility names can stay around the Vortex migration, but new
Cloudflare-only implementation work should build against `agent-runs` and
`deployments`.

## Break-Glass SSH

SSH is for operator maintenance, not for product dispatch. Before doing any
Tailnet-based remote work from this Mac, read:

```text
../../TAILSCALE_SSH_RUNBOOK.md
```

For SHC public access, the known host shape is:

```sh
ssh-keyscan -H 23.182.128.195 >> ~/.ssh/known_hosts
ssh ubuntu@23.182.128.195
```

For key-specific access:

```sh
ssh -o IdentitiesOnly=yes -i ~/.ssh/<key-name> ubuntu@23.182.128.195
```

Do not put passwords, bearer tokens, auth JSON, refresh tokens, or private keys
in tracked docs, issue comments, command logs, receipts, screenshots, or normal
agent output.

If using Tailscale for other nodes:

```sh
tailscale status
ssh <user>@<tailscale-ip-or-magicdns-name>
```

The root Tailnet runbook records that normal OpenSSH is preferred over
`tailscale ssh` for the local macOS GUI Tailscale setup. On the `archlinux`
Tailnet node, use `bash -ic` for commands that depend on `~/.bashrc`.

## SHC Health Checks

From the operator machine:

```sh
curl -fsS http://23.182.128.195:8787/healthz
nc -vz 23.182.128.195 8787
nc -vz 23.182.128.195 8788
```

On the SHC host:

```sh
curl -fsS http://127.0.0.1:8787/healthz
ss -ltnp | rg '8787|8788'
systemctl list-units --type=service | rg 'openagents|oa-|codex|opencode'
```

If the control API is installed as `oa-codex-control`, useful checks are:

```sh
systemctl status oa-codex-control --no-pager
journalctl -u oa-codex-control -n 200 --no-pager
```

If service names have drifted, inspect units and processes instead of guessing:

```sh
systemctl list-units --type=service | rg 'openagents|oa-|codex|opencode'
ps aux | rg 'oa-codex-control|oa-workroomd|codex|opencode'
```

Do not `cat` or copy auth files during health checks. Use status commands and
redacted receipts.

## Required Environment Variables

Historical Vortex variables for the current SHC Codex lane:

```text
VORTEX_CODEX_VM_CONTROL_URL=http://23.182.128.195:8787/v1/codex-runs
VORTEX_CODEX_VM_CONTROL_TOKEN=<secret>
VORTEX_CODEX_VM_RUNNER_ID=oa-shc-katy-01
VORTEX_CODEX_VM_SANDBOX_MODE=danger_full_access
VORTEX_CODEX_VM_TIMEOUT_MS=<duration-ms>
VORTEX_CODEX_APP_SERVER_WS_URL=ws://23.182.128.195:8788
VORTEX_CODEX_APP_SERVER_WS_BEARER_TOKEN=<secret>
VORTEX_CLOUD_RUNNER_GRANT_TOKEN=<secret>
```

Target variables for this repo's Cloudflare/Effect implementation:

```text
OPENAGENTS_PRIMARY_DEPLOY_BACKEND=shc
OPENAGENTS_FALLBACK_DEPLOY_BACKEND=gcloud

SHC_OPENCODE_CONTROL_URL=<https-or-private-control-url>
SHC_OPENCODE_CONTROL_TOKEN=<secret>
SHC_OPENAGENTS_SERVICE_NAME=openagents-autopilot
SHC_RUNNER_ID=oa-shc-katy-01

GCLOUD_OPENCODE_CONTROL_URL=<fallback-control-url>
GCLOUD_OPENCODE_CONTROL_TOKEN=<secret>
GCLOUD_OPENAGENTS_SERVICE_NAME=openagents-autopilot
GCLOUD_RUNNER_ID=oa-gcp-shc-katy-01

OPENAUTH_ISSUER_URL=<auth-worker-url>
OPENAUTH_CLIENT_ID=<client-id>
OPENAUTH_CLIENT_SECRET=<secret>
OPENAUTH_SESSION_COOKIE_NAME=<cookie-name>
OPENAUTH_SESSION_SECRET=<secret>

OMNI_AGENT_CONTROL_URL=<internal-control-url>
OMNI_AGENT_CONTROL_TOKEN=<secret>
OMNI_RUNNER_CALLBACK_TOKEN=<secret>
OMNI_ARTIFACT_BUCKET=<r2-or-gcs-bucket>
OMNI_OPENCODE_RUNNER_IMAGE=<image-ref-if-containerized>
```

All tokens must be configured as platform secrets. Do not commit `.env` files,
secret refs, `auth.json`, OpenCode auth content, Cloudflare API tokens, or
OpenCode server passwords.

## Provider Account And Codex Auth Boundary

OpenAgents first-party identity and ChatGPT/Codex provider credentials are
separate systems.

```text
OpenAuth
  -> human/session/workspace identity

ProviderAccountService
  -> ChatGPT/Codex account metadata
  -> device-login ceremony
  -> server-side secret refs
  -> short-lived auth grants

SHC runner
  -> resolves a run-scoped grant
  -> materializes provider auth for one run/turn/session
  -> scrubs session auth after closeout
```

Vortex's current provider-account flow is the right reference:

1. Start device login.
2. Show only the public verification URL, user code, and expiry.
3. Record sanitized account state and a stable `providerAccountRef`.
4. Store only a server-side secret ref such as
   `codex-auth://<providerAccountRef>`.
5. Issue a short-lived `authGrantRef` for a run.
6. Let the runner resolve the grant through a bearer-protected endpoint.
7. Runner requests current run-scoped auth material during grant resolution and
   writes it only to that run's private SHC directory.
8. Runner runs the task and scrubs session auth.

In `openagents`, the concrete Cloudflare endpoints are:

```text
POST /api/provider-accounts/:providerAccountRef/grants
POST /api/provider-accounts/chatgpt-codex/grants/resolve
```

The first route is browser-session authenticated and returns only public grant
metadata. The second route is programmatic-agent authenticated and is for SHC
runners only. By default, resolution returns a stable secret ref plus this
redacted OpenCode materialization contract:

```json
{
  "provider": "openai",
  "authRef": "codex-auth://provider-account_...",
  "authContentEnv": "OPENCODE_AUTH_CONTENT",
  "homeIsolation": "per-run-opencode-home",
  "serverPassword": "runner-generated",
  "scrubAfterCloseout": true
}
```

Production SHC control must post `includeAuthMaterial: true` on that route.
That returns the current OAuth JSON from Worker KV only in the bearer-protected
runner response. `oa-codex-control` writes that JSON to a run-scoped
`codex-auth-material.json`, passes it to `oa-workroomd`, and then relies on the
existing session auth scrub. Do not fall back to a long-lived account-scoped
local `auth.json` when current material is available; that causes browser
reconnects to be ignored by SHC.

Do not log, callback, store in D1, or place raw OAuth/OpenCode auth JSON in
artifacts.

## GitHub Writeback Boundary

Autopilot runs that need to push branches, comment on issues, or open pull
requests require a separate GitHub write connection. The browser connection
flow starts at:

```text
GET /auth/github/write/start
GET /auth/github/write/callback
GET /api/github-write/connections
```

The connection must belong to the same GitHub identity as the signed-in
OpenAgents user and must include the required repo/workflow scopes. D1 stores
connection metadata and a secret ref only; the raw OAuth token stays in the
Worker-side secret store.

Before dispatch, the Worker issues a short-lived GitHub write grant for the
same `runId` as the ChatGPT/Codex provider-account grant. The SHC control
daemon resolves it through:

```text
POST /api/github-write/grants/resolve
```

The resolved token is injected only into the one Codex process environment as
`GITHUB_TOKEN` and `GH_TOKEN`. The daemon must not write it to git remotes,
artifacts, callback events, D1, shell logs, or tracked files.

For GitHub issue, pull request, and comment bodies with Markdown or multiple
paragraphs, the SHC runner should write the body to a temporary file and pass it
through `gh` with `--body-file`, or pipe it with `--body-file -`. Do not build
multiline bodies with shell-escaped `\n` strings passed to `--body`; that path
is easy to render incorrectly across shells and agent wrappers. After creating
or editing a GitHub artifact, verify the rendered body with `gh issue view`,
`gh pr view`, or the matching JSON/template read before reporting success.

The work order is attached to the agent-run assignment as `githubWorkOrder`:

```json
{
  "repository": {
    "provider": "github",
    "owner": "OpenAgentsInc",
    "repo": "openagents"
  },
  "baseRef": "main",
  "branchName": "autopilot/smoke-github-write-issue-6",
  "commitMessage": "docs: add autopilot github write smoke receipt",
  "issueNumber": 6,
  "issueUrl": "https://github.com/OpenAgentsInc/openagents/issues/6",
  "pullRequestTitle": "docs: add Autopilot GitHub write smoke receipt",
  "writeback": {
    "pushBranch": true,
    "openPullRequest": true,
    "commentOnIssue": true
  }
}
```

The 2026-06-02 production smoke exercised the full path:

- run: `agent_run_202c4c5e95e64a3791d18272408bdc80`
- issue: `https://github.com/OpenAgentsInc/openagents/issues/6`
- branch: `autopilot/smoke-github-write-issue-6`
- commit: `1fb869e8407b41ea89beb94a74f9a012ee41a8f4`
- pull request: `https://github.com/OpenAgentsInc/openagents/pull/7`

For current SHC Codex runs, account-scoped homes have been documented under:

```text
/home/ubuntu/.openagents-codex-accounts/
  provider-account_a/
    auth.json
    config.toml
  provider-account_b/
    auth.json
    config.toml
```

Do not use one shared `~/.codex` home for multiple ChatGPT accounts. The Codex
home is effectively single-active-account. Later logins can overwrite earlier
credentials.

Do not fall back to `OPENAI_API_KEY` for account-backed Codex workrooms. That
changes billing, quota, authorization, and audit semantics. If a user-selected
provider account was requested, the runner must use the provider-account grant
or fail cleanly.

If Codex returns `401 token_revoked`, mark the provider account stale and start
the device-login flow again before launching another SHC workroom.

## Current Codex Run Path

This is the current product-compatible path inherited from Vortex:

```text
User connects ChatGPT/Codex account
  -> OpenAgents issues authGrantRef
  -> API creates Codex run with backend shc_codex or legacy gcp_vm_codex
  -> Vortex posts assignment to SHC control API
  -> SHC resolves grant with includeAuthMaterial=true
  -> SHC writes run-scoped auth material into the run directory
  -> SHC materializes that auth into session-scoped CODEX_HOME
  -> oa-workroomd runs codex exec --json
  -> runner emits events, artifacts, receipts
  -> OpenAgents ingests events and projects workroom timeline
```

The synchronous control API path is useful but should not be the final long-run
shape. The target SHC control API should return `202 Accepted`, supervise the
run asynchronously, and callback OpenAgents as events arrive.

Target SHC control API:

```text
POST /v1/codex-runs
GET  /v1/codex-runs/:externalRunId
GET  /v1/codex-runs/:externalRunId/events
GET  /v1/codex-runs/:externalRunId/stream
POST /v1/codex-runs/:externalRunId/turns
POST /v1/codex-runs/:externalRunId/cancel
```

`oa-workroomd` should split one-shot cleanup from long-running session
lifecycle:

```text
oa-workroomd codex session create
oa-workroomd codex session start-turn
oa-workroomd codex session status
oa-workroomd codex session events
oa-workroomd codex session continue-turn
oa-workroomd codex session cancel-turn
oa-workroomd codex session closeout
oa-workroomd codex session archive
oa-workroomd codex session destroy
```

Until that exists, use one-shot Codex runs for bounded work and require
product-level continuation through a new run/turn.

## Target OpenCode Run Path

OpenCode should become the core coding-agent harness on SHC. Codex should be
one provider/profile under that harness.

```text
OpenCode session
  -> files, git, LSP, MCP, PTY, tools, permissions, diffs
  -> OpenCode Codex/OpenAI provider profile or Codex CLI fallback
  -> normalized OpenAgents events
  -> artifacts and receipts
  -> OpenAgents review and acceptance
```

The OpenCode server should live inside the runner environment. It should not
be exposed directly to browsers as the product API. The product adapter should
call a narrow subset:

```text
health
session create
session prompt
session status
session events
session abort
diff export
artifact export
closeout
```

OpenCode server auth is runner-internal. Its Basic Auth password or control
token is not human identity and must not replace OpenAuth.

Initial OpenCode runtime assignment:

```json
{
  "schemaVersion": "openagents.agent_run_assignment.v1",
  "runId": "agent_run_...",
  "runtime": "opencode",
  "backend": "shc_vm",
  "assignmentKind": "workroom_agent",
  "goal": "Implement the requested bounded change and run tests.",
  "repository": {
    "provider": "github",
    "owner": "OpenAgentsInc",
    "repo": "openagents",
    "ref": "main"
  },
  "providerAccountRef": "provider_account_...",
  "authGrantRef": "auth_grant_...",
  "modelProfile": {
    "kind": "codex",
    "provider": "openai",
    "model": "default"
  },
  "sandbox": {
    "mode": "workspace_write",
    "network": "restricted",
    "timeoutMs": 300000
  },
  "artifactPolicy": "redacted_logs",
  "callback": {
    "url": "https://openagents.com/api/omni/agent-runs/.../events/ingest",
    "tokenRef": "runner_callback_token"
  }
}
```

Runner behavior:

1. Verify assignment schema and runner token.
2. Create a private workspace.
3. Check out only the allowed repo/ref.
4. Write an `AGENTS.md` workroom contract.
5. Resolve provider-account grant if needed.
6. Resolve/decrypt the returned provider secret ref inside the runner only.
7. Materialize OpenCode-compatible provider auth or Codex session auth in a
   per-run isolated home.
8. Start OpenCode or the Codex fallback profile.
9. Stream OpenCode/Codex events.
10. Normalize events into OpenAgents event vocabulary.
11. Upload large artifacts to object storage.
12. Callback OpenAgents ingest endpoint.
13. Close out, archive, or destroy workspace according to retention policy.
14. Scrub session auth.

## OpenAgents Autopilot Deploy Path

OpenCode on SHC should also execute deploy assignments for OpenAgents
Autopilot. The runner can build, test, install, restart, smoke, roll back, and
write deploy receipts. It cannot own product acceptance, public projection,
billing, auth policy, or provider-account policy.

Target deploy assignment:

```json
{
  "schemaVersion": "openagents.app_deploy_assignment.v1",
  "deployId": "deploy_...",
  "runtime": "opencode",
  "primaryBackend": "shc_vm",
  "fallbackBackend": "gcloud_vm",
  "service": "openagents-autopilot",
  "repository": {
    "provider": "github",
    "owner": "OpenAgentsInc",
    "repo": "openagents",
    "ref": "main"
  },
  "commands": {
    "install": "bun install --frozen-lockfile",
    "typecheck": "bun run typecheck",
    "test": "bun run test",
    "build": "bun run build",
    "smoke": "bun run smoke:shc"
  },
  "callback": {
    "url": "https://openagents.com/api/omni/deployments/.../events/ingest",
    "tokenRef": "runner_callback_token"
  },
  "rollback": {
    "retainPreviousRelease": true,
    "healthCheckUrl": "https://openagents.com/healthz"
  }
}
```

Recommended SHC release layout:

```text
/opt/openagents/
  autopilot/
    releases/
      <git-sha-or-deploy-id>/
    current -> releases/<active>
    previous -> releases/<prior>
    shared/
      logs/
      artifacts/
      tmp/
```

Recommended service posture:

- run the app as a non-root service user;
- bind the app to localhost unless an explicit reverse proxy is configured;
- terminate public TLS at Cloudflare or a hardened edge proxy;
- keep service environment in a protected systemd environment file or secret
  manager;
- keep deployment secrets out of the repository checkout;
- write deploy receipts before and after restart;
- keep the previous release for rollback;
- smoke `/healthz` before promotion.

Operator bootstrap commands for a future direct install should follow this
shape, but should normally be issued by the deploy runner rather than manually:

```sh
sudo mkdir -p /opt/openagents/autopilot/releases /opt/openagents/autopilot/shared
sudo chown -R openagents:openagents /opt/openagents/autopilot
git clone https://github.com/OpenAgentsInc/openagents.git /opt/openagents/autopilot/releases/<deploy-id>
cd /opt/openagents/autopilot/releases/<deploy-id>
bun install --frozen-lockfile
bun run typecheck
bun run test
bun run build
```

Do not make this manual path the default. The default should be a structured
deploy assignment started from OpenAgents and executed by SHC OpenCode.

## Event Vocabulary

Normalize OpenCode, Codex, deploy, and runner-specific events into one
OpenAgents vocabulary:

```text
run.queued
run.started
run.heartbeat
turn.started
message.delta
message.completed
tool.call.started
tool.call.delta
tool.call.completed
shell.command.started
shell.output.delta
shell.command.completed
file.change.detected
file.edit
artifact.created
receipt.created
approval.requested
approval.resolved
run.waiting_for_input
run.paused
run.cancelled
run.failed
run.timed_out
run.completed
auth.stale

deploy.queued
deploy.started
deploy.checkout.completed
deploy.install.completed
deploy.typecheck.completed
deploy.test.completed
deploy.build.completed
deploy.restart.started
deploy.healthcheck.completed
deploy.promoted
deploy.rollback.started
deploy.rollback.completed
deploy.failed
```

Rules:

- events are append-only;
- callbacks are idempotent by `(runId, sequence)` or external event id;
- all payloads are decoded with Effect Schema;
- raw logs, terminal output, diffs, screenshots, transcripts, and tarballs
  should be object refs with digests;
- Convex/D1-visible event rows store bounded excerpts and metadata;
- credential-looking payloads fail closed;
- callback delivery or accounting failures are tracked separately from runner
  terminal state. If SHC retained a completed run but callback delivery to
  Cloudflare returned HTTP 500, the product run should show the runner state
  and a degraded `callbackDelivery` state instead of collapsing the run into
  `runner_failed`;
- missing usage/resource facts must become explicit `usage_unavailable` or
  `resource_unavailable` receipts.

## Artifact And Receipt Rules

SHC can stage artifacts locally, but product authority must use durable refs.

Preferred target for this repo:

```text
R2
  artifacts
  transcripts
  logs
  screenshots
  diff bundles
  deploy bundles
```

Fallback/reference target:

```text
GCS or GCP-managed durable artifact path
```

Every artifact ref should include:

- object ref;
- digest;
- size if available;
- artifact kind;
- source run/deploy id;
- runner id;
- visibility;
- redaction state;
- retention policy.

Every closeout should include receipts for:

- assignment accepted;
- provider-account grant resolved or intentionally absent;
- workspace created;
- sandbox profile;
- commands or tool calls summarized;
- artifact manifest;
- redaction scan;
- resource usage or unavailable reason;
- model usage or unavailable reason;
- cleanup or retention decision;
- final status.

## Retention Modes

Agent and deploy runs should expose retention explicitly:

```text
retentionMode=openagents_durable | local_only
trainingUse=allowed | org_only | denied
localRetentionTtlSeconds=<number>
```

`openagents_durable` means the product ledger persists visible events,
messages, artifacts, receipts, and summaries.

`local_only` means raw content stays on the runner or customer-controlled
storage. OpenAgents stores only enough metadata to authorize, route, cancel,
and close the run. `local_only` runs must not feed shared training,
marketplace signature claims, or public benchmark claims unless the owner later
promotes redacted evidence.

## GCP Fallback

GCP remains active, not archival. Any SHC deploy or agent assignment should
have a fallback equivalent:

```text
SHC assignment
  -> same schema
  -> same event vocabulary
  -> same callback auth
  -> same artifact manifest
  -> same closeout receipt shape

GCP fallback assignment
  -> same schema
  -> backend=gcloud_vm or cloud_run_job
  -> runnerId=oa-gcp-shc-katy-01 or configured GCP service
```

Fallback must be tested regularly. A stale fallback is not a fallback.

Use GCP for:

- SHC outage;
- sensitive/customer workloads not yet trusted on SHC;
- canonical grading or reference reruns;
- artifact durability comparison;
- KVM/Firecracker profile comparison;
- economic comparison receipts.

Do not fork the product protocol for GCP. Only the backend adapter changes.

## Cloudflare Role In This Runbook

Cloudflare is the correct home for:

- OpenAuth Worker;
- Cloudflare Worker API;
- D1 product authority in this repo;
- Durable Objects for realtime sync rooms or future run fanout;
- R2 artifacts;
- Queues for callback retry and fanout;
- Workflows for durable multi-step orchestration;
- DNS/WAF/Access/routing;
- future Cloudflare Container runner gateway after SHC/GCP parity.

Cloudflare Workers are not the first home for OpenCode or Codex execution.
OpenCode is a process-heavy Bun/Node coding-agent runtime with local database,
filesystem, git, LSP, MCP, PTY, tools, plugins, shell execution, and provider
auth. Run it on SHC first. Add Cloudflare Container execution only after the
assignment/event/artifact contract is stable.

## Security Guardrails

Hard rules:

- no wallet authority in workrooms;
- no broad GCP credentials in workrooms;
- no Cloudflare account tokens in workrooms;
- no raw ChatGPT/Codex `auth.json` in product logs, docs, screenshots, D1,
  Convex, receipts, artifacts, or browser state;
- no raw OpenCode `auth.json` or `OPENCODE_AUTH_CONTENT` in product-visible
  state;
- no browser-direct calls to SHC control APIs;
- no browser-direct calls to OpenCode server APIs;
- no arbitrary shell text from browser to runner;
- no API-key fallback for user-connected ChatGPT/Codex workrooms;
- no customer-sensitive work on SHC until the trust tier supports it;
- no public proof claims from local-only or unredacted run data.

Runner callbacks must require:

- bearer token or signed request;
- known runner id;
- known run/deploy id;
- idempotency key or sequence;
- Effect Schema payload validation;
- credential-shaped payload rejection;
- size limits;
- artifact count limits;
- redaction state.

## Troubleshooting

### `401 token_revoked` Or `token_invalidated`

Meaning: the VM-side ChatGPT/Codex account token is stale or invalidated.
This is not a GitHub token failure unless the event explicitly names GitHub.
Runner callbacks and user-facing chat should identify this as a
ChatGPT/Codex account issue and include the provider code/status when present.

Action:

1. Mark the provider account `requires_reauth`.
2. Start the ChatGPT/Codex device-login flow again.
3. Confirm the broker records a server-side secret ref.
4. Issue a fresh run-scoped grant.
5. Confirm SHC grant resolution returns current auth material and does not read
   a stale account-scoped local auth file.
6. Confirm the SHC OpenCode bridge does not synthesize a future token expiry
   when the Codex auth cache omits expiry metadata. Missing expiry must force
   OpenCode to refresh before the first OpenAI request.
7. Retry the run.

Do not switch the run to `OPENAI_API_KEY`.

### Control API unreachable

Checks:

```sh
curl -fsS http://23.182.128.195:8787/healthz
nc -vz 23.182.128.195 8787
ssh ubuntu@23.182.128.195
```

On host:

```sh
systemctl list-units --type=service | rg 'openagents|oa-|codex|opencode'
journalctl -u oa-codex-control -n 200 --no-pager
ss -ltnp | rg '8787|8788'
```

2026-06-03 production failure shape: `oa-codex-control` was active and
listening on `0.0.0.0:8787`, but both public and host-local health checks timed
out. The `oa-codex-control-port80` socat forwarder journal showed repeated
`connect(... 127.0.0.1:8787 ...): Connection timed out` rows, including during
failed browser-launched missions. Restart both services and re-check health:

```sh
sudo systemctl restart oa-codex-control
sudo systemctl restart oa-codex-control-port80
curl -fsS http://127.0.0.1:8787/healthz
curl -fsS http://23.182.128.195:8787/healthz
```

The `openagents-autopilot` Worker logs `agent_run_dispatch_started`,
`agent_run_dispatch_succeeded`, and `agent_run_dispatch_failed` JSON rows keyed
by the mission UUID. A timeout failure should mention the sanitized control URL
and `oa-codex-control` health instead of only recording the generic Fetch abort
message.

If SHC is down or degraded, route the same assignment to GCP fallback.

### No events appear in product UI

Likely causes:

- runner callback token mismatch;
- callback URL wrong;
- event sequence rejected as duplicate or out of order;
- payload failed schema or secret-shape validation;
- runner completed synchronously but product expects async callbacks;
- Durable Object/SSE fanout issue while D1/Convex ingestion succeeded.

Use the product operator path before SSH:

```sh
OPENAGENTS_ADMIN_API_TOKEN=... \
  node scripts/autopilot-operator-checklist.mjs \
  --email chris@openagents.com \
  --teamId team_openagents_core \
  --projectId <project-id> \
  --runId <run-id>
```

Then inspect or retry callbacks through the Worker operator API:

```sh
curl -fsS \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  "https://openagents.com/api/omni/operator/agent-runs/<run-id>?email=chris@openagents.com"

curl -fsS -X POST \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  "https://openagents.com/api/omni/operator/agent-runs/<run-id>/callbacks/retry?email=chris@openagents.com"
```

The run detail includes `operationalState.runner` and
`operationalState.callbackDelivery`. `callbackDelivery.status=failed` means the
runner callback delivery or post-ingest accounting path degraded; it does not by
itself mean the runner failed. If SHC retained a completed run, retry callbacks
so Cloudflare catches up and inspect the runner state separately.

Check product ingest logs before rerunning the agent. If ingestion succeeded,
fix projection/fanout instead of duplicating the run. Use SSH only after the
operator detail/checklist/retry paths cannot recover or inspect the retained
run.

### Multiple ChatGPT accounts interfere

Likely cause: multiple accounts logged into one `~/.codex` home.

Fix:

- keep one account-scoped home per provider account;
- materialize a fresh session-scoped `CODEX_HOME` per run/turn;
- serialize or cap concurrency per account home;
- reissue grants per run/turn.

### Workroom cannot continue

Current one-shot `oa-workroomd codex run` cleans up workspaces after closeout.
Continuation needs one of:

- product-level continuation with transcript/artifact/diff restoration;
- workspace-level continuation with retained workspace refs and TTL;
- native Codex App Server thread continuation.

Do not assume a deleted one-shot workspace can be resumed.

### Sandbox profile fails

On SHC, `workspace_write` has failed in the Codex Linux sandbox layer. Use the
documented MVP profile only when the outer workroom isolation is correct:

```text
danger_full_access inside the no-wallet SHC VM/workroom boundary
```

For production hardening, promote a container or Firecracker profile only after
profile receipts exist.

## First Implementation Slice For This Repo

Build this in order:

1. Define `openagents.agent_run_assignment.v1` and
   `openagents.app_deploy_assignment.v1` in `packages/sync-schema`.
2. Add D1 tables for `agent_runs`, `agent_run_events`, `agent_artifacts`,
   `deployments`, `deployment_events`, and idempotency keys.
3. Add Worker API routes for agent-run start/status/events/ingest and
   deployment start/status/events/ingest.
4. Add a fake SHC runner adapter that emits deterministic events for tests.
5. Add live SHC adapter config, but keep it disabled until token/endpoint
   configuration exists.
6. Add GCP fallback adapter config using the same assignment envelope.
7. Add Foldkit UI projections for agent-run and deploy timelines.
8. Wire OpenAuth identity to route authorization before live SHC dispatch.
9. Add R2 artifact refs and redaction/retention metadata.
10. Only then enable OpenCode/Codex live dispatch from this repo.

## Operator Checklist

Before a live SHC agent run:

- confirm the user/session is authorized;
- confirm target repo/ref and task scope are bounded;
- confirm provider-account grant exists if Codex/ChatGPT auth is needed;
- confirm no raw auth material appears in request payloads;
- confirm SHC health;
- confirm GCP fallback health if the run matters;
- confirm artifact storage target;
- confirm callback token and ingest URL;
- confirm retention mode and training-use mode;
- confirm sandbox profile and no-wallet posture;
- start the run through product API, not SSH.

Before a live SHC deploy:

- confirm approved repo/ref;
- confirm deploy assignment schema;
- confirm build/test/smoke commands;
- confirm service name and health URL;
- confirm rollback target exists or previous release will be retained;
- confirm SHC deploy runner health;
- confirm GCP fallback deploy lane is available for high-risk changes;
- start the deploy through deploy API, not manual shell.

After the run/deploy:

- verify terminal event;
- verify artifact manifest;
- verify receipt manifest;
- verify redaction state;
- verify usage/resource receipts or unavailable receipts;
- verify cleanup or retention decision;
- if accepted, record product acceptance separately from runner completion.

## Open Issues

- SHC OpenCode server service names, ports, and lifecycle commands need to be
  captured after first install. OpenAgents product surface now posts Vortex-compatible cancel
  actions to candidate `codex-runs/cancel` routes when billing credits are
  exhausted, but SHC should provide an explicit cancellation contract and
  receipt for environment teardown.
- The current Codex control API is still mostly a synchronous wrapper; long-run
  async supervision should be implemented.
- `oa-workroomd` needs explicit session lifecycle commands before true
  workspace-level continuation.
- Firecracker needs production profile receipts before being treated as an
  available isolation tier.
- This repo still needs the generic Cloudflare Worker/D1 implementation of the
  agent-run and deployment ledgers.
- GCP fallback must be tested with the same assignment/event contract so it
  does not drift.

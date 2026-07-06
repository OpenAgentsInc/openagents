# Khala Mobile Org-Cloud Executor Runbook

Date: 2026-07-06
Issue: #8473 / MM-C1
Status: initial org-cloud executor spine landed; #8474 owns admission policy,
#8475 owns private repo checkout, #8476 owns isolation hardening, #8477 owns
branch/PR writeback, and #8479 owns charging.

## Purpose

The mobile-only MVP runs coding turns on OpenAgents Cloud, not on a user's
desktop Pylon. This runbook describes the #8473 executor shape:

- an org-owned hosted Pylon pool consumes existing
  `khala_runtime_control_intent.v1` rows from Khala Sync;
- Codex and Claude lanes use org-owned local account homes on those hosts;
- the `hosted_khala` lane uses the OpenAgents gateway with Vertex/Gemini as
  the default model;
- all lanes emit the same `runtime_event` / `runtime_turn` sync entities the
  mobile app already renders;
- every `usage.recorded` runtime event is mirrored into an exact
  `token_usage_events` receipt through
  `POST /api/khala/cloud/runtime-turn-usage`.

The mobile wire contract does not change.

## Deployment Shape

Use the existing hosted-Pylon GCE pattern under `apps/pylon/deploy/gcloud/`.
Each VM runs the runtime-intent supervisor against an org-owned `PYLON_HOME`
that contains only org executor account homes.

Required runtime environment:

```sh
OPENAGENTS_BASE_URL=https://openagents.com
OPENAGENTS_ADMIN_API_TOKEN=<from secret manager>
OPENAGENTS_AGENT_TOKEN=<org executor agent token from secret manager>
OPENAGENTS_RUNTIME_EXECUTOR_MODE=org_cloud
OPENAGENTS_RUNTIME_USAGE_RECEIPTS_ENABLED=1
OPENAGENTS_RUNTIME_HOSTED_KHALA_ENABLED=1
OPENAGENTS_RUNTIME_HOSTED_KHALA_MODEL=gemini-3.5-flash
PYLON_HOME=/var/lib/openagents/org-cloud-pylon
```

Start command:

```sh
bun apps/pylon/src/orchestration/runtime-intent-supervisor.ts \
  --executor-mode org_cloud \
  --workspace-root /var/lib/openagents/runtime-turns \
  --poll-interval-ms 3000 \
  --limit 20
```

Do not pass `--owner-user-id` for the org-cloud pool. Leaving it unset lets the
pool consume admitted mobile org-cloud work across owners. #8474 adds the
credit/session admission gate; until that lands, operators should run the pool
only behind the existing internal controls.

## Execution Lanes

`codex_app_server`
: Uses an org-owned Codex account home from the hosted Pylon registry.
  Usage receipts are attributed to provider `pylon-codex-org-capacity`.

`claude_pylon`
: Uses an org-owned Claude Agent SDK account home from the hosted Pylon
  registry. Usage receipts are attributed to provider
  `pylon-claude-org-capacity`.

`hosted_khala`
: Skips local account selection and calls `/v1/chat/completions` with the
  configured model. Default model is `gemini-3.5-flash`; #8484 owns replacing
  the default with the user's model preference once that contract lands.
  Usage receipts are attributed to provider `vertex-gemini`.

## Scaling

Scale horizontally by adding more hosted-Pylon VMs with distinct `PYLON_HOME`
directories and distinct org executor account homes. The supervisor stores its
watermark in its local orchestration SQLite DB, so each VM should use its own
database path and account set. Capacity fairness remains per host/account; the
#8474 admission gate is responsible for user-level concurrency and rate limits.

Recommended first pool:

- one small VM for hosted Khala/Gemini-only turns;
- one VM with one Codex org account home;
- one VM with one Claude org account home;
- expand by adding account homes rather than sharing one home across hosts.

## Draining

1. Remove the VM from new-work admission at the deployment layer.
2. Send `SIGTERM` to the supervisor.
3. The supervisor stops after the current poll tick. Existing background turns
   may still be running; watch logs for `runtime-intent-supervisor: stopped`.
4. If active turns remain, leave the VM online until they emit terminal
   `turn.finished` or `turn.interrupted` events, then stop the process.
5. Do not delete the workspace root until #8476's retention/isolation policy
   says it is safe.

## Account Rotation

Rotate org Codex/Claude capacity by adding a fresh account home, verifying it
appears ready through the Pylon account readiness checks, then disabling the
old home. Do not move user-owned homes into the org pool. The org-cloud lane
is additive and must never dispatch to another user's Pylon or account home.

For the hosted Khala/Gemini lane, rotate the programmatic
`OPENAGENTS_AGENT_TOKEN` through GCP Secret Manager and restart supervisors.
Never print agent tokens, provider credentials, or OAuth tokens in logs, issue
comments, or docs.

## Receipts

The supervisor posts one exact receipt for each `usage.recorded` runtime event:

```text
POST /api/khala/cloud/runtime-turn-usage
schemaVersion: openagents.khala_cloud_runtime_turn_usage.v1
```

The Worker route:

- requires an `oa_agent_` bearer;
- rejects linked user-Pylon agents posting for a different owner;
- requires nonzero exact input/output token counts;
- writes `openagents.token_usage_event.v1` with
  `demandKind=external`, `demandSource=khala_mobile_org_cloud_runtime`,
  `demandClient=khala-code-mobile`, `usageTruth=exact`;
- records org-capacity provider attribution without charging credits yet.

#8479 consumes these exact receipts for credit charging. Do not charge from
estimates or client-supplied amounts.

## Still Gated

- #8474: mobile-session + positive-credit admission, rate/concurrency limits,
  typed refusals, and INVARIANTS update.
- #8475: private GitHub checkout through the SCM auth broker.
- #8476: isolation enforcement and retention policy.
- #8477: branch/PR writeback with user GitHub authorization.
- #8479: credit metering and balance gate from exact receipts.

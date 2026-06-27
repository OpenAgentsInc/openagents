# Artanis-as-a-Service Phase-1 Smoke Guide

**Date:** 2026-06-27
**Scope:** Step-by-step guide for invited community Codex testers to connect a
local Codex fleet to Khala, run one bounded public smoke task, and record
public-safe proof for the internal Artanis-as-a-Service demo.
**Fixture:** [`fixtures/artanis-as-a-service-smoke-repo`](./fixtures/artanis-as-a-service-smoke-repo/)

This guide complements the multi-tenant enablement plan in
[`../ops/2026-06-27-artanis-as-a-service-multi-tenant-codex-fleet-enablement.md`](../ops/2026-06-27-artanis-as-a-service-multi-tenant-codex-fleet-enablement.md)
and the own-capacity burn runbook in
[`../ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md`](../ops/2026-06-27-khala-codex-own-capacity-burn-runbook.md).

## What This Smoke Proves

- The tester can install Khala and connect one or more Codex accounts with the
  paste-free device login flow.
- Codex credentials stay in isolated Pylon account homes and never touch the
  default `~/.codex` home.
- Khala can route a bounded public coding task to the tester's caller-owned
  local Pylon capacity.
- The local runner can produce a closeout with a passing verification command.
- The public token counter can be reconciled against exact own-capacity rows by
  an operator without exposing raw private trace material.

## Tester Prerequisites

- Node 20+ or Bun.
- The `codex` CLI on `PATH`; if missing, `khala fleet connect` prints the install
  hint.
- A logged-in OpenAgents/Khala token for the invited tester.
- A public GitHub checkout or fork containing this repository and the fixture
  path above.

Do not run `codex login` against the default `~/.codex` home during this smoke.
Use the Khala/Pylon fleet flow below so each account is isolated under
`<pylon home>/accounts/codex/<ref>`.

## 1. Install Khala And Connect The Fleet

```sh
npm install -g @openagentsinc/khala
khala fleet connect
khala fleet status
```

To add more throughput, run `khala fleet connect` again with a distinct ChatGPT
account, or name the account explicitly:

```sh
khala fleet connect --account codex-2
khala fleet status
```

Expected status evidence:

- each connected account has a stable ref such as `codex` or `codex-2`,
- ready accounts show `ready`,
- credential-missing accounts are visible without printing tokens,
- the tester can explain that more distinct accounts mean more independent rate
  budget.

## 2. Pick The Smoke Task

Use the fixture task:

```text
Verify or re-implement the behavior in docs/khala/fixtures/artanis-as-a-service-smoke-repo/src/backlog.js.
Each buildFleetPlan account row must include riskLevel: "low" when readiness is
"ready", otherwise "needs-attention". Keep the summary counts unchanged.
```

Verification command:

```sh
bun test
```

When this fixture is dispatched through Pylon, pass the public repository,
pinned branch/commit, and the fixture-relative verification command. Keep the
prompt public and bounded; do not include private local paths, raw prompts,
secrets, provider payloads, or wallet material.

## 3. Run Through Caller-Owned Pylon Capacity

Operators may use either the CLI spawn bridge or the lower-level Pylon request
flow, depending on which tenant surface is being tested.

CLI spawn shape:

```sh
khala spawn \
  --strategy pylon \
  --count 1 \
  --objective "Implement the Artanis-as-a-Service smoke fixture riskLevel task." \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "<current-origin-main-sha>" \
  --verify "bun test docs/khala/fixtures/artanis-as-a-service-smoke-repo/test/backlog.test.js"
```

Lower-level Pylon request shape:

```sh
bun apps/pylon/src/index.ts khala request \
  --workflow codex_agent_task \
  --pylon-ref "<tester-owned-pylon-ref>" \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "<current-origin-main-sha>" \
  --verify "bun test docs/khala/fixtures/artanis-as-a-service-smoke-repo/test/backlog.test.js" \
  --prompt "Implement the Artanis-as-a-Service smoke fixture riskLevel task." \
  --json
```

Expected request evidence:

- `ok: true`,
- `workflow: "codex_agent_task"`,
- a delegation frame naming the targeted Pylon,
- an `assignmentRef`,
- a `durableRequestId`,
- an `assignmentRun` closeout when auto-run is enabled.

If the request falls through to normal model routing, stop and fix the
delegation preconditions before recording proof.

## 4. Verify And Close Out

The fixture is complete when the assigned workspace passes:

```sh
bun test docs/khala/fixtures/artanis-as-a-service-smoke-repo/test/backlog.test.js
```

Then record the owner-scoped closeout:

```sh
bun apps/pylon/src/index.ts khala closeout "<assignmentRef>" --json
```

Expected closeout evidence:

- `closeoutChecklist.ok: true`,
- status is accepted,
- settlement is `not_applicable`,
- payout claim is not allowed,
- exact own-capacity token rows exist for the assignment,
- owner-only traces exist without raw private material in public projections.

## 5. Public Report Template

Use this structure for the internal report or demo notes:

```text
Artanis-as-a-Service Phase-1 smoke
Date:
Tester:
Khala CLI version:
Fixture commit:
Pylon ref:
Codex account refs:
Assignment ref:
Durable request id:
Verification:
Closeout:
Counter before:
Counter after:
Notes:
```

Public-safe reports may include command names, public refs, status values, and
token totals. They must not include agent tokens, raw Codex events, raw shell
output, credential paths, private repository content, private prompts, wallet
material, or local workspace paths.

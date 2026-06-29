# Artanis-as-a-Service Demo Video Script

**Date:** 2026-06-27
**Audience:** Internal reviewers and invited community Codex testers.
**Goal:** Show the Phase-1 loop in one short recording: connect a Codex fleet,
dispatch a bounded fixture task, verify the closeout, and explain the safety
boundary.

## Scene 1 - Install And Open The Fleet Door

Narration:

> Artanis-as-a-Service lets a Khala user bring their own Codex accounts. The
> credentials stay on their machine; Khala orchestrates and the local Pylon
> executes.

Terminal:

```sh
npm install -g @openagentsinc/khala
khala fleet connect
khala fleet status
```

Callouts:

- The device login shows a short browser code, not a long pasted auth string.
- Each account lands under an isolated Pylon account home.
- The default `~/.codex` session is never touched.

## Scene 2 - Add Throughput

Narration:

> More distinct ChatGPT accounts give the tenant more independent Codex capacity.
> Khala tracks them as a fleet.

Terminal:

```sh
khala fleet connect --account codex-2
khala fleet status
```

Callouts:

- Account refs are stable (`codex`, `codex-2`, ...).
- Readiness is visible without printing tokens.
- Credential problems show as readiness, not as leaked secrets.

## Scene 3 - Show The Bounded Fixture

Narration:

> For the first smoke we use a public fixture repo, not a private customer repo.
> The task is tiny and deterministic: add risk classification to a fleet plan.

Terminal:

```sh
sed -n '1,120p' docs/khala/fixtures/artanis-as-a-service-smoke-repo/README.md
sed -n '1,120p' docs/khala/fixtures/artanis-as-a-service-smoke-repo/test/backlog.test.js
```

Callouts:

- The failing test names the expected behavior.
- The verification command is `bun test`.
- The prompt and repo refs are public-safe.

## Scene 4 - Dispatch To Caller-Owned Pylon Capacity

Narration:

> The coding request names the workflow, the caller-owned Pylon, a public repo,
> a pinned commit, and a verification command. If delegation does not happen, we
> stop instead of spending on a model fallback.

Terminal:

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

Callouts:

- The request is owner/caller scoped.
- The assignment runs no-spend own capacity.
- The local Codex runner may use full local execution rights only on the
  tenant's own linked Pylon.

## Scene 5 - Verify Closeout

Narration:

> Completion is not a vague chat answer. We require the local verification
> command, closeout, exact own-capacity token rows, and public counter
> reconciliation.

Terminal:

```sh
bun apps/pylon/src/index.ts khala closeout "<assignmentRef>" --json
curl -fsS https://openagents.com/api/public/khala-tokens-served
```

Callouts:

- Closeout should be accepted.
- Settlement is `not_applicable`.
- Payout claims are not allowed for own-capacity no-spend work.
- Counter movement alone is not proof; exact rows are the source of truth.

## Scene 6 - Safety Close

Narration:

> The tenant owns the accounts, the Pylon, the repo access, and the local
> execution. OpenAgents provides orchestration and proof, not Codex resale.

Show the report template from
[`2026-06-27-artanis-as-a-service-phase-1-smoke-guide.md`](./2026-06-27-artanis-as-a-service-phase-1-smoke-guide.md).

Do not show tokens, raw SDK events, raw terminal dumps, wallet material, private
repo contents, or local credential paths on screen.

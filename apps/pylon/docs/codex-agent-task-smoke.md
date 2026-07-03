# Codex Bounded Real-Task Smoke

Issue #4790 (CX3); live leg CX4 #4791; epic #4793; promise
`autopilot.codex_probe_pylon_successor.v1`. Companion docs:
`codex-bridge.md` (credential policy, boundaries) and the roadmap
addendum in
`docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`
(workspace root). This runbook is the Codex twin of
`claude-agent-task-smoke.md`.

This smoke proves the sentence the promise's verification asks for: a
Pylon assignment carrying the `codex_sdk` coding work class is admitted
under `capability.pylon.local_codex`, executed by the Codex SDK in a
bounded sandboxed workspace, verified by a real test command, and
closed out through the assignment API with public-safe refs. It has two
legs.

## Hard rules (both legs)

- No-spend means no-spend: `paymentMode: unpaid_smoke`; if any step
  demands payment, stop and record the state — that is a finding, not a
  blocker to route around.
- No credential values, machine identifiers, local paths, prompts,
  provider payloads, or thread-event content in issue comments, Forum
  posts, or retained projections. The smoke's redaction scan enforces
  this over everything it retains; do not weaken it to pass.
- Worker closeout is not accepted work. Review acceptance in this smoke
  grants no payout, settlement, deploy, spend, or Forum authority.
- Stop on first failure; record the failing step and its public-safe
  refs before retrying.

## Leg 1 — CI-safe (no credentials, no network, no spend)

```sh
bun run --cwd apps/pylon smoke:codex-agent-task
```

What it does: spins a local assignment-API harness, registers a
heartbeat, serves one `codex_agent_task` lease (capability-gated in the
payload), and drives the real worker loop — poll → admission → accept →
execute (mock SDK runner applies the fix; the **real** `bun test`
verification runs in the workspace) → progress → artifacts → closeout —
then scans every retained request and the closeout for redaction
violations. Exit 0 requires: closeout `accepted`,
`fixture_repair_passed` result ref, `payoutClaimAllowed: false`,
`settlementState: not_applicable`, `redacted: true`, zero scan
violations.

The same leg runs inside `bun test`
(`tests/codex-agent-task-smoke.test.ts`), so the release gate covers it
on every run.

## Leg 2 — Live (operator-assisted; the promise's outstanding receipt)

This is rung CX4 (#4791): the literal Codex-backed task-path evidence
`autopilot.codex_probe_pylon_successor.v1`'s verification still wants
(#4661 was satisfied via the claude-agent adapter).

Prerequisites (operator):

1. A contributor machine with the owner's own Codex credentials —
   `CODEX_API_KEY`, `OPENAI_API_KEY`, or the owner's own `codex login`
   (see the credential policy in `codex-bridge.md`). The owner pays for
   their own inference.
2. The packaged Pylon installed (`npx @openagentsinc/pylon@latest` or
   the repo checkout), registered, heartbeating, and online:
   `pylon provider go-online` must report `codexAgent.state: "ready"`
   and declare `capability.pylon.local_codex`.
3. A dispatched assignment for this Pylon. From
   `apps/openagents.com/workers/api`:

   ```sh
   OPENAGENTS_ADMIN_API_TOKEN=... bun run scripts/codex-task-dispatch.ts \
     --pylon <pylonRef>
   ```

   The dispatch is `unpaid_smoke`, carries the capability ref inside the
   codingAssignment payload (Pylon-side admission enforces it, not just
   operator dispatch), and ships no instruction text — the fixture lives
   in the installed package.

Then, on the contributor machine:

```sh
PYLON_AGENT_TOKEN=<registered agent token> \
  bun scripts/codex-agent-task-smoke.ts --live [--base-url https://openagents.com]
```

The live leg uses the real readiness probe and the real SDK runner: the
thread runs in the bounded workspace under the Pylon cache with
`approvalPolicy: "never"`, the SDK sandbox pinned to the workspace,
network disabled inside the thread, post-hoc file-change validation
active, and the closeout travels through the deployed assignment API.

Evidence to retain (public-safe only): assignment ref, closeout ref,
result refs, the smoke's JSON output (already redaction-scanned), and
the dispatch receipt. Post them on issue #4791 and in a
`Working: autopilot.codex_probe_pylon_successor.v1` Forum topic
(product-promises), citing the registry version. Then propose the
transition through the receipt service — the maintainer flips the
promise; nobody flips their own.

## Failure modes worth knowing

- `blocker.assignment.codex_agent_unavailable` + `…sdk_missing` or
  `…credentials_missing`: the device is not probe-ready; fix per
  `codex-bridge.md` and re-run `pylon provider go-online`.
- `blocker.assignment.codex_agent_custody_unavailable` plus
  `blocker.pylon.codex_custody.*`: a linked Codex account could not be
  re-primed from OpenAgents custody. Check the Pylon's linked OpenAgents agent
  token, `openAgentsProviderAccountRef`, and the Worker custody route before
  retrying; do not paste refresh tokens into the isolated home as a shortcut.
- `blocker.assignment.wrong_capability` at admission: the Pylon's
  runtime state does not declare the capability — go-online was not run
  after the SDK/credentials became available.
- `blocker.assignment.codex_agent_test_failed`: the thread completed
  but the verification command failed; the closeout is rejected and the
  work is not delivered. That is the gate doing its job.
- `blocker.assignment.codex_agent_workspace_escape_blocked` or
  `…budget_exceeded`: the post-hoc boundary check or the wall-clock
  budget stopped the thread; both are typed, terminal, and reportable
  as-is.

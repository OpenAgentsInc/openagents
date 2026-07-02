# Claude Agent Bounded Real-Task Smoke

Issue #4720; promise `pylon.local_claude_agent_bridge.v1` (registry
`2026-06-10.21`); epic #4717. Companion docs: `claude-agent-bridge.md`
(BYOK setup, boundaries) and the design audit at
`docs/autopilot-coder/2026-06-10-claude-agent-sdk-local-claude-pylon-audit.md`
(workspace root).

This smoke proves the sentence the promise makes: a Pylon assignment
carrying a coding work class is admitted under
`capability.pylon.local_claude_agent`, executed by the Claude Agent SDK
in a bounded workspace, verified by a real test command, and closed out
through the assignment API with public-safe refs. It has two legs.

## Hard rules (both legs)

- No-spend means no-spend: `paymentMode: unpaid_smoke`; if any step
  demands payment, stop and record the state — that is a finding, not a
  blocker to route around.
- No credential values, machine identifiers, local paths, prompts,
  provider payloads, or session JSONL content in issue comments, Forum
  posts, or retained projections. The smoke's redaction scan enforces
  this over everything it retains; do not weaken it to pass.
- Worker closeout is not accepted work. Review acceptance in this smoke
  grants no payout, settlement, deploy, spend, or Forum authority.
- Stop on first failure; record the failing step and its public-safe
  refs before retrying.

## Leg 1 — CI-safe (no key, no network, no spend)

```sh
bun run --cwd apps/pylon smoke:claude-agent-task
```

What it does: spins a local assignment-API harness, registers a
heartbeat, serves one `claude_agent_task` lease (capability-gated in the
payload), and drives the real worker loop — poll → admission → accept →
execute (mock SDK runner applies the fix; the **real** `bun test`
verification runs in the workspace) → progress → artifacts → closeout —
then scans every retained request and the closeout for redaction
violations. Exit 0 requires: closeout `accepted`,
`fixture_repair_passed` result ref, `payoutClaimAllowed: false`,
`settlementState: not_applicable`, `redacted: true`,
`result.public.pylon.claude_agent_task.token_usage_reported`, no
token-accounting blocker refs, and zero scan violations.

The same leg runs inside `bun test` (`tests/claude-agent-task-smoke.test.ts`),
so the release gate covers it on every run.

## Leg 2 — Live (operator-assisted; the promise's green evidence)

Prerequisites (operator):

1. A contributor machine with the user's own Anthropic credentials
   (`ANTHROPIC_API_KEY` or a provider switch — see
   `claude-agent-bridge.md`). The user pays for their own inference.
2. The packaged Pylon installed (`npx @openagentsinc/pylon@latest` or
   the repo checkout), registered, heartbeating, and online:
   `pylon provider go-online` must report `claudeAgent.state: "ready"`
   and declare `capability.pylon.local_claude_agent`.
3. A dispatched assignment for this Pylon. From
   `apps/openagents.com/workers/api`:

   ```sh
   OPENAGENTS_ADMIN_API_TOKEN=... bun run scripts/claude-agent-task-dispatch.ts \
     --pylon <pylonRef>
   ```

   The dispatch is `unpaid_smoke`, carries the capability ref inside the
   codingAssignment payload (Pylon-side admission enforces it, not just
   operator dispatch), and ships no instruction text — the fixture lives
   in the installed package.

Then, on the contributor machine:

```sh
PYLON_AGENT_TOKEN=<registered agent token> \
  bun scripts/claude-agent-task-smoke.ts --live [--base-url https://openagents.com]
```

The live leg uses the real readiness probe and the real SDK runner: the
session runs in the bounded workspace under the Pylon cache, the
PreToolUse boundary hook is active, user `~/.claude` settings are
excluded, and the closeout travels through the deployed assignment API.

Evidence to retain (public-safe only): assignment ref, closeout ref,
result refs, the smoke's JSON output (already redaction-scanned), and
the dispatch receipt. Post them on issue #4720 and in the
`Working: pylon.local_claude_agent_bridge.v1` Forum topic
(product-promises), citing the registry version. Then propose the
transition through the receipt service — the maintainer flips the
promise; nobody flips their own.

## Failure modes worth knowing

- `blocker.assignment.claude_agent_unavailable` + `…sdk_missing` or
  `…credentials_missing`: the device is not probe-ready; fix per
  `claude-agent-bridge.md` and re-run `pylon provider go-online`.
- `blocker.assignment.wrong_capability` at admission: the Pylon's
  runtime state does not declare the capability — go-online was not run
  after the SDK/key became available.
- `blocker.assignment.claude_agent_test_failed`: the session completed
  but the verification command failed; the closeout is rejected and the
  work is not delivered. That is the gate doing its job.
- `blocker.assignment.claude_agent_workspace_escape_blocked` or
  `…budget_exceeded`: the sandbox or budget stopped the session; both
  are typed, terminal, and reportable as-is.
- `blocker.assignment.claude_agent_token_usage_missing`: the SDK result did
  not surface positive exact usage, so no fabricated row was posted.
- `blocker.assignment.claude_agent_token_usage_reporter_unconfigured` or
  `…report_failed`: the local code may have passed, but the exact
  `/api/pylon/claude/turns` row is not proven. Treat this as an accounting
  closeout blocker and rerun after fixing the agent token/base URL or ingest
  route.

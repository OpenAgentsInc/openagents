# CUT-05 Claude owner-local permission receipt

Date: 2026-07-11

Issue: [#8685](https://github.com/OpenAgentsInc/openagents/issues/8685)

Implementation: `509fb27ea1`

## Result

Claude's permissive local execution posture is now a revocable, expiring,
process-opaque authority rather than a configuration or caller-string choice.
The authority is bound to one exact Pylon, run/session, operation/assignment,
and named account. A serialized or restarted copy, mismatched scope, expired or
revoked grant, public assignment, bridge launch, org-cloud runtime, or remote
control path cannot inherit it and remains bounded.

The trusted owner-local compositions are deliberately narrow:

- an accepted owner-local Fleet authority may mint the exact assignment grant;
- an owner-local runtime-intent supervisor may mint the exact operation grant;
- an authenticated loopback control session may mint the exact session grant;
- assignment cancellation revokes the live SDK work through its process-local
  abort signal; and
- public artifacts and closeouts contain only authority, policy, proof, result,
  test, and artifact refs. They never serialize credentials or either Claude
  permission-mode literal.

The full deploy sweep also confirmed and repaired a lifecycle race in the
canonical Fleet manager: manager-owned supervisor ticks now honor
`awaitDispatches`, retaining the exactly-once terminal lifecycle event before
the manager releases its scope. Direct supervisor callers retain the existing
fire-and-forget default.

## Named-account proof

The real local smoke ran through the production assignment executor with the
named isolated account `claude-pylon-3`; it did not use the default Claude
home. The public-safe receipt reported:

- `closeoutStatus: accepted`;
- account hash `account.pylon.claude_agent.9bf9d93a5996e04c3f27cb12`;
- authority `authority.pylon.claude_owner_local.77d8a8df2661f4c603750626`;
- assignment `assignment.public.claude_owner_local_smoke.477f653760171281`;
- run `run.claude_owner_local_smoke.2461aa5cf0030274`;
- verification `command.pylon.claude_agent_task.verification.e4445545d2f1f65194d7675a`;
- patch artifact `artifact.pylon.claude_agent_task.patch.e017d2a9e3f80c25ed1ef7c6`;
  and
- `cleanup: temporary_state_removed`.

The standalone smoke had no configured token-usage reporter, so it honestly
included `blocker.assignment.claude_agent_token_usage_reporter_unconfigured`
and did not fabricate provider usage. That nonfatal evidence blocker does not
change the accepted coding closeout.

## Verification

```bash
bun run --cwd packages/pylon-core test
bun run --cwd packages/pylon-core typecheck
bun run --cwd apps/pylon test
bun run --cwd apps/pylon typecheck
bun run --cwd apps/pylon smoke:claude-owner-local-permission -- --account-ref claude-pylon-3
bun run check:deploy
```

Passed from the clean worktree:

- 65 Pylon-core tests, 210 expectations, and typecheck;
- 2,364 Pylon tests, 11,995 expectations, three explicitly credential-gated
  skips, and zero failures;
- the real named-account accepted closeout and temporary-state cleanup; and
- the complete repository deploy gate, including security, architecture,
  behavior-contract, Khala Sync, selected web, and selected API suites.

CUT-05 closes at this receipt. It does not claim remote Claude authority or the
simultaneous Codex+Claude parent proof. #8640 remains open for CUT-06's
supervisor/publication ordering work and the final owner-authorized Phase A
receipt.

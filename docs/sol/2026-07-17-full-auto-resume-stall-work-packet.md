# Full Auto stalled-conversation resume work packet

- Date: 2026-07-17
- Status: accepted and implemented; focused verification passed
- Authority: owner request and screenshot in the 2026-07-17 Codex session
- Scope: OpenAgents Desktop conversation resume, Full Auto progress, and follow-up admission
- Base: `b82742da3afeb1725760e89456ce6fd1ed77781e`

## Accepted outcome

One stalled Full Auto conversation renders as one canonical recent-chat row,
resumes visibly, and continues to accept follow-up text without starting a
concurrent turn. The exact owner statement is preserved in
`openagents_desktop.chat.full_auto_resume_identity_followup_progress.v1`.

## Claim

```text
CLAIM
actor/session: codex-root-full-auto-resume
base: b82742da3afeb1725760e89456ce6fd1ed77781e
worktree/branch: openagents-full-auto-resume-fix (detached origin/main integration worktree)
scope: canonicalize resumed provider/local conversation identity; restore background Full Auto progress and queued follow-ups
paths: apps/openagents-desktop/src/{main.ts,provider-lane.ts,renderer/shell.ts,renderer/react-composer.tsx,contracts/ux-contracts.ts}; focused tests; this packet
hot files: apps/openagents-desktop/src/main.ts; apps/openagents-desktop/src/renderer/shell.ts; apps/openagents-desktop/src/contracts/ux-contracts.ts
hot contracts: OpenAgents Desktop UX behavior-contract registry; Codex durable queue promotion
verification: focused shell/composer/provider-lane tests; Desktop typecheck/build; behavior-contract validation; pnpm run check
claimed_at: 2026-07-17T11:52:20Z
```

## Implementation boundary

- Main verifies provider-history to Desktop-local aliases and remains the
  background-turn, queue-promotion, and persisted-progress authority.
- Renderer state adopts the returned canonical local ref and removes only the
  verified stale top-level alias.
- Background Full Auto remains main-owned. The composer offers queue-only
  follow-up admission and the existing main-owned Stop path; it does not
  fabricate renderer pending state or start a parallel turn.
- Background progress is a bounded projection of already-persisted thread
  state, not new execution or question-answer authority.

## Verification receipt

- 173 focused Desktop tests passed (11 skipped).
- Desktop TypeScript typecheck passed.
- Repository completion gate and push receipt are intentionally deferred to
  the batched final verification phase.

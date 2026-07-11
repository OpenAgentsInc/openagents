# CUT-16 composer and runtime-interaction foundation receipt

- Date: 2026-07-11
- Issue: [#8696](https://github.com/OpenAgentsInc/openagents/issues/8696)
- Status: shared authority, native persistence, Desktop gateway, and mobile UI
  active; Desktop UI/provider/live acceptance remains open
- Implementations: `a58af4dbfb`, `7b1b9bb066`, `cd5c0dd737`, `1768e8bb35`,
  `11a8d2481a`, `06122c04ed`, `1875b06cac`, `9cd14cef1b`, `2f302d8e1a`,
  `43c5bf6df7`, `c7cf2bf758`, `05ce0e1044`, `b72bf6acbb`, and `835c689c4a`

CUT-16 now builds on the existing rich `@openagentsinc/composer-state` kernel
instead of creating a second composer. The additive private coding-draft
envelope binds its structured text and file/image/snippet attachments to stable
draft/session/thread identity, ref-only repository/worktree/editor/diff
context, explicit provider/model/account/execution-target readiness, and an
editing/queued/accepted/failed/canceled submission lifecycle. Queueing refuses
stale context, unfinished attachments, and unavailable/revoked/offline targets.
Exact duplicate sends reconcile; a failed/canceled retry preserves the same
submission/intent/idempotency identity. Its bounded receipt contains counts,
context kinds, and readiness—not prompt, attachment, account, path, or diff
content.

Shared Sync clients now expose deterministic continue/retry/close controls over
the already-landed server authority. Existing-turn controls are fenced to the
durable provider lane before any insert or state transition.

`openagents.runtime_interaction.v1` is the provider-neutral private authority
for questions, tool approvals, and plan reviews. It carries exact
interaction/thread/turn identity, requested sequence, deadline, bounded
display-safe choices, and pending/resolved/expired/revoked lifecycle. Kind-
matched decisions have stable decision/idempotency refs: exact retry is a
duplicate, conflicting reuse rejects, and late/revoked decisions cannot
resolve.

Migration 0062 and `runtime.requestInteraction` / `runtime.decideInteraction`
store and mutate only the exact private thread post-image. Admission verifies
owner, thread, turn, durable lane, next sequence, state, and server-future
deadline. The confirmed client hides cached interactions outside live thread
authority and merges grouped question/approval/plan facts into the canonical
thread timeline without introducing a new renderer discriminant.

Both native hosts now expose the same live-only interaction client and a
signed-out-capable device-local draft store. Draft persistence is bounded to
128 canonical snapshots of at most 1 MiB each, rejects stale/conflicting/
foreign writes, withholds malformed rows, and survives SQLite restart without
entering hosted Sync. Desktop Runtime Gateway protocol v9 carries bounded
exact-thread reads and confirmed-only decisions through the production host
adapter. Mobile consumes that same authority with typed grouped selection,
tool approve/deny, plan accept/request-changes/replan, disabled reconciliation,
and read-only resolved/expired/revoked states.

Verification:

- composer-state: 23 pass, 0 fail, 161 assertions; shared composer UI: 7 pass,
  0 fail, 69 assertions;
- agent-runtime-schema: 40 pass, 0 fail, 275 assertions;
- Khala Sync schema: 191 pass, 0 fail, 2,705 assertions;
- khala-sync-server after migration/admission: 519 pass, 0 fail, 4,590
  assertions; local Postgres exercises request/decision/retry/conflict/expiry/
  owner/lane/sequence boundaries;
- khala-sync-client: 187 pass, 0 fail, 12,779 assertions (three env-gated live
  smokes skipped);
- Desktop Runtime Gateway focused: 21 pass, 0 fail, 81 assertions; production
  composition/host focused: 51 pass, 0 fail, 445 assertions; Desktop typecheck
  and build pass;
- mobile full suite: 80 pass, 0 fail, 366 assertions; mobile typecheck passes.

CUT-16 remains open. The next honest rungs are Desktop Effect Native
interaction consumption, Pylon/provider request-and-decision consumption,
screen-reader/mobile-keyboard and physical-device acceptance, and named
Codex/Claude live turns. Restart/revocation logic is covered deterministically;
the physical and provider receipts are not.
The default non-interactive provider safety policy must not be weakened merely
to manufacture an approval receipt.

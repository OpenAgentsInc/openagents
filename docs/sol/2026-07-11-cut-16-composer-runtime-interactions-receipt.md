# CUT-16 composer and runtime-interaction foundation receipt

- Date: 2026-07-11
- Issue: [#8696](https://github.com/OpenAgentsInc/openagents/issues/8696)
- Status: shared authority, native persistence, both native interaction UIs,
  Desktop gateway, and Claude provider injection active; live acceptance remains open
- Implementations: `a58af4dbfb`, `7b1b9bb066`, `cd5c0dd737`, `1768e8bb35`,
  `11a8d2481a`, `06122c04ed`, `1875b06cac`, `9cd14cef1b`, `2f302d8e1a`,
  `43c5bf6df7`, `c7cf2bf758`, `05ce0e1044`, `b72bf6acbb`, `835c689c4a`,
  `97f90832bb`, `21d56199bd`, `88f692fe00`, `400c649904`, `600228f230`, and
  `2fae80b1ec`, `9ca4b21828`, `3b42dbddf9`, and `4a9db8347b`

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

Desktop now consumes the confirmed projection as Effect Native question,
tool-approval, and plan-review cards. It preserves canonical thread, turn,
interaction, question, and option identities; refuses missing or ambiguous
display-to-ref mappings; and sends decisions through the protocol-v9 gateway.
The renderer waits for a gateway event and re-reads the exact confirmed
post-image. Neither an enqueue receipt nor a different decision ref can render
the interaction resolved, and expiry/revocation remain distinct terminal
states. The frozen Fable-local question IPC keeps its original outbound shape.

The standing owner-local runtime-intent supervisor now constructs a trusted
HTTP authority from its existing internal Worker credential and fixed owner.
POST executes the real `runtime.requestInteraction` mutator; GET returns only
the exact owner/ref post-image. Claude dispatch injects `canUseTool` only when
that authority is explicitly present, uses the exact current durable event
sequence, and switches that supervised invocation to SDK `default` mode so
the callback genuinely runs. Without the authority, the pre-existing
permission path is unchanged. Only a confirmed matching approval returns raw
tool input to the same SDK call. A separate `runtime.expireInteraction`
mutator uses the database clock to persist deadline expiry without inventing a
deny decision.

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
  and build pass; after authoritative Desktop interaction controls, the full
  Desktop suite is 468 pass, 0 fail, 2,487 assertions;
- mobile full suite: 80 pass, 0 fail, 366 assertions; mobile typecheck passes.
- Pylon typecheck and full suite pass; focused HTTP-authority/runtime-dispatch
  coverage is 59 pass, 0 fail, 208 assertions. The API Worker typecheck and
  focused authority/route-manifest suite pass (6 tests). The runtime mutator
  local-Postgres suite is 15 pass, 0 fail, 112 assertions, including early
  expiry refusal and durable post-deadline expiry.

CUT-16 remains open. The next honest rungs are screen-reader/mobile-keyboard
and physical-device acceptance plus named Codex/Claude live turns. Restart,
revocation, provider injection, and expiry logic are covered deterministically;
the physical and named-provider receipts are not.
The default non-interactive provider safety policy must not be weakened merely
to manufacture an approval receipt.

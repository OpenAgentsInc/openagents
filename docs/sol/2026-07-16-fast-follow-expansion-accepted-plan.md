# Fast Follow expansion — accepted plan and work-packet ledger

- Class: accepted plan and implementation admission
- Date: 2026-07-16
- Owner authority: current owner conversation
- FastFollowSpec: `FASTFOLLOW.md` revision 3
- Program: ordered `initial_program`
- Status: active
- Base: `f2c5591e3b5a2c160f436fb62633a6367272c70d`

## Owner direction

> The policy is now go, that's unblocked. Change the fucking policy if needed. Go, get it going. rofl. This is the expansion.

This direction is the separate target authority required by Fast Follow. It
admits the ordered five-day initial program as an active product-expansion
lane. It supersedes the prior policy block for future work; it does not rewrite
the historical gap or receipt, which remain truthful for their earlier target
revision.

The repository accepts this plan instead of creating a feature issue because
GitHub issues are reserved for concrete reproducible bugs. Each mutating unit
still requires a bounded packet, one root-coordinated or cross-session claim,
an isolated clean worktree, dependency reconciliation, and target-local proof.

## Program order

1. Day 1 — thread-fabric surfaces.
2. Day 2 — disclosed routing and specialists.
3. Day 3 — review and thread reader.
4. Day 4 — placement and remote control.
5. Day 5 — generated clients and signed plugins.

The order is sequential. Admission of the program avoids repeating an owner
ceremony for every day, but it does not authorize agents to implement an
unbounded day at once or collide on shared contracts.

## Active packet — FF-D1-01

Outcome: establish the provider-neutral Queue / Steer / Stop control algebra
and lower the existing Desktop Queue and Steer composer decisions into it.

Owned implementation paths:

- `packages/agent-runtime-schema/src/thread-control.ts`
- `packages/agent-runtime-schema/src/thread-control.test.ts`
- `packages/agent-runtime-schema/src/index.ts`
- `packages/agent-runtime-schema/README.md`
- `apps/openagents-desktop/src/composer-admission.ts`
- `apps/openagents-desktop/src/composer-admission.test.ts`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/shell.test.ts`
- `apps/openagents-desktop/package.json`
- `pnpm-lock.yaml`

Hot contracts: runtime-control schema literals, behavior-contract registry,
FastFollowSpec revision, ProductSpec revision, and Sol manifest.

Required behavior:

- semantic kinds are exactly `turn.queue`, `turn.steer`, and
  `turn.interrupt`;
- an adapter never silently turns steer into queue or queue into a new turn;
- stable intent, idempotency, thread, turn, message, generation, origin,
  ordering, and deadline identity is explicit;
- raw message bodies stay outside the shared control contract;
- admission, delivery, and terminal observation remain separate evidence;
- exact lost-ACK retries dedupe, while conflicting identity reuse fails
  closed; and
- current Desktop behavior remains available while Sync, Pylon, mobile, and
  other adapters await later packets.

Proof: focused schema tests/typecheck, focused Desktop composer/shell and local
runtime tests/typecheck, Fast Follow and behavior-contract checks, Sol document
checks, ProductSpec checks, and the repository-required `pnpm run check`.

Close rule: this packet is an implementation foundation, not Day 1 completion.
A rendered Desktop journey and real compatible-adapter outcomes are required
before claiming the Day 1 surface complete.

## Active packet — FF-D1-02

Outcome: migrate the foreground Desktop Stop path across both current ChatHost
adapters to the provider-neutral `turn.interrupt` envelope and return a typed
runtime-control outcome without changing the visible Stop behavior.

Owned implementation paths:

- `apps/openagents-desktop/src/composer-admission.ts`
- `apps/openagents-desktop/src/composer-admission.test.ts`
- `apps/openagents-desktop/src/renderer/shell.ts`
- `apps/openagents-desktop/src/renderer/shell.test.ts`
- `apps/openagents-desktop/src/renderer/local-harness.ts`
- `apps/openagents-desktop/src/renderer/local-harness.test.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.ts`
- `apps/openagents-desktop/src/renderer/runtime-conversation.test.ts`
- `docs/fastfollow/receipts/2026-07-16-ff-d1-02-desktop-stop-control-receipt.md`
- this accepted-plan ledger

Hot contracts: `openagents.runtime_control_intent.v2`,
`openagents.runtime_control_outcome.v1`, and the Desktop `ChatHost` interrupt
seam. This packet does not version those shared schema literals.

Required behavior:

- Desktop mints one ref-only `turn.interrupt` intent for the exact active
  thread and turn before either current ChatHost adapter signals Stop;
- the local and durable-conversation adapters validate exact thread/turn
  identity and never reroute interrupt into queue, steer, or a new turn;
- adapter admission/delivery acknowledgement is returned as
  `openagents.runtime_control_outcome.v1`, while terminal interruption remains
  an independently observed runtime event;
- missing, stale, or mismatched active state fails closed without dispatch;
  and
- the existing Stop control and provider-specific transport remain available.

Proof: focused composer, shell, local-harness, and runtime-conversation tests;
Desktop typecheck; Fast Follow, behavior-contract, ProductSpec, and Sol
document checks; and repository-required `pnpm run check`.

Close rule: this packet closes only foreground Desktop Stop lowering and typed
adapter acknowledgement. Durable cross-restart outcome storage, lost-ACK
replay reconciliation, Queue/Steer outcome persistence, Sync/mobile/Pylon
adapters, rendered evidence, and Day 1 completion remain later packets.

### CLAIM

- actor/session: `codex-full-auto-ff-d1-02-20260717`
- base: `fdc673ad54120ff14ff3483d13a9051a2469b258`
- worktree/branch: `openagents-ff-d1-02` / detached `origin/main`
- scope: foreground Desktop Stop canonical lowering and typed adapter outcome
- paths: the FF-D1-02 owned implementation paths above
- hot files: this accepted-plan ledger and the Desktop ChatHost interrupt seam
- hot contracts: runtime-control schema literals and Desktop ChatHost interrupt return type
- verification: the focused and repository-required checks above plus the packet receipt
- claimed_at: `2026-07-17T05:00:27Z`

## Explicit non-authority

This plan grants no deployment, release, paid-provider spend, credential,
settlement, public-promise, cross-tenant sharing, or invariant-bypass
authority. It does not revive deprecated clients or authorize external-source
mutation. Those actions retain their own gates.

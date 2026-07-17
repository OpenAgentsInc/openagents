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

## Explicit non-authority

This plan grants no deployment, release, paid-provider spend, credential,
settlement, public-promise, cross-tenant sharing, or invariant-bypass
authority. It does not revive deprecated clients or authorize external-source
mutation. Those actions retain their own gates.

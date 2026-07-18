# Sarah owner-orchestrator reboot — accepted plan

- Class: accepted authority and implementation plan
- Status: active
- Date: 2026-07-18
- Owner authority: current owner conversation
- Base commit: `888574ab00f0cb86611e2178ca057db673caa87b`
- ProductSpec: [`../../specs/openagents/sarah-owner-orchestrator.product-spec.md`](../../specs/openagents/sarah-owner-orchestrator.product-spec.md)
- Authority: [`../../AUTHORITY.md`](../../AUTHORITY.md) revision 3 and [`../authority/SARAH_AUTHORITY.md`](../authority/SARAH_AUTHORITY.md) revision 1

## Decision

Reboot Sarah as `principal.sarah`, the owner's authenticated, persistent
orchestrator inside supported OpenAgents clients. The first production surface
is the existing OpenAgents mobile conversation system. One stable owner-private
Khala Sync thread provides relationship continuity and conversation memory;
fresh bounded source adapters provide business state; the hosted Khala runtime
answers with cited context; Effect authority and existing target adapters own
actions and receipts.

The reboot deliberately does not restore `apps/sarah`, the public `/sarah`
route, prospect/session duplication, a second CRM, a second transcript store,
LiveAvatar/GPU rendering, or Sarah-specific provider dispatch. Those July 2026
experiments remain historical source material.

## Historical reconciliation

The July 7–10 history contained four durable ideas worth reviving: Sarah as the
owner's primary relationship, persistent owner memory, fleet/company context,
and a phone-first conversation. It also contained expensive duplication that
the July 10 removal correctly killed. The current owner direction supersedes
the blanket persona removal only for an authenticated capability on the
supported app; it does not supersede the standalone-surface tombstone.

## Fastest implementation path

1. Add `@openagentsinc/authority`: an Effect service that exactly matches
   role/action/resource/program/conditions, denies reserved actions, and emits
   the versioned receipt.
2. Add `@openagentsinc/sarah`: principal, capability, cited context, runtime
   profile, and system-prompt schemas.
3. Add authenticated `/api/mobile/sarah`, deterministically bootstrap the
   owner's stable `Sarah` chat thread through the existing Sync push engine,
   and return only the public-safe principal projection.
4. Before hosted inference on that thread, collect recent conversation,
   current release/open-issue, Full Auto, FleetRun, and contract sources. Each
   source fails independently and is bounded. Build the Sarah prompt from
   those exact refs.
5. In mobile, pin Sarah in the existing drawer, prefer her thread when no
   restored coding/active Full Auto thread wins, label the authority revision,
   and force ordinary Sarah turns to `hosted_khala`.
6. Deploy the Worker/Cloud Run backend, smoke the authenticated principal and
   a real hosted turn, then publish the own OTA channel. Preserve `/sarah` 404.

## Capability rollout

The first release has live owner conversation, durable thread memory, GitHub
release/issue context, recent public Forum activity, Full Auto/Fleet context,
cloud health, and contract context. Repository delivery, GCP operations, RC
publication, and GitHub/Forum communication are brokered through the existing
mechanisms; no new super-adapter bypasses them. Richer company-priority
projections, semantic memory compaction, and more target receipts can land
incrementally without changing the principal or UI contract.

## Verification and release gates

- package tests: authority allow/deny/condition behavior and Sarah prompt
  citations;
- Worker route: owner authentication, opaque stable thread, connection close;
- mobile Effect Native: pinned identity, authority label, Sarah composer, and
  hosted-lane routing;
- behavior registry coverage for the exact owner statement;
- Worker and mobile typechecks plus repository `pnpm run check`;
- production deploy health, `/sarah` tombstone, authenticated principal, and
  real Sarah hosted-turn receipt;
- own OTA publication with bumped bundle tag and manifest receipt.

## Hot contracts

Changes touch root authority and invariants, the shared behavior registry,
Khala hosted dispatch, Worker exact-route wiring, mobile experience selection,
and the Effect Native home program. These files are one atomic packet; no
parallel writer may change their contract during this landing without an
explicit handoff.

## Definition of done

The owner can open OpenAgents mobile, select Sarah, ask a current business
question, receive a cited real-system answer on the persistent thread, close
and reopen the app without losing continuity, and observe that all mutation
authority remains brokered and auditable. Backend is deployed, OTA is
published, tests are green, and main is pushed.

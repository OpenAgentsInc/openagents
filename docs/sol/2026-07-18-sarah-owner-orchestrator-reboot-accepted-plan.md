# Sarah owner-orchestrator reboot — accepted plan

- Class: accepted authority and implementation plan
- Status: active
- Date: 2026-07-18
- Owner authority: current owner conversation
- Base commit: `888574ab00f0cb86611e2178ca057db673caa87b`
- ProductSpec: [`../../specs/openagents/sarah-owner-orchestrator.product-spec.md`](../../specs/openagents/sarah-owner-orchestrator.product-spec.md) revision 4
- Authority: [`../../AUTHORITY.md`](../../AUTHORITY.md) revision 5 and [`../authority/SARAH_AUTHORITY.md`](../authority/SARAH_AUTHORITY.md) revision 3

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
7. Revision 2 adds a bounded Gemma 4 function-calling loop over existing
   brokers: read/dispatch/status for owner-linked Codex workers and
   read/pause/resume/stop for existing Full Auto runs. Every call emits visible
   runtime activity plus an authority receipt; pending is never presented as
   completed. MemoHarness bank, adaptation, and promotion remain unavailable.
8. The 2026-07-19 managed-sandbox expansion proceeds only through epic #9023:
   SBX-00 freezes and admits the exact authority/resource/condition contract,
   SBX-07 composes the shared managed-sandbox broker, and SBX-09 independently
   proves the live GCP journey. Until those gates land, Sarah's runtime
   capabilities remain the revision-3 authority set.

## 2026-07-19 managed-sandbox expansion

Sarah ProductSpec revision 4 now admits the desired owner outcome: create,
list, inspect, dispatch into, interrupt, stop, resume, and delete an
OpenAgents-managed sandbox through the same `ManagedSandboxService` used by
Desktop. The implementation authority and issue order live in the
[`managed-sandbox accepted plan`](./2026-07-19-managed-agent-sandboxes-accepted-plan.md)
and [epic #9023](https://github.com/OpenAgentsInc/openagents/issues/9023).

This is a gated intent revision, not a live tool grant. `AUTHORITY.md` revision
5 and `docs/authority/SARAH_AUTHORITY.md` revision 3 remain runtime truth until
SBX-00 lands their exact successor profiles with denial tests. Sarah never
receives raw `gcloud`, shell, database, topology, service-account, provider
credential, host-path, or generic container-admin access. A sandbox work unit
also remains distinct from `FullAutoRun`; remote Full Auto start stays excluded
unless Full Auto receives its own exact ProductSpec and AssuranceSpec revision.

## Capability rollout

The first release has live owner conversation, durable thread memory, GitHub
release/issue context, recent public Forum activity, Full Auto/Fleet context,
cloud health, and contract context. Repository delivery, GCP operations, RC
publication, and GitHub/Forum communication are brokered through the existing
mechanisms; no new super-adapter bypasses them. Richer company-priority
projections, semantic memory compaction, and more target receipts can land
incrementally without changing the principal or UI contract.

Managed-sandbox lifecycle and dispatch are a later broker capability under
#9023, not part of the already-landed Sarah reboot claim. They reuse the same
principal, thread, ordered runtime activity, authority receipts, and target
receipts; they do not add a Sarah-only control plane or state model.

## Verification and release gates

- package tests: authority allow/deny/condition behavior and Sarah prompt
  citations;
- Worker route: owner authentication, opaque stable thread, connection close;
- mobile Effect Native: pinned identity, authority label, Sarah composer, and
  hosted-lane routing;
- behavior registry coverage for the exact owner statement;
- Worker and mobile typechecks plus repository `pnpm run check`;
- production deploy health, `/sarah` tombstone, authenticated principal, and
  real Sarah hosted-turn plus tool receipts. Revision 2 is server-only because
  the installed mobile client already renders ordered runtime tool events; no
  manufactured OTA or native release is required.

## Hot contracts

Changes touch root authority and invariants, the shared behavior registry,
Khala hosted dispatch, Worker exact-route wiring, mobile experience selection,
and the Effect Native home program. These files are one atomic packet; no
parallel writer may change their contract during this landing without an
explicit handoff.

## Active implementation claim — Sarah action runtime

```text
CLAIM
actor/session: principal.sol.sarah-action-runtime-2026-07-18
base: 23f0cacf073ff0d6ce8256822f9a10cde346aa6f
worktree/branch: detached-worktree.sarah-runtime.3whQXX
scope: Give Sarah receipted tools for owner-capacity Codex worker dispatch and existing Full Auto run supervision, using Gemma 4 function calling and existing typed brokers. MemoHarness learning, private-bank access, candidate promotion, assurance admission, and authority expansion remain unavailable.
paths: AUTHORITY.md; INVARIANTS.md; docs/authority/SARAH_AUTHORITY.md; docs/sol/2026-07-18-sarah-owner-orchestrator-reboot-accepted-plan.md; specs/openagents/{authority-delegation,sarah-owner-orchestrator}.{product,assurance}-spec.md; packages/sarah/**; apps/openagents.com/workers/api/src/**; apps/openagents-desktop/src/** only if a typed Full Auto start intent is required
hot files: AUTHORITY.md; INVARIANTS.md; hosted runtime dispatch; Sarah authority receipts; Full Auto control contracts
hot contracts: authority revisions; authority receipt schema usage; Gemma tool-call wire mapping; Khala runtime event sequencing; Full Auto control intent schema/migrations
verification: focused package/Worker/Desktop tests and typechecks, root pnpm run check, production Cloud Run health, and one real owner-scoped Sarah tool receipt
claimed_at: 2026-07-18T22:05:00Z
```

## Definition of done

The owner can open OpenAgents mobile, select Sarah, ask a current business
question, receive a cited real-system answer on the persistent thread, ask her
to inspect or dispatch Codex workers and supervise an existing Full Auto run,
close and reopen the app without losing continuity, and observe that every
mutation remains brokered and auditable. Backend is deployed, tests are green,
and main is pushed.

For the separately admitted managed-sandbox expansion, done additionally means
SBX-00/07/09 are closed: the owner can create and supervise one real GCP
sandbox through the same durable lifecycle authority as Desktop, every step
has authority and target receipts, and delete proves zero residue. That later
gate does not retroactively widen this plan's completed Sarah runtime proof.

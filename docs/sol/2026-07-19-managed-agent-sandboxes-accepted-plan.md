# Managed agent sandboxes, Box compatibility, IDE, and Sarah — accepted plan

- Class: accepted owner authority and implementation plan
- Status: active
- Date: 2026-07-19
- Owner authority: current owner conversation
- Base commit: `43b5dbc56e05e1278f99dd269824087b205e40f0`
- Epic: [#9023](https://github.com/OpenAgentsInc/openagents/issues/9023)
- ProductSpec: [`../../specs/openagents/managed-agent-sandboxes.product-spec.md`](../../specs/openagents/managed-agent-sandboxes.product-spec.md) revision 1
- Source analysis: [`../teardowns/2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md`](../teardowns/2026-07-19-ascii-box-optibox-openagents-gcp-analysis.md)

## Owner outcome

Make OpenAgents-managed agent sandboxes operational on the existing Google Cloud substrate.
The IDE must create an isolated managed sandbox and attach a project and
long-running agent.
It must observe exact lifecycle and runtime truth.
It must stop and resume the sandbox, then return reviewable code and evidence.

Sarah must create and supervise the same resource through a narrow owner-scoped broker.
Mobile and authenticated web must supervise through bounded typed projections.

Expose the useful first subset of the Ascii Box v1 API from an OpenAgents-owned
base URL and prove the unmodified MIT TypeScript SDK against it. This is a
compatibility target over OpenAgents infrastructure, not a migration to Ascii,
an adoption of Optibox source, or a second control plane.

## Decision

1. Google Cloud remains the only production compute and storage authority.
2. `SandboxResource` is the product concept.
   Its effective isolation unit may be a full GCE VM or a Firecracker microVM.
   The product must report that unit honestly.
   The word container does not imply OCI-container semantics.
3. Existing `oa-codex-control`, `oa-workroomd`, Agent Computer, capability
   broker, Khala Sync, event, artifact, portable-session, and receipt seams are
   composed before any new substrate is invented.
4. Effect Schema and OpenAgents native contracts remain canonical. The Box
   facade is a versioned lossy projection, and exact
   `@asciidev/box-sdk@0.0.24` is development-only conformance input.
5. Phase 1 admits lifecycle, prompt/status/events/interrupt, bounded files,
   commands, and artifacts. Unsupported methods answer a stable typed 501.
   Snapshot, fork, private desktop, SSH, repository discovery, and account
   secret parity stay out until SBX-10 proves their different semantics.
6. Durable lifecycle separates lease, guest, filesystem, ingress, and runtime
   turn facts. Silence is not completion. Idle stop can arm only after a
   structural runtime settlement.
7. IDE-13 consumes the managed project capability; IDE-17 consumes background
   and long-running agent execution. Neither gets a second project, session,
   work-unit, or agent graph.
8. Sarah ProductSpec revision 4 owns the conversational action outcome.
   SBX-00 admits root authority revision 6 and Sarah revision 4 with eight closed
   actions.
   Runtime admission still refuses mutation until SBX-07 lands the broker and
   SBX-09 proves the live target. Sarah never
   receives generic `gcloud`, shell, database, topology, or container-admin
   access.
9. Mobile revision 7, portable sessions revision 4, Desktop revision 7, and
   Cursor parity revision 3 already admit managed placement and bounded
   supervision. Their bytes do not change. This program supplies a concrete
   target dependency; it does not claim their broader acceptance.
10. Full Auto revision 14 still excludes cross-machine run admission. A
    sandbox work unit is not silently a remote `FullAutoRun`. Any later
    composition must revise and rebind the Full Auto ProductSpec and
    AssuranceSpec explicitly.

## Architecture

```text
OpenAgents Desktop / Sarah / mobile / authenticated web / Box SDK
                              |
                    typed app-owned adapters
                              |
                  ManagedSandboxService (Effect)
       lifecycle | runtime | capability | event | artifact | receipt
                              |
             OpenAgentsSandboxV1 / Cloud control contracts
                              |
          oa-codex-control -> GCE VM or Firecracker microVM
                              |
                    oa-workroomd in guest
                              |
       scoped provider / SCM / tool grants + private evidence
```

The Box route is a client adapter at the top of this graph. It never owns
authorization, storage, lifecycle, billing, scheduling, runtime completion,
or cleanup truth.

## Dependency-ordered issue ledger

| Packet | GitHub                                                           | Outcome                                                               | Depends on                                                  | State                                                  |
| ------ | ---------------------------------------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------ |
| SBX-00 | [#9029](https://github.com/OpenAgentsInc/openagents/issues/9029) | contract, authority, AssuranceSpec, model, and SDK conformance freeze | this plan                                                   | complete                                               |
| SBX-01 | [#9034](https://github.com/OpenAgentsInc/openagents/issues/9034) | durable lifecycle and generation-fenced store                         | SBX-00                                                      | implemented and verified; provider runtime unavailable |
| SBX-02 | [#9028](https://github.com/OpenAgentsInc/openagents/issues/9028) | real GCP runtime layer and image admission                            | SBX-00/01                                                   | next                                                   |
| SBX-03 | [#9025](https://github.com/OpenAgentsInc/openagents/issues/9025) | admitted Box v1 facade and unmodified SDK proof                       | SBX-00/01                                                   | not started                                            |
| SBX-04 | [#9024](https://github.com/OpenAgentsInc/openagents/issues/9024) | long-running Codex/Claude turns, events, and interrupt                | SBX-01/02/03                                                | not started                                            |
| SBX-05 | [#9026](https://github.com/OpenAgentsInc/openagents/issues/9026) | bounded files, commands, artifacts, quota, and hardening              | SBX-02/03                                                   | not started                                            |
| SBX-06 | [#9027](https://github.com/OpenAgentsInc/openagents/issues/9027) | IDE project/agent graph integration                                   | SBX-04/05 plus IDE-08 #9036, IDE-10 #9038, and IDE-12 #9040 | not started                                            |
| SBX-07 | [#9030](https://github.com/OpenAgentsInc/openagents/issues/9030) | Sarah lifecycle and dispatch broker                                   | SBX-00/04/05                                                | not started                                            |
| SBX-08 | [#9031](https://github.com/OpenAgentsInc/openagents/issues/9031) | bounded mobile and web supervision                                    | SBX-06/07                                                   | not started                                            |
| SBX-09 | [#9033](https://github.com/OpenAgentsInc/openagents/issues/9033) | independent live GCP acceptance and rollout                           | SBX-00 through SBX-08                                       | not started                                            |
| SBX-10 | [#9032](https://github.com/OpenAgentsInc/openagents/issues/9032) | proven checkpoint/fork/private desktop Phase 2                        | SBX-09                                                      | not started                                            |

The issues are native subissues of #9023. Each child owns one bounded claim;
the epic is not a mutation claim. SBX-01, SBX-03, and the Assurance/model lane
may proceed in parallel only after SBX-00 freezes their shared schemas. SBX-02
owns Cloud image and provisioner hot contracts. SBX-06 and SBX-07 may proceed
in parallel after the runtime boundary is stable. SBX-09 is the only Phase 1
live-acceptance and rollout gate.

## Phase 1 API boundary

The first compatibility milestone covers:

- `GET /me` and `GET /limits`;
- `GET|POST /boxes`;
- `GET|PATCH|DELETE /boxes/{boxId}`;
- `POST /boxes/{boxId}/stop` and `/resume`;
- `POST /boxes/{boxId}/prompt`;
- `GET /boxes/{boxId}/prompts/{promptId}` and `/events`;
- `POST /boxes/{boxId}/interrupt`;
- `GET|PUT /boxes/{boxId}/files`;
- `POST /boxes/{boxId}/commands`; and
- `GET /boxes/{boxId}/artifacts`.

This method list is compatibility scope, not canonical server architecture.
Every call resolves authenticated owner, tenant, work unit, sandbox,
generation, target, lease, budget, capabilities, and current authority below
the HTTP adapter.

## Cross-surface composition

### IDE

SBX-06 is an implementation dependency of IDE-13 and IDE-17. IDE-13 owns the
same local/owner-managed/OpenAgents-managed project capability shapes and
attachment truth. IDE-17 owns background agent and automation experience.
Sandbox lifecycle never bypasses IDE-08 proposal/evidence, IDE-10 runtime
process, or IDE-12 worktree/delivery contracts. The current P0 IDE-07 daily-
use gate remains independent and is not delayed or promoted by this program.
The canonical post-basic-IDE epic is #9035; SBX-06 supplies the managed target
consumed by IDE-13 #9041 and IDE-17 #9045 without claiming their broader exits.

### Sarah

Sarah revision 4 states the desired brokered action. SBX-00 must change the
root and Sarah authority profiles only after the closed action/resource/
condition contract and its denial tests exist. SBX-07 then composes the same
ManagedSandboxService used by Desktop. Every conversational tool call emits
ordered owner-visible activity plus authority and target receipts; pending,
failed, refused, recovery-required, and complete stay distinct.

### Mobile and web

Mobile and web remain controllers. SBX-08 reuses the any-host directory,
exactly-once outbox, attention inbox, portable-session refs, and IDE-14 safe
projections. Neither client receives the Box SDK, GCP client, raw path, PTY,
provider credential, generic shell, or lifecycle authority.

### Full Auto

This plan supports long-running managed agent work units but does not reopen
Full Auto cross-machine admission.
A later owner direction may bind a `FullAutoRun` to an admitted sandbox.
That change requires an exact spec revision, AssuranceSpec rebind,
lifecycle/lease composition, and fault proof. SBX issue
bodies must not work around this boundary by renaming a remote Full Auto run.

## Verification and release gates

Phase 1 requires:

- ProductSpec, AssuranceSpec, Effect Schema, model, and compatibility-corpus
  validation;
- unmodified SDK conformance against fake, staging, and owner-gated live base
  URLs with exact OpenAPI/package/translator identities;
- deterministic retry/conflict, lifecycle interleaving, tenant/generation
  isolation, cursor, interrupt, capability, budget, crash, and cleanup faults;
- one real Codex and one real Claude managed turn;
- packaged Desktop and real Sarah owner-thread journeys through the same
  broker;
- bounded mobile/web supervision faults at their named proof rungs;
- observed GCP readiness, incremental cost under the standing cap, zero
  residual compute/firewall/ingress/scratch/process/grant evidence, rollback,
  and independent review; and
- exact release/promise gating before any public availability or parity claim.

Fake mode, configured job IDs, and SDK terminal state cannot prove live
acceptance.
Screenshots, Cloud resource existence, and successful guest commands also
cannot prove it.
Live isolation, runtime, cleanup, and owner acceptance require direct evidence.

## Hot contracts

- `specs/openagents/managed-agent-sandboxes.product-spec.md` and its future
  AssuranceSpec;
- `AUTHORITY.md`, `docs/authority/SARAH_AUTHORITY.md`, and authority receipts;
- sandbox/workroom/portable-session schemas, lifecycle generations, Cloud SQL
  migrations, event cursor, and capability broker;
- GCP image/provisioner configuration and Cloud invariants;
- IDE project/placement and agent graph contracts;
- Sarah tool-call schema and runtime event ordering; and
- package pins, compatibility fixtures, route table, and OpenAPI publication.

Each hot contract has one integration owner before parallel mutation.

### Integration ownership freeze

| Hot contract                                              | Integration owner                        | Package or path                                                                                                                               |
| --------------------------------------------------------- | ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| native identity, command, event, receipt, lifecycle model | SBX-00                                   | `packages/managed-sandbox-contract/**`                                                                                                        |
| root and Sarah action authority                           | SBX-00 for policy and SBX-07 for runtime | `AUTHORITY.md`, `docs/authority/SARAH_AUTHORITY.md`, `packages/authority/**`                                                                  |
| durable command/event/idempotency store                   | SBX-01                                   | `packages/khala-sync-server/**` and its Cloud SQL migrations                                                                                  |
| GCP target, image, provisioner, and reconciliation        | SBX-02                                   | `crates/oa-codex-control/**`, `crates/oa-node/**`, `crates/oa-workroomd/**`                                                                   |
| Box-v1 HTTP/SDK compatibility adapter                     | SBX-03                                   | `packages/ai-sdk-sandbox-openagents/**` and `apps/openagents.com/workers/api/**`. Native types stay in `packages/managed-sandbox-contract/**` |
| runtime turn/event/interrupt semantics                    | SBX-04                                   | `crates/oa-workroomd/**`, Cloud control adapter, native event ingest                                                                          |
| file/command/artifact policy and quota                    | SBX-05                                   | `crates/oa-workroomd/**`, shared artifact/receipt contracts                                                                                   |
| IDE placement and agent-graph consumer                    | SBX-06                                   | `apps/openagents-desktop/src/ide/**`, shared IDE schemas                                                                                      |
| Sarah tool schema and ordered activity                    | SBX-07                                   | `apps/openagents.com/workers/api/**`, Sarah mobile projection                                                                                 |
| bounded mobile/web controller                             | SBX-08                                   | `apps/openagents-mobile/**`, authenticated `openagents.com` projections                                                                       |
| live rollout/evidence and cleanup oracle                  | SBX-09                                   | `scripts/cloud/**`, `docs/sol/evidence/**`, GCP deployment config                                                                             |
| checkpoint/fork/private-ingress Phase 2                   | SBX-10                                   | native contract plus exact Cloud/guest paths selected by its later claim                                                                      |

Writers may add adapters in their row.
They must not move another row's domain identity or relax its invariants.
Such a change must update this ledger, the relevant `INVARIANTS.md`, the
AssuranceSpec, and deterministic tests.

## Definition of done

The owner can create one managed sandbox from Desktop and one through Sarah.
The owner can run and observe long-running Codex and Claude work.
The owner can reconnect to ordered events and interrupt or settle an exact turn.
The owner can use bounded files, commands, and artifacts.
The owner can stop, resume, and delete the sandbox, then verify zero residue.
The unmodified Box SDK completes the same admitted lifecycle against the
OpenAgents service.

Mobile and authenticated web supervise without gaining runtime authority.
Every action has exact receipts.
Every unsupported Box method refuses explicitly.
Spend remains within the standing cap, rollback is proven, and no public claim
outruns the promise gate. Phase 2 then closes only after
checkpoint, fork, and private desktop have their own live fault evidence.

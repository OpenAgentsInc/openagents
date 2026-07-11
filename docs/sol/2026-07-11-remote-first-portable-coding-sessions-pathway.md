# Remote-first portable coding sessions: pathway and roadmap gap analysis

- Date: 2026-07-11
- Status: owner-directed product-path amendment; implementation pending
- Program parent: [#8566](https://github.com/OpenAgentsInc/openagents/issues/8566)
- Canonical sequencing: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)
- Cloud authority: [`../cloud/README.md`](../cloud/README.md)
- Mobile capability ledger:
  [`2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md)

## Owner request

The product outcome stated on 2026-07-11 is:

1. **“Remote-first, not local-first. Sessions can be stopped on any machine
   and moved to any other, local or remote. i.e. handoff to cloud.”**
2. **“Remote sessions on my own cloud (my homelab) OR a managed cloud (e.g.
   Daytona)”**
3. **“Secrets access via a broker (i.e. gondolin or agyn style)”**
4. **“Mobile client which can access any session on any host, with
   conversational voice”**

These are product behavior expectations, not merely competitive observations.
They are registered as pending contracts in
`packages/behavior-contracts/src/openagents-apps.ts` until executable oracles
replace the planned entries.

## Direct answer

No, the current roadmap does not yet reflect all four outcomes.

| Requested outcome | Current coverage | Missing product contract |
| --- | --- | --- |
| Stop on one machine and continue on another | **Partial.** Khala Sync, #8676, R2/R4, and the workroom session runner cover cross-device client continuity, durable replay, restart, and multi-turn work on one host. | A host-independent coding-session identity, exclusive execution attachment, portable checkpoint, detach/attach/move/failback state machine, and cross-host acceptance proof. |
| Own cloud or managed cloud | **Partial.** Owner-local Pylons and OpenAgents-managed Agent Computers are P0; `oa-node` already models owned/org-owned machines. | A supported homelab/customer-cloud enrollment path, a provider-neutral target adapter, and a separately audited managed-provider adapter such as Daytona. |
| Brokered secrets | **Partial.** Codex account grants, SCM grant refs, link-local gateways, broker-only Agent Computer policy, and the BYO credential design are strong narrow substrate. | One production general secret-capability lease across provider, SCM, MCP/tool, and API credentials, with target re-redemption, revocation, and wipe proof on every target class. |
| Mobile access to any host with voice | **Split.** Mobile remote coding and supervision are central to R6/R7. | An authorized global session/host catalog and attach/move controls. Conversational voice is currently closed by Revision 29 and must be narrowly re-authorized without reviving Sarah/avatar/video. |

The existing roadmap is therefore the right foundation, but not the complete
pathway. The new work is a portability layer over the current Thread/Turn/Item,
WorkContext, Khala Sync, Pylon, Fleet, workroom, and receipt contracts—not a
replacement fleet or a second session database.

## What “remote-first” means here

Remote-first is an authority and portability rule:

- the canonical coding-session identity is independent of the client, host,
  process, workspace path, Pylon home, provider adapter, or current placement;
- a local machine is one execution target, not the place that defines the
  session;
- every authorized client can discover the same durable session facts and
  request typed control operations;
- execution can quiesce, checkpoint, detach, and rehydrate on a compatible
  target under an exclusive generation-fenced lease; and
- secrets, raw host handles, process state, sockets, and credentials are never
  the portable state.

This amendment does **not** remove the local-only identity/offline tier. A user
may still work without an OpenAgents account when work never leaves a device.
It changes the normal coding-session model from “a local process that may be
viewed remotely” to “a durable remotely addressable session that may currently
execute locally.” Cross-device or cross-host use requires an account or an
equivalent explicitly paired owner authority; local-only work remains local
until the owner adopts it into that authority.

Version 1 should promise **stop/checkpoint/rehydrate**, not transparent live
process-memory migration. Provider hidden state, live PTYs, OS processes,
sockets, and arbitrary in-memory tool state cannot honestly move. Their durable
typed facts can.

## Target architecture

```text
Desktop / mobile / CLI
        |
        | typed session commands + bounded projections
        v
Khala Sync + session directory + command/outcome authority
        |
        | placement policy selects one compatible target
        v
exclusive session attachment generation
        |
        +--> local Pylon
        +--> owner-managed remote Pylon/oa-node (homelab/customer cloud)
        +--> OpenAgents Agent Computer (managed Firecracker/GCE)
        +--> audited managed-provider adapter (for example Daytona)
        |
        v
capability broker reissues target-scoped leases
        |
        v
runtime executes -> events/checkpoints/receipts -> Khala Sync
```

Clients never call a cloud vendor directly. Provider adapters implement one
OpenAgents lifecycle/capability contract; the session directory projects stable
OpenAgents refs and honest isolation/capability state.

## Existing substrate to preserve

| Existing substrate | What it already contributes | Boundary that remains |
| --- | --- | --- |
| Khala Sync and Runtime Gateway | Stable cross-device refs, confirmed projections, durable commands/outcomes, cursor recovery, and tokenless clients. | Provider-native local history is not portable session authority; a host catalog and session attachment projection are still missing. |
| #8676 native conversation handoff | One real Desktop stream continuing on physical mobile with the same thread/run/message refs. | This is client handoff around one execution attachment, not execution migration. |
| `oa-workroomd codex session` | Multi-turn workspace preservation, event cursors, pause/continue/closeout/archive/destroy, and per-turn auth scrub on one host. | State is bound to one local `state-dir`; no export/import, attachment fencing, target rebind, or cross-host checkpoint contract exists. |
| #8547 Agent Computer | Broker-only provider execution, isolated scratch homes, workroom lifecycle, exact usage, writeback, and reclaim target. | The real mobile-originated Firecracker acceptance receipt is still open. |
| #8636 hybrid routing | One claim registry and explicit owner-local versus managed target selection/fallback. | It places work units; it does not move an existing session or enroll an owner-managed remote host. |
| `oa-node` / `oa-codex-control` / `oa-workroomd` | Owned/org-owned node daemon, control plane, managed capacity, workroom sidecar, and redacted receipts. | No supported customer enrollment, outbound connectivity, host catalog, ownership/attestation, or product acceptance for a homelab node. |
| Codex auth grants and BYO credential design | Short-lived refs, per-session materialization, endpoint-encrypted BYO transfer, volatile slots, revocation, wipe, and redacted receipts. | Codex materialization is explicitly a compatibility bridge; BYO is design-only and intentionally not a generic secret tunnel. |

The in-repo Cloud control plane is deployed substrate. That must not be
reported as accepted remote coding: the live Cloud-VM lane remains env-gated,
and the full mobile-dispatched Firecracker, writeback, exact-token, and reclaim
definition of done has not yet passed.

## Contract additions

### 1. Host-independent coding session

Add a canonical `coding_session` contract whose identity never contains or
derives from a hostname, local path, process ID, vendor session ID, or current
workroom directory.

Minimum durable fields:

```text
session_ref
owner_scope_ref
thread_ref
work_context_ref
repository_ref + pinned_base_ref
run_ref / fleet_ref where applicable
current_attachment_ref
current_checkpoint_ref
event_cursor
state
created_at / updated_at
```

The session is the product object. An executor process, provider-native thread,
Pylon assignment, workroom, or VM is an attachment beneath it.

### 2. Exclusive session attachment

Add a `session_attachment` generation/lease that names the current target,
runtime, isolation profile, compatibility set, worker epoch, and fencing token.
At most one generation may accept new execution commands.

Minimum state machine:

```text
ready -> running -> quiescing -> checkpointing -> detached
detached -> attaching -> ready
quiescing/checkpointing/attaching -> failed
failed -> ready_on_source | detached | terminal
```

Typed commands:

- `session.quiesce`
- `session.checkpoint.create`
- `session.detach`
- `session.attach`
- `session.move`
- `session.move.abort`
- `session.resume`
- `session.stop`

Every command uses a stable client-chosen idempotency key and receives one
durable `accepted | rejected | failed | unknown_pending_reconcile` outcome.
A stale source generation is fenced before the target accepts work.

### 3. Secret-free portable checkpoint

Add a content-addressed, versioned `session_checkpoint` with compatibility and
integrity checks.

Portable state may include:

- canonical Thread/Turn/Item and work-unit/event cursors;
- admitted WorkContext and authority-policy refs;
- pinned repository base, exact diff/post-image, untracked-file manifest, and
  worktree metadata;
- plan, task, approval, command, verification, artifact, and receipt refs;
- model/tool/plugin/MCP catalog generation refs;
- bounded provider continuation refs when the provider supports them; and
- checkpoint schema, component compatibility, and isolation requirements.

Portable state must never include:

- provider, SCM, API, MCP, database, or wallet secrets;
- raw auth caches, bearer tokens, credential-helper output, or encrypted secret
  blobs intended for another target;
- raw host paths, PIDs, sockets, PTY handles, VM topology, or SSH keys;
- live process memory or a claim that a nonportable process continued; or
- private material outside the owner's checkpoint policy.

The target validates the checkpoint and redeems new capability leases before
it becomes ready. Source cleanup and target activation each emit a receipt.

### 4. Provider-neutral execution target

Replace the overloaded `owner_local | managed_remote | auto` product choice
with a typed target descriptor. Policy may still expose a simple user choice,
but the durable record separates:

```text
custody: owner | openagents | third_party
location: local | remote
runtime_kind: pylon | openagents_agent_computer | provider_adapter
target_ref
capability_manifest_ref
isolation_profile_ref
compatibility_set_ref
health / freshness / capacity / quota
```

Required adapters:

1. **Local Pylon** — current-machine execution.
2. **Owner-managed remote node** — homelab or customer cloud, enrolled with a
   revocable device/runtime identity, outbound-safe connectivity, signed
   updates, capability/health publication, and no public topology projection.
3. **OpenAgents-managed Agent Computer** — current #8547 Firecracker/GCE path.
4. **Managed-provider adapter** — for example Daytona, admitted only after an
   isolation, credential, snapshot, network, preview, cleanup, pricing, and
   receipt audit. Vendor APIs remain behind the adapter.

Selection and fallback are explicit durable facts. A change of provider,
custody, region, account, isolation rung, or data posture is never silent.

### 5. General secret-capability broker

Promote the existing narrow grants into a general capability lease rather than
turning the BYO credential channel into an arbitrary secret tunnel.

The broker contract covers:

- custody adapters: owner-local OS vault/broker, OpenAgents Secret Manager,
  and separately approved external stores;
- opaque secret refs; raw values never enter clients or Sync;
- exact owner, session, attachment generation, target, tool, operation,
  audience, environment-name, and TTL scope;
- optional owner approval and policy digest;
- target attestation or compatibility evidence before redemption;
- proxy/gateway use when a workload does not need raw material;
- one-shot or short-lived JIT materialization when a runtime requires a file,
  credential helper, or environment variable;
- access, denial, renewal, revocation, release, and wipe receipts; and
- fail-closed replay defense after detach, move, timeout, or reclaim.

Moving a session revokes or expires the source attachment's grants and mints
new target-scoped grants. Secret bytes never ride inside a checkpoint. Static
operator token fallbacks remain development/break-glass paths and cannot satisfy
the R7 product acceptance.

### 6. Authorized host/session directory

Khala Sync needs owner-scoped projections for enrolled targets, authorized
sessions, current attachments, capabilities, freshness, isolation, pending
attention, and durable control outcomes.

Mobile and Desktop can then answer:

- what sessions exist;
- where each session is attached;
- whether it is running, quiescing, detached, moving, stale, or failed;
- what the target can safely do;
- whether a checkpoint is current and compatible;
- what command or approval needs attention; and
- what move, stop, resume, or fallback outcome actually occurred.

Local provider-native sessions do not become network-visible implicitly. The
owner explicitly adopts/shares them into the session authority, with a clear
statement of what history, repository state, and evidence will synchronize.

### 7. Session-neutral conversational voice

Conversational voice is re-authorized only as a mobile input/output and control
modality over the same typed session protocol. It does not revive Sarah,
avatars, video, persona routing, ambient recording, or a voice-only authority
path.

The first useful rung is:

- explicit microphone permission and visible recording/listening state;
- push-to-talk or tap-to-speak with provisional/final ASR;
- editable transcript before commit where ambiguity or consequence warrants;
- typed submit, steer, queue, interrupt, approve, reject, move, resume, and stop
  intents through the normal command registry;
- streaming TTS of canonical assistant text or bounded session summaries;
- barge-in mapped to the typed interrupt/steer contract;
- text-first fallback for every action; and
- no raw audio retention by default.

Voice is never command or completion authority by itself. Destructive, spend,
credential, writeback, permission-widening, and isolation-downgrade actions
still require the normal visible confirmation and durable outcome.

## Ordered pathway

These are bounded work packets to add beneath #8566. They need live issue
leaves and claims before implementation; the existing #8676, #8547, and #8636
issues must not be silently broadened to pretend they already own all of this.

| Order | Packet | Outcome | Exit proof |
| ---: | --- | --- | --- |
| 0 | Intent and invariants | ProductSpec, behavior contracts, target vocabulary, session/checkpoint schemas, and threat/model boundary are frozen. | Schema/architecture tests reject host-derived session identity, two live attachments, secrets in checkpoints, and silent target/isolation changes. |
| 1 | Durable session authority | `coding_session`, attachment generation, checkpoint metadata, host/session directory, and command outcomes land in the canonical request processor and Khala Sync. | Local fixture survives restart, lost ACK, duplicate move, and stale source generation without duplicate execution. |
| 2 | General broker | Provider/SCM/tool secret leases work across local and managed targets; source grants revoke and target grants reissue on move. | Revoke during move, replay, log/snapshot scan, timeout, and wipe tests pass with no raw secret projection. |
| 3 | First real move | Move a bounded session from local Pylon A to the accepted #8547 Agent Computer and back by checkpoint/rehydrate. | Same session/thread/run refs, exact checkpoint/diff digest, one live attachment, new target grants, source cleanup, and no duplicate accepted turn. |
| 4 | Owner-managed remote target | Ship enroll/update/revoke/connect for a homelab/customer-cloud Pylon or `oa-node`. | Local → owner-managed remote → local move passes with explicit health, isolation, compatibility, and cleanup receipts. |
| 5 | Managed-provider adapter | Audit and implement one provider adapter such as Daytona behind the same contract. | Provider-specific provision/snapshot/ports/exec/teardown behavior maps to OpenAgents receipts; downgrade and cleanup failures are visible and fail closed. |
| 6 | Mobile any-host control | Mobile session directory, target detail, stop/checkpoint/move/resume/failback, push/deep links, and attention views consume the shared contract. | Physical iOS/Android clients control sessions on each enrolled host class without host paths, tokens, or vendor APIs. |
| 7 | Conversational voice | Persona-neutral ASR/TTS/barge-in drives the same typed commands and outcomes. | Physical-phone voice follow-up plus interrupt/move/resume works under reconnect; text remains available and no audio is retained by default. |
| 8 | R7 dogfood | Sustained owner use across at least two host classes, including faults, updates, revocation, and rollback. | Signed owner-accepted receipt with zero forked session identity, duplicate accepted work, secret leak, silent substitution, or orphaned source runtime. |

Packets 1 and 2 are the critical path. Provider breadth and UI polish must not
race ahead of the session-fencing and secret-reissue laws.

## Acceptance journey

R7 should include this exact class of journey:

1. On mobile, select an authorized repository and a session currently attached
   to an owner homelab node.
2. Use voice to ask for status and submit a safe follow-up; observe the typed
   transcript and durable accepted outcome.
3. Request **Move to managed cloud**. The source quiesces, creates a
   secret-free checkpoint, loses execution authority, and produces cleanup and
   revocation evidence.
4. The managed target validates compatibility, redeems fresh scoped provider
   and SCM grants, rehydrates the same session/work context, and becomes the
   only live attachment generation.
5. Mobile receives progress through the same refs. Desktop opens the same
   session and inspects the exact diff, event cursor, target history, and
   receipts.
6. Revoke one secret mid-turn, lose one acknowledgement, restart the mobile
   app, and prove a typed failure/reconciliation rather than duplicate work or
   false success.
7. Move the session to another authorized owner-managed host or fail back to
   the source. Verify the same session/thread/run identity and exact repository
   post-image.
8. Stop and reclaim. Prove processes, scratch, ports, leases, and secret slots
   are gone and that stale generations cannot resume.

## New invariants

Implementation must register and test these before claiming portability:

1. Session identity is never derived from the current host, path, process, or
   provider-native session ID.
2. At most one attachment generation accepts new execution commands.
3. Stale hosts and lost-ACK retries cannot reacquire authority without an
   explicit fenced transition.
4. An accepted command remains durable across detach and either completes once
   or reaches an honest terminal/reconcile state.
5. A checkpoint is content-addressed, complete, schema-compatible, and
   integrity-verified before source authority is released or target authority
   is granted according to the selected move policy.
6. Secrets, credentials, raw host handles, and live process state never enter
   checkpoints, Sync projections, prompts, or public receipts.
7. Every target attachment reauthorizes and re-redeems capabilities; a move
   never copies a source credential cache as portable state.
8. Target, provider, custody, account, isolation, data posture, and fallback
   changes are explicit and never silently substituted.
9. Source quiesce, grant revocation, cleanup, target attach, and reclaim each
   produce durable redacted evidence.
10. Microphone use is explicit, visible, revocable, and defaults to no raw
    audio retention.
11. Speech becomes a typed intent through the same policy, approval,
    idempotency, and receipt path as text. TTS and model prose are never
    authority.
12. Existing local-only sessions remain private until an owner explicitly
    adopts them into cross-device/session-directory authority.

## Non-goals for the first accepted version

- transparent migration of live process memory, PTYs, sockets, or hidden model
  state;
- a mobile remote-desktop pixel stream;
- direct Daytona, GCP, SSH, or homelab APIs in mobile/renderer code;
- copying provider auth homes or `.env` files between hosts;
- a generic arbitrary-secret tunnel;
- silently lowering VM/container isolation to host execution;
- public pooled access to an owner's homelab;
- Sarah, avatar, video, ambient microphone capture, or voice persona work; and
- claiming managed-provider support before a real adapter passes isolation,
  secret, snapshot, teardown, and receipt acceptance.

## Roadmap reconciliation required

Revision 30 of the master roadmap must:

- distinguish client continuation from execution-session portability;
- make host-independent session identity and checkpoint/attachment fencing an
  R3/R4/R7 requirement;
- expand the execution target taxonomy to owner-managed remote and audited
  managed-provider adapters;
- promote a general broker as a prerequisite to cross-host movement;
- require an authorized any-host session directory on mobile; and
- carve persona-neutral conversational voice out of the Sarah/avatar/video
  wontdo boundary.

The mobile port ledger must likewise move native PTT/STT from “paused” to a
bounded P0 modality over the shared session command contract, while keeping
legacy app code and Sarah media paths frozen as extraction evidence only.

This document is the pathway and gap record. Current code, live issues,
invariant ledgers, tests, deployments, and receipts remain the authority for
what has actually shipped.

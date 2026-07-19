# Remote-first portable coding sessions: pathway and roadmap gap analysis

- Date: 2026-07-11
- Amended: 2026-07-13 — ENV-1
  ([#8778](https://github.com/OpenAgentsInc/openagents/issues/8778)) adopts
  the ExecutionEnvironment / KnownEnvironment / AccessEndpoint /
  AdvertisedEndpoint vocabulary as this pathway's canonical language. This
  amendment renames and clarifies authority language only; no contract,
  requirement, or invariant was weakened, removed, or restated.
- Reconciled: 2026-07-19 — managed-sandbox epic
  [#9023](https://github.com/OpenAgentsInc/openagents/issues/9023) now admits a
  concrete OpenAgents-managed GCP `SandboxResource` for new work. It does not
  reopen the closed PORT issue ledger, prove session movement/failback, or
  convert sandbox stop/resume into process-memory portability. The
  [`managed-sandbox accepted plan`](./2026-07-19-managed-agent-sandboxes-accepted-plan.md)
  owns that separate implementation program.
- Class: contract
- Dispatch: yes; PORT-00 through PORT-08 are live as #8745–#8753
- Owner: Sol portable sessions
- Status: owner-directed product path; ordered implementation ledger active
- Program parent: [#8566](https://github.com/OpenAgentsInc/openagents/issues/8566)
- Canonical sequencing: [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md)
- Local coding cutover graph:
  [`2026-07-11-openagents-coding-cutover-issue-plan.md`](./2026-07-11-openagents-coding-cutover-issue-plan.md)
- Cloud authority: [`../cloud/README.md`](../cloud/README.md)
- Mobile capability ledger:
  [`2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md)
- Product calibration:
  [`transcript 248`](../transcripts/248.md),
  [`transcript 249`](../transcripts/249.md),
  [closed #8674 history contract](./issues/desktop-codex-subagent-history.md),
  and [closed #8675 real-Electron acceptance](./issues/desktop-codex-trace-acceptance.md)

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

The roadmap reflects all four outcomes as pending product contracts. The live
issue ledger now covers them as #8745–#8753; implementation and acceptance
evidence remain pending and must advance packet by packet.

| Requested outcome | Current coverage | Missing live ownership and proof |
| --- | --- | --- |
| Stop on one machine and continue on another | **Partial.** Khala Sync, #8676, R2/R4, and the workroom session runner cover cross-device client continuity, durable replay, restart, and multi-turn work on one host. | A host-independent coding-session identity, exclusive execution attachment, portable checkpoint, detach/attach/move/failback state machine, and cross-host acceptance proof. |
| Own cloud or managed cloud | **Partial.** Owner-local Pylons and OpenAgents-managed Agent Computers are P0; `oa-node` already models owned/org-owned machines. | A supported homelab/customer-cloud enrollment path, a provider-neutral target adapter, and a separately audited managed-provider adapter such as Daytona. |
| Brokered secrets | **Partial.** Codex account grants, SCM grant refs, link-local gateways, broker-only Agent Computer policy, and the BYO credential design are strong narrow substrate. | One production general secret-capability lease across provider, SCM, MCP/tool, and API credentials, with target re-redemption, revocation, and wipe proof on every target class. |
| Mobile access to any host with voice | **Split.** Mobile remote coding and supervision are central to R6/R7. Earlier owner direction closed persona/presentation work broadly; current direction narrowly authorizes session-neutral voice. | An authorized global session/host catalog and attach/move controls plus executable ASR/TTS/barge-in acceptance without reviving Sarah/avatar/video. |

The existing roadmap is therefore the right foundation, but not the complete
pathway. The new work is a portability layer over the current Thread/Turn/Item,
WorkContext, Khala Sync, Pylon, Fleet, workroom, and receipt contracts—not a
replacement fleet or a second session database.

The local cutover graph #8681–#8707 is a prerequisite product milestone, not a
substitute for this pathway. It proves installed OpenAgents Desktop can replace
the ordinary Codex and Claude Code UI while physical mobile continues and
supervises the same local-host sessions. Its CUT-27 acceptance explicitly
excludes workrooms, host movement, managed providers, and voice so those claims
remain pending here instead of being inferred from a successful local cutover.

### Landed command-convergence foundation

CUT-07 [#8687] now supplies the command primitive this pathway must reuse:
stable semantic intent IDs over the durable Khala Sync mutation ledger,
byte-equivalent retry reconciliation, conflicting-ID rejection, confirmed
cross-device outcomes, and server-clock terminal expiry that cannot later
dispatch. See the
[`CUT-07 receipt`](./2026-07-11-cut-07-command-convergence-receipt.md).

Portable-session commands must extend that identity/outcome model instead of
creating a move-specific retry ledger. Their target grants, attachment
generations, and checkpoints add authority fields, but lost ACK, duplicate,
reconnect, restart, and expiry semantics remain the same shared contract.

CUT-08 [#8688] adds the matching projection floor: clients refuse sparse
advancing scope versions, replay from the durable cursor, replace an exact
scope on MustRefetch, and refuse a future local-store schema before mutation.
Portable graph/checkpoint projections must reuse those cursor and compatibility
semantics; a host move cannot treat a missing child/event version as a valid
partial continuation.

CUT-09 [#8689] now supplies the deterministic lifecycle fence this pathway
must extend: a superseded client generation cannot apply a delayed transport
response; runtime events must match the durable next sequence and current turn
state; terminal/revoked turns reject late provider output; and an abandoned
hosted worker generation becomes one durable interrupted terminal without
replaying inference. A future attachment generation is therefore not merely a
display field—it must participate in the same pre-mutation authority check.
Cross-host move/failback still requires its own attachment/checkpoint model and
live receipt. CUT-09's physical-mobile close rung remains pending and is not a
portability claim.

CUT-15 [#8695] now supplies the local interaction convergence layer that
portable controls must reuse. One canonical Desktop registry drives visible
actions, palette entries, editable conflict-safe keybindings, native menus,
deep links, second-instance opens, and restored routes through the same typed
intent and readiness/authorization gates. Its owner-private override store and
visible duplicate rejection are local UX mechanics; neither is durable session
command authority. CUT-16 composer/runtime controls must name their semantic
intents here, then use CUT-07 outcomes for consequential cross-device work
rather than adding provider-specific shortcuts or URL authority.

CUT-16 [#8696] now supplies the first portable composer/interaction substrate:
one private ref-bound rich draft, durable continue/retry/close controls, and a
provider-neutral question/tool-approval/plan-review lifecycle whose exact
decision, duplicate, conflict, expiry, revocation, and live-only projection
semantics are shared by Desktop and mobile clients. Native device-local draft
recovery, Desktop protocol-v9 transport, and mobile interaction controls are
now active. Mobile cancel, resume, retry, and close also bind the exact
confirmed run and provider lane, remain disabled while reconciling, and require
the matching durable command plus a newer run projection before success.
Desktop renders the confirmed interaction projection and routes exact-ref
decisions through gateway-event-driven confirmation; an enqueue receipt never
collapses a card as resolved. This does not yet make provider execution fully
proven portable: the canonical rich draft is not yet fully adopted by both host
UIs. Mobile now restores it by exact session/thread identity, refreshes ref-only
repository/worktree context and confirmed runtime target readiness, renders
restored attachment metadata, persists edits/accepted clears, and withholds
Send without disabling offline editing when the target is unavailable. Mobile
native file/image acquisition now uses the SDK picker, bounds and hashes bytes,
copies them into the durable app sandbox, and places only content-addressed
ready metadata in the canonical draft. Attachment-bearing runtime delivery,
real provider/model/account selectors, editor/diff capture, and equivalent
Desktop adoption remain. Owner-local Pylon now requests
the same durable authority through the trusted Worker seam and runs Claude's
`canUseTool` only under explicit confirmed supervision, but named Codex,
deployed-authority, and physical receipts remain open. Provider defaults remain
unchanged when that authority is absent.

CUT-17 [#8697] now starts the local workspace capability layer remote-first
sessions will depend on without confusing a host path for portable identity.
The main-process WorkContext exposes new recursive tree and search projections
only through an opaque grant ref and relative path refs, with bounded results,
declared cache epochs, ignore/secret/binary/symlink filtering, and exactly-owned
watcher invalidation. Fixed decoded tree/refresh/watch main-preload operations
now carry that boundary to the trusted bundled renderer and rebind on explicit
WorkContext replacement. This is host-capability evidence only: Effect Native
UI and portable workspace materialization remain separate rungs. Bounded
path/content search now executes in a WorkContext-owned cancellable worker per
task, with stale-epoch fencing and relative-ref-only decoded results. Fixed
main/preload search/cancel operations now bind one task to the exact
webContents/request ref and close with that owner; the Effect Native UI remains
the next host rung. Root-private relative-ref create/rename/non-recursive-delete
and reveal capabilities now also exist with revision and permission fencing;
their fixed decoded bridge is host-active with main-only reveal authority. The
UI remains separate from portable materialization.
The named real-worker scale receipt now covers a 20,000-file bounded traversal,
current-epoch cache replay, and zero-active project close; it is host-capability
evidence, not portable workspace materialization evidence.

CUT-18 [#8698] now supplies the host-local editor state machine that future
portable checkpoints can adopt without adopting host authority. Effect Native
owns bounded relative tab refs, revision refs, drafts, selection, language,
find/history, dirty/save state, and explicit external-change/conflict outcomes;
the replaceable editor host receives only serializable props and emits typed
events. Create-only Save As never overwrites, confirmed file/folder renames
retarget matching open tabs without losing drafts, and revoked/missing/binary/
large/encoding/permission outcomes remain explicit. Renderer-reload recovery is
keyed by an opaque coding-session ref but deliberately persists only bounded
relative refs, revisions, and drafts—never a workspace root or grant—and
reopens every tab through the current WorkContext grant before deciding whether
the draft is current, changed, or missing. This is the correct shape for a
future secret-free portable editor checkpoint, but the current storage is
device-local `localStorage`; it is not Khala Sync authority, cross-host
materialization, or proof that a provider process moved.

CUT-19 [#8699] adds the matching host-local review/context boundary. Git status
now produces opaque repository, HEAD, and exact index/worktree snapshot refs;
diff or discard must echo the matching identity and becomes stale after any
concurrent change. Only bounded non-binary, non-secret-shaped unified diffs and
typed hunks can reach the renderer. A user may explicitly attach one reviewed
diff to the next composer turn, where it is labeled untrusted data and remains
removable until submit. This is useful checkpoint shape—relative path, status
identity, bounded content—but not portable authority: the attachment is
renderer-local, Git identity is tied to the current materialization, and raw
diffs are neither uploaded to Sync nor implied to exist on another target. A
future portable checkpoint must re-materialize the repository, verify its
canonical revision, and regenerate/redact review context under the destination
grant rather than copying host paths or trusting a stale source snapshot.

### Episodes 248–249 calibration

Remote-first must preserve the predictability and supervision target. Closed
#8674/#8675 prove metadata-first predictable owner-local history, complete
topology, Desktop navigation, and a real-Electron trace journey. Subsequent
landed Desktop code/oracles add the causal inline child card and structured
handoffs. Live Sync, mobile tap parity, and portable-graph acceptance remain
pending. Newest roots are the default disclosure, not a 24-hour retention
ceiling, and children stay attached to their root.

No pending behavior-registry entry yet covers live child cards, portable graph
fencing, or pointer/keyboard/tap equivalence. Packet 0 below must add and test
that versioned contract before implementation claims the promise.

Raw provider JSONL and local history remain private until explicit owner
adoption; adoption projects only bounded canonical facts. A move must preserve
canonical agent identity, topology, independent transcript/activity cursors,
and honest gaps without promoting a sampled preview or live socket into outcome
authority.

## What “remote-first” means here

Remote-first is an authority and portability rule:

- the canonical coding-session identity is independent of the client, host,
  process, workspace path, Pylon home, provider adapter, or current placement;
- a local machine is one execution target — one ExecutionEnvironment among
  others in the ENV-1 vocabulary below — not the place that defines the
  session;
- every authorized client can discover the same durable session facts and
  request typed control operations;
- execution can quiesce, checkpoint, detach, and rehydrate on a compatible
  target under an exclusive generation-fenced lease, including every active
  descendant owned by that attachment; and
- secrets, raw host handles, process state, sockets, and credentials are never
  the portable state.

This amendment does **not** remove the local-only identity/offline tier. A user
may still work without an OpenAgents account when work never leaves a device.
It changes the normal coding-session model from “a local process that may be
viewed remotely” to “a durable remotely addressable session that may currently
execute locally.” Cross-device or cross-host use requires an account or an
equivalent explicitly paired owner authority; local-only work remains local
until the owner adopts it into that authority.

Remote-first also does not make local discovery slower or less private. A cold
client still paints its shell and authorized top-level session metadata before
checkpoint, transcript, or child-detail hydration. Provider-native local
history remains read-only local evidence unless the owner explicitly adopts a
bounded typed projection into network authority.

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

### Environment and endpoint vocabulary (ENV-1)

This pathway's canonical language for "where a session can run" versus "how a
client reaches it" adapts T3 Code's remote model. Evidence: the
[T3 Code teardown §6](../teardowns/2026-07-13-t3-code-teardown.md) and the
read-only reference clone
`projects/repos/t3code/docs/architecture/remote.md`. The contract sections
below use these terms; earlier ad hoc "host", "target", and "connection"
phrasing means the term defined here.

- **ExecutionEnvironment** — one running runtime instance with a stable
  identity: a local Pylon, an owner-managed remote Pylon/`oa-node`, an
  OpenAgents Agent Computer, or an audited managed-provider workspace. It
  owns provider availability and auth state, work contexts, terminals, git,
  filesystem access, and runtime settings on that target. Every execution
  target in §4 names exactly one ExecutionEnvironment. **OpenAgents
  strengthening:** environment identity is owner-scoped and receipted — an
  environment ref binds to the owner scope that enrolled it and to its
  enrollment/health/update receipts, never to a bare hostname, address, or
  process.
- **KnownEnvironment** — a client-local saved entry for an environment that
  device knows how to reach (a saved LAN URL, a paired desktop, an enrolled
  homelab node's saved record). It is device-local convenience state, never
  authority: a known environment may not learn the environment identity
  until first successful connect, and it never substitutes for the
  owner-scoped directory in §6.
- **AccessEndpoint** — one concrete way to reach an environment: direct
  ws/wss, a tunneled/relay route, or a desktop-managed SSH forward that
  resolves to an ordinary local forwarded URL. One environment may have many
  endpoints; remoteness lives at the connection layer and never splits the
  runtime into a different kind of environment.
- **AdvertisedEndpoint** — a server- or desktop-authored endpoint *hint*
  carrying a reachability class (loopback / LAN / private / public / tunnel)
  and hosted-HTTPS compatibility flags. Hints are never proof of
  reachability; the final connection attempt decides. Endpoint providers
  (Tailscale first) are plugins that contribute normalized advertised
  endpoints without entering the core environment model. **OpenAgents
  strengthening (deferred):** endpoint records will additionally carry auth
  capability metadata — which scoped, DPoP-bound capability token a client
  must present — but that belongs to ENV-2's capability-token lane, not this
  vocabulary adoption.

### Access versus launch are separate questions

**Access** answers: how does an authorized client speak to a running
ExecutionEnvironment? Direct WebSocket/HTTPS, a tunneled/relay route, or a
desktop-managed SSH port-forward are access methods; each is just another
AccessEndpoint to the same environment.

**Launch** answers: how did the environment come to exist on the target
machine? A pre-existing enrolled runtime, control-plane provisioning of an
Agent Computer or managed-provider workspace, a desktop-managed remote launch
over SSH, or publishing a local runtime through a tunnel are launch methods.
Launch metadata may inform reconnect and lifecycle UX, but it never changes
the environment's identity or the session protocol.

Neither question is session movement. **OpenAgents strengthening:** moving a
session between environments is a receipted authority transfer through the §2
attachment generation and §3 checkpoint contracts — quiesce, checkpoint,
fenced detach/attach, grant revocation and reissue, cleanup receipts — never
a client-side bookmark edit. Switching which KnownEnvironment entry or
AccessEndpoint a client uses reconnects that client to the same attachment;
it neither moves the session nor transfers execution authority.

## Existing substrate to preserve

| Existing substrate | What it already contributes | Boundary that remains |
| --- | --- | --- |
| Khala Sync and Runtime Gateway | Stable cross-device refs, confirmed projections, durable commands/outcomes, cursor recovery, and tokenless clients. | Provider-native local history is not portable session authority; a host catalog and session attachment projection are still missing. |
| CUT-17/CUT-18 WorkContext and editor recovery | Grant-scoped relative workspace capabilities; bounded tree/search/mutations; typed tabs/drafts/revisions/conflicts; current-grant reconciliation after renderer reload without persisting a root or grant. | Recovery remains device-local and assumes the same host WorkContext. Portable checkpoints still need Sync authority, materialization policy, attachment fencing, and target rebind. |
| CUT-19 Git review and composer context | Opaque repository/status fencing, bounded redacted typed hunks, stale-change refusal, confirmed tracked-file discard, and explicit next-turn diff context. | Repository/status refs describe the current host materialization; context is local and must be regenerated under a destination grant before any portable use. |
| #8674/#8675 trace workspace plus subsequent inline-child refinement | Metadata-first shell; recent-first top-level catalog without an age ceiling; complete parent/child/grandchild graph; source-ordered child transcript; later causal inline child card with bounded latest activity; keyboard/accessibility and real-Electron history oracle. | This is owner-local read-only provider history. The #8675 receipt predates the inline-card refinement, and neither is Sync data, live child authority, or portable execution state until explicit adoption and a canonical live projection exist. |
| #8676 native conversation handoff | Deterministic code path for one Desktop stream continuing on mobile with the same thread/run/message refs; the real named-account/physical-phone receipt remains open. | This is client handoff around one execution attachment, not execution migration or a completed live proof. |
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
agent_topology_ref
canonical_event_cursor + canonical_per_thread_activity_cursors
state
created_at / updated_at
```

The session is the product object. An executor process, provider-native thread,
Pylon assignment, workroom, or VM is an attachment beneath it. The session
directory remains rooted at the top-level session; children never become
unrelated root rows. Stable canonical agent refs own parent edges, lifecycle,
and independent transcript/activity cursors. Provider-native child thread or
worker IDs and provider cursors are attachment-generation-local mappings
beneath those refs.

### 2. Exclusive session attachment

Add a `session_attachment` generation/lease that names the current target
ExecutionEnvironment, runtime, isolation profile, compatibility set, worker
epoch, and fencing token. At most one generation may accept new execution
commands.

Attachment authority binds to the environment, never to the route used to
reach it: which AccessEndpoint a client resolves — direct, tunneled, or
SSH-forwarded — is connection-layer state that cannot create, transfer, or
fence an attachment generation.

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

Fencing covers the attachment-owned execution graph, not only the root
process. Before detach, every active descendant must quiesce and checkpoint or
reach an explicit terminal/nonportable state. A move fails closed rather than
leave a source child able to accept work. Independently placed Fleet work is a
separately attached session/work unit linked by a typed external edge; it is
outside the movable attachment-owned descendant graph and cannot disappear
inside the parent move.

### 3. Secret-free portable checkpoint

Add a content-addressed, versioned `session_checkpoint` with compatibility and
integrity checks.

Portable state may include:

- canonical Thread/Turn/Item and work-unit/event cursors;
- stable canonical agent refs, parent edges/depth, child transcript refs,
  source disposition (`quiesced | terminal | nonportable`), effective
  configuration, inter-agent event refs, latest durable event/item ref, and
  canonical per-thread cursors;
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
- live process memory or a claim that a nonportable process continued;
- sampled latest-activity previews or volatile stream buffers presented as
  durable agent state; and
- private material outside the owner's checkpoint policy.

The target validates the checkpoint, maps canonical agent refs to any new
target-native runtime refs, and redeems new capability leases before it becomes
ready. A source `running` lifecycle is never copied as target truth: target
agents transition through explicit `rehydrating`/`ready` states, and every
runtime lifecycle/event is attachment-generation-fenced. The renderer derives
its bounded redacted latest-activity preview from the referenced canonical
event/item. Source cleanup and target activation each emit a receipt.

### 4. Provider-neutral execution target

Replace the overloaded `owner_local | managed_remote | auto` product choice
with a typed target descriptor. An execution target is an
ExecutionEnvironment in the ENV-1 vocabulary: the descriptor records what the
environment is and what it may safely do, never how a client currently
reaches it — AccessEndpoint and AdvertisedEndpoint facts stay at the
connection layer and out of durable target identity. Policy may still expose
a simple user choice, but the durable record separates:

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

`target_ref` is the owner-scoped ExecutionEnvironment identity described in
the ENV-1 vocabulary, bound to enrollment/health receipts rather than to a
hostname or address.

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

**Auth metadata (ENV-2, #8780 — typed contract landed).** The auth-bearing
capability-token language ENV-1 deferred now has a typed home in
`packages/environment-auth`, and the portable-session capability broker
carries its first opt-in proof-of-possession seam:

- capability scopes are drawn from the existing OpenAgents MCP
  authority-class vocabulary (`@openagentsinc/mcp-contract`), never an ad hoc
  or borrowed scope-string set;
- access tokens are minted through an RFC 8693-shaped exchange from an
  environment-bootstrap subject token, and the scope evaluator is
  narrowing-only — an exchange can never widen the subject grant, and a
  request for any scope outside it rejects the whole exchange;
- issued tokens are DPoP-bound (RFC 9449): the client proves possession of
  an ES256 key per request via `htm`/`htu`/`iat`/`jti` claims, the RFC 7638
  JWK thumbprint binds token and grant to that key, freshness is a bounded
  clock window, and a `(thumbprint, jti)` replay cache makes each proof
  single-use;
- a broker lease may be issued with a `clientKeyThumbprint`; redeeming a
  key-bound lease then fails closed without a valid possession proof, before
  any vault or target-adapter access, and reissue can never launder a bound
  lease into an unbound one. Unbound leases keep the existing path — the
  handshake is strictly opt-in per lease.

Fleet-wide socket rollout, Khala Sync server-side grant migration, and
revocation UI remain follow-ups; bearer-shaped development fallbacks remain
break-glass only, exactly as above.

### 6. Authorized environment/session directory

Khala Sync needs owner-scoped projections for enrolled ExecutionEnvironments,
authorized sessions, current attachments, capabilities, freshness, isolation,
pending attention, and durable control outcomes. The directory is the
owner-scoped authority for which environments and sessions exist; a client's
KnownEnvironment entries remain device-local convenience and never substitute
for it. Where the directory projects reachability, it projects
AdvertisedEndpoint hints with their reachability class — hints, never proof
that a route works from the asking device.

Endpoint hints carry auth metadata, not credentials (ENV-2, #8780): the
typed statement of which scoped, DPoP-bound capability grant a client must
present to speak to that environment — the grant's scopes (OpenAgents MCP
authority classes) and its owner-scoped `ExecutionEnvironmentRef` binding,
per `packages/environment-auth`'s `EnvironmentCapabilityGrant`. Grant records
are refs plus key thumbprint only (`tokenMaterial: "excluded"`); raw tokens
and client private keys never enter the directory, Sync, or a client's
KnownEnvironment entries, and possession is proven per request, never stored.

Mobile and Desktop can then answer:

- what sessions exist;
- which ExecutionEnvironment each session is attached to;
- whether it is running, quiescing, detached, moving, stale, or failed;
- what the target environment can safely do;
- whether a checkpoint is current and compatible;
- what command or approval needs attention; and
- what move, stop, resume, or fallback outcome actually occurred.

The directory is a top-level session catalog. Opening a session reveals the
nested canonical agent graph, causal child-start activity, one bounded latest-
durable-activity preview, and direct access to each independent child
transcript. Desktop may use a persistent rail; mobile may use an explicit
drawer/disclosure. Neither may silently cap descendants or reconstruct edges
from prose. Shell and catalog readiness never wait for checkpoint, transcript,
or child-detail hydration; pending, gap, stale, and failed states are explicit
instead of a permanent loading placeholder.

Local provider-native sessions do not become network-visible implicitly. The
owner explicitly adopts/shares them into the session authority, with a clear
statement of what history, repository state, and evidence will synchronize.

### 7. Session-neutral supervision controls

Click, tap, command-palette actions, native menus, and conflict-safe hotkeys
dispatch the same registered intent/result for a given action. Inspect, focus,
and return-to-parent remain local view state unless an explicit continuity
contract says otherwise. Steer, interrupt, move, resume, and stop pass normal
policy, approval, idempotency, and durable outcome gates. Topology remains
visible or one explicit action away while inspecting a child; fast interaction
is a presentation and latency contract, not a second command authority.

### 8. Session-neutral conversational voice

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

These bounded work packets are live beneath #8566 after the local cutover
graph. The existing #8676, #8547, #8636, and CUT-27 issues must not be silently
broadened to pretend they already own all of this.

| Order | Issue | Packet | Outcome | Exit proof |
| ---: | --- | --- | --- | --- |
| 0 | [#8745](https://github.com/OpenAgentsInc/openagents/issues/8745) | Intent and invariants | ProductSpec, executable behavior/UX contracts and real-host QA journeys, target vocabulary, session/checkpoint/agent-graph schemas, and threat/model boundary are frozen. | Schema/architecture tests reject host-derived session identity, root-catalog child leakage, two live attachments, orphaned active descendants, secrets in checkpoints, and silent target/isolation changes; the journey rejects detail-blocked first paint and click/hotkey divergence. |
| 1 | [#8746](https://github.com/OpenAgentsInc/openagents/issues/8746) | Durable session authority | `coding_session`, canonical agent graph/per-thread cursors, attachment generation, checkpoint metadata, host/session directory, authoritative durable log, derived current projection, volatile stream, and command outcomes land in the canonical request processor and Khala Sync. | Local fixture survives restart, live-stream gap, lost ACK, duplicate move, stale source generation, and replay without duplicate execution, duplicate child launch cards, or a flattened graph. |
| 2 | [#8747](https://github.com/OpenAgentsInc/openagents/issues/8747) | General broker | Provider/SCM/tool/API secret leases work across local and managed targets; source grants revoke and target grants reissue on move. | Revoke during move, replay, log/snapshot scan, timeout, and wipe tests pass with no raw secret projection. |
| 3 | [#8748](https://github.com/OpenAgentsInc/openagents/issues/8748) | First real move | Move a bounded session with at least one child from local Pylon A to the accepted #8547 Agent Computer and back by checkpoint/rehydrate. | Same session/thread/run/agent refs, parent edges and per-thread cursors, exact checkpoint/diff digest, one live attachment, new target grants, source cleanup, no source child accepting work, and no duplicate accepted parent or child turn. |
| 4 | [#8749](https://github.com/OpenAgentsInc/openagents/issues/8749) | Owner-managed remote target | Ship enroll/update/revoke/connect for a homelab/customer-cloud Pylon or `oa-node`. | Local → owner-managed remote → local move passes with explicit health, isolation, compatibility, and cleanup receipts. |
| 5 | [#8750](https://github.com/OpenAgentsInc/openagents/issues/8750) | Managed-provider adapter | Audit and implement one provider adapter such as Daytona behind the same contract. | Provider-specific provision/snapshot/ports/exec/teardown behavior maps to OpenAgents receipts; downgrade and cleanup failures are visible and fail closed. |
| 6 | [#8751](https://github.com/OpenAgentsInc/openagents/issues/8751) | Mobile any-host control | Mobile session directory, compact nested agent supervision, causal child activity/direct transcript access, target detail, stop/checkpoint/move/resume/failback, push/deep links, and attention views consume the shared contract. | Physical iOS and Android-emulator clients inspect and control the same canonical session/agent graph on each enrolled host class through the same typed tap/shortcut actions without host paths, tokens, or vendor APIs. |
| 7 | [#8752](https://github.com/OpenAgentsInc/openagents/issues/8752) | Conversational voice | Persona-neutral ASR/TTS/barge-in drives the same typed commands and outcomes. | Physical-phone voice follow-up plus interrupt/move/resume works under reconnect; text remains available and no audio is retained by default. |
| 8 | [#8753](https://github.com/OpenAgentsInc/openagents/issues/8753) | R7 dogfood | Sustained owner use across owner-local, accepted managed, and owner-managed target classes, including faults, updates, revocation, and rollback. | Signed owner-accepted receipt with zero forked session identity, duplicate accepted work, secret leak, silent substitution, or orphaned source runtime. |

Packets 1 and 2 are the critical path. Provider breadth and UI polish must not
race ahead of the session-fencing and secret-reissue laws.

### PORT-00 contract freeze

PORT-00 [#8745](https://github.com/OpenAgentsInc/openagents/issues/8745) is
landed at the schema/model rung. The intent artifact is
[`portable-coding-sessions.product-spec.md`](../../specs/openagents/portable-coding-sessions.product-spec.md).
`@openagentsinc/portable-session-contract` now owns the v1 public-safe schemas,
cross-record invariant audit, target/command vocabulary, and exact real-host
journey. The enforced behavior contract
`openagents_apps.portable_session_contract_freeze.v1` runs its bounded oracle
in the normal test sweep.

This freeze grants no runtime authority. PORT-01 must place the schemas and
equivalent pre-mutation checks in the canonical request/Sync authority, and
PORT-02 must implement actual broker redemption. No movement, target, mobile,
voice, or dogfood claim is inferred from PORT-00.

### PORT-01 durable authority

PORT-01 [#8746](https://github.com/OpenAgentsInc/openagents/issues/8746) now
lands the first production authority rung in Cloud SQL/Khala Sync. Migration
`0066` and `portable-session-authority.ts` persist the owner-minted session,
complete graph, per-thread cursors, authorized targets, one generation-fenced
attachment, checkpoint metadata, append-only events, repairable current rows,
and byte-idempotent commands/outcomes. The normal Worker request registry owns
session registration and command admission. A real-Postgres restart/fault
oracle proves graph preservation, current repair after a stream gap, lost-ACK
and duplicate-move reconciliation, out-of-order/stale-source refusal, generic
outcomes, and retention cascade/tombstones.

This is durable control-plane authority, not a claim that execution already
moved. PORT-02 still owns target-scoped credential redemption and PORT-03 owns
the first real graph-wide checkpoint/rehydrate/failback receipt.

## Acceptance journey

R7 should include this exact class of journey:

1. On mobile, select an authorized repository and a session currently attached
   to an owner homelab node.
2. Cold-open the session: the shell and recent top-level metadata appear before
   transcript/checkpoint detail. Open one causal child activity card, inspect
   its bounded latest activity and independent transcript, and verify the
   complete graph remains directly reachable.
3. Use voice to ask for status and submit a safe follow-up; observe the typed
   transcript and durable accepted outcome.
4. Request **Move to managed cloud**. The source graph quiesces, creates a
   secret-free checkpoint, loses execution authority, and produces cleanup and
   revocation evidence.
5. The managed target validates compatibility, redeems fresh scoped provider
   and SCM grants, rehydrates the same session/work context, and becomes the
   only live attachment generation.
6. Mobile receives progress through the same session, agent, and transcript
   refs. Desktop opens the same graph and inspects the child cursors, exact
   diff, event cursor, target history, and receipts. No source child remains
   live and replay creates no duplicate launch card.
7. Revoke one secret mid-turn, lose one acknowledgement, restart the mobile
   app, and prove a typed failure/reconciliation rather than duplicate work or
   false success.
8. Move the session to another authorized owner-managed host or fail back to
   the source. Verify the same session/thread/run identity and exact repository
   post-image.
9. Stop and reclaim. Prove processes, scratch, ports, leases, and secret slots
   are gone and that stale generations cannot resume.

## Pending target invariants

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
13. Children never leak into the top-level session catalog or become unrelated
    roots merely because a client, target, or provider uses a different layout
    or native thread identifier.
14. Canonical parent edges, agent refs, independent transcript refs, and
    canonical per-thread activity cursors are durable session state. Lifecycle
    facts are attachment-generation-fenced; source `running` state is never
    copied into target truth or reconstructed from prose.
15. Attachment fencing and quiescence cover every descendant owned by the
    attachment; no source child may accept work after target activation.
16. The durable per-thread log is event authority and repairs the bounded
    derived current projection before volatile supervision resumes. Previews
    and socket health are never receipt authority.
17. Shell and authorized top-level metadata paint before checkpoint,
    transcript, selected-child, or inspector hydration. Slow detail produces
    an explicit pending/gap/failure state, never a blank app or stuck loading
    label.
18. Click, tap, menu, palette, and conflict-safe key paths invoke the same
    registered typed action/result. Local inspect/focus/return state remains
    ephemeral by default; only authority-bearing controls reconcile a durable
    command outcome.
19. Importing or adopting local provider history is explicit and projects
    bounded canonical facts; it never uploads raw rollout files or silently
    widens retention/custody.

## Non-goals for the first accepted version

- transparent migration of live process memory, PTYs, sockets, or hidden model
  state;
- a mobile remote-desktop pixel stream;
- direct Daytona, GCP, SSH, or homelab APIs in mobile/renderer code;
- copying provider auth homes or `.env` files between hosts;
- a generic arbitrary-secret tunnel;
- silently lowering VM/container isolation to host execution;
- public pooled access to an owner's homelab;
- using Sarah, avatar, video, ambient microphone capture, or voice persona as
  a portability authority; Sarah's separately admitted managed-sandbox broker
  remains a bounded client of the same target authority; and
- claiming managed-provider support before a real adapter passes isolation,
  secret, snapshot, teardown, and receipt acceptance.

## Roadmap and issue reconciliation status

The master roadmap incorporated the original remote-first additions:

- distinguish client continuation from execution-session portability;
- make host-independent session identity and checkpoint/attachment fencing an
  R3/R4/R7 requirement;
- expand the execution target taxonomy to owner-managed remote and audited
  managed-provider adapters;
- promote a general broker as a prerequisite to cross-host movement;
- require an authorized any-host session directory on mobile; and
- carve persona-neutral conversational voice out of the Sarah/avatar/video
  wontdo boundary.

The 2026-07-19 managed-sandbox program supplies a concrete new-work placement
target for IDE-13/17 and Sarah. It intentionally does not satisfy portable
checkpoint, exclusive detach/attach, move, failback, or cross-machine Full
Auto criteria. Those claims remain independently gated.

The subsequent reconciliation adds the episode-249 consequence: the canonical agent graph,
independent child transcripts/activity cursors, and the fast click/tap/hotkey
supervision contract survive live streaming and host movement. #8691/#8692 now
own the local live-graph contract/UI and closed #8683/#8684 complete #8678's
topology residual. The former remote-portability issue-ledger gap is closed by
#8745–#8753. The remaining work is implementation and acceptance:

- execute the bounded portable-session, broker, target, any-host mobile, voice,
  and dogfood leaves without overloading #8676/#8547/#8636 or CUT-27;
- reconcile live #8566/#8574/#8597 bodies that still cite superseded voice scope, name
  closed work as future, or classify all voice as a non-goal.

The mobile port ledger must likewise move native PTT/STT from “paused” to a
bounded P0 modality over the shared session command contract, while keeping
legacy app code and Sarah media paths frozen as extraction evidence only.

This document is the pathway and gap record. Current code, live issues,
invariant ledgers, tests, deployments, and receipts remain the authority for
what has actually shipped.

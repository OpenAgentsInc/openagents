# Full Auto cross-app agent delegation over Nostr

## 2026-07-23 owner disposition

The owner canceled the standalone Nostr relay and forge program and the
separate Buzz installation. This document remains research evidence only.
Future Nostr interoperability must enter through the accepted
[`Omega plan`](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md).
It must not create relay, forge, command, outcome, or receipt authority.

- Date: 2026-07-22
- Type: speculative design and vision. This is research plus informed
  speculation. It is not a product promise, a release decision, or dispatch
  authority. It describes one possible future path.
- Scope: how OpenAgents Full Auto can extend so that the Full Auto agent in one
  OpenAgents app delegates bounded work to the agent in another OpenAgents app.
  The two agents find each other and talk over Nostr. Results return as cited
  evidence that the host verifies.
- Grounding sources:
  - `docs/teardowns/2026-07-21-buzz-teardown.md` (Block Buzz, a Nostr agent
    workspace).
  - The shared Nostr stack `nostr-effect` and `packages/nip90`.
  - `apps/openagents-desktop/src/full-auto-mission.ts` and the provider-lane
    model.
  - The hands-off Full Auto epic (issue #9171) and the 2026-07-22 autonomy
    audits.

## Label key

Each idea below carries one label.

- **[EXISTS]** — the code, kind, or contract is in the repository or in
  `nostr-effect` today.
- **[SPECULATION]** — a proposed design that no code implements yet.
- **[NEW]** — a wire kind, schema, or profile that does not exist yet and that
  this path would need to add.

## 1. The idea

Full Auto today is a strong continuation engine inside one app. The host builds
a private mission packet for each attempt. The packet names the owner objective,
the done condition, and the provider lane for the turn. The host then delegates
that turn to a local provider worker, such as a local Codex lane or a local
Claude lane. The host treats the provider result as self-reported evidence. The
host or the owner verifies the done condition. **[EXISTS]**

Evidence: `full-auto-mission.ts` defines `FullAutoMissionPacketSchema` with
schema id `openagents.desktop.full_auto_mission.v1`. The packet carries
`objective`, `doneCondition`, `responseObligations`, and a fixed
`completionAuthority` literal: "provider completion is self-reported evidence
only. the host or owner verifies the done condition". The provider lanes live in
`provider-lane.ts`. The in-flow delegated child turn uses
`delegation-runtime-card.ts`, with the stable child ref `codex`. **[EXISTS]**

The speculative extension is one step further out. **[SPECULATION]** The host
does not only delegate a turn to a local worker in the same app. The host also
delegates a bounded task to an agent that runs inside a **different OpenAgents
app**, possibly on a different machine. The two agents belong to the same owner.
They find each other and exchange the delegation over Nostr. The remote agent
runs the work on its own local provider lane. The remote agent returns a
public-safe result with artifact references. The host verifies the done
condition against the returned evidence, exactly as it verifies a local turn.
The remote app becomes one more worker that the Full Auto loop can dispatch to
and verify.

This idea is a direct read of the Buzz teardown. Buzz makes agents members of a
Nostr community. An agent holds its own keypair, its own memberships, and its
own audit trail. An agent reaches the workspace through the same protocol that
humans use (teardown §2, §3.3). The teardown recommends that OpenAgents adopt a
Nostr protocol edge but keep Cloud SQL and Khala Sync as authority (teardown
§6.8.3). Cross-app delegation is a concrete use of that protocol edge. The wire
carries identity, discovery, the delegation request, the delegation result, and
live progress. The wire does not carry settlement authority or the canonical
work record.

## 2. What Buzz shows and what OpenAgents keeps

The Buzz teardown records these Buzz mechanics. **[EXISTS in Buzz, studied]**

- The relay is the workspace. Every message, review, and Git status is a signed
  NIP-01 event in one log. The `kind` integer is the sole dispatch switch
  (teardown §Summary, §2).
- Agents hold their own Nostr secret key. Agent memory (engrams, kind 30174) is
  encrypted with a NIP-44 owner-readable key (teardown §3.3).
- Direct messages use NIP-17 gift wrap. Groups use NIP-29. Buzz authors about
  fifteen custom NIPs, including agent auth, personas, engrams, turn metrics,
  and observability (teardown §2, §3.3).
- The Buzz agent turn model is non-streaming. Text is reasoning and tool calls
  are the output (teardown §3.3, §5).

The Buzz teardown also records the central OpenAgents decision. OpenAgents
adopts selected protocols. OpenAgents does not adopt the Buzz relay as product
authority, the Tauri shell, the Flutter mobile lane, or the non-streaming turn
model (teardown §Summary, §8, §9).

The teardown lists four product postures (teardown §6.8.3). The **signed
projection bus** is the best fit. A Cloud SQL outbox produces deterministic
signed events after a canonical write. A relay failure can delay the projection.
A relay failure cannot reverse the canonical write. This model gives OpenAgents
portable signed records without a dual-authority system.

Cross-app delegation sits in the **admitted collaboration input** posture
(teardown §6.8.3). A signed event enters as a proposal. A valid signature proves
who signed the proposal. A valid signature does not prove permission, execution,
acceptance, or a public claim. The handler checks the schema, the owner scope,
the generation, the idempotency key, and the current policy before it changes
any state.

## 3. The OpenAgents Nostr stack today

The shared `nostr-effect` package implements the full Nostr protocol in typed
Effect. It exports 200-plus symbols. The relevant standard NIPs are all present.
**[EXISTS]**

| NIP | Role | Code in `nostr-effect` |
| --- | --- | --- |
| NIP-01 | Signed events, tag filters | `src/relay/core/nip/modules/Nip01Module.ts` |
| NIP-42 | Relay client authentication | `src/relay/core/nip/modules/Nip42Module.ts` |
| NIP-44 | Versioned encryption | `src/services/Nip44Service.ts` |
| NIP-59 | Gift wrap | `src/wrappers/nip59.ts` |
| NIP-17 | Private direct messages | `src/client/Nip17Service.ts` |
| NIP-28 | Public chat channels | `src/client/ChatService.ts` |
| NIP-89 | Application handler discovery | `src/client/HandlerService.ts` (kinds 31989, 31990) |
| NIP-90 | Data vending machine jobs | `src/client/DVMService.ts` (kinds 5000-6999, 7000) |
| NIP-65 | Relay list metadata | `src/client/RelayListService.ts` |
| NIP-57 | Lightning zaps | `src/client/ZapService.ts` |
| NIP-47 | Nostr Wallet Connect | `src/core/Nip47.ts` |

The workspace surface `packages/nip90` re-exports the shared NIP-90 helpers and
adds the OpenAgents labor market wrappers. **[EXISTS]** The package defines
labor request kinds `5930`-`5939` with result kinds at `+1000`. The current
agentic-coding pair is request `5934` and result `6934`, with quote and
acceptance feedback on kind `7000`. The package keeps the relay payload
reference-only. It rejects raw prompts, private paths, credentials, and payment
material at decode time. The `lbr-closeout` export composes one labor lifecycle
into a single content-addressed public-safe receipt. The receipt moves no sats
and grants no settlement authority.

The OpenAgents draft NIPs are also present in `nostr-effect`
(`src/core/OpenAgentsDrafts.ts`). **[EXISTS as wire format]** They include NIP-SA
(Sovereign Agents), NIP-AC (Agent Credit), NIP-SKL (Skills), NIP-LBR (Agentic
Labor), NIP-DS (Datasets), and NIP-SB (Remote Sandbox Protocol). NIP-LBR carries
a status note: it is POSTPONED behind the business focus. Direction is retained.
It is not current dispatch authority (`docs/nips/LBR.md`).

The Buzz-parity custom NIP family is also implemented in `nostr-effect`. **[EXISTS
as wire format]** It includes NIP-OA (Owner Attestation,
`src/services/OwnerAttestationService.ts`), NIP-AA (Agent Authentication, kind
22242), NIP-AP (Agent Personas, kinds 30175 and 30177), NIP-AE (Agent Engrams,
kind 30174), NIP-AM (Agent Turn Metrics, kind 44200), and NIP-AO (Agent
Observability, ephemeral kind 24200). These NIPs advertise through NIP-11
`supported_extensions`, never `supported_nips`.

The Buzz teardown is careful about the difference between protocol support and
product support (teardown §6.8). Some `nostr-effect` modules are complete client
services. Other modules are wire formats or readers. OpenAgents must add product
policy, custody, persistence, and evidence rules around each module before use.

## 4. Transport and protocol mapping

This section maps each delegation step to a concrete Nostr primitive. Each row
carries a label for what exists and what is new.

### 4.1 Identity: one owner scope, one key per role

Each app, each agent, and each device holds a Nostr public key role. The product
never displays or exports a raw secret key (teardown §6.8.4). **[SPECULATION for
the product wiring, EXISTS for the primitives]**

- A NIP-OA owner attestation proves that a given agent key belongs to a given
  owner. `OwnerAttestationService` implements the attestation. **[EXISTS as wire
  format]**
- A NIP-AA agent authentication event (kind 22242) proves that the agent
  controls the key and carries the owner attestation in an `auth` tag.
  **[EXISTS as wire format]**
- The delegating host and the remote host must prove that both agents share one
  owner scope before any work crosses. This owner-scope check is the primary
  authorization gate. It keeps the first version owner-scoped and closed, not an
  open marketplace. **[NEW policy]**

This mirrors the existing OpenAgents rule for local Pylon and Codex delegation.
The authorization boundary there is the token-resolved owner scope. A remote
issuer may only target capacity linked to that same owner scope (root
`CLAUDE.md`, "Known public-safe steering gaps to keep visible"). Cross-app Nostr
delegation must inherit the same owner-scope rule.

### 4.2 Discovery: which remote agent can take the task

The delegating host must find a remote OpenAgents agent that is online, owned by
the same owner, and able to do the task class. Two standard mechanisms fit.

- **NIP-89 handler discovery.** A remote app publishes a handler information
  event (kind 31990) that advertises its capability, such as a code-task lane or
  a review lane. The delegating host reads handler events and picks a candidate.
  `HandlerService` implements both kinds. **[EXISTS as wire format]**
- **NIP-90 DVM advertisement.** A remote agent can also advertise as a data
  vending machine for a given job kind. The delegating host then addresses a job
  request to that provider key. `DVMService` implements the job model. **[EXISTS
  as wire format]**

The agent card the owner sees can combine a NIP-AP persona, a NIP-OA owner
proof, a NIP-AE memory status, a NIP-AM usage summary, and a NIP-AO live state
(teardown §6.8.4). **[SPECULATION for the product view]**

A handler advertisement is a capability claim, not a capability proof. The host
must treat an advertised capability as a claim. The host verifies the returned
work, never the advertisement (see §5).

### 4.3 The delegation request and result

This is the core of the design. The delegating host must send a bounded task to
the remote agent and receive a bounded result.

- **NIP-90 job request and result.** The natural carrier is a NIP-90 job request
  (kinds 5000-5999) and a NIP-90 job result (kinds 6000-6999), with feedback on
  kind 7000. `DVMService` implements this lifecycle. **[EXISTS as wire format]**
- **OpenAgents labor kinds.** The OpenAgents labor pair `5934` code task and
  `6934` result already exists in `packages/nip90`, together with the ref-only
  guards and the content-addressed closeout receipt. **[EXISTS]** The labor spec
  is POSTPONED, so this path would reuse the wire shape, not the paid market
  lifecycle.
- **A cross-app Full Auto sub-task envelope.** The current mission packet is
  deliberately private. `full-auto-mission.ts` states that the packet is not a
  public receipt, and that the objective and the done condition are owner
  content that may leave the boundary only for the selected provider turn.
  Crossing the wire to another app needs a separate public-safe delegation
  envelope. This envelope carries a bounded objective summary, a named
  verification command, pinned repository references, and an idempotency key. It
  carries no raw prompt, no secret, no local path, and no wallet material. This
  envelope does not exist yet. **[NEW schema]**

The public-safe envelope must follow the same rule the OpenAgents Khala coding
delegation already follows. That path sends a short public-safe objective summary
plus pinned checkout references, and it keeps raw events and local workspace
paths on the device (root `CLAUDE.md`, Khala coding delegation runbook). The
cross-app envelope is the Nostr projection of that same discipline.

### 4.4 Agent-to-agent chat and clarification

The two agents may need to exchange a short clarification during the task, such
as a bounded question about scope.

- **NIP-17 private direct messages.** Owner-private agent-to-agent chat uses
  NIP-17 over NIP-59 gift wrap. `Nip17Service` implements it. **[EXISTS as wire
  format]** This keeps the exchange encrypted and owner-readable.
- **NIP-28 public chat.** A shared public channel is possible for non-sensitive
  coordination. `ChatService` implements it. **[EXISTS as wire format]** The
  default posture is private, so NIP-17 is the primary carrier.

A clarification exchange must not become a hidden authority path. A message can
adjust bounded scope only inside the already admitted task. A message can never
grant new permission, new budget, or acceptance.

### 4.5 Live progress and telemetry

The owner wants to watch a long remote task.

- **NIP-AO observability.** Ephemeral kind 24200 carries live agent state.
  `AgentObservabilityService` implements it. **[EXISTS as wire format]** An AO
  frame can show live state. An AO frame cannot prove that a command ran or that
  an outcome is accepted (teardown §6.8.2).
- **NIP-AM turn metrics.** Kind 44200 carries per-turn usage, encrypted to the
  owner. `AgentMetricsService` implements it. **[EXISTS as wire format]** An AM
  event can report usage. An AM event must not become billing authority
  (teardown §6.8.2).

OpenAgents keeps token accounting exact and platform-owned. The public counter
projects the exact `token_usage_events` rows. Counter movement alone is never
completion evidence (root `CLAUDE.md`). A remote NIP-AM frame is a convenience
projection, not the accounting source of truth.

### 4.6 Evidence and receipt

The remote agent returns evidence. The evidence must be dereferenceable and
public-safe.

- **NIP-LBR closeout receipt.** The `lbr-closeout` export composes a labor
  lifecycle into one content-addressed public-safe receipt and re-derives the
  digest so any reader can confirm that the receipt dereferences the exact
  lifecycle. **[EXISTS]** This is the natural evidence shape for a returned
  cross-app task. It moves no sats and grants no settlement authority.
- Large artifacts, such as checkpoints or repository packs, stay off Nostr. The
  events carry references and digests for those bytes (teardown §6.8.1). This
  matches the OpenAgents rule for the NIP-DS and NIP-LBR flows, which keep the
  payload reference-only.

### 4.7 Payment and settlement

Buzz is Block and Cash App work, so payment is native to its world. The Buzz
relay tunnels many features, and the ecosystem uses Lightning. OpenAgents keeps
a strict boundary here.

- The first version is **owner-scoped and internal**. Both agents belong to one
  owner. No money moves between parties. Settlement is not part of the path.
  **[SPECULATION, and the recommended first posture]**
- If a later version delegates across owners, the wire stays reference-only.
  NIP-LBR keeps invoices, preimages, and wallet material off Nostr. Settlement
  authority stays in the platform ledger, not in relay events (`docs/nips/LBR.md`,
  root `CLAUDE.md`). NIP-57 zaps or NIP-47 Nostr Wallet Connect could carry a
  payment signal, but a payment signal is not settlement authority. **[NEW policy
  work, gated]**

This boundary is a hard OpenAgents invariant. No settlement without authority.
Raw secret export, custody, and settlement remain reserved actions
(`AUTHORITY.md`, root `CLAUDE.md`).

## 5. How this composes with the hands-off autonomy work

The hands-off Full Auto epic (issue #9171) names six child lanes. The most
relevant lanes are HANDS-1 (owner-priority objective selection), HANDS-2
(host-executed done-condition verification), and HANDS-3 (a persistent decomposed
plan across turns). **[EXISTS as an epic and audits]**

Cross-app delegation is a natural fit for that loop.

- The Full Auto planner (HANDS-3) decomposes an objective into bounded work
  items. A work item names a read target, a bounded deliverable, and a named
  verification (issue #9171). **[SPECULATION for the cross-app case]**
- The objective-selection lane (HANDS-1) may route a bounded item to a remote
  OpenAgents worker, exactly as it routes a turn to a local provider lane. The
  remote worker is one more lane in the provider-lane registry.
- The verification lane (HANDS-2) runs the named verification host-side. The
  remote result is self-reported evidence only. The host does not accept the
  remote completion until the host runs the done condition and it passes. This
  is the exact rule the mission packet already carries in its
  `completionAuthority` literal and its `treat_provider_completion_as_unverified`
  response obligation (`full-auto-mission.ts`). **[EXISTS as the invariant to
  extend]**

The key property is that cross-app delegation adds no new trust. The host trusts
a remote result exactly as much as it trusts a local provider result, which is to
say, not at all until the host verifies. The Nostr boundary does not change the
verification rule. The Nostr boundary only changes where the worker runs.

## 6. Trust, safety, and boundaries

These boundaries keep the path consistent with existing OpenAgents invariants.

- **Owner-scoped, not an open marketplace.** The first version delegates only
  between apps that share one owner scope. The owner-scope check uses NIP-42
  relay auth plus the NIP-OA owner attestation. A third-party relay or a pooled
  marketplace is out of scope for the first version. This matches the current
  own-capacity rule for Pylon and Codex delegation (root `CLAUDE.md`). **[NEW
  policy over EXISTS primitives]**
- **The host verifies completion across the boundary.** Remote completion is
  self-reported evidence. The host runs the done condition. The host or the owner
  accepts the result. This is the same invariant that governs in-flow sub-agent
  delegation today (`full-auto-mission.ts`). **[EXISTS, extended]**
- **Redaction and public-safety of the wire.** The private mission packet stays
  private. Only a bounded public-safe envelope crosses. The envelope carries a
  short objective summary, a named verification, pinned references, and an
  idempotency key. The envelope carries no raw prompt, no secret, no local path,
  no wallet material, and no private repository content. This is the Nostr
  projection of the existing Khala coding delegation redaction rules (root
  `CLAUDE.md`). **[NEW schema over EXISTS discipline]**
- **Capability honesty.** A NIP-89 handler advertisement or a NIP-90 DVM
  advertisement is a claim, not a proof. The host never treats an advertised
  capability as a completed capability. The returned artifact reference is a
  claim that the host verifies. **[SPECULATION over EXISTS primitives]**
- **No unverified public claims.** A NIP-AM usage frame and a NIP-AO live frame
  are projections, not authority. The public counter still projects the exact
  `token_usage_events` rows. Counter movement is never completion evidence (root
  `CLAUDE.md`). **[EXISTS invariant]**
- **No settlement without authority.** The wire is reference-only. Settlement
  authority stays in the platform ledger. Custody and raw secret export remain
  reserved (`AUTHORITY.md`, `docs/nips/LBR.md`). **[EXISTS invariant]**
- **Effect Schema at the boundary.** Every event that enters a product boundary
  passes an Effect Schema decode. Direct NIP reuse supplies protocol mechanics,
  not product policy (teardown §6.8.6). **[EXISTS rule]**
- **Authority stays in Cloud SQL and Khala Sync.** The relay is transport. The
  relay is not the authority for the canonical work record. This keeps the
  signed projection bus posture, not the relay-as-workspace posture (teardown
  §6.8.3). A future owned relay must run on Google Cloud (root `CLAUDE.md`,
  teardown §6.8.6). **[EXISTS decision]**

## 7. A minimal first slice

The smallest real thing to build is one delegation between two owner-owned
OpenAgents apps over one relay, with host-side verification.

**Setup.** **[SPECULATION]**

- Two OpenAgents apps that belong to one owner. For example, two OpenAgents
  Desktop installs, or one Desktop and one mobile app. Each holds its own agent
  key under the sovereign signer boundary. The owner never exports a raw secret
  key.
- One scoped relay. A `nostr-effect` relay is enough for the first proof. A
  future owned relay runs on Google Cloud.

**Flow.** **[SPECULATION over EXISTS primitives]**

1. The remote app publishes a NIP-89 handler information event (kind 31990) that
   advertises a bounded code-task lane, plus a NIP-OA owner attestation and a
   NIP-AA auth event.
2. A Full Auto run in the delegating app selects a bounded work item. The host
   builds a public-safe cross-app delegation envelope. The envelope reuses the
   NIP-90 or NIP-LBR request shape (kind 5934). The envelope carries a short
   objective summary, a named verification command, and pinned repository
   references.
3. The host publishes the request to the relay, addressed to the remote agent
   key. The remote app reads the request, checks the owner scope, and runs the
   work on its own local Codex or Claude lane.
4. The remote app streams live NIP-AO progress frames. The remote app returns a
   NIP-90 or NIP-LBR result (kind 6934) with artifact references and a
   content-addressed closeout receipt.
5. The delegating host reads the result. The host treats the result as
   self-reported evidence. The host runs the named verification locally. The host
   records the result as cited evidence only after the verification passes.
6. The result never becomes an accepted public claim by itself. The existing
   evidence and promise gates apply.

**What this proves.** The proof shows that one Full Auto run can dispatch one
bounded task to a remote owner-owned OpenAgents agent over Nostr, and that the
host verifies the returned work before it accepts it. The proof adds no new
trust and no settlement.

## 8. Ordered gaps to get there

This list follows the staged path in the Buzz teardown (§6.8.7) and adapts it to
cross-app delegation. Each item is a research and design gap, not dispatch.

1. **Pin a reviewed `nostr-effect` revision and expose stable profile exports.**
   Add cross-language test vectors for each selected NIP. **[matches teardown
   §6.8.7 step 1]**
2. **Define the owner-scope identity map.** Bind each app agent key to the owner
   with a NIP-OA attestation. Prove the shared owner scope with NIP-42 relay auth
   and the attestation before any work crosses. Do not export raw keys.
3. **Author the public-safe cross-app delegation envelope schema.** Use Effect
   Schema. Reuse the NIP-90 or NIP-LBR request and result shape. Add a redaction
   guard that rejects raw prompts, secrets, local paths, wallet material, and
   private repository content at decode time. **[NEW]**
4. **Add a remote delegation provider lane to the provider-lane registry.** The
   lane publishes a job and awaits a result. The lane maps the remote result onto
   the frozen streaming event envelope that the two existing lanes already use.
   The lane fails closed to an honest interrupted disposition on any restart or
   relay gap, exactly as the local recovery contract does (`provider-lane.ts`).
   **[NEW, over EXISTS pattern]**
5. **Extend host-side done-condition verification to the remote result** (epic
   #9171, HANDS-2). Reuse the same verification path the local lane uses. The
   remote lane inherits the `treat_provider_completion_as_unverified` obligation.
6. **Add live progress observability.** Consume NIP-AO frames for a live view and
   NIP-AM frames for a usage view. Keep the exact counter and the exact usage
   rows platform-owned. Do not let a Nostr frame become accounting authority.
7. **Keep settlement off the wire.** The first version moves no money. A future
   cross-owner version stays reference-only and gates any payment behind the
   platform ledger and explicit owner authority.

## 9. Speculation and status summary

- What exists today: the local Full Auto mission packet, local provider lanes,
  in-flow sub-agent delegation, the host-verifies-completion invariant, and the
  full Nostr stack in `nostr-effect` and `packages/nip90` (NIP-89, NIP-90,
  NIP-17, NIP-28, NIP-44, NIP-59, NIP-42, and the OpenAgents and Buzz-parity
  draft NIP families as wire formats).
- What is new: the public-safe cross-app delegation envelope schema, the owner-
  scope authorization policy across two apps, the remote delegation provider
  lane, and the product wiring that binds Nostr identity, discovery, delegation,
  and evidence into the Full Auto loop.
- What stays reserved: settlement, custody, raw secret export, and any public
  claim that is not host-verified.

This document is a vision. It is not a plan of record, an issue, or a product
promise. A real build follows the normal claim and admission rules. The natural
home for a first slice is a bounded child lane under the hands-off Full Auto epic
(issue #9171) or an owner-accepted work packet, not a new feature issue, because
the repository issue policy reserves GitHub issues for strict reproducible bugs.

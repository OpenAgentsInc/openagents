# Sarah program — full assessment (2026-07-19)

> **Scope.** This is a point-in-time assessment of the entire Sarah program:
> what it was, what it is now, what is implemented, what is gated, and how the
> current open and recently closed issues move it. It reconciles the normative
> artifacts (ProductSpec, AssuranceSpec, `AUTHORITY.md`, `SARAH_AUTHORITY.md`),
> the Sol master roadmap, the live GitHub issue state, and the landed code.
> It is an assessment, not new product intent. The normative authorities remain
> the ProductSpec, the authority profiles, and `docs/sol/MASTER_ROADMAP.md`.

## 1. One-paragraph status

Sarah has been through a hard pivot. The original **public sales surface** —
`openagents.com/sarah`, the `apps/sarah` app, prospect CRM, and the owned
avatar/voice/GPU stack (OAV) — was **removed entirely** on 2026-07-10 at owner
direction. `/sarah` and `/sarah/api/*` are 404 tombstones and the GPU node is
stopped. The current Sarah is **`principal.sarah`**: an authenticated
owner-orchestrator that lives on one stable owner-private Khala Sync thread
inside the supported OpenAgents clients (mobile first), answers from cited
bounded business projections, and delegates admitted work through existing
capability brokers. That runtime is **code-landed and live** in a bounded form
(TestFlight build 119, server deploy) with **authority revision 6 / Sarah
profile revision 4**. The newest layer is the **managed agent sandbox broker**
(SBX): **8 of 11 issues in epic #9023 are closed and code-landed** (SBX-00
through SBX-07), leaving **SBX-08 supervision, SBX-09 live GCP proof, and
SBX-10 deferred snapshot/fork** open. The sandbox broker ships **default-off**.
mutation stays unavailable until SBX-09 independently proves the live GCP
target, cleanup, cost, and rollback. The **AssuranceSpec is `proposed`, not
admitted** — SARAH-AC-21..23 have proof design, SARAH-AC-01..20 remain
`needs_design` — so nothing here is assurance-verified or released beyond the
bounded owner dogfood.

## 2. Two eras of Sarah

### Era 1 — the public sales employee + avatar (RETIRED)

The first Sarah was OpenAgents' disclosed AI sales employee and intended product
front door: a live avatar, per-prospect CRM memory, typed pricing/tool
authority, served at `openagents.com/sarah` from `apps/sarah` (Bun + Effect).
Its stack: browser speech → speak bridge → Khala-gateway brain (#8600) →
sentence-streamed TTS (hydralisk-tts: Chirp 3 HD + CosyVoice2) → owned realtime
avatar (hydralisk-avatar: MuseTalk lip-sync, WebRTC) on GPU node
`sarah-avatar-gpu-1`, with a live Blueprint Map canvas (#8626).

That entire era is **retired**:

- **Removed 2026-07-10** (`git show 13bc1e7443` — "remove the Sarah surface
  entirely"). `apps/sarah` deleted, `/sarah` + `/sarah/api/*` → 404 tombstones
  served from `src/cloudrun/server.ts`.
- The GPU render node `sarah-avatar-gpu-1` (hydralisk-avatar + hydralisk-tts)
  serves nothing and is **stopped**.
- Presentation-quality programs were closed **WONTDO** (#8610, #8646).
- The behavior contracts that bounded the surface are preserved **verbatim as
  `retired`** in `packages/behavior-contracts/src/sarah-retired.ts` — **13
  retired contracts** (version `2026-07-10.1`, statements verbatim, oracle refs
  marked historical/deleted), human rendering at `SARAH_CONTRACTS.md`.
- Everything under this folder's OAV / avatar / opener / scoreboard headings
  (`2026-07-09-*`, `scoreboards/`, `receipts/`, `QUALITY_SCOREBOARD.md`,
  `GPU_MEDIA_RUN_CLOSEOUT.md`, `SARAH_KNOWLEDGE_BASE.md`, `MIGRATION.md`,
  `historical/`) is **historical source material**, not current architecture.
  The `docs/sarah/README.md` body below its top banner describes Era 1.

Era-1 issue lanes, all **closed** (2026-07-09/10): consolidation SM-1..6
(#8594), FC-BRAIN Khala gateway (#8600), quality SQ-1..8 (#8618–#8625),
Blueprint Map BM-1..5 (#8626–#8630), avatar/voice OAV-3/4 (#8613/#8614),
LiveAvatar AV-1..6 (#8598), fleet supervision FC-1/3 (#8637/#8639), in-conversation
auth KHS-5/7/9 (#8604/#8606/#8608), outreach OB-4/5 (#8561/#8562), and the
mobile consumable GL-3 (#8649).

### Era 2 — `principal.sarah`, the owner orchestrator (CURRENT)

Owner direction **2026-07-18** rebooted Sarah as an authenticated
owner-orchestrator capability *inside* the supported apps — **not a fourth
app**. She is the owner's persistent, cited, action-capable point of contact
across Full Auto, releases, issues, Forum, product delivery, cloud ops, users,
and company priorities, on one durable owner-private thread. First production
surface is the ordinary OpenAgents mobile conversation UI over hosted Khala.

Normative current artifacts:

| Artifact | State |
| --- | --- |
| `specs/openagents/sarah-owner-orchestrator.product-spec.md` | **spec_revision 4**, admitted as product/roadmap intent |
| `specs/openagents/sarah-owner-orchestrator.assurance-spec.md` | **assurance_revision 4, `proposed`** (not admitted) |
| `AUTHORITY.md` | **revision 6** (root profile) |
| `docs/authority/SARAH_AUTHORITY.md` | **authority_revision 4, `admitted`** |
| `docs/sol/MASTER_ROADMAP.md` | revision governs sequencing. Active P1 = SBX |

The reboot reuses existing primitives (mobile conversation, hosted Khala, Full
Auto, FleetRun, claims, repo/GitHub, Forum, Google Cloud, release,
product-promise) and **adds no** Sarah-specific CRM, transcript store, issue
queue, provider router, raw-credential path, or authority model.

## 3. Authority model (what Sarah may and may not do)

Sarah's effective authority is the **intersection** of the root profile,
Sarah profile, active program, target policy, and exact capability — explicit
deny wins, and **self-amplification is impossible**. `AUTHORITY.md` revision
history seats her:

- **Rev 3** designates Sarah as the owner's persistent orchestrator.
- **Rev 4** admits two runtime brokers: (a) dispatch bounded code workers
  through the owner's linked Pylon capacity (real only when actual assignment
  refs pin an exact public commit), and (b) pause/resume/stop an existing owner
  Full Auto run (incomplete until owner Desktop applies the durable intent). No
  remote Full Auto start, no MemoHarness bank access, no shell/db/cloud
  superuser.
- **Rev 5** admits owner-private terminal adaptation of Sarah's conversational
  harness: she may inspect the released bundle, review only completed
  owner-thread turns, compile immutable private experience refs, and propose a
  bounded policy candidate — but **the producer cannot evaluate, release, or
  activate its own output**. A separate evaluator + Blueprint gate own held-out
  quality/regression/privacy/safety/compatibility/authority checks and the
  atomic compare-and-swap activation.
- **Rev 6** admits the closed **managed-sandbox broker** actions (Sarah profile
  rev 4 mirror): create/list/inspect/dispatch/interrupt/stop/resume/delete for
  the authenticated owner's OpenAgents-managed GCP sandboxes only — **no raw
  `gcloud`, shell, database, topology, guest address, service-account/provider
  credential, filesystem path, or generic container-admin tool**.

**Reserved (always refused):** secret export, financial custody/settlement,
legal/employment/tax/regulatory/natural-person commitments, destructive
customer-data ops, human-identity ceremonies, over-budget spend, invariant
weakening, unsupported public claims, self-amplification, and stable release
without current owner direction.

Every action must emit an `openagents.authority_decision_receipt.v1` and a
target receipt. **Visibility never implies mutation**, and Sarah **never claims
an action ran until a target receipt exists**.

> **Note — a version label to watch.** The ProductSpec `tool_metadata` reads
> "AUTHORITY.md revision 6 + Sarah runtime authority revision 4", and
> `SARAH_AUTHORITY.md` is `authority_revision: 4`. `AUTHORITY.md` itself is
> revision 6 but its prose describes the rev-5 harness-adaptation and rev-6
> sandbox grants that the Sarah profile also carries. The numbering is internally
> consistent (root rev ≠ Sarah profile rev) but the two counters are easy to
> conflate. Keep the root-vs-profile distinction explicit in future edits.

## 4. ProductSpec acceptance-criteria surface

Revision 4 carries **23 acceptance criteria (SARAH-AC-01..23)**:

- **AC-01..10** — identity/thread continuity, mobile pinning, cited context,
  redaction, intersection authority, visibility≠mutation, reserved refusals,
  epistemic labeling (fact/inference/recommendation/delegated/succeeded/refused/
  unavailable), and revocation-to-safe-checkpoint.
- **AC-11..15** — Gemma 4 buffered function calling (≤6 rounds), ≤8 Codex worker
  dispatch pinning exact commit, Full Auto pause/resume/stop returning `pending`
  until Desktop applies, per-tool ordered activity + authority receipts, and the
  explicit **no-tool** list (no remote FA start, no harness mutation, no
  current-turn learning, no self-promotion, no self-admission).
- **AC-16..20** — the terminal-adaptation harness: released content-addressed
  six-dimension policy bound per turn, terminal-only owner-thread review,
  disjoint train/held-out snapshots, a separate Blueprint gate requiring
  held-out quality/regression ≥0.75 and privacy/safety ≥0.90 before
  compare-and-swap, and no public/mobile projection of harness internals.
- **AC-21..23** — the managed-sandbox additions (rev 4): owner-scoped
  lifecycle, one bounded long-running work unit per ready sandbox with
  interrupt, and the **no-raw-cloud-surface** guarantee.

Success metrics: owner-contact continuity ≥95%, **0** unsupported
state/completion claims (lifetime), delegated completion ≥90%, and 100% managed
-sandbox actions with exact authority/target/cleanup outcomes.

## 5. AssuranceSpec — proof status

- **Revision 4, lifecycle `proposed`.** It is bound to the exact rev-4
  ProductSpec bytes (`document_digest sha256:9de58d7e…`).
- **Every one of the 23 criteria is in assurance scope** — none silently
  excluded.
- **SARAH-AC-21..23 have complete proposed proof design.** **SARAH-AC-01..20
  remain `needs_design`** — each criterion currently has one incomplete
  proposed obligation whose missing proof-design fields project as
  `needs_design` and **prevent admission or execution**.
- The risk model is **empty/`[]`** — reviewers must still design the applicable
  risk model from the ProductSpec risk prose (source snapshot preserved).
- Repository inventory is `absent` / `not-supplied`. It names candidate
  artifacts only: `sarah-managed-sandbox.test.ts`,
  `managed-sandbox-broker.test.ts`, and
  `docs/sol/evidence/2026-07-19-sbx07-sarah-managed-sandbox-broker.json`.
- **Bottom line: Sarah is not assurance-verified.** The proposal claims
  "neither execution, independent verification, admission, nor release." This is
  the single largest gap between the landed code and a defensible release.

## 6. Managed agent sandboxes (SBX) — the current Active-P1 layer

Epic **[#9023](https://github.com/OpenAgentsInc/openagents/issues/9023)**
("Managed agent sandboxes: GCP lifecycle, Box SDK compatibility, IDE, and
Sarah") is the current Active-P1 program and the layer that most directly moves
Sarah forward. It defines one owner-scoped, generation-fenced `SandboxResource`
over the existing GCE/Firecracker workroom substrate, serves an admitted subset
of the Box v1 API (unmodified `@asciidev/box-sdk@0.0.24` as an isolated
conformance client — **compatibility, not a control-plane migration**), and
gives `principal.sarah` a closed lifecycle broker.

### Issue ledger (epic #9023)

| Issue | Lane | State | Effect on Sarah |
| --- | --- | --- | --- |
| #9029 | **SBX-00** freeze contract/authority/assurance/SDK provenance | ✅ CLOSED 07-19 | Admits the exact Sarah authority + broker contract (unblocks AC-21..23) |
| #9034 | **SBX-01** durable lifecycle authority + generation-fenced store | ✅ CLOSED 07-19 | Lifecycle truth Sarah's broker calls into |
| #9028 | **SBX-02** real GCP runtime + image/IAM/network/capacity admission | ✅ CLOSED 07-19 | The live target Sarah dispatches to (still flag-gated) |
| #9025 | **SBX-03** Box v1 facade + unmodified SDK conformance | ✅ CLOSED 07-19 | External-SDK compatibility path |
| #9024 | **SBX-04** long-running Codex/Claude turns, events, interrupt | ✅ CLOSED 07-19 | The turn/interrupt semantics behind AC-22 |
| #9026 | **SBX-05** bounded files/commands/artifacts/quotas/hardening | ✅ CLOSED 07-19 | I/O + quota policy under Sarah's dispatch |
| #9027 | **SBX-06** IDE project + agent-graph integration | ✅ CLOSED 07-19 | Same broker Sarah uses, from the IDE |
| #9030 | **SBX-07** **Sarah** managed-sandbox lifecycle + dispatch broker | ✅ CLOSED 07-19 | **The Sarah broker itself — code-landed, default-off** |
| #9031 | **SBX-08** bounded mobile/web supervision | 🔲 OPEN (P1) | Supervision UI for the sandboxes Sarah creates |
| #9033 | **SBX-09** independent live GCP acceptance + rollout | 🔲 OPEN (P1) | **The gate that makes Sarah's sandbox mutation live** |
| #9032 | **SBX-10** deferred snapshot/fork/private desktop | 🔲 OPEN (P2-deferred) | Phase-2 only, after distinct proofs |

Related open item **[#9041](https://github.com/OpenAgentsInc/openagents/issues/9041)**
(IDE-13, P0) — portable project capabilities with exclusive attachment and
verified checkpoints — is the IDE-side dependency the same broker serves.

### What "code-landed, default-off" means precisely

Per the roadmap Active-P1 note and owner decision #2: **SBX-00 through SBX-07
are code-landed** — the native Worker broker now serves the closed Sarah tool
set and authenticated Desktop endpoints. **The broker flag stays off until
SBX-09** independently proves the deployed GCP target, cleanup, cost, rollback,
packaged Desktop, and real owner-thread journeys. So the *capability exists in
code and passes its own tests*, but **Sarah cannot mutate a real sandbox** until
SBX-09 flips the gate. Fixture/fake proof cannot satisfy SBX-09 (MSB-AC-18).

## 7. Implemented code inventory

The entire Sarah runtime lives inside **`apps/openagents.com/workers/api`** (the
Cloud Run / Worker API). **There is no standalone Sarah app** — `apps/sarah/` is
a deleted empty shell, and `packages/sarah/` is a shared identity/contract
package, not a runtime. Mobile (`apps/openagents-mobile`) holds only thin API
clients. Every file below has a matching `*.test.ts`.

**Identity + authority (shared):**

- `packages/sarah/src/index.ts` — `principal.sarah` projection, the 15-entry
  `SARAH_CAPABILITIES` directory, business-context schema, the compiled
  `SARAH_RUNTIME_AUTHORITY_PROFILE` (Sarah profile rev 4 over root rev 6, 3
  grants incl. `grant.sarah.managed_sandbox` with its 8 actions),
  `buildSarahSystemPrompt`, `sanitizeSarahConversationResponse`.

**Turn loop + hosted inference:**

- `apps/openagents.com/workers/api/src/sarah-agent-runtime.ts` — `runSarahAgentTurn`,
  the bounded agentic loop (`SARAH_AGENT_MAX_TOOL_ROUNDS = 6`, sequential tool
  execution, last-round tool suppression, usage accounting).
- Wiring lives in the **hosted-runtime dispatch tick** (`runHostedRuntimeTurnDispatch`
  in `src/index.ts` ~6800–7078) — Sarah runs on a **queued/scheduled turn
  processor, not a synchronous HTTP chat route**. `prepareTurn` gates on
  `hasSarahThreadAuthority`, collects business context, binds the harness, and
  builds the system prompt. `owner_conversation` calls `runSarahAgentTurn`.
- `apps/openagents.com/workers/api/src/inference/gemma4-adapter.ts` — `makeGemma4Adapter`,
  the live inference lane: Google Generative Language API
  (`generativelanguage.googleapis.com/v1beta/models/{model}:generateContent`),
  thinking-model thought filtering, buffered function calling.

**Routes, receipts, context:**

- `apps/openagents.com/workers/api/src/sarah-owner-routes.ts` — route
  `/api/mobile/sarah`. `ensureSarahPrincipal`, `authorizeSarahOperation`
  (resolves via `resolveAuthorityDecision`, writes **receipt-first** to the
  `sarah_authority_decision_receipts` table before any broker runs), and
  `hasSarahThreadAuthority` (admitted-bootstrap receipt + admin-email gate).
- `sarah-business-context.ts` (`collectSarahBusinessContext`),
  `sarah-runtime-tools.ts` (tool defs incl. harness inspect/review),
  `sarah-harness-service.ts`, `sarah-speech-routes.ts`, `crm-sarah-handoff.ts`.

**Managed sandbox broker (SBX):**

- `apps/openagents.com/workers/api/src/managed-sandbox-broker.ts` —
  `makeManagedSandboxBroker` (policy assertion, idempotent reservation,
  create/dispatch/interrupt/inspect over a `BoxV1Runtime`/`BoxV1NativeStore`).
- `packages/managed-sandbox-contract/src/` — `SandboxResource` schemas,
  lifecycle, guest-io, provenance (`BOX_V1_TRANSLATOR_REF`), Box-v1 op map.
- `packages/khala-sync-server/src/managed-sandbox-store.ts` —
  `PostgresManagedSandboxStore` (reservation/settle/replay).
- `managed-sandbox-box-v1-adapter.ts` + `managed-sandbox-box-v1-routes.ts`
  (~2044 lines, the `/v1` Box facade) + `managed-sandbox-desktop-routes.ts`.
  `@asciidev/box-sdk@0.0.24` is a **dev-only conformance dependency**
  (production must not import it). Unsupported methods → `501
  capability_not_implemented`.
- **Sarah's SBX-07 broker: `apps/openagents.com/workers/api/src/sarah-managed-sandbox.ts`**
  (~832 lines) — `makeSarahManagedSandboxTools`: the 8 closed owner-scoped tools
  (create/list/inspect/dispatch/interrupt/stop/resume/delete), each routed
  through `authorizeSarahOperation` (receipt-first). Wired at `index.ts`
  ~6907–6942, gated on `MANAGED_SANDBOX_BROKER_ENABLED` +
  `OA_CLOUD_CONTROL_URL/TOKEN` + `KHALA_SYNC_DB`. Authority tests also in
  `packages/authority/src/managed-sandbox-authority.test.ts`.

**Tombstone (confirmed):** `apps/openagents.com/workers/api/src/cloudrun/server.ts`
lines **149–159** — `url.pathname === '/sarah' || url.pathname.startsWith('/sarah/')`
returns `404 {error:'not_found'}` (covers `/sarah` and every `/sarah/api/*`),
citing owner direction 2026-07-10 / #8610.

**FleetRun:** `apps/openagents.com/workers/api/src/sarah-fleet-run-routes.ts`
defines `FLEET_RUNS_PATH = '/api/fleet-runs'` (neutral canonical) and
`SARAH_FLEET_RUNS_PATH = '/api/sarah/fleet-runs'` (**served compatibility alias**,
identical handler — kept, not 410'd, because shipped binaries hardcode it).
Authority backend `packages/khala-sync-server/src/fleet-run-authority.ts`.
the execution engine is `apps/pylon/src/orchestration/fleet-run-*.ts`.

## 8. How open / recently-closed issues move the program

**Recently closed (advances):**

- The whole Era-1 lane set closed 2026-07-09/10 — Sarah's *old* shape is fully
  wound down, not half-migrated. That is why the folder reads as historical.
- **SBX-00..07 closing 2026-07-19** is the substantive forward motion: the
  managed-sandbox contract, GCP runtime, Box facade, long-running turns, I/O,
  IDE integration, and the **Sarah broker** all landed in one day. This takes
  Sarah from "orchestrator that can dispatch coding workers and control Full
  Auto" to "orchestrator that also has a code-complete sandbox lifecycle broker."
- #9013 (mobile voice message-scoped, closed 07-19) and #8597 (P0 mobile Sync/
  remote coding/fleet control, closed 07-13) firmed the mobile surface Sarah
  rides on.

**Open (gates / next motion):**

- **SBX-09 (#9033)** is the critical gate. Until its independent live-GCP
  acceptance, isolation, cleanup, cost, and rollback proofs pass, Sarah's
  sandbox broker stays default-off and unproven end-to-end. It is the highest-
  leverage open item for the Sarah program.
- **SBX-08 (#9031)** gives mobile/web the supervision projection for sandboxes
  Sarah spins up — needed for the owner to actually watch delegated work.
- **SBX-10 (#9032)** is explicitly deferred (P2) — snapshot/fork/private desktop
  after distinct proofs. Not on the critical path.
- **IDE-13 (#9041, P0)** shares the broker. Its portable-capability work is a
  parallel dependency, not Sarah-specific.

**Not moving (by design):** no open issue reopens the public `/sarah` surface,
the avatar/OAV stack, prospect CRM, or a standalone Sarah app. The roadmap
explicitly excludes standalone/public Sarah UI, persona/role expansion, and any
avatar/voice/video revival.

## 9. Risks and gaps (assessment)

1. **Assurance is the gap, not code.** 20 of 23 criteria are `needs_design` and
   the AssuranceSpec is `proposed`. The code exists and self-tests, but there is
   **no admitted independent-verification path** for AC-01..20 (identity,
   redaction, authority intersection, epistemic labeling, harness adaptation).
   Closing SBX code without advancing the AssuranceSpec risks a defensible-proof
   deficit if any public claim is ever attempted.
2. **Default-off is doing a lot of work.** The safety story depends on the
   SBX-09 flag staying off. Any accidental enablement before SBX-09 would put an
   unproven GCP mutation path in Sarah's hands. The gate must remain owner-held.
3. **Empty risk model in the AssuranceSpec.** Reviewers still owe the applicable
   risk objects. The ProductSpec risk prose is captured but not yet modeled.
4. **Version-label conflation** (root rev 6 vs Sarah profile rev 4) — cosmetic
   today, but a documentation footgun. Keep the two counters distinct.
5. **Historical/current bleed in `docs/sarah/`.** The folder mixes retired Era-1
   material with current Era-2 authority. The `README.md` banner handles it, but
   a reader skimming filenames could mistake OAV/scoreboard docs for live
   architecture. This assessment is the current-state index.

## 10. Recommended next steps (non-binding)

> **Update (2026-07-19, later):** owner direction now targets immediate
> activation — Sarah live, sandbox usage, coding delegation, and mobile push
> updates. The decomposition and issue ledger for that direction live in
> **`2026-07-19-sarah-activation-gap-analysis.md`** (#9062–#9065, plus SBX-09
> #9033 re-prioritized to P0). The steps below remain the release-grade view.

1. **Advance the Sarah AssuranceSpec from `proposed` toward `needs_design →
   designed`** for AC-01..20, starting with the highest-risk criteria
   (AC-01 identity isolation, AC-05 redaction, AC-06 intersection authority,
   AC-08 reserved refusals). This is the real blocker to any release beyond
   bounded owner dogfood.
2. **Keep SBX-09 owner-gated and independent.** Its live-GCP proof is what turns
   the code-landed broker into a usable Sarah capability. Do not let the
   producer self-admit it.
3. **Land SBX-08 supervision** so the owner can watch delegated sandbox work
   from mobile/web — the missing half of "delegate and follow."
4. **Design the AssuranceSpec risk model** and attach obligations to the two
   named test artifacts.
5. **Leave Era-1 retired.** No open issue should revive the public surface,
   avatar, or CRM. Treat this folder's OAV material as archive.

## 11. Source index

- ProductSpec: `specs/openagents/sarah-owner-orchestrator.product-spec.md` (rev 4)
- AssuranceSpec: `specs/openagents/sarah-owner-orchestrator.assurance-spec.md` (rev 4, proposed)
- Root authority: `AUTHORITY.md` (rev 6). Sarah profile: `docs/authority/SARAH_AUTHORITY.md` (rev 4)
- Managed sandboxes: `specs/openagents/managed-agent-sandboxes.product-spec.md`,
  `docs/sol/2026-07-19-managed-agent-sandboxes-accepted-plan.md`, epic #9023
- Roadmap: `docs/sol/MASTER_ROADMAP.md` (Active P1, owner decisions #2, #12, #14)
- Retired contracts: `packages/behavior-contracts/src/sarah-retired.ts`, `docs/sarah/SARAH_CONTRACTS.md`
- Era-1 history: `docs/sarah/README.md` (below banner), `MIGRATION.md`, `historical/`, OAV docs, `scoreboards/`
- Removal decision: `docs/sol/decisions/2026-07-10-greenfield-clients-and-sarah-removal.md`, commit `13bc1e7443`

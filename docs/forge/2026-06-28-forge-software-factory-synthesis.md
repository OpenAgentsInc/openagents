# Forge is not JUST a git forge — it's also a software factory

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.
Forge implementation routing must respect the separate-private-repo boundary.


Date: 2026-06-28
Status: Synthesis / product-framing doc. Public-safe; no secrets, no tokens,
no deploy. Theme: fold the historical OpenAgents **software-factory** vision
into the current owned **git-forge coordination-layer** direction so they ship
as one product, not two.

> Read this alongside the build/architecture docs it synthesizes:
> `docs/forge/2026-06-28-forge-openagents-com-owned-coordination-layer-audit.md`
> (the coordination-layer decision + roadmap),
> `docs/forge/2026-06-28-forge-standup-spec.md` (the SU-0..SU-8 stand-up),
> `docs/forge/2026-06-28-forge-boundary-contract.md` (auth + storage boundary),
> `docs/forge/2026-06-28-forge-cross-system-leverage.md` (Pylon/Tassadar/
> Artanis/Payments/Blueprint/Khala/Psionic/Nostr leverage),
> `docs/forge/origin.md` (Cursor Origin analysis) and `docs/forge/linear.md`
> (Linear Diffs analysis), plus
> `docs/forge/2026-06-28-forge-linear-adaptation.md` (what to adapt from
> Linear onto Forge's software-factory layer). The **historical**
> software-factory vision lives in
> the root workspace `products/forge.md` +
> `products/2026-04-14-openagents-com-forge-mvp-roadmap.md`, and in the
> `docs/blitz/forge/*` blitz docs cited throughout.

---

## 0. The one-sentence thesis

Two separate Forge histories have converged on the same name, and they are not
rivals — they are two layers of one product:

- the **coordination layer** (a git forge) — *how* arbitrarily many agents push,
  verify, queue, promote, and mirror code without GitHub as the bottleneck; and
- the **software-factory layer** (a work system) — *what* the work is, who it is
  for, how it is triaged into a production line, how it is measured, and how it
  is delivered to and trusted by customers.

The coordination layer is the machine substrate. The factory layer is the
product that runs *on* it. **Forge is "the forge" and "the factory" at once.**
This doc shows the seam, says honestly what is built vs envisioned on each side,
maps the factory ideas worth folding in onto the live stand-up sequence, and
ends with a biggest-synergies-first list.

---

## 1. Two histories, one name

### 1.1 The git-forge / coordination-layer history (recent, decided 2026-06-28)

The owner decided to fan out to arbitrary-N coding agents now and to stop
fighting GitHub's coordination ceiling (PR/merge serialization, secondary
rate-limits, abuse-flag blast radius, the 10–30s `push→webhook→pull` floor).
GitHub becomes a **downstream read-only mirror**; OpenAgents owns the real
coordination layer at `forge.openagents.com`
(`docs/forge/2026-06-28-forge-openagents-com-owned-coordination-layer-audit.md`
§0). Cursor's announced "Origin" forge is read as convergent evidence, not a
template (`docs/forge/origin.md`); the named differentiators are GitHub-as-mirror
(no migration ask) and a native economic loop.

This layer is concrete and partly built. The FORGE-1..6 first wave is merged on
`main`:

- **Coordination source of truth (D1).**
  `apps/openagents.com/workers/api/src/forge-coordination-store.ts` +
  `migrations/0251_forge_coordination_source_of_truth.sql` — work records,
  change records, NIP-34-aligned status rows, dispatch leases, merge-queue
  ledger snapshots.
- **Git intake parser.** `apps/pylon/src/git-receive-pack.ts` — owns commit
  intake (pkt-line framing, ref-update commands, packfile digest) before any
  storage/mirror layer.
- **Packfile archive (R2 + D1).**
  `apps/openagents.com/workers/api/src/forge-git-packfile-archive-store.ts` +
  `0252_forge_git_packfile_archives.sql` — raw pack bytes in R2, refs/metadata
  in D1, idempotent by tenant+digest.
- **Tenant git auth.**
  `apps/openagents.com/workers/api/src/forge-tenant-git-auth-store.ts` +
  `0253_forge_tenant_git_access_tokens.sql` — token-scoped per-tenant git access
  (`git:upload-pack` / `git:receive-pack` / `git:admin`), fail-closed.
- **Dispatch protocol.** `packages/forge-protocol/` +
  `apps/pylon/src/forge-dispatch-protocol.ts` — typed Pylon↔Forge work-item /
  decision / closeout messages mapped onto `assignment_lease.v0.3`.
- **Verification runner.** `apps/pylon/src/forge-verification-runner.ts` —
  Docker-isolated `bun test` executor (`--network none`, read-only rootfs,
  dropped caps) returning a public-safe receipt.

Plus the M0 ordering/governance primitives already landed in `apps/pylon`:
virtual merge queue planner + gates (`apps/pylon/src/virtual-merge-queue.ts`,
`apps/pylon/src/blueprint-gates/*`), priority dispatch, and the fan-out
coordinator (`apps/pylon/src/coordinator/`). The boundary is frozen in
`docs/forge/2026-06-28-forge-boundary-contract.md`
(`@openagentsinc/forge-protocol`: `ForgeControlPlaneScope`,
`ForgeVerificationReceipt`, `ForgePromotionDecisionReceipt`), and the stand-up
sequence SU-0..SU-8 is in `docs/forge/2026-06-28-forge-standup-spec.md`. SU-1/1B
shipped the separate `apps/forge/` shell (#6759, #6769); SU-2 shipped
`/api/forge/*` control-plane routes (#6770, `0254_forge_control_plane_receipts.sql`);
SU-3 git-intake wiring is filed as #6771.

### 1.2 The software-factory history (older, the product thesis)

Long before the coordination decision, Forge was specified as **the internal
software factory** that owns the lifecycle of software work *after intent is
declared*: intake → scoping → workspace materialization → execution assignment
→ verification → evidence capture → handoff → delivery → retention/cleanup
(`products/forge.md` §1). Its core product objects are a full lifecycle model —
**Work Order, Run, Workspace, Controller Lease, Knowledge Pack, Evidence Bundle,
Verification Report, Delivery Receipt, Handoff, Artifact** — each with stable IDs
and state-transition tables (`products/forge.md` §5, §16). It defines a
control-plane responsibility set (intake/scheduling, queueing/prioritization,
lease assignment, workspace provisioning, runtime supervision, restart/recovery,
evidence, delivery, attribution, visibility, cleanup — `products/forge.md` §6),
distinct **customer** vs **internal-operator** surfaces (`products/forge.md`
§11), and an MVP roadmap whose real bar is the *dogfood* loop "use Forge to
improve Forge" (`products/2026-04-14-openagents-com-forge-mvp-roadmap.md`). The
strategy framing in `alpha` is blunt: "Forge is the software factory," with a
"Forge Confidential" enterprise tier on top of the same lifecycle model.

That vision already shipped a visible product surface — the `/forge` **factory
dashboard** inside `apps/openagents.com` — built on the *Autopilot Work* control
plane, with a rich set of blitz-era features:

- a canonical eight-stage **production line**: Signal → Triage → Code Gen →
  Validate → Release → Document → Monitor → Deploy, with run-states bucketed
  into stages and every number tagged `live` / `seeded`
  (`docs/blitz/forge/2026-06-16-forge-factory-metric-definitions.md`);
- **factory metrics**: throughput, stage throughput, cycle time, pass rate,
  token efficiency, MTTR, backlog/queue-burn, week-over-week intake (same doc);
- an **automations surface** — one automation per stage that creates a *real*
  work order via `POST /api/autopilot/work`
  (`docs/blitz/forge/2026-06-16-forge-automations-surface.md`);
- **per-vertical stage templates** (E-commerce, Legal, Marketing-Agency,
  General Knowledge-Work) that keep the same canonical stage *keys* while
  renaming the display per domain
  (`docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md`), with
  typed seeds in
  `apps/openagents.com/workers/api/src/prefilled-workspace-vertical-templates.ts`;
- **prefilled vertical workspaces** seeded as public-safe drafts plus a
  seed→invite→engagement loop
  (`docs/blitz/forge/2026-06-16-ecommerce-prefilled-workspace.md`,
  `…-legal-prefilled-workspace.md`, `…-marketing-agency-prefilled-workspace.md`,
  `docs/blitz/forge/2026-06-16-workspace-seeding-invite-engagement.md`);
- **customer-one dogfood + cohort** instrumentation: a dogfood status strip, a
  spend-routing row (owned-Pylon vs fallback vs metered vs blocked), a Slack
  Connect business-intake form, and a refs-only, privacy-reviewed cohort
  evidence ledger
  (`docs/blitz/forge/2026-06-16-customer-one-dogfood-factory.md`,
  `…-customer-one-spend-routing.md`,
  `docs/blitz/forge/2026-06-16-business-slack-connect-intake.md`,
  `docs/blitz/forge/2026-06-17-customer-one-cohort-source-contract.md`).

The Autopilot Coder roadmap
(`docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`)
records that the factory side already has many terminal-agent primitives: change
capture, delivery readiness, diff-review artifacts, a progress lane, session
navigation, context snapshots, repository memory, and retrieval-plan
projections.

### 1.3 The collision — and why it is good

The stand-up spec is explicit that the old `/forge` page is "historical source
material … not the expansion target," and that the canonical UI is now the
separate `apps/forge/` shell on `forge.openagents.com`
(`docs/forge/2026-06-28-forge-standup-spec.md`, "Product/UI boundary";
`apps/forge/README.md`). So today there are literally **two Forge dashboards**:
the factory dashboard (old, on the Autopilot Work control plane) and the
coordination shell (new, on the coordination store). Treated as a contradiction,
that is wasteful. Treated as a **layering**, it is the whole product: the factory
dashboard is the *product semantics* the coordination shell now needs, and the
coordination store is the *durable authority* the factory dashboard always
lacked. The rest of this doc folds them together.

---

## 2. The two layers, defined precisely

| | Coordination layer (the git forge) | Software-factory layer (the work system) |
|---|---|---|
| Question it answers | *How* does code move safely at arbitrary N? | *What* is the work, for whom, and is it trustworthy? |
| Primary objects | work record, change record, NIP-34 status, dispatch lease, merge-queue ledger, verification receipt, promotion decision (`@openagentsinc/forge-protocol`, `forge-coordination-store.ts`) | Work Order, Run, Workspace, Knowledge Pack, Evidence Bundle, Verification Report, Delivery Receipt, Handoff (`products/forge.md` §5) |
| Vocabulary | refs, packfiles, fast-forward, promotion, mirror | Signal/Triage/Code Gen/Validate/Release/Document/Monitor/Deploy, verticals, cohorts |
| Authority | D1 source of truth; canonical git object/ref store; Blueprint-gated promotion | lifecycle truth above runtime; verification-before-delivery; customer/operator surfaces |
| Built today | FORGE-1..6, M0 queue/dispatch/coordinator, SU-1/1B shell, SU-2 routes | `/forge` factory dashboard, metrics, automations, vertical templates, prefilled workspaces, cohort ledger |
| Still envisioned | SU-3..SU-8 (intake→merge authority→mirror→dogfood→multi-tenant), M3-M5 | re-homing onto the coordination store; durable Work Order/Run lifecycle on the new authority |
| Closest external analogue | Cursor **Origin** (owned forge) | **Linear** (product-context + agent-orchestration above code) |

The decisive observation: **the factory's "stage" is a projection over the
coordination layer's state, and the factory's "Work Order" is the human-facing
name for the coordination layer's "work record."** They are the same objects at
two altitudes. The factory layer adds *meaning, audience, and measurement*; the
coordination layer adds *durable authority and arbitrary-N throughput*.

---

## 3. How the factory layer sits ON the coordination layer

The eight factory stages are not a parallel state machine — they are a read-model
over coordination + verification + promotion state. The mapping:

| Factory stage (`products/forge.md`, blitz stage templates) | Coordination-layer truth it projects | Stand-up step |
|---|---|---|
| **Signal** (intent enters) | `POST /api/forge/work-records` create; intake source refs | SU-2 |
| **Triage** (scoped, prioritized, assigned) | work record scoped + `dispatch_lease` acquired; priority tier | SU-2, M1 (FORGE-2) |
| **Code Gen** (artifact drafted) | dispatch protocol work-item → Pylon run → `git-receive-pack` intake → packfile archive → change record | SU-3 (FORGE-5/dispatch) |
| **Validate** (checked vs criteria) | `forge-verification-runner` → `ForgeVerificationReceipt` | SU-5 |
| **Release** (candidate accepted) | virtual-merge-queue `nextActualPromotion` + Blueprint gates → `ForgePromotionDecisionReceipt` | SU-4 |
| **Deploy** (applied to target) | GitHub mirror worker fast-forwards promoted commit | SU-6 |
| **Document** (made reusable) | evidence/handoff refs attached to the change record | (factory-owned) |
| **Monitor** (outcomes/defects observed) | post-promotion signals; NIP-22/CI events as new work records | M4 (FORGE-14) |

Two structural facts make this a clean compose rather than a re-label:

1. **One row model, two renderings.** The cross-system-leverage doc's synergy #4
   ("Unify `/forge` and Autopilot workrooms over one SU-1 row model") is exactly
   this fold: the factory dashboard, per-run workrooms, vertical workspaces, and
   cohort lanes should all render the *same* coordination work-record / change /
   status / verification / promotion refs, so a work item never has one truth in
   a workroom and a different truth on a factory dashboard
   (`docs/forge/2026-06-28-forge-cross-system-leverage.md` §"Autopilot And
   OpenAgents Web"). The factory's `live`/`seeded`/`absent` provenance discipline
   (`…-forge-factory-metric-definitions.md`) becomes *automatic* once the numbers
   read from coordination rows instead of best-effort Autopilot projections.

2. **The stage taxonomy survives, the storage changes.** The vertical stage
   templates already insist the canonical stage *keys*
   (`signal/triage/codegen/validate/release/document/monitor/deploy`) stay fixed
   while display names vary per vertical
   (`…-per-vertical-forge-stage-templates.md`, "Template Selection Rules"). That
   is precisely the property needed to re-home them onto the coordination store:
   store `templateRef` + canonical `stageKey` on the work record, and the factory
   read-model is a pure projection.

---

## 4. Which historical software-factory ideas to fold in (the synergistic set)

These are the historical ideas that make Forge a *product*, not just a git host,
and that the coordination layer makes *cheaper and more honest* to build. Each is
flagged as fold-in (synergistic), keep-separate, or defer.

### 4.1 Fold in — they upgrade the coordination layer into a product

- **The Work Order / Run / Delivery lifecycle as the factory's name for
  coordination rows.** `products/forge.md`'s Work Order
  (`draft→queued→leased→running→blocked→verification_pending→delivered`) and
  Delivery Receipt (`draft→ready→submitted→reviewing→merged/accepted/rejected/
  rolled_back`) state machines are richer, more customer-legible versions of the
  coordination layer's work-record + status + promotion-decision rows. Fold the
  lifecycle *vocabulary and state tables* onto the coordination store as the
  product-facing layer; do not build a second authority. This is the single
  biggest unification.

- **The eight-stage production line as the canonical read-model** (§3). It is the
  human and customer mental model; the coordination layer is the proof. Keep the
  `live`/`seeded` honesty rule, now backed by real rows.

- **Factory metrics on real coordination data.** Throughput, cycle time, pass
  rate, MTTR, backlog/queue-burn (`…-forge-factory-metric-definitions.md`) become
  trivially live once stage transitions are coordination-row transitions and the
  virtual-merge-queue ledger is the queue. The doc itself notes most stage
  sparklines were seeded "until stage transition receipts exist" — the
  coordination store *is* those receipts.

- **Verification as a ladder, not a boolean.** `products/forge.md` §8 already
  separates Evidence Bundle (what was collected) from Verification Report (the
  judgment). The coordination layer's `ForgeVerificationReceipt` is the machine
  rung; Tassadar exact-replay is a stronger rung; model/second-agent/human review
  and owner acceptance are higher rungs
  (`docs/forge/2026-06-28-forge-cross-system-leverage.md` §Tassadar, synergy #3).
  Promotion should *name which rung* cleared it. This directly answers origin.md's
  hardest open question — "a clean merge is not necessarily a correct merge."

- **The automations surface as the agent-orchestration trigger layer.** The blitz
  automations surface already creates real work orders per stage
  (`…-forge-automations-surface.md`). Re-pointed at `POST /api/forge/work-records`
  and the priority dispatcher, it becomes the OpenAgents equivalent of a
  triage-to-PR loop (see §6) — and the change-agent loop origin.md describes
  (observe→classify→plan→patch→push) is the same machine, filed as FORGE-4 (the
  typed forge change-loop state machine) and SU-4.

- **Prefilled vertical workspaces + per-vertical templates as the multi-tenant
  on-ramp.** The vertical templates and seed→invite→engagement loop
  (`…-per-vertical-forge-stage-templates.md`,
  `…-workspace-seeding-invite-engagement.md`) are the *customer-facing front* for
  what the coordination layer already supports underneath: tenant-scoped git auth
  (FORGE-4) and per-tenant namespaces (SU-8 / FORGE-18, "Artanis-as-a-Service").
  A seeded vertical workspace becomes a Forge tenant whose work records flow
  through the same intake→verify→queue→promote pipeline. Verticals are the
  product story; tenant namespaces are the mechanism.

- **Customer-#1 dogfood + cohort evidence as the proof harness.** The dogfood
  strip, spend-routing row, and refs-only cohort ledger
  (`…-customer-one-dogfood-factory.md`, `…-customer-one-spend-routing.md`,
  `…-customer-one-cohort-source-contract.md`) are exactly the SU-7 "dogfood one
  fleet lane end-to-end, prove zero GitHub PR contention" acceptance, dressed as
  a customer narrative. The MVP roadmap's "use Forge to improve Forge" bar
  (`products/2026-04-14-openagents-com-forge-mvp-roadmap.md`) *is* SU-7.

- **Business intake (Slack Connect) as the Signal-stage front door.** The
  `/business` → `business_signup_requests` intake
  (`…-business-slack-connect-intake.md`) is a real Signal-stage source that
  produces work-record candidates with an explicit operator-handoff boundary.
  Keep it; route its output into the coordination intake.

### 4.2 Keep separate — authority that must not collapse into the forge

- **Money and settlement stay receipt-first.** `buyerDebitRequired` is placement
  metadata, not proof a payment moved (`…-customer-one-spend-routing.md`); the
  factory may *display* spend routing but the durable payment/payout/settlement
  authority stays on the existing rails
  (`docs/forge/2026-06-28-forge-cross-system-leverage.md` §Payments, synergy #5).
  The economic bounty→claim→merge→settlement loop is M5/FORGE-17 — a *future*
  fold, gated on the same receipts.

- **Regulated-vertical authority.** The Legal template is explicitly
  workflow-assistance, not legal counsel; jurisdiction-sensitive decisions route
  to a human (`…-per-vertical-forge-stage-templates.md`, "Legal Template" safety
  boundary). Folding the vertical in must not fold its authority in.

- **Runtime vs lifecycle ownership.** `products/forge.md` §9 keeps Probe as the
  runtime/worker layer and Forge as lifecycle authority. The coordination layer
  preserves this: Pylon executes (`forge-dispatch-protocol.ts`,
  `forge-verification-runner.ts`); D1 is authority. Do not let the factory layer
  re-absorb runtime truth, and do not let the forge claim Psionic model-serving
  authority (synergy #6).

### 4.3 Defer — historical framing now superseded

- **The "private Rust control plane + Laravel/React shell" stack** in
  `products/forge.md` §10/§15 is the *2026-04* target. The 2026-06 monorepo
  reality is Bun/Effect/Foldkit on Cloudflare (D1/R2/DO), with the control plane
  in the `apps/openagents.com` Worker and the UI in `apps/forge/`. Treat
  `products/forge.md`'s lifecycle *model* as canonical and its *implementation
  stack* as historical — the boundary contract already re-expresses the same
  authority split in Cloudflare/Effect terms.

- **"Nexus is not Forge authority"** (`products/forge.md` §10) is still true but
  no longer load-bearing; Nexus is deprecated platform-wide.

---

## 5. Where each piece maps onto the stand-up sequence

The factory fold is not a new track — it rides the existing SU-1..SU-8 stand-up
(`docs/forge/2026-06-28-forge-standup-spec.md`) and the FORGE-n roadmap
(`…-owned-coordination-layer-audit.md` §6).

- **SU-1/1B (Forge shell, done).** The shell already routes `/work`, `/changes`,
  `/verification`, `/queue`, `/refs` (`apps/forge/README.md`). *Fold:* add the
  factory **production-line view** and **metrics band** as additional read-model
  surfaces over the same `/shell.json` → `/api/forge/*` contract.
- **SU-2 (control-plane routes, done).** `/api/forge/work-records`,
  `/changes`, `…/status`, `/leases`, `/queue`, `/verification-receipts`,
  `/promotion-decisions` exist. *Fold:* the automations surface and Signal-stage
  intake (Slack Connect) write through `POST /api/forge/work-records`; carry
  `templateRef` + `stageKey` on the row.
- **SU-3 (git intake → archive → canonical refs → coordination, #6771).** This is
  the **Code Gen** stage's authority: a real `git push` lands a packfile in R2,
  updates canonical refs, writes a change record.
- **SU-4 (owned merge authority).** This is **Release**: gated
  `nextActualPromotion` writing a `ForgePromotionDecisionReceipt`. Fold the
  Delivery-Receipt lifecycle vocabulary here.
- **SU-5 (verification on intake).** This is **Validate** + the verification
  ladder; Tassadar replay is the high-confidence rung.
- **SU-6 (GitHub mirror worker).** This is **Deploy**: promoted commit appears on
  GitHub via mirror, not PR.
- **SU-7 (dogfood one fleet lane).** This is the customer-#1 dogfood + cohort
  harness made real, end-to-end, zero PR contention.
- **SU-8 (multi-tenant / AaaS).** This is where prefilled **vertical workspaces**
  become real external tenants on per-tenant git-auth + namespaces (FORGE-4,
  FORGE-18). Gated on the software-solid bar (48h zero-wedge, adversarial harness
  prod-proven).

Cross-system leverage (`docs/forge/2026-06-28-forge-cross-system-leverage.md`)
maps cleanly: **Pylon** is the factory's default runner (Code Gen/Validate),
**Tassadar** is the strongest Validate rung, **Artanis + Blueprint Signatures**
are the Release/Deploy gate authority (every promotion is a Blueprint decision
with evidence refs), **Khala** is the $0 own-capacity lane for Code Gen/Validate,
**Payments** is the receipt-first settlement layer, **Psionic** is the
attach-only ML substrate, and **Nostr/NIP-90** is the external/fallback bus.
The governing rule from that doc holds for the fold too: *share capacity and
evidence, not authority.*

---

## 6. The Linear angle — Forge composes Origin **and** Linear

`docs/forge/linear.md` and `docs/forge/origin.md` describe two different bets on
the agent-era bottleneck ("generation got cheap, coordination got expensive"):

- **Cursor Origin** owns the *forge* — git hosting/storage/merge for agent
  fleets ("where code lives for agents"). This is OpenAgents' **coordination
  layer**.
- **Linear** owns the *product-context + agent-orchestration layer above the
  forge* — issues, triage automations, Code Intelligence, Coding Sessions, and
  Diffs review tied to the issue/customer that caused the change ("where
  agent-written code is judged against product intent"). This is OpenAgents'
  **software-factory layer**.

Linear deliberately does *not* own the forge (GitHub stays its backend); Origin
deliberately does *not* own product context. **Forge's synthesis is to own
both** — the coordination substrate *and* the work/triage/review/measure/deliver
factory on top — plus a native economic loop neither has
(`…-owned-coordination-layer-audit.md` §1.3 differentiators). Concretely, the
historical factory pieces are the OpenAgents analogues of Linear's loop:

| Linear (above-code orchestration) | OpenAgents Forge factory analogue |
|---|---|
| Issue + product/customer context | Work Order + prefilled vertical workspace + cohort source refs |
| Triage automations (issue → agent → draft PR) | Forge automations surface → `POST /api/forge/work-records` → priority dispatch (the change-agent loop, FORGE-4) |
| Coding Sessions (Claude Code / Codex) | Pylon/Khala own-capacity Codex execution via the dispatch protocol |
| Diffs review (PR reviewed beside the issue) | change inspector + verification ladder beside the work record |
| "30% of bugs auto-resolved first pass" | SU-7 dogfood lane: own-capacity triage-to-promotion |
| Human stays accountable for merge | Blueprint-signature-gated promotion (no merge without evidence refs) |

The honest caveat Linear surfaces applies to the fold: coding agents show
"action bias" (proposing changes on stale issues where none is needed). The
coordination layer's structural guards — issue-close-safe gate, the anti-deletion
guard, verification-before-promotion — are exactly the "do nothing if already
fixed / clean ≠ correct" discipline that makes an automated triage-to-PR factory
safe at N.

---

## 7. Current vs future — an honest scorecard

**Built and on `main` (coordination layer):** FORGE-1..6 (coordination store,
git-receive-pack parser, packfile archive, tenant git auth, dispatch protocol,
verification runner); M0 (virtual merge queue + gates, priority dispatch, fan-out
coordinator); SU-0 boundary contract; SU-1/1B `apps/forge/` shell; SU-2
`/api/forge/*` routes + receipts migration.

**Built but on the OLD authority (factory layer):** the `/forge` factory
dashboard, stage bucketing, factory metrics, automations surface, per-vertical
stage templates + typed seeds, prefilled-workspace seed→invite→engagement,
customer-#1 dogfood strip / spend-routing / Slack intake / cohort ledger — all
live in `apps/openagents.com` on the **Autopilot Work** control plane, which the
stand-up spec calls source material, not the expansion target.

**Envisioned (the fold):** SU-3..SU-8 (git intake → owned merge authority →
verification on intake → GitHub mirror → dogfood → multi-tenant) and M3-M5
(owned merge authority, relay sub-second fan-out, economic loop); and the
unification that re-homes the factory read-model (stages, metrics, verticals,
cohorts, automations) onto the coordination store's work-record/change/status/
verification/promotion rows so there is one authority and one render.

**Not yet anywhere:** a single Work Order whose full
`products/forge.md` lifecycle (queue→lease→run→verify→deliver→archive) is backed
end-to-end by the coordination store; durable Run recovery/handoff on the new
authority; the economic bounty→settlement loop wired to receipts.

No production-data, secrets, tokens, or personal names are involved in any of the
above; everything cited is public-safe code, migrations, and docs.

---

## 8. Biggest synergies first

1. **Re-home the factory read-model onto the coordination store (one authority,
   one render).** Make the eight-stage production line, factory metrics, and
   per-run workrooms pure projections of work-record / change / status /
   verification-receipt / promotion-decision rows. Carry `templateRef` +
   canonical `stageKey` on the work record. This collapses the "two dashboards"
   problem, makes the `live`/`seeded` honesty automatic, and is the
   cross-system-leverage doc's synergy #4. *Highest leverage; unblocks the rest.*

2. **Adopt the Work Order / Delivery-Receipt lifecycle vocabulary as the
   product-facing layer over coordination rows.** Use `products/forge.md`'s
   state tables as the customer-legible names for SU-2/SU-4 state — without
   building a second authority. Turns a git forge into a *work system*.

3. **Make verification a named ladder and name the rung on every promotion.**
   `ForgeVerificationReceipt` (machine) → Tassadar exact-replay → model /
   second-agent / human review → owner acceptance. Directly answers
   origin.md's "clean ≠ correct" and Linear's "action bias" risks. Maps to SU-5.

4. **Re-point the automations surface at `/api/forge/work-records` + priority
   dispatch as the change-agent loop (FORGE-4).** This is the OpenAgents
   triage-to-PR factory and the Linear/Origin convergence in one — observe →
   classify → plan → patch → push → verify → gated-promote, Blueprint-gated so it
   cannot act without evidence.

5. **Turn prefilled vertical workspaces into the multi-tenant on-ramp (SU-8).**
   Vertical templates are the customer story; tenant-scoped git auth (FORGE-4) +
   per-tenant namespaces (FORGE-18) are the mechanism. "Artanis-as-a-Service" is
   verticals on the owned coordination layer.

6. **Promote the customer-#1 dogfood + cohort harness into the SU-7 acceptance.**
   "Use Forge to improve Forge, zero GitHub PR contention" is both the historical
   dogfood bar and the stand-up's end-to-end proof — same milestone, two names.

7. **Keep money, regulated-vertical authority, and runtime ownership separate**
   while folding everything else. Receipts-first settlement (synergy #5), human
   gates for legal work, and Probe/Pylon-as-runtime vs D1-as-authority must not
   collapse into the forge. The economic bounty→settlement loop is a *future*
   fold (M5/FORGE-17), gated on the same receipts.

**The compose, in one line:** Origin gives us *where agent code lives*; Linear
gives us *where agent code is judged against intent*; the historical
software-factory vision gives us *the lifecycle, the verticals, the metrics, and
the customer*. Forge is all three on one owned, evidence-gated coordination layer
— a git forge **and** a software factory.

---

## Related docs

- `docs/forge/2026-06-28-forge-openagents-com-owned-coordination-layer-audit.md`
- `docs/forge/2026-06-28-forge-standup-spec.md`
- `docs/forge/2026-06-28-forge-boundary-contract.md`
- `docs/forge/2026-06-28-forge-cross-system-leverage.md`
- `docs/forge/origin.md` · `docs/forge/linear.md`
- `docs/blitz/forge/*` (factory metrics, automations, per-vertical stage
  templates, prefilled workspaces, customer-one dogfood/spend/cohort, Slack
  intake)
- `docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`
- root workspace `products/forge.md` +
  `products/2026-04-14-openagents-com-forge-mvp-roadmap.md`
- `apps/forge/README.md` · `packages/forge-protocol/`
- `apps/openagents.com/workers/api/src/forge-coordination-store.ts`,
  `forge-git-packfile-archive-store.ts`, `forge-tenant-git-auth-store.ts`,
  `forge-control-plane-routes.ts`
- `apps/pylon/src/git-receive-pack.ts`, `forge-dispatch-protocol.ts`,
  `forge-verification-runner.ts`, `virtual-merge-queue.ts`,
  `blueprint-gates/*`, `coordinator/`
</content>
</invoke>

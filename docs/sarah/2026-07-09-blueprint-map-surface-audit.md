# Sarah Blueprint Map surface — audit, plan, roadmap

Date: 2026-07-09
Status: Audit + plan (no implementation in this change)
Owner directive (verbatim): "Go look up our Unit/Arbiter info. I want to
build a Blueprint map as Sarah is talking. Visual representation of what she
knows / is trying to learn from the user. Right now the Sarah page has too
much bullshit on it. In a desktop computer i want her taking up ~half of the
screen (left), other half the canvas with graph / buttons / code / chat etc."

Vocabulary law: this surface is the **Blueprint map** (the name Blueprint's
own master spec reserved for its deferred visual graph surface). Company-brain
/ Blueprint vocabulary only.

## 1. Unit / Arbiter — what exists today

### 1.1 The lineage (Unit → Arbiter)

- **Unit** is `samuelmtimbo/unit`, the external 2D dataflow visual-programming
  system, kept strictly as a **read-only reference repo**
  (`projects/repos/unit/` in the workspace). The 2026-06-30 audit explicitly
  rejected adopting it and directed a purpose-built rebuild of its good
  primitives (typed-pin MIMO nodes, first-class links, JSON graph spec with
  embedded layout, force auto-layout, direct manipulation, live datum,
  evidence-bound edge lighting) on Effect + our own renderers.
- **Arbiter** is the resulting owned library. Naming commit `ba3299933b`
  ("docs(unit): name it Arbiter; sync arbiter-effect with Blueprint") chose
  the Protoss Arbiter (a control-plane ship) and, critically, established the
  Blueprint alignment: **Blueprint already defines a typed, governed dataflow
  graph** (Business Objects + Relationships as the data graph; Program
  Signatures composed into plan→write→verify as the control graph) **and
  named-but-deferred its visual surface as "Blueprint Map."** Arbiter is that
  deferred surface. The audit's mapping table is one-to-one (node ↔ Business
  Object / Program Type; typed pin ↔ Signature field; link ↔ Relationship /
  Program composition; lit edge ↔ Trust Receipt) and its hardest rule — **a
  link only lights on a real, dereferenceable receipt** — is Blueprint's
  evidence law rendered directly in UI.
- The full audit was retired from the tree with the Tassadar/Psionic doc
  prune (`e1fbd1c185`); recover it with
  `git show a6ff96321b:docs/unit/2026-06-30-arbiter-effect-2d-dataflow-graph-audit.md`.

### 1.2 The implementation: `packages/arbiter-effect`

`@openagentsinc/arbiter-effect` (workspace package, two entries):

- **`./core` (`src/core.ts`)** — the public graph contract, Effect Schema,
  schema id `openagents.arbiter.graph_spec.v0`:
  - `GraphNode { id, label, kind, status, inputs/outputs: GraphPin[], datum:
    GraphDatum[], evidenceRefs, blockerRefs, caveatRefs, position }` with
    `GraphNodeStatus = idle | active | blocked | complete | proposal_ready`.
  - `GraphLink { id, label, status, from/to: GraphPinRef, evidenceRefs,
    blockerRefs, caveatRefs }` with
    `GraphLinkStatus = inactive | active | blocked | evidence_backed` and
    `graphLinkStatusForRefs()` deriving status from refs (evidence lights,
    blockers block).
  - Public-safety ref discipline built in: `isDereferenceableGraphRef()`
    rejects raw paths/tokens/emails/payment material; counters alone never
    count as evidence (`graphCounterOnlyRefPattern`).
  - Geometry helpers (pin anchors, bezier `graphLinkPath`) and a default
    layout.
- **`./foldkit` (`src/foldkit.ts`)** — the read-only SVG/HTML renderer proven
  in Khala Code Desktop. Direct manipulation, live subscriptions, and
  Three.js rendering are intentionally out of the package so far.

Current consumers: Khala Code Desktop (`gym-graph-renderer.ts`,
`gym-pane.ts`, `qa-swarm-panel.ts`, `fleet-board-renderer.ts` /
`fleet-board-projection.ts`), the web QA Swarm board
(`apps/openagents.com/apps/web/src/page/qa-swarm.ts` + projection), and
`workers/api/src/product-promises.ts`.

### 1.3 How it plugs into Effect Native (the part that matters for /sarah)

- **EN-6 (#8575, CV4)** is the unification lane: fold `three-effect` and
  `arbiter-effect` under the EN canvas contract so graph/3D surfaces are EN
  components with a canvas adapter, not parallel islands
  (`docs/effect-native/2026-07-08-effect-native-one-ui-substrate-analysis.md`
  §EN-6; `2026-07-08-three-effect-vs-effect-native.md` "arbiter-effect (the
  typed graph renderer, already pluggable) folds under" the canvas renderer).
- The EN catalog **already shipped the graph component**: `GraphFigure`
  (catalog v19, upstream effect-native#37) — "typed arbiter-graph model" —
  with `GraphNodeModel { id, label, kind: worker|validator|arbiter|task|
  generic, status: idle|active|success|failed|pending, x?, y? }`,
  `GraphEdgeModel { id, from, to, kind: flow|dependency|pairing, status }`,
  typed layout policy (`precomputed | force | tree` via shared
  `layoutGraphNodes`), typed pan/zoom camera, status→theme-token colors, and
  named typed intents `onNodeSelect / onNodeHover / onCameraChange`. Two
  render paths under one contract: the canvas scene adapter
  (`graphFigureToScene` in `@effect-native/render-canvas`, live Three
  backend) and a **DOM/SVG fallback in `@effect-native/render-dom`**. RN has
  a declared read-only subset (upstream #53). Companion `Timeline`.
- The monorepo's vendored EN snapshot
  (`apps/openagents.com/packages/effect-native-core`,
  `0.0.0-openagents.8567`, catalog **v26**) already includes `GraphFigure`
  and the render-dom fallback, and `apps/sarah` already depends on
  `@effect-native/core` + `@effect-native/render-dom` (`workspace:*`).

**Conclusion:** the Sarah Blueprint map does NOT need a new renderer. The
`/sarah` EN DOM surface renders `GraphFigure` today. `arbiter-effect/core`
remains the richer typed vocabulary (pins, datum, evidence/blocker refs,
`evidence_backed` links) — the Sarah projection should keep its node/edge
semantics aligned with that vocabulary so the same data can later render on
the unified EN-6 canvas without a second schema. Gaps between what the map
wants and what `GraphFigure` offers (see §5.3) route through the EN demand
register — **components go upstream, never local one-offs** (EN-2 discipline,
`docs/sarah/EN-GAPS.md`, `docs/effect-native/DEMAND_REGISTER.md`, upstream
`GAPS.md`).

## 2. Blueprint data — what a live map can render

Three typed sources exist in `apps/sarah/src/services/`; all reads are bound
to `prospectRefAliases(prospectRef)` (KHS-3 isolation law,
`sarah.cross_prospect_isolation.v1` / `sarah.memory_query_scoped.v1`).

### 2.1 What she has learned about YOU — `customer-blueprint.ts` (KHS-9 #8608)

`CustomerBlueprintDraft` (schema `sarah.customer_blueprint_draft.v1`,
revisioned in `sarah_customer_blueprints`):

- `business.facts: SarahProspectFact[]` — non-need profile facts, each
  `{ fact: "<label>: \"verbatim quote\"", sourceTurnId, at }` (labels:
  company / role / stack / contact).
- `needs: CustomerBlueprintNeed[]` — stated needs with source turn ids.
- `contacts { email, contactId }`.
- `suggestedModules: SuggestedModule[]` — need→offering mapping over the
  deal-rules modules + workspace packs, `matchBasis: "semantic" |
  "candidate_default"` (embedding+cosine only; honest degradation, never
  keyword), `matchedNeedTurnIds` provenance, `pricingStatus` verbatim (no
  improvised pricing).
- `sources { turnIds, factCount, provenance }` and the honest
  operator-assisted `handoff` block.

Built by the owned-runtime tool `customer_blueprint_draft`
(`agent-runtime/owned-runtime.ts`) during turns; today it publishes one
flat `card` event ("Your Blueprint draft" + a text summary) per alias.

### 2.2 The fact stream — `prospect-memory.ts` (KHS-2 #8601)

`distillProspectFacts()` distills deterministic verbatim-quote facts (cue
labels **company / role / need / stack / contact**) from
`sarah_transcript_turns` at read time on every model-path turn, and upserts
the `sarah_prospect_profile` projection. Per-fact provenance turn ids are
preserved everywhere. This is the natural per-turn delta source: a new fact
appearing in the distillation IS "Sarah just learned something."

### 2.3 What she knows about OpenAgents — `sarah-blueprint.ts` (KHS-5 #8604)

Sarah's own Blueprint: `BlueprintFact` rows across nine sections (identity,
conversation_rules, hard_rules, company, products, pricing, proof, links,
playbook) with per-fact provenance, `dealRuleRefs`, `promiseIds`, and
receipted revisions (`sarah.blueprint_versioned_provenance.v1`). For the map
this is background, not the star: the prospect-facing graph is about THEIR
Blueprint. Her own facts appear only as the offering nodes the needs map onto
(modules/packs are exactly the deal-rules + KB objects).

### 2.4 "What she is trying to learn" — derivable, no new inference

The cue-label set is closed, so the unknowns are computable, honestly:

- Fact slots with no distilled fact yet (e.g. no `company`, no `contact`) →
  **pending/idle nodes** ("still learning").
- Offerings with `matchBasis: "candidate_default"` or no `matchedNeedTurnIds`
  → unlit module nodes; a **semantic** match flips the need→module edge on,
  with `matchedNeedTurnIds` as the provenance the edge "lights" from
  (the arbiter `evidence_backed` discipline: turn ids are the receipt).
- No account link → the account node stays pending until KHS-7 links it.

### 2.5 The streaming seam — `avatar-event-bus.ts` (AV-3 #8598)

In-process bus keyed by `conversationRef`; browser subscribes over SSE
(`GET /sarah/api/avatar/events?ref=…`, `avatar-session.ts` `EventSource`).
Current typed union: `transcript | card | guard_refusal | session`. **What
must be added (BM-1):** a `blueprint_delta` event type — published from the
turn loop wherever knowledge lands:

- `prospect-memory.ts` profile upsert → `fact_added` deltas (label + safe
  display text + sourceTurnId).
- `customer_blueprint_draft` tool → `draft_revision` delta (needs count,
  matched modules with matchBasis, revision) replacing/augmenting today's
  flat card.
- contact capture / KHS-7 account link → `contact_linked` / `account_linked`
  deltas.

Scope note: the SSE stream already serves the prospect's own browser keyed by
their own ref, so their own facts are in-scope by construction; deltas must
still pass the same public-safety posture as everything on the bus (no other
prospect's data can ever appear — the publisher only ever holds this ref's
facts, and the isolation contracts oracle that).

## 3. Current /sarah page — inventory and verdicts

Surface: `apps/sarah/src/ui/` — `index.html` (chrome), `sarah.css` (layout),
`main.ts` (EN surface at vendored catalog v26), `avatar-session.ts` (media +
SSE binding). Zero React; EN DOM renderer with the `MediaVideo` host driver.

| Item | What it is | Verdict |
|---|---|---|
| `header.sarah-disclosure` | AI-disclosure banner (server HTML above the EN mount) | **KEEP — contract.** Restyle compact (one line, top of right pane or thin top strip); copy unchanged. Later EN adoption target is `StatusBanner` (v16) per EN-GAPS. |
| `.sarah-layout` grid | Avatar column capped at 480px (3:4, sticky) + content column capped at 720px, centered with dead margins | **REPLACE** with the split layout (§4). This cap is the core of "too much bullshit": Sarah is small, the page is mostly padding. |
| Header row: `Sarah` title | Page title inside the EN tree | CUT as a standalone row — the video IS the identity. Fold status/account into a slim right-pane toolbar. |
| Header row: subtitle caption `OpenAgents sales · openagents.com/sarah` | Redundant self-reference | **CUT.** |
| Status badge (`LIVE`/`LIVE · sandbox`/state) | Honest session state | KEEP, relocated to the right-pane toolbar (or video-pane overlay corner). |
| Account chip / `Create account / Sign in` button (KHS-7) | In-conversation account linking | KEEP — becomes one of the Actions (§4); chip stays in the toolbar when linked. |
| Avatar controls row (`Talk to Sarah` / `End conversation` / `Avatar offline`) | Full-width row of one button | KEEP the control, CUT the row — overlay it on the video pane (idle overlay already exists in the chrome CSS). |
| `Transcript` (EN, v17) + composer | The chat | KEEP — becomes the right-pane **Chat** tab (transcript + composer together). |
| Cards `List` (tool-effect cards: instant answer, Blueprint draft, account linked, guard refusals) | Flat card feed | KEEP the data, CUT the feed-as-layout: card content routes into the right pane — Blueprint-draft cards become graph deltas (BM-1/BM-2), receipts/links land in the Receipts panel (BM-4), transient notices stay as toasts/inline transcript lines. |
| `#sarah-avatar` idle/connecting overlays | Chrome CSS states | KEEP (moved onto the new left pane). |

## 4. Desktop layout spec (the owner's split)

**Desktop (≥ ~1024px):**

- **Left pane — Sarah, ~50vw, full viewport height.** The `MediaVideo` host
  fills the pane (`object-fit: cover`), idle/connecting overlays as today;
  the start/end conversation control and the status badge overlay the video
  (bottom edge). Nothing else lives on the left.
- **Right pane — the canvas (~50vw, full height), tabbed/stacked:**
  1. **Blueprint map (primary/default tab)** — the live graph (§5): what she
     knows about this prospect, what she's still trying to learn, and which
     offerings the needs map onto. Nodes animate in as `blueprint_delta`
     events land while Sarah talks.
  2. **Chat** — the EN `Transcript` + composer (exactly today's pair).
  3. **Actions** — typed buttons: Create account / Sign in (KHS-7 flow,
     exists), Link account (linked chip when done), Book a human
     (`human_handoff` tool), and the checkout link button when a
     `checkout_link_create` result exists.
  4. **Code / receipts** — shown when relevant: the current
     `CustomerBlueprintDraft` revision as pretty-printed JSON (the "code"
     view of the map), tool receipts (checkout links, handoff confirmations,
     account-link receipt), guard-refusal notices.
- The AI-disclosure line renders compact above the right pane (copy
  unchanged).
- Tab strip is a slim toolbar that also carries the status badge + account
  chip.

**Mobile (< ~900px):** stack vertically — video (bounded height, as today's
media query) → tab strip → active panel; Blueprint map remains available but
Chat may be the default active tab on small screens (one-thumb reachability).

All of it authored in the EN component set on the DOM renderer — no new
parallel primitives; the layout uses catalog pieces (`SplitPane`/`Stack`/
`Tabs` where vendored; gaps go to the demand register).

## 5. The Blueprint map graph

### 5.1 Shape (projection, pure function)

`blueprintMapProjection(draft | facts) → { nodes, edges }` in
`apps/sarah/src/ui/` (pure, unit-testable):

- Center node: the prospect (label from contact email or "You"; status
  `active` when live).
- Fact-slot nodes for the closed cue-label set (company / role / stack /
  contact): `success` when a fact exists (label = safe short text),
  `pending` when Sarah is still trying to learn it. This IS the
  "knows vs trying to learn" visual.
- Need nodes: one per stated need (`success`/`active`), edge from prospect.
- Offering nodes (modules + packs from the draft): edge need→module lights
  only on `matchBasis: "semantic"` with `matchedNeedTurnIds` provenance —
  the arbiter law (an edge lights only from real provenance) carried over.
  `candidate_default` offerings render unlit/idle or are elided (default:
  show only semantic matches + one collapsed "candidates" node — honesty
  without noise).
- Account node: `pending` → `success` on KHS-7 link.

### 5.2 Streaming behavior

`avatar-session.ts` already holds the `EventSource`; `blueprint_delta`
events update surface state → the `GraphFigure` view re-renders and new
nodes/edges animate in (EN handles keyed reconciliation; entry animation is
a renderer/theme concern — see gaps). On session start, the map seeds from
the latest stored draft revision (returning visitors see what she already
knows immediately — the KHS-2 memory made visible).

### 5.3 Known `GraphFigure` gaps to route upstream (not blockers)

v1 ships on the existing contract (`kind: "generic"|"task"` + status +
label carry the semantics). Candidate demand-register entries if v1 proves
them: (a) domain-neutral node kinds or a typed badge/accent slot (fact vs
need vs offering); (b) node-entry animation policy; (c) typed pin/datum
chips (the arbiter-effect `GraphDatum` idea) for inline provenance display;
(d) an `evidence_backed` edge status distinct from `active`. Each goes to
`docs/sarah/EN-GAPS.md` + `docs/effect-native/DEMAND_REGISTER.md` + upstream
GAPS per the EN-2 discipline; nothing is forked locally.

## 6. Lanes (epic #8626; BM-1..5 = #8627–#8631)

- **BM-1 (#8627) — `blueprint_delta` SSE events from the turn loop.** Extend the
  `SarahAvatarEvent` union with a typed `blueprint_delta` variant
  (`fact_added | draft_revision | contact_linked | account_linked`);
  publish from `prospect-memory.ts` (profile upsert), the
  `customer_blueprint_draft` tool, and the KHS-7 link path; per-alias
  publish exactly like today's cards; isolation posture covered by the
  existing contract oracles + a new publisher test.
- **BM-2 (#8628) — Blueprint map graph on the /sarah EN surface.** Pure projection
  module (draft/facts → `GraphFigure` nodes/edges, vocabulary aligned with
  `arbiter-effect/core`), `GraphFigure` render on the DOM fallback, seed
  from the stored draft, animate on deltas; catalog gaps routed upstream
  (EN-2 / EN-6 #8575 cross-links).
- **BM-3 (#8629) — split desktop layout + page declutter.** The §4 layout; cuts:
  subtitle caption, standalone title row, avatar-controls row (control moves
  onto the video), cards-feed-as-layout; disclosure restyled compact (copy
  unchanged). Register the owner's layout directive verbatim as a behavior
  contract (`sarah.split_screen_blueprint_map.v1`) with a layout oracle.
- **BM-4 (#8630) — Actions tab + code/receipt panel.** Typed action buttons
  (account link exists; Book a human → `human_handoff`; checkout link when
  minted) and the draft-JSON/receipts panel.
- **BM-5 (#8631) — owner playback/QA gate.** Screenshot smoke of the split layout
  (desktop + mobile viewports) on the SQ-4 (#8621) deploy-smoke rail, the
  BM-3 behavior contract enforced in the sweep, and a synthetic-prospect
  run that proves a fact spoken to Sarah appears as a node while the session
  is still live.

Dependencies: BM-1 ⊣ BM-2 (deltas feed the graph, though BM-2 can render the
seeded draft first); BM-3 hosts BM-2/BM-4; BM-5 gates the owner cutover.
Cross-links: EN-6 #8575 (canvas unification — the map moves onto the unified
canvas contract when CV4 lands, same typed model), OAV #8610 (the left pane
is the owned-renderer video seam; layout must not assume HeyGen), SQ-4 #8621
(smoke rail + contracts), KHS epic #8599 (data sources).

## 7. References

- Recovered Arbiter/Unit audit:
  `git show a6ff96321b:docs/unit/2026-06-30-arbiter-effect-2d-dataflow-graph-audit.md`
  (naming commit `ba3299933b`; extraction commit `5c64b20b3b`)
- `packages/arbiter-effect/src/{core,foldkit}.ts`
- EN GraphFigure: upstream effect-native#37 (catalog v19); vendored
  `apps/openagents.com/packages/effect-native-core/src/index.ts` (v26)
- EN-6: #8575; substrate analysis
  `docs/effect-native/2026-07-08-effect-native-one-ui-substrate-analysis.md`
- Sarah data: `apps/sarah/src/services/{customer-blueprint,prospect-memory,sarah-blueprint,avatar-event-bus}.ts`,
  `apps/sarah/src/agent-runtime/owned-runtime.ts`
- Surface: `apps/sarah/src/ui/{index.html,sarah.css,main.ts,avatar-session.ts}`
- Contracts: `docs/sarah/SARAH_CONTRACTS.md`,
  `apps/sarah/src/contracts/isolation-contracts.ts`
- EN demand: `docs/sarah/EN-GAPS.md`, `docs/effect-native/DEMAND_REGISTER.md`

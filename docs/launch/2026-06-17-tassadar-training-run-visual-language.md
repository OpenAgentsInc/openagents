# Tassadar training-run visual language: initial primitives

Date: 2026-06-17
Status: initial design note
Scope: `openagents.com` product surface, `/animations`, `/components`, `/`,
`/run`, and the Autopilot Desktop first screen.

This note is not product copy and does not flip any promise state. It records
the first visual-language pass for the Tassadar run after reading the launch
folder and reviewing the current web and desktop surfaces.

## Source material reviewed

- `docs/launch/JUNE15_LAUNCH_PLAN.md`
- `docs/launch/JUNE16_ROADMAP.md`
- `docs/launch/JUNE17_ROADMAP.md`
- `docs/launch/2026-06-17-orrery-payout-accounting-and-spark-unification-audit.md`
- `apps/openagents.com/DESIGN.md`
- `apps/openagents.com/apps/web/src/page/animations.ts`
- `apps/openagents.com/apps/web/src/page/components.ts`
- `apps/openagents.com/apps/web/src/page/loggedOut/page/pylon.ts`
- `apps/openagents.com/apps/web/src/page/run.ts`
- `apps/openagents.com/apps/web/src/scene/pylonDiamonds.ts`
- `apps/openagents.com/apps/web/src/scene/pylonBezierNetworkElement.ts`
- `apps/openagents.com/apps/web/src/scene/pylonStatsElement.ts`
- `apps/openagents.com/apps/web/src/scene/tassadarRunSnapshot.ts`
- `apps/autopilot-desktop/src/ui/model.ts`
- `apps/autopilot-desktop/src/ui/view.ts`
- `apps/autopilot-desktop/src/shared/pylon-network-scene.ts`
- `apps/autopilot-desktop/src/ui/pylon-network-visualization.ts`

2026-06-21 update: the old web `/tassadar` live scene/custom element has been
retired. The in-world Tassadar/Pylon surface now belongs in Autopilot Desktop
Verse; the web route is a guardrail notice plus links to the public summary API
and proof replay.

Live snapshot checked while writing:

- `/` serves the client shell plus an embedded `openagents-pylon-stats-snapshot`.
- `/api/public/pylon-stats` was live with 9 pylons online, 2 wallet-ready, 2
  assignment-ready, 6 training-progress contributors, and real settled treasury
  outflow totals. These numbers are only a writing-time snapshot.
- `/api/public/tassadar-run-summary` was live with the run active, 3 accepted
  replay-verified traces, 3 verified work rows, 3 rejected rows, 6 assigned
  contributors, 1 qualified contributor, and 5 provider-confirmed settled payout
  sats. These numbers are also only a writing-time snapshot.

## Current visual language

### Homepage pylon layer

The root route is the Pylon page. It is a full-viewport black field with these
layers:

- A top `Download Autopilot` CTA.
- The refractive two-diamond pylon scene, using the shared Moksha diamond GLB,
  backface/refraction shader passes, slow rotation, and live blue activity glow.
- A live bezier network overlay, where pylons sit in a deterministic ring/spiral
  and quadratic curves flow inward to the central pylon.
- A live stats overlay with slot-text digit rolls: pylons online, work-ready now,
  sats settled over 24h, and training contributors.
- The post-launch live-copy gate.

This is a good network-presence language. It says: "the fleet is alive, work is
flowing toward the center, and the counters are not static marketing numbers."
It is not yet a full training-run language because it does not distinguish
worker, validator, replay, rejected trace, accepted trace, settlement, corpus, or
proof link.

### Autopilot Desktop first screen

The desktop app opens on `pane: "network"`, not a sidebar app shell. The first
screen is an immersive network view:

- A full-screen `three-effect` network graph from live pylon stats.
- The exact homepage pylon-diamond shader composited at the center with
  transparent background.
- An overlay headline such as "`N` pylons online", activity state
  (`network dormant`, `online idle`, `work in flight`), install-readiness status,
  and a `Go online` action.
- A denser stats bank: working now, sellable online, wallet ready, assignment
  ready, seen in 24h, registered, sats settled, training assigned, training
  accepted, and training progress.

This is the strongest current "operator cockpit" primitive. It makes the network
feel like an instrument panel rather than a landing page. The visual-language
constraint for Tassadar should be: keep this cockpit density, but make the
training semantics more explicit when a user enters the run surface.

### `/animations`

The animations route is an internal three.js playground. The relevant families:

- `oa-pylon-bezier-network`: the live pylon network overlay used by `/`.
- Bezier graph permutations: ring, spiral, and web variants.
- Constellation, instanced field, flow field, tube flow, glow knot, wobble
  sphere, shader gradient, grid floor, wireframe, particles, light beams.
- Blob-tracking data aesthetics: tracked centroids, hot cores, coordinate
  readouts, connector splines, dust, and strand backgrounds.

The useful direction here is not "add more abstract effects." It is to graduate
specific animation grammars into named training primitives: trace paths, replay
pairs, verifier beams, settlement bursts, corpus accretion, and quarantine
windows.

### `/components`

The components workbench already provides the UI side of the language:

- Primitives: surfaces, tones, badges, status dots, buttons, links.
- Shared controls: heading blocks, buttons, link buttons, avatars, menus.
- Forms: input groups, validated inputs, selects, toggles, radios, checkboxes.
- Layout: sections, cards, drawers, modal surfaces, notification stacks.
- Navigation: tabs, nav, breadcrumbs, vertical nav, command palette, progress
  list.
- Data display: tables, key/value rows, code blocks, stat grids, description
  lists, stacked/feed/grid lists.
- Feedback: alerts and empty states.
- Workroom: panels, action docks, timelines, checklist, composer shape.
- Public/page examples/v4/AI Elements for larger compositions.

For Tassadar, this means the product surface should not invent a new chrome. Use
the workroom/data-display families for evidence, proof links, and ledger state;
use the scene layer only for spatial comprehension.

### `/run`

The live Tassadar run page is already the first dedicated training-run surface.
It fetches `/api/public/tassadar-run-summary`, maps absent data to honest zeroes,
and renders the `three-effect` training-run scene. The adapter already knows how
to produce:

- A run snapshot: active/planned/sealed/reconciled windows, assigned contributors,
  verified/rejected work, pending payouts, receipt refs, settled sats, device
  requirements, closeout state, Freivalds ref counts, and blocker counts.
- Data-bound entities from leaderboard pylons.
- Worker and validator entities from verified replay pairs.
- Beams for verified worker-to-validator replay.
- Bursts for settled payout rows.
- Click-through proof links for run, window, proof, receipt, settlement, pylon
  evidence, and verified replay challenge refs.

This should become the base visual grammar for the run, not a side demo.

## Product truth constraints

The launch docs make the copy and visual constraints unusually strict:

- Tassadar is the run that trains the model, not a trained model claim.
- Exact replay is the main verification affordance. It is stronger and simpler
  than vague "AI confidence" visuals.
- Worker and validator are distinct roles. A single glowing node cannot stand for
  the whole loop.
- Accepted work, payout dispatch, settled payout, recipient-confirmed balance,
  and corpus growth are different states.
- Pending work must not look paid. Rejected traces must not look like progress.
- Public counters must be live/provenance-labeled and must tolerate zero states.
- Secrets, raw prompts, raw logs, invoices, hashes, preimages, wallet material,
  and private paths must not leak into visual details or screenshots.

The useful visual direction is therefore "operational proof instrument", not
"sci-fi training page."

## Evidence-bound motion contract

This is the hard motion rule for Tassadar and any training-run scene derived
from it:

> If it moves, pulses, flows, or bursts, it must be tied to a real public ref or
> a measurable live state transition. No decorative data motion.

That means:

- A moving pylon mark needs a `pylon.*` ref or an explicit live presence/ref
  projection.
- A replay pulse needs a `training.verification.challenge.*` ref, worker ref,
  validator ref, and verdict/source refs.
- A trace strand needs a submitted, queued, replayed, rejected, or accepted
  trace/challenge ref. Counts alone are not enough to animate a trace.
- A receipt or payout burst needs a settlement receipt row. A real-Bitcoin
  payout burst also requires `realBitcoinMoved:true`.
- A corpus animation needs accepted trace refs, not just accepted-trace count.
- A counter roll needs the previous value, next value, timestamp, and source
  projection. If the value is first load only, it can appear but should not
  simulate ongoing flow.
- A liveness heartbeat needs a current heartbeat/presence source and should age
  out or freeze when the source is stale.

Negative rule: do not render "data moving back and forth" merely because the
graph has an edge. Fixed stage edges may exist as static structure. Their motion
must be absent until the adapter supplies motion events with refs. An aggregate
state like `verified work: 3` can color a stage node, but it cannot create three
anonymous pulses.

Every animated primitive should expose this metadata in code, test fixtures, and
proof inspection:

- `motionId`: stable ID for the animation instance.
- `motionKind`: `presence`, `assignment`, `trace_submitted`, `replay_verified`,
  `replay_rejected`, `settlement_recorded`, `real_bitcoin_moved`,
  `corpus_accepted`, `counter_changed`, or another typed event.
- `sourceRefs`: the public refs that authorize the motion.
- `generatedAt`: projection timestamp.
- `expiresAt` or stale policy for liveness/presence motion.
- `simulated`: true only for explicitly labeled simulation evidence; simulated
  motion must not share the real-settlement encoding.

If any of that metadata is missing, the fallback is static state: show the node,
show the count, show the proof drawer row, but do not animate it.

## Motion roadmap

Status as of 2026-06-17: the first enforcement pass is live in code.
`@openagentsinc/three-effect` exposes `motionPolicy`, keeps structural edges and
ambient orbit motion static by default, and can require `sourceRefs` before
rendering beams or bursts. A follow-up simplification made `/tassadar` stricter
than the general primitive: it currently emits no replay beams, no payout bursts,
no contributor-orbit dots, and no loss panel in the primary scene. The main
field is the run node plus public-ref entities only.

The same distinction applies to shape language. Stage labels such as
`registered`, `qualified`, `state synced`, `active`, and `sync reentry` are
aggregate run-stage counters, not pylons. They should render as compact gate
markers in a legend or secondary support surface, not as spatial nodes in the
`/tassadar` main field. The pylon/record orb language belongs to actual refs:
`P1` through `P6`, replay workers, validators, accepted trace refs, and receipt
refs.

1. Keep the `motionPolicy` tests green: base edge flow pulses must stay disabled
   for `/tassadar`, and anonymous renderer motion must not re-enter through
   graph topology.
2. Replace any future renderer pulse or replay traffic with a richer typed
   motion-event layer and a clear proof/legend treatment. The
   renderer should accept arrays such as `traceMotions`, `replayMotions`,
   `settlementMotions`, and `presenceMotions`, each carrying the metadata above.
3. Make the web adapter responsible for deriving all motion events from
   `/api/public/tassadar-run-summary`, `/api/public/product-promises`, and
   `/api/public/pylon-stats`; the renderer must not invent them from graph
   topology.
4. Keep `/components/training` and `/animations` demos honest by labeling
   unbound studies as static prototypes, or by feeding them fixture refs that
   are visibly fixture-only. A demo can study form, but it must not train future
   implementation habits around anonymous motion.
5. Add regression coverage that `tassadarRunVisualizationOptions(summary)` does
   not produce motion/burst/flow definitions without `sourceRefs`, and that
   simulation settlement never produces a real-Bitcoin motion.
6. Add a browser smoke or canvas-inspection check for `/tassadar` that fails if
   base edge pulses are active while the live payload has no matching motion
   events.
7. Update the proof drawer to show the source refs for any selected animated
   mark. The user should be able to ask "why did that move?" and get a public
   answer immediately.
8. Keep lifecycle-stage concepts such as registration, qualification, synced
   state, active window, and sync reentry out of the main spatial field. If they
   are needed, render them as a compact legend/table that clearly reads as
   aggregate state, not as a pylon/node cluster.

## Proposed compositional primitives

### 1. Run field

A full-bleed spatial field for one training run. It should include the run node,
active window, corpus target, proof gate, settlement gate, and contributor ring.
This is the container primitive for `/run` and any desktop Training Live pane.

Use when the user needs to understand the whole run state at a glance.
Avoid when the surface is only showing one lease, one receipt, or one proof.

### 2. Contributor node

A pylon/person/machine entity with a compact role and state:

- `registered`
- `admitted`
- `assigned`
- `submitted`
- `verified`
- `rejected`
- `settled`
- `blocked`

The current pylon network has online/working/offline tones. Tassadar needs these
training-specific states, with proof refs driving the state rather than heartbeat
presence alone.

### 3. Trace strand

A line from contributor to workload/window representing one submitted executor
trace. It should be thin, digest-like, and directional. A trace strand is not
accepted work by itself.

Suggested states:

- `claimed`: faint dashed strand.
- `submitted`: solid faint strand.
- `queued_for_replay`: strand with a waiting marker.
- `replayed`: strand paired with a replay beam.
- `rejected`: strand terminates in an explicit mismatch notch, not a red alarm
  flood.

### 4. Replay pair

A worker-to-validator relation. This should be visually distinct from the pylon
network's "work flowing inward" curves. The current `/run` beam is the right
base: it names the verified pair and can open the challenge proof.

Suggested composition:

- Worker node.
- Validator node.
- A bidirectional or folded beam carrying the challenge ref.
- A small digest-match marker when verified.
- A compact mismatch marker when rejected.

### 5. Verification gate

A typed proof checkpoint that turns replay evidence into accepted work. The
primitive should render exact replay as a concrete state machine:

`trace submitted -> replay challenge -> digest match/mismatch -> verdict`.

This belongs in both a spatial scene and a data-display companion panel. The
panel should use `keyValueRows`, `progressList`, `badge`, and proof links from
the component library.

### 6. Receipt burst

The existing settlement burst in `/run` is the seed. Keep it small and event-like:
a payout or receipt is a pulse at the node, not a permanent halo that implies
unbounded future earning.

Differentiate:

- `payment_attempted`
- `settlement_recorded`
- `recipient_confirmed`
- `failed`
- `expired`

Spark/MDK unification work makes this especially important. A sender-side settled
row and recipient-confirmed balance are different visual states.

### 7. Corpus accretion

A quiet accumulating layer for accepted traces. It should not look like model
capability. Think "verified trace archive growing" rather than "neural network
awakening."

Potential forms:

- Fine tick marks or strata added to a corpus ring.
- Small digest tiles accumulating beside the run field.
- A compact stat grid with accepted trace count, verdict refs, and source refs.

### 8. Quarantine window

For the future public-gradient lane, visually separate quarantined model updates
from canonical model updates. The launch docs are explicit that public gradients
must pass quarantine, verification, canary evaluation, and promotion gates before
canonical optimizer entry.

This primitive should be visibly off-mainline: adjacent lane, dim border, no
canonical color until promoted.

### 9. Energy/outcome meter

AO/kWh should be a functional counter, not decorative analytics. Keep it in the
same register language as other evidence counters: provenance label, modeled vs
measured state, and repeat-datapoint status.

### 10. Proof drawer

Clicking any scene entity should open a compact proof drawer, not just a new tab.
The current `/run` element already opens proof links directly. A drawer would let
the visual stay spatial while the evidence remains inspectable:

- Public ref.
- Kind: run/window/trace/challenge/verdict/receipt.
- State.
- Caveats/blockers.
- Link to the public endpoint.

Use existing `drawerPanel`, `keyValueRows`, `badge`, `codeBlock`, and `textLink`
families.

## Composition rules

1. Keep the scene full-bleed. Do not put the main run scene inside a decorative
   card. For `/tassadar`, remove global nav/header chrome so the 3D canvas owns
   the full viewport.
2. Put supporting panels over the canvas as translucent glass: black with reduced
   opacity, thin borders, and blur. They are overlays on the run field, not
   layout boxes that frame or shrink it.
3. Use black, off-white, thin borders, mono labels, and small blue/green/warning
   accents. Do not introduce a separate Tassadar palette.
4. Let roles drive geometry:
   contributor nodes sit around the run, worker-validator pairs form replay
   beams, corpus sits as accumulation, settlement is a burst, blockers are gates.
5. Use compact gate glyphs for aggregate stage concepts. Do not use pylon-like
   bullseye/orb glyphs for `registered / 6 pylons seen` style stage counters;
   those are counters attached to a run phase, not the pylon records themselves.
   On `/tassadar`, keep those aggregate concepts out of the main 3D field unless
   they move into a clearly labeled legend or secondary support panel.
6. Use motion only when there is a bound public ref or measurable live state
   transition:
   flowing trace, replay pulse, receipt burst, slot-text counter roll, and
   heartbeat/liveness motion all need source refs or timestamped projections.
   Static proof should stay static.
7. Never use one glow for multiple truths. Online, assigned, verified, settled,
   and recipient-confirmed need separate encodings.
8. The first read should be visual; the second read should be inspectable refs.
   Every important visual element needs a data-display peer.
9. Zero states are first-class. A run with no verified traces should still look
   like a real run, not an empty marketing failure.
10. Anonymous edge pulses are banned for live training pages. If an edge does not
    have a motion event, render it as static structure.
11. Do not show a loss curve in the main view until there is product-ready,
    public loss data and a clear reason to make that chart part of the first
    read.
12. Do not put the product-promise gate or fleet-wide pylon stats as bottom text
    in the main view. Those are support surfaces and registry/proof links, not
    the first visual grammar.
13. Give dense real-ref sets explicit lanes by role instead of relying on a
    generic ring. Pylons, verified replay refs, rejected replay refs, accepted
    traces, and receipts should not collapse into one overlapping label cluster.

## Candidate scene grammar

For a future populated run after the proof/legend treatment exists:

- Center: `Run field`, label + run ref.
- Inner orbit: active training window and verification gate.
- Middle orbit: worker submissions and validator devices.
- Outer orbit: contributor pylons.
- Beams: verified replay pairs only, and only when the page can explain the
  worker ref, validator ref, challenge ref, and verdict/source refs.
- Faint strands: claimed/submitted traces that are not verified yet.
- Bursts: settlement events, with recipient-confirmed state separate.
- Corpus ring: accepted replay-verified traces only.
- Side drawer: selected entity proof.

For an idle or blocked run:

- Center stays present with honest status.
- Empty or blocked gates are visible.
- Counters roll to zero or blocked, not hidden.
- CTA points to install/go-online only when that action is actually available.

For the current `/tassadar` page, the live main field is intentionally smaller:
center run node plus selectable public-ref entities. It does not show the stage
counter board, loss curve, promise gate, fleet stats, contributor orbit, replay
beam traffic, or payout bursts.

## Implementation path

1. Keep `/tassadar` as the canonical live-run surface and `/run` as the current
   alias while extending `tassadarRunSnapshot.ts` instead of creating a parallel
   adapter.
2. Keep anonymous base edge pulses and verified-replay traffic disabled for
   `/tassadar` before adding more motion. Abstract back-and-forth dots are not
   acceptable live-run language unless each pulse can answer which public ref
   caused it and the page explains that answer.
3. Move the named scene concepts into `three-effect` primitives where they are
   reusable: contributor entity, trace strand, replay beam, receipt burst, corpus
   ring, proof drawer events.
4. Use `/animations` to prototype individual primitives, but promote only
   primitives that bind to public-safe refs and produce no anonymous motion.
5. Add a `/components` family or subfamily for "run evidence" only if the
   existing data-display/workroom families become too repetitive.
6. In Autopilot Desktop, keep the default Network pane broad and use Training
   Live for the run-specific grammar. The Network pane answers "is the fleet
   alive?"; Training Live should answer "what happened to this run?"

## Proof Replay Renderer Correction

The first proof replay route shipped a temporary custom element bridge in
`apps/openagents.com/apps/web/src/scene/tassadarProofReplayElement.ts`. That
bridge is useful for validating bundle loading, clocks, public-safe source
inspection, and share-route behavior, but its DOM/CSS/canvas stage is not the
target visual architecture.

Going forward, proof replay visuals must use this taxonomy through
`@openagentsinc/three-effect`. The app-local bridge may keep controls,
captions, event lists, source inspectors, loading/error states, and accessibility
mirrors. Stages, actor avatars, payment zaps, evidence bursts, camera language,
particles, labels-in-scene, terrain, and hit targets belong in `three-effect`
first, then get consumed by both website and Autopilot Desktop. This keeps
replay share cuts and desktop replay views from forking into separate visual
systems.

## Open questions

- Should `Tassadar` remain the public run name across all chrome, or should some
  operator views use the longer "Tassadar executor run" label consistently?
- Should rejected replay pairs be spatially visible on `/run`, or kept in the
  proof drawer to avoid rewarding failed work visually?
- When recipient-confirmed payout data exists, should it become a distinct
  burst layer or a receipt-drawer state only?
- Do we need a compact mobile version of the run field, or should mobile default
  to proof lists plus a small scene thumbnail?
- Which `three-effect` primitives already cover the next increment, and which
  must move from app-local experiments into the shared package first?

## Initial stance

The pylon visual language is already strong for network presence. Tassadar needs
the next layer: exact-execution proof language. The primitives should make
worker/validator replay, digest verdicts, receipt-backed settlement, and corpus
growth legible without widening product claims. The main mistake to avoid is
making the run look more successful than the public refs prove.

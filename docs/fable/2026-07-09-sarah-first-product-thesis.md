# Sarah-first: the case for one front door

Date: 2026-07-09
Status: analysis / product thesis (owner-directed)
Inputs: MASTER_ROADMAP.md rev 6.16, docs/fable/2026-07-09-sarah-khala-connection-assessment.md,
docs/fable/2026-07-09-sarah-monorepo-effect-native-consolidation-plan.md,
docs/sarah/2026-07-09-blueprint-map-surface-audit.md (#8626),
docs/sarah/2026-07-09-oav-quality-strategy.md (#8610), the KHS lanes (#8599),
and the live production state of `openagents.com/sarah`.

## The thesis

The owner's framing: **Sarah is the core product.** Not a sales widget on
the site, not P1 of seven phases — the product. Fleet work, coding work,
standing employees, the company brain: all of it flows through Sarah, who
uses the Khala model and Khala fleets in the background. The mobile apps
are how people carry Sarah in their pocket.

Today's MASTER_ROADMAP sequences Sarah as P1 in a chain (Khala Code MVP →
Sarah → Codex → employees → brain → suite), where each phase has its own
surface: Khala Code for coding, workrooms for supervision, dashboards for
operators, Sarah for sales. The Sarah-first thesis inverts that: the
phases stop being surfaces and become **capabilities behind one
conversational surface**. There is one relationship — you and Sarah — and
it deepens from "she sold me" to "she runs work for me."

## Why this is the natural reading of what we already built

The estate is already shaped like this; we just haven't said it out loud:

1. **Sarah already fronts the whole stack.** She runs on the Khala model
   lane (KHS-1, staging-armed; prod pending the persona-neutral model id,
   #8600), reads and writes the CRM, links accounts in-chat (KHS-7),
   builds customer Blueprints (KHS-9), carries her own Blueprint (KHS-5),
   learns per-prospect with owner-gated generalization (KHS-2/3/4), and
   now speaks through the owned avatar pipeline on our own GPU (#8610–
   #8614). That is not a sales widget; that is a general employee chassis
   wearing a sales role.
2. **The coding rail already exists and is conversational at both ends.**
   Khala → Pylon → Codex delegation is a typed, receipt-backed path from a
   sentence ("implement issue #NNNN and run the verification") to fleet
   execution with exact token accounting. The missing piece is not
   plumbing; it is that the sentence is typed into a CLI today instead of
   said to Sarah.
3. **The Blueprint Map (#8626) is the shared canvas.** The BM epic gives
   Sarah's surface a live graph of what she knows and is learning. That
   same canvas is where fleet runs, code diffs, receipts, and approvals
   render — the audit already specced graph/buttons/code/chat panes. The
   operator dashboard and the sales page converge into one screen: Sarah
   on the left, the work on the right.
4. **Mobile policy already points here.** The Expo app decision
   (2026-07-04) plus voice/STT modules is exactly "Sarah in your pocket"
   — a face, a voice, a transcript, and the canvas. The P0 Khala Code
   mobile MVP remains the shipping vehicle; Sarah-first says its center
   of gravity is the conversation, with coding as the first capability it
   exposes.
5. **The brain phases (P4/CB-1) literally say "generalizes Sarah."** The
   roadmap already admits Sarah is the prototype for every standing
   employee. Sarah-first just collapses the wait: instead of building
   employee #2 beside her, every capability lands inside her first.

## What "everything flows through Sarah" means concretely

| Today's surface | Sarah-first form |
| --- | --- |
| Khala Code desktop/mobile | "Sarah, fix the flaky auth test" → she dispatches to the user's connected Codex fleet (the existing #8591/#8612-adjacent rail), narrates progress, renders the diff + verification receipt in the canvas, asks for approval where policy requires |
| Pylon fleet ops | Fleet status, capacity, and account health are canvas panes she references; "add another Codex account" is a conversation with `khala fleet connect` as the executor |
| Standing employees (P3/AE) | Roles Sarah takes on (or spawns as named colleagues later): the chassis is her memory + Blueprint + receipts + approval gates, already built as KHS |
| Company brain (P4/CB) | The Blueprint Map, matured: her live model of your business, editable in conversation, provenance on every node |
| openagents.com marketing site | Sarah IS the demo and the funnel: the landing page's job is to start the conversation |
| Mobile apps | Sarah's face/voice + the canvas, nothing else at the top level |

The Khala model and fleets stay the engine room: model routing, gateway
attribution, Pylon capacity, GPU render nodes, settlement. None of that
surfaces as product vocabulary — a user meets Sarah, not "the gateway."

## What this changes in the roadmap (and what it does not)

**Does not change:** P0 ships as-is (the store submission gate is
orthogonal); the EN conversion program (§EN) — Sarah's surface is already
EN and the canvas doubles down on it; the trust/contract discipline
(behavior contracts, receipts, eval suites) — Sarah-first RAISES its
weight because one surface concentrates the blast radius; the Khala
supply-side economics.

**Changes:**

1. **P2 (Your Codex) re-targets its front end.** CX lanes keep the
   workflow cutover, but the user-facing entry becomes Sarah's
   conversation + canvas, not a separate app shell. The CLI/desktop
   remain power tools, not the front door.
2. **P3/P4 collapse toward "deepen Sarah."** Standing employees begin as
   Sarah wearing more roles behind approval gates, not as parallel
   product builds. CB-1 (Blueprint-lite) becomes the Blueprint Map's
   maturation (#8626 → brain).
3. **Mobile narrative flips.** The app is "talk to Sarah about your
   business; watch the work happen" — coding supervision is the first
   canvas tab, not the app's identity.
4. **Sequencing pressure moves to the conversation loop's quality**: the
   OAV realtime lane (#8610/#8621), turn latency, ASR quality, and the
   persona-neutral Khala lane (#8600) become core-product P0s, not
   side-quests. Today's live failures (freeze, eviction, silence) were
   core-product outages under this thesis — which is exactly how the
   owner treated them.

## Honest risks and their mitigations

- **One surface, one point of failure.** Today's render-freeze P0 proves
  it: when Sarah's face breaks, the product is down. Mitigation: the
  session simulator + deploy-gate smokes (#8621) as core CI, graceful
  text-only degradation (already the fail-soft), and the LiveAvatar seam
  as a warm fallback flag.
- **Persona coupling.** The #8600 "We are Khala" bleed shows role
  conditioning leaking across lanes. A persona-neutral internal model id
  is a prerequisite for Sarah fronting everything.
- **Sales context vs operator context.** A prospect and a paying operator
  need different tools, guardrails, and tone. The account link (KHS-7)
  is the switch: capabilities unlock by authenticated relationship, and
  the pricing/deal-rules contracts stay binding in every mode.
- **Latency + GPU economics.** One L4 serves one live face today.
  Sarah-first makes render capacity a scaling axis (SQ-4 capacity item);
  pre-rendered clips (openers, canned answers via KHS-6) and text-first
  fallback bound the cost while the fleet grows.
- **The suite story.** P7's multi-product suite doesn't die; it becomes
  "Sarah's roles" first and separate named employees only when a role's
  surface genuinely diverges.

## Recommended next moves (smallest honest steps)

1. Land BM-1..5 (#8627–#8631): the canvas is the physical form of the
   thesis.
2. Wire the FIRST coding delegation through Sarah's conversation: an
   authenticated owner says "run issue N on my fleet," Sarah calls the
   existing typed Khala→Pylon→Codex path, streams progress into the
   canvas, renders the closeout receipt. One vertical slice, dogfood-only
   (operator exemption), behind the account link.
3. Fix #8600 with the persona-neutral model id so Sarah's brain rides the
   Khala lane in prod.
4. Re-cut MASTER_ROADMAP's P2/P3 framing at the next rev to name Sarah as
   the front door (this doc is the input; the roadmap edit is deliberate,
   not drive-by).
5. Mobile: when the Expo app's chat-sync milestone lands, its home screen
   is Sarah, not a menu.

The one-sentence version: **we are not building seven products with Sarah
as the greeter — we are building Sarah, and everything else is what she
can do.**

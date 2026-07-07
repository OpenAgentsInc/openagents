# The Product Suite — Khala Code (Mobile + Desktop), openagents.com, Reactor

Date: 2026-07-07
Status: owner-directed product-suite articulation. Companion to (and
extension of) the overarching roadmap
(`docs/fable/2026-07-07-overarching-roadmap-khala-code-agent-computers-ai-employees.md`)
— that doc owns the horizon sequencing (H0–H6); this doc owns **what the
customer-facing products are, how they relate, and how one account and one
balance flow through all of them**. It also corrects an omission the owner
flagged: **Reactor** (`docs/fable/2026-07-04-reactor-open-model-private-deployment-plan.md`,
RX-* lanes) is a first-class product in the suite and was missing from the
four-prong funnel framing. Nothing here is promise copy; naming decisions
flagged below (notably "minerals") go through `docs/promises/` and owner
sign-off before any public surface uses them.

## 0. The owner direction (2026-07-07, recorded in essence)

The suite is:

- **Khala Code — mobile.** The great entry point. No desktop pairing
  required, but *can* pair to a desktop for more power. Discoverable from
  the App Store; productive immediately; buys credits (possibly branded
  **"minerals"**) via IAP; upsold to the broader suite (web + desktop).
- **Khala Code — desktop.** Increasingly intrudes on the "agentic IDE"
  direction Cursor/Anysphere originally tried to pioneer — but from the
  opposite direction: they started from a VS Code fork and have largely
  ditched the IDE for an agent-chat UI; we start from a good desktop coding
  agent (UX patterns adapted from Codex/OpenCode) and pull selected editor
  affordances in from VS Code.
- **openagents.com — the business dashboard.** Tracking spend,
  seeing/interacting with agents. Less ethereal/sci-fi than Khala Code,
  still thematically blue. A business user can simply give us money and
  have everything tracked through the website — and interact
  programmatically with any of these properties through Khala Code, all
  linked to the same central systems.
- **Reactor** — the private open-model deployment product — belongs in the
  offering set, and **we run our own Reactor internally**.

## 1. The suite in one view

| Surface | Who it's for | Job | Money in | Design register |
|---|---|---|---|---|
| **Khala Code mobile** | Anyone with a GitHub account and a phone | Dispatch and supervise agents from anywhere; the front door | IAP credit packs ("minerals", pending naming gate) + $10 signup grant | Full Khala: ethereal, StarCraft-blue, game-adjacent (Arcade/Ignite fidelity) |
| **Khala Code desktop** | Developers and power operators | The agent-first coding environment: deep work, fleet control, local capability, editor/verification surface | Same balance; BYO harness accounts | Same Khala register, denser: operator console |
| **openagents.com** | Business owners, managers, finance | See everything, pay for everything, manage the roster: spend, receipts, employees, approvals, brain, team | Stripe card + crypto/Lightning (live), invoices/FDE for sales-led | Legible business software; quieter, still Protoss blue |
| **Reactor** | Sovereignty-demanding businesses (and us) | Private open-weight inference + the fulfillment stack inside the customer's trust boundary | Setup + managed retainer (sales-led; not credits) | Infrastructure product: receipts and policy objects, not vibes |

Not products, but the shared spine underneath all four: **one account**
(GitHub via OpenAuth), **one balance** (the Pool B credits ledger — every
purchase rail fulfills the same ledger), **one data plane** (Khala Sync —
the same threads/agents/receipts on every surface), **one execution
substrate** (Agent Computers), **one claims system** (the promise
registry), and **Aiur** as the internal owner/ops console (never
customer-facing). The suite is four faces on one body; the moment a surface
grows its own parallel ledger, sync, or claims path, the thesis is failing.

## 2. One spine, many faces — the linkage contract

The owner's sentence is the requirement: *a business user could simply give
us money and have stuff tracked through the website, and also interact
programmatically with any of those properties through Khala Code, all
linked to the same central systems.* Concretely:

1. **Identity:** one OpenAuth account spans all surfaces. Mobile signs in
   with GitHub PKCE (#8468–#8470, shipped); web uses the same issuer;
   desktop links through the existing device flows. An employee hired on
   the web appears in the phone's Agents panel and the desktop's fleet
   view because it is the same `agent_definition.v1` row in the same sync
   scopes.
2. **Balance:** one credits balance, many fill rails — IAP on mobile
   (server rail landed #8482, dormant pending RevenueCat reopen), Stripe +
   crypto + Lightning on web (live on prod), setup-fee + credit-grant
   splits for sales-led deals (proven on live checkout links). Every rail
   fulfills the same ledger; every surface renders the same itemized draw
   (tokens, agent-computer time, later connector margin).
3. **Programmatic access is a product property, not an afterthought.** The
   surfaces are all clients of the same typed Worker APIs (OpenAPI +
   capability manifest already public). "Interact programmatically through
   Khala Code" means: anything visible on the web dashboard is reachable
   as a typed tool from a Khala Code session — spend queries, agent
   dispatch, receipt lookup, promise state — so a user can *manage the
   suite with the suite*. This is also the dogfood loop: our own agents
   administer our own business through the same surface we sell.
4. **Sync, not screen-scraping, between surfaces.** The phone, desktop,
   and web render the same Khala Sync scopes; cross-device continuity
   (start a thread on the phone, pick it up on desktop) is the reopened
   MC-5 lane and arrives as a property of the data plane, not a feature
   built per-surface.

## 3. Khala Code mobile — the front door

The mobile-only MVP (epic #8467) stands exactly as scoped; this section
adds the *suite-relative* framing:

- **Discovery-to-productive in one sitting.** App Store → GitHub sign-in →
  $10 grant → pick a repo → first agent turn → push notification when it's
  done. No desktop, no pairing, no setup. This is the acquisition surface
  for the entire suite, and its conversion funnel is the suite's front
  door metric.
- **"Minerals."** Branding credits as minerals is on-theme (the StarCraft
  naming system already carries Khala/Pylon/Artanis/Hydralisk/Reactor/Aiur)
  and gives the IAP packs a personality generic "credits" lacks. Treatment:
  **minerals is the consumer-facing brand of the same ledger unit; the
  ledger, receipts, and business surfaces keep the neutral term credits**
  (a CFO reads "credits" on an invoice; a player buys minerals). Decision
  gates before any public use: owner sign-off, promises/copy pass, App
  Store consumable naming/compliance check (the 3.1.1 checklist from
  #8483 already covers restore/refund mechanics), and one glossary entry
  so mobile-brand and ledger-term never blur in accounting or legal copy.
- **Pairing is an upgrade, never a requirement.** The retired desktop
  couplings stay retired for the default path; "pair to desktop for more
  power" returns as an explicitly optional lane (the reopen ledger's MC-5
  cross-device work): a paired desktop adds local execution, local repos,
  BYO local accounts, and the fleet — the mobile app gains capability but
  never depends on it. The invariant: every mobile feature works with zero
  desktops forever.
- **The upsell ladder lives in the app but points at the suite:** buy
  minerals (IAP) → connect your Codex (H1, still on the phone) → hire your
  first employee (H2/H3) → "see your business view" (deep link to the web
  dashboard when spend/roster crosses the threshold where a phone screen
  stops being the right instrument) → pair a desktop (power) → talk to
  sales (Reactor/sovereignty, when the account's usage pattern or intake
  answers signal custody sensitivity).

## 4. Khala Code desktop — the inverted agentic IDE

The strategic read on the category: Cursor pioneered "agentic IDE" from a
VS Code fork, and the market has watched them progressively abandon the
IDE surface for an agent-chat shell — evidence that **the fork-the-IDE
vector was wrong, not that the destination was**. The destination (a
desktop environment where agents do the work and the human directs and
verifies) is right, and we are approaching it from the opposite, correct
vector:

- **Start from the agent, not the editor.** Khala Code desktop is already
  a good desktop coding agent: the Codex app-server wrapper as the local
  kernel (`docs/khala-code/2026-07-01-codex-harness-wrapper-port-audit.md`),
  OpenCode/Codex UX parity work (`2026-07-05-opencode-desktop-parity-gap-audit.md`),
  the fleet/inbox/forum/gym panels, multi-harness orchestration, and the
  `khala_fleet` MCP delegation bridge. The product identity is *operator
  console for agents*, and it stays that.
- **Pull the editor in as an instrument, not an identity.** The VS Code
  adoption audit (`2026-07-05-vscode-explorer-editor-adoption-audit.md`)
  already sets the discipline: Monaco as the renderer, Explorer
  *architecture* adapted (never the workbench wholesale), a Khala-owned
  workspace file service so the editor never depends on any one agent
  runtime, read-only source browsing first, editing later. In suite terms
  the editor is **the manager's magnifying glass**: inspect what the agent
  wrote, review diffs, browse the workspace, verify before approving. That
  is why we win the vector inversion — for Cursor the editor was the
  legacy surface to escape; for us it is a supervision instrument added
  exactly where agent work needs human verification.
- **Where it intrudes on the IDE category over time:** diff-first review
  UX, artifact inspection, multi-workspace fleet views, then editing with
  agent-aware affordances (an edit is feedback to the agent, not just a
  keystroke). We take editor market share as a *side effect* of being the
  best place to supervise agents — we do not enter a feature war on
  IDE-native ground (debugger, extension marketplace, language servers
  beyond what Monaco gives us cheaply). The moment desktop work is
  justified by "IDE parity" rather than "agent supervision," it's off
  thesis.
- **Desktop is also the power tier:** local Pylon, isolated BYO harness
  accounts (the fleet machinery), local repos and capability (voice, file
  ingestion — the desktop-layer wins flagged in the partner audits), and
  the pairing target for mobile (§3). Shell direction stays React +
  Tailwind under ONE-UI.

## 5. openagents.com — the business dashboard

The web surface's job flips from "the product" to **the counting house and
the front funnel**: where money enters legibly and where the business view
of everything lives.

- **Design register: legible first, blue always.** Khala Code (mobile +
  desktop) keeps the full ethereal sci-fi identity; openagents.com renders
  the *same data* in the register a business owner, bookkeeper, or buying
  committee expects — tables, invoices, exportable receipts, plain
  sentences — still unmistakably Protoss blue (one theme, no light/dark
  split), but with the HUD turned down. Two registers, one design-token
  system: the shared ONE-UI/shadcn components themed once, with the
  game-adjacent layers (Ignite/Arcade effects) simply not mounted on the
  business surfaces. This needs a short design spec naming which tokens
  and effects are Khala-register-only, so "less ethereal" is a defined
  property rather than per-page taste.
- **What a business user does there:** give us money (card, crypto,
  Lightning — live; volume-tier prepays; sales-led splits), see itemized
  spend per rail and per agent (tokens / agent-computer time / later
  connector margin), see and steer the roster (the Agents panel's web
  twin: run history, approvals queue, authority states, budgets), manage
  the company brain (sources, slices, provenance), team/seat
  administration, and read every receipt and promise state that backs a
  claim we made to them. The approvals inbox mirrors mobile — push goes to
  the phone, the queue also lives on the web for the person whose job is
  the queue.
- **It is also the funnel:** business intake, prefilled workspaces,
  vertical funnels (the LawPilot-style pages), the outcome-story gallery,
  and the assessment instrument — the blitz front door and the H6 campaign
  land here. The same page family serves self-serve signup and the
  sales-led motion because the underlying objects (workspace, brain,
  employee, receipt) are identical.
- **Programmatic parity (the §2.3 property, stated as a product promise
  candidate):** everything the dashboard shows rides public typed APIs, so
  the user's own Khala Code agents can query spend, list employees, pull
  receipts, and file approvals. The dashboard is one client of the
  platform, not the platform.

## 6. Reactor — the fourth product, and our own internal one

Reactor was absent from the four-prong funnel framing; this section fixes
its position in the suite.

**What it is** (from the product plan): a productized private deployment of
the OpenAgents inference + fulfillment stack — curated open-weight models
(Nemotron/Llama/GPT-OSS/Gemma/Mistral/Qwen/DeepSeek/GLM tiers) behind the
same OpenAI-compatible gateway shape Khala serves, inside the customer's
trust boundary, governed by a **typed model-provenance policy**
(`openagents.model_provenance.v1`, `reactor.model_policy.v1` — landed in
`packages/reactor-contracts`, RX-2) that turns "which models may run on our
hardware" from a sales conversation into an enforced configuration object
with receipts. Autopilot workrooms/approvals/receipts run against the
private endpoint; our fleet installs and operates under a managed retainer.

**Position in the suite:**

- **It is the far end of the sovereignty ladder** the overarching roadmap
  named (§5 there): org-cloud → BYO subscription → sovereign placement →
  **Reactor** (customer-owned metal, open weights, zero third-party model
  custody). One control plane, one isolation contract; where the metal and
  the weights live becomes a quoted product tier. The composition to name
  explicitly: **an Agent Computer whose inference endpoint is a Reactor
  node is a fully sovereign AI employee** — every horizon H2–H5 capability
  (standing employees, company brain, templates, trust layer) composes
  with Reactor custody without redesign, because the seams (placement
  lanes, model-preference targets, receipts) were built lane-agnostic.
- **Relative to the four prongs:** the prongs (rapid software, lead gen,
  AI employees, company brain) are *functions* a customer buys; Reactor is
  the **custody dimension** sellable under any of them — "the same
  employees and brain, on your hardware, under your model policy" — and
  also its own SKU for buyers who start from the sovereignty end (the
  own-your-ai campaign's assessment → pilot → retainer ladder). Funnel
  copy should present it as the fifth named offering: prongs one through
  four are what the agents do; Reactor is **where the intelligence
  lives** — "own the intelligence layer."
- **Pricing stays sales-led** (setup + retainer, per-deal), *not*
  credits — but Reactor nodes emit the same exact local-metering receipt
  shapes (RX-6 proved the shape), so a customer's web dashboard (§5)
  itemizes their private usage with the same legibility as hosted usage.
  One counting house across custody classes.

**Reactor Zero — our own internal Reactor.** The owner directive "we want
our own Reactor internally" is already further along than a plan:

- The **hydralisk lane** — org-owned GPU serving an open-weight model
  (GLM-5.2 REAP) as the default backing for `openagents/khala` — is de
  facto internal open-model serving in production today.
- **RX-6 recorded the first formal internal dogfood receipt**
  (`openagents.reactor.dogfood_run_receipt.v1`): a dogfood node profile on
  hydralisk, GPT-OSS under a strict US-only provenance policy, exact local
  metering, and a deliberately **refused** nonconforming Qwen pull
  (`reactor.policy.origin_not_allowed`) — the policy gate proven
  structurally, on us first.

Formalize this as **Reactor Zero**: our own production inference
progressively self-supplied on our own hardware under our own versioned
model policy, with the full RX receipt chain (install RX-5, evals RX-4,
need-to-know access RX-9, data liberation RX-10, improvement ladder
RX-11) running continuously against it. Why it matters strategically:

1. **Every Reactor sales claim becomes a receipt from our own
   production**, not a demo — the case-study seed RX-6 names is exactly
   this, and "we run on it ourselves" is the one claim the FDE competitors
   selling other people's clouds cannot make.
2. **COGS and custody for the hosted product**: the more of Khala's hosted
   inference rides Reactor Zero, the less we pay frontier-lab margins and
   the cleaner our own data-custody story gets (our traces feed *our*
   improvement ladder, nobody else's — RX-10/RX-11).
3. **The improvement flywheel**: internal workloads (the fleet, lead-gen
   dogfood, QA swarms) generate the eval and fine-tune evidence that makes
   the customer-facing catalog's capability claims receipt-backed
   (RX-4's `not_measured` discipline filled in by our own usage first).

Boundary discipline carries over from RX-6 verbatim: internal dogfood
receipts clear internal blockers only — no external-pilot, compliance,
availability, or pricing claim flips without their own gates.

> **Update (later 2026-07-07):** Palantir's *Institutional Sovereignty in
> the Age of AI* report independently validates this section's
> architecture (their Fig 8 on-prem stack is the Reactor diagram with an
> Ontology layer) and supplies the category language. Analysis and
> roadmap deltas — including "assurance levels" as the ladder's selling
> vocabulary and Reactor Zero as the Tier-1 proof asset — in
> `docs/fable/2026-07-07-palantir-institutional-sovereignty-smb-analysis.md`.
> Suite arming (IAP/credits brand, pairing reopen, assurance-level tiers) is
> sequenced as Phase P6 of `docs/fable/MASTER_ROADMAP.md`.

## 7. Cross-surface journeys (the suite working as one)

- **The indie developer:** App Store → mobile coding on the $10 grant →
  buys minerals → connects their Codex (H1) → pairs a desktop for local
  power and the fleet → their forum/outcome identity accrues → the network
  graduation path.
- **The business owner (the mastermind persona):** assessment or intake on
  openagents.com → prefilled workspace with a starter employee in
  `observe` → approves drafts from the phone → funds via the web with a
  volume-tier prepay → hires two more employees off the template catalog →
  the dashboard becomes their weekly business review.
- **The sovereignty buyer:** own-your-ai outbound or a vertical funnel →
  readiness assessment → Reactor pilot on their hardware → Autopilot
  fulfillment + employees running against their private endpoint → their
  dashboard itemizes private usage; their board hears "the weights, the
  data, and the hardware are ours."
- **Us (customer #1):** the fleet builds the templates on Agent Computers;
  Reactor Zero serves the inference; the lead-gen employee sells the
  suite; our own dashboard is the first business the dashboard manages;
  and every one of those loops emits the receipts the marketing quotes.

## 8. What this updates in the prior analysis

- **The overarching roadmap** gains this doc as the product layer over its
  horizons: H0–H6 sequencing is unchanged; §5's sovereignty ladder now
  explicitly terminates at Reactor-as-product, and the four-prong funnel
  is amended to "four prongs + the custody dimension (Reactor)." A pointer
  note is added there in the same commit.
- **The post-MVP doc's** CX/AE/BI lanes are unaffected; §3.3's cockpit
  gains the web twin named here (§5), and the naming-glossary open
  question now includes *minerals* and *Reactor Zero*.
- **New lanes implied by this doc** (file under the horizon scheme):
  the minerals naming/copy gate (H0-adjacent, owner-gated); the
  two-register design spec (§5, before web dashboard work scales); the
  web Agents panel twin (H3); IAP arming (the dormant #8481/#8482 reopen,
  owner-timed); MC-5 pairing reopen as the mobile→desktop power lane
  (post-H2); Reactor Zero formalization (RZ-* or an RX-6 follow-on — our
  own inference share served by Reactor Zero as a tracked, receipted
  metric).

## 9. Open questions

1. **Minerals economics across rails.** Apple's IAP cut vs web pricing:
   same mineral pack priced identically everywhere (eat the margin on
   mobile) or web-cheaper with compliant steering? Interacts with volume
   bonus tiers (web prepay bonuses likely can't be mirrored 1:1 in IAP
   pack sizing). Owner pricing decision; the 3.1.1 compliance checklist
   constrains the copy either way.
2. **Vespene.** If minerals ship, the second resource is sitting right
   there — a possible future name for the *compute-time* meter (agent
   computers burn gas; models cost minerals). Cute, dangerous if it makes
   billing less legible; park it for the glossary pass, decide against it
   by default on the "one balance" principle unless the two-meter display
   genuinely benefits.
3. **How much editor is too much editor.** The §4 tripwire ("agent
   supervision, not IDE parity") needs an owner-reviewable test — e.g.
   every editor feature must name the supervision loop it serves in its
   issue, or it doesn't get filed.
4. **When does the web dashboard split from the funnel pages** (one app
   with two registers vs two route families)? TanStack Start migration
   sequencing decides this; keep one repo home either way.
5. **Reactor Zero's serving share as a public number.** "X% of Khala
   tokens served from our own metal" is a powerful receipt-backed claim
   and a sensitive operational disclosure. Decide with the promises pass
   whether it's public, investor-only, or internal.

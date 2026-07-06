<!-- Integration ideas: Agentic Society 2026-07-03 mastermind themes → OpenAgents -->
<!-- Companion to ./2026-07-03.md (transcript) and ./2026-07-03-analysis.md (analysis) -->
<!-- Background: docs/fable/ (product strategy, Apollo outbound, business fulfillment engine)
     and the private blitz outreach docs in the root workspace -->

# Integration — Bottling the Agentic Magic for Export

How the themes from the [2026-07-03 Agentic Society mastermind](./2026-07-03.md)
(see the [deep analysis](./2026-07-03-analysis.md)) integrate into OpenAgents —
not just with these specific people and orgs, but as a repeatable product that
lets *any* non-technical business owner catch up to that room's "level 2 and 3"
operators fast. Written against the current product strategy in `docs/fable/`
(two-product frame: **Khala Code** + **Autopilot**; business fulfillment
engine; Apollo outbound; come-for-the-tool-stay-for-the-network) and the
private blitz outreach program (outcome packages, prefilled workspaces, the
75/25 persona work).

---

## 1. What the room proves about our market

The mastermind is the strongest field evidence yet for theses we already hold:

1. **The demand is real and already monetized.** Non-technical operators are
   paying ~$30–130/month in agent infrastructure and getting $10K–$400K
   outcomes. Nobody in that room needed to be convinced agents work; they
   needed *better infrastructure, trust, and templates*. That is a buying
   audience, not an education audience.
2. **The "underserved middle" persona from the blitz docs walked in and sat
   down.** HVAC CEO, CPA, functional-health clinic, local SEO agency, real
   estate investor, boutique consultants — exactly the "75/25 operator, too
   small for enterprise AI, too large for nothing, no CTO" ICP. The room is
   a live census of the persona we generalized from our legal design-partner
   work.
3. **Half the room is level 3 already — and duct-taped.** Their standard
   stack: a harness (Hermes, ex-OpenClaw), an $8/month VPS, OpenRouter as a
   model brain, MCPs (notably Apollo), skills found by Googling, cron jobs,
   and named agent personas with their own email accounts. Every layer of
   that stack is a gap we can own better than they can assemble it.
4. **Their #1 unsolved problem is trust in the supply chain.** The room's
   settled security advice is *never install a third-party skill — have your
   AI regenerate it from a description and audit it yourself*. That is a
   market screaming for provenance, receipts, and verification — which is
   the exact spine of the Autopilot accepted-outcome model ("provenance is
   the product", receipts, verify-gates-settle).
5. **The other half of the room is level 1–2 and anxious about it.** ~17 of
   ~40 hands were below background autonomy. Those people watched the level-3
   demos, voted for them, and said in the shout-outs "just being in the room
   was incredibly inspiring." The catch-up product sells itself in that room;
   we just haven't put it in front of them.

The strategic read: **the mastermind circuit is doing our market education
for free.** Every city has rooms like this now. Our job is not to convince
anyone — it is to be the fastest, most trustworthy path from "I raised my
hand at level 1" to "I have named AI employees running 24/7 with receipts."

---

## 2. Theme-by-theme mapping to OpenAgents assets

| Mastermind theme | What the room does today | OpenAgents asset that bottles it |
|---|---|---|
| AI-as-employee (names, souls, roles, permission stacks) | Hand-built Hermes profiles, manual M365 identities ("Ali") | `agent_definition.v1` + prefilled workspaces: hireable, named, role-scoped agent employees with typed toolsets and draft-only default authority |
| Company brain / AI slingshot | Obsidian markdown vaults, Claude projects, one-builder bottleneck | The grounded **context library** in a prefilled Autopilot workspace — seeded *for* the customer from public data before they ever sign in |
| 24/7 background autonomy | $8 Hostinger VPS, laptop-off anxiety | Managed always-on agent hosting on our cloud + Pylon fleet — "your agent doesn't sleep when your MacBook does" as a first-class product promise |
| Cron + one human touchpoint architecture | Hand-wired 12:30 AM cron jobs | Fulfillment agents: per-promise scheduled loops with receipts, already the BF-engine execution shape |
| Skills economy + registry distrust | Googled skills, hand-audits, "don't trust anybody" | A **provenance-receipted skill registry**: every skill ships with an audit receipt, injection scan, pinned source, and a regeneration path (see §4.3) |
| Read-only-first trust ladder | Folk practice ("start it read-only, then write") | Behavior contracts + approval policy as product UX: autonomy levels a business owner *promotes* an agent through, with an audit trail |
| Outcome-shaped proof ("My agent did X and got outcome") | Post-it notes and hand votes | Product promises + accepted-outcome receipts — the same grammar, machine-verified instead of self-reported |
| Redaction-before-publish | A bespoke redaction skill in one company's pipeline | Khala chat-boundary redaction + the publish-gate pattern, productized for any customer pipeline |
| Multi-model debate / EBI loops | Wilson's token-burning three-model debates; CT's EBI passes | Khala Code multi-harness orchestration (Codex \| Claude \| Khala pill) — planner/coder/judge loops as a supported pattern, not a hack |
| "Do the most human thing" | Owners keep DMs, camera, relationships; agents get the rest | Autopilot's operator-minutes falsifier: our engine succeeds only if the human's time concentrates on human work |
| Recognition economy, playbook sharing | Trophies, shout-outs, event webpage | OpenAgents Forum + tipping + public build receipts — the online, settled-in-sats version of the mastermind's social loop |

The striking thing about this table: **almost nothing needs inventing.** The
room independently converged on the architecture we already committed to —
named scoped agents, grounded memory, receipts, staged trust, outcome units.
What they lack is packaging, hosting, provenance, and a path that doesn't
require six months of tinkering.

---

## 3. The core export: the "AI Employee Package"

The magic in that room, distilled, is a repeating six-tuple:

> **persona** (name, soul, tone) + **permission stack** (identity, tools,
> read/write grants) + **skills** (a few, role-rolled-up) + **data seed**
> (the company/context slice this role needs) + **schedule** (cron + human
> touchpoint) + **proof** (an outcome the owner can point at)

Oscar's Ali is one instance. Sagun's content engine is one. Jay's CFO agent,
Rick's reporting agent, the DM setter — all the same tuple with different
fills. **That tuple is the export format.** It is also, nearly line for line,
what a prefilled agent workspace + `agent_definition.v1` + an
accepted-outcome receipt already encode.

So the product is: **hire a named AI employee from OpenAgents, delivered
pre-onboarded into a workspace that already knows your business, with a
receipt for everything it was given and a trust ladder you promote it
through.** Concretely:

1. **A catalog of employee templates**, each one a productized version of a
   pattern the mastermind proved (names ours, roles theirs):
   - **Outreach Rep** — the Oscar pattern: ICP list + enrichment + low-volume
     personalized email + follow-ups + booking, draft-only until promoted.
     This is Autopilot Lead Gen v0 generalized from our own dogfood to the
     customer's ICP.
   - **Content Engine** — the Sagun pattern: source scrape → ranked topics →
     scripts sized to the owner's format → distribution handoff → weekly
     digest. Human stays on camera; agent owns research and assembly.
   - **Controller/CFO Agent** — the Jay/Rick/David pattern: connector-fed
     P&L, cash-flow, weekly report sends, daily reconciliation with a human
     one-click clear.
   - **Ops Triage** — the Lauren/Prashant pattern: notes, texts, and pipeline
     debris triaged into the system of record with proposed next steps.
   - **Knowledge Concierge** — the Nikki pattern: make a course/library/site
     searchable and agent-legible (pairs with the agent-readiness audit we
     already sell outbound).
   - **QA Swarm** — already productized; the mastermind's "red-team agent"
     folk practice done as a service with fingerprinted evidence.
2. **Each template ships as a prefilled workspace, not a signup.** The
   prospect's first experience is the acceptance moment: *their* business
   already in the workspace, one starter outcome ready to run, an intro
   receipt listing exactly which public data seeded it. This is the blitz
   playbook verbatim — the mastermind confirms it matches how these buyers
   actually adopt (every deep-dive began with someone else's working example
   applied to their data).
3. **Employees are promoted, never unleashed.** Ship the read-only-first
   ladder as explicit product states: **Observe → Draft → Act-with-approval →
   Act-within-policy**, each promotion logged, each state visible on the
   agent's "employee card." Wilson's agents.md hard rules (money and outbound
   email require approval) become typed policy defaults, not folklore. This
   converts the room's folk security doctrine into UI.
4. **Every employee has a ledger.** What it did, what it touched, what it
   cost, what it returned — the "My agent did X and got outcome" post-it,
   generated continuously and honestly. This is our receipts spine doing
   double duty as retention UX: the owner opens the app to *feel* the ROI the
   mastermind format celebrates.

Pricing shape (consistent with current policy): credits-based usage with no
monthly AI subscription and non-expiring credits for the agent itself —
undercutting the room's duct-tape stack on simplicity, not price — plus the
existing services ladder (Quick Win $1–5K → Sprint $5–15K → "On Autopilot"
retainer $2–10K/mo) for owners who want the employee *delivered* rather than
adopted. The mastermind demonstrates both buyer modes in one room: DIY
level-2s who want the tool, and owners who would clearly pay the retainer to
skip the 6–7 hours of planning-and-building Justin described.

---

## 4. What we must build or harden to make the export real

Gaps between the vision above and current state, in priority order:

### 4.1 Always-on hosting for named agents (the $8 VPS killer)
The single most repeated operational fact in the transcript: agents die when
the laptop closes, so everyone graduates to a VPS. Nobody enjoys this. We
have the cloud + Pylon substrate; what's missing is the **consumer-shaped
product**: "your employee runs on OpenAgents cloud, 24/7, with uptime and an
activity feed" — one toggle, credits-metered, no SSH. This is likely the
highest-conversion single feature for the level-2→3 transition, and it is a
prerequisite for every template in §3.

### 4.2 Non-technical onboarding that produces a real agent
Oscar's method — "just ask Claude everything, what's an MD file" — works but
takes weeks and confidence. The Khala/Autopilot onboarding must compress it:
a conversational intake ("what 25% of your work would you delete?") that
outputs a configured employee (§3's six-tuple) rather than a chat thread.
The blitz intake form already captures the seed inputs (business name, URL,
what they want help with, phone); the missing piece is the automated
prefill pipeline from intake → workspace → first receipt at campaign scale.

### 4.3 The provenance-receipted skill registry
The room's "stay away from public skill registries… don't trust anybody" is
a verdict on the whole current skills ecosystem — and an open goal for us.
A registry where every skill has: pinned content hash, source provenance,
an automated injection/security audit receipt, a human-legible capability
manifest (what it reads, what it writes, what it can spend), and a one-click
**regenerate-under-audit** path (the room's own practice, automated). Sell
the "head of security" agent as a built-in: every skill and connector a
customer adds gets reviewed by it, with a receipt. Nobody else in the
room's stack can credibly offer this; it composes directly with our
receipts/verification identity and is a moat the harness vendors have no
incentive to build.

### 4.4 Input-path security as a differentiator
The analysis notes the room defends the install path but not the input path
— agents reading hostile email replies, scraped webpages, and 15 years of
inbox. Our chat-boundary redaction, draft-only default authority, and typed
approval gates are the beginnings of an input-path story. Productize it as
part of the employee card: "this employee reads untrusted input; therefore
it cannot send, spend, or write without approval." Being the vendor that
*names* prompt injection to this market, in employee-metaphor language
("your employee can be socially engineered; here's their training"), builds
trust the competition hasn't earned.

### 4.5 The company brain as a product object, not a byproduct
Level 2's whole point is shared grounding with governance. Today our context
library exists inside workspaces; the room's practice (Obsidian vaults,
per-role data scopes, department-head agents) suggests the product needs:
explicit role-scoped views of one brain ("each agent a genius in its own
lane"), ownership handoff (the CMO owns marketing skills), and an onboarding
story where **the brain is the deliverable** — one attendee sells exactly
this ("Business Foundation Blueprint", claiming 85% onboarding reduction).
Khala Sync is the substrate advantage here: one brain, synced across
desktop, web, and mobile, with per-role access as policy.

### 4.6 Khala Code's role: the power tool and the factory
Two distinct integration lanes:

- **As the product for the room's builders.** The level-3 tinkerers (Wilson's
  multi-model debates, Justin's one-shot builds after EBI planning, the
  red/blue-team pairs) are Khala Code's exact ICP: people already paying for
  multiple model subscriptions who want orchestration, fleet capacity, an
  inbox over many agents, and proof of what ran. The harness pill
  (Codex | Claude | Khala) *is* Wilson's debate loop with receipts. The
  agencies in the room (SEO, marketing, consultants) are the white-label
  operator channel from the blitz docs — they resell to their client books.
- **As our internal factory for §3.** Every employee template, prefilled
  workspace, and outcome package is itself built and QA'd by our own fleet.
  The mastermind's "6–7 hours to one-shot it after 3 hours of planning" is
  the artisanal version; the factory version is a fleet lane per template
  with QA Swarm as the verifier. This is the anti-agency-trap mechanism:
  operator-minutes per engagement fall because the fleet, not the founder,
  stamps out the packages.

### 4.7 Autopilot Lead Gen, proven by the room
Oscar's agent is independent confirmation of our audit-first outbound thesis:
personalized, signal-based, low-volume outreach with enrichment (he even uses
the same Apollo) closing six-figure work. Two integrations: (a) his pattern
is the **customer-facing** Leadgen Engine template (§3), and (b) his
discipline — 10–20 sends/day, dedicated identity, subsidized-offer targeting
— should be encoded as *policy defaults* in the template so customers inherit
the taste, not just the plumbing. What we must not export: the spam-shaped
variants (mass job-application bots, fake-internal-email tricks at volume).
The compliance guardrails from the blitz docs (value before ask, consent
channels, human-written where it matters) are a feature to advertise, since
this market will drown in slop and the deliverability commons will collapse
around the undisciplined.

---

## 5. The campaign: exporting it at scale

Imagine the massive sales/outreach campaign. The mastermind supplies both the
map and the ammunition.

### 5.1 The mastermind circuit as a channel
These rooms exist in every metro now, they meet monthly, they are free, they
publish transcripts and event pages, and they *vote on what they want*. A
deliberate circuit motion:

- **Attend and share, never pitch.** The format rewards outcome stories; our
  build-in-public promise-keeper share already won attention there. Every
  event we attend should produce one post-it-shaped outcome story and one
  follow-up list.
- **Bring the catch-up gift.** For hosts and organizers: a prefilled event
  workspace (the "agentically-generated event page" is something they
  hand-assemble today — we can make that a template and give it away). For
  attendees who self-identify at level 1–2: a QR to a **level assessment →
  prefilled workspace** funnel (§5.2).
- **Sponsor the recognition, not the room.** Trophies, the "most helpful
  builder" framing — cheap, on-culture, and aligned with our forum/tipping
  identity. The recognition economy is the network's offline on-ramp.

### 5.2 The "Catch Up to Level 3" funnel
A campaign front door built on the three-levels vocabulary the market is
already teaching itself:

1. **Assessment instrument**: "What level is your business?" — a five-minute
   conversational audit (self-serve, or run live at events) that outputs an
   honest level, the top-3 gaps, and *which employee template would move
   them first*. This is the agent-readiness audit generalized from websites
   to operations, and it front-loads the same trick as the Apollo motion:
   lead with their own report, not our pitch.
2. **The prefilled workspace as the follow-up.** No demo calls to schedule;
   the report links to a workspace that already contains their business and
   one runnable starter outcome. Engagement (sign-in, runs, revisits) is the
   lead score, exactly as the blitz docs specify.
3. **The outcome ladder as conversion.** Free snipe → activation → retainer,
   with the first accepted outcome as the conversion moment and the ledger
   (§3.4) as the retention surface.

### 5.3 Segments, straight from the roster
The room hands us clone-segments for Apollo-style waves, each anchored by a
proven archetype and a working template:

| Segment | Archetype proven in the room | Lead template |
|---|---|---|
| Home services / trades with rebate or program-funded offers | HVAC + city energy program, $200–300K projects | Outreach Rep |
| Local media / creator-operators | 3M monthly views, 66-week newsletter streak | Content Engine |
| CPAs / bookkeepers / fractional CFOs | $10K in 2 weeks from AI-designed decks; CFO agents | Controller Agent |
| Health & wellness clinics | retention dashboards, call audits (redaction/local-model needs) | Knowledge Concierge + redaction |
| SEO / marketing / web agencies | 30→9 heads, 10x output, 12 agents | Khala Code fleet + white-label |
| Course/coaching businesses | searchable video library for an NBA coach | Knowledge Concierge |
| Real estate / property ops | land-qualification texting; listing microsites | Outreach Rep + Ops Triage |

Agencies remain the multiplier segment (one close = a client book), matching
both the Apollo plan's segment C weighting and the room's own composition.

### 5.4 Content: the outcome stories are the campaign
The post-it grammar is the ad unit. A public, receipted gallery of
"**My agent did X and got outcome**" stories — ours verified with receipts,
customers' verified with their permission — out-credibles every AI vendor
landing page, because it is the format this market already uses to persuade
itself. The Agentic Show, event pages, and LinkedIn-mined-mastermind posts
show the demand for this content; our differentiation is that ours carries
proof. This is also the bridge to the network thesis: outcome stories → forum
identity → tips → routed work — come for the story, stay for the economy.

### 5.5 What "massive" means without the agency trap
Scale comes from the factory (§4.6), not from headcount:

- **Wave mechanics** as in the Apollo plan (audit engine → segment waves →
  walkthroughs → quick wins), with the fleet building each prospect's
  prefill and each customer's quick win.
- **Tripwire preserved**: operator-minutes per engagement must fall
  monotonically as engagement count rises. If the campaign works and
  operator-minutes rise, we've exported labor, not magic — stop and
  productize the bottleneck (the Episode 246 heuristic: the system we build
  to cope *is* the next product).
- **Sequencing**: templates ship one at a time behind real customers
  (Leadgen Engine first — it's furthest along and self-demonstrating: the
  campaign that sells it is it), each new template gated on a verification
  rubric and at least one receipted external outcome, promise-registry
  style. No template enters the catalog on a self-reported number — that
  discipline is precisely what distinguishes us from the mastermind's
  unaudited claims.

---

## 6. What we deliberately do NOT copy

- **AI-written mass outreach.** The room's best operator keeps volume at
  10–20/day; the blitz rule (AI does the product work, humans write the
  outreach that matters) stays.
- **Fake-internal-email deliverability tricks and mass-application bots.**
  Short-half-life plays that poison the commons and our brand.
- **Unverified multiplier claims.** "3 years into 2 hours" is a great story
  and unusable as marketing. Every number we publish rides on a receipt.
- **Harness competition.** The room's harness churn (OpenClaw → Hermes) is a
  warning: harnesses commoditize. We wrap and orchestrate (Khala Code over
  codex/claude), and we differentiate on trust, hosting, provenance, and the
  economy — the layers the room cannot duct-tape.
- **Personality-first agents without permission substance.** Souls and names
  are adoption UX, and we use them — but every named employee is backed by
  typed authority states and receipts, or the metaphor is a liability.

---

## 7. Summary: the bottle

The mastermind room is what the agentic economy looks like at artisanal
scale: every operator hand-blowing their own glass. The magic worth bottling
is a six-tuple — persona, permission stack, skills, data seed, schedule,
proof — and OpenAgents already owns the right container for each element:
`agent_definition.v1`, behavior contracts and staged authority,
provenance-receipted skills, the grounded context library in a prefilled
workspace, fulfillment-agent scheduling on managed always-on capacity, and
accepted-outcome receipts. Khala Code is both the power tool we sell the
room's builders and the factory that stamps out employee packages for
everyone else; Autopilot is the business that hires them out. The campaign
is the blitz playbook aimed at the segments this room just handed us, using
the market's own vocabulary (levels, employees, outcomes) and its own proof
grammar ("my agent did X and got outcome") — with the one upgrade nobody in
that room can fake: receipts.

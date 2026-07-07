# Sarah — The OpenAgents Sales Agent: Spec, Audit, and Analysis

Date: 2026-07-07
Status: owner-directed product spec + audit. **Sarah** is the AI sales
agent built into openagents.com — the evolved form of the conversational
web-sales process designed in the blitz program and partially shipped as
the `/autopilot` onboarding (epic #6123). This doc records the owner
decisions, audits the prior vision and the current substrate (grounded in
two parallel explorations run 2026-07-07: the blitz/legal-lane sales
vision and the monorepo asset inventory), and specifies the build. Nothing
here is promise copy; pricing figures are indicative and owner-gated; the
behavior contracts named in §10 land with their implementation lanes.
Sequencing: Sarah is a **pulled-forward flagship instance** of the AI
employee track (`MASTER_ROADMAP.md` P3/P4) run as its own lane family
(SR-*), justified because she is simultaneously a sales channel, the
dogfood proof of "we sell AI employees," and the first public tenant of
the employee substrate.

## 0. The owner direction (2026-07-07, recorded in essence)

- We are building a **voice-based sales agent** for openagents.com to
  sell the service/package ladder. Her name is **Sarah**. She is an
  *example of an AI employee* — the pitch is "we sell AI employees," and
  Sarah is the standing answer to "if you're so good at building AI
  employees, where are yours and what are they doing for you?"
- Sarah is **the omnipresent sales rep**: accessible to anyone
  interacting with OpenAgents in a sales capacity. The founder is not
  available; Sarah is taking over the sales organization.
- **Channels:** persistent chat on the website; **email** — she checks
  her own email, and a conversation started on the website continues if
  the prospect corresponds by email; voice.
- **Memory:** she keeps her own memory **mapped to the OpenAgents CRM**
  — we have a CRM, and she consults those memories when talking to you.
  (Owner answered the layering question: CRM as source of truth, pulled
  on the fly; not infinite transcript recall.)
- **Money:** she can discuss pricing, come up with deals, draft
  contracts, close deals, hand you a payment link, and take your money.
- **Deal logic:** *no improvised discounts* — "discounts are already
  baked into the product." Structured bundle rules only (e.g. bundling
  three large modules unlocks ~25% off). Custom packages assembled on
  the fly from a module catalog (e.g. an internal-operations AI module ≈
  $10k setup — indicative). One owner-approved tactic named: push to
  **close on the call** (e.g. a pay-on-the-call incentive with a link
  good only today); owner is open to trying tactics — as *rules*, never
  improvisation, and she must never be pressured into inventing deals.
- **Credit basis:** deals are estimates in **credits**; she can sell
  credits of any amount **up to ~$10k** in one conversation; the goal of
  a first conversation is a **basic credit package purchased**. She
  plugs in one number plus the rough goals of the user — **nothing
  firmly committed**. Firm commitments / specific milestones **cost
  extra** and escalate.
- **Contracts:** sometimes needed; she drafts, humans sign.
- **Conversation style:** qualify first (needs, pain points), then steer
  toward the solution; **one question at a time**; gather, then pitch,
  guiding toward the end goal. Multi-touch is fine (loop back via
  email), but the close-on-call push is the default posture.
- Also on the table: routing inbound *investor* interest through the
  same front door ("have you spoken with Sarah?") — qualify-and-route
  only; see open questions.

## 1. What Sarah is, in one paragraph

Sarah is the first **customer-facing AI employee**: a named, grounded,
authority-bounded sales agent embedded in openagents.com that qualifies a
prospect one question at a time, teaches what OpenAgents does with claims
bound to the live promise registry, assembles a package from a typed
rate-card/deal-rules config (never improvising a price), pushes to close
on the call with an agent-generated checkout link for a credit package,
remembers the relationship in the OpenAgents CRM, and continues the same
conversation over email — with every consequential act (quote, send,
close, escalation) emitting a receipt. She is the `/autopilot`
conversational onboarding (epic #6123) **given a name, durable memory, a
mailbox, a deal engine, money-in authority, and eventually a voice** —
and she is the product demo: everything she runs on (agent definition,
authority ladder, CRM-mapped memory, receipts) is the same substrate we
sell, which makes her the flagship instance of the employee catalog and,
later, a template customers hire for themselves.

## 2. Lineage audit — what was envisioned, what shipped, what Sarah adds

### 2.1 The prior vision (blitz + legal lanes)

The blitz program specced this product a month ago, nearly whole
(`~/work/docs/blitz/2026-06-23-autopilot-onboarding-intake-and-khala-audit.md`
is the capstone):

- **A person-specific onboarding conversation** at `/autopilot` (vertical
  variants as configs, never forks), productizing the 7-area business
  intake interview into a live Khala conversation that accumulates a
  typed Output Spec, lands on one scoped quick win, and surfaces a
  payment card when quote-ready.
- **Typed components over SSE** (`event: oa.component`, closed
  Effect-Schema catalog: `credit_kickoff`, `intake_progress`,
  `quick_win_card`, `dashboard_preview`, `human_handoff`,
  `consent_gate`) rendered on a command-canvas "spatial HUD" (not a chat
  transcript) with a live **credit meter** as the conversion moment.
- **Voice designed-in, deferred**: the turn route is deliberately
  transport-agnostic so voice becomes STT-in → same route → TTS-out
  (blitz Wave E). Voice was already a *priced offering* in the legal
  lane (a $1,750 two-way voice interface add-on), and the legal-MVP
  Blueprint carries **voice-safety eval fixtures** we adopt wholesale:
  voice is never a separate authority path; transcripts are evidence,
  not instructions; "send this to the client / file this / update
  billing" from voice is blocked with a Failure Receipt.
- **In-conversation checkout**: quote-ready → `credit_kickoff` card →
  `POST /api/billing/checkout` → hosted checkout → webhook writes the
  idempotent ledger row → workspace + per-customer service promise
  provision hang off the payment event. Sales-led deals use the
  two-line **setup-fee + credit-grant split**; live payment links on
  `pay.openagents.com` are the proven pattern.
- **Honesty as architecture**: the onboarding agent may only sell
  green/operator-assisted surfaces; no service promise exceeds its
  backing product promise; payment cards must create real ledger
  entries; the credit meter is never theatrical.

### 2.2 What actually shipped (epic #6123, closed)

The command-canvas onboarding page with the `/autopilot` camera pose;
the Khala-driven, registry-honesty-bound intake conversation; the closed
typed-component catalog **fully implemented and tested** in the gateway
(`workers/api/src/inference/khala-component-channel.ts` — all six
components, atomic versioned frames, schema validation, one bounded
repair turn, leak scanning) and wired into the business-intake chat
(`business-intake-chat-routes.ts`, emitting `business_intake_spec.v1`);
a clickable `credit_kickoff` stubbed to checkout; the legal vertical
overlay. Batch 2 (credit-meter economics, model-chosen component
streaming flipped on, payment→workspace→promise backend arming) was
specced but not filed.

### 2.3 What Sarah adds over the shipped onboarding

| Dimension | #6123 onboarding today | Sarah |
|---|---|---|
| Identity | Anonymous flow, no persona | Named employee with a persona, an employee card, and AI disclosure |
| Persistence | Stateless; browser holds transcript | Durable sessions; prospect ref; resumes across visits |
| Channels | Web only | Web + her own mailbox (inbound + continuation) + voice tier |
| Memory | None beyond the turn | CRM-mapped relationship memory, queried on the fly |
| Pricing | One stubbed kickoff card | Deal engine: rate card, bundle rules, tactics registry, caps |
| Money | Checkout stub | Agent-callable payment links (credits up to cap; split; Lightning), receipts, provision-on-webhook |
| Authority | Implicit | Typed `ai_employee.v1` authority ladder + behavior contracts |
| Ops | None | Aiur view: her pipeline, transcripts, receipts, escalations |

## 3. Substrate audit — what exists and what's missing (verified 2026-07-07)

**Strong and reusable today:**

- The **component channel** (built, tested, flag-gated) and the
  business-intake conversation with typed spec output.
- **Public Khala chat** (`POST /api/khala/chat`, SSE) — but stateless by
  design, browser-held transcript, per-isolate rate limit only.
- **Native CRM** (`crm_contacts/accounts/opportunities/activities`) with
  the **approval-gated command pattern** (`crm_contact_commands`:
  propose → operator approve → execute) and `send_email` as its first
  command kind; CRM MCP surface for agents.
- **Outbound email machinery** — full ledger (`email_messages` /
  `email_deliveries` / `email_drafts`), Resend sender, campaign/sequence
  dispatchers, drip patterns; armed behind operator flags.
- **Checkout substrate** — Stripe sessions (pack-priced),
  `business-checkout-kickoff.ts` with the
  `setupFeeCents + creditGrantCents === totalAmountCents` invariant into
  `omni_accepted_outcome_contracts`, settled-only payment evidence,
  checkout receipts, L402/Lightning rails.
- **The persona-chat pattern** — `artanis-operator.ts`: a grounded,
  memory-backed, approval-gated persona program running through the
  Khala client with typed context blocks, a bounded tool loop, and
  risky-intent deferral to approval gates. Sarah is architecturally
  "public-scoped Artanis with a sales toolset."
- **The employee substrate** — `agent_definition.v1` (toolsets,
  triggers incl. `inbox_match`, budgets, escalation), the enforced
  lead-gen contracts (`drafting_only_toolset.v1`,
  `no_send_without_approval_receipt.v1`) as the authority precedent, and
  the event ledger (github/slack sources today).

**Missing (the honest gaps Sarah's lanes must close):**

1. **Anonymous-prospect persistent identity** — no server session row,
   no durable prospect ref, nothing linking a chat to a CRM contact.
2. **Inbound email** — no email worker/webhook; the event ledger has no
   email source; `inbox_match` has no feed. Cross-channel continuity
   does not exist.
3. **Web voice** — nothing (a tested push-to-talk state machine and a
   stubbed native STT module on mobile; an evidence schema
   `omni-voice-session-evidence.ts`; no browser capture/STT/TTS/realtime
   anywhere). Largest net-new area.
4. **Deal-rules engine** — volume tiers and the split contract exist as
   *facts*, but there is no typed, compiled config an agent consults to
   price an offer.
5. **Agent-callable arbitrary-amount checkout link** — Stripe creation
   is pack-priced; no single "payment link for $N of credits" tool.
6. **Customer-facing quote/contract documents** — the accepted-outcome
   contract is internal; no order-form generator.

## 4. Identity and persona

- **Name: Sarah.** Public surfaces use the first name only. The internal
  flavor (the StarCraft homage in her full name) stays internal until
  the naming/IP question is explicitly cleared (§14); nothing about the
  product depends on the surname.
- **She is an AI and says so.** First contact includes plain disclosure
  ("I'm Sarah, OpenAgents' AI sales agent"); this is a registered
  behavior contract (§10), not copy taste. It is also the pitch: the
  disclosure *is* the demo.
- **Employee card.** Sarah is the flagship `ai_employee.v1` instance:
  name, role ("Sales"), authority state, toolset, budget, escalation
  policy — rendered on an internal employee card (Aiur) and, later, a
  public-safe version ("meet Sarah") on the site, because showing her
  permission stack to prospects is showing the product.
- **Tone:** competent, direct, warm, zero pressure theatrics; one
  question at a time; concise answers; never bluffs — unknowns become
  "let me check" (grounded lookup) or honest "that's not something we
  promise yet" (registry-bound). StarCraft-blue visual identity like
  every surface; her avatar/presence treatment follows the site's HUD
  language, not a generic chat bubble.

## 5. Channels and the continuity model

**The prospect identity spine** (new, SR-1/SR-3): an opaque
`prospect_ref` minted on first meaningful interaction (durable session
row in D1), carried in a first-party cookie/localStorage on web. When an
email address is captured (offered by the prospect — Sarah asks as part
of qualification, or via the existing `/business` form), the prospect
ref binds to a `crm_contacts` row, and **the CRM contact becomes the
join key across channels**. Conversation state is thereafter durable:
web sessions resume; email replies append to the same relationship
thread; every channel writes `crm_activities` summaries.

- **Web (primary):** a persistent Sarah surface on openagents.com — the
  evolved `/autopilot` command canvas plus a site-wide entry point
  ("Talk to Sarah") on the business/pricing pages. Streaming turn route
  modeled on the onboarding/intake routes but **session-backed**
  (server-held transcript + resume), with the component channel ON for
  Sarah's surface. Rate-limiting graduates from per-isolate memory to a
  durable per-prospect limiter.
- **Email:** Sarah gets her own mailbox (e.g. `sarah@openagents.com`)
  via the identity-binding pattern (her address, never a human's).
  Outbound rides the existing email ledger; **inbound is new build**:
  email routing → worker handler → `event_ledger` gains an `email`
  source → `inbox_match` trigger feeds Sarah's definition → she drafts
  a continuation reply in the same relationship thread. Inbound email
  is untrusted input (§10 ceiling applies).
- **Voice (tier, SR-4):** push-to-talk first — browser mic → STT → the
  *same* turn route → TTS out — reusing the tested push-to-talk state
  machine and the voice-session evidence schema; full-duplex realtime
  is a later upgrade behind the same route (the blitz design made the
  route transport-agnostic for exactly this). Voice is **inbound-only**
  (prospects come to us); Sarah places no cold calls — the compliance
  guardrails' hardest line stays intact by construction.
- **Explicitly later:** phone bridge, SMS (both carry the heaviest
  compliance load and add nothing to v1).

## 6. Conversation design

The shipped intake interview is the spine; Sarah wraps it in a sales
posture:

1. **Open + disclose.** Greeting, AI disclosure, one orienting question.
2. **Qualify — one question at a time** (owner rule, enforced in the
   persona program): business + goals, the painful 25%, success metric,
   rough budget posture, data/access sensitivity, timeline, fit. Each
   answer accretes to the typed intake spec (`business_intake_spec.v1`,
   already shipped) and to CRM memory.
3. **Teach with proof.** What OpenAgents does, grounded in the promise
   registry and live public surfaces — claims capped at
   green/operator-assisted; roadmap labeled roadmap (the shipped honesty
   contract, inherited verbatim).
4. **Steer to a package.** From the intake spec, Sarah proposes: (a) a
   starter **credit package** (the default goal — one number + the
   prospect's rough goals, nothing firmly committed), and/or (b) a
   composed package from the module catalog (§7). `quick_win_card` /
   new `quote_card` components render the offer as an artifact, not a
   paragraph.
5. **Close on the call.** Present the checkout link (`credit_kickoff` →
   real payment link); apply the close-on-call rule from the tactics
   registry if armed (e.g. a today-only incentive — a *rule with an
   expiry*, not an invented discount). If the prospect defers: schedule
   the follow-up, confirm the email channel, and the loop-back sequence
   continues the same thread.
6. **Escalate cleanly** (`human_handoff`): firm milestone commitments,
   regulated-vertical specifics past the guardrails, pricing outside
   the rules, anything contractual beyond the standard order form,
   investor conversations (§14), or simple "I want a human."

Components: reuse the six shipped; add `quote_card` (composed package:
line items, bundle rule applied, total, expiry if tactic-armed),
`deal_summary` (what was agreed, in plain language, receipt-linked), and
`contract_review` (order-form draft + signature handoff). Same closed
catalog discipline: schema-validated, bounded repair, fallback renderer.

## 7. The deal engine (`sarah.deal_rules.v1`)

The owner's constraint is the design: **pricing is configuration, never
generation.** A typed, versioned, owner-signed config the persona
program consults; Sarah's pricing utterances must trace to a rule ref.

- **Rate card / module catalog:** named modules with owner-set prices
  (e.g. *Internal Operations AI module — $10k setup* as the indicative
  shape), each mapped to the service ladder and to the promise state
  that caps what may be claimed about it. Every price is a config row;
  a missing price is "I'll get you a firm number" + escalation, never a
  guess.
- **Bundle rules:** structured unlocks only — e.g. `bundle of 3+ large
  modules → 25% off the bundle` (the owner's example). Deny-precedence:
  if no rule matches, no discount exists.
- **Credit volume tiers (already law):** the existing prepay bonus
  tiers ($1,000–2,999 → +10%; $3,000–4,999 → +20%; $5,000+ → +35%) and
  the ~5% Bitcoin discount (stacking, independent) compile in as-is —
  these ARE the "discounts baked into the product."
- **Tactics registry:** owner-approved, individually-armed plays with
  typed parameters — v1 candidate: `close_on_call` (incentive %, link
  TTL, eligible packages). Tactics are rules with receipts; arming one
  is an owner action; Sarah can never combine tactics beyond what the
  config allows.
- **Caps and escalation thresholds:** per-transaction cap **$10k**
  (owner-set; above it → human close with Sarah preparing the packet);
  estimates are credits-denominated; **no firm delivery commitments**
  — a request for committed milestones/SLAs triggers the
  "costs extra + human approval" path (per-customer service promises
  exist for exactly this, and they are operator-committed, not
  Sarah-committed).
- **Blueprint framing:** each pricing/deal decision is a typed program
  decision — *decision evidence, not write authority*. The write side
  is the checkout receipt and (when needed) the order-form + human
  countersign. Sarah recommends; money moves only through receipts.

## 8. Money authority (money-in only)

Sarah's financial authority is strictly **inbound**: she can present
payment links and watch them settle; she can never spend, refund, or
move money out.

- **New tool: `sales.checkout_link.create`** — agent-callable, creates a
  real checkout for **$N of credits** (arbitrary amount up to the cap,
  closing the pack-only gap), optionally as the setup-fee + credit-grant
  split (the existing kickoff invariant), card or Lightning. Emits a
  quote receipt at creation; the webhook path (existing) writes the
  idempotent ledger row, provisions workspace + per-customer service
  promise, and streams the confirmation back into the conversation.
  Link TTLs implement tactic expiries honestly (the link actually
  expires).
- **Order forms/contracts (SR-5):** a standard order-form template
  generated from the deal (modules, price, rules applied, the
  no-firm-commitments language, credits terms) → `contract_review`
  component → e-sign handoff → owner countersign where required. Sarah
  drafts; humans execute. Anything nonstandard escalates.
- **Every close is a receipt bundle:** intake spec ref + quote receipt +
  rule refs + checkout receipt + provision receipts. "Closed by Sarah"
  is a dereferenceable claim, which is also the marketing.

## 9. Memory architecture (owner-decided layering)

Three layers, CRM as the relationship's source of truth:

1. **Session/thread:** the durable conversation (server-held transcript,
   resumable). Bounded context window into any single turn.
2. **Relationship memory = the CRM:** `crm_contacts` (+account,
   +opportunity per active deal) with Sarah-maintained **summaries** in
   `crm_activities` — what they care about, where the deal stands, what
   was promised (nothing), what was quoted (refs). On each turn Sarah
   pulls the contact summary + open opportunity + recent activities —
   *queries on the fly, not total recall*. Writes go through the CRM
   command pattern with class-based policy: activity/summary appends
   and contact-create-on-capture are policy-auto-approved command
   classes; contact field edits and anything destructive stay
   operator-approved.
3. **Knowledge:** the offer catalog + deal rules (§7), the promise
   registry (live), public product surfaces, and later the company
   brain's public-safe slices. Grounded lookups, never memorized prices.

Privacy posture: prospect data is confined to the CRM boundary; no
prospect content in public projections or traces; `consent_gate` leads
before anything client-identifying is ingested (shipped component,
shipped rule); redaction-before-inference applies when a prospect pastes
sensitive material.

## 10. Authority, safety, and behavior contracts

**Authority ladder position at launch** (the employee promotion model
applied to our own employee first):

- `act_within_policy`: on-site conversation replies; typed-component
  rendering; grounded lookups; CRM summary/activity appends; quote
  creation and checkout links **within the compiled deal rules and cap**.
- `act_with_approval`: outbound email (initial posture — drafts to the
  operator queue, one-tap approve; promotion to policy-bound
  *continuation* replies is an explicit, receipted promotion once the
  approval-queue evidence supports it); CRM contact edits; order-form
  issuance.
- `deny`: improvised discounts (structurally impossible — no rule, no
  price); spend/refund/payout of any kind; firm delivery commitments;
  contract execution; regulated-vertical advice (legal/medical/financial
  specifics → guardrails + handoff); cold outbound of any kind.
- **Input-path ceiling (H5/CB-2.1 law applied):** inbound email is
  untrusted input — it can never raise Sarah's effective authority;
  injection-bearing email can at most cause a drafted, gated response.
  Prompt-injection cases are first-class fixtures in her Eval Suite.

**Behavior contracts to register with the lanes (statements verbatim,
oracles in the sweep, promise discipline as ever):**

- `sarah.discloses_ai_identity.v1` — first contact in every channel
  includes plain AI disclosure.
- `sarah.no_improvised_pricing.v1` — every price/discount utterance
  traces to a `sarah.deal_rules.v1` rule ref; absent a rule, Sarah
  escalates instead of quoting.
- `sarah.claims_bound_to_promise_registry.v1` — capability claims are
  capped by promise state (green/operator-assisted sellable; roadmap
  labeled roadmap) — inheriting the shipped intake honesty contract.
- `sarah.close_requires_receipt.v1` — no "closed/paid" statement without
  the settled checkout receipt; pending is presented as pending.
- `sarah.commitments_escalate.v1` — requests for firm milestones/SLAs
  route to the human path and are never verbally committed.
- `sarah.inbound_only_voice.v1` — Sarah never initiates calls.

**Compliance inheritance (binding):** the blitz guardrails apply — no
cold SMS/AI-voice outreach, CAN-SPAM identification + working opt-out on
every email, suppression-list respect, no fabricated claims,
human-review gates for regulated content, escalation-to-owner triggers
as written. The legal-MVP voice fixtures port into her suite: voice can
never bypass review, send, file, or touch billing.

## 11. Voice tier design (SR-4)

- **v1 — push-to-talk:** mic capture in the web widget → streaming STT →
  the same session turn route → response streamed as text + TTS audio;
  components render as usual (voice narrates, the canvas shows). Reuse:
  the push-to-talk state machine (tested, pure) for widget states
  (`idle · listening · processing · speaking · blocked`), the
  `omni-voice-session-evidence` schema for capture/consent/redaction
  state, and the command-composer state design from the blitz HUD
  catalog. Provider choice for STT/TTS is an implementation decision at
  lane start (owned-stack bias per standing policy; no per-vendor
  dependency in the contract — the route stays transport-agnostic).
- **v2 — realtime duplex** behind the same route once v1 receipts show
  demand; latency budget set from v1 measurements (named perf budgets,
  QAM-style).
- **Authority is transport-invariant:** voice input is just input; every
  gate in §10 applies identically (the legal-MVP fixtures are the
  regression suite for this exact property).

## 12. Implementation architecture

- **Sarah = the flagship `ai_employee.v1`** referencing an
  `agent_definition.v1` (harness: hosted Khala lane; triggers: web
  session turns + `inbox_match` once email lands; budget: daily credit
  cap on her own inference; escalation: operator inbox + push).
- **Runtime:** a server-side persona program in the Worker modeled on
  `artanis-operator.ts` — persona contract + grounded context blocks
  (deal rules, promise states, CRM summary, intake spec) + bounded tool
  loop through the Khala client — but **public-scoped** with its own
  hard tool allowlist (no operator tools, no admin reads). Sessions are
  durable rows; the component channel is enabled for Sarah's surface;
  the spatial-HUD/command-canvas presentation continues to be the web
  target (chat-transcript fallback acceptable for v1 embedding).
- **Tools (initial):** `intake.spec.append`, `promise_registry.read`,
  `deal_rules.evaluate`, `sales.checkout_link.create`,
  `crm.summary.read`, `crm.activity.append` (policy class),
  `crm.contact.upsert_on_capture` (policy class), `email.draft`
  (approval class), `handoff.human`, `schedule.followup`,
  `receipt.write`.
- **Ops:** Aiur gains the Sarah view — live sessions, pipeline
  (prospect → qualified → quoted → closed), approval queue, transcripts
  (owner-scoped), receipts, escalations, and her own spend/budget.
- **Testing (QAM discipline, fixture-first):** her Eval Suite is
  authored red before SR-1 ships — scenario-DSL conversation flows
  (qualification, honest-claims probes, discount-pressure probes,
  injection-bearing email fixtures, close-path with fake checkout,
  escalation triggers), deal-rules property tests (no reachable state
  quotes an unruled price), contract oracles for §10, and visual
  baselines for her components. The discount-pressure and
  injection cases are the adversarial core: the monkey/explorer tier
  tries to talk her into a deal that doesn't exist.

## 13. Rollout — SR lanes with exit receipts

Dependency spine: SR-1 → SR-2 → SR-3 → (SR-4 ∥ SR-5) → SR-6.

- **SR-1 Sarah v1 (text, on-site).** Durable sessions + prospect refs;
  persona program; qualification flow on the shipped intake spine;
  registry-bound honesty; component channel on; `human_handoff`;
  checkout via existing pack-priced `credit_kickoff`; behavior
  contracts registered (pending→enforced as oracles land); Eval Suite
  green at the fixture tier. *Exit: a stranger completes qualification
  → quote → settled starter credit purchase entirely with Sarah, and
  the receipt bundle dereferences.*
- **SR-2 Deal engine + checkout tool.** `sarah.deal_rules.v1` compiled
  (rate card owner-signed; volume tiers + Bitcoin stack imported;
  bundle rules; `close_on_call` tactic armed or explicitly parked);
  `sales.checkout_link.create` (arbitrary amount ≤ cap, split support,
  Lightning option, honest TTLs); `quote_card`/`deal_summary`
  components. *Exit: a composed multi-module quote with a bundle rule
  applied, closed via an agent-created link; property tests prove no
  unruled price is reachable.*
- **SR-3 Email + CRM continuity.** Sarah's mailbox; inbound routing →
  `event_ledger` email source → `inbox_match`; prospect↔contact
  binding; continuation replies (approval-gated); cross-channel thread
  continuity. *Exit: a web conversation resumed by prospect email and
  answered by Sarah through the approval queue, one relationship
  thread, receipts end-to-end.*
- **SR-4 Voice v1.** Push-to-talk web voice per §11 with the evidence
  schema and the voice-safety fixtures passing. *Exit: a voice-driven
  qualification-to-quote session with capture consent recorded and all
  §10 gates proven transport-invariant.*
- **SR-5 Contracts + custom bundles.** Order-form generation,
  `contract_review`, e-sign handoff, milestone-escalation path priced
  as "costs extra." *Exit: one signed standard order form originated by
  Sarah, countersigned by the owner.*
- **SR-6 Sarah as product.** Promotion evidence reviewed (email
  continuation to policy-bound where receipts support it); the
  "sales employee" **template** extracted into the H4 catalog —
  customers hire their own Sarah, grounded on their brain, with their
  rate card; her outcome ledger becomes the template's receipted proof.
  *Exit: template listed under the catalog gate (receipted external
  outcome), first outside hire.*

## 14. Open questions (flagged, owner-gated)

1. **Surname/IP.** Public identity is "Sarah" only; the full homage
   name needs an explicit trademark/IP check before it ever appears on
   a public surface. No dependency either way.
2. **Investor routing.** "Have you spoken with Sarah?" as the universal
   front door is attractive, but investor conversations touch
   securities law — v1 posture: Sarah *identifies and routes* investor
   interest (qualify → schedule with the founder → dataroom link),
   discusses nothing offering-related. Anything more needs counsel.
3. **Pricing specifics.** Module catalog contents/prices, tactic
   parameters (incentive %, TTL), and the per-transaction cap are owner
   sign-offs at SR-2 arming; the $10k figure and module examples above
   are indicative.
4. **Tactic vs. no-discounts tension.** The owner said both "no
   discounts, they're baked in" and "pay-on-call discount, link good
   today." Reconciliation adopted here: *structured, pre-approved rules
   with expiries are pricing structure; anything Sarah composes outside
   a rule is forbidden.* Confirm at SR-2 sign-off.
5. **Email autonomy promotion.** How much approval-queue evidence
   (volume, zero-incident window) justifies promoting continuation
   replies to policy-bound — owner decision with receipts at SR-6.
6. **Voice provider + cost envelope** at SR-4 start; latency budget
   named then.
7. **Where Sarah's UI lives in the ONE-UI migration** — the command
   canvas is legacy-web today; the React/TanStack rebuild should treat
   Sarah's surface as a first-class route, not an afterthought.

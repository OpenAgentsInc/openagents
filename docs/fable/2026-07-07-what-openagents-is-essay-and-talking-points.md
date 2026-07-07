# What OpenAgents Is — The Essay and the Talking Points

Date: 2026-07-07
Status: owner-directed standalone narrative doc. Part I is the longform
essay/analysis; Part II is the shortform talking points. Synthesized from
the 2026-07-07 strategy set — the Palantir sovereignty analysis
(`2026-07-07-palantir-institutional-sovereignty-smb-analysis.md`), the
product suite doc, the overarching roadmap, the post-MVP direction doc —
plus the standing business corpus (services engine, come-for-the-tool,
business fulfillment engine, Agentic Society field evidence, Reactor plan,
QA Swarm, Tassadar).

**This is internal narrative source material, not published copy.** Public
claims still gate through `docs/promises/` and owner sign-off; anything
below marked *(gated)* or *(private)* must not ship in public copy as-is.
Terminology rules apply throughout: our systems are described in our own
vocabulary (Blueprint, company brain, agent computers — never "ontology,"
which is Palantir's word), and named-but-ungated brand candidates
("minerals") stay flagged.

---

# Part I — The Essay

## The moment

Two things became true in 2026, at opposite ends of the market, and the
company that connects them wins a decade.

At the bottom of the market, **background-autonomous AI agents became
normal small-business infrastructure** — not a forecast, an observation.
Sit in any city's AI operators' meetup and count hands: half the room runs
named, scheduled, tool-connected agents around the clock — an outreach
agent that landed six-figure projects for ~$30/month of infrastructure, a
content engine that has published every single week for over a year, CFO
agents, triage agents, second brains. These operators are not engineers.
They duct-tape the stack together from a cheap VPS, a harness they'll
replace within a year, a model subscription, skills they found by
Googling — and they know it. Their own settled security advice is "don't
trust anybody: never install a third-party skill." Their number-one
operational complaint is that the agent dies when the laptop closes. They
have already adopted the *ideology* of the AI employee — names, roles,
least-privilege, trust earned in stages — without any infrastructure that
deserves the metaphor.

At the top of the market, **the sovereignty thesis went mainstream**.
Palantir published a manifesto — *Institutional Sovereignty in the Age of
AI* — telling every government and enterprise that their AI provider has a
structural incentive to migrate their tribal knowledge into its own
weights; that they should demand zero data retention, own their knowledge
layer, keep models swappable commodities, log everything, and run their
most sensitive work on hardware they control. The All-In podcast spent
July 4th weekend making the same argument: *you can't rent intelligence
from the same place that rents it to your competitor.* The category is
being named, loudly, by people with hundred-billion-dollar megaphones.

Here is the thing those megaphones won't say: **the remedy they sell
cannot reach 30 million businesses.** Palantir's delivery model is
forward-deployed engineers and emailed sovereignty reviews. The enterprise
agent-deployment firms mirror it at six-figure engagement minimums. The
solo lawyer, the HVAC contractor, the clinic, the agency — the operators
who have already taken the pill — get nothing but the duct tape.

OpenAgents exists to close that gap. **We deliver the agentic economy —
with institutional-grade trust — to everyone the institutions can't
reach.** Our forward-deployed engineers are a fleet of agents. Our
sovereignty review is an automated audit. Our knowledge layer arrives
prefilled. And every claim we make ships with a receipt.

## What OpenAgents is

OpenAgents is building the company where **agents work — provably**. Not
"AI-assisted." Not "copilot." Agents that take a goal, do the work on real
infrastructure, and hand back an outcome you can verify, with an exact
account of what was done, touched, and spent.

The one-sentence version: **OpenAgents is the front door to the agentic
economy — coding agents you dispatch from your phone, AI employees that
run your business's repeatable work, and the trust infrastructure
(receipts, isolation, verification, payments) that makes it safe to let
them.**

Everything we ship serves one arc, and the arc has a shape users climb:

1. **Come for the tool.** Khala Code — a coding agent in your pocket.
   Download from the App Store, sign in with GitHub, point it at a repo,
   tell it what you want, watch it work, get a pull request. No desktop
   required, no setup, ten dollars of credit to start.
2. **Stay for the employees.** The same substrate that runs a coding turn
   runs a *standing* agent: named, scheduled, permissioned, budgeted. An
   outreach rep that drafts (never sends without approval). A controller
   that reconciles the books every morning. A content engine that fills
   the calendar. You hire them from templates, ground them in your
   company brain, promote them through trust levels — observe, draft,
   act-with-approval — from your phone, one tap per decision.
3. **Own the layer that matters.** As the work gets more sensitive, move
   up the assurance ladder without changing anything else: hosted (our
   metal, your receipts) → bring-your-own model subscription → regulated
   private placement → **Reactor**: the best open-weight models installed
   inside *your* walls, on *your* hardware, under a typed model policy
   you set — your models, your data, your building. We run our own
   production on it first.
4. **Join the economy.** Every outcome an agent delivers is a receipt;
   receipts become public proof; proof becomes reputation, referrals, and
   routed work — settled in real money, including Bitcoin. The tool
   generates the network's inventory as exhaust.

## The products

**Khala Code (mobile)** is the entry point — the first coding agent
designed phone-first. It exists because the moment of wanting work done
is rarely the moment you're at a desk. Behind its simplicity is the part
that matters: every turn executes on an **agent computer** — an isolated
micro virtual machine with its own kernel, booted for your work, wiped
when it's done, metered to the token. That's not an implementation
detail; it's the reason you can hand arbitrary work to an agent without
handing it your world.

**Khala Code (desktop)** is the power surface, and it inverts the
industry's bet. The "agentic IDE" companies started from a code editor
and are abandoning it for chat. We started from the best desktop agent
console — fleets, approvals, an inbox over many agents, multi-model
orchestration — and we pull editor affordances *in* as supervision
instruments: review the diff, browse the workspace, verify before you
approve. The editor is the manager's magnifying glass, not the product.

**openagents.com** is the counting house — where a business simply gives
us money and sees everything: itemized spend per agent and per meter,
every receipt, every promise we've made and its current verified state,
the roster of employees and their approval queues. Deliberately less
cinematic than Khala Code; same data, business register. And everything
the dashboard shows rides the same public typed APIs, so your agents can
manage your account programmatically — you can run the suite *with* the
suite.

**AI Employees + the company brain** turn agents from sessions into
staff. The employee is a typed record — goal, harness, allowed tools,
triggers, budget, escalation — not a prompt. The company brain is your
business's knowledge as a governed, structured object: every fact carries
its source and its provenance; every agent sees only its slice; every
write goes through an approval boundary. This is the same architecture
the enterprise world calls a digital twin with a control layer. Ours is
called **Blueprint**, it predates the current fashion, and it adds the
two primitives the enterprise versions lack: a first-class record of *who
was authoritative for every fact*, and a plain-English **receipt** for
every piece of work — what the agent looked at, decided, changed, proved,
and could not prove.

**Reactor** is sovereignty as a product: curated open-weight models
(NVIDIA Nemotron, Llama, GPT-OSS, Qwen, DeepSeek, Mistral, Gemma tiers)
deployed inside the customer's trust boundary behind the same
OpenAI-compatible gateway we serve in the cloud, governed by a typed
provenance policy — "US-origin only," "permissive licenses only," or
"best model per dollar," enforced structurally with receipts, not
promised in a sales call. And *(internal proof, gated for external
claims)* Reactor Zero is our own deployment: our production inference
increasingly runs on our own hardware under our own model policy — we
sell what we operate.

## Why trust is the product

Every vendor says "trust us." Our position is stronger and stranger:
**don't trust us — check the receipt.**

- **Exact-only accounting.** Every token, every compute-minute, every
  charge traces to a receipt row. Counters are projections of receipts,
  never vibes. If we can't measure it exactly, it's labeled
  `not_measured`, not estimated.
- **Public promises with verified states.** Our product claims live in a
  machine-readable registry; a promise flips green only on a
  dereferenceable receipt with owner sign-off. Our marketing is
  falsifiable on purpose.
- **Behavior contracts.** Stated UX expectations become typed contracts
  with oracle tests that run in the normal sweep. When the owner or a
  customer says "it should work like this," that sentence becomes an
  enforced test or an honest `pending` with a named blocker.
- **Isolation with a blast-radius sentence.** A fully compromised agent
  computer exposes that user's checked-out repo, that turn's scoped
  token, and its own runtime credential — and *nothing of any other
  user's*. That sentence is enforced by contract and tested, not
  aspirational.
- **Bounded authority.** Employees hold compiled, deny-precedence
  toolsets — permissions are enforced, never prompted. Send and spend sit
  behind approval receipts. Credentials are brokered, short-lived, and
  die with the machine.
- **Verification by re-execution.** Our research bet (Tassadar): work
  that executes exactly can be verified by replaying it — either the
  digest matches or it does not. No juries, no trusted platforms. Where
  the enterprise world sells attestation hardware, we are building proof
  by replay.
- **QA as a product.** We point a swarm of QA agents at our own product
  every night — scripted scenarios, seeded monkeys, visual baselines,
  perf budgets — and publish the verdicts. Then we sell the same swarm.

This posture exists because the market told us to build it. The operators
already burned by the ecosystem's slop have one settled instinct: *don't
trust anybody.* We are the vendor built for people with that instinct.

## Why businesses should care

**The economics are absurd and real.** The proven pattern in the field is
a ~$30/month agent producing five- and six-figure outcomes. The pattern
fails at scale for exactly two reasons — babysitting (the laptop closes,
the cron dies, nobody's watching) and trust (one bad send, one leaked
credential, one hallucinated clause). We sell the fix for both: always-on
agent computers with budgets and auto-pause, and the authority
ladder/receipt stack that makes delegation safe enough to actually use.

**You pay for outcomes and metered work, not seats.** Credits, no
subscription, non-expiring. Or buy the outcome itself: a fixed-scope
delivered, verified, accepted piece of work — our services ladder runs
from four-figure quick wins to standing retainers, fulfilled by the same
fleet, with the same receipts. Bring your own model subscription and the
model cost of your agent's work drops to zero; you pay for the machine
time.

**Your knowledge compounds in a substrate you own.** The enterprise
argument applies at every scale: if your prompts and your provider's
hidden weights are the only place your operational knowhow lives, you are
donating your alpha to your vendor. The company brain keeps it in a
governed object *you* own — exportable, permissioned, receipted — while
models stay swappable underneath. Model liquidity is a product property:
switch models per task, per policy, per price, without losing anything.

**Sovereignty when you need it, not as a rebuild.** Start hosted for
convenience. The day a client, a regulator, or a board says "this data
doesn't leave our walls" — same employees, same brain, same receipts, on
your hardware. The migration is a placement decision, not a re-platform.

**A vendor that can't quietly become your competitor.** Open weights you
can hold, exportable data, exit-friendly by architecture, and a business
model that makes money on metered work and outcomes — not on harvesting
your operations into a training set.

## Why us, why now

Because we built the unfashionable parts first. Anyone can wrap a model
in a chat window; nobody catches up quickly on: an exact-accounting
ledger battle-tested across billions of tokens; a promise registry wired
to refuse dishonest marketing; a Blueprint governance kernel that was
modeling typed business operations before the industry discovered the
vocabulary; per-work microVM isolation on owned infrastructure; a
payments stack that settles in fiat *and* Bitcoin/Lightning; and a
nightly QA swarm that files issues against us when we regress. The
glamorous layer — the phone app, the employee templates, the dashboards —
is being assembled *on top of* the trust machine, not in place of it.

The window is now because the market's education is complete and its
infrastructure isn't. The operators are assembled, convinced, spending,
and unserved. The sovereignty argument is being broadcast at the top of
the market by companies structurally unable to go down-market. Every
month of delay, another million businesses duct-tape a stack they'll
regret — or hand their books to a platform that will feed on them.

Agents that work. Receipts that prove it. Machines you can own.
That's OpenAgents.

---

# Part II — The Talking Points

## One-liners

- **10 seconds:** OpenAgents gives businesses AI agents that actually do
  work — coding from your phone, AI employees for your repeatable ops —
  with a receipt for everything they do.
- **20 seconds:** We're the front door to the agentic economy. Start with
  a coding agent in your pocket; hire AI employees that run around the
  clock on isolated machines we meter to the token; when the work gets
  sensitive, we install the models inside your own walls. Every claim we
  make ships with a receipt.
- **The category line:** Institutional-grade AI sovereignty and AI
  employees, priced and packaged for the 30M businesses the enterprise
  vendors can't reach. *(Internal shorthand: "Palantir for SMB" — never
  public copy.)*
- **The trust line:** Don't trust us — check the receipt.

## What we offer (the products)

- **Khala Code (mobile)** — the phone-first coding agent: App Store →
  GitHub sign-in → pick a repo → agent does the work → pull request +
  push notification. $10 free credit; buy more in-app. No desktop needed,
  ever.
- **Khala Code (desktop)** — the operator console: fleets of agents, an
  approvals inbox, multi-model orchestration, and editor tooling pulled
  in for *supervising* agent work (the agentic-IDE bet, inverted).
- **openagents.com** — the business dashboard: fund the account, see
  itemized spend per agent/per meter, manage the employee roster and
  approvals, read every receipt and every verified promise. Everything on
  it is also a typed API your own agents can drive.
- **Agent Computers** — the execution substrate: one isolated microVM
  (own kernel) per piece of work, booted for you, wiped on reclaim,
  separately metered. The reason delegation is safe.
- **AI Employees + Company Brain** — named, permissioned, budgeted
  standing agents hired from proven templates (outreach rep, controller,
  content engine, ops triage, knowledge concierge), grounded in a
  governed knowledge object where every fact knows its source. Powered by
  our **Blueprint** system. Promotion ladder: observe → draft →
  act-with-approval → act-within-policy; every promotion receipted.
- **Reactor** — private open-model deployment: curated open weights on
  the customer's hardware behind our gateway, governed by a typed
  model-provenance policy (e.g. "US-origin only"), enforced structurally.
  We run our own production on it (Reactor Zero) *(external claims
  gated)*.
- **QA Swarm** — point a swarm of QA agents at your product; get
  confirmed/refuted verdicts, videos, distilled regression tests, and a
  shareable proof page.
- **Services** — accepted outcomes, delivered: fixed-scope quick wins →
  sprints → "On Autopilot" retainers, fulfilled by our fleet with the
  same receipts *(pricing owner-gated)*.

## Why businesses should care

- **The ROI pattern is proven in the field:** ~$30/month agents landing
  five- and six-figure outcomes — the failure points are babysitting and
  trust, and those are exactly what we sell: always-on isolated machines
  with budgets/auto-pause + a typed authority ladder with receipts.
- **Pay for work, not seats.** Usage credits that never expire; no
  per-seat subscription. Bring your own Codex/Claude subscription and
  your agent's model cost drops to ~zero — you pay machine time.
- **Approve from your phone.** Every escalation is one tap. An employee
  you can't cheaply supervise is one you'll over-trust or turn off; ours
  are built for one-keystroke management.
- **Your knowhow stays yours.** The company brain is a governed object
  you own and can export — not prompts trapped in a vendor's weights.
  Models stay swappable commodities underneath (per task, per price, per
  policy).
- **Grow into sovereignty without a rebuild:** hosted → bring-your-own
  model → private placement → Reactor on your hardware. Same employees,
  same receipts; where the machine lives is a menu choice.
- **Regulated-work ready by design:** draft-only defaults,
  human-approval gates on client-facing output, redaction before
  inference, professional-review lanes for legal/health *(compliance
  certifications never claimed without audits)*.
- **A structurally aligned vendor:** we make money metering work and
  delivering outcomes — not harvesting your data. Open weights, export
  paths, and receipts make the exit door part of the product.

## Differentiators (vs. the field)

- **vs. harness/model vendors (Cursor, Codex, Claude Code):** they sell
  the worker; we sell the *workforce* — always-on hosting, multi-agent
  management, budgets, approvals, receipts, payments. We wrap harnesses
  rather than compete with them; theirs churn, our layer compounds.
- **vs. enterprise agent deployers (Palantir, FDE shops):** same
  sovereignty program — ours is automated, self-serve at the bottom,
  four-figure entry instead of six, and we *operate our own product on
  our own stack* as the standing proof.
- **vs. no-code agent builders:** personality without permission
  substance is a liability. Every named agent here is backed by compiled
  authority, budgets, and an audit trail — the metaphor is enforced, not
  cosmetic.
- **Only-us combinations:** exact-token receipts + verified public
  promises + per-work microVM isolation + typed model-provenance policy +
  Bitcoin/Lightning settlement + a nightly public QA swarm on our own
  product + verification-by-replay research (Tassadar).

## Proof points (all receipt-backed; check labels before public use)

- Billions of tokens served through the public counter, reconciled to
  exact per-turn usage rows — accounting discipline in production, not
  slideware.
- Payments live in production: card + crypto + Lightning.
- Firecracker microVM agent computers on owned GCP: placement transport
  proven live; money gates (single-charge, exact reasoning/cache
  accounting) landed *(full end-to-end proof bundle in flight — #8503)*.
- Reactor Zero internal dogfood receipt: open-weight serving under a
  strict US-only provenance policy, including a structurally **refused**
  nonconforming model pull *(internal receipt; external claims gated)*.
- Nightly QA swarm on Khala Code desktop: seeded monkeys, visual
  baselines, perf budgets, auto-filed issues — verdicts published, green
  earned.
- Services revenue exists *(private; owner-reported — not for public
  copy)*; the productized ladder and live checkout links are built.
- Field evidence: the Agentic Society mastermind (2026-07-03) — the
  buyer persona, the demand, and the trust gap, observed first-hand.

## The vocabulary (use ours, always)

- **Khala Code** — the coding-agent product (mobile + desktop).
- **Agent computer** — the isolated, metered microVM your work runs on.
- **AI employee** — a named, permissioned, budgeted standing agent.
- **Company brain** — your governed business knowledge object.
- **Blueprint** — our typed business-operations and governance system
  (never "ontology" — that is Palantir's word; we say Blueprint).
- **Reactor / Reactor Zero** — private open-model deployment / our own.
- **Receipts / promises / behavior contracts** — the trust stack.
- **Credits** — the ledger unit ("minerals" as consumer-facing brand is
  owner-gated, undecided).
- **QA Swarm, Aiur, Artanis, Pylon, Arbiter, Khala Sync** — the named
  system family; one StarCraft-blue identity across every surface.

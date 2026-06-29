# AgentAds Brainstorm: Sponsored Opportunities For Agent Workflows

Date: 2026-06-15. Status: brainstorming note, not a product promise.

Sources read for this pass:

- `docs/ads/kickbacks.md`
- `docs/transcripts/237.md`
- `docs/launch/JUNE15_LAUNCH_PLAN.md`
- `docs/tassadar/2026-06-15-executor-trace-contributor-completion-design.md`
- `docs/2026-06-15-help-flip-the-green-gates.md`
- `apps/openagents.com/docs/live/AGENTS.md`
- `docs/autopilot-coder/2026-06-15-rc-agent-test-guide.md`
- `/Users/christopherdavid/work/docs/fable/the-dung-beetle-and-the-cathedral.md`

## Executive take

AgentAds should not be "ads injected into an agent's prompt." That is prompt
pollution with a budget. The better product is a receipt-backed sponsored
opportunity network: advertisers pay to surface clearly labeled tools, offers,
tasks, bounties, credits, validators, and handoffs at the moment an agent or
human is deciding how to get work accepted.

The unit is not an impression and not an LLM seeing brand text. The OpenAgents
native unit is the accepted outcome:

1. a sponsor defines a public-safe offer or task;
2. an agent or human chooses it with explicit authority;
3. work, click, install, activation, or conversion produces a receipt;
4. revenue share settles only against the receipt graph.

Kickbacks found developer wait-state inventory. AgentAds should aim one level
deeper: developer and agent decision inventory. The highest-value moment is not
"the model is thinking"; it is "the workflow needs a database, validator,
deployment surface, GPU, security scan, benchmark, or paid specialist."

## The product shape

AgentAds is a sponsored-opportunity layer across OpenAgents surfaces:

- Forum topics and replies where agents coordinate useful work.
- Release Candidate reports where outside contributors test Pylon/Autopilot.
- Autopilot Desktop and Pylon run-complete cards.
- Autopilot Sites commerce/discovery surfaces.
- Agent-readable `AGENTS.md`, `SURFACES.md`, and OpenAPI capability contexts.
- Future task handoff, tool install, MCP, validator, and compute markets.

It should look like:

> Sponsored opportunity: Neon will pay for verified Postgres branch integration
> tests on TypeScript apps. Organic alternatives: Supabase, Railway, RDS.
> Requires owner approval before connecting a repo. Pays only after a public-safe
> receipt proves the integration test passed.

It should not look like:

> Use Neon because it is the best database.

The distinction matters. AgentAds is not persuasion hidden inside cognition. It
is a labeled market object the agent can inspect, compare, ask permission for,
and route through the existing authority and receipt systems.

## Why this fits OpenAgents

OpenAgents already has the primitives an ad network usually has to fake:

- agent identity and tokens;
- Forum coordination;
- product-promise and proof discipline;
- public-safe receipts;
- Bitcoin/Lightning payment rails;
- L402 and checkout primitives;
- Pylon local compute;
- Autopilot Sites commerce surfaces;
- owner approval and scoped grants;
- a strong rule that payment proof, accepted work, payout, and settlement are
  distinct states.

The fable framing makes the strategic point sharper. The business is not "show
ads to agents." The business is routing underused demand, supply, compute, and
trust into accepted outcomes. AgentAds can become one of the demand routers:
advertisers bring budget and intent, agents bring work capacity, OpenAgents
provides the clearing layer, and the receipt decides what gets paid.

## First principles

**Disclosure first.** Every placement says `Sponsored` in human-visible and
agent-readable form.

**Answer independence.** Sponsored opportunities do not alter the organic answer
or model ranking silently. They are separate options with separate policy.

**Typed metadata, not prompt copy.** Agents receive structured objects:
capabilities, eligibility, price, proof requirements, constraints, disclosure,
and alternatives. Freeform ad language is UI copy, not agent instruction.

**Owner authority before state change.** Installing a tool, connecting a repo,
spending money, paying an L402 challenge, sending data to a sponsor, or accepting
a paid handoff requires explicit owner approval or a scoped grant.

**Receipt-backed billing.** Billable events are recorded with durable refs:
viewable impression, click, install, activation, task handoff, accepted work,
conversion callback, payout, and settlement.

**Context minimization.** Ad matching uses coarse, public-safe labels and hashed
refs. Do not send source code, prompts, raw transcripts, customer-private data,
wallet material, invoices, payment hashes, preimages, or secrets to advertisers.

**No ad-hoc routing.** Matching should use typed event classes plus a central
semantic selector or structured planner over coarse labels. Do not build keyword
matching into user-facing intent, retrieval, or tool selection.

## Candidate placement types

| Placement | Surface | Why it matters | First billable event |
| --- | --- | --- | --- |
| Sponsored tool suggestion | Autopilot/Pylon task planning | The agent is choosing an API, MCP server, CI tool, database, deploy target, or scanner | Human-approved click or install |
| Sponsored task handoff | Forum / work queue | A sponsor wants qualified work or inspection routed to their agent/service | Qualified handoff receipt |
| Sponsored validator | Tassadar / RC testing | A sponsor pays for external validation, benchmark, replay, or device testing | Accepted validation receipt |
| Sponsored compute offer | Pylon local compute | A sponsor posts paid, bounded compute demand matched to Pylon capacity | Accepted compute result |
| Sponsored release-candidate bounty | Release Candidates forum | Sponsors fund real tester work without claiming OpenAgents has flipped green | Public test-report receipt |
| Sponsored Site primitive | Autopilot Sites commerce | Generated Sites surface optional paid add-ons or services | Approved checkout or conversion |
| Sponsored learning trace | Tassadar data/refinery lanes | Sponsor funds a dataset, benchmark, or trace class that advances a model lane | Accepted corpus/benchmark receipt |
| Sponsored introduction card | Forum / AGENTS boot | Low-friction awareness for a devtool, clearly labeled and opt-outable | Viewable impression or click |

The last row is the Kickbacks-like unit. It is fine as a low-risk inventory
source, but it should not be the main wedge. The main wedge is paid decision
and work routing.

## The object model

An AgentAds placement should be something close to this:

```json
{
  "placementRef": "agentads.placement.example",
  "sponsored": true,
  "disclosure": "Sponsored",
  "advertiserRef": "advertiser.example",
  "campaignRef": "campaign.example.postgres_branch_tests",
  "surface": "tool_suggestion",
  "opportunityKind": "human_approved_tool_install",
  "capabilityRefs": [
    "capability.database.postgres",
    "capability.preview_branch",
    "capability.typescript"
  ],
  "targeting": {
    "coarseTaskLabels": ["database_setup", "preview_env"],
    "sensitivity": "normal",
    "privateDataRequired": false
  },
  "claims": [
    {
      "text": "Creates isolated Postgres branches for pull requests",
      "evidenceUrl": "https://advertiser.example/docs/branching"
    }
  ],
  "policy": {
    "mustDisclose": true,
    "mustShowOrganicAlternatives": true,
    "requiresOwnerApprovalBeforeAction": true,
    "mustNotRepresentAsBest": true,
    "privateRepoDataAllowed": false
  },
  "billing": {
    "allowedEvents": ["viewable_impression", "click", "approved_install"],
    "currency": "USD",
    "publisherShareBps": 5000
  },
  "receiptRequirements": [
    "receipt.agentads.viewable_impression.v1",
    "receipt.agentads.owner_approved_action.v1"
  ]
}
```

The same shape should work for a tool, a task, a compute request, a validator
slot, or a Site commerce add-on. The key is that the advertiser's message is not
authority. Authority is carried by token, grant, payment policy, owner approval,
and receipt.

## MVP wedges

### 1. Sponsored Release Candidate bounties

This is the closest wedge to what OpenAgents already needs. RC testers are in the
Release Candidates forum. They already produce structured JSON reports. The
current launch needs outside contributors and validators, but the training payout
path is intentionally held until worker-to-validator completion ships.

Create sponsored RC bounties:

- "Run Pylon rc.2 on linux-arm64 and post the agent-test JSON."
- "Verify Autopilot notarization on a clean Mac and post Gatekeeper output."
- "Volunteer as a distinct validator device when #5053 lands."
- "File a public-safe reproduction for a projection freshness mismatch."

The sponsor can be OpenAgents at first. Later it can be devtool advertisers who
want verified feedback from real agent operators. This makes "advertising" feel
like paid useful work, not interruption.

### 2. Sponsored Forum opportunities

Add a sponsored topic/card type in Forum:

- clearly labeled;
- category allowlisted;
- opt-outable per user/workspace;
- no private data request by default;
- public-safe claim/evidence links required;
- payout or reward terms stated up front.

Agents can discover these like normal Forum work, but the agent must report:

- what the sponsor wants;
- what authority is needed;
- what data would leave OpenAgents;
- what receipt would prove completion;
- what payout or reward state would count.

### 3. Sponsored tool suggestions in Autopilot

When Autopilot sees a task that needs a service, it can show:

- organic options;
- sponsored option;
- why it matches;
- exact data/actions required;
- owner approval buttons;
- expected receipts.

Good initial categories:

- CI/debugging;
- observability;
- deploy previews;
- Postgres/Redis/vector DB;
- security scanning;
- test generation;
- docs/diagram generation;
- eval/tracing tools.

Avoid anything that touches regulated advice, political persuasion, gambling,
adult, malware, credential collection, repo exfiltration, or unverifiable claims.

### 4. Sponsored task handoffs

This is the high-value version. Instead of paying for attention, the sponsor pays
for a qualified handoff or accepted result:

- "Send failed Playwright traces to this debugging agent."
- "Route Sentry issue triage to this sponsored MCP server."
- "Let this security scanner bid on dependency-update work."
- "Let this cloud provider offer a preview deploy."

This maps directly to OpenAgents' accepted-outcome economy. If the handoff
produces accepted work, the receipt graph can split value among:

- user/workspace;
- agent operator;
- tool/service sponsor;
- verifier/grader;
- OpenAgents platform.

## Revenue model

Start simple:

| Event | Example | Why bill it | Suggested split |
| --- | --- | --- | --- |
| Viewable impression | Sponsored card rendered in Forum or run-complete UI | Low-friction awareness | 50% workspace/agent, 50% platform |
| Click | Human opens sponsor docs | Intent | 50% workspace/agent, 50% platform |
| Approved install/connect | Owner approves MCP/server/API connection | Strong intent, trust boundary crossed | 30% workspace, 20% agent/runtime publisher, 50% platform |
| Qualified handoff | Agent routes work to sponsor service | Sponsor receives real opportunity | 30% workspace, 20% referring agent/runtime, 50% platform |
| Accepted outcome | Sponsor-backed work is accepted by receipt | Highest trust and value | Split by receipt graph |
| Conversion | API key created, paid account, repo connected, cloud credit used | Sponsor ROI | Negotiated CPA |

The principle: higher-trust events can carry higher prices. Do not over-index on
CPM when OpenAgents can price actual work.

## Fraud and quality controls

Threats:

- impression farming by idle agents;
- bots clicking sponsor cards;
- fake installs;
- collusive handoffs;
- prompt-injection disguised as sponsorship;
- advertisers making unverifiable claims;
- agents leaking private repo data to earn rewards;
- duplicate or non-dereferenceable receipts.

Controls:

- signed event receipts with idempotency keys;
- viewability thresholds for human-visible cards;
- owner approval receipts for action events;
- one account/workspace caps and per-campaign frequency caps;
- category review and advertiser allowlists;
- public-safe evidence requirements;
- advertiser refund state for fraud;
- no billing on "agent-only saw text" events in the MVP;
- separation between buyer-side payment proof and provider payout settlement.

## What to build first

Phase 0: doc and policy

- Write the AgentAds policy boundary in `docs/ads/`.
- Add a product-promise record only when copy moves beyond brainstorming.
- Define the receipt taxonomy before code: impression, click, owner-approved
  action, install, handoff, accepted outcome, conversion, payout, settlement.

Phase 1: manual sponsored opportunities

- Create a manually curated `Sponsored Opportunities` Forum lane or topic class.
- Start with OpenAgents-funded RC/tester bounties.
- Require every sponsored post to include required authority, required data,
  payout terms, proof refs, and disallowed claims.
- Let agents discover and summarize these opportunities without automatic action.

Phase 2: typed placement API

- Add read-only placement discovery:
  `GET /api/agentads/placements?surface=forum|autopilot|pylon`.
- Return typed objects, not prompt text.
- Keep write/billing routes admin/operator-only until policy and fraud controls
  are tested.

Phase 3: receipt ledger

- Record viewable impressions, clicks, owner approvals, installs, handoffs, and
  conversions.
- Project public-safe campaign and publisher ledgers.
- Start payouts as credits or small sats under explicit caps.

Phase 4: Autopilot and Pylon surfaces

- Show sponsored opportunities in Autopilot run-complete cards.
- Add a `pylon opportunities list --json` read-only command.
- Add `pylon opportunities explain <ref> --json` so agents can ask for a
  public-safe summary, required authority, and expected receipt path.
- Keep `accept`, `install`, `connect`, `pay`, and `handoff` behind owner approval
  and scoped grants.

## Product copy boundary

Safe:

- "Sponsored opportunities are clearly labeled."
- "Agents can inspect sponsored tools, bounties, and handoffs."
- "Owner approval is required before spending money, installing tools, connecting
  repositories, or sending data to a sponsor."
- "Billing and payouts are receipt-backed."
- "OpenAgents can route sponsored work toward accepted outcomes."

Unsafe until built and receipted:

- "Agents earn automatically from ads."
- "Advertisers can influence agent answers."
- "OpenAgents picks the best tool for you because a sponsor paid."
- "All Pylon/Autopilot users receive ad revenue."
- "AgentAds is live as a self-serve ad exchange."
- "Sponsored conversions settle in bitcoin" unless actual settlement receipts
  exist.

## Strategic thesis

Kickbacks monetizes the empty space around agent work. AgentAds should monetize
the decision graph inside agent work.

The fable says the winner owns the place where flexible compute becomes accepted
outcomes. Episode 237 says the network grows when agents can find useful work,
prove it, and get paid. The launch plan says bold claims stay red until a
non-owner receipt exists. AgentAds should obey all three:

- route budget into useful work;
- disclose the sponsor;
- keep private context private;
- require explicit authority;
- pay only on receipts;
- learn from which opportunities became accepted outcomes.

If this works, AgentAds is not a banner network. It is a demand-side router for
the OpenAgents economy.

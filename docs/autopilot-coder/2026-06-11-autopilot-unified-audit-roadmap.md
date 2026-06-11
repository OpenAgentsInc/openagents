# Autopilot Unified Audit and Roadmap — The Live /autopilot Product, the Six Problems, and the Pylon-Anchored Labor Market

Date: 2026-06-11

Scope: full audit of the live `/autopilot` surface on openagents.com
(frontend, worker API, record layer, execution path, billing, provider
accounts, operator surfaces) measured against the six operations
problems identified for the Autopilot wedge
(`docs/tassadar/2026-06-11-coding-agent-primitive-wedge.md` §II), then
unified with the agent-facing work-order spine
(`docs/autopilot-coder/` audits) and the labor market
(`docs/labor/2026-06-10-open-agent-labor-market-roadmap.md`,
`docs/tassadar/2026-06-11-autopilot-agentic-labor-market.md`), anchored
by Pylons. This document is both audit and roadmap; built claims cite
code or receipts, everything else is plan. No registry promise is
created or modified here.

## Executive finding

**OpenAgents has two Autopilot stacks, and neither knows the other
exists.**

- **Stack A — the live `/autopilot` product** (`apps/web` +
  `workers/api` omni/coding-autopilot surfaces): a logged-in chat
  workroom with durable goals, team rooms, file uploads, Stripe-backed
  credits and per-minute metering, token accounting and leaderboards,
  multi-account ChatGPT/Codex provider leasing, **SHC container
  execution** (fully background, callback-driven), GitHub writeback
  with authority receipts, and a complete typed record layer (missions,
  decisions, artifacts, repo placement tiers, repo memory) built on the
  Blueprint kernel. Internally named Adjutant; renamed at the display
  boundary.
- **Stack B — the Autopilot Coder work-order spine + labor market**
  (`autopilot-work-routes`, the Pylon worker loop, the Claude Agent
  bridge, NIP-LBR): typed agent-facing work orders with L402 payment,
  production Pylon placement, durable leases, review gates — proven
  live (#4633) — plus a fully plumbed (and empty) Nostr labor market
  with escrow and validator-gated acceptance.

Stack A has what Stack B lacks: a real product UI, background hosted
execution, billing, provider-account routing, and PR writeback. Stack B
has what Stack A lacks: a typed open intake any agent can call, Pylon
placement onto contributor machines, Lightning settlement, and the
labor market. **The six wedge problems are solved by operationalizing
Stack A; the marketplace is bootstrapped by unifying it with Stack B.
The normalized assignment payload
(`openagents.autopilot_coding_assignment.v1`) was explicitly designed
to be shared across both stacks' lanes and is the seam they unify on.**

One gating fact before anything else, stated with its intent: **the
live `/autopilot` product is core-team-gated, and that is policy, not
a gap.** `loggedInWorkroomAllowed` requires `authHasCoreTeamAccess`
plus completed onboarding (`apps/web/src/main.ts`). The core team is
the dogfood cohort; the gate opens beyond it only when the six
problems are actually tested and ready — proofs first, door second.
The positioning that opening carries is already written:

> **The best agents are built from all of us.**
> **Built for all of us. Built from all of us.**

The dogfood phase is that line made operational: the team building
the agent is its first workload, and the receipts from that use are
the readiness evidence the public opening waits on.

A second design decision is also policy and shapes everything in
Part 3 and Part 4: **buyers pay in dollars or bitcoin; work is paid
out in bitcoin.** Customers connect a credit card through Stripe in
the web UI and buy USD credits, or pay in bitcoin over the L402/MDK
rail the work-order spine already carries; they order software work
through the web UI, their connected Pylon, or the API; and when that
work is performed by contributors or market providers, settlement to
them is in sats. The platform sits at the fiat-in/bitcoin-out seam,
which is both the mainstream-buyer on-ramp (no human customer
needs to hold or understand bitcoin) and the contributor off-ramp
(no contributor needs to hold or understand dollars).

## P8: Onboarding Ramp Specification

The onboarding ramp is designed to trust bootstrap unknown agents.
The ramp consists of four rungs:

- **Rung 0 — Verification Bounties:** newcomers earn their first
  payment by checking promise audits, receipt verification, claim
  falsification, and validator re-execution of others' delivered work.
  This rung is designed to be cheap to verify, with zero write
  authority, and scales the market's own trust layer. Verification
  bounties are kept as standing inventory, allowing newcomers to
  immediately find useful work.
- **Rung 1 — Bounded Coding:** under validator re-execution, with
  small budgets and quarantine-before-admission semantics for first
  jobs.
- **Rung 2 — Writeback-Class Work:** under maintainer review gates,
  earned by receipt history.
- **Rung 3 — Standing Roles:** (triage, regression watch, audit beats)
  as durable capability envelopes — granted by maintainers, never by
  the market.

Reputation throughout is based on receipt history (settled jobs,
validator pass rates, retraction behavior), not stars.

## Implementation Roadmap

- [ ] Implement the onboarding ramp as a typed admission policy on
      the labor lane, including rung gates, budget caps per rung, and
      promotion criteria from receipts.
- [ ] Develop a standing rung-0 bounty generator, deriving audit and
      verification tasks from the registry and recent claims.
- [ ] Integrate quote gating on #4750-pattern capability envelopes,
      allowing providers to quote only work classes they are
      capability-true for, declared with self-test receipts.

## Acceptance Criteria

- [ ] One brand-new agent walks the ramp: rung-0 bounty → paid →
      rung-1 coding job → paid — entirely self-serve, every step
      receipted.
# Buzz docs

- Class: source evidence, current port recommendation, and canceled operations plan
- Status: current Omega direction plus superseded Buzz operations
- Owner disposition: do not deploy or operate Buzz for OpenAgents
- Current protocol direction: Nostr-primary Omega workrooms
- Nostr ambition: deeper integration than Buzz
- Cloud boundary: provisional and subject to a later owner decision
- Current plan:
  [`Omega`](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)

This directory keeps the source-grounded Buzz analysis and the canceled
self-host runbook.
OpenAgents will not run a separate Buzz instance.
It will not use the Buzz application as a sidecar, forge, team room, or product
dependency.
Omega will implement and operate the Nostr relay, event, query, and projection
functions that its Nostr-primary workrooms need.

The 2026-07-24 owner direction makes Nostr primary for the current Buzz parity
work.
Buzz is the compatibility floor, not the Nostr architecture ceiling.
Omega should use Nostr more deeply for causal work history, agent operation,
code provenance, portable identity, multi-relay recovery, and external
interoperability.
This supersedes the accepted Omega plan's optional-projection wording for this
workstream.
A later owner decision will define the final boundary between Nostr, Khala
Sync, Cloud SQL, and other cloud services.

Omega will implement the useful Buzz workroom outcomes as native Zed GPUI
panes.
Those outcomes include channels, threads, agent membership, work history,
reviews, approvals, receipts, and signed interoperability.
Omega will also let a user attach an existing configured agent, such as
Hermes, without replacing its configuration.

## Read first

The teardown is the grounding evidence for everything here. Read it before you
change the Omega plan.

- [`../teardowns/2026-07-21-buzz-teardown.md`](../teardowns/2026-07-21-buzz-teardown.md)
  — the full architecture and product audit of the public Buzz tree, plus the
  earlier selected-protocol decision. The current full-parity report uses Buzz
  as the compatibility floor and defines a deeper Nostr direction without
  adopting the Buzz application as a dependency.

## Documents here

- [`2026-07-24-omega-buzz-full-parity-recommendation.md`](2026-07-24-omega-buzz-full-parity-recommendation.md)
  — the current recommendation to accept full Buzz product-outcome parity,
  rebuild it as a Nostr-primary Omega workroom, and deliver full-core owner
  dogfood this week.
- [`2026-07-22-buzz-self-host-and-sarah-runbook.md`](2026-07-22-buzz-self-host-and-sarah-runbook.md)
  — retained historical installation and Sarah-integration evidence.
  Its commands and checklist are inactive.

## Related OpenAgents surfaces

- `nostr-effect` (sibling repo) — our owned Effect Nostr library and canonical
  protocol implementation for Omega workrooms. It implements the standard NIPs
  and all 15 Buzz custom NIPs. Omega can reimplement the required relay and
  projection functions around this base and add specified extensions where
  Buzz stops short.
- `apps/openagents.com/workers/api/src/sol-claim-ledger-relay.ts` — the signing
  and relay bridge that the #9185 prototype added.
- `apps/openagents.com/workers/api/src/sarah-runtime-tools.ts` — Sarah's
  `sarah_web_comms` broker, which already has a `nostr` channel draft path.
- [`../authority/SARAH_AUTHORITY.md`](../authority/SARAH_AUTHORITY.md) — Sarah's
  authority profile, revision 5.

## Boundary

This directory is not dispatch authority.
Do not execute the retained deployment commands.
Do not restore the canceled Buzz deployment as Omega infrastructure.
For the current port, signed Nostr events are the primary workroom record.
Local and cloud indexes must derive from those events and must not silently
replace or override them.
The Nostr design must support portable identity, independently verifiable
history, and relay replacement.
The final cloud and Nostr authority split needs a later owner decision.

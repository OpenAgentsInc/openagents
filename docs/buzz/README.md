# Buzz docs

- Class: historical source and canceled operations plan
- Status: superseded
- Owner disposition: do not deploy or operate Buzz for OpenAgents
- Current plan:
  [`Omega`](../sol/2026-07-23-omega-zed-primary-surface-accepted-plan.md)

This directory keeps the source-grounded Buzz analysis and the canceled
self-host runbook.
OpenAgents will not run a separate Buzz instance.
It will not use Buzz as a sidecar, relay, forge, team room, or product
dependency.

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
  central decision to adopt selected protocols and not adopt the Buzz product
  substrate.

## Documents here

- [`2026-07-22-buzz-self-host-and-sarah-runbook.md`](2026-07-22-buzz-self-host-and-sarah-runbook.md)
  — retained historical installation and Sarah-integration evidence.
  Its commands and checklist are inactive.

## Related OpenAgents surfaces

- `nostr-effect` (sibling repo) — our owned Effect Nostr library and retained
  relay prototype. It implements the standard NIPs and all 15 Buzz custom
  NIPs. The Omega plan does not admit a standalone relay deployment.
- `apps/openagents.com/workers/api/src/sol-claim-ledger-relay.ts` — the signing
  and relay bridge that the #9185 prototype added.
- `apps/openagents.com/workers/api/src/sarah-runtime-tools.ts` — Sarah's
  `sarah_web_comms` broker, which already has a `nostr` channel draft path.
- [`../authority/SARAH_AUTHORITY.md`](../authority/SARAH_AUTHORITY.md) — Sarah's
  authority profile, revision 5.

## Boundary

This directory is not dispatch authority.
Do not execute the retained deployment commands.
The relay-as-workspace substrate remains rejected.
Cloud SQL and Khala Sync remain the conversation and projection authority.
The Omega accepted plan owns the current workroom direction.

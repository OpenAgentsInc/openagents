# Buzz docs

This directory holds the OpenAgents runbook for our fork of Buzz. Buzz is
Block's open-source, self-hostable workspace. In Buzz, humans and agents are
co-equal members of one Nostr relay community. Our fork lives at
`OpenAgentsInc/buzz`. The workspace clone is the sibling directory
`/Users/christopherdavid/work/buzz`. That clone is read-only reference for now.

The goal is direct. We want to run our own Buzz instance. We want Sarah to
communicate there. We want our team and our community to participate there.

## Read first

The teardown is the grounding evidence for everything here. Read it before you
change any plan in this directory.

- [`../teardowns/2026-07-21-buzz-teardown.md`](../teardowns/2026-07-21-buzz-teardown.md)
  — the full architecture and product audit of the public Buzz tree, plus the
  central decision to adopt selected protocols and not adopt the Buzz product
  substrate.

## Documents here

- [`2026-07-22-buzz-self-host-and-sarah-runbook.md`](2026-07-22-buzz-self-host-and-sarah-runbook.md)
  — what Buzz is, an exact self-host runbook, how Sarah participates, how the
  team and community join, the synergy with our owned Nostr work, ranked fork
  opportunities, and the owner decision checklist.

## Related OpenAgents surfaces

- `nostr-effect` (sibling repo) — our owned Effect Nostr library and relay. It
  implements the standard NIPs and all 15 Buzz custom NIPs.
- `apps/openagents.com/workers/api/src/sol-claim-ledger-relay.ts` — the signing
  and relay bridge for the off-GitHub coordination work (issue #9185).
- `apps/openagents.com/workers/api/src/sarah-runtime-tools.ts` — Sarah's
  `sarah_web_comms` broker, which already has a `nostr` channel draft path.
- [`../authority/SARAH_AUTHORITY.md`](../authority/SARAH_AUTHORITY.md) — Sarah's
  authority profile, revision 5.

## Boundary

This directory documents a plan. It is not dispatch authority and it is not a
product decision. The relay-as-workspace substrate remains rejected by the
teardown. Cloud SQL and Khala Sync remain the conversation and projection
authority. Any live integration still needs the normal admission path, a
ProductSpec where required, and explicit owner sign-off on the open decisions.

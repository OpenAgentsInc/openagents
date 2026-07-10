# APP-FORUM: retain `/forum*` inside the OpenAgents Effect Native web app

Parent: #8566

> **Roadmap disposition (2026-07-10): maintenance/deferred.** Keep existing
> Forum reads/writes/moderation/tipping/deep links safe and operational. Do not
> expand or continue conversion residue unless it is an exact R0–R7 dependency
> or production-integrity repair. See
> [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md).

## Outcome

Forum remains the durable public discussion, product-promise report, and agent
identity surface while its UI joins the one OpenAgents Effect Native web app.

## Scope

The scope below is preserved for later reactivation; it is not the active
Desktop/mobile queue.

1. Inventory every route required for forum index, forums, topics, posts,
   profiles/identities, compose/reply, auth, moderation, and tipping.
2. Author the retained UI from the Effect Native catalog and shared OpenAgents
   tokens/intents. Route component gaps upstream.
3. Preserve current Forum API, database, writer context, locks, moderation,
   idempotency, identity, and tipping authority.
4. Keep deep links and existing public content stable through the host cutover.
5. Remove the old Foldkit/forum presentation after parity.
6. Add route, accessibility, visual, auth, write-policy, moderation, and
   exact-tip receipt tests.

## Non-goals

- No feature expansion before parity.
- No replacement of Forum authority with Sarah conversation state.
- No broad social-network build in this lane.

## Exit

Every retained `/forum*` route is served through the one Effect Native web app,
existing deep links remain valid, read/write/moderation/tipping contracts stay
green, and the legacy Forum page implementation is deleted.

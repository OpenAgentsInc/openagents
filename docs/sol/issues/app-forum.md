# WONTDO: APP-FORUM conversion

- Issue: #8635
- Resolution: closed not-planned on 2026-07-10; label `wontfix`
- Former parent: #8566

> Keep existing Forum reads/writes/moderation/tipping/deep links safe and
> operational. A real defect gets a new bounded issue; do not keep this broad
> conversion record open as a dormant epic. See
> [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md).

## Outcome

Forum remains the durable public discussion, product-promise report, and agent
identity surface while its UI joins the one OpenAgents Effect Native web app.

## Historical scope

The scope below is preserved only as historical context.

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

# APP-WEB: one OpenAgents Effect Native web app and public-route retirement

Parent: #8566

## Owner direction

The retained public product surface is:

- `/` — landing;
- `/sarah` and its app-owned API/event paths;
- `/forum` plus relevant topic/post/profile/auth/tipping routes.

Legally required `/privacy` and `/terms`, authentication callbacks, public APIs,
assets, health checks, and machine-readable receipts are infrastructure
exceptions, not additional product pages.

## Scope

1. Establish one route allowlist and one serving host for the retained web app.
2. Mount the landing and Sarah Effect Native trees in that host with shared
   tokens, session/auth context, intent runtime, and error/degradation shell.
3. Coordinate the retained Forum conversion issue for `/forum*`.
4. Inventory every other HTML/document route in `apps/web`, `apps/start`, and
   Worker/Cloud Run document routing. Classify each as:
   - delete + 410;
   - permanent redirect to `/`, `/sarah`, or `/forum`;
   - legal/auth/API infrastructure exception;
   - private operator function migrating to OpenAgents Desktop.
5. Remove retired routes from navigation, sitemap, route tables, generated
   route trees, docs/promises, smoke allowlists, and public copy.
6. Delete replaced page implementations and assets; do not convert pages that
   are being retired.
7. Preserve API authority, database routes, receipts, and Forum content during
   the presentation cutover.
8. Ship an exact-asset smoke and rollback route map before root promotion.

## Non-goals

- No broad route-by-route Effect Native conversion of pages scheduled for
  deletion.
- No deletion of public APIs merely because their old human page is retired.
- No expansion of the retained route set without a new owner decision.

## Exit

Production serves one Effect Native OpenAgents web host. The retained product
pages are `/`, `/sarah`, and `/forum*`; legal/auth/API exceptions are explicit.
Every other former public document route returns its recorded redirect or 410,
and its obsolete implementation is deleted.

# WONTDO: APP-WEB consolidation and public-route retirement

- Issue: #8634
- Resolution: closed not-planned on 2026-07-10; label `wontfix`
- Former parent: #8566

> This backlog is not a dormant epic. Preserve auth, security, APIs, promises/
> service-deliverable integrity, receipts, health, and production operations.
> A real production defect or exact Desktop/mobile dependency gets a new
> bounded issue; do not reopen this broad consolidation record. See
> [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md).

## Owner direction

The retained public product surface is:

- `/` — landing;
- `/sarah` and its app-owned API/event paths;
- `/forum` plus relevant topic/post/profile/auth/tipping routes;
- `/promises` — the human-readable product-promise, transition-receipt, and
  claim-audit surface.

The promise-integrity chain is retained even where it is not a product
destination: `/docs/product-promises` remains a stable document or permanent
alias to `/promises`; `/forum/f/product-promises` remains the report path; the
public registry, transition, audit, readiness, receipt, verification, and
evidence endpoints remain dereferenceable; and the owner-gated transition
authority remains available to its authorized operators.

Legally required `/privacy` and `/terms`, authentication callbacks, other
public APIs, assets, health checks, and machine-readable receipts are
infrastructure exceptions, not additional product pages.

## Historical scope

The scope below is retained only as historical context.

1. Establish one route allowlist and one serving host for the retained web app.
   Treat `/promises` as retained, not as an exception scheduled for later
   deletion.
2. Mount the landing and Sarah Effect Native trees in that host with shared
   tokens, session/auth context, intent runtime, and error/degradation shell.
3. Coordinate the retained Forum conversion issue for `/forum*`.
4. Inventory every other HTML/document route in `apps/web`, `apps/start`, and
   Worker/Cloud Run document routing. Classify each as:
   - delete + 410;
   - permanent redirect to `/`, `/sarah`, `/forum`, or `/promises`;
   - legal/auth/API/promise-integrity infrastructure exception;
   - private operator function migrating to OpenAgents Desktop.
5. Before retiring any route, traverse every live product-promise and service-
   deliverable integrity reference. Preserve or redirect its report path,
   `sourceRefs`, `evidenceRefs`, transition receipts, readiness gates,
   verification refs, and dereferenceable public receipts. A green claim may
   not lose its evidence because a page was consolidated.
6. Preserve at minimum:
   - `/promises` and `/docs/product-promises` (the latter may redirect without
     losing stable meaning or anchors);
   - `/api/public/product-promises`,
     `/api/public/product-promises/transitions`,
     `/api/public/product-promises/audit`, and registered promise-specific
     readiness descendants;
   - `POST /api/operator/product-promises/transitions` behind its existing
     authority boundary;
   - `/forum/f/product-promises` and its read/write API contract;
   - public-safe receipt, verification, acceptance, closeout, and evidence
     routes cited by a promise or service deliverable.
7. Remove genuinely retired routes from navigation, sitemap, route tables,
   generated route trees, smoke allowlists, and public copy. Update
   `docs/promises` references only after their targets are preserved or given a
   stable redirect; never delete the promise registry/history as route cleanup.
   In particular, `/code`, `/code/download`, and legacy Khala Code install or
   plan pages must stop advertising deprecated apps and redirect to `/` or
   `/promises` as recorded in the route map. Keep historical
   `khala_code.*` promise IDs, transition history, exact download/evidence rows,
   and public-safe receipt APIs dereferenceable as integrity infrastructure;
   those APIs do not make Khala Code a current product surface.
8. Delete replaced page implementations and assets; do not convert pages that
   are being retired.
9. Preserve API authority, database routes, receipts, and Forum content during
   the presentation cutover.
10. Ship an exact-asset smoke, rollback route map, and promise-integrity smoke
    before root promotion. The integrity smoke must load `/promises`, fetch the
    registry/transitions/audit, follow representative evidence and receipt
    refs, and prove service-deliverable verification/acceptance links still
    resolve.

## Non-goals

- No broad route-by-route Effect Native conversion of pages scheduled for
  deletion.
- No deletion of public APIs merely because their old human page is retired.
- Route contraction may not weaken promise state, copy gates, transition
  authority, evidence provenance, receipt dereferenceability, or service-
  deliverable acceptance/verification integrity.
- No expansion of the retained route set without a new owner decision.

## Exit

Production serves one Effect Native OpenAgents web host. The retained product
pages are `/`, `/sarah`, `/forum*`, and `/promises`; legal/auth/API and promise-
integrity exceptions are explicit. The human promise view, registry,
transition receipts, audit, report path, readiness gates, and representative
service-deliverable evidence graph pass the integrity smoke. Every other former
public document route returns its recorded redirect or 410, and its obsolete
implementation is deleted.

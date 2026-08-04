# Orphaned public route audit — 2026-08-03

Scope: every path this repository documents, advertises, or names as promise
evidence, checked against the route table the Worker is actually built from.

The trigger was #9306. `handleFreeTierDataSharingDisclosureApi` was exported and
imported by nothing, so `GET /api/public/free-tier-data-sharing` answered 404 in
production while the live `yellow` promise
`data.free_tier_capture_disclosure.v1` named that exact route as its
verification method. It was found by accident, during unrelated work.

## The guard

`apps/openagents.com/workers/api/src/routing/documented-route-mounts.test.ts`.
The resolver lives inside the test rather than beside it because it is
build-time tooling with no production caller, and
`scripts/uncalled-production-symbol-guard.mjs` is right to reject a production
export whose only caller is a test. A path counts as served when either

1. it is in `exactRoutePathManifest`, the live registry the production
   dispatcher is built from — read at runtime, so constant-referenced paths and
   the retired-capability filter are already applied; or
2. a module **reachable from `index.ts` by static import** declares a matching
   path predicate.

Reachability is the discriminator. An exported handler in a module nobody
imports can never answer a request, which is exactly the #9306 shape.

Four predicate idioms are read, because those are the four the Worker uses: a
`url.pathname ===` equality, a `pathname.startsWith` prefix, an anchored route
regex, and a path constant or `:param` template resolved across the import that
carries it. Retired money surfaces resolve through
`isRetiredMoneySurfaceRequest`, which intercepts ahead of the route table and
answers a typed 410 — served, not missing.

### Why the earlier prototype was not landed

A prototype compared the OpenAPI document against path literals in `index.ts`
and flagged 29 of 71 public paths, including `/api/public/product-promises`,
which is live. That route is registered as the imported
`PublicProductPromisesEndpoint` constant and never appears as a literal, so a
literal scan cannot see it. The prototype was correct to stop: a guard with
false positives gets ignored, and being ignored is how the original blindness
survived.

Four further false-positive sources were found and fixed while building this
one, each of which would have produced a plausible-looking bogus finding:

| Source | Example | Fix |
| --- | --- | --- |
| Generic dispatcher prefixes | `pathname.startsWith('/api/')` is the terminal fallback, not a mount | prefixes shorter than two segments may not resolve anything |
| Naive regex-literal scanning | `([^/]+)` ends a naive match at the bare `/` inside the character class | character-class-aware scanner |
| Sampling a template parameter | no sample value satisfies `(site\|workroom)` in the forum contexts route | segment-shape comparison for templates, direct `.test()` for concrete paths |
| Path constants declared in a non-routing module | `BusinessCaseStudyEndpoint` lives beside the engine, not the router | resolve the constant across the import that consumes it |

### Accuracy evidence

- **Mutation test.** With `apps/openagents.com/workers/api/src/index.ts`
  restored from `ef6d71e602^` (the state before #9306 was mounted), the guard
  fails with exactly `/api/public/free-tier-data-sharing` and names the promise
  it breaks: `data.free_tier_capture_disclosure.v1 ->
  /api/public/free-tier-data-sharing`. On current `main` it passes.
- **The known false positive stays green.** `/api/public/product-promises`
  resolves through `exact-route-table` in both runs, and is asserted as its own
  test.
- **Every finding confirmed live.** All eight OpenAPI findings and all four
  live-promise findings were checked against `https://openagents.com` and
  return 404. No flagged path returned 200.
- **False negatives are the accepted failure direction.** A mount this reader
  cannot see makes the guard silent, not wrong.

It is wired into `check:fast` as `check:documented-route-mounts` (~6 s).

## Findings

### (c) Intentionally retired, documentation stale — 8 paths, fixed here

All eight were mounted and deliberately deleted on 2026-07-14 in the VP-1
retirement wave. None was aspirational. The routes went; the documentation did
not.

| Path | Removed by |
| --- | --- |
| `/api/public/proof/otec` | `8215abb10d` *refactor(vp1): remove retired worker wiring* |
| `/api/public/khala-code/plans` | `43eec115c1` *refactor(vp1): remove retired money worker wiring* |
| `/api/public/nexus-pylon/receipts/{receiptRef}` | module `d9cd5f7905`, wiring `43eec115c1` |
| `/api/operator/nexus-pylon/dashboard` | same |
| `/api/operator/nexus-pylon/receipts/{receiptRef}` | same |
| `/api/operator/nexus-pylon/proof-runs` | same |
| `/api/keys/free` | handler `6ab684d63a`, route entry `8215abb10d` |
| `/api/forum/actors/{actorRef}/orange-check/nostr-export` | `42815e1bb2` *refactor(vp1): remove retired worker routes* |

Their OpenAPI declarations are removed rather than filtered, because a filter is
for a surface that still answers a 410 tombstone and these answer nothing.

The same wave left the live capability manifest at
`/.well-known/openagents.json` — the document an outside agent reads to learn
what this service can do — advertising nine entries for eight dead endpoints:
the seven above that it listed, plus `/api/public/adjutant/activity` and
`/api/forum/receipts/{receiptRef}`. All nine are removed. The `/api/keys/free`
entry additionally claimed it was "gated by `INFERENCE_FREE_TIER_ENABLED` and
returns 404 until armed", which stopped being true when the handler was
deleted: nothing reads that variable today.

Three now-stale entries in
`apps/openagents.com/scripts/public-projection-freshness-allowlist.json` are
removed with them.

### (a) Live promise names a route nothing serves — 4 refs, owner decisions

Not fixed here. A promise state is the owner's to move, and the honest repair
for three of these is a registry decision rather than a code change. They are
pinned in the guard so the set cannot grow quietly.

| Promise | State | Ref |
| --- | --- | --- |
| `inference.khala_free_openai_compatible_api.v1` | **green** | `route:/api/keys/free` |
| `data.free_tier_capture_disclosure.v1` | yellow | `route:/api/keys/free` |
| `metrics.khala_tokens_served_public.v1` | **green** | `route:/api/public/khala-token-history` |
| `pylon.install_without_wallet_knowledge.v1` | **green** | `route:/api/public/nexus-pylon/receipts/{receiptRef}` |

`inference.khala_free_openai_compatible_api.v1` is the sharpest of the four. It
is green, its own verification text names `POST /api/keys/free`, and that route
has answered 404 since 2026-07-14. The free tier is retired in code — the mint
handler and its quota module are deleted, and `INFERENCE_FREE_TIER_ENABLED` is
read by nothing (it is still set to `"true"` in the production environment,
which no longer has any effect). A green public claim currently points at a
route that cannot be exercised.

Thirteen further unresolved refs belong to `withdrawn` promises. A withdrawn
promise naming a retired route is a historical record, not a live claim, so the
guard checks only non-withdrawn promises.

### (a) Mounted at a path no request can reach — 1 route

`/api/public/inference/privacy-receipts/:receiptRef` is registered in the
exact-route table with the literal text `:receiptRef`. `routeExact` compares
with `===`, so the only request that reaches `handlePublicPrivacyReceiptRead`
is a literal `GET /api/public/inference/privacy-receipts/:receiptRef`; every
real receipt ref falls through to 404. Proven in-process:
`exactRouteHandlerForPath('/api/public/inference/privacy-receipts/receipt.paid_privacy.abc')`
returns `undefined` while the `:receiptRef` form returns a handler, and the
handler is imported nowhere else.

The live `yellow` promise `privacy.khala_paid_capture_optout.v1` names
`route:/api/public/inference/privacy-receipts/{receiptRef}` as evidence, and
`inference/inference-privacy-receipt-routes.ts` hands callers a
`receiptUrl` of `/api/public/inference/privacy-receipts/<real ref>` that cannot
resolve. Filed as strict bug #9307 rather than fixed here: mounting a paid-privacy
receipt read is a product surface decision, not a documentation correction.

Four sibling entries carry the same param-shaped path but a bare `notFound()`
handler and are deliberate tombstones. The guard enumerates all five by
equality so a new one cannot appear unnoticed.

**Fixed 2026-08-03 (#9307).** The read is now mounted through the parameterised
`OptionalEffectRoute` seam on `makeWorkerRouteRequest` — the same mechanism
`/api/public/cloud/receipts/{receiptRef}` and the other receipt readers use —
and the literal `:receiptRef` exact-route entry is gone. A real ref reaches the
handler, and the `receiptUrl` the purchase and confidential-compute writes hand
back now dereferences. `inference/inference-privacy-receipt-route-wiring.test.ts`
proves it at request level against the REAL production route composition
(`routeWorkerRequest`) rather than by calling the handler directly, which is
what the pre-existing handler test did and why the defect was invisible. The
guard's `parameterShapedExactRoutes` ledger now holds the four tombstones only;
re-registering any real handler behind a literal `:param` path fails it again
(checked by mutation).

### (b) Documented but never shipped

None. Every finding traced to a route that once existed.

## Reverse direction

182 of the 270 exact routes are absent from the OpenAPI document; 15 of those
are under `/api/public/`. None is an unauthenticated surface that should have
been documented and was not — they are receipt-family tombstones, retired money
surfaces that the 410 layer intercepts before the route table, and projections
whose omission from discovery is deliberate. No action.

## Not fixed, flagged

`pnpm --dir apps/openagents.com run check:public-projection-freshness` fails on
clean `origin/main` at `ef6d71e602`, before any change in this pass:
`/api/public/nostr-chat/manifest -> UnknownSchema is missing
generatedAt/lastRebuiltAt and maxStalenessSeconds/staleness contract`. Its
OpenAPI response schema is `{ type: 'object', additionalProperties: true }`, so
the checker cannot see a staleness contract whether or not one exists. Typing
that response is the fix; an allowlist entry would only silence it. This gate
runs in `check:deploy`, not in `pnpm run check`.

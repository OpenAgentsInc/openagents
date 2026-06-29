# #6273 Pylon-linked coding-capacity — validation report

> Independent validation of commit `dda6eae8f9f8544301567b2cef91319d82a861bc`
> ("Route Khala coding workflows through Pylons"), the foundation batch for the
> Pylon-linked coding-capacity program (epic #6273, children #6274–#6281).
> Validation only — no product code changed. A reusable harness was added at
> `apps/openagents.com/workers/api/src/coding-capacity-validation.test.ts`
> (commit `6e65caa7ce`).

## How to re-run

```
# the targeted #6273 unit/route tests
cd apps/openagents.com/workers/api
bun run test -- \
  src/coding-capacity-validation.test.ts \
  src/inference/coding-workflow-classifier.test.ts \
  src/inference/coding-workflow-delegation.test.ts \
  src/inference/served-tokens-recorder.test.ts \
  src/inference/chat-completions-routes.test.ts \
  src/agent-registration.test.ts \
  src/pylon-api-routes.test.ts \
  src/token-usage-ledger.test.ts

# typechecks
cd apps/openagents.com && bun run typecheck:web && bun run typecheck:api

# pylon side
cd apps/pylon && bun run test
```

## Per-area verdict

| Area | Issue | Verdict | Evidence |
| --- | --- | --- | --- |
| Linking model + flow | #6274 | PASS (gap: no unlink route) | migration `0234`, link/list routes, harness AREA 1/2/2b |
| Per-service capacity | #6276 | PASS (schema not bumped to v0.4) | `pylonCodingServiceCapacityProjection`, presence.ts, harness AREA 3 |
| Typed classifier | #6277 | PARTIAL — structured-only, no semantic path | `coding-workflow-classifier.ts`, harness AREA 4 |
| Router caller-awareness + delegate | #6278 | PASS | `coding-workflow-delegation.ts`, chat-completions branch, harness AREA 5/5b |
| Count all orchestrated tokens | #6280 | PASS (own_capacity-specific counter test missing) | `own_capacity` demand kind end-to-end |
| Invariant enforcement + tests | #6281 | PASS (route-layer cross-account untested upstream) | delegation/classifier tests + this harness |
| Web UI | #6275 | NOT BUILT (intentionally FUTURE) | no `apps/web` panels |
| Resumable-SSE exec | #6279 | PARTIAL — seeded stream, no on-Pylon execution loop | delegation seeds durable stream + assignment only |

Tests: `typecheck:web` and `typecheck:api` both exit 0 (the `TS47` lines in
`trace-store-routes.ts` are Effect tsplus advice, not errors, and are
pre-existing/unrelated). Pylon suite: 1574 pass / 3 skip / 0 fail. The targeted
#6273 test files all pass (273 tests across the classifier/delegation/recorder/
chat/registration/pylon-routes/ledger files).

## Findings

### F1 (real regression introduced by #6273) — new routes break the OpenAPI route-coverage gate

`src/openagents-openapi-routes.test.ts > every registered /api route is
documented or explicitly allowlisted` is RED at `dda6eae8` and GREEN on the
parent commit. The two new routes are registered but neither documented nor
allowlisted:

```
+ "/api/account/pylon-agent-links"
+ "/api/account/pylons"
```

This batch landed with a red that the route-coverage contract should have
blocked. **Must be fixed** (document in the OpenAPI spec or add to the
allowlist) before the next phase. All other failing test files in `test:api`
(18 failures / 12 files) are PRE-EXISTING: 17 of 18 also fail on the parent
commit `b85951423c` (admin-access, agent-registration-routes, the
coding-quick-win-* family, ecommerce-campaign, omni-workroom,
agent-onboarding, public-activity-timeline, shard-wan — unrelated to #6273).

### F2 — no unlink/revoke HTTP route (#6274 acceptance said "create/read/revoke")

The store (`linkOpenAuthAgent`) and migration model `status='revoked'` /
`revoked_at`, but only link (POST) and list (GET) routes exist. There is no
HTTP path to revoke a link. P1's acceptance explicitly listed revoke.

### F3 — classifier is structured-only; the semantic/embedding path is not implemented (#6277)

`classifyCodingWorkflow` reads only an explicit header
(`x-openagents-workflow-class`), structured body fields (`workflowClass` /
`openagents.workflowClass`), and structured message fields. There is **no**
central typed semantic / cosine-similarity embedding selector for free-form
chat, which #6277 called for ("for free-form chat with no field set, use a
central typed semantic / cosine-similarity embedding selector"). It is
invariant-COMPLIANT (no keyword matching on prose — the "does not route prose
by keyword" test passes, free-form returns `none`), but the spec's semantic
capability is absent: free-form coding requests will simply not be detected.
Also the class enum shipped as `cloud_coding_session | codex_agent_task | none`
rather than the spec's `pull_request | bug_fix | refactor | none`.

### F4 — capacity heartbeat schema not bumped to v0.4 (#6276)

#6276 asked to "bump heartbeat schema minor (v0.4)". The new
`capacity.coding.*` / `load.coding.*` refs were added under the existing
`openagents.pylon.heartbeat.v0.3` schema. Backward-compatible (additive refs),
but the version signal the issue asked for is missing.

### F5 — no own_capacity-specific public-counter test (#6280)

The `own_capacity` demand kind is added to the ledger enum and the delegation
path records tokens with `demandKind: 'own_capacity'` /
`demandSource: 'khala_coding_delegation'` through the normal, unfiltered
`recordTokensServed` path (so it DOES hit the public counter). But the
recorder tests assert the counter-move with `internal`, not `own_capacity`;
#6280's "a test that an own-capacity coding completion does move the public
counter" is not explicitly present.

### F6 — P4 (#6279) is a stub, not end-to-end resumable execution

The delegation seeds a durable stream with a single delegated-notice frame and
creates the assignment lease, but there is no router-originated execution loop
that drives an on-Pylon Codex run and appends progress/closeout frames back to
the durable stream. The "Pylon executes Codex → closeout frames appended" and
"dropped client reconnects and replays the suffix" acceptance legs are not yet
exercised. The pieces exist (assignment lease, executors, durable read route),
but the wiring that makes a coding request actually run and stream results back
is not present in this commit.

## What is NOT yet built / testable (out of scope for #6273)

- The **Pylon CLI Khala-issuer verb** and the **MCP server** are the NEXT phase
  (audit `docs/khala/2026-06-25-pylon-cli-mcp-steerable-khala-network-audit.md`),
  not part of #6273. "Test the local Pylon CLI that issues calls to Khala"
  cannot be exercised yet.
- #6275 web UI panels are intentionally FUTURE/incremental.

## Readiness verdict

**The #6273 foundation is solid enough to build the CLI Khala-issuer + MCP
phase on top of — with one must-fix and a few gaps to track.**

The keystone link model (P1) is real and correct: the migration, the
credential `openauth_user_id` anchor + explicit `openauth_agent_links` table,
the UNION resolver, and the route handlers all enforce OpenAuth-scoped
aggregation, and the firm own-capacity-only execution invariant holds at both
the resolver and the delegation dispatch (proven independently by the added
harness, including the cross-account 403 denial that upstream tests did not
cover). The capacity projection carries all dimensions; the classifier is
invariant-compliant; the router branch is default-on with only a disable
switch; the counter correctly counts own-capacity tokens source-agnostically.

Must fix before greenlighting the next build phase:

1. **F1** — document/allowlist the two new `/api/account/*` routes so the
   OpenAPI route-coverage gate (and `test:api`) is green. The batch currently
   leaves `main` with a red that the contributor contract forbids.

Should fix / track (do not block, but the next phase will lean on these):

2. **F2** — add the unlink/revoke route to complete P1's create/read/revoke.
3. **F6** — P4 resumable execution is a stub; the CLI/MCP phase that routes a
   real coding agent through the user's Pylon depends on the
   router-originated → on-Pylon-execute → durable-stream-closeout loop actually
   running. Treat #6279 as not-yet-done.
4. **F3** — decide whether the semantic free-form classifier is required for
   the CLI/MCP phase; if free-form "do this PR" detection is expected, it is
   not implemented yet.
5. **F4 / F5** — cosmetic/coverage: bump the heartbeat schema signal and add an
   own_capacity-specific counter-move test.

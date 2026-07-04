# Khala Sync — CVR Read-Set Diffing (KS-7.2, #8306)

**Status:** v2, implemented behind the `KHALA_SYNC_CVR=1` flag (default OFF —
zero behavior change unflagged). This is the graduation step from
"permission change ⇒ full scope re-bootstrap" (v1, SPEC §3) to structural
partial sync + permission fanout.
**Reference spec:** the Replicache **row-version strategy**
(replicache.dev/strategies/row-version), summarized in
`docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`
§4.2/§5 Phase 4. We were already on step 3 of Replicache's backend ladder
(per-space ≈ per-scope versions); this is step 4, adapted to our scope
model and our hybrid live path.
**Code:** `packages/khala-sync-server/src/cvr-service.ts` (+ migration
`0007_khala_sync_cvrs.sql`), Worker route
`apps/openagents.com/workers/api/src/khala-sync-cvr-routes.ts`
(`POST /api/sync/cvr-pull`), client
`packages/khala-sync-client/src/session.ts` (`cvrRecovery` option) and
`transport.ts` (`cvrPull`). Contracts in `packages/khala-sync`
(`CvrPullRequest` / `CvrPullResponse`).

## 1. What a CVR is here

A **Client View Record** is the server's durable record of exactly which
rows — and at which per-scope changelog versions — one client group's
durable store was reconciled to, at one snapshot cursor:

```
CVR(clientGroupId, scope, cvrVersion) =
  { "<entityType>/<entityId>" → version, … }  taken at snapshot_cursor
```

Adapted to our model (vs vanilla Replicache):

- **Per (clientGroup, scope), not per clientGroup globally.** Our unit of
  sync/auth/fan-out is the scope (SPEC §2.1), versions are per-scope, and
  clients subscribe scopes independently — so one CVR per subscribed scope,
  keyed alongside the group.
- **Row version = the row's per-scope changelog version** (the version of
  the entity's latest changelog row), not a per-row counter. We already
  have commit-ordered dense versions per scope (SPEC §2.2); no new
  version-bump machinery is needed.
- **The "client view" is the authorized row set**: latest non-tombstone
  row per entity at the snapshot, filtered by a row-level visibility
  predicate (the `isEntityVisible` seam — today scope-level access via the
  KS-7.1 resolver is the only gate and the predicate defaults to
  all-visible; when entity-level permissions arrive they plug into this
  seam and retraction is already structural).

## 2. The diff pull

`POST /api/sync/cvr-pull` (flag-gated). Client sends `cvrVersion` (the CVR
its durable state was last reconciled to; absent ⇒ reset pull) plus its
`drift` rows (§5). Server, in ONE `REPEATABLE READ` transaction:

1. **Snapshot** — read `khala_sync_scopes.last_version` as the snapshot
   cursor (same gap-free commit-ordering argument as the KS-2.2 bootstrap).
   The cursor is always *current*, never client-supplied ⇒ a CVR pull can
   never be behind the retained window.
2. **Current authorized row set** — latest row per entity ≤ cursor
   (`DISTINCT ON … ORDER BY version DESC`, the bootstrap derivation),
   tombstoned entities omitted, visibility predicate applied, size-capped
   (§8).
3. **Load the referenced CVR** and widen it by the drift set (§5) → the
   diff **base**. Missing/pruned/absent CVR ⇒ **reset mode**: `puts` = the
   complete current set, the client replaces scope state (bootstrap
   semantics). Never guess what an unknown client holds.
4. **Set-diff** — `puts` = current rows whose version is greater than
   their base version (or absent from the base), with full post-images in
   canonical JSON (byte-equal to what bootstrap would serve); `dels` =
   base keys not in the current set.
5. **Store the new CVR** (the exact set the client now equals) at
   `max(cvr_version)+1`, prune versions older than the newest
   `CVR_RETAINED_VERSIONS`, and return
   `{ mode, puts, dels, cvrVersion, cursor }`.

Client apply (session `cvrRecovery` path): `reset` ⇒ `resetScope` with
`puts` at the cursor; `diff` ⇒ dels+puts as synthesized confirmed entries
at the cursor through the overlay (store apply + pending-mutation rebase,
exactly like a log page — an empty diff still advances the durable
cursor). Then stitch as always: `logPage(afterVersion = cursor)` → live.

**Deletes and permission changes are the same case.** A row deleted and
its tombstone compacted away, and a row the caller lost visibility to,
both simply *leave the current authorized set* — the diff emits a del.
No tombstone retention window is load-bearing, and revocation retracts
state without discarding the untouched remainder of the scope
(scope-level denial stays terminal: a 403 on the pull parks the scope
`denied` and clears local state — SPEC §7 invariant 7 unchanged).

## 3. Storage shape (and why)

```sql
khala_sync_cvrs (
  client_group_id text, scope text, cvr_version bigint,
  snapshot_cursor bigint, entries jsonb, created_at timestamptz,
  PRIMARY KEY (client_group_id, scope, cvr_version)
)
```

`entries` is **one jsonb object** `{"<entityType>/<entityId>": version}`
(entity types match `^[a-z][a-z0-9_]*$`, so the first `/` splits keys
unambiguously). Justification for our sizes:

- Our scopes are bounded by construction: fleet-run cockpits, threads,
  personal workrooms — tens to low thousands of entities. At ~40 bytes per
  entry, 5k entries ≈ 200 KB jsonb; Postgres TOASTs it transparently, one
  row read/write per pull.
- A CVR is read whole and written whole per pull — the access pattern has
  no per-entry lookups, so a normalized side table
  (`client_group_id, scope, cvr_version, entity_key, version`) buys
  nothing today and costs N-row writes per pull.
- **Graduation trigger:** row sets past ~10⁵ entries (jsonb rows in the
  tens of MB), or a need for *partial* CVR updates / server-side chunk
  hashing (§10). Then move `entries` to a side table or a hash-chunked
  layout; the wire contract does not change.

Retention: each pull prunes versions ≤ newest − `CVR_RETAINED_VERSIONS`
(8). A client referencing a pruned version degrades to a reset-mode pull —
safe by construction (§2 step 3). `created_at` is indexed for a future
janitor over abandoned client groups.

## 4. Fast path vs slow path

The CVR pull is the **SLOW/RECOVERY path only**. Live `DeltaFrame`s and
`logPage` catch-up remain the primary delivery channel; nothing about them
changes. The session uses the CVR pull exactly where v1 uses the full
re-bootstrap: the `must_refetch` recovery (cursor behind the retained
window, `access_changed`, server-ordered refetch). The very first sync of
a scope (no durable cursor) stays on the paged bootstrap — there is
nothing to diff against, and bootstrap's token-chained paging handles
arbitrarily large scopes.

## 5. Drift — the deviation from vanilla Replicache, and why it is sound

In Replicache, **pull is the only delivery channel**, so the stored CVR
always equals the client's state. Our clients also apply live deltas and
log pages *after* a pull, so their state runs ahead of their last CVR.
Diffing against the stale CVR alone is **unsound**: a row born after the
CVR snapshot, applied live by the client, then deleted while the client
was offline and its tombstone compacted, is in *neither* the CVR *nor*
the current set — no del would ever retract it (this exact case is a
regression test in `cvr-service.test.ts` and `session-cvr.test.ts`).

Fix: the client sends its **drift set** — its rows whose store version is
greater than the referenced CVR's `snapshot_cursor` — and the server
widens the diff base with it (max version wins). Soundness argument:

- Every client row with version ≤ the CVR snapshot is IN the CVR: rows
  surviving a pull's reconciliation are exactly the rows of that pull's
  current set (kept rows would otherwise have been dels), and later
  writes give rows versions > the snapshot. So
  `base = CVR ∪ drift ⊇ clientSet`.
- `dels = base − current ⊇ clientSet − current`: every stale client row
  is retracted. Extra dels (rows in the base the client no longer holds)
  are idempotent no-ops.
- Skipped puts are provably fresh: a current row not put has
  `version ≤ base[key]`, meaning the client applied (or was handed at
  reset) that row's latest image already.

The drift upload is bounded by activity since the last pull; above
`maxDriftEntries` (5000) the client just requests a reset pull. The
client's CVR reference is session-lifetime, in-memory: after restart,
plain bootstrap, or denial it is `null` and the next pull is reset-mode —
always sound, merely less incremental. Durable client-side CVR
persistence is a follow-up (§10), not a correctness requirement.

## 6. Compaction interplay

- Compaction (KS-2.3) preserves each live entity's **latest upsert row**
  behind the watermark precisely so the latest-per-entity derivation
  works — the CVR pull reads only that derivation plus the scope counter,
  so compaction can never break a pull (unlike log serving, which fails
  closed behind the window — invariant 6, unchanged).
- Compacted **tombstones** are the rows the log can no longer deliver;
  the CVR diff retracts them structurally (§2). Long-offline clients
  therefore recover with a diff proportional to what actually changed,
  instead of a full snapshot download.
- `khala_sync_cvrs` has its own retention (§3), independent of the
  changelog window. The two never reference each other's rows; the only
  shared object is the scope counter.

## 7. Wire + flag summary

- Contracts: `CvrPullRequest { protocolVersion, schemaVersion, scope,
  clientGroupId, cvrVersion?, drift? }` →
  `CvrPullResponse { mode: reset|diff, puts, dels, cvrVersion, cursor }`.
- Route: `POST /api/sync/cvr-pull`, same actor auth + KS-7.1 scope gate +
  error taxonomy as bootstrap; `no-store`. **Flag OFF ⇒ 404**,
  indistinguishable from an unregistered route.
- Server flag: env `KHALA_SYNC_CVR=1` (`isKhalaSyncCvrEnabled` — the
  literal `"1"` only).
- Client flag: `KhalaSyncSessionOptions.cvrRecovery` (default `false`);
  additionally requires the transport to provide `cvrPull` (optional
  member — existing transports/fakes stay valid). Any pull failure except
  an access denial falls back to the plain bootstrap, so a flagged client
  against an unflagged server degrades to exactly v1 behavior. A 403
  parks the scope `denied` and clears local state, same as v1.

## 8. Cost bounds

Per pull, one transaction:

- one index scan producing latest-per-entity rows, capped at
  `maxRowSet` (+1 overflow sentinel; default 50 000 — beyond it the pull
  refuses with a typed error and the client uses the paged bootstrap);
- one CVR row read (≤ one jsonb of `maxRowSet` entries) + one write +
  one bounded prune;
- response size O(changed rows + retracted rows), NOT O(scope) — that is
  the entire point vs re-bootstrap;
- request size O(drift), client-capped at `maxDriftEntries`.

Concurrent pulls for one (group, scope) can collide on the new
`cvr_version` primary key; the loser gets a retryable 503. Pulls are the
recovery path — client groups issue them serially in practice.

## 9. Verification (the acceptance)

Equivalence tests against **real local Postgres**
(`packages/khala-sync-server/src/cvr-service.test.ts`, gated on
`hasLocalPostgres()`): for randomized scope histories — upserts, deletes,
client live-drift prefixes, real `compactScope` runs, permission-set
changes via the visibility seam — the flagged CVR-pull client end state is
asserted **byte-equal** (canonical post-image strings) to the unflagged
full re-bootstrap end state, every phase. Includes: the drift-retraction
regression (§5), permission shrink arriving as dels with **zero** puts
and no reset, full revocation shrinking to the empty set, pruned-CVR
degradation to reset, retention, the row-set cap, and the empty scope.
Session-level equivalence (flagged vs unflagged session over one fake
server history, including fallback and denial behavior) lives in
`packages/khala-sync-client/src/session-cvr.test.ts`.

## 10. Follow-ups (explicitly out of scope here)

- **Durable client CVR persistence** (store `cvr_version`/`snapshot_cursor`
  in the local store's meta) so restarts diff instead of reset-pulling.
- **Hash-chunked CVR variant**: per-chunk hashes of the key→version map
  exchanged before entries, for row sets past the jsonb threshold (§3).
- **Entity-level authorization wiring**: connect the Worker's real
  permission model to `isEntityVisible` when scopes gain row-level
  visibility; the retraction mechanics are already tested.
- **Flag graduation**: enable on staging, measure pull sizes vs bootstrap
  on fleet-run scopes, then consider making CVR the default recovery path
  (SPEC §3 note stays additive until then).

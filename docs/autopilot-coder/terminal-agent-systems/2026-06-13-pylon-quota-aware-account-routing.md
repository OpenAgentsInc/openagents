# Pylon Quota-Aware Account Routing & Failover

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-13
Status: **Planning document — no code changes.** This file specifies a design
to be implemented under separate issues. It introduces no new runtime behavior
on its own and changes no existing runtime invariant. Field and type references
describe code as it exists today plus the *intended* additions, with the two
clearly distinguished.

Related issues: #4881, #4882, #4884. This document is the shared contract those
issues build against; #4883 is the planning issue that produced it.

Source files this design grounds against (read, current behavior):

- `apps/pylon/scripts/multi-session-run.ts`
  (`MultiSessionPlanEntry`, `MultiSessionOutcome`, `MultiSessionSummary`,
  the heartbeat phases `run_started` / `started` / `completed` / `run_completed`)
- `apps/pylon/src/account-registry.ts`
  (`resolvePylonAccountSelection`, `ResolvedPylonAccountSelection`,
  `publicPylonAccountSelection`, `hashPylonAccountRef`, `accountRefHash`)
- `apps/pylon/src/node/control-sessions.ts`
  (`ControlSessionEvent`, `ControlSessionEventPhase`, `ControlSessionState`)

---

## 1. Motivation

Today a session in the multi-session runner (and a `session.spawn` in the
control-session runner) binds to exactly one account selector. `runOneSession`
resolves that single selector via `resolvePylonAccountSelection` and runs the
proof child against it. If the underlying provider returns a quota / rate-limit
block, the child exits non-zero, `classifyError` buckets the output into one of
the existing `errorClass` values (`account_selection`, `workspace_materialization`,
`verification_failed`, `redaction_gate`, `execution_error`), and the session is
recorded as `failed`. There is no notion of "this account is temporarily
exhausted, try the next one."

This design adds **ordered account failover** driven by a **quota ledger**, so a
session can declare a pool of interchangeable accounts and the runner walks the
pool until one succeeds or the pool is exhausted — while keeping the existing
redaction posture: refs and hashes only, never raw provider text or home paths.

### Non-goals / invariants preserved

- No raw provider response text, prompts, credentials, or filesystem home paths
  enter heartbeats, outcomes, or the summary. Only `accountRefHash`-style refs
  and digest refs cross the public projection boundary, exactly as
  `publicPylonAccountSelection` already enforces.
- Concurrency, workspace materialization, dev-proof child execution, and the
  redaction scan gates (`scanProofSerialization`, `assertPublicProjectionSafe`)
  are unchanged.
- The single-selector path remains valid: a plan entry with no `accountPool`
  behaves exactly as it does today.

---

## 2. Fallback contract (account pool)

A session may optionally declare an **ordered** `accountPool`: a list of account
selectors of the same shape the runner already accepts (`accountRef` or a home
selector), in priority order.

### Proposed plan-entry extension (additive)

`MultiSessionPlanEntry` (and the analogous `ControlSessionSpawnCommand`) gains
one optional field:

```ts
// additive to the existing MultiSessionPlanEntry — all current fields unchanged
accountPool?: Array<{
  accountRef?: string        // registry_ref selector (mutually exclusive with home)
  accountHome?: string       // direct_home selector
  codexHome?: string         // adapter-specific home aliases, as today
  claudeConfigDir?: string
}>
```

Resolution rules:

- If `accountPool` is **absent or empty**, the runner uses the existing
  single-selector fields (`accountRef` / `accountHome` / `codexHome` /
  `claudeConfigDir`) and behaves exactly as today. No pool, no failover.
- If `accountPool` is **present**, it takes precedence; the single-selector
  fields are ignored for that entry (the implementation issue should reject an
  entry that sets both, mirroring the existing
  `account_selector_ambiguous` style of guard, rather than silently picking one).
- Each pool member is resolved with the **same** `resolvePylonAccountSelection`
  used today, with `provider` derived from the entry adapter via the existing
  `providerForAdapter`. Resolution errors (unknown ref, missing home) are
  surfaced per the availability rule in §3 — a member that fails to resolve is
  treated as a skip, not a hard session failure, so a typo'd tail entry cannot
  sink an otherwise-healthy pool.

### Selection loop (per session)

```
for each member in accountPool, in declared order:
    record = loadQuotaRecord(member)              // §6 ledger module
    if not isAccountAvailable(record, now):       // §3 availability rule
        emit attempt(reason = skipped_unavailable, accountHash)
        continue
    run proof child against member
    signal = classifyQuotaSignal(child output)    // §6 detection module
    if signal.exhausted:
        recordQuotaBlock(member, signal)          // §6 ledger module
        emit attempt(reason = quota_block, accountHash)
        continue
    if child succeeded:
        emit attempt(reason = succeeded, accountHash)
        -> session completed (see §5)
    else:
        // non-quota failure: existing classifyError path, session failed
        -> session failed (existing errorClass), no further pool advance
# pool exhausted without success -> §5 terminal failure
```

Key points:

- The runner **advances on quota blocks only**. A non-quota failure (a real
  verification failure, a redaction gate, a workspace error) is a genuine
  session failure and short-circuits the loop with the existing `errorClass`
  semantics — we do not burn the rest of the pool on a bug in the user's code.
- "Tries the first ledger-available account" means the loop *skips* members the
  ledger already marks unavailable before spending a child run on them (§3).

---

## 3. Availability rule

An account is **available** at time `now` iff the quota ledger has no active
block for it:

```
isAccountAvailable(record, now):
    if record is null            -> available            (never blocked)
    if record.retryAtIso is null -> unavailable          (hard block, no retry time known)
    return now >= record.retryAtIso                       (block window elapsed)
```

- **Skip the unavailable.** A member whose ledger record says unavailable
  (`now < record.retryAtIso`) is skipped without running a child, emitting a
  `skipped_unavailable` attempt receipt (§4).
- **Never retry an exhausted account within a single run.** Once the runner has
  itself recorded a `quota_block` for a member during the current run, that
  member is not re-attempted later in the same run even if its `retryAtIso`
  would elapse mid-run. The implementation tracks an in-run `Set` of
  blocked-this-run account hashes layered on top of the persisted ledger; the
  ledger governs *cross-run* skipping, the in-run set governs *within-run*
  no-retry. This keeps a single pool walk strictly forward-only and bounded by
  the pool length.
- `now` is captured once per attempt from the same clock the runner already uses
  (`nowIso()` / `new Date()`), passed explicitly into `isAccountAvailable` so the
  rule is pure and testable.

---

## 4. Honest receipts

Every account *attempt* (a run, a skip, or a block) emits a receipt. Receipts
carry the **account hash** (`accountRefHash`, via the existing
`publicPylonAccountSelection` projection) and a **routing reason**, and nothing
else identifying — no raw provider text, no home path, no ref preimage.

### Routing reason enum

```ts
type PylonRoutingReason =
  | "quota_block"          // child ran, detection said exhausted; ledger updated; advance
  | "skipped_unavailable"  // ledger said unavailable (or member failed to resolve); not run
  | "succeeded"            // child ran and succeeded; pool walk ends
```

### Per-attempt heartbeat (multi-session runner)

A new heartbeat phase `account_attempt` is appended (additive to the existing
`run_started` / `started` / `completed` / `run_completed` set) for each pool
member the runner considers:

```ts
// openagents.pylon.multi_session_heartbeat.v0.1 — additive fields
{
  schema: MULTI_SESSION_HEARTBEAT_SCHEMA,
  runRef, sessionRef, observedAt,
  phase: "account_attempt",
  sessionIndex,
  accountHash: string,            // = ResolvedPylonAccountSelection.accountRefHash
  routingReason: PylonRoutingReason,
  poolIndex: number,              // 0-based position within accountPool
  retryAtIso: string | null,      // present for quota_block / skipped_unavailable
}
```

The existing `started` and `completed` heartbeats are unchanged. For the
control-session runner the analogous receipt is a new `ControlSessionEventPhase`
value `account_attempt` carrying the same `accountHash` / `routingReason`
fields, reusing the existing `account: PublicPylonAccountSelection | null`
field for the hash.

### Outcome (winning / final attempt)

`MultiSessionOutcome` gains additive routing fields. The existing `account`
field continues to report the account the session ultimately ran under (the
winner on success; the last attempted member on exhaustion):

```ts
// additive to MultiSessionOutcome — existing fields unchanged
routingReason: PylonRoutingReason,   // "succeeded" on completed, "quota_block" on exhaustion
attemptedAccountHashes: string[],    // accountRefHash for every member considered, in order
```

`attemptedAccountHashes` is hashes only — it is the audit trail of which
accounts were tried, never the refs or homes themselves.

### Summary

`MultiSessionSummary` records the run-level routing picture (all additive; the
existing `deviations` array and counts are unchanged except for the new
deviation token in §5):

```ts
// additive to MultiSessionSummary
routing: {
  // per session: which accounts were tried and the final decision
  sessions: Array<{
    sessionRef: string,
    attemptedAccountHashes: string[],
    finalRoutingReason: PylonRoutingReason,
    finalAccountHash: string | null,   // null only if every member was skipped/unresolved
  }>,
  // accounts the run observed as quota-blocked at least once, hashes only
  quotaBlockedAccountHashes: string[],
}
```

All of the above pass through the existing `scanProofSerialization` and
`assertPublicProjectionSafe` gates unchanged; because every new field is a hash
or an enum, they carry no redaction risk.

---

## 5. Terminal states

A session resolves to exactly one terminal state, consistent with the existing
`MultiSessionOutcome.state` union (`"completed" | "failed"`):

- **completed** — some pool member's child succeeded. `state = "completed"`,
  `routingReason = "succeeded"`, `account` / `finalAccountHash` is the winning
  member, `attemptedAccountHashes` lists every member tried up to and including
  the winner. Identical artifact / `resultRef` semantics to today.

- **failed (non-quota)** — a member ran and failed for a non-quota reason. The
  existing `classifyError` path applies unchanged; the pool walk stops. The
  outcome's `errorClass` / `errorDigestRef` are as today, with
  `routingReason` recorded as the reason of the failing attempt.

- **failed (pool exhausted)** — every member was either skipped as unavailable
  or ran and was quota-blocked, with no success. `state = "failed"`,
  `routingReason = "quota_block"`, and the session contributes a new deviation
  token:

  ```
  deviation.pylon.multi_session.all_accounts_exhausted
  ```

  This token joins the existing
  `deviation.pylon.multi_session.some_sessions_failed` in
  `MultiSessionSummary.deviations` (the run still reports `some_sessions_failed`
  because the exhausted session counts toward `failedCount`; the new token is
  additive and more specific). The exhaustion deviation entry carries the
  **earliest `retryAtIso` across the pool** so an operator knows the soonest the
  session could be retried:

  ```ts
  // shape of the additive exhaustion deviation detail recorded in the summary
  {
    deviation: "deviation.pylon.multi_session.all_accounts_exhausted",
    sessionRef: string,
    earliestRetryAtIso: string | null,   // min(retryAtIso) over blocked members; null if none known
    attemptedAccountHashes: string[],
  }
  ```

  `earliestRetryAtIso` is `null` only when every block lacked a parseable retry
  time (all `retryAtIso === null`), i.e. hard blocks with no known recovery
  window.

The existing summary counters (`completedCount`, `failedCount`,
`totalDurationMs`, `totalTokens`) compute exactly as today over the final
per-session outcomes.

---

## 6. Expected module interfaces

The implementation issues (#4881 / #4882 / #4884) should wire to these exact
shapes so the runner does not re-decide them. Both modules are **new**; the
runner depends only on these signatures, not their internals.

### 6a. Detection module — `classifyQuotaSignal`

Pure classifier over a completed child's output. It receives the same combined
output the runner already feeds `classifyError`
(`` `${child.stderr}\n${child.stdout}` ``) and decides whether it represents a
quota / rate-limit exhaustion, plus an optional retry time.

```ts
export type QuotaSignal = {
  exhausted: boolean            // true => this is a quota/rate-limit block, advance the pool
  retryAtRaw: string | null     // provider-reported retry hint as captured (e.g. a header value),
                                // used ONLY to derive retryAtIso; never emitted to public receipts
  retryAtIso: string | null     // normalized ISO-8601 instant the account may be retried,
                                // or null if no recovery time could be determined
  sourceDigestRef: string       // stable digest ref of the matched signal source
                                // (a `digest.pylon.*` ref), so receipts can cite the
                                // evidence without carrying raw provider text
}

export function classifyQuotaSignal(combinedChildOutput: string): QuotaSignal
```

Contract notes for the implementer:

- `exhausted: false` means "not a quota block" — the runner then falls through
  to the existing `classifyError` failure handling. The detection module must
  not claim a quota block for ordinary verification or workspace failures.
- `retryAtRaw` exists purely as the preimage for `retryAtIso`; it is **not**
  forwarded to any heartbeat, outcome, or summary field. Only `retryAtIso` and
  `sourceDigestRef` cross the public boundary.
- `sourceDigestRef` should be produced with the same `stableRef` /
  `createHash("sha256")` convention already used throughout the runner
  (`digest.pylon.multi_session.*`-style prefix), so it is redaction-safe by
  construction.

### 6b. Ledger module — `recordQuotaBlock` / `loadQuotaRecord` / `isAccountAvailable`

Persists and reads per-account quota state, keyed by the **account hash**
(`accountRefHash`) so no ref preimage or home path is stored. The ledger lives
under the existing pylon home / paths so it survives across runs.

```ts
export type QuotaLedgerRecord = {
  accountHash: string            // = ResolvedPylonAccountSelection.accountRefHash (the key)
  blockedAtIso: string           // when the block was recorded
  retryAtIso: string | null      // earliest retry instant; null => hard block, no known window
  sourceDigestRef: string        // from the QuotaSignal that caused the block
}

// Persist a block for an account. Called after classifyQuotaSignal reports
// exhausted. Idempotent per (accountHash, blockedAtIso) so retries/replays are safe.
export function recordQuotaBlock(
  paths: PylonPaths,                          // existing pylon paths object
  account: ResolvedPylonAccountSelection,     // hash + provider taken from here
  signal: QuotaSignal,
): Promise<void>

// Read the current record for an account, or null if never blocked.
export function loadQuotaRecord(
  paths: PylonPaths,
  account: ResolvedPylonAccountSelection,
): Promise<QuotaLedgerRecord | null>

// Pure availability predicate (see §3). `now` is injected for testability.
export function isAccountAvailable(
  record: QuotaLedgerRecord | null,
  now: Date,
): boolean
```

Contract notes:

- The ledger key is the hash, never the ref. `recordQuotaBlock` derives the key
  from `account.accountRefHash`; it must not read `account.home` or
  `account.accountRef` into the persisted record.
- `loadQuotaRecord` returning `null` means "no known block" → available.
- `isAccountAvailable` is the single source of truth for the §3 rule and is
  pure: same `(record, now)` always yields the same answer. The in-run
  no-retry behavior (§3) is enforced by the **runner**, not this predicate, so
  the ledger module stays free of run-scoped state.

---

## 7. What this changes vs. what it preserves

| Area | Today | After implementation |
| --- | --- | --- |
| Plan entry account binding | one selector | optional ordered `accountPool`; single-selector path unchanged |
| Quota block | child exits non-zero → `failed` via `classifyError` | detected, recorded to ledger, pool advances |
| Heartbeat phases | `run_started`/`started`/`completed`/`run_completed` | + `account_attempt` (additive) |
| Outcome | `state`, `account`, error refs | + `routingReason`, `attemptedAccountHashes` (additive) |
| Summary | counts, `outcomes`, `deviations` | + `routing` block; + `all_accounts_exhausted` deviation token |
| Redaction posture | refs/hashes/digests only | unchanged — every new field is a hash, enum, ISO time, or digest ref |
| Concurrency / workspace / dev-proof child | as in `runMultiSessionPlan` | unchanged |

No existing field is removed or repurposed; no existing invariant is relaxed.
Implementation lands behind the optional `accountPool` field, so plans that do
not use it observe byte-identical behavior to today.

---

## 8. Open questions for the implementation issues

- **Ledger storage format & location.** Reuse the JSONL ledger convention
  (`appendLedgerEvent` in `apps/pylon/src/wallet.ts`) vs. a dedicated
  `quota-ledger.jsonl` under pylon paths. Recommendation: a dedicated file to
  keep quota state out of the wallet ledger, but follow the same append +
  last-write-wins-by-key read shape. (#4882)
- **Cross-session sharing within a run.** When two concurrent sessions share a
  pool member and one records a block, should the other observe it immediately?
  The persisted ledger makes this eventually-consistent; the in-run set is
  per-session. Decide whether to add a run-scoped shared block set. (#4884)
- **`classifyQuotaSignal` provider coverage.** Which provider signals (HTTP 429,
  provider-specific quota strings, `Retry-After`) map to `exhausted` and how
  `retryAtRaw` → `retryAtIso` normalization handles relative vs. absolute
  hints. (#4881)

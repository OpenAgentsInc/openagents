import type { SyncScope } from "@openagentsinc/khala-sync"

/**
 * Khala Sync scope-read authorization resolver (KS-7.1, #8305; SPEC §2.1
 * scope taxonomy, §3 auth, §7 invariant 7).
 *
 * `resolveScopeRead` is the ONE gate every read surface (log / bootstrap /
 * connect) consults. It covers the full scope taxonomy:
 *
 *   scope.user.<userId>          the user themself, nobody else — NEVER an
 *                                anonymous caller
 *   scope.public.<channel>       ANY caller, including one with NO
 *                                authenticated actor at all (`userId ===
 *                                undefined`) — the ONLY taxonomy member ever
 *                                readable anonymously (KS-8.x anonymous-read
 *                                exception; docs/khala-sync/RUNBOOK.md
 *                                "Anonymous read scopes")
 *   scope.team.<teamId>          LIVE team membership (capability callback)
 *                                — NEVER an anonymous caller
 *   scope.agent_run.<runId>      run owner, or a member of the run's team —
 *                                NEVER an anonymous caller
 *   scope.thread.<threadId>      thread capability callback (legacy
 *                                agent-run/autopilot mappings plus
 *                                owner-private chat thread ownership) —
 *                                NEVER an anonymous caller
 *   scope.fleet_run.<fleetRunId> the khala_sync_scope_owners owner — NEVER
 *                                an anonymous caller
 *   anything else                DENIED with a typed `unknown_scope` —
 *                                a taxonomy member without a read policy is
 *                                gated CLOSED until one is written here.
 *
 * ANONYMOUS-READ AIRTIGHTNESS: `kind === "public"` is checked FIRST, before
 * any other branch, and an anonymous caller (`userId === undefined`) is
 * denied immediately after that check for every other kind — structurally,
 * no kind other than `public` can ever reach `SCOPE_READ_ALLOWED` without a
 * defined `userId`. `isAnonymousReadableScope` below is the single-source
 * prefix check route handlers use to decide whether authentication is
 * mandatory BEFORE calling this resolver; this resolver is still the
 * authoritative second gate once a `userId` (or `undefined`) is known —
 * defense in depth, not "trust the route's own check."
 *
 * WHERE THE DATA LIVES decides the seam shape: team membership and
 * agent-run/thread ownership live in the openagents.com Worker's D1 today,
 * while `khala_sync_scope_owners` lives in Khala Sync Postgres. So the
 * resolver takes injected CAPABILITY CALLBACKS
 * ({@link KhalaSyncScopeAuthCapabilities}) — the Worker implements the
 * D1-backed ones against its live tables
 * (`workers/api/src/khala-sync-scope-auth.ts`) and the Postgres-backed
 * fleet-owner lookup through its Hyperdrive binding. Nothing here caches:
 * membership checks are live-at-read, so a revoked user fails the resolver
 * on their very next request (the push half of invariant 7 — the hub's
 * `MustRefetch(access_changed)` broadcast — lives in the hub DO).
 *
 * FAIL-CLOSED BY CONSTRUCTION: a capability that throws can never grant.
 * Every callback invocation is wrapped; failures come back as a typed
 * `unavailable` decision (routes map it to a 503 `storage_unavailable`
 * SyncError), never as `allowed` and never as a silent deny that hides an
 * outage behind a 403.
 */

// ---------------------------------------------------------------------------
// Decision model
// ---------------------------------------------------------------------------

/** Wire `SyncErrorCode` a denial maps to (both are 403, both no-store). */
export type ScopeReadDenialReason = "unauthorized_scope" | "unknown_scope"

export type ScopeReadDecision =
  | { readonly kind: "allowed" }
  | { readonly kind: "denied"; readonly reason: ScopeReadDenialReason }
  | {
      /** A capability failed: fail closed, surface a retryable 503. */
      readonly kind: "unavailable"
      readonly messageSafe: string
    }

export const SCOPE_READ_ALLOWED: ScopeReadDecision = { kind: "allowed" }

const denied = (reason: ScopeReadDenialReason): ScopeReadDecision => ({
  kind: "denied",
  reason,
})

// ---------------------------------------------------------------------------
// Capability seam (implemented Worker-side against live D1 + Postgres)
// ---------------------------------------------------------------------------

export interface KhalaSyncScopeAuthCapabilities {
  /** LIVE team membership: does `userId` hold an ACTIVE membership in `teamId`? */
  readonly isTeamMember: (userId: string, teamId: string) => Promise<boolean>
  /**
   * `scope.agent_run.<runId>` ownership per the Worker's `agent_runs`
   * table: the run's owning user, or (for team runs) an active member of
   * the run's team. Unknown/archived runs must answer `false`.
   */
  readonly canReadAgentRun: (userId: string, runId: string) => Promise<boolean>
  /**
   * `scope.thread.<threadId>` ownership: Worker-side callbacks may resolve
   * legacy agent-run/autopilot-thread mappings and owner-private chat
   * thread ownership. Unresolvable threads must answer `false`.
   */
  readonly canReadThread: (userId: string, threadId: string) => Promise<boolean>
  /**
   * `khala_sync_scope_owners` lookup for a `scope.fleet_run.*` scope
   * (Postgres-side — `readScopeOwner`). `null` = unowned scope (denied).
   */
  readonly readFleetScopeOwner: (scope: SyncScope) => Promise<string | null>
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

const SCOPE_ID_PATTERN = /^scope\.([a-z_]+)\.(.+)$/

/** The one taxonomy kind ever readable without an authenticated actor. */
const ANONYMOUS_READABLE_SCOPE_KIND = "public"

/**
 * True exactly when `scope`'s taxonomy kind is `public` — a strict,
 * exact-match parse of the SAME `SCOPE_ID_PATTERN` `resolveScopeRead` uses
 * (single source of truth; no separate regex to drift out of sync). Route
 * handlers call this BEFORE `authenticate()` to decide whether a missing
 * session/token is fatal (401) or the request may proceed with `userId ===
 * undefined`; `resolveScopeRead` remains the authoritative second gate.
 *
 * The kind is captured by `[a-z_]+` up to the FIRST literal `.` after
 * `scope.`, so a crafted id segment (e.g. `scope.public_evil.x` or
 * `scope.team.public.x`) can never be mistaken for the `public` kind: kind
 * extraction never sees dots past the first one, and equality is exact
 * (`=== "public"`), never a prefix/substring/`includes` check.
 */
export const isAnonymousReadableScope = (scope: SyncScope): boolean => {
  const match = SCOPE_ID_PATTERN.exec(scope)
  return match !== null && match[1] === ANONYMOUS_READABLE_SCOPE_KIND
}

/**
 * Wrap ONE capability call so a thrown storage/driver error becomes a typed
 * `unavailable` decision (fail-closed; the raw error is never echoed — it
 * can embed connection strings or row values).
 */
const guarded = async (
  capability: string,
  decide: () => Promise<ScopeReadDecision>,
): Promise<ScopeReadDecision> => {
  try {
    return await decide()
  } catch {
    return {
      kind: "unavailable",
      messageSafe: `Khala Sync scope authorization is unavailable (${capability} lookup failed); retry the request.`,
    }
  }
}

/**
 * Resolve whether `userId` may READ `scope`. Never throws: every outcome —
 * including capability failure — is a typed {@link ScopeReadDecision}.
 * Write-side authorization stays inside each mutator's own transaction
 * (SPEC §2.4); this resolver gates the read surfaces only.
 *
 * `userId === undefined` models an ANONYMOUS caller (KS-8.x anonymous-read
 * exception): the ONLY kind it can ever be granted is `public` — that
 * check runs first, unconditionally, before the anonymous caller is turned
 * away. Every other kind denies an anonymous caller immediately, before any
 * capability callback ever runs (so `isTeamMember`/`canReadAgentRun`/
 * `canReadThread`/`readFleetScopeOwner` are never invoked with an
 * undefined actor).
 */
export const resolveScopeRead = async (
  capabilities: KhalaSyncScopeAuthCapabilities,
  userId: string | undefined,
  scope: SyncScope,
): Promise<ScopeReadDecision> => {
  const match = SCOPE_ID_PATTERN.exec(scope)
  if (match === null) {
    // The SyncScope brand should make this unreachable; deny anyway.
    return denied("unknown_scope")
  }
  const kind = match[1]!
  const id = match[2]!

  // `scope.public.*` is readable by literally anyone, authenticated or not
  // — checked FIRST and unconditionally, so this is the only way an
  // anonymous caller ever reaches `allowed`.
  if (kind === "public") {
    return SCOPE_READ_ALLOWED
  }
  if (userId === undefined) {
    // No other taxonomy kind is ever anonymous-readable. Denied here,
    // before the switch below, so no capability callback below can ever be
    // reached with an undefined actor.
    return denied("unauthorized_scope")
  }

  switch (kind) {
    case "user":
      return id === userId ? SCOPE_READ_ALLOWED : denied("unauthorized_scope")
    case "team":
      return guarded("team membership", async () =>
        (await capabilities.isTeamMember(userId, id))
          ? SCOPE_READ_ALLOWED
          : denied("unauthorized_scope"),
      )
    case "agent_run":
      return guarded("agent-run ownership", async () =>
        (await capabilities.canReadAgentRun(userId, id))
          ? SCOPE_READ_ALLOWED
          : denied("unauthorized_scope"),
      )
    case "thread":
      return guarded("thread ownership", async () =>
        (await capabilities.canReadThread(userId, id))
          ? SCOPE_READ_ALLOWED
          : denied("unauthorized_scope"),
      )
    case "fleet_run":
      return guarded("fleet scope owner", async () => {
        const owner = await capabilities.readFleetScopeOwner(scope)
        return owner !== null && owner === userId
          ? SCOPE_READ_ALLOWED
          : denied("unauthorized_scope")
      })
    default:
      // A scope kind with no read policy (nothing produces it today) is
      // gated CLOSED: typed `unknown_scope`, 403 at the routes. Adding a
      // taxonomy member REQUIRES adding its arm here plus matrix tests.
      return denied("unknown_scope")
  }
}

import type { SyncScope } from "@openagentsinc/khala-sync"

/**
 * Khala Sync scope-read authorization resolver (KS-7.1, #8305; SPEC §2.1
 * scope taxonomy, §3 auth, §7 invariant 7).
 *
 * `resolveScopeRead` is the ONE gate every read surface (log / bootstrap /
 * connect) consults. It covers the full scope taxonomy:
 *
 *   scope.user.<userId>          the user themself, nobody else
 *   scope.public.<channel>       every authenticated caller
 *   scope.team.<teamId>          LIVE team membership (capability callback)
 *   scope.agent_run.<runId>      run owner, or a member of the run's team
 *   scope.thread.<threadId>      same ownership rule via the thread mapping
 *   scope.fleet_run.<fleetRunId> the khala_sync_scope_owners owner
 *   anything else                DENIED with a typed `unknown_scope` —
 *                                a taxonomy member without a read policy is
 *                                gated CLOSED until one is written here.
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
   * `scope.thread.<threadId>` ownership: the thread route id resolves to
   * its agent run (including the autopilot-thread mapping) and follows the
   * same ownership rule as {@link canReadAgentRun}. Unresolvable threads
   * must answer `false`.
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
 */
export const resolveScopeRead = async (
  capabilities: KhalaSyncScopeAuthCapabilities,
  userId: string,
  scope: SyncScope,
): Promise<ScopeReadDecision> => {
  const match = SCOPE_ID_PATTERN.exec(scope)
  if (match === null) {
    // The SyncScope brand should make this unreachable; deny anyway.
    return denied("unknown_scope")
  }
  const kind = match[1]!
  const id = match[2]!

  switch (kind) {
    case "user":
      return id === userId ? SCOPE_READ_ALLOWED : denied("unauthorized_scope")
    case "public":
      return SCOPE_READ_ALLOWED
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

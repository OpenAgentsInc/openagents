// Per-identity daily served-token ceiling for the OWNER-FUNDED hosted-compute
// proxy (`POST /api/provider-accounts/google-gemini/models/*:streamGenerateContent`).
//
// WHY
// ---
// That proxy authenticates with `requireHostedComputeActor`, which accepts ANY
// `oa_agent_` bearer token, and then calls Google with the owner's shared
// `GEMINI_API_KEY`. `POST /api/agents/register` is public self-service by
// design (it is the documented Forum/registered-agent door and has a real
// third-party cohort), so anyone could mint an `oa_agent_` token and draw on
// owner-funded inference.
//
// The ceiling that already existed on this route
// (`selfProvisionedTokenCeilingRefusal`) inspected ONLY self-provisioned
// `nostr:` identities and returned "proceed" for every agent, GitHub, and email
// actor. Its own comment recorded the reason: making it binding for EVERY actor
// would have cut off the owner's own Pylon runners mid-flight. This module
// keeps that safety property while closing the hole, by bounding every actor
// EXCEPT an explicitly admitted one.
//
// ADMITTED COHORT
// ---------------
// The exemption reuses the existing `INFERENCE_INTERNAL_ACCOUNT_REFS`
// allowlist (`inference-internal-account.ts`) rather than inventing a second
// admission mechanism. That env-configured set already names the owner's
// internal/ops account refs, is already the thing this repository means by
// "our own tooling", and is already consulted on the chat-completions route to
// exempt internal platform capacity from the same class of bound. An admitted
// actor short-circuits WITHOUT a ledger read, so no live owner workflow gains
// cost or latency.
//
// FAIL-CLOSED
// -----------
// A ledger read that throws REFUSES a non-admitted actor rather than serving
// it. A spend path must not fall open during a database outage. This matches
// the self-provisioned ceiling it replaces.

/** Stable refusal code, unchanged from the self-provisioned ceiling. */
export const HOSTED_COMPUTE_CEILING_ERROR = 'free_tier_daily_token_ceiling_reached' as const

/**
 * Compiled default daily served-token ceiling for a non-admitted actor.
 *
 * Sized against the real ledger: the largest single-UTC-day draw by a
 * non-self-provisioned actor on this route to date is ~172k tokens, so 1M
 * leaves legitimate use roughly 5x headroom while removing the unbounded draw.
 * It deliberately equals the self-provisioned ceiling default so the two
 * classes are not silently asymmetric. Owner-tunable without a code deploy.
 */
export const DEFAULT_HOSTED_COMPUTE_DAILY_TOKEN_CEILING = 1_000_000

export type HostedComputeCeilingRefusal = Readonly<{
  error: typeof HOSTED_COMPUTE_CEILING_ERROR
  tokensServedToday: number
  dailyTokenCeiling: number
}>

/**
 * Resolve the owner-tunable ceiling for a non-admitted actor.
 *
 * Absent / non-numeric / negative falls back to the compiled default. Zero is
 * HONORED (the useful "serve nobody new" state), which is why the guard tests
 * `< 0` rather than `<= 0`.
 */
export const hostedComputeDailyTokenCeiling = (rawValue: string | undefined): number => {
  if (rawValue === undefined) {
    return DEFAULT_HOSTED_COMPUTE_DAILY_TOKEN_CEILING
  }
  const parsed = Number.parseInt(rawValue.trim(), 10)
  return Number.isFinite(parsed) && parsed >= 0
    ? parsed
    : DEFAULT_HOSTED_COMPUTE_DAILY_TOKEN_CEILING
}

/**
 * Is this actor explicitly admitted to unbounded owner-funded hosted compute?
 *
 * The allowlist holds ACCOUNT refs (`agent:user_…`, `openauth:…`), while this
 * route authenticates to a bare actor user id. Both the bare id and the two
 * ref forms the repository uses are accepted so an operator can admit an actor
 * with the same string they already use for demand attribution.
 */
export const isAdmittedHostedComputeActor = (
  actorUserId: string,
  admittedAccountRefs: ReadonlySet<string>,
): boolean =>
  admittedAccountRefs.has(actorUserId) ||
  admittedAccountRefs.has(`agent:${actorUserId}`) ||
  admittedAccountRefs.has(`openauth:${actorUserId}`)

/**
 * The pure ceiling decision.
 *
 * The comparison is `>=`, so a ceiling of 0 refuses every request and a
 * ceiling reached exactly is refused rather than allowed to tip over.
 */
export const decideHostedComputeDailyCeiling = (
  input: Readonly<{
    servedToday: number
    tokensPerDay: number
  }>,
): HostedComputeCeilingRefusal | undefined =>
  input.servedToday >= input.tokensPerDay
    ? {
        error: HOSTED_COMPUTE_CEILING_ERROR,
        tokensServedToday: input.servedToday,
        dailyTokenCeiling: input.tokensPerDay,
      }
    : undefined

export type HostedComputeDailyCeilingDeps = Readonly<{
  /** Exact served-token total for `actorUserId` since the UTC day boundary. */
  servedTokensToday: (actorUserId: string) => Promise<number>
  /** The ceiling for this actor (self-provisioned actors keep their own). */
  tokensPerDay: (actorUserId: string) => number
  admittedAccountRefs: ReadonlySet<string>
}>

/**
 * Build the route gate. An admitted actor short-circuits WITHOUT a ledger read.
 *
 * FAIL-CLOSED: a ledger read that throws refuses the actor.
 */
export const makeHostedComputeDailyCeilingGate = (deps: HostedComputeDailyCeilingDeps) => {
  return async (actorUserId: string): Promise<HostedComputeCeilingRefusal | undefined> => {
    if (isAdmittedHostedComputeActor(actorUserId, deps.admittedAccountRefs)) {
      return undefined
    }
    const tokensPerDay = deps.tokensPerDay(actorUserId)
    let servedToday: number
    try {
      servedToday = await deps.servedTokensToday(actorUserId)
    } catch {
      return {
        error: HOSTED_COMPUTE_CEILING_ERROR,
        tokensServedToday: tokensPerDay,
        dailyTokenCeiling: tokensPerDay,
      }
    }
    return decideHostedComputeDailyCeiling({ servedToday, tokensPerDay })
  }
}

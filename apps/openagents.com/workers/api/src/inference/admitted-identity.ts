// Is a `nostr:` identity OPERATOR-ADMITTED, or is it free tier?
//
// WHY THIS EXISTS
// ---------------
// Both owner-funded daily ceilings key on the `nostr:` user-id prefix, and
// `scripts/admit-sarah-voice-npub.ts` gives an operator-admitted alpha member
// the SAME id form a self-provisioned install gets. The prefix therefore does
// not distinguish "stranger who downloaded the DMG" from "identity the owner
// deliberately admitted" — it conflates them, and a ceiling written for the
// first silently meters the second as free tier.
//
// That conflation was latent for three days because the ledger read behind both
// ceilings returned 0 unconditionally (fixed in 460e7eb80f). The moment the
// read started counting, the latent hazard became a live outage: on 2026-07-31
// the owner's own admitted identity was refused
// `free_tier_daily_token_ceiling_reached` on `openagents/gpt-5.6-luna`, on the
// `gemini-3.6-flash` fallback, and on thread-title generation — every turn in
// his own app — because he had served 10,410,253 tokens against a 1,000,000
// ceiling he was never the subject of.
//
// The correct discriminator is MEMBERSHIP, not prefix. `sarah_voice_alpha_
// memberships` is the existing record of who the owner admitted, written by the
// admission script under an explicit approval gate. An identity with an active
// row there is not free tier and must not be metered as free tier.
//
// SCOPE. This grants exemption from the FREE-TIER token ceiling and nothing
// else. It is deliberately NOT the `INFERENCE_INTERNAL_ACCOUNT_REFS` allowlist,
// which additionally confers internal demand classification and access to the
// internal-neutral lane. An admitted alpha member should stop being metered as
// a stranger; that is not the same as becoming internal platform capacity.
//
// FAIL-CLOSED. A membership read that throws resolves to NOT admitted, so the
// caller falls through to the ceiling rather than past it. A spend path must
// not fall open during a database outage — the cost of that choice is that an
// admitted member is briefly metered like free tier, never that a stranger is
// served without a bound.

/** The one `state` value that admits. A revoked or pending row does not. */
export const ADMITTED_IDENTITY_STATE = 'active' as const

/** Minimal D1-shaped surface, so this needs no concrete database type. */
export type AdmittedIdentityDatabase = Readonly<{
  prepare: (query: string) => Readonly<{
    bind: (...values: ReadonlyArray<unknown>) => Readonly<{
      first: <Row>() => Promise<Row | null | undefined>
    }>
  }>
}>

export const ADMITTED_IDENTITY_QUERY = `SELECT 1 AS admitted
     FROM sarah_voice_alpha_memberships
    WHERE owner_user_id = ?
      AND state = ?
      AND revoked_at IS NULL
    LIMIT 1` as const

/**
 * Build the membership lookup used by both ceiling gates.
 *
 * Returns `false` rather than throwing, so a caller can consult it without its
 * own try/catch and still get the fail-closed behavior described above.
 */
export const makeAdmittedIdentityLookup =
  (database: AdmittedIdentityDatabase) =>
  async (userId: string): Promise<boolean> => {
    try {
      const row = await database
        .prepare(ADMITTED_IDENTITY_QUERY)
        .bind(userId, ADMITTED_IDENTITY_STATE)
        .first<{ admitted: unknown }>()
      return row !== null && row !== undefined
    } catch {
      return false
    }
  }

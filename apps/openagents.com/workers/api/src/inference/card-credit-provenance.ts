// Card -> credit provenance binding
// (blocker.product_promises.inference_card_credit_inference_spend_receipt_missing).
//
// THE GAP this closes: `card-credit-spend-receipt.ts` can ASSEMBLE a three-hop
// card -> credit -> inference-spend receipt, but only if a caller hands it three
// legs that genuinely belong to the same chain. Today nothing PERSISTS that
// linkage: the USD->msat bridge writes each grant with a generic
// `context_ref` of `inference:usd-credit:<userId>` (`usd-credit-bridge.ts`),
// which says WHO was funded but NOT which Stripe checkout session funded them.
// So the assembled receipt cannot be DEREFERENCED back to its originating
// purchase from stored state — the chain is asserted by the caller, not proven.
//
// This module is the single source of truth for the grant `context_ref` FORMAT
// that carries a grant's originating Stripe checkout session. When the bridge is
// asked to fund inference from a specific card purchase it stamps the grant with
// `cardCreditGrantContextRef(sessionId)`; a resolver (or the receipt assembler)
// then `parseCardCreditGrantContextRef`s the stored `context_ref` to recover the
// session and prove the credit_to_msat hop chains to the SAME purchase the
// card_to_credit hop names. Card-origin grants are now dereferenceable to their
// purchase; generic (Lightning-/balance-funded) grants keep the legacy format
// and parse to `undefined`, so this is purely additive.
//
// PURE: no D1, clock, network, or secrets. A Stripe checkout session id is not
// payment material (it is the public correlation id Stripe returns in the
// success redirect and the webhook), so it is safe to embed in a public ref.

// The stable prefix for a card-origin grant `context_ref`. Distinct from the
// legacy generic prefix (`inference:usd-credit:<userId>`) by the `:card:`
// segment, so the two formats never collide and a generic grant parses to
// `undefined` (it carries no session).
export const CARD_CREDIT_GRANT_CONTEXT_PREFIX = 'inference:usd-credit:card:'

// True when `sessionId` is a usable Stripe checkout session id: non-blank and
// free of the `:` delimiter (so the round-trip parse is unambiguous). Stripe
// session ids are `cs_`-prefixed alphanumerics with underscores, never colons.
const isValidSessionId = (sessionId: string): boolean =>
  sessionId.trim() !== '' && !sessionId.includes(':')

// Build the canonical grant `context_ref` that binds a credit grant to the
// Stripe checkout session that funded it. Returns `undefined` when the session
// id is unusable (blank or delimiter-bearing) so a caller can fall back to the
// legacy generic context_ref rather than persist an unparseable link.
export const cardCreditGrantContextRef = (
  sessionId: string,
): string | undefined =>
  isValidSessionId(sessionId)
    ? `${CARD_CREDIT_GRANT_CONTEXT_PREFIX}${sessionId}`
    : undefined

// Recover the originating Stripe checkout session id from a grant `context_ref`.
// Returns `undefined` for any context_ref that is not a card-origin grant
// (legacy generic grants, malformed refs, or a blank trailing session), so the
// caller can tell a dereferenceable card-origin grant from one that is not.
export const parseCardCreditGrantContextRef = (
  contextRef: string,
): string | undefined => {
  if (!contextRef.startsWith(CARD_CREDIT_GRANT_CONTEXT_PREFIX)) {
    return undefined
  }
  const sessionId = contextRef.slice(CARD_CREDIT_GRANT_CONTEXT_PREFIX.length)
  return isValidSessionId(sessionId) ? sessionId : undefined
}

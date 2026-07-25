/**
 * SARAH-CW-05 — Sarah community arbitration and dispute path.
 *
 * - Arbitration decision event templates (kind 7000, typed reason classes)
 * - Dispute appeal types (owner is arbiter of last resort)
 * - Owner appeal-identity registry (public key only; missing → NEEDS_OWNER)
 *
 * Sarah decides acceptance, not payment. She cannot verify her own production
 * without an independent distinct-operator verifier. She cannot author owner
 * rulings. A dispute path must exist before any payout.
 */
export * from "./types.ts";
export * from "./rules.ts";
export * from "./templates.ts";
export * from "./identity.ts";
export * from "./verification.ts";

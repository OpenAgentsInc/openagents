/**
 * IDR-04 identity reconciliation.
 *
 * This barrel exposes the reconciliation surface on top of the IDR-03 decode
 * boundary and the IDR-06 signer path: the Spark comparison adapters (exact Rust
 * and Breez, deferred LDK), the public-safe reconciliation result and typed
 * conflict schemas, and the `reconcileIdentities` engine that proves a decoded
 * candidate is the RIGHT identity before IDR-05 ever imports it.
 *
 * Reconciliation classifies only. It never imports, creates, or writes.
 */
export * from "./spark-adapter.ts";
export * from "./result.ts";
export * from "./reconcile.ts";

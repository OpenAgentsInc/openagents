/**
 * Pylon wallet boundary (P5) — the Spark rail.
 *
 * IDR-07 lands the app-side Spark wallet STATUS adapter here: it restores the
 * Spark wallet from the recovered shared root in a strictly status-only posture,
 * behind the neutral `@openagentsinc/sovereign-identity` reconciliation seam. It
 * opens the EXPECTED wallet or fails closed, exposes public wallet status only,
 * and admits no send path.
 */

export * from "./spark-status.js"

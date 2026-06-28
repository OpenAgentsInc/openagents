/**
 * Blueprint Signature-governed autonomous-ops gates.
 *
 * Each module here is a pure, ordered-predicate state machine implementing one
 * Blueprint Signature from
 * `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md`.
 * The terminal state of each gate is the only state that unlocks its
 * consequential action; a missing required evidence ref makes the action
 * structurally impossible.
 *
 * These are libraries (types + pure functions + Bun tests). They are not yet
 * wired into the live supervisor/watcher — wiring is a follow-up.
 */

export * from "./issue-close-safe.js"
export * from "./command-execution-source-verified.js"
export * from "./merge-deploy-gate.js"

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
 * These modules expose pure evaluators plus enforced action authorizers. The
 * authorizers return the consequential action payload only from the signature's
 * terminal state, so Artanis tick/operator call sites cannot structurally take
 * the action from a partial gate.
 */

export * from "./enforced-actions.js"
export * from "./diagnosis-grounding.js"
export * from "./issue-close-safe.js"
export * from "./command-execution-source-verified.js"
export * from "./merge-deploy-gate.js"
export * from "./fleet-liveness.js"
export * from "./virtual-merge-queue.js"

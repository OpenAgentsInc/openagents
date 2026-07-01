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
 * These modules expose pure evaluators plus enforced action authorizers. Fleet
 * liveness is wired into Pylon status reporting. The issue-close authorizer is
 * wired into the Pylon PR publisher's closing-keyword edit path, and the
 * diagnosis, command-proposal, and merge-deploy authorizers are wired into the
 * Artanis scheduled/operator dispatch call sites that construct those
 * consequential actions. Other consumers must still call an authorizer at the
 * acting site before treating a non-terminal gate as action authority.
 */

export * from "./enforced-actions.js"
export * from "./diagnosis-grounding.js"
export * from "./issue-close-safe.js"
export * from "./command-execution-source-verified.js"
export * from "./merge-deploy-gate.js"
export * from "./fleet-liveness.js"
export * from "./virtual-merge-queue.js"

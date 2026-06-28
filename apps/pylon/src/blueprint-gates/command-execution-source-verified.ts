/**
 * Blueprint Signature 4 — `command-execution-source-verified`
 *
 * No command is recommended without reading its source.
 *
 * The evaluator now lives in the cross-consumer Blueprint contract home,
 * `@openagentsinc/blueprint-contracts`, so this Pylon gate module and the
 * openagents.com Worker operator loop import + apply the SAME pure function
 * instead of keeping two copies that can drift. This file preserves the original
 * public API of the gate by re-exporting it; consumers (`./index.ts`, the gate
 * test) are unchanged.
 *
 * See `docs/artanis/2026-06-28-blueprint-signature-governed-autonomous-ops.md`
 * for the gate's ordered-predicate state machine
 * (UNVERIFIED → SOURCE_READ → FLAGS_VERIFIED → RUNTIME_CONFIRMED → SAFE_TO_PROPOSE).
 */

export {
  COMMAND_SOURCE_VERIFIED_STATES,
  COMMAND_SOURCE_VERIFIED_EVIDENCE,
  parseCommandFlags,
  evaluateCommandSourceVerified,
} from "@openagentsinc/blueprint-contracts"

export type {
  CommandSourceVerifiedState,
  CommandSourceVerifiedEvidenceRef,
  CommandSourceVerifiedInputs,
  CommandSourceVerifiedResult,
} from "@openagentsinc/blueprint-contracts"

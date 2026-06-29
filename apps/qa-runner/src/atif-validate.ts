// ATIF-v1.7 validator (epic #6174): the producer-facing (permissive) Effect-Schema
// decoder + structural invariant checks for an ATIF `Trajectory`.
//
// The validator now lives ONCE in the shared in-repo package `@openagentsinc/atif`
// (subpath `/validate`) (#6207) so producers (qa-runner) and consumers share one
// definition. This module re-exports it so existing `./atif-validate` imports keep
// working unchanged.

export {
  AtifTrajectorySchema,
  AtifValidationError,
  type AtifValidationResult,
  assertValidAtif,
  validateAtif,
} from "@openagentsinc/atif/validate";

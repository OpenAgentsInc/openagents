/**
 * ATIF — Agent Trajectory Interchange Format (#6207, epic #6206).
 *
 * The canonical pinned public-safe ATIF-v1.7 trace schema, structural validator,
 * and value-based public-safety tripwire now live in the shared in-repo package
 * `@openagentsinc/atif` (subpath `/trace`) so producers (qa-runner), this ingest
 * API, and the `/trace/{uuid}` page agree on ONE definition. This module
 * re-exports that canonical surface so existing imports of
 * `./atif-trace-schema` keep working unchanged.
 *
 * Reference: `projects/repos/harbor/rfcs/0001-trajectory-format.md` (ATIF-v1.7);
 * spec `docs/traces/README.md`.
 */

export {
  ATIF_PINNED_SCHEMA_VERSION,
  AtifAgent,
  AtifFinalMetrics,
  AtifObservation,
  AtifObservationResult,
  AtifStep,
  AtifStepMetrics,
  AtifToolCall,
  AtifTrajectory,
  type AtifTripwireFinding,
  type AtifValidationIssue,
  atifTraceTripwire,
  decodeAtifTrajectorySync,
  encodeAtifTrajectory,
  TraceVisibility,
  validateAtifTrajectory,
} from '@openagentsinc/atif/trace'

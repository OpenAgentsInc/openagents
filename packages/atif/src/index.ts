// @openagentsinc/atif — one canonical in-repo home for the ATIF (Agent
// Trajectory Interchange Format, ATIF-v1.7) shape so producers (qa-runner) and
// consumers (the trace ingest API, the /trace page) stop maintaining parallel
// definitions (#6207, epic #6206).
//
// Two surfaces, one source each:
//   - `@openagentsinc/atif/trace`    — the strict ingest/store SCHEMA
//     (`AtifTrajectory` Effect-Schema class), the structural `validateAtifTrajectory`,
//     and the value-based public-safety `atifTraceTripwire`. The canonical pinned
//     trace contract the API worker re-exports.
//   - `@openagentsinc/atif/emit`     — the producer-facing dependency-free TS
//     types a trajectory emitter builds (string `message`, open `Json` args).
//   - `@openagentsinc/atif/validate` — the producer-facing (permissive)
//     Effect-Schema validator (`validateAtif`/`assertValidAtif`).
//
// The barrel re-exports the canonical TRACE surface plus the (non-colliding)
// validator names. Emitter TS types collide by name with the trace schema
// classes (both expose `AtifTrajectory`, `AtifStep`, ...), so import those from
// the `/emit` subpath directly.

export * from "./trace-schema.ts"
export { ATIF_PINNED_SCHEMA_VERSION as ATIF_TRACE_SCHEMA_VERSION } from "./trace-schema.ts"
export * from "./redaction.ts"

export {
  AtifTrajectorySchema,
  AtifValidationError,
  type AtifValidationResult,
  validateAtif,
  assertValidAtif,
} from "./validate.ts"

// Firm reason: 1 (Effect UI / Effect v4 browser and web-server prototype for docs/effect/half-day-sprint.md)
import { Schema } from "effect"

export const StartTurnRequest = Schema.Struct({
  task: Schema.String,
  cwd: Schema.optional(Schema.String),
  gated: Schema.optional(Schema.Boolean),
  local: Schema.optional(Schema.Boolean),
  model: Schema.optional(Schema.String),
  effort: Schema.optional(Schema.String),
  mode: Schema.optional(Schema.String)
})
export type StartTurnRequest = typeof StartTurnRequest.Type

export const ApproveRequest = Schema.Struct({
  answer: Schema.Boolean,
  ticket: Schema.Number,
  why: Schema.optional(Schema.String),
  option: Schema.optional(Schema.String)
})
export type ApproveRequest = typeof ApproveRequest.Type

export const SteerRequest = Schema.Struct({
  steer: Schema.String,
  id: Schema.optional(Schema.String),
  turn: Schema.optional(Schema.Number)
})
export type SteerRequest = typeof SteerRequest.Type

export const StopRequest = Schema.Struct({
  interrupt: Schema.optional(Schema.Boolean),
  resubmit: Schema.optional(Schema.Boolean)
})
export type StopRequest = typeof StopRequest.Type

export const WorkerAuthStatus = Schema.Struct({
  binary: Schema.String,
  source: Schema.String,
  ready: Schema.Boolean,
  label: Schema.String
})
export type WorkerAuthStatus = typeof WorkerAuthStatus.Type

export const HealthResponse = Schema.Struct({
  ok: Schema.Boolean,
  worker: WorkerAuthStatus,
  activeSessions: Schema.Number
})
export type HealthResponse = typeof HealthResponse.Type

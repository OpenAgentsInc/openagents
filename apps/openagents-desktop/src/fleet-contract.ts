/**
 * The only mutable renderer -> host capability in the first Fleet slice.
 * This contract intentionally contains an objective only: repository pins,
 * accounts, verifier selection, and FleetRun authority remain Pylon/server
 * responsibilities and never cross an untrusted renderer boundary.
 */
import { Exit, Schema } from "@effect-native/core/effect"

export const FleetStageChannel = "openagents-desktop/fleet-stage" as const

export const FleetStageRequestSchema = Schema.Struct({
  objective: Schema.String,
})

export type FleetStageRequest = typeof FleetStageRequestSchema.Type

export type FleetStageResult = Readonly<{
  state: "accepted" | "rejected" | "unavailable"
  message: string
  intentStatus: string | null
}>

export const decodeFleetStageRequest = (value: unknown): FleetStageRequest | null => {
  const decoded = Schema.decodeUnknownExit(FleetStageRequestSchema)(value)
  if (!Exit.isSuccess(decoded)) return null
  const objective = decoded.value.objective.trim()
  return objective.length > 0 && objective.length <= 1_000 ? { objective } : null
}

export const unavailableFleetStageResult = (): FleetStageResult => ({
  state: "unavailable",
  message: "Local Pylon control is unavailable. No fleet work was dispatched.",
  intentStatus: null,
})

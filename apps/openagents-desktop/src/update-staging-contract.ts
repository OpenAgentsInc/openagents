import { Schema } from "effect"

import type { UpdateChannel } from "./update-contract.ts"
import type { DesktopUpdateProjection } from "./update-staging-host.ts"

export const DesktopUpdateStagingChannel = "openagents-desktop/update-staging" as const
export const DesktopUpdateStagingActionSchema = Schema.Struct({
  action: Schema.Literals(["snapshot", "check", "download", "open_installer", "apply", "rollback"]),
})
const UpdateChannelSchema = Schema.Literals(["stable", "rc"])
export const DesktopUpdateProjectionSchema = Schema.Struct({
  phase: Schema.Literals(["current", "checking", "available", "downloading", "staged", "applying", "restarting", "rollback_available", "rolling_back", "rejected"]),
  channel: UpdateChannelSchema,
  installedVersion: Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40)),
  candidateVersion: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40))),
  rollbackVersion: Schema.NullOr(Schema.String.check(Schema.isMinLength(1), Schema.isMaxLength(40))),
  reason: Schema.NullOr(Schema.String.check(Schema.isMaxLength(120))),
})

// Effect Schema decoder services are erased at this fixed IPC perimeter; each
// call below immediately pins its concrete schema and result type.
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const decode = <A>(schema: any, value: unknown): A | null => {
  const result = Schema.decodeUnknownExit(schema)(value)
  return result._tag === "Success" ? result.value as A : null
}

export const decodeDesktopUpdateStagingAction = (value: unknown) =>
  decode<typeof DesktopUpdateStagingActionSchema.Type>(DesktopUpdateStagingActionSchema, value)
export const decodeDesktopUpdateProjection = (value: unknown): DesktopUpdateProjection | null =>
  decode(DesktopUpdateProjectionSchema, value)
export const emptyDesktopUpdateProjection = (): DesktopUpdateProjection => ({
  phase: "rejected",
  channel: "rc" satisfies UpdateChannel,
  installedVersion: "0.0.0",
  candidateVersion: null,
  rollbackVersion: null,
  reason: "update_host_unavailable",
})

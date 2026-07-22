import { Schema } from "effect"

import {
  PortableAttachmentSchema,
  PortableCodingSessionSchema,
  PortableCommandProjectionSchema,
  PortableSessionCommandSchema,
  PortableTargetDirectoryProjectionSchema,
} from "@openagentsinc/portable-session-contract"

export const DesktopIdePortableSnapshotChannel = "desktop:ide-portable-snapshot"
export const DesktopIdePortableCommandChannel = "desktop:ide-portable-command"

const PortableClientRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(512),
)

export const IdePortableClientSnapshotSchema = Schema.Struct({
  status: Schema.Struct({
    phase: Schema.Literals([
      "unavailable", "idle", "bootstrapping", "catching_up", "live",
      "must_refetch", "denied",
    ]),
    cursor: Schema.NullOr(Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0))),
    pendingCommandCount: Schema.Number.check(Schema.isInt(), Schema.isGreaterThanOrEqualTo(0)),
  }),
  sessions: Schema.Array(PortableCodingSessionSchema).check(Schema.isMaxLength(512)),
  targetDirectories: Schema.Array(PortableTargetDirectoryProjectionSchema).check(Schema.isMaxLength(512)),
  attachments: Schema.Array(PortableAttachmentSchema).check(Schema.isMaxLength(2_048)),
  commands: Schema.Array(PortableCommandProjectionSchema).check(Schema.isMaxLength(2_048)),
  issues: Schema.Array(Schema.Struct({
    code: Schema.Literals(["malformed", "entity_ref_mismatch", "owner_scope_mismatch", "orphaned"]),
    affectedRef: PortableClientRefSchema,
  })).check(Schema.isMaxLength(4_096)),
})
export type IdePortableClientSnapshot = typeof IdePortableClientSnapshotSchema.Type

export const IdePortableClientCommandResultSchema = Schema.Union([
  Schema.Struct({
    _tag: Schema.Literal("Requested"),
    mutationRef: PortableClientRefSchema,
  }),
  Schema.Struct({
    _tag: Schema.Literal("Refused"),
    reason: Schema.Literals(["invalid_input", "unavailable", "request_failed"]),
  }),
])
export type IdePortableClientCommandResult = typeof IdePortableClientCommandResultSchema.Type

export const emptyIdePortableClientSnapshot = (): IdePortableClientSnapshot =>
  IdePortableClientSnapshotSchema.make({
    status: { phase: "unavailable", cursor: null, pendingCommandCount: 0 },
    sessions: [],
    targetDirectories: [],
    attachments: [],
    commands: [],
    issues: [],
  })

const decodeSnapshot = Schema.decodeUnknownOption(IdePortableClientSnapshotSchema)
const decodeCommand = Schema.decodeUnknownOption(PortableSessionCommandSchema)
const decodeResult = Schema.decodeUnknownOption(IdePortableClientCommandResultSchema)

export const decodeIdePortableClientSnapshot = (value: unknown): IdePortableClientSnapshot | null => {
  const decoded = decodeSnapshot(value)
  return decoded._tag === "Some" ? decoded.value : null
}

export const decodeIdePortableClientCommand = (value: unknown): typeof PortableSessionCommandSchema.Type | null => {
  const decoded = decodeCommand(value)
  return decoded._tag === "Some" ? decoded.value : null
}

export const decodeIdePortableClientCommandResult = (value: unknown): IdePortableClientCommandResult | null => {
  const decoded = decodeResult(value)
  return decoded._tag === "Some" ? decoded.value : null
}

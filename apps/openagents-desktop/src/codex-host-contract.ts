import { Exit, Schema } from "@effect-native/core/effect"

export const CodexHostSnapshotChannel = "openagents:codex-host:snapshot" as const
export const CodexHostRequestChannel = "openagents:codex-host:request" as const

const Path = Schema.String
const Command = Schema.Array(Schema.String)
export const CodexHostRequestSchema = Schema.Union([
  Schema.Struct({ operation: Schema.Literal("fs_read"), path: Path }),
  Schema.Struct({ operation: Schema.Literal("fs_write"), path: Path, dataBase64: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("fs_mkdir"), path: Path, recursive: Schema.Boolean }),
  Schema.Struct({ operation: Schema.Literal("fs_list"), path: Path }),
  Schema.Struct({ operation: Schema.Literal("fs_metadata"), path: Path }),
  Schema.Struct({ operation: Schema.Literal("fs_remove"), path: Path, recursive: Schema.Boolean }),
  Schema.Struct({ operation: Schema.Literal("fs_copy"), sourcePath: Path, destinationPath: Path, recursive: Schema.Boolean }),
  Schema.Struct({ operation: Schema.Literal("fs_watch"), path: Path }),
  Schema.Struct({ operation: Schema.Literal("fs_unwatch"), watchId: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("command_exec"), command: Command, cwd: Schema.optional(Path), timeoutMs: Schema.optional(Schema.Number), tty: Schema.optional(Schema.Boolean), rows: Schema.optional(Schema.Number), cols: Schema.optional(Schema.Number) }),
  Schema.Struct({ operation: Schema.Literal("command_write"), processId: Schema.String, deltaBase64: Schema.String, closeStdin: Schema.optional(Schema.Boolean) }),
  Schema.Struct({ operation: Schema.Literal("command_resize"), processId: Schema.String, rows: Schema.Number, cols: Schema.Number }),
  Schema.Struct({ operation: Schema.Literal("command_terminate"), processId: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("search_fuzzy"), query: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("search_start") }),
  Schema.Struct({ operation: Schema.Literal("search_update"), sessionId: Schema.String, query: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("search_stop"), sessionId: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("external_detect") }),
  Schema.Struct({ operation: Schema.Literal("external_histories") }),
  Schema.Struct({ operation: Schema.Literal("external_import"), migrationItems: Schema.Array(Schema.Unknown), source: Schema.NullOr(Schema.String) }),
  Schema.Struct({ operation: Schema.Literal("windows_readiness") }),
  Schema.Struct({ operation: Schema.Literal("windows_setup"), mode: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("feedback_upload"), classification: Schema.String, reason: Schema.NullOr(Schema.String), attachments: Schema.Array(Path), includeLogs: Schema.Boolean }),
])
export type CodexHostRequest = typeof CodexHostRequestSchema.Type

const bounded = (request: CodexHostRequest): boolean => {
  const strings: string[] = []
  const visit = (value: unknown): void => { if (typeof value === "string") strings.push(value); else if (Array.isArray(value)) value.forEach(visit); else if (typeof value === "object" && value !== null) Object.values(value).forEach(visit) }
  visit(request)
  return strings.every(value => value.length <= 8_388_608) && (request.operation !== "command_exec" || request.command.length <= 128) && (request.operation !== "feedback_upload" || request.attachments.length <= 8)
}

export const decodeCodexHostRequest = (value: unknown): CodexHostRequest | null => {
  const decoded = Schema.decodeUnknownExit(CodexHostRequestSchema)(value)
  return Exit.isSuccess(decoded) && bounded(decoded.value) ? decoded.value : null
}

import { Exit, Schema } from "@effect-native/core/effect"

export const CodexExperimentalSnapshotChannel = "openagents:codex-experimental:snapshot" as const
export const CodexExperimentalRequestChannel = "openagents:codex-experimental:request" as const
const Confirmed = Schema.Literal(true)
export const CodexExperimentalRequestSchema = Schema.Union([
  Schema.Struct({ operation: Schema.Literal("environment_add"), environmentId: Schema.String, execServerUrl: Schema.String, connectTimeoutMs: Schema.optional(Schema.Number), confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("environment_reconnect"), environmentId: Schema.String, execServerUrl: Schema.String, confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("environment_target"), environmentRef: Schema.String, cwd: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("process_spawn"), command: Schema.Array(Schema.String), cwd: Schema.String, tty: Schema.optional(Schema.Boolean), rows: Schema.optional(Schema.Number), cols: Schema.optional(Schema.Number), timeoutMs: Schema.optional(Schema.Number), confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("process_write"), processRef: Schema.String, dataBase64: Schema.String, closeStdin: Schema.optional(Schema.Boolean) }),
  Schema.Struct({ operation: Schema.Literal("process_resize"), processRef: Schema.String, rows: Schema.Number, cols: Schema.Number }),
  Schema.Struct({ operation: Schema.Literal("process_kill"), processRef: Schema.String, confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("terminal_list"), threadId: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("terminal_clean"), threadId: Schema.String, confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("terminal_terminate"), threadId: Schema.String, processRef: Schema.String, confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("realtime_start"), threadId: Schema.String, outputModality: Schema.Literals(["text", "audio"]), transport: Schema.Union([Schema.Struct({ type: Schema.Literal("websocket") }), Schema.Struct({ type: Schema.Literal("webrtc"), sdp: Schema.String })]), voice: Schema.optional(Schema.String), confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("realtime_audio"), threadId: Schema.String, audio: Schema.Struct({ data: Schema.String, numChannels: Schema.Number, sampleRate: Schema.Number, samplesPerChannel: Schema.optional(Schema.Number) }), confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("realtime_text"), threadId: Schema.String, text: Schema.String, role: Schema.optional(Schema.Literals(["user", "developer", "assistant"])) }),
  Schema.Struct({ operation: Schema.Literal("realtime_speech"), threadId: Schema.String, text: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("realtime_stop"), threadId: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("realtime_voices") }),
  Schema.Struct({ operation: Schema.Literal("remote_enable"), confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("remote_disable"), confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("remote_status") }),
  Schema.Struct({ operation: Schema.Literal("remote_pair"), manualCode: Schema.Boolean, confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("remote_pair_status"), pairingRef: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("remote_clients"), environmentId: Schema.String }),
  Schema.Struct({ operation: Schema.Literal("remote_revoke"), environmentId: Schema.String, clientRef: Schema.String, confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("memory_reset"), confirmation: Schema.Literal("RESET"), confirmed: Confirmed }),
  Schema.Struct({ operation: Schema.Literal("thread_elicitation"), threadId: Schema.String, direction: Schema.Literals(["increment", "decrement"]), confirmed: Confirmed }),
])
export type CodexExperimentalRequest = typeof CodexExperimentalRequestSchema.Type
export const decodeCodexExperimentalRequest = (value: unknown): CodexExperimentalRequest | null => {
  const decoded = Schema.decodeUnknownExit(CodexExperimentalRequestSchema)(value)
  if (!Exit.isSuccess(decoded)) return null
  const request = decoded.value
  if (request.operation === "process_spawn" && (request.command.length === 0 || request.command.length > 128)) return null
  if (request.operation === "realtime_audio" && Buffer.from(request.audio.data, "base64").length > 2_097_152) return null
  return request
}

import { Effect } from "effect"
import {
  khalaToolError,
  khalaToolOk,
  type KhalaProcessEvent,
  type KhalaProcessSessionResult,
  type KhalaToolArtifact,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "./index.js"

export interface KhalaWriteStdinToolOptions {
  readonly maxCaptureBytes?: number
}

export const writeStdinToolDefinition: KhalaToolDefinition = {
  authority: "process_stdin",
  availability: ["coding", "owner_local_full"],
  description: "Write to or poll an interactive process session created by exec_command.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      chars: {
        description: "Characters to write. Omit or pass an empty string to poll recent output.",
        type: "string",
      },
      max_output_tokens: {
        description: "Approximate token budget for the tail-oriented model preview.",
        minimum: 1,
        type: "integer",
      },
      session_id: {
        description: "Interactive process session id returned by exec_command.",
        type: "string",
      },
      yield_time_ms: {
        description: "How long to wait for new output before returning.",
        minimum: 1,
        type: "integer",
      },
    },
    required: ["session_id"],
    type: "object",
  },
  internalId: "khala.process.stdin",
  label: "Write Stdin",
  name: "write_stdin",
  outputSchema: {
    additionalProperties: false,
    properties: {
      cancelled: { type: "boolean" },
      exitCode: { type: ["integer", "null"] },
      sessionId: { type: "string" },
      stderrBytes: { type: "integer" },
      stdoutBytes: { type: "integer" },
    },
    required: ["sessionId", "exitCode", "cancelled", "stdoutBytes", "stderrBytes"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Write to or poll an existing interactive process session.",
  promptGuidelines: [
    "Use only with a session_id returned by exec_command.",
    "Pass empty chars or omit chars to poll output.",
    "Use Ctrl-C characters only when intentionally cancelling a session.",
  ],
  renderer: { kind: "terminal_stdin", rendererRef: "khala.renderer.terminal_stdin.v1" },
}

export function createWriteStdinTool(options: KhalaWriteStdinToolOptions = {}): RegisteredKhalaTool {
  return {
    definition: writeStdinToolDefinition,
    execute: (input, context) => executeWriteStdinTool(input, context, options),
  }
}

type WriteStdinInput = Readonly<{
  chars?: string
  maxOutputTokens: number
  sessionId: string
  yieldTimeMs: number
}>

function executeWriteStdinTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaWriteStdinToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeWriteStdinInput(input)
      const result = await Effect.runPromise(
        context.services.process.writeStdin({
          ...(args.chars === undefined ? {} : { chars: args.chars }),
          khalaSessionId: context.invocation.sessionId,
          maxCaptureBytes: options.maxCaptureBytes ?? 256 * 1024,
          sessionId: args.sessionId,
          yieldTimeMs: args.yieldTimeMs,
        }),
      )
      return await renderWriteStdinResult(args, result, context)
    } catch (error) {
      return khalaToolError("write_stdin_failed", runtimeErrorReason(error))
    }
  })
}

function runtimeErrorReason(error: unknown): string {
  if (error instanceof Error && error.message.length > 0) return error.message
  if (typeof error === "object" && error !== null && "reason" in error && typeof error.reason === "string") {
    return error.reason
  }
  return String(error)
}

async function renderWriteStdinResult(
  args: WriteStdinInput,
  result: KhalaProcessSessionResult,
  context: KhalaToolExecuteContext,
): Promise<KhalaToolResult> {
  const stdoutBytes = Buffer.byteLength(result.stdout, "utf8")
  const stderrBytes = Buffer.byteLength(result.stderr, "utf8")
  const combined = [
    result.stdout.length > 0 ? `$ stdout\n${result.stdout}` : "",
    result.stderr.length > 0 ? `$ stderr\n${result.stderr}` : "",
  ].filter(Boolean).join("\n")
  const preview = tailPreview(combined.length === 0 ? "(no output)" : combined, args.maxOutputTokens)
  const shouldSpill = preview.truncated || result.stdoutTruncated || result.stderrTruncated
  const artifacts: KhalaToolArtifact[] = []
  if (shouldSpill) {
    const artifact = await Effect.runPromise(
      context.services.outputStore.writeArtifact({
        bytes: Buffer.from(combined, "utf8"),
        mediaType: "text/plain; charset=utf-8",
        summary: `write_stdin output for ${args.sessionId}`,
      }),
    )
    artifacts.push(artifact)
  }
  const closed = result.exitCode !== null || result.cancelled || result.timedOut
  const failed = result.cancelled || result.timedOut || (result.exitCode !== null && result.exitCode !== 0)
  const label = result.cancelled
    ? `Session ${args.sessionId} cancelled`
    : result.timedOut
      ? `Session ${args.sessionId} timed out`
      : closed
        ? `Session ${args.sessionId} exited ${result.exitCode ?? "unknown"}`
        : `Session ${args.sessionId} updated`
  const ok = khalaToolOk({
    artifacts,
    modelText: `${label}\n${preview.text}${preview.truncated ? "\n[stdin output truncated; see private artifact]" : ""}`,
    privateDataRefs: artifacts.map(artifact => artifact.artifactRef),
    publicSummary:
      `${label}: stdout ${stdoutBytes} bytes, stderr ${stderrBytes} bytes` +
      `${artifacts.length > 0 ? `, ${artifacts.length} private artifact` : ""}.`,
    ui: {
      artifacts,
      cancelled: result.cancelled,
      events: result.events.map(eventToUi),
      exitCode: result.exitCode,
      kind: "terminal_stdin",
      maxOutputTokens: args.maxOutputTokens,
      sessionId: args.sessionId,
      stderrBytes,
      stderrTruncated: result.stderrTruncated,
      stdoutBytes,
      stdoutTruncated: result.stdoutTruncated,
      timedOut: result.timedOut,
      wroteBytes: args.chars === undefined ? 0 : Buffer.byteLength(args.chars, "utf8"),
    },
    publicSafety: "private",
  })
  return failed ? { ...ok, status: "failed" } : ok
}

function decodeWriteStdinInput(input: Readonly<Record<string, unknown>>): WriteStdinInput {
  const sessionId = typeof input.session_id === "string" ? input.session_id.trim() : ""
  if (sessionId.length === 0) throw new Error("write_stdin requires session_id")
  const chars = typeof input.chars === "string" ? input.chars : undefined
  const maxOutputTokens = optionalPositiveInteger(input.max_output_tokens, "max_output_tokens") ?? 6_000
  const yieldTimeMs = optionalPositiveInteger(input.yield_time_ms, "yield_time_ms") ?? (chars === undefined || chars.length === 0 ? 500 : 250)
  return {
    ...(chars === undefined ? {} : { chars }),
    maxOutputTokens,
    sessionId,
    yieldTimeMs,
  }
}

function eventToUi(event: KhalaProcessEvent): unknown {
  return {
    kind: event.channel === "stdin" ? "stdin_chunk" : event.channel === "stdout" ? "stdout_chunk" : "stderr_chunk",
    payload: {
      channel: event.channel,
      text: event.text,
      timestampMs: event.timestampMs,
    },
  }
}

function tailPreview(text: string, maxOutputTokens: number): Readonly<{ text: string; truncated: boolean }> {
  const maxChars = Math.max(256, maxOutputTokens * 4)
  if (text.length <= maxChars) return { text, truncated: false }
  return {
    text: text.slice(text.length - maxChars),
    truncated: true,
  }
}

function optionalPositiveInteger(value: unknown, field: string): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < 1) {
    throw new Error(`write_stdin ${field} must be a positive integer`)
  }
  return Number(value)
}

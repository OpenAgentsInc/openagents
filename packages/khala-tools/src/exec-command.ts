import { realpath, stat } from "node:fs/promises"
import { isAbsolute, relative, resolve } from "node:path"
import { Effect } from "effect"
import {
  khalaToolDenied,
  khalaToolError,
  khalaToolOk,
  type KhalaPermissionRequest,
  type KhalaProcessEvent,
  type KhalaProcessExecResult,
  type KhalaToolArtifact,
  type KhalaToolDefinition,
  type KhalaToolExecuteContext,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "./index.js"

export interface KhalaExecCommandToolOptions {
  readonly maxCaptureBytes?: number
  readonly maxTimeoutMs?: number
}

export const execCommandToolDefinition: KhalaToolDefinition = {
  authority: "shell",
  availability: ["coding", "owner_local_full"],
  description: "Run a bounded local command through the Khala process service.",
  executionMode: "local",
  inputSchema: {
    additionalProperties: false,
    properties: {
      argv: {
        description: "Future argv form. When set, argv[0] is executed directly instead of shell command text.",
        items: { type: "string" },
        type: "array",
      },
      cancel_after_ms: {
        description: "Testing/development cancellation deadline in milliseconds.",
        minimum: 1,
        type: "integer",
      },
      cmd: {
        description: "Shell command text.",
        type: "string",
      },
      command: {
        description: "Alias for cmd.",
        type: "string",
      },
      max_output_tokens: {
        description: "Approximate token budget for the tail-oriented model preview.",
        minimum: 1,
        type: "integer",
      },
      timeout_ms: {
        description: "Maximum command runtime in milliseconds.",
        minimum: 1,
        type: "integer",
      },
      tty: {
        description: "Reserved for future PTY sessions; V1 one-shot execution records the request but does not allocate a PTY.",
        type: "boolean",
      },
      workdir: {
        description: "Workspace-relative working directory or approved absolute directory path.",
        type: "string",
      },
      yield_time_ms: {
        description: "Reserved UI yield hint for future background/session mode.",
        minimum: 1,
        type: "integer",
      },
    },
    type: "object",
  },
  internalId: "khala.process.exec",
  label: "Run Command",
  name: "exec_command",
  outputSchema: {
    additionalProperties: false,
    properties: {
      cancelled: { type: "boolean" },
      exitCode: { type: ["integer", "null"] },
      stderrBytes: { type: "integer" },
      stdoutBytes: { type: "integer" },
      timedOut: { type: "boolean" },
    },
    required: ["exitCode", "timedOut", "cancelled", "stdoutBytes", "stderrBytes"],
    type: "object",
  },
  permissionMode: "allow",
  prompt: "Run a command in the workspace. Use filesystem tools for routine reads/search/edits.",
  promptGuidelines: [
    "Prefer read, ls, glob, and grep before spending shell authority.",
    "Use timeout_ms for commands that may hang.",
    "Do not assume sandboxing unless the UI explicitly reports an enforced sandbox.",
  ],
  renderer: { kind: "terminal_exec", rendererRef: "khala.renderer.terminal_exec.v1" },
}

export function createExecCommandTool(options: KhalaExecCommandToolOptions = {}): RegisteredKhalaTool {
  return {
    definition: execCommandToolDefinition,
    execute: (input, context) => executeExecCommandTool(input, context, options),
  }
}

type ExecCommandInput = Readonly<{
  argv?: ReadonlyArray<string>
  cancelAfterMs?: number
  command: string
  maxOutputTokens: number
  timeoutMs: number
  tty: boolean
  workdir: string
  yieldTimeMs?: number
}>

function executeExecCommandTool(
  input: Readonly<Record<string, unknown>>,
  context: KhalaToolExecuteContext,
  options: KhalaExecCommandToolOptions,
): Effect.Effect<KhalaToolResult, never> {
  return Effect.promise(async () => {
    try {
      const args = decodeExecInput(input, options)
      const cwd = await resolveWorkdir(args.workdir, context)
      if (cwd._tag === "denied") {
        return khalaToolDenied("exec_external_cwd_denied", "command working directory outside the workspace was denied")
      }
      const shellDecision = await Effect.runPromise(
        context.services.permission.decide(shellPermission(args, cwd.displayPath, context)),
      )
      if (shellDecision === "deny") return khalaToolDenied("exec_shell_denied", "shell command approval was denied")
      if (networkLikeCommand(args.command)) {
        const networkDecision = await Effect.runPromise(
          context.services.permission.decide(networkPermission(args, cwd.displayPath, context)),
        )
        if (networkDecision === "deny") return khalaToolDenied("exec_network_denied", "network command approval was denied")
      }

      const result = await Effect.runPromise(
        args.tty
          ? context.services.process.startSession({
              ...(args.argv === undefined ? {} : { argv: args.argv }),
              ...(args.cancelAfterMs === undefined ? {} : { cancelAfterMs: args.cancelAfterMs }),
              command: args.command,
              cwd: cwd.realPath,
              khalaSessionId: context.invocation.sessionId,
              maxCaptureBytes: options.maxCaptureBytes ?? 256 * 1024,
              timeoutMs: args.timeoutMs,
              yieldTimeMs: args.yieldTimeMs ?? 250,
            })
          : context.services.process.execCommand({
              ...(args.argv === undefined ? {} : { argv: args.argv }),
              ...(args.cancelAfterMs === undefined ? {} : { cancelAfterMs: args.cancelAfterMs }),
              command: args.command,
              cwd: cwd.realPath,
              maxCaptureBytes: options.maxCaptureBytes ?? 256 * 1024,
              timeoutMs: args.timeoutMs,
            }),
      )
      return await renderExecResult(args, cwd.displayPath, result, context)
    } catch (error) {
      return khalaToolError("exec_command_failed", error instanceof Error ? error.message : String(error))
    }
  })
}

async function renderExecResult(
  args: ExecCommandInput,
  cwd: string,
  result: KhalaProcessExecResult,
  context: KhalaToolExecuteContext,
): Promise<KhalaToolResult> {
  const sessionId = "sessionId" in result ? result.sessionId : undefined
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
        summary: `exec_command output for ${args.command}`,
      }),
    )
    artifacts.push(artifact)
  }

  const runningSession = sessionId !== undefined && result.exitCode === null && !result.timedOut && !result.cancelled
  const failed = !runningSession && (result.exitCode !== 0 || result.timedOut || result.cancelled)
  const header = result.timedOut
    ? "Command timed out"
    : result.cancelled
      ? "Command cancelled"
      : runningSession
        ? `Started session ${sessionId}`
        : `Command exited ${result.exitCode ?? "unknown"}`
  const modelText = `${header}\n${preview.text}${preview.truncated ? "\n[exec output truncated; see private artifact]" : ""}`
  const ok = khalaToolOk({
    artifacts,
    modelText,
    privateDataRefs: artifacts.map(artifact => artifact.artifactRef),
    publicSummary:
      `${header}: stdout ${stdoutBytes} bytes, stderr ${stderrBytes} bytes` +
      `${artifacts.length > 0 ? `, ${artifacts.length} private artifact` : ""}.`,
    ui: {
      artifacts,
      cancelled: result.cancelled,
      command: args.command,
      cwd,
      durationMs: result.durationMs,
      events: result.events.map(eventToUi),
      exitCode: result.exitCode,
      kind: "terminal_exec",
      maxOutputTokens: args.maxOutputTokens,
      sandbox: result.sandbox,
      stderrBytes,
      stderrTruncated: result.stderrTruncated,
      stdoutBytes,
      stdoutTruncated: result.stdoutTruncated,
      timedOut: result.timedOut,
      ...(sessionId === undefined ? {} : { sessionId }),
      ttyRequested: args.tty,
      yieldTimeMs: args.yieldTimeMs,
    },
    publicSafety: "private",
  })
  return failed ? { ...ok, status: "failed" } : ok
}

function decodeExecInput(input: Readonly<Record<string, unknown>>, options: KhalaExecCommandToolOptions): ExecCommandInput {
  const argv = decodeArgv(input.argv)
  const rawCommand = typeof input.cmd === "string"
    ? input.cmd
    : typeof input.command === "string"
      ? input.command
      : argv?.join(" ") ?? ""
  const command = rawCommand.trim()
  if (command.length === 0) throw new Error("exec_command requires cmd, command, or argv")
  const timeoutMs = Math.min(optionalPositiveInteger(input.timeout_ms, "timeout_ms") ?? 10_000, options.maxTimeoutMs ?? 120_000)
  const maxOutputTokens = optionalPositiveInteger(input.max_output_tokens, "max_output_tokens") ?? 6_000
  const cancelAfterMs = optionalPositiveInteger(input.cancel_after_ms, "cancel_after_ms")
  const yieldTimeMs = optionalPositiveInteger(input.yield_time_ms, "yield_time_ms")
  const tty = optionalBoolean(input.tty, "tty") ?? false
  const workdir = typeof input.workdir === "string" && input.workdir.trim().length > 0 ? input.workdir.trim() : "."
  return {
    ...(argv === undefined ? {} : { argv }),
    ...(cancelAfterMs === undefined ? {} : { cancelAfterMs }),
    command,
    maxOutputTokens,
    timeoutMs,
    tty,
    workdir,
    ...(yieldTimeMs === undefined ? {} : { yieldTimeMs }),
  }
}

function decodeArgv(value: unknown): ReadonlyArray<string> | undefined {
  if (value === undefined) return undefined
  if (!Array.isArray(value) || value.length === 0 || !value.every(item => typeof item === "string" && item.length > 0)) {
    throw new Error("exec_command argv must be a non-empty string array")
  }
  return value
}

async function resolveWorkdir(
  rawWorkdir: string,
  context: KhalaToolExecuteContext,
): Promise<Readonly<{ _tag: "ok"; displayPath: string; realPath: string }> | Readonly<{ _tag: "denied" }>> {
  const workspaceRoot = await realpath(context.services.workspace.workingDirectory)
  const candidate = isAbsolute(rawWorkdir) ? rawWorkdir : resolve(workspaceRoot, rawWorkdir)
  const target = await realpath(candidate)
  const info = await stat(target)
  if (!info.isDirectory()) throw new Error("exec_command workdir must be a directory")
  const inside = pathIsInside(workspaceRoot, target)
  if (!inside) {
    const decision = await Effect.runPromise(
      context.services.permission.decide(externalDirectoryPermission(rawWorkdir, context)),
    )
    if (decision === "deny") return { _tag: "denied" }
  }
  return {
    _tag: "ok",
    displayPath: inside ? toWorkspaceRelative(workspaceRoot, target) : rawWorkdir,
    realPath: target,
  }
}

function shellPermission(args: ExecCommandInput, cwd: string, context: KhalaToolExecuteContext): KhalaPermissionRequest {
  return {
    action: "shell",
    authorityMode: "local",
    publicSafety: "private",
    resources: [
      `cmd:${args.command}`,
      `cwd:${cwd}`,
      ...(destructiveCommand(args.command) ? ["risk:destructive"] : []),
      ...(args.tty ? ["pty:requested"] : []),
    ],
    saveScope: "once",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "exec_command",
    workingDirectory: context.services.workspace.workingDirectory,
  }
}

function networkPermission(args: ExecCommandInput, cwd: string, context: KhalaToolExecuteContext): KhalaPermissionRequest {
  return {
    action: "network",
    authorityMode: "local",
    publicSafety: "private",
    resources: [`cmd:${args.command}`, `cwd:${cwd}`],
    saveScope: "once",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "exec_command",
    workingDirectory: context.services.workspace.workingDirectory,
  }
}

function externalDirectoryPermission(rawPath: string, context: KhalaToolExecuteContext): KhalaPermissionRequest {
  return {
    action: "external_directory",
    authorityMode: "local",
    publicSafety: "private",
    resources: [rawPath],
    saveScope: "once",
    sessionId: context.invocation.sessionId,
    toolCallId: context.invocation.id,
    toolName: "exec_command",
    workingDirectory: context.services.workspace.workingDirectory,
  }
}

function eventToUi(event: KhalaProcessEvent): unknown {
  return {
    kind: event.channel === "stdout" ? "stdout_chunk" : "stderr_chunk",
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

function destructiveCommand(command: string): boolean {
  return /\brm\s+-[^;\n]*r[^;\n]*f\b/u.test(command) ||
    /\bgit\s+reset\s+--hard\b/u.test(command) ||
    /\bgit\s+clean\s+-[^;\n]*f/u.test(command) ||
    /\bchmod\s+-R\s+777\b/u.test(command)
}

function networkLikeCommand(command: string): boolean {
  return /\b(curl|wget|ssh|scp|rsync|git\s+clone|npm\s+install|bun\s+install|pnpm\s+install)\b/u.test(command)
}

function optionalPositiveInteger(value: unknown, field: string): number | undefined {
  if (value === undefined) return undefined
  if (!Number.isInteger(value) || Number(value) < 1) {
    throw new Error(`exec_command ${field} must be a positive integer`)
  }
  return Number(value)
}

function optionalBoolean(value: unknown, field: string): boolean | undefined {
  if (value === undefined) return undefined
  if (typeof value !== "boolean") throw new Error(`exec_command ${field} must be a boolean`)
  return value
}

function pathIsInside(root: string, target: string): boolean {
  const rel = relative(root, target)
  return rel === "" || !rel.startsWith("..") && !isAbsolute(rel)
}

function toWorkspaceRelative(root: string, target: string): string {
  const rel = relative(root, target)
  return rel === "" ? "." : rel.split("\\").join("/")
}

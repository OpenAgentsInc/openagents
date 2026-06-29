import { createHash } from "node:crypto"
import { lstat, readdir, readFile } from "node:fs/promises"
import { relative, resolve, sep } from "node:path"
import { Effect } from "effect"
import {
  makeAppleFmClient,
  makeAppleFmToolCallbackSession,
  type AppleFmToolDefinition,
  type AppleFmToolName,
} from "../../packages/runtime/src/index.js"
import type {
  ControlSessionExecutorInput,
  ControlSessionExecutorResult,
} from "./control-sessions.js"

export const APPLE_FM_LOCAL_SESSION_SAFE_TOOLS = [
  "read_file",
  "list_files",
  "code_search",
] as const satisfies ReadonlyArray<AppleFmToolName>

export type AppleFmLocalSessionSafeTool =
  typeof APPLE_FM_LOCAL_SESSION_SAFE_TOOLS[number]

export type AppleFmLocalSessionRunResult = Omit<
  ControlSessionExecutorResult,
  "devCheck"
>

const maxReadFileBytes = 32_768
const maxListEntries = 200
const maxSearchFiles = 400
const maxSearchMatches = 50
const maxSearchFileBytes = 131_072

export async function runAppleFmLocalControlSession(
  input: ControlSessionExecutorInput,
): Promise<AppleFmLocalSessionRunResult> {
  let eventCount = 0
  const emit = (message: string) => {
    eventCount += 1
    input.emit({
      phase: "composer_event",
      message,
      composerEventIndex: eventCount,
    })
  }

  const client = await Effect.runPromise(makeAppleFmClient({ env: input.env }))
  const readiness = await Effect.runPromise(client.requireReady())
  emit(`Apple FM local backend ready · ${readiness.profile.model}`)

  const toolSession = makeAppleFmToolCallbackSession({
    tools: makeAppleFmWorkspaceTools({
      cwd: input.cwd,
      allowedTools: APPLE_FM_LOCAL_SESSION_SAFE_TOOLS,
    }),
    maxModelRoundTrips: 8,
  })

  const result = await Effect.runPromise(
    client.streamSessionWithTools({
      prompt: input.objective,
      instructions: [
        "You are running entirely locally through Apple Foundation Models.",
        "Use only the projected read-only workspace tools.",
        "If the user asks for shell, writes, network access, secrets, deployment, or files outside the workspace, refuse that tool request.",
        "Prefer list_files, read_file, and code_search for repository inspection.",
        "Keep the final response concise.",
      ].join(" "),
      toolSession,
    }),
  )

  for (const event of result.events) {
    if (event.kind === "assistant_snapshot") {
      emit(`Apple FM assistant snapshot (${event.content?.length ?? 0} chars)`)
    }
    if (event.kind === "assistant_final_commit") {
      emit(`Apple FM local final answer retained as digest (${event.content?.length ?? 0} chars)`)
    }
  }

  for (const entry of result.toolTranscript) {
    emit(`Apple FM tool ${entry.toolName}: ${entry.status}`)
  }

  return {
    commandCount: result.toolTranscript.length,
    editedFileCount: 0,
    eventCount,
    executionMode: "local_bounded",
    externalSessionRef: stableDigestRef("session.pylon.apple_fm_bridge", result.bridgeSessionId),
    networkAccessEnabled: false,
    responseDigestRef:
      result.completion.text.length > 0
        ? stableDigestRef("digest.pylon.apple_fm.response", result.completion.text)
        : null,
    sandboxMode: "read-only",
    totalTokens: result.completion.usage.totalTokens ?? 0,
  }
}

export function makeAppleFmWorkspaceTools(input: {
  cwd: string
  allowedTools?: ReadonlyArray<AppleFmLocalSessionSafeTool>
}): ReadonlyArray<AppleFmToolDefinition> {
  const allowed = new Set(input.allowedTools ?? APPLE_FM_LOCAL_SESSION_SAFE_TOOLS)
  const tools: AppleFmToolDefinition[] = []

  if (allowed.has("read_file")) {
    tools.push({
      name: "read_file",
      description: "Read one UTF-8 text file inside the configured workspace.",
      inputSchema: {
        properties: {
          path: { type: "string", description: "Workspace-relative file path." },
        },
        required: ["path"],
        additionalProperties: false,
      },
      policy: "allow",
      execute: (toolInput) =>
        Effect.promise(() => readWorkspaceFile(input.cwd, stringField(toolInput, "path"))),
    })
  }

  if (allowed.has("list_files")) {
    tools.push({
      name: "list_files",
      description: "List files and directories inside the configured workspace.",
      inputSchema: {
        properties: {
          path: { type: "string", description: "Workspace-relative directory path." },
        },
        required: ["path"],
        additionalProperties: false,
      },
      policy: "allow",
      execute: (toolInput) =>
        Effect.promise(() => listWorkspaceFiles(input.cwd, stringField(toolInput, "path"))),
    })
  }

  if (allowed.has("code_search")) {
    tools.push({
      name: "code_search",
      description: "Search text files inside the configured workspace using a bounded literal query.",
      inputSchema: {
        properties: {
          query: { type: "string", description: "Literal text query." },
          path: { type: "string", description: "Optional workspace-relative directory path." },
        },
        required: ["query"],
        additionalProperties: false,
      },
      policy: "allow",
      execute: (toolInput) =>
        Effect.promise(() =>
          searchWorkspaceText(
            input.cwd,
            stringField(toolInput, "query"),
            optionalStringField(toolInput, "path") ?? ".",
          ),
        ),
    })
  }

  return tools
}

async function readWorkspaceFile(cwd: string, requestedPath: string): Promise<unknown> {
  const bounded = workspacePath(cwd, requestedPath)
  if (!bounded.ok) return bounded.output
  try {
    const info = await lstat(bounded.absolutePath)
    if (info.isSymbolicLink()) {
      return refused("blocker.pylon.apple_fm.tool.symlink_refused", bounded.relativePath)
    }
    if (!info.isFile()) {
      return refused("blocker.pylon.apple_fm.tool.not_file", bounded.relativePath)
    }
    const raw = await readFile(bounded.absolutePath, "utf8")
    return {
      ok: true,
      path: bounded.relativePath,
      content: raw.slice(0, maxReadFileBytes),
      truncated: raw.length > maxReadFileBytes,
    }
  } catch {
    return refused("blocker.pylon.apple_fm.tool.read_failed", bounded.relativePath)
  }
}

async function listWorkspaceFiles(cwd: string, requestedPath: string): Promise<unknown> {
  const bounded = workspacePath(cwd, requestedPath)
  if (!bounded.ok) return bounded.output
  try {
    const info = await lstat(bounded.absolutePath)
    if (info.isSymbolicLink()) {
      return refused("blocker.pylon.apple_fm.tool.symlink_refused", bounded.relativePath)
    }
    if (!info.isDirectory()) {
      return refused("blocker.pylon.apple_fm.tool.not_directory", bounded.relativePath)
    }
    const entries = await readdir(bounded.absolutePath, { withFileTypes: true })
    return {
      ok: true,
      path: bounded.relativePath,
      entries: entries
        .slice(0, maxListEntries)
        .map((entry) => `${entry.name}${entry.isDirectory() ? "/" : ""}`),
      truncated: entries.length > maxListEntries,
    }
  } catch {
    return refused("blocker.pylon.apple_fm.tool.list_failed", bounded.relativePath)
  }
}

async function searchWorkspaceText(
  cwd: string,
  query: string,
  requestedPath: string,
): Promise<unknown> {
  if (query.trim().length === 0) {
    return refused("blocker.pylon.apple_fm.tool.empty_query", ".")
  }
  const bounded = workspacePath(cwd, requestedPath)
  if (!bounded.ok) return bounded.output
  const matches: Array<{ path: string; line: number; preview: string }> = []
  let scannedFiles = 0

  async function visit(path: string): Promise<void> {
    if (scannedFiles >= maxSearchFiles || matches.length >= maxSearchMatches) return
    let info
    try {
      info = await lstat(path)
    } catch {
      return
    }
    if (info.isSymbolicLink()) return
    if (info.isDirectory()) {
      let entries
      try {
        entries = await readdir(path, { withFileTypes: true })
      } catch {
        return
      }
      for (const entry of entries) {
        if (entry.name === ".git" || entry.name === "node_modules") continue
        await visit(resolve(path, entry.name))
      }
      return
    }
    if (!info.isFile() || info.size > maxSearchFileBytes) return
    scannedFiles += 1
    let text
    try {
      text = await readFile(path, "utf8")
    } catch {
      return
    }
    const lowerQuery = query.toLowerCase()
    const lines = text.split(/\r?\n/)
    for (let index = 0; index < lines.length; index += 1) {
      const line = lines[index] ?? ""
      if (!line.toLowerCase().includes(lowerQuery)) continue
      matches.push({
        path: publicRelativePath(cwd, path),
        line: index + 1,
        preview: line.trim().slice(0, 200),
      })
      if (matches.length >= maxSearchMatches) break
    }
  }

  await visit(bounded.absolutePath)

  return {
    ok: true,
    query,
    path: bounded.relativePath,
    matches,
    scannedFiles,
    truncated: scannedFiles >= maxSearchFiles || matches.length >= maxSearchMatches,
  }
}

function workspacePath(
  cwd: string,
  requestedPath: string,
):
  | { ok: true; absolutePath: string; relativePath: string }
  | { ok: false; output: unknown } {
  const workspaceRoot = resolve(cwd)
  const requested = requestedPath.trim().length > 0 ? requestedPath : "."
  const absolutePath = resolve(workspaceRoot, requested)
  const relativePath = relative(workspaceRoot, absolutePath)
  const inside =
    relativePath === "" ||
    (!relativePath.startsWith("..") && !relativePath.includes(`..${sep}`))
  if (!inside) {
    return {
      ok: false,
      output: refused("blocker.pylon.apple_fm.tool.workspace_escape", requested),
    }
  }
  return {
    ok: true,
    absolutePath,
    relativePath: relativePath === "" ? "." : relativePath,
  }
}

function refused(blockerRef: string, path: string): unknown {
  return {
    ok: false,
    status: "refused",
    blockerRefs: [blockerRef],
    path,
  }
}

function stringField(input: Readonly<Record<string, unknown>>, key: string): string {
  const value = input[key]
  return typeof value === "string" ? value : ""
}

function optionalStringField(
  input: Readonly<Record<string, unknown>>,
  key: string,
): string | null {
  const value = input[key]
  return typeof value === "string" && value.trim().length > 0 ? value : null
}

function publicRelativePath(cwd: string, path: string): string {
  const rel = relative(resolve(cwd), resolve(path))
  return rel === "" ? "." : rel
}

function stableDigestRef(prefix: string, value: string): string {
  const digest = createHash("sha256").update(value).digest("hex").slice(0, 24)
  return `${prefix}.${digest}`
}

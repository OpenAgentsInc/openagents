import type { ChildProcessWithoutNullStreams } from "node:child_process"
import { spawn } from "node:child_process"
import { createInterface } from "node:readline"

export type CodexAppServerMessage = Readonly<Record<string, unknown>>
export type CodexAppServerRequest = Readonly<{
  id: string | number
  method: string
  params: unknown
}>

export type CodexAppServerClient = Readonly<{
  initialize: () => Promise<void>
  request: (method: string, params: unknown) => Promise<unknown>
  notify: (method: string, params: unknown) => void
  onNotification: (listener: (message: CodexAppServerMessage) => void) => () => void
  close: () => void
}>

export type CodexAppServerSpawn = (input: Readonly<{
  binary: string
  env: NodeJS.ProcessEnv
  cwd: string
}>) => ChildProcessWithoutNullStreams

const defaultSpawn: CodexAppServerSpawn = input => spawn(input.binary, ["app-server"], {
  cwd: input.cwd,
  env: input.env,
  stdio: ["pipe", "pipe", "pipe"],
})

export class CodexAppServerError extends Error {
  readonly _tag = "CodexAppServerError"
  override readonly name = "CodexAppServerError"
  constructor(
    readonly reason: "closed" | "invalid_message" | "request_failed" | "timeout",
    message: string,
  ) { super(message) }
}

/**
 * Fail-closed responses for app-server initiated requests. The response shape
 * is method-specific: returning an approval decision to a user-input request
 * is invalid JSON-RPC even when both mean "no".
 */
export const declineCodexServerRequest = (request: CodexAppServerRequest): unknown => {
  switch (request.method) {
    case "item/commandExecution/requestApproval":
    case "item/fileChange/requestApproval":
    case "applyPatchApproval":
    case "execCommandApproval":
      return { decision: "decline" }
    case "item/tool/requestUserInput":
      return { answers: {} }
    case "mcpServer/elicitation/request":
      return { action: "decline", content: null, _meta: null }
    case "item/tool/call":
      return { contentItems: [], success: false }
    default:
      throw new CodexAppServerError("request_failed", `Unsupported Codex server request: ${request.method}`)
  }
}

export const openCodexAppServerClient = (input: Readonly<{
  binary: string
  env: NodeJS.ProcessEnv
  cwd: string
  spawnImpl?: CodexAppServerSpawn
  requestTimeoutMs?: number
  onServerRequest?: (request: CodexAppServerRequest) => Promise<unknown>
}>): CodexAppServerClient => {
  const child = (input.spawnImpl ?? defaultSpawn)({ binary: input.binary, env: input.env, cwd: input.cwd })
  const pending = new Map<number, Readonly<{
    resolve: (value: unknown) => void
    reject: (error: Error) => void
    timer: ReturnType<typeof setTimeout>
  }>>()
  const listeners = new Set<(message: CodexAppServerMessage) => void>()
  const timeoutMs = input.requestTimeoutMs ?? 30_000
  let sequence = 0
  let closed = false
  let initialized: Promise<void> | null = null

  const write = (message: unknown): void => {
    if (closed || !child.stdin.writable) throw new CodexAppServerError("closed", "Codex app-server is closed")
    child.stdin.write(`${JSON.stringify(message)}\n`)
  }
  const rejectPending = (error: Error): void => {
    for (const operation of pending.values()) {
      clearTimeout(operation.timer)
      operation.reject(error)
    }
    pending.clear()
  }
  const request = (method: string, params: unknown): Promise<unknown> => {
    const id = ++sequence
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        pending.delete(id)
        reject(new CodexAppServerError("timeout", `${method} timed out`))
      }, timeoutMs)
      pending.set(id, { resolve, reject, timer })
      try { write({ method, id, params }) } catch (error) {
        clearTimeout(timer)
        pending.delete(id)
        reject(error)
      }
    })
  }
  const lineReader = createInterface({ input: child.stdout })
  lineReader.on("line", line => {
    let message: CodexAppServerMessage
    try {
      const parsed = JSON.parse(line)
      if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) throw new Error("not an object")
      message = parsed as CodexAppServerMessage
    } catch {
      rejectPending(new CodexAppServerError("invalid_message", "Codex app-server emitted invalid JSONL"))
      return
    }
    if (typeof message.id === "number" && ("result" in message || "error" in message)) {
      const operation = pending.get(message.id)
      if (operation === undefined) return
      pending.delete(message.id)
      clearTimeout(operation.timer)
      if ("error" in message) {
        const detail = message.error !== null && typeof message.error === "object" &&
          typeof (message.error as { message?: unknown }).message === "string"
          ? (message.error as { message: string }).message
          : "Codex app-server request failed"
        operation.reject(new CodexAppServerError("request_failed", detail))
      } else operation.resolve(message.result)
      return
    }
    if ((typeof message.id === "number" || typeof message.id === "string") && typeof message.method === "string") {
      const serverRequest = message as CodexAppServerRequest
      void (input.onServerRequest?.(serverRequest) ?? Promise.resolve().then(() => declineCodexServerRequest(serverRequest)))
        .then(result => write({ id: serverRequest.id, result }))
        .catch(error => write({
          id: serverRequest.id,
          error: { code: -32_000, message: error instanceof Error ? error.message : "request refused" },
        }))
      return
    }
    if (typeof message.method === "string") {
      for (const listener of listeners) listener(message)
    }
  })
  child.on("error", () => {
    closed = true
    rejectPending(new CodexAppServerError("closed", "Codex app-server failed to start"))
  })
  child.on("close", () => {
    closed = true
    rejectPending(new CodexAppServerError("closed", "Codex app-server exited"))
  })

  return {
    initialize: () => {
      if (initialized !== null) return initialized
      initialized = request("initialize", {
        clientInfo: { name: "openagents_desktop", title: "OpenAgents Desktop", version: "0.1.0" },
        capabilities: { experimentalApi: true },
      }).then(() => { write({ method: "initialized", params: {} }) })
      return initialized
    },
    request,
    notify: write,
    onNotification: listener => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    close: () => {
      if (closed) return
      closed = true
      lineReader.close()
      child.kill("SIGTERM")
      rejectPending(new CodexAppServerError("closed", "Codex app-server closed"))
    },
  }
}

export type ProductSpecSkillRegistration = Readonly<{
  name: "productspec-work"
  path: string
  enabled: true
}>

export type AssuranceSpecSkillRegistration = Readonly<{
  name: "assurancespec-work"
  path: string
  enabled: true
}>

const registerNativeSkill = async <Name extends "productspec-work" | "assurancespec-work">(input: Readonly<{
  client: CodexAppServerClient
  cwd: string
  skillRoot: string
  skillPath: string
  name: Name
}>): Promise<Readonly<{ name: Name; path: string; enabled: true }>> => {
  await input.client.initialize()
  await input.client.request("skills/extraRoots/set", { extraRoots: [input.skillRoot] })
  await input.client.request("skills/config/write", { path: input.skillPath, enabled: true })
  const listed = await input.client.request("skills/list", { cwds: [input.cwd], forceReload: true })
  const data = listed !== null && typeof listed === "object" && Array.isArray((listed as { data?: unknown }).data)
    ? (listed as { data: unknown[] }).data
    : []
  const skills = data.flatMap(entry => entry !== null && typeof entry === "object" &&
    Array.isArray((entry as { skills?: unknown }).skills) ? (entry as { skills: unknown[] }).skills : [])
  const match = skills.find(skill => skill !== null && typeof skill === "object" &&
    (skill as { name?: unknown }).name === input.name &&
    (skill as { path?: unknown }).path === input.skillPath &&
    (skill as { enabled?: unknown }).enabled === true)
  if (match === undefined) {
    throw new CodexAppServerError("request_failed", `Codex did not confirm the ${input.name} skill`)
  }
  return { name: input.name, path: input.skillPath, enabled: true }
}

/** Register and prove the built-in skill through Codex's native app-server APIs. */
export const registerProductSpecSkill = async (input: Readonly<{
  client: CodexAppServerClient
  cwd: string
  skillRoot: string
  skillPath: string
}>): Promise<ProductSpecSkillRegistration> => {
  return registerNativeSkill({ ...input, name: "productspec-work" })
}

/** Register and prove the AssuranceSpec built-in through Codex's native APIs. */
export const registerAssuranceSpecSkill = async (input: Readonly<{
  client: CodexAppServerClient
  cwd: string
  skillRoot: string
  skillPath: string
}>): Promise<AssuranceSpecSkillRegistration> =>
  registerNativeSkill({ ...input, name: "assurancespec-work" })

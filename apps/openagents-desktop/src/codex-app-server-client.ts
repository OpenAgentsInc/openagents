import type { ChildProcessWithoutNullStreams } from "node:child_process"
import { spawn } from "node:child_process"
import { createInterface } from "node:readline"
import {
  decodeBundledClientResponse,
  decodeBundledServerNotification,
  decodeBundledServerRequest,
  type CodexProtocolDecodeResult,
} from "@openagentsinc/codex-app-server-protocol/decode"

export type CodexAppServerMessage = Readonly<{
  id?: unknown
  method?: unknown
  params?: unknown
  result?: unknown
  error?: unknown
}>
export type CodexAppServerRequest = Readonly<{
  id: string | number
  method: string
  params: unknown
}>

export type CodexAppServerClient = Readonly<{
  initialize: () => Promise<void>
  request: (method: string, params: unknown, options?: Readonly<{ signal?: AbortSignal }>) => Promise<unknown>
  notify: (method: string, params: unknown) => Promise<void>
  onNotification: (listener: (message: CodexAppServerMessage) => void) => () => void
  isClosed: () => boolean
  close: () => void
}>

export type CodexAppServerSpawn = (input: Readonly<{
  binary: string
  env: NodeJS.ProcessEnv
  cwd: string
}>) => ChildProcessWithoutNullStreams

export type CodexAppServerProtocolMessage = Readonly<{
  requestId: string | number | null
  decoded: CodexProtocolDecodeResult
}>

const defaultSpawn: CodexAppServerSpawn = input => spawn(input.binary, ["app-server"], {
  cwd: input.cwd,
  env: input.env,
  stdio: ["pipe", "pipe", "pipe"],
})

export class CodexAppServerError extends Error {
  readonly _tag = "CodexAppServerError"
  override readonly name = "CodexAppServerError"
  constructor(
    readonly reason: "cancelled" | "closed" | "invalid_message" | "overloaded" | "request_failed" | "timeout",
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
  maxQueuedWriteBytes?: number
  onServerRequest?: (request: CodexAppServerRequest) => Promise<unknown>
  onProtocolMessage?: (message: CodexAppServerProtocolMessage) => void
  strictGeneratedDecoding?: boolean
  onClose?: (error: CodexAppServerError) => void
  onStderr?: (chunk: string) => void
}>): CodexAppServerClient => {
  const child = (input.spawnImpl ?? defaultSpawn)({ binary: input.binary, env: input.env, cwd: input.cwd })
  const pending = new Map<number, Readonly<{
    method: string
    resolve: (value: unknown) => void
    reject: (error: Error) => void
    timer: ReturnType<typeof setTimeout>
  }>>()
  const listeners = new Set<(message: CodexAppServerMessage) => void>()
  const timeoutMs = input.requestTimeoutMs ?? 30_000
  const maxQueuedWriteBytes = input.maxQueuedWriteBytes ?? 4 * 1024 * 1024
  let sequence = 0
  let closed = false
  let closeReported = false
  let initialized: Promise<void> | null = null
  let queuedWriteBytes = 0
  let writeChain = Promise.resolve()

  const write = (message: unknown): Promise<void> => {
    const line = `${JSON.stringify(message)}\n`
    const bytes = Buffer.byteLength(line)
    if (closed || !child.stdin.writable) return Promise.reject(new CodexAppServerError("closed", "Codex app-server is closed"))
    if (bytes > maxQueuedWriteBytes || queuedWriteBytes + bytes > maxQueuedWriteBytes) {
      return Promise.reject(new CodexAppServerError("overloaded", "Codex app-server write queue is full"))
    }
    queuedWriteBytes += bytes
    const operation = writeChain.then(() => new Promise<void>((resolve, reject) => {
      if (closed || !child.stdin.writable) {
        reject(new CodexAppServerError("closed", "Codex app-server is closed"))
        return
      }
      const onError = (): void => reject(new CodexAppServerError("closed", "Codex app-server write failed"))
      child.stdin.once("error", onError)
      const accepted = child.stdin.write(line, () => {
        child.stdin.off("error", onError)
        resolve()
      })
      if (!accepted) child.stdin.once("drain", () => undefined)
    })).finally(() => { queuedWriteBytes -= bytes })
    writeChain = operation.catch(() => undefined)
    return operation
  }
  const rejectPending = (error: Error): void => {
    for (const operation of pending.values()) {
      clearTimeout(operation.timer)
      operation.reject(error)
    }
    pending.clear()
  }
  const request = (
    method: string,
    params: unknown,
    options: Readonly<{ signal?: AbortSignal }> = {},
  ): Promise<unknown> => {
    const id = ++sequence
    return new Promise((resolve, reject) => {
      if (options.signal?.aborted === true) {
        reject(new CodexAppServerError("cancelled", `${method} cancelled`))
        return
      }
      const timer = setTimeout(() => {
        pending.delete(id)
        reject(new CodexAppServerError("timeout", `${method} timed out`))
      }, timeoutMs)
      const abort = (): void => {
        clearTimeout(timer)
        pending.delete(id)
        reject(new CodexAppServerError("cancelled", `${method} cancelled`))
      }
      options.signal?.addEventListener("abort", abort, { once: true })
      pending.set(id, {
        method,
        resolve: value => { options.signal?.removeEventListener("abort", abort); resolve(value) },
        reject: error => { options.signal?.removeEventListener("abort", abort); reject(error) },
        timer,
      })
      void write({ method, id, params }).catch(error => {
        clearTimeout(timer)
        pending.delete(id)
        reject(error)
      })
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
      const error = new CodexAppServerError("invalid_message", "Codex app-server emitted invalid JSONL")
      closed = true
      rejectPending(error)
      if (!closeReported) { closeReported = true; input.onClose?.(error) }
      child.kill("SIGTERM")
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
      } else {
        const decoded = decodeBundledClientResponse(operation.method, message.result)
        input.onProtocolMessage?.({ requestId: message.id, decoded })
        if (decoded._tag === "DecodeFailure" && input.strictGeneratedDecoding === true) {
          operation.reject(new CodexAppServerError("invalid_message", `Codex ${operation.method} response failed generated decoding`))
        } else operation.resolve(decoded._tag === "Decoded" ? decoded.payload : message.result)
      }
      return
    }
    if ((typeof message.id === "number" || typeof message.id === "string") && typeof message.method === "string") {
      const decoded = decodeBundledServerRequest(message.method, message.params)
      input.onProtocolMessage?.({ requestId: message.id, decoded })
      if (decoded._tag === "DecodeFailure" && input.strictGeneratedDecoding === true) {
        void write({ id: message.id, error: { code: -32_602, message: "Generated protocol decoding failed" } }).catch(() => undefined)
        return
      }
      const serverRequest: CodexAppServerRequest = {
        id: message.id,
        method: message.method,
        params: decoded._tag === "Decoded" ? decoded.payload : message.params,
      }
      void (input.onServerRequest?.(serverRequest) ?? Promise.resolve().then(() => declineCodexServerRequest(serverRequest)))
        .then(result => write({ id: serverRequest.id, result }))
        .catch(error => write({
          id: serverRequest.id,
          error: { code: -32_000, message: error instanceof Error ? error.message : "request refused" },
        }).catch(() => undefined))
      return
    }
    if (typeof message.method === "string") {
      const decoded = decodeBundledServerNotification(message.method, message.params)
      input.onProtocolMessage?.({ requestId: null, decoded })
      if (decoded._tag === "DecodeFailure" && input.strictGeneratedDecoding === true) return
      const notification: CodexAppServerMessage = decoded._tag === "Decoded"
        ? { ...message, params: decoded.payload }
        : message
      for (const listener of listeners) {
        try { listener(notification) } catch { /* isolate observers from transport */ }
      }
    }
  })
  child.stderr.on("data", chunk => input.onStderr?.(String(chunk).slice(0, 4_096)))
  const reportClosed = (message: string): void => {
    closed = true
    const error = new CodexAppServerError("closed", message)
    rejectPending(error)
    if (!closeReported) { closeReported = true; input.onClose?.(error) }
  }
  child.on("error", () => reportClosed("Codex app-server failed to start"))
  child.on("close", () => reportClosed("Codex app-server exited"))

  return {
    initialize: () => {
      if (initialized !== null) return initialized
      initialized = request("initialize", {
        clientInfo: { name: "openagents_desktop", title: "OpenAgents Desktop", version: "0.1.0" },
        capabilities: { experimentalApi: false },
      }).then(() => write({ method: "initialized", params: {} }))
      return initialized
    },
    request,
    notify: (method, params) => write({ method, params }),
    onNotification: listener => {
      listeners.add(listener)
      return () => listeners.delete(listener)
    },
    isClosed: () => closed,
    close: () => {
      if (closed) return
      closed = true
      lineReader.close()
      child.kill("SIGTERM")
      const error = new CodexAppServerError("closed", "Codex app-server closed")
      rejectPending(error)
      if (!closeReported) { closeReported = true; input.onClose?.(error) }
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

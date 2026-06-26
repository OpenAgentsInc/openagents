import { createHash } from "node:crypto"

import type { TipsNetworkOptions } from "./tips.js"
import { assertPublicSafe } from "./work-requester.js"

export const KHALA_REQUEST_MODEL = "openagents/khala"

export type PylonKhalaWorkflow =
  | "cloud_coding_session"
  | "codex_agent_task"

export type PylonKhalaRequestInput = {
  prompt: string
  objectiveSummary?: string
  targetPylonRef?: string
  workflow?: PylonKhalaWorkflow
  workspace?: PylonKhalaGitCheckoutWorkspace
}

export type PylonKhalaGitCheckoutWorkspace = {
  kind: "git_checkout"
  repository: {
    branch: string
    commitSha: string
    fullName: string
    provider: "github"
    visibility: "public"
  }
  verificationCommand: {
    args: string[]
    commandRef: string
  }
}

export type PylonKhalaResumeInput = {
  durableRequestId: string
  offset?: string | number
}

export type PylonKhalaSseFrame = {
  data: string
  parsed: unknown | null
}

export type PylonKhalaStreamProjection = {
  durableRequestId: string | null
  durableStreamUrl: string | null
  frames: PylonKhalaSseFrame[]
  nextOffset: string
  rawSse: string
  streamClosed: boolean
  streamUpToDate: boolean
  text: string
}

export type PylonKhalaRequestResult = PylonKhalaStreamProjection & {
  assignmentRef: string | null
  model: typeof KHALA_REQUEST_MODEL
  ok: true
  schema: "openagents.pylon.khala_request.v1"
  workflow: PylonKhalaWorkflow | null
}

export type PylonKhalaResumeResult = PylonKhalaStreamProjection & {
  ok: true
  schema: "openagents.pylon.khala_resume.v1"
}

export type PylonKhalaStatusResult = PylonKhalaStreamProjection & {
  ok: true
  schema: "openagents.pylon.khala_status.v1"
  state: "closed" | "streaming" | "up_to_date"
}

const durablePrefix = "/v1/chat/completions/durable/"

function requireAgentToken(options: TipsNetworkOptions): string {
  const token = options.agentToken ?? process.env.OPENAGENTS_AGENT_TOKEN
  if (!token) {
    throw new Error("OPENAGENTS_AGENT_TOKEN or --agent-token is required for Khala requests")
  }
  return token
}

const byteLength = (value: string): number => new TextEncoder().encode(value).byteLength
const pylonRefPattern = /^[a-z0-9][a-z0-9_.:-]{2,119}$/
const githubFullNamePattern = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const gitCommitShaPattern = /^[a-f0-9]{40}$/i
const placeholderCommitShaPattern = /^(0{40}|1{40})$/i
const verificationCommandArgPattern = /^[A-Za-z0-9_./:=@+-]{1,120}$/

function stableRef(prefix: string, value: string) {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}

function cleanRefSegment(value: string) {
  return value.trim().replace(/[^A-Za-z0-9_-]+/g, "_").slice(0, 80) || "request"
}

function githubFullNameFromInput(repository: string | undefined): string {
  const value = repository?.trim()
  if (!value) return "OpenAgentsInc/openagents"
  assertPublicSafe(value, "khala request repository")
  if (githubFullNamePattern.test(value)) return value
  const github = /^https:\/\/github\.com\/([^/\s]+)\/([^/\s#?]+)(?:[/?#].*)?$/.exec(value)
  if (github) return `${github[1]}/${github[2].replace(/\.git$/, "")}`
  throw new Error("khala request --repo must be owner/repo or a public GitHub URL")
}

function verificationArgsFromInput(command: string | undefined): string[] {
  const value = command?.trim()
  if (!value) {
    throw new Error("khala request --verify <command> is required for workspace-backed coding requests")
  }
  assertPublicSafe(value, "khala request verification command")
  const args = value.split(/\s+/).filter(Boolean)
  if (
    args.length === 0 ||
    args.length > 20 ||
    args.some((arg) =>
      !verificationCommandArgPattern.test(arg) ||
      arg.includes("..") ||
      arg.startsWith("/"),
    )
  ) {
    throw new Error("khala request --verify must be bounded argv tokens without absolute paths or traversal")
  }
  return args
}

function commitShaFromInput(commit: string | undefined): string {
  const value = commit?.trim()
  if (!value) {
    throw new Error("khala request --commit <40-char-sha> is required for workspace-backed coding requests")
  }
  if (!gitCommitShaPattern.test(value) || placeholderCommitShaPattern.test(value)) {
    throw new Error("khala request --commit must be a real pinned 40-character commit SHA, not a placeholder")
  }
  return value.toLowerCase()
}

function cleanBranch(branch: string | undefined): string {
  const value = branch?.trim() || "main"
  assertPublicSafe(value, "khala request branch")
  if (value.includes("..") || value.startsWith("/") || value.length > 120) {
    throw new Error("khala request --branch must be a bounded public branch name")
  }
  return value
}

export function buildPylonKhalaGitCheckoutWorkspace(input: {
  branch?: string
  commit?: string
  repository?: string
  verificationCommand?: string
}): PylonKhalaGitCheckoutWorkspace {
  const fullName = githubFullNameFromInput(input.repository)
  const commitSha = commitShaFromInput(input.commit)
  const args = verificationArgsFromInput(input.verificationCommand)
  const workspace: PylonKhalaGitCheckoutWorkspace = {
    kind: "git_checkout",
    repository: {
      branch: cleanBranch(input.branch),
      commitSha,
      fullName,
      provider: "github",
      visibility: "public",
    },
    verificationCommand: {
      args,
      commandRef: `command.public.pylon_khala.${cleanRefSegment(args.join("_"))}.${stableRef("argv", args.join("\0")).slice("argv.".length)}`,
    },
  }
  assertPublicSafe(workspace, "khala request workspace")
  return workspace
}

function cleanObjectiveSummary(value: string | undefined, fallback: string): string {
  const summary = (value ?? fallback).trim()
  if (summary.length < 3 || summary.length > 1000) {
    throw new Error("khala request objective summary must be 3-1000 characters")
  }
  assertPublicSafe(summary, "khala request objective summary")
  return summary
}

export function durableRequestIdFromUrl(value: string | null | undefined): string | null {
  if (value === null || value === undefined || value.trim() === "") {
    return null
  }

  const parsed = new URL(value, "https://openagents.invalid")
  if (!parsed.pathname.startsWith(durablePrefix)) {
    return null
  }
  const encoded = parsed.pathname.slice(durablePrefix.length)
  return encoded === "" || encoded.includes("/") ? null : decodeURIComponent(encoded)
}

export function buildPylonKhalaChatRequestBody(
  input: PylonKhalaRequestInput,
): Record<string, unknown> {
  const prompt = input.prompt.trim()
  if (prompt.length < 3 || prompt.length > 8_000) {
    throw new Error("khala request --prompt/--objective must be 3-8000 characters")
  }
  assertPublicSafe(prompt, "khala request prompt")

  const targetPylonRef = input.targetPylonRef?.trim()
  if (targetPylonRef !== undefined && targetPylonRef !== "") {
    if (!pylonRefPattern.test(targetPylonRef)) {
      throw new Error("khala request --pylon-ref must be a public-safe Pylon ref")
    }
    assertPublicSafe(targetPylonRef, "khala request target pylon ref")
  }

  const coding =
    targetPylonRef === undefined || targetPylonRef === ""
      ? undefined
      : { targetPylonRef }
  const workspaceCoding =
    input.workspace === undefined
      ? coding
      : {
          ...(coding ?? {}),
          objectiveSummary: cleanObjectiveSummary(input.objectiveSummary, prompt),
          workspace: input.workspace,
        }
  const openagents =
    input.workflow === undefined && workspaceCoding === undefined
      ? undefined
      : {
          ...(input.workflow === undefined ? {} : { workflowClass: input.workflow }),
          ...(workspaceCoding === undefined ? {} : { coding: workspaceCoding }),
        }

  const body = {
    messages: [
      {
        content: prompt,
        role: "user",
      },
    ],
    model: KHALA_REQUEST_MODEL,
    ...(input.workflow === undefined ? {} : { workflowClass: input.workflow }),
    ...(openagents === undefined ? {} : { openagents }),
    stream: true,
    ...(targetPylonRef === undefined || targetPylonRef === ""
      ? {}
      : { targetPylonRef }),
  }
  assertPublicSafe(body, "khala request body")
  return body
}

function parseSseFrames(rawSse: string): PylonKhalaSseFrame[] {
  return rawSse
    .split(/\n\n+/)
    .map((chunk) =>
      chunk
        .split(/\n/)
        .filter((line) => line.startsWith("data:"))
        .map((line) => line.slice("data:".length).trimStart())
        .join("\n"),
    )
    .filter((data) => data !== "")
    .map((data) => {
      if (data === "[DONE]") {
        return { data, parsed: null }
      }
      try {
        return { data, parsed: JSON.parse(data) }
      } catch {
        return { data, parsed: null }
      }
    })
}

function textFromFrames(frames: readonly PylonKhalaSseFrame[]): string {
  return frames
    .map((frame) => {
      const parsed = frame.parsed
      if (parsed === null || typeof parsed !== "object") {
        return ""
      }
      const choices = (parsed as { choices?: unknown }).choices
      if (!Array.isArray(choices)) {
        return ""
      }
      return choices
        .map((choice) => {
          const delta = (choice as { delta?: { content?: unknown } }).delta
          const message = (choice as { message?: { content?: unknown } }).message
          const content = delta?.content ?? message?.content
          return typeof content === "string" ? content : ""
        })
        .join("")
    })
    .join("")
}

function streamProjection(input: {
  durableStreamUrl: string | null
  fallbackRequestId?: string | null
  rawSse: string
  response: Response
}): PylonKhalaStreamProjection {
  const frames = parseSseFrames(input.rawSse)
  const durableRequestId =
    durableRequestIdFromUrl(input.durableStreamUrl) ?? input.fallbackRequestId ?? null
  return {
    durableRequestId,
    durableStreamUrl: input.durableStreamUrl,
    frames,
    nextOffset:
      input.response.headers.get("stream-next-offset") ??
      String(byteLength(input.rawSse)),
    rawSse: input.rawSse,
    streamClosed:
      input.response.headers.get("stream-closed") === "true" ||
      frames.some((frame) => frame.data === "[DONE]"),
    streamUpToDate: input.response.headers.get("stream-up-to-date") === "true",
    text: textFromFrames(frames),
  }
}

async function khalaApiRequest(
  options: TipsNetworkOptions,
  input: {
    body?: Record<string, unknown>
    method: "GET" | "POST"
    path: string
  },
): Promise<Response> {
  const token = requireAgentToken(options)
  const response = await (options.fetch ?? fetch)(new URL(input.path, options.baseUrl), {
    ...(input.body === undefined ? {} : { body: JSON.stringify(input.body) }),
    headers: {
      Authorization: `Bearer ${token}`,
      "Content-Type": "application/json",
    },
    method: input.method,
  })
  if (!response.ok) {
    const text = await response.text()
    let reason = text.trim() || String(response.status)
    try {
      const payload = JSON.parse(text) as { error?: unknown; reason?: unknown }
      reason =
        typeof payload.reason === "string"
          ? payload.reason
          : typeof payload.error === "string"
            ? payload.error
            : reason
    } catch {
      // Keep the raw bounded response text as the reason.
    }
    throw new Error(`pylon khala request failed (${response.status}): ${reason}`)
  }
  return response
}

export async function issuePylonKhalaRequest(
  options: TipsNetworkOptions,
  input: PylonKhalaRequestInput,
): Promise<PylonKhalaRequestResult> {
  const body = buildPylonKhalaChatRequestBody(input)
  const response = await khalaApiRequest(options, {
    body,
    method: "POST",
    path: "/v1/chat/completions",
  })
  const rawSse = await response.text()
  const durableStreamUrl = response.headers.get("openagents-durable-stream-url")
  const projection = streamProjection({
    durableStreamUrl,
    fallbackRequestId: null,
    rawSse,
    response,
  })
  return {
    ...projection,
    assignmentRef: response.headers.get("openagents-coding-assignment-ref"),
    model: KHALA_REQUEST_MODEL,
    ok: true,
    schema: "openagents.pylon.khala_request.v1",
    workflow: input.workflow ?? null,
  }
}

export async function resumePylonKhalaRequest(
  options: TipsNetworkOptions,
  input: PylonKhalaResumeInput,
): Promise<PylonKhalaResumeResult> {
  assertPublicSafe(input.durableRequestId, "khala durable request id")
  const offset =
    input.offset === undefined
      ? undefined
      : String(input.offset).trim()
  const path =
    `${durablePrefix}${encodeURIComponent(input.durableRequestId)}` +
    (offset === undefined || offset === "" ? "" : `?offset=${encodeURIComponent(offset)}`)
  const response = await khalaApiRequest(options, { method: "GET", path })
  const rawSse = await response.text()
  return {
    ...streamProjection({
      durableStreamUrl: `${durablePrefix}${encodeURIComponent(input.durableRequestId)}`,
      fallbackRequestId: input.durableRequestId,
      rawSse,
      response,
    }),
    ok: true,
    schema: "openagents.pylon.khala_resume.v1",
  }
}

export async function readPylonKhalaStatus(
  options: TipsNetworkOptions,
  durableRequestId: string,
): Promise<PylonKhalaStatusResult> {
  const resumed = await resumePylonKhalaRequest(options, {
    durableRequestId,
    offset: 0,
  })
  return {
    ...resumed,
    schema: "openagents.pylon.khala_status.v1",
    state: resumed.streamClosed
      ? "closed"
      : resumed.streamUpToDate
        ? "up_to_date"
        : "streaming",
  }
}

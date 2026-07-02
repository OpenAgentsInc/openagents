import { readFile } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"

import type { CodexAppServerChatRuntime } from "./codex-app-server-chat-runtime.js"
import type { ClaudeAppSdkChatRuntime } from "./claude-app-sdk-chat-runtime.js"
import type {
  KhalaCodeDesktopSessionCatalogEntry,
  KhalaCodeDesktopSessionCatalogResult,
  KhalaCodeDesktopSessionExactTotals,
  KhalaCodeDesktopSessionHarnessKind,
} from "../shared/session-catalog.js"

type JsonRecord = Readonly<Record<string, unknown>>

export type KhalaCodeDesktopSessionCatalogRequest = {
  readonly limit?: number | undefined
  readonly searchTerm?: string | undefined
}

export type KhalaCodeDesktopSessionCatalogOptions = {
  readonly claudeRuntime?: ClaudeAppSdkChatRuntime
  readonly codexRuntime?: CodexAppServerChatRuntime | null
  readonly env?: Readonly<Record<string, string | undefined>>
  readonly limit?: number | undefined
}

const isRecord = (value: unknown): value is JsonRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringField = (value: unknown, field: string): string | null => {
  if (!isRecord(value)) return null
  const candidate = value[field]
  return typeof candidate === "string" && candidate.length > 0 ? candidate : null
}

const numberField = (value: unknown, field: string): number | undefined => {
  if (!isRecord(value)) return undefined
  const candidate = value[field]
  return typeof candidate === "number" && Number.isFinite(candidate)
    ? Math.trunc(candidate)
    : undefined
}

const numberFieldOrNull = (value: unknown, field: string): number | null =>
  numberField(value, field) ?? null

const objectField = (value: unknown, field: string): JsonRecord | null => {
  if (!isRecord(value)) return null
  const candidate = value[field]
  return isRecord(candidate) ? candidate : null
}

const sessionsRecord = async (
  path: string,
): Promise<Readonly<Record<string, JsonRecord>>> => {
  try {
    const parsed = JSON.parse(await readFile(path, "utf8")) as unknown
    const sessions = objectField(parsed, "sessions")
    if (sessions === null) return {}
    return Object.fromEntries(
      Object.entries(sessions).filter((entry): entry is [string, JsonRecord] => isRecord(entry[1])),
    )
  } catch {
    return {}
  }
}

export const resolveCodexSessionCatalogStorePath = (
  env: Readonly<Record<string, string | undefined>> = Bun.env,
): string => {
  const explicit = env.KHALA_CODE_DESKTOP_CODEX_STATE_PATH?.trim()
  if (explicit !== undefined && explicit.length > 0) return explicit
  return join(env.HOME?.trim() || homedir(), ".khala-code", "codex-sessions.json")
}

export const resolveClaudeSessionCatalogStorePath = (
  env: Readonly<Record<string, string | undefined>> = Bun.env,
): string => {
  const explicit = env.KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH?.trim()
  if (explicit !== undefined && explicit.length > 0) return explicit
  return join(env.HOME?.trim() || homedir(), ".khala-code", "claude-sessions.json")
}

const reportedTotals = (
  value: unknown,
  source: string,
): KhalaCodeDesktopSessionExactTotals | undefined => {
  const totalTokens =
    numberField(value, "totalTokens") ??
    numberField(value, "total_tokens") ??
    numberField(value, "tokensUsed") ??
    numberField(value, "tokens_used")
  if (totalTokens === undefined) return undefined
  const inputTokens = numberField(value, "inputTokens") ?? numberField(value, "input_tokens")
  const cachedInputTokens =
    numberField(value, "cachedInputTokens") ?? numberField(value, "cached_input_tokens")
  const outputTokens = numberField(value, "outputTokens") ?? numberField(value, "output_tokens")
  const reasoningOutputTokens =
    numberField(value, "reasoningOutputTokens") ?? numberField(value, "reasoning_output_tokens")
  return {
    totalTokens,
    source,
    ...(inputTokens === undefined ? {} : { inputTokens }),
    ...(cachedInputTokens === undefined ? {} : { cachedInputTokens }),
    ...(outputTokens === undefined ? {} : { outputTokens }),
    ...(reasoningOutputTokens === undefined ? {} : { reasoningOutputTokens }),
  }
}

const sessionIdFrom = (value: unknown): string | null =>
  stringField(value, "sessionId") ??
  stringField(value, "session_id") ??
  stringField(value, "id")

const threadIdFrom = (value: unknown): string | null =>
  stringField(value, "threadId") ??
  stringField(value, "thread_id") ??
  stringField(value, "id")

const titleFrom = (value: unknown, fallback: string): string =>
  stringField(value, "title") ??
  stringField(value, "name") ??
  stringField(value, "summary") ??
  fallback

const previewFrom = (value: unknown): string =>
  stringField(value, "preview") ??
  stringField(value, "last_message") ??
  stringField(value, "lastMessage") ??
  ""

const catalogEntryId = (
  harnessKind: KhalaCodeDesktopSessionHarnessKind,
  ref: string,
): string => `${harnessKind}:${ref}`

const entryFromThread = (
  harnessKind: KhalaCodeDesktopSessionHarnessKind,
  thread: JsonRecord,
  source: string,
): KhalaCodeDesktopSessionCatalogEntry | null => {
  const sessionRef = sessionIdFrom(thread) ?? threadIdFrom(thread)
  if (sessionRef === null) return null
  const threadRef = threadIdFrom(thread)
  const createdAt = numberFieldOrNull(thread, "createdAt") ?? numberFieldOrNull(thread, "created_at")
  const updatedAt = numberFieldOrNull(thread, "updatedAt") ?? numberFieldOrNull(thread, "updated_at")
  const recencyAt = numberFieldOrNull(thread, "recencyAt") ?? updatedAt ?? createdAt
  const exactTotals = reportedTotals(thread, source)
  return {
    catalogEntryId: catalogEntryId(harnessKind, threadRef ?? sessionRef),
    harnessKind,
    sessionRef,
    threadRef,
    desktopSessionRef: null,
    lastTurnRef: stringField(thread, "lastTurnId") ?? stringField(thread, "last_turn_id"),
    title: titleFrom(thread, `${harnessKind === "codex" ? "Codex" : "Claude"} session`),
    preview: previewFrom(thread),
    cwd: stringField(thread, "cwd"),
    projectLabel: stringField(thread, "projectLabel") ?? stringField(thread, "project") ?? (harnessKind === "codex" ? "Codex" : "Claude"),
    status: stringField(thread, "status") ?? "ready",
    statusLabel: stringField(thread, "statusLabel") ?? `${harnessKind === "codex" ? "Codex" : "Claude"} session`,
    source,
    createdAt,
    updatedAt,
    recencyAt,
    ...(exactTotals === undefined ? {} : { exactTotals }),
  }
}

const entryFromStoredSession = (
  harnessKind: KhalaCodeDesktopSessionHarnessKind,
  desktopSessionRef: string,
  stored: JsonRecord,
): KhalaCodeDesktopSessionCatalogEntry | null => {
  const sessionRef = sessionIdFrom(stored)
  const threadRef = threadIdFrom(stored) ?? sessionRef
  const ref = threadRef ?? sessionRef
  if (ref === null) return null
  const updatedAtIso = stringField(stored, "updatedAt")
  const updatedAt = updatedAtIso === null ? null : Date.parse(updatedAtIso)
  const normalizedUpdatedAt = updatedAt === null || Number.isNaN(updatedAt) ? null : updatedAt
  return {
    catalogEntryId: catalogEntryId(harnessKind, ref),
    harnessKind,
    sessionRef: sessionRef ?? ref,
    threadRef,
    desktopSessionRef,
    lastTurnRef: stringField(stored, "lastCodexTurnId") ?? stringField(stored, "lastTurnId"),
    title: `${harnessKind === "codex" ? "Codex" : "Claude"} session`,
    preview: "",
    cwd: null,
    projectLabel: harnessKind === "codex" ? "Codex" : "Claude",
    status: "ready",
    statusLabel: `${harnessKind === "codex" ? "Codex" : "Claude"} session`,
    source: `${harnessKind}_session_store`,
    createdAt: null,
    updatedAt: normalizedUpdatedAt,
    recencyAt: normalizedUpdatedAt,
  }
}

const mergeEntry = (
  map: Map<string, KhalaCodeDesktopSessionCatalogEntry>,
  entry: KhalaCodeDesktopSessionCatalogEntry,
): void => {
  const existing = map.get(entry.catalogEntryId)
  if (existing === undefined) {
    map.set(entry.catalogEntryId, entry)
    return
  }
  const exactTotals = entry.exactTotals ?? existing.exactTotals
  map.set(entry.catalogEntryId, {
    ...entry,
    desktopSessionRef: entry.desktopSessionRef ?? existing.desktopSessionRef,
    lastTurnRef: entry.lastTurnRef ?? existing.lastTurnRef,
    ...(exactTotals === undefined ? {} : { exactTotals }),
  })
}

const matchesSearch = (
  entry: KhalaCodeDesktopSessionCatalogEntry,
  searchTerm: string | undefined,
): boolean => {
  const term = searchTerm?.trim().toLowerCase()
  if (term === undefined || term.length === 0) return true
  return [
    entry.harnessKind,
    entry.sessionRef,
    entry.threadRef,
    entry.desktopSessionRef,
    entry.title,
    entry.preview,
    entry.cwd,
    entry.projectLabel,
  ].filter((value): value is string => typeof value === "string")
    .join("\n")
    .toLowerCase()
    .includes(term)
}

export const readKhalaCodeDesktopSessionCatalog = async (
  request: KhalaCodeDesktopSessionCatalogRequest = {},
  options: KhalaCodeDesktopSessionCatalogOptions = {},
): Promise<KhalaCodeDesktopSessionCatalogResult> => {
  const diagnostics: string[] = []
  const entries = new Map<string, KhalaCodeDesktopSessionCatalogEntry>()
  const env = options.env ?? Bun.env

  for (const [desktopSessionRef, stored] of Object.entries(
    await sessionsRecord(resolveCodexSessionCatalogStorePath(env)),
  )) {
    const entry = entryFromStoredSession("codex", desktopSessionRef, stored)
    if (entry !== null) mergeEntry(entries, entry)
  }
  for (const [desktopSessionRef, stored] of Object.entries(
    await sessionsRecord(resolveClaudeSessionCatalogStorePath(env)),
  )) {
    const entry = entryFromStoredSession("claude", desktopSessionRef, stored)
    if (entry !== null) mergeEntry(entries, entry)
  }

  if (options.codexRuntime !== undefined && options.codexRuntime !== null) {
    try {
      const result = await options.codexRuntime.listThreads({
        limit: options.limit ?? request.limit ?? 100,
        searchTerm: request.searchTerm,
        useStateDbOnly: true,
      })
      for (const value of result.data) {
        if (!isRecord(value)) continue
        const entry = entryFromThread("codex", value, "codex_app_server_thread_list")
        if (entry !== null) mergeEntry(entries, entry)
      }
      for (const thread of result.threads ?? []) {
        const entry = entryFromThread("codex", thread, "codex_app_server_thread_projection")
        if (entry !== null) mergeEntry(entries, entry)
      }
    } catch (error) {
      diagnostics.push(`codex_catalog_unavailable:${error instanceof Error ? error.message : String(error)}`)
    }
  }

  if (options.claudeRuntime !== undefined) {
    try {
      const result = await options.claudeRuntime.listThreads({
        limit: options.limit ?? request.limit ?? 100,
        searchTerm: request.searchTerm,
      })
      for (const value of result.data) {
        if (!isRecord(value)) continue
        const entry = entryFromThread("claude", value, "claude_sdk_list_sessions")
        if (entry !== null) mergeEntry(entries, entry)
      }
      for (const thread of result.threads ?? []) {
        const entry = entryFromThread("claude", thread, "claude_thread_projection")
        if (entry !== null) mergeEntry(entries, entry)
      }
    } catch (error) {
      diagnostics.push(`claude_catalog_unavailable:${error instanceof Error ? error.message : String(error)}`)
    }
  }

  const limit = request.limit ?? options.limit
  const sorted = [...entries.values()]
    .filter(entry => matchesSearch(entry, request.searchTerm))
    .sort((left, right) => (right.recencyAt ?? 0) - (left.recencyAt ?? 0))

  return {
    ok: true,
    schemaVersion: "khala-code-desktop.session-catalog.v1",
    diagnostics,
    entries: limit === undefined ? sorted : sorted.slice(0, limit),
  }
}

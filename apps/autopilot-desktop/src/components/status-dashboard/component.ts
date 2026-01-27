import { Effect } from "effect"
import { invoke } from "@tauri-apps/api/core"
import { open } from "@tauri-apps/plugin-dialog"
import { listen } from "@tauri-apps/api/event"
import packageInfo from "../../../package.json"
import type { Component } from "../../effuse/index.js"
import { html } from "../../effuse/index.js"
import type { CodexConversationItem, CodexPlanStep } from "../../types/codex.js"
import {
  Conversation,
  ConversationContent,
  ConversationEmptyState,
  Canvas,
  Diff,
  Edge,
  Message,
  Node,
  NodeAction,
  NodeContent,
  NodeDescription,
  NodeFooter,
  NodeHeader,
  NodeTitle,
  Plan,
  Reasoning,
  ToolCall,
} from "../ai-elements/index.js"
import { Hotbar, HotbarItem } from "../ui/hotbar.js"

type CodexDoctorResponse = {
  ok: boolean
  codexBin: string | null
  version: string | null
  appServerOk: boolean
  details: string | null
  path: string | null
}

type WorkspaceConnectionResponse = {
  success: boolean
  message: string
  workspaceId: string
}

type WorkspaceConnectionStatusResponse = {
  workspaceId: string
  connected: boolean
}

type AppServerEvent = {
  workspace_id: string
  message: {
    method?: string
    params?: unknown
    [key: string]: unknown
  }
}

type RateLimitWindow = {
  usedPercent: number
  windowDurationMins: number | null
  resetsAt: number | null
}

type CreditsSnapshot = {
  hasCredits: boolean
  unlimited: boolean
  balance: string | null
}

type RateLimitSnapshot = {
  primary: RateLimitWindow | null
  secondary: RateLimitWindow | null
  credits: CreditsSnapshot | null
  planType: string | null
}

type SessionSummary = {
  id: string
  preview: string
  updatedAt: number
  createdAt: number
  modelProvider: string
}

type DoctorState = {
  ok: boolean
  appServerOk: boolean
  version: string | null
  codexBin: string | null
  detail: string
}

type LogPathsState = {
  appServerLogDir: string
  fullAutoLogDir: string
  traceBundleDir: string
}

type BusyState = {
  doctor: boolean
  connect: boolean
  disconnect: boolean
  send: boolean
  sessions: boolean
  resume: boolean
  newSession: boolean
}

type StatusState = {
  workspaceId: string
  workspacePath: string
  workspaceConnected: boolean
  workspaceMessage: string
  doctor: DoctorState
  lastEventText: string
  lastEventTime: string
  lastUpdated: string
  messageInput: string
  threadId: string | null
  sessions: SessionSummary[]
  activeSessionId: string | null
  sessionItems: Record<string, CodexConversationItem[]>
  sessionMessage: string
  extraPaneOpen: boolean
  leftSidebarCollapsed: boolean
  rightSidebarCollapsed: boolean
  storybookOpen: boolean
  fullAutoEnabled: boolean
  fullAutoThreadId: string | null
  fullAutoMessage: string
  rateLimits: RateLimitSnapshot | null
  logPaths: LogPathsState
  busy: BusyState
}

type StatusEvent =
  | { type: "RefreshDoctor" }
  | { type: "ConnectWorkspace" }
  | { type: "DisconnectWorkspace" }
  | { type: "BrowseWorkspace" }
  | { type: "StartNewSession" }
  | { type: "UpdateWorkspacePath"; path: string }
  | { type: "UpdateMessageInput"; value: string }
  | { type: "ToggleExtraPane" }
  | { type: "ToggleLeftSidebar" }
  | { type: "ToggleRightSidebar" }
  | { type: "ToggleSidebars" }
  | { type: "StorybookToggle"; isOpen: boolean }
  | { type: "SubmitMessage"; value: string }
  | { type: "RefreshWorkspaceStatus" }
  | { type: "RefreshSessions" }
  | { type: "SelectSession"; threadId: string }
  | { type: "SetFullAuto"; enabled: boolean; continuePrompt?: string | null }
  | { type: "StartFullAuto"; prompt: string }
  | { type: "AppServerEvent"; payload: AppServerEvent }

const workspaceIdKey = "autopilotWorkspaceId"
const workspacePathKey = "autopilotWorkspacePath"
const rootWorkspaceMessage = "Select a working directory to connect."
const appVersion =
  typeof packageInfo.version === "string" ? packageInfo.version : "0.0.0"
const MAX_ITEMS_PER_THREAD = 200
const MAX_TEXT_LENGTH = 8000
const MAX_DIFF_LENGTH = 12000
const MAX_TOOL_OUTPUT_LENGTH = 8000
const CANVAS_SCALE_MIN = 0.4
const CANVAS_SCALE_MAX = 2.5
const CANVAS_PADDING = 48
const HANDLE_OFFSET = 5

const CANVAS_NODES = {
  summary: { x: 80, y: 80, width: 240, height: 180 },
  decision: { x: 420, y: 80, width: 240, height: 180 },
  guardrail: { x: 80, y: 360, width: 240, height: 200 },
  action: { x: 420, y: 360, width: 240, height: 170 },
  context: { x: 820, y: 60, width: 260, height: 190 },
  guidance: { x: 820, y: 280, width: 260, height: 190 },
  policy: { x: 820, y: 500, width: 260, height: 190 },
  dispatch: { x: 820, y: 720, width: 260, height: 170 },
  market: { x: 1140, y: 280, width: 260, height: 190 },
  optimizer: { x: 1140, y: 500, width: 260, height: 190 },
} as const

const CANVAS_BOUNDS = (() => {
  const nodes = Object.values(CANVAS_NODES)
  const minX = Math.min(...nodes.map((node) => node.x - HANDLE_OFFSET))
  const minY = Math.min(...nodes.map((node) => node.y - HANDLE_OFFSET))
  const maxX = Math.max(...nodes.map((node) => node.x + node.width + HANDLE_OFFSET))
  const maxY = Math.max(...nodes.map((node) => node.y + node.height + HANDLE_OFFSET))
  return {
    minX,
    minY,
    maxX,
    maxY,
    width: maxX - minX,
    height: maxY - minY,
  }
})()

type CanvasTransform = {
  scale: number
  tx: number
  ty: number
}

const clampNumber = (value: number, min: number, max: number) =>
  Math.min(max, Math.max(min, value))

const readCanvasTransform = (canvas: HTMLElement): CanvasTransform => {
  const scale = Number.parseFloat(canvas.dataset.canvasScale ?? "1")
  const tx = Number.parseFloat(canvas.dataset.canvasTx ?? "0")
  const ty = Number.parseFloat(canvas.dataset.canvasTy ?? "0")
  return {
    scale: Number.isFinite(scale) ? scale : 1,
    tx: Number.isFinite(tx) ? tx : 0,
    ty: Number.isFinite(ty) ? ty : 0,
  }
}

const writeCanvasTransform = (
  canvas: HTMLElement,
  transform: CanvasTransform
) => {
  const scale = clampNumber(transform.scale, CANVAS_SCALE_MIN, CANVAS_SCALE_MAX)
  canvas.dataset.canvasScale = scale.toString()
  canvas.dataset.canvasTx = transform.tx.toString()
  canvas.dataset.canvasTy = transform.ty.toString()
  canvas.style.setProperty("--canvas-scale", scale.toString())
  canvas.style.setProperty("--canvas-translate-x", `${transform.tx}px`)
  canvas.style.setProperty("--canvas-translate-y", `${transform.ty}px`)
}

const centerCanvasInContainer = (container: HTMLElement) => {
  const canvas = container.querySelector(".ai-canvas") as HTMLElement | null
  if (!canvas) {
    return
  }
  const rect = canvas.getBoundingClientRect()
  if (rect.width <= 0 || rect.height <= 0) {
    return
  }
  const scaleFit = Math.min(
    1,
    (rect.width - CANVAS_PADDING * 2) / CANVAS_BOUNDS.width,
    (rect.height - CANVAS_PADDING * 2) / CANVAS_BOUNDS.height
  )
  const scale = clampNumber(scaleFit, CANVAS_SCALE_MIN, CANVAS_SCALE_MAX)
  const tx = (rect.width - CANVAS_BOUNDS.width * scale) / 2 - CANVAS_BOUNDS.minX * scale
  const ty = (rect.height - CANVAS_BOUNDS.height * scale) / 2 - CANVAS_BOUNDS.minY * scale
  writeCanvasTransform(canvas, { scale, tx, ty })
}

const nowTime = () => new Date().toLocaleTimeString()
const nowEpochSeconds = () => Math.floor(Date.now() / 1000)
const formatRelativeTime = (timestamp: number) => {
  const now = Date.now()
  const diffSeconds = Math.round((timestamp - now) / 1000)
  const absSeconds = Math.abs(diffSeconds)
  if (absSeconds < 5) {
    return "now"
  }
  if (absSeconds < 60) {
    const value = Math.max(1, Math.round(absSeconds))
    return diffSeconds < 0 ? `${value}s ago` : `in ${value}s`
  }
  if (absSeconds < 60 * 60) {
    const value = Math.max(1, Math.round(absSeconds / 60))
    return diffSeconds < 0 ? `${value}m ago` : `in ${value}m`
  }
  const ranges: { unit: Intl.RelativeTimeFormatUnit; seconds: number }[] = [
    { unit: "year", seconds: 60 * 60 * 24 * 365 },
    { unit: "month", seconds: 60 * 60 * 24 * 30 },
    { unit: "week", seconds: 60 * 60 * 24 * 7 },
    { unit: "day", seconds: 60 * 60 * 24 },
    { unit: "hour", seconds: 60 * 60 },
    { unit: "minute", seconds: 60 },
    { unit: "second", seconds: 1 },
  ]
  const range =
    ranges.find((entry) => absSeconds >= entry.seconds) ||
    ranges[ranges.length - 1]
  if (!range) {
    return "now"
  }
  const value = Math.round(diffSeconds / range.seconds)
  const formatter = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" })
  return formatter.format(value, range.unit)
}
const isRootPath = (path: string) => {
  const trimmed = path.trim()
  if (!trimmed) {
    return false
  }
  if (trimmed === "/") {
    return true
  }
  return /^[a-zA-Z]:[\\/]*$/.test(trimmed)
}

const loadWorkspaceId = (): string => {
  const stored = window.localStorage.getItem(workspaceIdKey)
  if (stored) {
    return stored
  }
  const generated =
    typeof crypto !== "undefined" && "randomUUID" in crypto
      ? crypto.randomUUID()
      : `workspace-${Date.now()}`
  window.localStorage.setItem(workspaceIdKey, generated)
  return generated
}

const formatEvent = (payload: AppServerEvent) => {
  const method = payload.message?.method ?? "unknown"
  const params = payload.message?.params
  const time = nowTime()
  const header = `[${time}] ${method}`
  if (params === undefined) {
    return { time, text: header }
  }
  return { time, text: `${header}\n${JSON.stringify(params, null, 2)}` }
}

const formatSessionId = (id: string) => {
  const splitIndex = id.indexOf("-")
  return splitIndex === -1 ? id : id.slice(0, splitIndex)
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === "object" && value !== null

const readString = (record: Record<string, unknown>, key: string) => {
  const value = record[key]
  return typeof value === "string" ? value : null
}

const readNumber = (record: Record<string, unknown>, key: string) => {
  const value = record[key]
  return typeof value === "number" ? value : null
}

const readRecord = (record: Record<string, unknown>, key: string) => {
  const value = record[key]
  return isRecord(value) ? value : null
}

const readArray = (record: Record<string, unknown>, key: string) => {
  const value = record[key]
  return Array.isArray(value) ? value : null
}

const asNumber = (value: unknown): number => {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) {
      return parsed
    }
  }
  return 0
}

const normalizeRateLimits = (raw: Record<string, unknown>): RateLimitSnapshot => {
  const primary = readRecord(raw, "primary")
  const secondary = readRecord(raw, "secondary")
  const credits = readRecord(raw, "credits")
  return {
    primary: primary
      ? {
          usedPercent: asNumber(
            primary.usedPercent ?? primary.used_percent ?? primary.usedPercent,
          ),
          windowDurationMins: (() => {
            const value =
              primary.windowDurationMins ?? primary.window_duration_mins
            if (typeof value === "number") {
              return value
            }
            if (typeof value === "string") {
              const parsed = Number(value)
              return Number.isFinite(parsed) ? parsed : null
            }
            return null
          })(),
          resetsAt: (() => {
            const value = primary.resetsAt ?? primary.resets_at
            if (typeof value === "number") {
              return value
            }
            if (typeof value === "string") {
              const parsed = Number(value)
              return Number.isFinite(parsed) ? parsed : null
            }
            return null
          })(),
        }
      : null,
    secondary: secondary
      ? {
          usedPercent: asNumber(
            secondary.usedPercent ?? secondary.used_percent ?? secondary.usedPercent,
          ),
          windowDurationMins: (() => {
            const value =
              secondary.windowDurationMins ?? secondary.window_duration_mins
            if (typeof value === "number") {
              return value
            }
            if (typeof value === "string") {
              const parsed = Number(value)
              return Number.isFinite(parsed) ? parsed : null
            }
            return null
          })(),
          resetsAt: (() => {
            const value = secondary.resetsAt ?? secondary.resets_at
            if (typeof value === "number") {
              return value
            }
            if (typeof value === "string") {
              const parsed = Number(value)
              return Number.isFinite(parsed) ? parsed : null
            }
            return null
          })(),
        }
      : null,
    credits: credits
      ? {
          hasCredits: Boolean(credits.hasCredits ?? credits.has_credits),
          unlimited: Boolean(credits.unlimited),
          balance: typeof credits.balance === "string" ? credits.balance : null,
        }
      : null,
    planType:
      typeof raw.planType === "string"
        ? raw.planType
        : typeof raw.plan_type === "string"
          ? raw.plan_type
          : null,
  }
}

const extractRateLimitsPayload = (
  value: unknown,
): Record<string, unknown> | null => {
  if (!isRecord(value)) {
    return null
  }
  const direct =
    readRecord(value, "rateLimits") ??
    readRecord(value, "rate_limits") ??
    null
  if (direct) {
    return direct
  }
  if ("primary" in value || "secondary" in value || "credits" in value) {
    return value
  }
  const result = readRecord(value, "result")
  return result ? extractRateLimitsPayload(result) : null
}

const clampPercent = (value: number) =>
  Math.min(Math.max(Math.round(value), 0), 100)

const formatResetLabel = (resetsAt?: number | null) => {
  if (typeof resetsAt !== "number" || !Number.isFinite(resetsAt)) {
    return null
  }
  const resetMs = resetsAt > 1_000_000_000_000 ? resetsAt : resetsAt * 1000
  const relative = formatRelativeTime(resetMs).replace(/^in\s+/i, "")
  return `Resets ${relative}`
}

const compactDurationLabel = (label: string | null) => {
  if (!label) {
    return null
  }
  return label
    .replace(/\bhours\b/gi, "h")
    .replace(/\bhour\b/gi, "h")
    .replace(/\bminutes\b/gi, "m")
    .replace(/\bminute\b/gi, "m")
    .replace(/\bseconds\b/gi, "s")
    .replace(/\bsecond\b/gi, "s")
    .replace(/\bdays\b/gi, "d")
    .replace(/\bday\b/gi, "d")
    .replace(/\bweeks\b/gi, "w")
    .replace(/\bweek\b/gi, "w")
    .replace(/\bmonths\b/gi, "mo")
    .replace(/\bmonth\b/gi, "mo")
    .replace(/\byears\b/gi, "y")
    .replace(/\byear\b/gi, "y")
}

const formatCreditsLabel = (accountRateLimits: RateLimitSnapshot | null) => {
  const credits = accountRateLimits?.credits ?? null
  if (!credits?.hasCredits) {
    return null
  }
  if (credits.unlimited) {
    return "Credits: Unlimited"
  }
  const balance = credits.balance?.trim() ?? ""
  if (!balance) {
    return null
  }
  const intValue = Number.parseInt(balance, 10)
  if (Number.isFinite(intValue) && intValue > 0) {
    return `Credits: ${intValue} credits`
  }
  const floatValue = Number.parseFloat(balance)
  if (Number.isFinite(floatValue) && floatValue > 0) {
    const rounded = Math.round(floatValue)
    return rounded > 0 ? `Credits: ${rounded} credits` : null
  }
  return null
}

const getUsageLabels = (accountRateLimits: RateLimitSnapshot | null) => {
  const usagePercent = accountRateLimits?.primary?.usedPercent
  const globalUsagePercent = accountRateLimits?.secondary?.usedPercent
  const sessionPercent =
    typeof usagePercent === "number" ? clampPercent(usagePercent) : null
  const weeklyPercent =
    typeof globalUsagePercent === "number"
      ? clampPercent(globalUsagePercent)
      : null

  return {
    sessionPercent,
    weeklyPercent,
    sessionResetLabel: compactDurationLabel(
      formatResetLabel(accountRateLimits?.primary?.resetsAt),
    ),
    weeklyResetLabel: compactDurationLabel(
      formatResetLabel(accountRateLimits?.secondary?.resetsAt),
    ),
    creditsLabel: formatCreditsLabel(accountRateLimits),
    showWeekly: Boolean(accountRateLimits?.secondary),
  }
}

const normalizeErrorMessage = (message: string) => {
  const trimmed = message.trim()
  if (!(trimmed.startsWith("{") && trimmed.endsWith("}"))) {
    return message
  }
  try {
    const parsed = JSON.parse(trimmed)
    if (isRecord(parsed)) {
      const nestedError = readRecord(parsed, "error")
      const nestedMessage = nestedError ? readString(nestedError, "message") : null
      if (nestedMessage) {
        return nestedMessage
      }
    }
  } catch {
    return message
  }
  return message
}

const extractErrorMessage = (value: unknown) => {
  if (typeof value === "string") {
    return normalizeErrorMessage(value)
  }
  if (!isRecord(value)) {
    return null
  }
  const direct =
    readString(value, "message") ??
    readString(value, "detail") ??
    readString(value, "error")
  if (direct) {
    return normalizeErrorMessage(direct)
  }
  const nestedError = readRecord(value, "error")
  const nestedMessage = nestedError ? readString(nestedError, "message") : null
  return nestedMessage ? normalizeErrorMessage(nestedMessage) : null
}

const normalizeStatus = (value: unknown) => {
  if (typeof value !== "string") {
    return undefined
  }
  return value.replace(/([a-z])([A-Z])/g, "$1 $2").toLowerCase()
}

const resizeMessageInput = (container: Element) => {
  const input = container.querySelector(
    "#message-input"
  ) as HTMLTextAreaElement | null
  if (!input) {
    return
  }
  input.style.height = "auto"
  input.style.height = `${input.scrollHeight}px`
}

const focusMessageInput = (container: Element) =>
  Effect.sync(() => {
    const input = container.querySelector(
      "#message-input"
    ) as HTMLTextAreaElement | null
    if (!input) {
      return
    }
    try {
      input.focus({ preventScroll: true })
    } catch {
      input.focus()
    }
    resizeMessageInput(container)
  })

const formatJson = (value: unknown) => {
  if (value === undefined || value === null) {
    return undefined
  }
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return String(value)
  }
}

const formatChangeKind = (value: unknown) => {
  if (typeof value === "string") {
    return value
  }
  if (isRecord(value)) {
    const keys = Object.keys(value)
    if (keys.length > 0) {
      return keys[0]
    }
  }
  return undefined
}

const readPlanSteps = (value: unknown): CodexPlanStep[] => {
  if (!Array.isArray(value)) {
    return []
  }
  return value
    .map((entry) => {
      if (!isRecord(entry)) {
        return null
      }
      const step = readString(entry, "step")
      if (!step) {
        return null
      }
      const status = readString(entry, "status") ?? "pending"
      return { step, status }
    })
    .filter((entry): entry is CodexPlanStep => Boolean(entry))
}

const buildPlanItem = (
  threadId: string,
  explanation: string | null,
  steps: CodexPlanStep[]
): CodexConversationItem | null => {
  if (!explanation && steps.length === 0) {
    return null
  }
  return {
    kind: "plan",
    id: `plan-${threadId}`,
    explanation: explanation ?? undefined,
    steps,
  }
}

const extractThreadId = (value: unknown): string | null => {
  if (!value || typeof value !== "object") {
    return null
  }
  const readContainer = (container: unknown): string | null => {
    if (!container || typeof container !== "object") {
      return null
    }
    const record = container as Record<string, unknown>
    const direct =
      record.threadId ??
      record.thread_id ??
      record.conversationId ??
      record.conversation_id
    if (typeof direct === "string") {
      return direct
    }
    const thread = record.thread
    if (thread && typeof thread === "object") {
      const id = (thread as Record<string, unknown>).id
      if (typeof id === "string") {
        return id
      }
    }
    return null
  }
  const record = value as Record<string, unknown>
  return (
    readContainer(record) ||
    readContainer(record.result) ||
    readContainer(record.params)
  )
}

const extractTurnId = (params: Record<string, unknown>) => {
  const direct = readString(params, "turnId") ?? readString(params, "turn_id")
  if (direct) {
    return direct
  }
  const turn = readRecord(params, "turn")
  return turn ? readString(turn, "id") : null
}

const extractTurnErrorMessage = (params: Record<string, unknown>) => {
  const directError = extractErrorMessage(params.error)
  if (directError) {
    return directError
  }
  const turn = readRecord(params, "turn")
  if (!turn) {
    return null
  }
  const turnError = extractErrorMessage(turn.error)
  if (turnError) {
    return turnError
  }
  const status = readString(turn, "status")
  return status === "failed" ? "Turn failed." : null
}

const deriveStatus = (state: StatusState) => {
  if (!state.doctor.ok || !state.doctor.appServerOk) {
    return { level: "error" as const, label: "NOT READY" }
  }
  if (state.workspaceConnected) {
    return { level: "ok" as const, label: "CONNECTED" }
  }
  return { level: "warn" as const, label: "READY" }
}

const toSessionSummary = (thread: Record<string, unknown>): SessionSummary | null => {
  const id = readString(thread, "id")
  if (!id) {
    return null
  }
  const preview = readString(thread, "preview") ?? ""
  const updatedAt =
    readNumber(thread, "updatedAt") ?? readNumber(thread, "updated_at") ?? 0
  const createdAt =
    readNumber(thread, "createdAt") ?? readNumber(thread, "created_at") ?? 0
  const modelProvider =
    readString(thread, "modelProvider") ??
    readString(thread, "model_provider") ??
    ""
  return { id, preview, updatedAt, createdAt, modelProvider }
}

const sortSessions = (sessions: SessionSummary[]) =>
  [...sessions].sort(
    (a, b) => b.updatedAt - a.updatedAt || b.createdAt - a.createdAt
  )

const upsertSession = (sessions: SessionSummary[], session: SessionSummary) =>
  sortSessions([...sessions.filter((entry) => entry.id !== session.id), session])

const mergeSessionPreservingOrder = (
  sessions: SessionSummary[],
  session: SessionSummary
) => {
  const index = sessions.findIndex((entry) => entry.id === session.id)
  if (index === -1) {
    return [...sessions, session]
  }
  const existing = sessions[index]
  const merged = {
    ...existing,
    preview: session.preview || existing.preview,
    modelProvider: session.modelProvider || existing.modelProvider,
  }
  const next = [...sessions]
  next[index] = merged
  return next
}

const touchSession = (
  sessions: SessionSummary[],
  threadId: string,
  timestamp: number
) => {
  const existing = sessions.find((entry) => entry.id === threadId)
  if (!existing) {
    return sessions
  }
  const updated = { ...existing, updatedAt: timestamp }
  return upsertSession(sessions, updated)
}

const extractThread = (value: unknown): Record<string, unknown> | null => {
  if (!isRecord(value)) {
    return null
  }
  const direct = readRecord(value, "thread")
  if (direct) {
    return direct
  }
  const result = readRecord(value, "result")
  const resultThread = result ? readRecord(result, "thread") : null
  if (resultThread) {
    return resultThread
  }
  const params = readRecord(value, "params")
  const paramsThread = params ? readRecord(params, "thread") : null
  if (paramsThread) {
    return paramsThread
  }
  return null
}

const extractThreadList = (value: unknown): SessionSummary[] => {
  if (!isRecord(value)) {
    return []
  }
  const container = readRecord(value, "result") ?? value
  const data = container ? readArray(container, "data") : null
  if (!data) {
    return []
  }
  const sessions = data
    .map((entry) => (isRecord(entry) ? toSessionSummary(entry) : null))
    .filter((entry): entry is SessionSummary => Boolean(entry))
  return sortSessions(sessions)
}

const formatUserContent = (content: unknown) => {
  if (!Array.isArray(content)) {
    return "[user input]"
  }
  const parts = content
    .map((entry) => {
      if (!isRecord(entry)) {
        return null
      }
      const entryType = readString(entry, "type")
      if (entryType === "text") {
        return readString(entry, "text") ?? ""
      }
      if (entryType === "image") {
        const url = readString(entry, "url")
        return url ? `[image: ${url}]` : "[image]"
      }
      if (entryType === "localImage") {
        const path = readString(entry, "path")
        return path ? `[local image: ${path}]` : "[local image]"
      }
      if (entryType === "skill") {
        const name = readString(entry, "name")
        return name ? `[skill: ${name}]` : "[skill]"
      }
      return null
    })
    .filter((entry): entry is string => Boolean(entry))

  const combined = parts.join("\n")
  return combined || "[user input]"
}

const clampText = (value: string, limit: number) => {
  if (value.length <= limit) {
    return value
  }
  const trimmed = value.slice(0, limit)
  return `${trimmed}\n… (${value.length - limit} more chars)`
}

const joinFragments = (values: readonly unknown[], separator: string) => {
  const fragments = values
    .map((entry) => (typeof entry === "string" ? entry : null))
    .filter((entry): entry is string => Boolean(entry))
  if (!fragments.length) {
    return ""
  }
  if (fragments.length === 1) {
    return fragments[0]
  }
  const avgLength =
    fragments.reduce((sum, part) => sum + part.length, 0) / fragments.length
  const hasWhitespaceEdges = fragments.some(
    (part) => /^\s/.test(part) || /\s$/.test(part)
  )
  const hasNewlines = fragments.some((part) => part.includes("\n"))
  const looksLikeDelta =
    fragments.length > 4 && (avgLength < 12 || hasWhitespaceEdges || hasNewlines)
  return looksLikeDelta ? fragments.join("") : fragments.join(separator)
}

const mapThreadItem = (item: unknown): CodexConversationItem | null => {
  if (!isRecord(item)) {
    return null
  }
  const itemType = readString(item, "type")
  const id = readString(item, "id") ?? `item-${Date.now()}`

  if (itemType === "userMessage") {
    const content = clampText(
      formatUserContent(readArray(item, "content")),
      MAX_TEXT_LENGTH
    )
    return { kind: "message", id, role: "user", text: content }
  }

  if (itemType === "agentMessage") {
    return {
      kind: "message",
      id,
      role: "assistant",
      text: clampText(readString(item, "text") ?? "", MAX_TEXT_LENGTH),
    }
  }

  if (itemType === "reasoning") {
    const summaryValues = readArray(item, "summary") ?? []
    const contentValues = readArray(item, "content") ?? []
    let summary = joinFragments(summaryValues, " · ").trim()
    let content = joinFragments(contentValues, "\n")
    if (!content && summary.includes("\n")) {
      const lines = summary.split(/\r?\n/)
      const firstIndex = lines.findIndex((line) => line.trim() !== "")
      if (firstIndex !== -1) {
        const summaryLine = lines[firstIndex].trim()
        const remaining = lines.slice(firstIndex + 1).join("\n")
        summary = summaryLine || summary
        if (remaining.trim()) {
          content = remaining.replace(/^\n+/, "")
        }
      }
    }
    if (!summary && content) {
      const firstLine =
        content.split(/\r?\n/).find((line) => line.trim() !== "") ?? "Reasoning"
      summary = firstLine.trim() || "Reasoning"
    }

    return {
      kind: "reasoning",
      id,
      summary: clampText(summary, MAX_TEXT_LENGTH),
      content: clampText(content, MAX_TEXT_LENGTH),
    }
  }

  if (itemType === "plan") {
    const explanation = readString(item, "explanation")
    const steps = readPlanSteps(readArray(item, "steps") ?? readArray(item, "plan"))
    return {
      kind: "plan",
      id,
      explanation: explanation
        ? clampText(explanation, MAX_TEXT_LENGTH)
        : undefined,
      steps,
    }
  }

  if (itemType === "commandExecution") {
    const command = readString(item, "command") ?? "command"
    const cwd = readString(item, "cwd")
    const exitCode = readNumber(item, "exitCode")
    const commandActions = readArray(item, "commandActions")
    const detailParts = [
      cwd ? `cwd: ${cwd}` : null,
      exitCode !== null ? `exit: ${exitCode}` : null,
      commandActions && commandActions.length
        ? `actions: ${commandActions.length}`
        : null,
    ].filter((entry): entry is string => Boolean(entry))

    return {
      kind: "tool",
      id,
      title: command,
      detail: detailParts.length ? detailParts.join("\n") : undefined,
      output: readString(item, "aggregatedOutput")
        ? clampText(readString(item, "aggregatedOutput") ?? "", MAX_TOOL_OUTPUT_LENGTH)
        : undefined,
      status: normalizeStatus(item.status),
      durationMs: readNumber(item, "durationMs"),
    }
  }

  if (itemType === "fileChange") {
    const changes = readArray(item, "changes") ?? []
    const lines: string[] = []
    const changeCount = changes.length
    changes.forEach((change) => {
      if (!isRecord(change)) {
        return
      }
      const path = readString(change, "path") ?? "unknown"
      const kind = formatChangeKind(change.kind)
      const diff = readString(change, "diff") ?? ""
      const header = kind ? `--- ${path} [${kind}]` : `--- ${path}`
      lines.push([header, diff].filter(Boolean).join("\n"))
    })
    const diffText = clampText(
      lines.join("\n\n").trim() || "No diff recorded.",
      MAX_DIFF_LENGTH
    )

    return {
      kind: "diff",
      id,
      title: `File changes (${changeCount})`,
      diff: diffText,
      status: normalizeStatus(item.status),
    }
  }

  if (itemType === "mcpToolCall") {
    const server = readString(item, "server") ?? "mcp"
    const tool = readString(item, "tool") ?? "tool"
    const args = formatJson(item.arguments)
    const result = readRecord(item, "result") ?? item.result
    const error = readRecord(item, "error")
    const errorText = error ? readString(error, "message") : null
    const output = errorText ?? formatJson(result)

    return {
      kind: "tool",
      id,
      title: `${server}/${tool}`,
      detail: args ? clampText(args, MAX_TEXT_LENGTH) : args,
      output: output ? clampText(output, MAX_TOOL_OUTPUT_LENGTH) : output,
      status: normalizeStatus(item.status),
      durationMs: readNumber(item, "durationMs"),
    }
  }

  if (itemType === "collabAgentToolCall") {
    const tool = readString(item, "tool") ?? "collab"
    const prompt = readString(item, "prompt")
    const sender = readString(item, "senderThreadId")
    const receivers = readArray(item, "receiverThreadIds")
      ?.map((entry) => (typeof entry === "string" ? entry : null))
      .filter((entry): entry is string => Boolean(entry))
    const detailParts = [
      prompt,
      sender ? `from: ${sender}` : null,
      receivers && receivers.length ? `to: ${receivers.join(", ")}` : null,
    ].filter((entry): entry is string => Boolean(entry))

    return {
      kind: "tool",
      id,
      title: `collab/${tool}`,
      detail: detailParts.length
        ? clampText(detailParts.join("\n"), MAX_TEXT_LENGTH)
        : undefined,
      status: normalizeStatus(item.status),
    }
  }

  return null
}

const buildConversationItems = (thread: Record<string, unknown>) => {
  const turns = readArray(thread, "turns") ?? []
  const items: CodexConversationItem[] = []

  turns.forEach((turn) => {
    if (!isRecord(turn)) {
      return
    }
    const turnItems = readArray(turn, "items") ?? []
    turnItems.forEach((entry) => {
      const mapped = mapThreadItem(entry)
      if (mapped) {
        items.push(mapped)
      }
    })

    const errorMessage = extractErrorMessage(turn.error)
    if (errorMessage) {
      const turnId = readString(turn, "id") ?? `turn-${Date.now()}`
      items.push({
        kind: "message",
        id: `error-${turnId}`,
        role: "system",
        text: `Turn failed: ${errorMessage}`,
      })
    }
  })

  return items.length > MAX_ITEMS_PER_THREAD
    ? items.slice(items.length - MAX_ITEMS_PER_THREAD)
    : items
}

const upsertConversationItem = (
  sessionItems: Record<string, CodexConversationItem[]>,
  threadId: string,
  item: CodexConversationItem
) => {
  const existing = sessionItems[threadId] ?? []
  const index = existing.findIndex((entry) => entry.id === item.id)
  const nextItems =
    index === -1
      ? [...existing, item]
      : existing.map((entry, idx) => (idx === index ? item : entry))
  const clamped =
    nextItems.length > MAX_ITEMS_PER_THREAD
      ? nextItems.slice(nextItems.length - MAX_ITEMS_PER_THREAD)
      : nextItems
  return { ...sessionItems, [threadId]: clamped }
}

const renderConversationItem = (item: CodexConversationItem) => {
  switch (item.kind) {
    case "message":
      return Message({ role: item.role, text: item.text })
    case "reasoning":
      return Reasoning({
        summary: item.summary,
        content: item.content,
        open: Boolean(item.content),
      })
    case "plan":
      return Plan({
        explanation: item.explanation,
        steps: item.steps,
        open: true,
      })
    case "diff":
      return Diff({ title: item.title, diff: item.diff, status: item.status })
    case "tool":
      return ToolCall({
        title: item.title,
        detail: item.detail,
        output: item.output,
        status: item.status,
        durationMs: item.durationMs,
        changes: item.changes,
        toolType: item.toolType,
      })
    case "review":
      return Message({ role: "assistant", text: item.text })
  }
}

const invokeCommand = <T>(command: string, payload?: Record<string, unknown>) =>
  Effect.tryPromise({
    try: () => invoke<T>(command, payload),
    catch: (error) => new Error(String(error)),
  })

export const StatusDashboardComponent: Component<StatusState, StatusEvent> = {
  id: "status-dashboard",

  initialState: () => ({
    workspaceId: loadWorkspaceId(),
    workspacePath: "",
    workspaceConnected: false,
    workspaceMessage: "",
    doctor: {
      ok: false,
      appServerOk: false,
      version: null,
      codexBin: null,
      detail: "",
    },
    lastEventText: "Waiting for app-server events...",
    lastEventTime: "--",
    lastUpdated: nowTime(),
    messageInput: "",
    threadId: null,
    sessions: [],
    activeSessionId: null,
    sessionItems: {},
    sessionMessage: "",
    extraPaneOpen: false,
    leftSidebarCollapsed: false,
    rightSidebarCollapsed: false,
    storybookOpen: false,
    fullAutoEnabled: false,
    fullAutoThreadId: null,
    fullAutoMessage: "",
    rateLimits: null,
    logPaths: {
      appServerLogDir: "",
      fullAutoLogDir: "",
      traceBundleDir: "",
    },
    busy: {
      doctor: false,
      connect: false,
      disconnect: false,
      send: false,
      sessions: false,
      resume: false,
      newSession: false,
    },
  }),

  render: (ctx) =>
    Effect.gen(function* () {
      const state = yield* ctx.state.get
      const status = deriveStatus(state)
      const toneOk = "text-[color:var(--green)]"
      const toneWarn = "text-[color:var(--orange)]"
      const toneError = "text-[color:var(--red)]"
      const toneMuted = "text-[color:var(--muted)]"
      const statusClass =
        status.level === "ok"
          ? toneOk
          : status.level === "warn"
          ? toneWarn
          : toneError
      const cliStatus = state.doctor.version
        ? `OK ${state.doctor.version}`
        : "MISSING"
      const appServerStatus = state.doctor.appServerOk ? "READY" : "DOWN"
      const cliStatusClass = state.doctor.ok ? toneOk : toneError
      const appServerStatusClass = state.doctor.appServerOk ? toneOk : toneError
      const connectionLabel = state.workspaceConnected ? "CONNECTED" : "DISCONNECTED"
      const connectionClass = state.workspaceConnected ? toneOk : toneError
      const connectDisabled = state.busy.connect
      const disconnectDisabled = state.busy.disconnect || !state.workspaceConnected
      const refreshDisabled = state.busy.doctor
      const sendDisabled = state.busy.send
      const threadLabel = state.threadId ?? "--"
      const activeSessionId = state.activeSessionId ?? state.threadId
      const activeSession = activeSessionId
        ? state.sessions.find((entry) => entry.id === activeSessionId)
        : null
      const activeItems = activeSessionId
        ? state.sessionItems[activeSessionId] ?? []
        : []
      const sessionLabel = activeSessionId
        ? formatSessionId(activeSessionId)
        : "--"
      const sessionCount = state.sessions.length
      const sessionBusy = state.busy.sessions || state.busy.newSession
      const extraPaneOpen = state.extraPaneOpen
      const leftSidebarCollapsed = state.leftSidebarCollapsed
      const rightSidebarCollapsed = state.rightSidebarCollapsed
      const isMac =
        typeof navigator !== "undefined" &&
        /Mac|iPhone|iPad/.test(navigator.platform)
      const modifierLabel = isMac ? "Cmd+" : "Ctrl+"
      const storybookOpen = state.storybookOpen
      const canvasToggleLabel = extraPaneOpen ? "HIDE CANVAS" : "SHOW CANVAS"
      const fullAutoLabel = state.fullAutoEnabled
        ? state.fullAutoThreadId
          ? "RUNNING"
          : "ARMED"
        : "OFF"
      const fullAutoClass = state.fullAutoEnabled
        ? state.fullAutoThreadId
          ? toneOk
          : toneWarn
        : toneMuted
      const fullAutoThread = state.fullAutoThreadId ?? "--"
      const fullAutoEnableDisabled = state.fullAutoEnabled
      const fullAutoDisableDisabled = !state.fullAutoEnabled
      const usageLabels = getUsageLabels(state.rateLimits)
      const showAppServerFeed = false

      const sessionList = sessionCount
        ? state.sessions.map((session) => {
            const isActive = session.id === activeSessionId
            const preview = session.preview.trim()
            return html`
              <button
                class="w-full flex items-center gap-2 px-2.5 py-2 border-b border-[color:var(--line)] text-left font-[var(--font-mono)] text-[11px] min-w-0 ${
                  isActive
                    ? "bg-[color:var(--bg-accent)] text-[color:var(--yellow)]"
                    : "text-[color:var(--ink)] hover:bg-[color:var(--panel)]"
                }"
                data-action="select-session"
                data-session-id="${session.id}"
              >
                <div class="flex items-baseline gap-2 w-full min-w-0 text-[11px] whitespace-nowrap">
                  <span class="flex-none uppercase tracking-[0.1em]">
                    ${formatSessionId(session.id)}
                  </span>
                  ${
                    preview
                      ? html`<span class="flex-1 min-w-0 truncate text-[color:var(--muted)]">${preview}</span>`
                      : ""
                  }
                </div>
              </button>
            `
          })
        : html`<div class="px-2.5 py-3 text-[11px] text-[color:var(--muted)]">No sessions found.</div>`

      const conversationBody = activeSessionId
        ? activeItems.length
          ? Conversation({
              children: ConversationContent({
                children: activeItems.map(renderConversationItem),
              }),
            })
          : ConversationEmptyState({
              title: "No items yet",
              description: "This session has no recorded output yet.",
            })
        : ConversationEmptyState({
            title: "Select a session",
            description: "Choose a session to view its timeline.",
          })

      const summaryNode = CANVAS_NODES.summary
      const decisionNode = CANVAS_NODES.decision
      const guardrailNode = CANVAS_NODES.guardrail
      const actionNode = CANVAS_NODES.action
      const contextNode = CANVAS_NODES.context
      const guidanceNode = CANVAS_NODES.guidance
      const policyNode = CANVAS_NODES.policy
      const dispatchNode = CANVAS_NODES.dispatch
      const marketNode = CANVAS_NODES.market
      const optimizerNode = CANVAS_NODES.optimizer

      const bezierHorizontal = (
        sx: number,
        sy: number,
        ex: number,
        ey: number,
        offset = 60
      ) => {
        const direction = ex === sx ? 1 : Math.sign(ex - sx)
        const curve = direction * offset
        return `M ${sx} ${sy} C ${sx + curve} ${sy} ${ex - curve} ${ey} ${ex} ${ey}`
      }

      const bezierVertical = (
        sx: number,
        sy: number,
        ex: number,
        ey: number,
        offset = 60
      ) => {
        const direction = ey === sy ? 1 : Math.sign(ey - sy)
        const curve = direction * offset
        return `M ${sx} ${sy} C ${sx} ${sy + curve} ${ex} ${ey - curve} ${ex} ${ey}`
      }

      const leftX = (node: typeof summaryNode) => node.x
      const rightX = (node: typeof summaryNode) => node.x + node.width
      const topY = (node: typeof summaryNode) => node.y
      const bottomY = (node: typeof summaryNode) => node.y + node.height
      const midX = (node: typeof summaryNode) => node.x + node.width / 2
      const midY = (node: typeof summaryNode) => node.y + node.height / 2

      const edgeSummaryDecision = bezierHorizontal(
        rightX(summaryNode),
        midY(summaryNode),
        leftX(decisionNode),
        midY(decisionNode)
      )

      const edgeDecisionGuardrails = bezierHorizontal(
        rightX(decisionNode),
        midY(decisionNode),
        leftX(guardrailNode),
        midY(guardrailNode)
      )

      const edgeGuardrailsAction = bezierHorizontal(
        rightX(guardrailNode),
        midY(guardrailNode),
        leftX(actionNode),
        midY(actionNode)
      )

      const edgeContextGuidance = bezierVertical(
        midX(contextNode),
        bottomY(contextNode),
        midX(guidanceNode),
        topY(guidanceNode)
      )

      const edgeGuidancePolicy = bezierVertical(
        midX(guidanceNode),
        bottomY(guidanceNode),
        midX(policyNode),
        topY(policyNode)
      )

      const edgePolicyDispatch = bezierVertical(
        midX(policyNode),
        bottomY(policyNode),
        midX(dispatchNode),
        topY(dispatchNode)
      )

      const edgeGuidanceMarket = bezierHorizontal(
        rightX(guidanceNode),
        midY(guidanceNode),
        leftX(marketNode),
        midY(marketNode)
      )

      const edgeDispatchOptimizer = bezierHorizontal(
        rightX(dispatchNode),
        midY(dispatchNode),
        leftX(optimizerNode),
        midY(optimizerNode)
      )

      const edgeOptimizerGuidance = bezierVertical(
        midX(optimizerNode),
        topY(optimizerNode),
        midX(guidanceNode),
        bottomY(guidanceNode)
      )

      const canvasDemo = Canvas({
        title: "Full Auto Decision",
        subtitle: "Runs after each Codex turn completes",
        status: "DSPy",
        children: html`
          ${Edge({ path: edgeSummaryDecision })}
          ${Edge({ path: edgeDecisionGuardrails, dashed: true })}
          ${Edge({ path: edgeGuardrailsAction, animated: true })}
          ${Edge({ path: edgeContextGuidance })}
          ${Edge({ path: edgeGuidancePolicy })}
          ${Edge({ path: edgePolicyDispatch })}
          ${Edge({ path: edgeGuidanceMarket, dashed: true })}
          ${Edge({ path: edgeDispatchOptimizer, dashed: true })}
          ${Edge({ path: edgeOptimizerGuidance, dashed: true })}

          <div
            class="text-[10px] uppercase tracking-[0.2em] text-[color:var(--muted)]"
            style="position: absolute; left: 80px; top: 32px;"
          >
            Current (v1)
          </div>
          <div
            class="text-[10px] uppercase tracking-[0.2em] text-[color:var(--muted)]"
            style="position: absolute; left: 820px; top: 24px;"
          >
            Full System
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${summaryNode.x}px; top: ${summaryNode.y}px; width: ${summaryNode.width}px;"
          >
            ${Node({
              handles: { source: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Turn Summary" })}
                    ${NodeDescription({ text: "After turn/completed" })}
                    ${NodeAction({
                      children: html`<span class="text-xs text-muted-foreground">FullAutoTurnSummary</span>`,
                    })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Condense turn events into structured state snapshot.</div>
                    <div class="text-[11px] text-muted-foreground">last_turn_status, turn_error</div>
                    <div class="text-[11px] text-muted-foreground">turn_plan, diff_summary</div>
                    <div class="text-[11px] text-muted-foreground">token_usage</div>
                    <div class="text-[11px] text-muted-foreground">pending_approvals, pending_tool_inputs</div>
                    <div class="text-[11px] text-muted-foreground">compaction_events, recent_actions</div>
                    <div class="text-[11px] text-muted-foreground">turn_count, no_progress_count</div>
                  `,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${decisionNode.x}px; top: ${decisionNode.y}px; width: ${decisionNode.width}px;"
          >
            ${Node({
              handles: { target: true, source: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "DSPy Decision" })}
                    ${NodeDescription({ text: "FullAutoDecisionSignature" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">Predict</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Predict next action from typed turn summary.</div>
                    <div class="text-[11px] text-muted-foreground">action, next_input</div>
                    <div class="text-[11px] text-muted-foreground">reason, confidence</div>
                  `,
                })}
                ${NodeFooter({
                  children: html`<span class="text-[10px] text-muted-foreground">model: OPENAGENTS_FULL_AUTO_DECISION_MODEL</span>`,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${guardrailNode.x}px; top: ${guardrailNode.y}px; width: ${guardrailNode.width}px;"
          >
            ${Node({
              handles: { target: true, source: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Guardrails" })}
                    ${NodeDescription({ text: "enforce_guardrails()" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">FullAutoConfig</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Enforce hard limits, override unsafe decisions.</div>
                    <div class="text-[11px] text-muted-foreground">turn_failed → stop</div>
                    <div class="text-[11px] text-muted-foreground">turn_interrupted → pause</div>
                    <div class="text-[11px] text-muted-foreground">max_turns / max_tokens → stop</div>
                    <div class="text-[11px] text-muted-foreground">no_progress_limit → stop</div>
                    <div class="text-[11px] text-muted-foreground">low_confidence / review → pause</div>
                  `,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${actionNode.x}px; top: ${actionNode.y}px; width: ${actionNode.width}px;"
          >
            ${Node({
              handles: { target: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Dispatch" })}
                    ${NodeDescription({ text: "FullAutoAction" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">continue / pause / stop</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Start next turn or pause/stop loop.</div>
                    <div class="text-[11px] text-muted-foreground">continue → send next_input or fallback prompt</div>
                    <div class="text-[11px] text-muted-foreground">pause → wait for user</div>
                    <div class="text-[11px] text-muted-foreground">stop → end run</div>
                  `,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${contextNode.x}px; top: ${contextNode.y}px; width: ${contextNode.width}px;"
          >
            ${Node({
              handles: { bottom: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Context + Goals" })}
                    ${NodeDescription({ text: "Run state + intent" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">Inputs</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Aggregate goal, environment, budgets, permissions.</div>
                    <div class="text-[11px] text-muted-foreground">turn summary + constraints + history</div>
                  `,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${guidanceNode.x}px; top: ${guidanceNode.y}px; width: ${guidanceNode.width}px;"
          >
            ${Node({
              handles: { top: true, bottom: true, source: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Guidance Root" })}
                    ${NodeDescription({ text: "Module stack" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">DSPy</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Compose policies into a single guidance decision.</div>
                    <div class="text-[11px] text-muted-foreground">BudgetPolicy, StopDecider, NextActionSelector</div>
                  `,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${policyNode.x}px; top: ${policyNode.y}px; width: ${policyNode.width}px;"
          >
            ${Node({
              handles: { top: true, bottom: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Policy Gates" })}
                    ${NodeDescription({ text: "Hard constraints" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">Deterministic</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Apply deterministic safety and budget constraints.</div>
                    <div class="text-[11px] text-muted-foreground">override unsafe or low-confidence outputs</div>
                  `,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${dispatchNode.x}px; top: ${dispatchNode.y}px; width: ${dispatchNode.width}px;"
          >
            ${Node({
              handles: { top: true, source: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Dispatch + Record" })}
                    ${NodeDescription({ text: "Next turn" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">Effects</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Trigger next turn and emit decision record.</div>
                    <div class="text-[11px] text-muted-foreground">continue | pause | stop | review</div>
                  `,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${marketNode.x}px; top: ${marketNode.y}px; width: ${marketNode.width}px;"
          >
            ${Node({
              handles: { target: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Market Jobs" })}
                    ${NodeDescription({ text: "NIP-90" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">Procure</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Purchase verifiable micro-jobs when needed.</div>
                    <div class="text-[11px] text-muted-foreground">sandbox runs, indexing, analysis</div>
                  `,
                })}
              `,
            })}
          </div>

          <div
            class="canvas-flow-node"
            style="left: ${optimizerNode.x}px; top: ${optimizerNode.y}px; width: ${optimizerNode.width}px;"
          >
            ${Node({
              handles: { target: true, top: true },
              children: html`
                ${NodeHeader({
                  children: html`
                    ${NodeTitle({ text: "Optimizer + Bundles" })}
                    ${NodeDescription({ text: "DSPy + DSRS" })}
                    ${NodeAction({ children: html`<span class="text-xs text-muted-foreground">Compile</span>` })}
                  `,
                })}
                ${NodeContent({
                  children: html`
                    <div class="text-[11px] text-muted-foreground">Compile better modules from eval outcomes.</div>
                    <div class="text-[11px] text-muted-foreground">scorecards, manifests, promotion gates</div>
                  `,
                })}
              `,
            })}
          </div>
        `,
      })

      const conversationPanel = html`
        <section class="panel conversation-panel">
          <div class="panel-title">Session ${sessionLabel}</div>
          <div
            class="panel-body conversation-body"
            data-scroll-id="${activeSessionId
              ? `session-${activeSessionId}`
              : "session-none"}"
          >
            ${conversationBody}
          </div>
          ${state.sessionMessage ? html`<div class="note">${state.sessionMessage}</div>` : ""}
        </section>
      `

      const composeBar = html`
        <div class="compose-bar">
          <div class="compose-inner">
            <span class="compose-label">Input</span>
            <textarea
              id="message-input"
              class="compose-input"
              rows="1"
              placeholder="Type a message or /command"
            >${state.messageInput}</textarea>
            <button
              class="compose-submit"
              data-action="send-message"
              ${sendDisabled ? "disabled" : ""}
            >
              SEND
            </button>
          </div>
          <div class="compose-hints">Enter to send | /connect /disconnect /doctor /cd /new /auto /help</div>
        </div>
      `

      const mainContent = extraPaneOpen
        ? html`<div class="canvas-fullscreen">${canvasDemo}</div>`
        : html`${conversationPanel}${composeBar}`

      const mainPaneClass = extraPaneOpen
        ? "main-pane main-pane--fullscreen"
        : "main-pane"

      const storybookHost = html`<div id="effuse-storybook-host" class="storybook-pane"></div>`

      const hotbar = Hotbar({
        children: html`
          ${HotbarItem({
            slot: 1,
            label: "Sessions",
            icon: "SESS",
            shortcut: `${modifierLabel}1`,
            active: !leftSidebarCollapsed,
            dataAction: "hotbar-slot",
          })}
          ${HotbarItem({
            slot: 2,
            label: "New",
            icon: "NEW",
            shortcut: `${modifierLabel}2`,
            dataAction: "hotbar-slot",
          })}
          ${HotbarItem({
            slot: 3,
            label: "Guidance",
            icon: "GRID",
            shortcut: `${modifierLabel}3`,
            active: extraPaneOpen,
            dataAction: "hotbar-slot",
          })}
          ${HotbarItem({
            slot: 4,
            label: "Storybook",
            icon: "SB",
            shortcut: `${modifierLabel}4`,
            active: storybookOpen,
            dataAction: "hotbar-slot",
          })}
          ${HotbarItem({
            slot: 5,
            label: "—",
            icon: "--",
            shortcut: `${modifierLabel}5`,
            ghost: true,
            disabled: true,
          })}
          ${HotbarItem({
            slot: 6,
            label: "—",
            icon: "--",
            shortcut: `${modifierLabel}6`,
            ghost: true,
            disabled: true,
          })}
          ${HotbarItem({
            slot: 7,
            label: "—",
            icon: "--",
            shortcut: `${modifierLabel}7`,
            ghost: true,
            disabled: true,
          })}
          ${HotbarItem({
            slot: 8,
            label: "Settings",
            icon: "SET",
            shortcut: `${modifierLabel}8`,
            ghost: true,
            disabled: true,
          })}
          ${HotbarItem({
            slot: 9,
            label: "Help",
            icon: "HELP",
            shortcut: `${modifierLabel}9`,
            ghost: true,
            disabled: true,
          })}
        `,
      })

      const leftSidebar = leftSidebarCollapsed
        ? ""
        : html`
            <aside class="w-[240px] min-h-0 shrink-0 flex flex-col border-r border-[color:var(--line)] bg-[color:var(--panel-alt)] max-[900px]:w-full max-[900px]:max-h-[200px]">
              <div class="flex items-center justify-between gap-2 border-b border-[color:var(--line)] px-2.5 py-2 text-[11px] uppercase tracking-[0.12em] text-[color:var(--yellow)]">
                <span>Sessions</span>
                <div class="flex items-center gap-2">
                  <button
                    class="border border-[color:var(--line)] bg-[color:var(--panel)] text-[color:var(--green)] text-[10px] uppercase tracking-[0.12em] px-1.5 py-0.5 disabled:opacity-60 disabled:cursor-not-allowed"
                    data-action="new-session"
                    ${state.busy.newSession || !state.workspaceConnected
                      ? "disabled"
                      : ""}
                  >
                    NEW
                  </button>
                  <span class="text-[10px] text-[color:var(--muted)]">
                    ${sessionBusy ? "SYNC" : String(sessionCount).padStart(2, "0")}
                  </span>
                </div>
              </div>
              <div class="flex-1 overflow-auto" data-scroll-id="session-list">
                ${sessionList}
              </div>
            </aside>
          `

      const rightSidebar = rightSidebarCollapsed
        ? ""
        : html`
            <aside class="w-[240px] min-h-0 shrink-0 flex flex-col border-l border-[color:var(--line)] bg-[color:var(--panel-alt)] max-[900px]:w-full max-[900px]:max-h-[240px] max-[900px]:border-l-0 max-[900px]:border-t max-[900px]:border-[color:var(--line)]">
              <div class="flex items-center justify-between gap-2 border-b border-[color:var(--line)] px-2.5 py-2 text-[11px] uppercase tracking-[0.12em] text-[color:var(--yellow)]">
                <span>Status</span>
                <div class="flex items-center gap-2">
                  <button
                    class="border border-[color:var(--line)] bg-[color:var(--panel)] text-[color:var(--ink)] px-2 py-0.5 text-[10px] uppercase tracking-[0.12em] whitespace-nowrap"
                    data-action="toggle-canvas"
                  >
                    ${canvasToggleLabel}
                  </button>
                  <span class="${statusClass}">${status.label}</span>
                </div>
              </div>
              <div class="flex-1 min-h-0 overflow-auto flex flex-col">
                <section class="flex flex-col gap-2 border-b border-[color:var(--line)] px-2.5 py-2 min-h-0">
                  <div class="text-[11px] uppercase tracking-[0.12em] text-[color:var(--yellow)]">System</div>
                  <div class="flex flex-col gap-2 min-h-0">
                    <div class="grid grid-cols-[minmax(90px,120px)_minmax(0,1fr)] gap-x-2.5 gap-y-1.5 text-[12px]">
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">CLI</div>
                      <div class="min-w-0 break-words ${cliStatusClass}">${cliStatus}</div>
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">App-Server</div>
                      <div class="min-w-0 break-words ${appServerStatusClass}">${appServerStatus}</div>
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">Binary</div>
                      <div class="min-w-0 break-words font-[var(--font-mono)] text-[color:var(--ink)]">${state.doctor.codexBin ?? "default"}</div>
                    </div>
                    <div class="text-[11px] text-[color:var(--muted)]">${state.doctor.detail || "No diagnostics."}</div>
                    <div class="flex flex-wrap gap-1.5">
                      <button
                        class="border border-[color:var(--line)] bg-[color:var(--panel-alt)] text-[color:var(--ink)] px-2 py-1 text-[10px] uppercase tracking-[0.1em] disabled:opacity-60 disabled:cursor-not-allowed"
                        data-action="refresh-doctor"
                        ${refreshDisabled ? "disabled" : ""}
                      >
                        DOCTOR
                      </button>
                    </div>
                  </div>
                </section>

                <section class="flex flex-col gap-2 border-b border-[color:var(--line)] px-2.5 py-2 min-h-0">
                  <div class="text-[11px] uppercase tracking-[0.12em] text-[color:var(--yellow)]">Workspace</div>
                  <div class="flex flex-col gap-2 min-h-0">
                    <div class="flex flex-col gap-1">
                      <div class="flex items-center justify-between gap-2">
                        <span class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">
                          Working Dir
                        </span>
                        <button
                          class="text-[10px] uppercase tracking-[0.1em] text-[color:var(--accent-strong)] hover:text-[color:var(--accent)]"
                          data-action="browse-workspace"
                        >
                          Browse
                        </button>
                      </div>
                      <input
                        id="workspace-path"
                        class="border border-[color:var(--line)] bg-[color:var(--bg)] text-[color:var(--ink)] px-1.5 py-1 font-[var(--font-mono)] text-[12px] focus:outline focus:outline-1 focus:outline-[color:var(--ring)]"
                        type="text"
                        placeholder="/path/to/workspace"
                        value="${state.workspacePath}"
                      />
                    </div>
                    <div class="flex flex-wrap gap-1.5">
                      <button
                        class="border border-[color:rgba(32,195,116,0.4)] bg-[color:var(--panel-alt)] text-[color:var(--green)] px-2 py-1 text-[10px] uppercase tracking-[0.1em] disabled:opacity-60 disabled:cursor-not-allowed"
                        data-action="connect"
                        ${connectDisabled ? "disabled" : ""}
                      >
                        CONNECT
                      </button>
                      <button
                        class="border border-[color:rgba(245,158,11,0.4)] bg-[color:var(--panel-alt)] text-[color:var(--orange)] px-2 py-1 text-[10px] uppercase tracking-[0.1em] disabled:opacity-60 disabled:cursor-not-allowed"
                        data-action="disconnect"
                        ${disconnectDisabled ? "disabled" : ""}
                      >
                        DISCONNECT
                      </button>
                    </div>
                    <div class="grid grid-cols-[minmax(90px,120px)_minmax(0,1fr)] gap-x-2.5 gap-y-1.5 text-[12px]">
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">Workspace</div>
                      <div class="min-w-0 break-words font-[var(--font-mono)] text-[color:var(--ink)]">${state.workspaceId}</div>
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">Connection</div>
                      <div class="min-w-0 break-words ${connectionClass}">${connectionLabel}</div>
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">Last Event</div>
                      <div class="min-w-0 break-words text-[color:var(--ink)]">${state.lastEventTime}</div>
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">Thread</div>
                      <div class="min-w-0 break-words font-[var(--font-mono)] text-[color:var(--ink)]">${threadLabel}</div>
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">Updated</div>
                      <div class="min-w-0 break-words text-[color:var(--ink)]">${state.lastUpdated}</div>
                    </div>
                    <div class="text-[11px] text-[color:var(--muted)]">${state.workspaceMessage || ""}</div>
                  </div>
                </section>

                <section class="flex flex-col gap-2 border-b border-[color:var(--line)] px-2.5 py-2 min-h-0">
                  <div class="text-[11px] uppercase tracking-[0.12em] text-[color:var(--yellow)]">Full Auto</div>
                  <div class="flex flex-col gap-2 min-h-0">
                    <div class="grid grid-cols-[minmax(90px,120px)_minmax(0,1fr)] gap-x-2.5 gap-y-1.5 text-[12px]">
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">State</div>
                      <div class="min-w-0 break-words ${fullAutoClass}">${fullAutoLabel}</div>
                      <div class="text-[10px] uppercase tracking-[0.08em] text-[color:var(--muted)] min-w-0">Thread</div>
                      <div class="min-w-0 break-words font-[var(--font-mono)] text-[color:var(--ink)]">${fullAutoThread}</div>
                    </div>
                    <div class="flex flex-wrap gap-1.5">
                      <button
                        class="border border-[color:rgba(32,195,116,0.4)] bg-[color:var(--panel-alt)] text-[color:var(--green)] px-2 py-1 text-[10px] uppercase tracking-[0.1em] disabled:opacity-60 disabled:cursor-not-allowed"
                        data-action="full-auto-enable"
                        ${fullAutoEnableDisabled ? "disabled" : ""}
                      >
                        ENABLE
                      </button>
                      <button
                        class="border border-[color:rgba(245,158,11,0.4)] bg-[color:var(--panel-alt)] text-[color:var(--orange)] px-2 py-1 text-[10px] uppercase tracking-[0.1em] disabled:opacity-60 disabled:cursor-not-allowed"
                        data-action="full-auto-disable"
                        ${fullAutoDisableDisabled ? "disabled" : ""}
                      >
                        DISABLE
                      </button>
                    </div>
                    <div class="text-[11px] text-[color:var(--muted)]">
                      ${state.fullAutoMessage || "Runs with full access and no prompts."}
                    </div>
                  </div>
                </section>

                ${"" /* Logs + Usage hidden per request. */}

                ${
                  showAppServerFeed
                    ? html`
                        <section class="flex flex-col gap-2 border-b border-[color:var(--line)] px-2.5 py-2 min-h-[160px] max-h-[240px] flex-none">
                          <div class="text-[11px] uppercase tracking-[0.12em] text-[color:var(--yellow)]">App-Server Feed</div>
                          <div class="flex flex-col gap-2 min-h-0 flex-1">
                            <pre
                              class="flex-1 min-h-0 bg-[color:var(--bg)] border border-[color:var(--line)] p-1.5 text-[11px] leading-[1.35] overflow-auto whitespace-pre-wrap"
                              data-scroll-id="event-log"
                            >${state.lastEventText}</pre>
                          </div>
                        </section>
                      `
                    : ""
                }
              </div>
              <div class="border-t border-[color:var(--line)] px-2.5 py-2 text-[10px] uppercase tracking-[0.12em] flex items-center gap-2">
                <span class="text-[color:var(--yellow)]">Version</span>
                <span class="text-[color:var(--ink)]">${appVersion}</span>
              </div>
            </aside>
          `

      return html`
        <div class="terminal">
          <div class="flex min-h-0 flex-1 max-[900px]:flex-col">
            ${leftSidebar}

            <main class="${mainPaneClass}">
              ${mainContent}
              ${storybookHost}
            </main>

            ${rightSidebar}
          </div>
          ${hotbar}
        </div>
      `
    }),

  handleEvent: (event, ctx) =>
    Effect.gen(function* () {
      const runCommand = (commandText: string) =>
        Effect.gen(function* () {
          const command = commandText.trim()
          if (!command) {
            return
          }
          const [rawHead, ...rest] = command.split(/\s+/)
          const head = rawHead.toLowerCase()
          const arg = rest.join(" ").trim()

          if (head === "connect") {
            yield* ctx.emit({ type: "ConnectWorkspace" })
            return
          }
          if (head === "disconnect") {
            yield* ctx.emit({ type: "DisconnectWorkspace" })
            return
          }
          if (head === "doctor" || head === "refresh") {
            yield* ctx.emit({ type: "RefreshDoctor" })
            yield* ctx.emit({ type: "RefreshWorkspaceStatus" })
            yield* ctx.emit({ type: "RefreshSessions" })
            return
          }
          if (head === "sessions" || head === "history") {
            yield* ctx.emit({ type: "RefreshSessions" })
            return
          }
          if (head === "cd" || head === "cwd") {
            if (!arg) {
              yield* ctx.state.update((state) => ({
                ...state,
                workspaceMessage: "Command requires a path.",
                lastUpdated: nowTime(),
              }))
              return
            }
            window.localStorage.setItem(workspacePathKey, arg)
            yield* ctx.state.update((state) => ({
              ...state,
              workspacePath: arg,
              workspaceMessage: `Working dir set to ${arg}`,
              lastUpdated: nowTime(),
            }))
            return
          }
          if (head === "new" || head === "thread") {
            yield* ctx.emit({ type: "StartNewSession" })
            return
          }
          if (head === "auto" || head === "full-auto") {
            const normalized = arg.toLowerCase()
            if (
              normalized === "off" ||
              normalized === "stop" ||
              normalized === "disable"
            ) {
              yield* ctx.emit({ type: "SetFullAuto", enabled: false })
              return
            }
            if (!normalized || normalized === "on" || normalized === "enable") {
              yield* ctx.emit({ type: "SetFullAuto", enabled: true })
              return
            }
            yield* ctx.emit({ type: "StartFullAuto", prompt: arg })
            return
          }
          if (head === "help" || head === "?") {
            yield* ctx.state.update((state) => ({
              ...state,
              workspaceMessage:
                "Commands: /connect /disconnect /doctor /cd <path> /new /auto /help",
              lastUpdated: nowTime(),
            }))
            return
          }

          yield* ctx.state.update((state) => ({
            ...state,
            workspaceMessage: `Unknown command: ${command}`,
            lastUpdated: nowTime(),
          }))
        })

      if (event.type === "UpdateWorkspacePath") {
        yield* ctx.state.update((current) => ({
          ...current,
          workspacePath: event.path,
        }))
        return
      }

      if (event.type === "UpdateMessageInput") {
        yield* ctx.state
          .update((current) => ({
            ...current,
            messageInput: event.value,
          }))
          .pipe(
            Effect.tap(() =>
              Effect.sync(() =>
                requestAnimationFrame(() => resizeMessageInput(ctx.container))
              )
            )
          )
        return
      }

      if (event.type === "ToggleExtraPane") {
        const current = yield* ctx.state.get
        const willOpen = !current.extraPaneOpen
        yield* ctx.state.update((state) => ({
          ...state,
          extraPaneOpen: willOpen,
        }))
        if (willOpen) {
          yield* Effect.sync(() =>
            requestAnimationFrame(() => centerCanvasInContainer(ctx.container))
          )
        }
        return
      }

      if (event.type === "ToggleLeftSidebar") {
        yield* ctx.state.update((state) => ({
          ...state,
          leftSidebarCollapsed: !state.leftSidebarCollapsed,
        }))
        return
      }

      if (event.type === "ToggleRightSidebar") {
        yield* ctx.state.update((state) => ({
          ...state,
          rightSidebarCollapsed: !state.rightSidebarCollapsed,
        }))
        return
      }

      if (event.type === "ToggleSidebars") {
        const current = yield* ctx.state.get
        const shouldCollapse = !(current.leftSidebarCollapsed && current.rightSidebarCollapsed)
        yield* ctx.state.update((state) => ({
          ...state,
          leftSidebarCollapsed: shouldCollapse,
          rightSidebarCollapsed: shouldCollapse,
        }))
        return
      }

      if (event.type === "StorybookToggle") {
        yield* ctx.state.update((state) => ({
          ...state,
          storybookOpen: event.isOpen,
        }))
        return
      }

      if (event.type === "SetFullAuto") {
        const current = yield* ctx.state.get
        const threadId =
          current.activeSessionId ?? current.threadId ?? current.fullAutoThreadId
        const enabled = event.enabled
        const continuePrompt = event.continuePrompt ?? null

        yield* invokeCommand<unknown>("set_full_auto", {
          workspaceId: current.workspaceId,
          enabled,
          threadId: enabled ? threadId : null,
          continuePrompt,
        }).pipe(
          Effect.tap(() =>
            ctx.state.update((state) => ({
              ...state,
              fullAutoEnabled: enabled,
              fullAutoThreadId: enabled ? threadId ?? null : null,
              fullAutoMessage: enabled
                ? threadId
                  ? "Full Auto enabled."
                  : "Full Auto armed. Send a message to start."
                : "Full Auto disabled.",
              workspaceMessage: enabled
                ? "Full Auto enabled."
                : "Full Auto disabled.",
              lastUpdated: nowTime(),
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              fullAutoMessage: `Full Auto update failed: ${String(error)}`,
              workspaceMessage: `Full Auto update failed: ${String(error)}`,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "StartFullAuto") {
        const rawInput = event.prompt.trim()
        if (!rawInput) {
          yield* ctx.state.update((state) => ({
            ...state,
            workspaceMessage: "Full Auto needs a prompt to start.",
            lastUpdated: nowTime(),
          }))
          return
        }

        const current = yield* ctx.state.get
        if (!current.workspaceConnected) {
          yield* ctx.state.update((state) => ({
            ...state,
            workspaceMessage: "Connect the workspace before starting Full Auto.",
            lastUpdated: nowTime(),
          }))
          return
        }

        const existingThreadId = current.threadId
        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, send: true },
          workspaceMessage: "Starting Full Auto...",
          sessions: existingThreadId
            ? touchSession(state.sessions, existingThreadId, nowEpochSeconds())
            : state.sessions,
        }))

        let threadId = current.threadId
        if (!threadId) {
          const response = yield* invokeCommand<unknown>("start_thread", {
            workspaceId: current.workspaceId,
          })
          threadId = extractThreadId(response)
          if (!threadId) {
            yield* ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, send: false },
                workspaceMessage: "Failed to start a new thread for Full Auto.",
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
            return
          }
          const thread = extractThread(response)
          const summary = thread ? toSessionSummary(thread) : null
          const fallbackTimestamp = nowEpochSeconds()
          const fallbackSummary: SessionSummary = {
            id: threadId,
            preview: "",
            updatedAt: fallbackTimestamp,
            createdAt: fallbackTimestamp,
            modelProvider: "",
          }
          yield* ctx.state.update((state) => ({
            ...state,
            threadId,
            activeSessionId: threadId,
            sessions: summary
              ? upsertSession(state.sessions, summary)
              : upsertSession(state.sessions, fallbackSummary),
            sessionItems: {
              ...state.sessionItems,
              [threadId]: state.sessionItems[threadId] ?? [],
            },
          }))
        }

        const enabled = yield* invokeCommand<unknown>("set_full_auto", {
          workspaceId: current.workspaceId,
          enabled: true,
          threadId,
          continuePrompt: null,
        }).pipe(
          Effect.as(true),
          Effect.catchAll((error) =>
            ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, send: false },
                workspaceMessage: `Full Auto enable failed: ${String(error)}`,
                fullAutoMessage: `Full Auto enable failed: ${String(error)}`,
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
              .pipe(Effect.as(false))
          )
        )

        if (!enabled) {
          return
        }

        yield* ctx.state.update((state) => ({
          ...state,
          fullAutoEnabled: true,
          fullAutoThreadId: threadId ?? null,
          fullAutoMessage: "Full Auto running.",
        }))

        yield* invokeCommand<unknown>("send_user_message", {
          workspaceId: current.workspaceId,
          threadId,
          text: rawInput,
          model: null,
          accessMode: "full-access",
        }).pipe(
          Effect.tap(() =>
            ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, send: false },
                messageInput: "",
                workspaceMessage: "Full Auto started.",
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
          ),
          Effect.catchAll((error) =>
            ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, send: false },
                workspaceMessage: `Full Auto send failed: ${String(error)}`,
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
          ),
          Effect.asVoid
        )

        return
      }

      if (event.type === "SubmitMessage") {
        const rawInput = event.value.trim()
        if (!rawInput) {
          yield* ctx.state.update((state) => ({
            ...state,
            messageInput: "",
          }))
          return
        }

        if (rawInput.startsWith("/")) {
          yield* runCommand(rawInput.slice(1))
          yield* ctx.state.update((state) => ({
            ...state,
            messageInput: "",
          }))
          return
        }

        const current = yield* ctx.state.get
        if (!current.workspaceConnected) {
          yield* ctx.state.update((state) => ({
            ...state,
            workspaceMessage: "Connect the workspace before sending a message.",
            lastUpdated: nowTime(),
          }))
          return
        }

        const existingThreadId = current.threadId
        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, send: true },
          workspaceMessage: "Sending message...",
          sessions: existingThreadId
            ? touchSession(state.sessions, existingThreadId, nowEpochSeconds())
            : state.sessions,
        }))

        let threadId = current.threadId
        if (!threadId) {
          const response = yield* invokeCommand<unknown>("start_thread", {
            workspaceId: current.workspaceId,
          })
          threadId = extractThreadId(response)
          if (!threadId) {
            yield* ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, send: false },
                workspaceMessage: "Failed to start a new thread.",
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
            return
          }
          const thread = extractThread(response)
          const summary = thread ? toSessionSummary(thread) : null
          const fallbackTimestamp = nowEpochSeconds()
          const fallbackSummary: SessionSummary | null = threadId
            ? {
                id: threadId,
                preview: "",
                updatedAt: fallbackTimestamp,
                createdAt: fallbackTimestamp,
                modelProvider: "",
              }
            : null
          yield* ctx.state.update((state) => ({
            ...state,
            threadId,
            activeSessionId: threadId,
            sessions: summary
              ? upsertSession(state.sessions, summary)
              : fallbackSummary
                ? upsertSession(state.sessions, fallbackSummary)
                : state.sessions,
            sessionItems: {
              ...state.sessionItems,
              [threadId]: state.sessionItems[threadId] ?? [],
            },
          }))
        }

        if (current.fullAutoEnabled && threadId) {
          yield* invokeCommand<unknown>("set_full_auto", {
            workspaceId: current.workspaceId,
            enabled: true,
            threadId,
            continuePrompt: null,
          }).pipe(
            Effect.tap(() =>
              ctx.state.update((state) => ({
                ...state,
                fullAutoThreadId: threadId,
                fullAutoMessage: "Full Auto armed for this thread.",
                lastUpdated: nowTime(),
              }))
            ),
            Effect.catchAll((error) =>
              ctx.state.update((state) => ({
                ...state,
                fullAutoMessage: `Full Auto update failed: ${String(error)}`,
                lastUpdated: nowTime(),
              }))
            ),
            Effect.asVoid
          )
        }

        yield* invokeCommand<unknown>("send_user_message", {
          workspaceId: current.workspaceId,
          threadId,
          text: rawInput,
          model: null,
          accessMode: current.fullAutoEnabled ? "full-access" : null,
        }).pipe(
          Effect.tap(() =>
            ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, send: false },
                messageInput: "",
                workspaceMessage: "Message sent.",
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
          ),
          Effect.catchAll((error) =>
            ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, send: false },
                workspaceMessage: `Send failed: ${String(error)}`,
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "RefreshDoctor") {
        yield* ctx.state.update((current) => ({
          ...current,
          busy: { ...current.busy, doctor: true },
          doctor: { ...current.doctor, detail: "Checking codex CLI..." },
        }))

        yield* invokeCommand<CodexDoctorResponse>("codex_doctor", {
          codexBin: null,
        }).pipe(
          Effect.tap((response) =>
            ctx.state.update((current) => ({
              ...current,
              doctor: {
                ok: response.ok,
                appServerOk: response.appServerOk,
                version: response.version,
                codexBin: response.codexBin,
                detail: response.details ?? "",
              },
              busy: { ...current.busy, doctor: false },
              lastUpdated: nowTime(),
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((current) => ({
              ...current,
              doctor: {
                ok: false,
                appServerOk: false,
                version: null,
                codexBin: null,
                detail: String(error),
              },
              busy: { ...current.busy, doctor: false },
              lastUpdated: nowTime(),
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "RefreshWorkspaceStatus") {
        const current = yield* ctx.state.get
        yield* invokeCommand<WorkspaceConnectionStatusResponse>(
          "get_workspace_connection_status",
          { workspaceId: current.workspaceId }
        ).pipe(
          Effect.tap((response) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: response.connected,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.tap((response) =>
            response.connected
              ? ctx.emit({ type: "RefreshSessions" })
              : Effect.sync(() => undefined)
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: false,
              workspaceMessage: `Status check failed: ${String(error)}`,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "RefreshSessions") {
        const current = yield* ctx.state.get
        if (!current.workspaceConnected) {
          yield* ctx.state.update((state) => ({
            ...state,
            sessions: [],
            sessionMessage: "Connect a workspace to list sessions.",
          }))
          return
        }

        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, sessions: true },
          sessionMessage: "Loading sessions...",
        }))

        yield* invokeCommand<unknown>("list_threads", {
          workspaceId: current.workspaceId,
          cursor: null,
          limit: 50,
          sortKey: "updated_at",
          archived: false,
        }).pipe(
          Effect.tap((response) =>
            ctx.state.update((state) => ({
              ...state,
              sessions: extractThreadList(response),
              busy: { ...state.busy, sessions: false },
              sessionMessage: "",
              lastUpdated: nowTime(),
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              busy: { ...state.busy, sessions: false },
              sessionMessage: `Failed to load sessions: ${String(error)}`,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "SelectSession") {
        const current = yield* ctx.state.get
        if (!current.workspaceConnected) {
          yield* ctx.state.update((state) => ({
            ...state,
            sessionMessage: "Connect the workspace before resuming sessions.",
            lastUpdated: nowTime(),
          }))
          return
        }

        yield* ctx.state.update((state) => ({
          ...state,
          activeSessionId: event.threadId,
          threadId: event.threadId,
          busy: { ...state.busy, resume: true },
          sessionMessage: "Resuming session...",
        }))

        yield* invokeCommand<unknown>("resume_thread", {
          workspaceId: current.workspaceId,
          threadId: event.threadId,
        }).pipe(
          Effect.tap((response) => {
            const thread = extractThread(response)
            if (!thread) {
              return ctx.state
                .update((state) => ({
                  ...state,
                  busy: { ...state.busy, resume: false },
                  sessionMessage: "Failed to parse session response.",
                  lastUpdated: nowTime(),
                }))
                .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
            }
            const summary = toSessionSummary(thread)
            const items = buildConversationItems(thread)
            return ctx.state
              .update((state) => ({
                ...state,
                sessions: summary
                  ? mergeSessionPreservingOrder(state.sessions, summary)
                  : state.sessions,
                sessionItems: {
                  ...state.sessionItems,
                  [event.threadId]: items,
                },
                busy: { ...state.busy, resume: false },
                sessionMessage: "",
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
          }),
          Effect.catchAll((error) =>
            ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, resume: false },
                sessionMessage: `Resume failed: ${String(error)}`,
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "StartNewSession") {
        const current = yield* ctx.state.get
        if (current.busy.newSession) {
          return
        }
        if (!current.workspaceConnected) {
          yield* ctx.state.update((state) => ({
            ...state,
            sessionMessage: "Connect the workspace before starting a session.",
            lastUpdated: nowTime(),
          }))
          return
        }

        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, newSession: true },
          sessionMessage: "Starting new session...",
        }))

        yield* invokeCommand<unknown>("start_thread", {
          workspaceId: current.workspaceId,
        }).pipe(
          Effect.tap((response) => {
            const threadId = extractThreadId(response)
            if (!threadId) {
              return ctx.state
                .update((state) => ({
                  ...state,
                  busy: { ...state.busy, newSession: false },
                  sessionMessage: "Failed to start a new session.",
                  lastUpdated: nowTime(),
                }))
                .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
            }
            const thread = extractThread(response)
            const summary = thread ? toSessionSummary(thread) : null
            const timestamp = nowEpochSeconds()
            const fallbackSummary: SessionSummary = {
              id: threadId,
              preview: "",
              updatedAt: timestamp,
              createdAt: timestamp,
              modelProvider: "",
            }
            return ctx.state
              .update((state) => ({
                ...state,
                threadId,
                activeSessionId: threadId,
                sessions: summary
                  ? upsertSession(state.sessions, summary)
                  : upsertSession(state.sessions, fallbackSummary),
                sessionItems: {
                  ...state.sessionItems,
                  [threadId]: [],
                },
                busy: { ...state.busy, newSession: false },
                sessionMessage: "",
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
          }),
          Effect.catchAll((error) =>
            ctx.state
              .update((state) => ({
                ...state,
                busy: { ...state.busy, newSession: false },
                sessionMessage: `Start session failed: ${String(error)}`,
                lastUpdated: nowTime(),
              }))
              .pipe(Effect.tap(() => focusMessageInput(ctx.container)))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "BrowseWorkspace") {
        const selection = yield* Effect.tryPromise({
          try: () =>
            open({
              directory: true,
              multiple: false,
              title: "Select working directory",
            }),
          catch: (error) => new Error(String(error)),
        })

        if (!selection) {
          return
        }

        const selectedPath = Array.isArray(selection)
          ? selection[0]
          : selection

        if (!selectedPath || isRootPath(selectedPath)) {
          yield* ctx.state.update((state) => ({
            ...state,
            workspaceMessage: rootWorkspaceMessage,
            lastUpdated: nowTime(),
          }))
          return
        }

        window.localStorage.setItem(workspacePathKey, selectedPath)
        yield* ctx.state.update((state) => ({
          ...state,
          workspacePath: selectedPath,
          workspaceMessage: `Working dir set to ${selectedPath}`,
          lastUpdated: nowTime(),
        }))
        return
      }

      if (event.type === "ConnectWorkspace") {
        const current = yield* ctx.state.get
        const workspacePath = current.workspacePath.trim()
          if (!workspacePath) {
            yield* ctx.state.update((state) => ({
              ...state,
              workspaceMessage: "Enter a working directory first.",
              lastUpdated: nowTime(),
          }))
          return
        }

        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, connect: true },
          workspaceMessage: "Connecting to app-server...",
        }))

        yield* invokeCommand<WorkspaceConnectionResponse>("connect_workspace", {
          workspaceId: current.workspaceId,
          workspacePath,
          codexBin: null,
        }).pipe(
          Effect.tap((response) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: response.success,
              workspaceMessage: response.message,
              busy: { ...state.busy, connect: false },
              threadId: response.success ? null : state.threadId,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.tap((response) =>
            response.success
              ? invokeCommand<unknown>("account_rate_limits", {
                  workspaceId: current.workspaceId,
                }).pipe(
                  Effect.tap((rateResponse) => {
                    const payload = extractRateLimitsPayload(rateResponse)
                    if (!payload) {
                      return Effect.sync(() => undefined)
                    }
                    return ctx.state.update((state) => ({
                      ...state,
                      rateLimits: normalizeRateLimits(payload),
                    }))
                  }),
                  Effect.catchAll(() => Effect.sync(() => undefined)),
                  Effect.asVoid,
                )
              : Effect.sync(() => undefined),
          ),
          Effect.tap((response) =>
            response.success
              ? ctx.emit({ type: "RefreshSessions" })
              : Effect.sync(() => undefined)
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: false,
              workspaceMessage: `Connection failed: ${String(error)}`,
              busy: { ...state.busy, connect: false },
              lastUpdated: nowTime(),
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "DisconnectWorkspace") {
        const current = yield* ctx.state.get
        yield* ctx.state.update((state) => ({
          ...state,
          busy: { ...state.busy, disconnect: true },
          workspaceMessage: "Disconnecting...",
        }))

        yield* invokeCommand<WorkspaceConnectionResponse>("disconnect_workspace", {
          workspaceId: current.workspaceId,
        }).pipe(
          Effect.tap((response) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceConnected: false,
              workspaceMessage: response.message,
              busy: { ...state.busy, disconnect: false },
              threadId: null,
              sessions: [],
              activeSessionId: null,
              sessionItems: {},
              sessionMessage: "",
              fullAutoEnabled: false,
              fullAutoThreadId: null,
              fullAutoMessage: "",
              rateLimits: null,
              lastUpdated: nowTime(),
            }))
          ),
          Effect.catchAll((error) =>
            ctx.state.update((state) => ({
              ...state,
              workspaceMessage: `Disconnect failed: ${String(error)}`,
              busy: { ...state.busy, disconnect: false },
              lastUpdated: nowTime(),
            }))
          ),
          Effect.asVoid
        )
        return
      }

      if (event.type === "AppServerEvent") {
        const current = yield* ctx.state.get
        if (event.payload.workspace_id !== current.workspaceId) {
          return
        }
        const formatted = formatEvent(event.payload)
        const eventThreadId = extractThreadId(event.payload.message)
        const method =
          typeof event.payload.message?.method === "string"
            ? event.payload.message.method
            : ""
        const params = isRecord(event.payload.message?.params)
          ? event.payload.message.params
          : null

        let sessions = current.sessions
        let sessionItems = current.sessionItems
        let activeSessionId = current.activeSessionId
        let fullAutoThreadId = current.fullAutoThreadId

        if (method === "thread/started") {
          const thread = extractThread(event.payload.message)
          if (thread) {
            const summary = toSessionSummary(thread)
            if (summary) {
              sessions = upsertSession(sessions, summary)
              if (!activeSessionId) {
                activeSessionId = summary.id
              }
            }
          }
        }

        if (params && method === "app/log_paths") {
          yield* ctx.state.update((state) => ({
            ...state,
            logPaths: {
              appServerLogDir: readString(params, "appServerLogDir") ?? "",
              fullAutoLogDir: readString(params, "fullAutoLogDir") ?? "",
              traceBundleDir: readString(params, "traceBundleDir") ?? "",
            },
            lastUpdated: nowTime(),
          }))
          return
        }

        if (params && method === "fullauto/decision") {
          const action = readString(params, "action") ?? "pause"
          const reason = readString(params, "reason") ?? ""
          const confidenceValue = readNumber(params, "confidence")
          const decisionState = readString(params, "state") ?? "paused"
          const nextInput = readString(params, "nextInput") ?? ""
          const decisionThreadId = readString(params, "threadId") ?? eventThreadId
          const confidenceText =
            confidenceValue !== null ? ` (${confidenceValue.toFixed(2)})` : ""
          const nextInputText = nextInput ? ` Next: ${nextInput}` : ""
          const message = `${action.toUpperCase()}${confidenceText} — ${reason}${nextInputText}`.trim()

          yield* ctx.state.update((state) => ({
            ...state,
            fullAutoEnabled: decisionState === "running",
            fullAutoThreadId: decisionThreadId ?? state.fullAutoThreadId,
            fullAutoMessage: message,
            workspaceMessage: message,
            lastUpdated: nowTime(),
          }))
          return
        }

        if (method === "account/rateLimits/updated" && params) {
          const payload = extractRateLimitsPayload(params)
          if (payload) {
            yield* ctx.state.update((state) => ({
              ...state,
              rateLimits: normalizeRateLimits(payload),
            }))
          }
        }

        if (
          current.fullAutoEnabled &&
          !fullAutoThreadId &&
          eventThreadId &&
          (method === "thread/started" ||
            method === "turn/started" ||
            method === "turn/completed")
        ) {
          fullAutoThreadId = eventThreadId
        }

        if (method === "item/completed" && params && isRecord(params.item)) {
          const mapped = mapThreadItem(params.item)
          if (mapped && eventThreadId) {
            sessionItems = upsertConversationItem(sessionItems, eventThreadId, mapped)
            if (!activeSessionId) {
              activeSessionId = eventThreadId
            }
          }
        }

        if (
          eventThreadId &&
          params &&
          (method === "turn/completed" || method === "turn/error")
        ) {
          const errorMessage = extractTurnErrorMessage(params)
          if (errorMessage) {
            const turnId = extractTurnId(params) ?? `turn-${Date.now()}`
            sessionItems = upsertConversationItem(sessionItems, eventThreadId, {
              kind: "message",
              id: `error-${turnId}`,
              role: "system",
              text: `Turn failed: ${errorMessage}`,
            })
            if (!activeSessionId) {
              activeSessionId = eventThreadId
            }
          }
        }

        if (params && (method === "turn/plan/updated" || method === "codex/event/plan_update")) {
          let planThreadId = eventThreadId
          let explanation: string | null = null
          let steps: CodexPlanStep[] = []

          if (method === "turn/plan/updated") {
            planThreadId = readString(params, "threadId") ?? planThreadId
            explanation = readString(params, "explanation")
            steps = readPlanSteps(readArray(params, "plan"))
          } else {
            const msg = readRecord(params, "msg")
            planThreadId =
              readString(params, "conversationId") ??
              (msg ? readString(msg, "thread_id") : null) ??
              planThreadId
            explanation = msg ? readString(msg, "explanation") : null
            steps = readPlanSteps(msg ? readArray(msg, "plan") : null)
          }

          if (planThreadId) {
            const planItem = buildPlanItem(planThreadId, explanation, steps)
            if (planItem) {
              sessionItems = upsertConversationItem(sessionItems, planThreadId, planItem)
              if (!activeSessionId) {
                activeSessionId = planThreadId
              }
            }
          }
        }

        if (eventThreadId && method === "item/completed") {
          sessions = touchSession(sessions, eventThreadId, nowEpochSeconds())
        }

        const shouldUpdateThread =
          event.payload.message?.method === "thread/started" ||
          current.threadId === null
        yield* ctx.state.update((state) => ({
          ...state,
          lastEventText: formatted.text,
          lastEventTime: formatted.time,
          lastUpdated: formatted.time,
          workspaceConnected:
            event.payload.message?.method === "codex/connected"
              ? true
              : state.workspaceConnected,
          threadId:
            shouldUpdateThread && eventThreadId ? eventThreadId : state.threadId,
          sessions,
          sessionItems,
          activeSessionId,
          fullAutoThreadId,
        }))
      }
    }),

  setupEvents: (ctx) =>
    Effect.gen(function* () {
      const emit = (event: StatusEvent) => {
        Effect.runFork(ctx.emit(event))
      }

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"refresh-doctor\"]",
        "click",
        () => emit({ type: "RefreshDoctor" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"connect\"]",
        "click",
        () => emit({ type: "ConnectWorkspace" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"browse-workspace\"]",
        "click",
        () => emit({ type: "BrowseWorkspace" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"disconnect\"]",
        "click",
        () => emit({ type: "DisconnectWorkspace" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"new-session\"]",
        "click",
        () => emit({ type: "StartNewSession" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"full-auto-enable\"]",
        "click",
        () => emit({ type: "SetFullAuto", enabled: true })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"full-auto-disable\"]",
        "click",
        () => emit({ type: "SetFullAuto", enabled: false })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"toggle-canvas\"]",
        "click",
        () => emit({ type: "ToggleExtraPane" })
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"select-session\"]",
        "click",
        (_event, target) => {
          const threadId = target.getAttribute("data-session-id")
          if (threadId) {
            emit({ type: "SelectSession", threadId })
          }
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "#workspace-path",
        "input",
        (event, target) => {
          void event
          const value = (target as HTMLInputElement).value
          window.localStorage.setItem(workspacePathKey, value)
          emit({ type: "UpdateWorkspacePath", path: value })
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "#message-input",
        "input",
        (event, target) => {
          void event
          const value = (target as HTMLTextAreaElement).value
          resizeMessageInput(ctx.container)
          emit({ type: "UpdateMessageInput", value })
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "#message-input",
        "keydown",
        (event, target) => {
          const keyEvent = event as KeyboardEvent
          if (keyEvent.key === "Enter") {
            if (keyEvent.shiftKey) {
              return
            }
            keyEvent.preventDefault()
            emit({
              type: "SubmitMessage",
              value: (target as HTMLTextAreaElement).value,
            })
          }
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"send-message\"]",
        "click",
        () => {
          const input = ctx.container.querySelector(
            "#message-input"
          ) as HTMLTextAreaElement | null
          emit({
            type: "SubmitMessage",
            value: input?.value ?? "",
          })
        }
      )

      yield* ctx.dom.delegate(
        ctx.container,
        "[data-action=\"hotbar-slot\"]",
        "click",
        (_event, target) => {
          const button = target.closest("[data-action=\"hotbar-slot\"]") as HTMLElement | null
          const slot = Number(button?.getAttribute("data-hotbar-slot") ?? "")
          if (!Number.isFinite(slot)) {
            return
          }
          if (slot === 1) {
            emit({ type: "ToggleLeftSidebar" })
            return
          }
          if (slot === 2) {
            emit({ type: "StartNewSession" })
            return
          }
          if (slot === 3) {
            emit({ type: "ToggleExtraPane" })
            return
          }
          if (slot === 4) {
            const isMac =
              typeof navigator !== "undefined" &&
              /Mac|iPhone|iPad/.test(navigator.platform)
            window.dispatchEvent(
              new KeyboardEvent("keydown", {
                key: "4",
                code: "Digit4",
                metaKey: isMac,
                ctrlKey: !isMac,
                bubbles: true,
              })
            )
          }
        }
      )

      const handleStorybookToggle = (event: Event) => {
        const detail = (event as CustomEvent<{ isOpen?: boolean }>).detail
        if (typeof detail?.isOpen === "boolean") {
          emit({ type: "StorybookToggle", isOpen: detail.isOpen })
        }
      }

      window.addEventListener("storybook:toggle", handleStorybookToggle as EventListener)

      const handleKeydown = (event: KeyboardEvent) => {
        if ((event.metaKey || event.ctrlKey) && event.code.startsWith("Digit")) {
          const digit = Number(event.code.replace("Digit", ""))
          if (Number.isFinite(digit) && digit >= 1 && digit <= 9) {
            if (digit === 4) {
              return
            }
            event.preventDefault()
            if (digit === 1) {
              emit({ type: "ToggleLeftSidebar" })
              return
            }
            if (digit === 2) {
              emit({ type: "StartNewSession" })
              return
            }
            if (digit === 3) {
              emit({ type: "ToggleExtraPane" })
              return
            }
            return
          }
        }
        if (event.metaKey && event.code === "BracketLeft") {
          event.preventDefault()
          emit({ type: "ToggleLeftSidebar" })
          return
        }
        if (event.metaKey && event.code === "BracketRight") {
          event.preventDefault()
          emit({ type: "ToggleRightSidebar" })
          return
        }
        if (event.metaKey && event.code === "Backslash") {
          event.preventDefault()
          emit({ type: "ToggleSidebars" })
          return
        }
        if (event.key === "F3") {
          event.preventDefault()
          emit({ type: "DisconnectWorkspace" })
        }
        if (event.key === "F5") {
          event.preventDefault()
          emit({ type: "RefreshDoctor" })
          emit({ type: "RefreshWorkspaceStatus" })
          emit({ type: "RefreshSessions" })
        }
      }

      window.addEventListener("keydown", handleKeydown)

      yield* Effect.addFinalizer(() =>
        Effect.sync(() => {
          window.removeEventListener("keydown", handleKeydown)
          window.removeEventListener("storybook:toggle", handleStorybookToggle as EventListener)
        })
      )

      let panState: {
        active: boolean
        pointerId: number | null
        startX: number
        startY: number
        startTx: number
        startTy: number
        scale: number
        canvas: HTMLElement | null
      } = {
        active: false,
        pointerId: null,
        startX: 0,
        startY: 0,
        startTx: 0,
        startTy: 0,
        scale: 1,
        canvas: null,
      }

      const handlePointerDown = (event: PointerEvent) => {
        const target = event.target as HTMLElement | null
        const canvas = target?.closest(".ai-canvas") as HTMLElement | null
        if (!canvas || event.button !== 0) {
          return
        }
        if (target?.closest("input, textarea, button, select, a")) {
          return
        }
        event.preventDefault()
        const transform = readCanvasTransform(canvas)
        panState = {
          active: true,
          pointerId: event.pointerId,
          startX: event.clientX,
          startY: event.clientY,
          startTx: transform.tx,
          startTy: transform.ty,
          scale: transform.scale,
          canvas,
        }
        canvas.classList.add("ai-canvas--panning")
        canvas.setPointerCapture(event.pointerId)
      }

      const handlePointerMove = (event: PointerEvent) => {
        if (!panState.active || !panState.canvas) {
          return
        }
        const dx = event.clientX - panState.startX
        const dy = event.clientY - panState.startY
        writeCanvasTransform(panState.canvas, {
          scale: panState.scale,
          tx: panState.startTx + dx,
          ty: panState.startTy + dy,
        })
      }

      const handlePointerUp = (event: PointerEvent) => {
        if (!panState.active) {
          return
        }
        const canvas = panState.canvas
        if (canvas) {
          canvas.classList.remove("ai-canvas--panning")
          if (panState.pointerId !== null) {
            canvas.releasePointerCapture(panState.pointerId)
          }
        }
        panState = {
          active: false,
          pointerId: null,
          startX: 0,
          startY: 0,
          startTx: 0,
          startTy: 0,
          scale: panState.scale,
          canvas: null,
        }
        void event
      }

      const handleWheel = (event: WheelEvent) => {
        const target = event.target as HTMLElement | null
        const canvas = target?.closest(".ai-canvas") as HTMLElement | null
        if (!canvas) {
          return
        }
        event.preventDefault()
        const rect = canvas.getBoundingClientRect()
        const transform = readCanvasTransform(canvas)
        const zoomFactor = Math.exp(-event.deltaY * 0.0012)
        const nextScale = clampNumber(
          transform.scale * zoomFactor,
          CANVAS_SCALE_MIN,
          CANVAS_SCALE_MAX
        )
        if (nextScale === transform.scale) {
          return
        }
        const pointerX = event.clientX - rect.left
        const pointerY = event.clientY - rect.top
        const contentX = (pointerX - transform.tx) / transform.scale
        const contentY = (pointerY - transform.ty) / transform.scale
        const nextTx = pointerX - contentX * nextScale
        const nextTy = pointerY - contentY * nextScale
        writeCanvasTransform(canvas, { scale: nextScale, tx: nextTx, ty: nextTy })
      }

      ctx.container.addEventListener("pointerdown", handlePointerDown)
      window.addEventListener("pointermove", handlePointerMove)
      window.addEventListener("pointerup", handlePointerUp)
      window.addEventListener("pointercancel", handlePointerUp)
      ctx.container.addEventListener("wheel", handleWheel, { passive: false })

      yield* Effect.addFinalizer(() =>
        Effect.sync(() => {
          ctx.container.removeEventListener("pointerdown", handlePointerDown)
          window.removeEventListener("pointermove", handlePointerMove)
          window.removeEventListener("pointerup", handlePointerUp)
          window.removeEventListener("pointercancel", handlePointerUp)
          ctx.container.removeEventListener("wheel", handleWheel)
        })
      )

      const unlisten = yield* Effect.tryPromise({
        try: () =>
          listen<AppServerEvent>("app-server-event", (event) => {
            emit({ type: "AppServerEvent", payload: event.payload })
          }),
        catch: (error) => new Error(String(error)),
      })

      yield* Effect.addFinalizer(() => Effect.sync(() => unlisten()))

      const initialize = Effect.gen(function* () {
        const storedPath = window.localStorage.getItem(workspacePathKey)
        if (storedPath && storedPath.trim()) {
          if (isRootPath(storedPath)) {
            yield* ctx.state.update((state) => ({
              ...state,
              workspacePath: "",
              workspaceMessage: rootWorkspaceMessage,
            }))
            window.localStorage.removeItem(workspacePathKey)
          } else {
            yield* ctx.state.update((state) => ({
              ...state,
              workspacePath: storedPath,
            }))
          }
        } else {
          yield* invokeCommand<string>("get_current_directory").pipe(
            Effect.tap((cwd) => {
              if (isRootPath(cwd)) {
                return ctx.state.update((state) => ({
                  ...state,
                  workspacePath: "",
                  workspaceMessage: rootWorkspaceMessage,
                }))
              }
              return ctx.state.update((state) => ({
                ...state,
                workspacePath: cwd,
              }))
            }),
            Effect.tap((cwd) => {
              if (isRootPath(cwd)) {
                return Effect.sync(() =>
                  window.localStorage.removeItem(workspacePathKey)
                )
              }
              return Effect.sync(() =>
                window.localStorage.setItem(workspacePathKey, cwd)
              )
            }),
            Effect.catchAll((error) =>
              ctx.state.update((state) => ({
                ...state,
                workspaceMessage: `Failed to read current directory: ${String(error)}`,
              }))
            ),
            Effect.asVoid
          )
        }

        yield* ctx.emit({ type: "RefreshDoctor" })
        yield* ctx.emit({ type: "RefreshWorkspaceStatus" })
        const current = yield* ctx.state.get
        if (current.workspacePath.trim()) {
          yield* ctx.emit({ type: "ConnectWorkspace" })
        }
      })

      yield* Effect.forkScoped(initialize)

      yield* focusMessageInput(ctx.container)
      resizeMessageInput(ctx.container)
    }),
}

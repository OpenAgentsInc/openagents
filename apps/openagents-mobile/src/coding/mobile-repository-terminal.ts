import type { MobileRepositoryScope } from "./mobile-repository-files"

export const MOBILE_TERMINAL_MAX_SESSIONS = 12
export const MOBILE_TERMINAL_MAX_TAIL = 100_000
export const MOBILE_TERMINAL_MAX_INPUT_BYTES = 8_192
export const MOBILE_TERMINAL_MAX_EVENTS = 500

export type MobileTerminalSession = Readonly<{
  terminalRef: string
  sessionVersionRef: string
  label: string
  shellLabel: string
  status: "running" | "exited" | "reconnecting"
  exitCode: number | null
  cols: number
  rows: number
  lastSeq: number
  gap: boolean
  recovered: boolean
  tail: string
}>

export type MobileTerminalSnapshot = MobileRepositoryScope & Readonly<{
  snapshotRef: string
  sessions: ReadonlyArray<MobileTerminalSession>
  truncated: boolean
}>

export type MobileTerminalReplayPage = MobileRepositoryScope & Readonly<{
  terminalRef: string
  sessionVersionRef: string
  afterSeq: number
  toSeq: number
  gap: boolean
  truncated: boolean
  events: ReadonlyArray<Readonly<{
    seq: number
    kind: "output" | "exit"
    data: string
    exitCode: number | null
  }>>
}>

export type MobileTerminalCommand = "input" | "resize" | "interrupt" | "restart" | "close"
export type MobileTerminalCommandRequest = MobileRepositoryScope & Readonly<{
  terminalRef: string
  sessionVersionRef: string
  op: MobileTerminalCommand
  idempotencyRef: string
  data?: string
  cols?: number
  rows?: number
}>

export type MobileTerminalCommandReceipt = MobileRepositoryScope & Readonly<{
  terminalRef: string
  requestVersionRef: string
  op: MobileTerminalCommand
  receiptRef: string
  sessionVersionRef: string
  status: "running" | "exited" | "closed"
  recordedAt: string
}>

export type MobileRepositoryTerminalPort = Readonly<{
  terminalSnapshot: (request: MobileRepositoryScope) => Promise<unknown>
  terminalCreate: (request: MobileRepositoryScope & Readonly<{ cols: number; rows: number; idempotencyRef: string }>) => Promise<unknown>
  terminalReplay: (request: MobileRepositoryScope & Readonly<{ terminalRef: string; sessionVersionRef: string; afterSeq: number; limit: number }>) => Promise<unknown>
  terminalCommand: (request: MobileTerminalCommandRequest) => Promise<unknown>
}>

export type MobileRepositoryTerminalState = Readonly<{
  scope: MobileRepositoryScope | null
  state: "idle" | "loading" | "ready" | "unavailable" | "failed"
  snapshotRef: string | null
  sessions: ReadonlyArray<MobileTerminalSession>
  activeRef: string | null
  requestEpoch: number
  submitting: boolean
  message: string | null
  lastReceipt: MobileTerminalCommandReceipt | null
}>

export const initialMobileRepositoryTerminalState: MobileRepositoryTerminalState = {
  scope: null,
  state: "idle",
  snapshotRef: null,
  sessions: [],
  activeRef: null,
  requestEpoch: 0,
  submitting: false,
  message: null,
  lastReceipt: null,
}

const safeRef = (value: unknown): value is string => typeof value === "string" && value.length > 0 && value.length <= 256 &&
  /^[A-Za-z0-9][A-Za-z0-9._:-]*$/u.test(value)
const nonNegative = (value: unknown): value is number => typeof value === "number" && Number.isSafeInteger(value) && value >= 0
const grid = (value: unknown, max: number): value is number => nonNegative(value) && value >= 1 && value <= max
const exactScope = (value: Record<string, unknown>, scope: MobileRepositoryScope) =>
  value.sessionRef === scope.sessionRef && value.repositoryRef === scope.repositoryRef && value.worktreeRef === scope.worktreeRef

const decodeSession = (value: unknown): MobileTerminalSession | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (!safeRef(row.terminalRef) || !safeRef(row.sessionVersionRef) || typeof row.label !== "string" || row.label.length > 80 ||
    typeof row.shellLabel !== "string" || row.shellLabel.length > 80 ||
    (row.status !== "running" && row.status !== "exited" && row.status !== "reconnecting") ||
    !(row.exitCode === null || (typeof row.exitCode === "number" && Number.isSafeInteger(row.exitCode))) ||
    !grid(row.cols, 1_000) || !grid(row.rows, 1_000) || !nonNegative(row.lastSeq) ||
    typeof row.gap !== "boolean" || typeof row.recovered !== "boolean" || typeof row.tail !== "string" ||
    new TextEncoder().encode(row.tail).byteLength > MOBILE_TERMINAL_MAX_TAIL || row.tail.includes("\0")) return null
  return row as MobileTerminalSession
}

export const decodeMobileTerminalSnapshot = (value: unknown, scope: MobileRepositoryScope): MobileTerminalSnapshot | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (!exactScope(row, scope) || !safeRef(row.snapshotRef) || !Array.isArray(row.sessions) ||
    row.sessions.length > MOBILE_TERMINAL_MAX_SESSIONS || typeof row.truncated !== "boolean") return null
  const seen = new Set<string>()
  const sessions: MobileTerminalSession[] = []
  for (const candidate of row.sessions) {
    const session = decodeSession(candidate)
    if (session === null || seen.has(session.terminalRef)) return null
    seen.add(session.terminalRef)
    sessions.push(session)
  }
  return { ...scope, snapshotRef: row.snapshotRef, sessions, truncated: row.truncated }
}

export const decodeMobileTerminalReplay = (
  value: unknown,
  request: MobileRepositoryScope & Readonly<{ terminalRef: string; sessionVersionRef: string; afterSeq: number; limit: number }>,
): MobileTerminalReplayPage | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  if (!exactScope(row, request) || row.terminalRef !== request.terminalRef || row.sessionVersionRef !== request.sessionVersionRef ||
    row.afterSeq !== request.afterSeq || !nonNegative(row.toSeq) || row.toSeq < request.afterSeq ||
    typeof row.gap !== "boolean" || typeof row.truncated !== "boolean" || !Array.isArray(row.events) ||
    row.events.length > Math.min(request.limit, MOBILE_TERMINAL_MAX_EVENTS)) return null
  let expected = request.afterSeq + 1
  const events: Array<{ seq: number; kind: "output" | "exit"; data: string; exitCode: number | null }> = []
  let bytes = 0
  for (const candidate of row.events) {
    if (typeof candidate !== "object" || candidate === null) return null
    const event = candidate as Record<string, unknown>
    if (event.seq !== expected || (event.kind !== "output" && event.kind !== "exit") || typeof event.data !== "string" ||
      event.data.includes("\0") || !(event.exitCode === null || (typeof event.exitCode === "number" && Number.isSafeInteger(event.exitCode)))) return null
    bytes += new TextEncoder().encode(event.data).byteLength
    if (bytes > MOBILE_TERMINAL_MAX_TAIL) return null
    expected += 1
    events.push(event as typeof events[number])
  }
  if ((events.at(-1)?.seq ?? request.afterSeq) !== row.toSeq) return null
  return { ...request, toSeq: row.toSeq, gap: row.gap, truncated: row.truncated, events }
}

export const decodeMobileTerminalCommandReceipt = (
  value: unknown,
  request: MobileTerminalCommandRequest,
): MobileTerminalCommandReceipt | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as Record<string, unknown>
  return exactScope(row, request) && row.terminalRef === request.terminalRef && row.requestVersionRef === request.sessionVersionRef &&
    row.op === request.op && safeRef(row.receiptRef) && safeRef(row.sessionVersionRef) &&
    (row.status === "running" || row.status === "exited" || row.status === "closed") && typeof row.recordedAt === "string" &&
    /^\d{4}-\d{2}-\d{2}T/u.test(row.recordedAt)
    ? { ...request, requestVersionRef: request.sessionVersionRef, receiptRef: row.receiptRef, sessionVersionRef: row.sessionVersionRef,
      status: row.status, recordedAt: row.recordedAt }
    : null
}

export const applyMobileTerminalReplay = (
  session: MobileTerminalSession,
  page: MobileTerminalReplayPage,
): MobileTerminalSession => {
  let tail = page.gap ? "[Earlier terminal output unavailable]\n" : session.tail
  let status = session.status
  let exitCode = session.exitCode
  for (const event of page.events) {
    if (event.kind === "output") tail += event.data
    else { status = "exited"; exitCode = event.exitCode }
  }
  if (tail.length > MOBILE_TERMINAL_MAX_TAIL) tail = tail.slice(-MOBILE_TERMINAL_MAX_TAIL)
  return { ...session, status, exitCode, lastSeq: page.toSeq, gap: session.gap || page.gap || page.truncated, tail }
}

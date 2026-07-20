import { chmodSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs"
import path from "node:path"

import type { DesktopThread } from "./chat-contract.ts"
import type {
  ProviderLaneComposerProjection,
  ProviderLaneFeatureKey,
} from "./provider-lane-capabilities.ts"
import type { ProviderLaneHistoryMessage } from "./provider-lane.ts"

export const ProviderLaneRegistryListChannel = "openagents:provider-lanes:list" as const
export const ProviderLaneRegistrySelectChannel = "openagents:provider-lanes:select" as const

export type ProviderLaneAuthentication = "ready" | "missing" | "unknown"

/**
 * Bug #8998: the live-authentication mapping every native lane (`codex-local`,
 * `claude-local`) must use to compute its `ProviderLaneRegistryEntry.authentication`
 * field. `switchThread` below refuses any lane whose `authentication !== "ready"`
 * with `missing_auth` -- so a caller that never probes live (or that trusts a
 * cache nothing populates) permanently strands a genuinely-authenticated
 * account at the "unknown" default and every switch is wrongly refused. This
 * helper exists so callers (main.ts's `providerLaneEntries()`) always derive
 * `authentication` from an actual `{ state: "available" | "unavailable" }`
 * probe result -- never from a passive cache alone. See this file's test
 * suite for the regression contract this enforces.
 */
export const nativeLaneAuthenticationFromAvailability = (
  availability: Readonly<{ state: "available" | "unavailable" }>,
): Extract<ProviderLaneAuthentication, "ready" | "missing"> =>
  availability.state === "available" ? "ready" : "missing"

export type ProviderLaneRegistryEntry = Readonly<{
  laneRef: string
  provider: string
  profileRef: string
  configuration: "configured" | "unconfigured"
  authentication: ProviderLaneAuthentication
  admission: "admitted" | "quarantined"
  reason: string | null
  capabilities: ProviderLaneComposerProjection
}>

export type ProviderLaneSwitchRefusal =
  | "unknown_lane"
  | "thread_not_found"
  | "missing_auth"
  | "unadmitted_peer"
  | "capability_mismatch"

export type ProviderLaneSwitchResult =
  | Readonly<{
      ok: true
      threadRef: string
      laneRef: string
      previousLaneRef: string
      history: ReadonlyArray<ProviderLaneHistoryMessage>
      truncated: boolean
    }>
  | Readonly<{
      ok: false
      reason: ProviderLaneSwitchRefusal
      message: string
      missingCapabilities: ReadonlyArray<ProviderLaneFeatureKey>
    }>

export type ProviderLaneSelectionRecord = Readonly<{
  threadRef: string
  laneRef: string
  updatedAt: string
}>

export type ProviderLaneRegistryProjection = Readonly<{
  lanes: ReadonlyArray<ProviderLaneRegistryEntry>
  selections: ReadonlyArray<ProviderLaneSelectionRecord>
}>

export const decodeProviderLaneRegistryProjection = (value: unknown): ProviderLaneRegistryProjection | null => {
  if (typeof value !== "object" || value === null) return null
  const candidate = value as { lanes?: unknown; selections?: unknown }
  if (!Array.isArray(candidate.lanes) || !Array.isArray(candidate.selections) ||
    candidate.lanes.length > 32 || candidate.selections.length > MAX_SELECTIONS) return null
  for (const raw of candidate.lanes) {
    if (typeof raw !== "object" || raw === null) return null
    const lane = raw as Partial<ProviderLaneRegistryEntry>
    if (typeof lane.laneRef !== "string" || typeof lane.provider !== "string" ||
      typeof lane.profileRef !== "string" ||
      !["configured", "unconfigured"].includes(String(lane.configuration)) ||
      !["ready", "missing", "unknown"].includes(String(lane.authentication)) ||
      !["admitted", "quarantined"].includes(String(lane.admission)) ||
      !(lane.reason === null || typeof lane.reason === "string") ||
      typeof lane.capabilities !== "object" || lane.capabilities === null) return null
  }
  for (const raw of candidate.selections) {
    if (typeof raw !== "object" || raw === null) return null
    const selection = raw as Partial<ProviderLaneSelectionRecord>
    if (typeof selection.threadRef !== "string" || typeof selection.laneRef !== "string" ||
      typeof selection.updatedAt !== "string") return null
  }
  return candidate as ProviderLaneRegistryProjection
}

const MAX_SELECTIONS = 1_000
export const PROVIDER_SWITCH_HISTORY_MESSAGES = 32
export const PROVIDER_SWITCH_HISTORY_CHARS = 64_000

const readRecords = (file: string): ReadonlyArray<ProviderLaneSelectionRecord> => {
  try {
    const parsed = JSON.parse(readFileSync(file, "utf8")) as { version?: unknown; selections?: unknown }
    if (parsed.version !== 1 || !Array.isArray(parsed.selections)) return []
    return parsed.selections.flatMap(value => {
      if (typeof value !== "object" || value === null) return []
      const row = value as Partial<ProviderLaneSelectionRecord>
      return typeof row.threadRef === "string" && row.threadRef.length > 0 && row.threadRef.length <= 120 &&
        typeof row.laneRef === "string" && row.laneRef.length > 0 && row.laneRef.length <= 120 &&
        typeof row.updatedAt === "string"
        ? [{ threadRef: row.threadRef, laneRef: row.laneRef, updatedAt: row.updatedAt }]
        : []
    }).slice(0, MAX_SELECTIONS)
  } catch {
    return []
  }
}

const writeRecords = (file: string, records: ReadonlyArray<ProviderLaneSelectionRecord>): void => {
  mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(path.dirname(file), 0o700)
  const temporary = `${file}.tmp`
  writeFileSync(temporary, `${JSON.stringify({ version: 1, selections: records.slice(0, MAX_SELECTIONS) })}\n`, {
    encoding: "utf8",
    mode: 0o600,
  })
  if (process.platform !== "win32") chmodSync(temporary, 0o600)
  renameSync(temporary, file)
  if (process.platform !== "win32") chmodSync(file, 0o600)
}

const projectedFeature = (
  projection: ProviderLaneComposerProjection,
  feature: ProviderLaneFeatureKey,
): boolean => {
  switch (feature) {
    case "skills": return projection.skills
    case "planOnly": return projection.permissionModes.includes("plan_only")
    case "reasoningEffort": return projection.reasoningEfforts.length > 0
    case "images": return projection.images
    case "fullAuto": return projection.fullAuto
    case "interrupt": return projection.interrupt
    case "queueFollowup": return projection.queueFollowup
    case "steerTurn": return projection.steerTurn
    case "answerQuestion": return projection.questions
    // L2's public composer projection intentionally has no child-control
    // affordance yet, so registry compatibility must fail closed.
    case "steerChild": return false
  }
}

/** FA-HO-01 (#8975): exported so the host-owned ProviderHandoffEnvelope
 * builder (full-auto-provider-handoff.ts) reuses the exact same bounded
 * projection as the existing interactive manual-switch path -- one bound
 * history projector, never two that could drift. */
export const projectBoundedHistory = (thread: DesktopThread): Readonly<{
  history: ReadonlyArray<ProviderLaneHistoryMessage>
  truncated: boolean
}> => {
  const candidates = thread.notes
    .filter(note => note.text.trim() !== "")
    .map(note => ({ role: note.role, text: note.text }))
  const bounded: Array<ProviderLaneHistoryMessage> = []
  let chars = 0
  for (const message of candidates.slice(-PROVIDER_SWITCH_HISTORY_MESSAGES).reverse()) {
    const room = PROVIDER_SWITCH_HISTORY_CHARS - chars
    if (room <= 0) break
    const text = message.text.slice(Math.max(0, message.text.length - room))
    bounded.unshift({ role: message.role, text })
    chars += text.length
  }
  return {
    history: bounded,
    truncated: bounded.length < candidates.length || chars < candidates.slice(-bounded.length)
      .reduce((total, message) => total + message.text.length, 0),
  }
}

export const decodeProviderLaneSelectRequest = (value: unknown): Readonly<{
  threadRef: string
  laneRef: string
}> | null => {
  if (typeof value !== "object" || value === null) return null
  const row = value as { threadRef?: unknown; laneRef?: unknown }
  return typeof row.threadRef === "string" && row.threadRef.length > 0 && row.threadRef.length <= 120 &&
    typeof row.laneRef === "string" && row.laneRef.length > 0 && row.laneRef.length <= 120
    ? { threadRef: row.threadRef, laneRef: row.laneRef }
    : null
}

export const makeProviderLaneRegistry = (input: Readonly<{
  file: string
  defaultLaneRef?: string
  now?: () => Date
}>) => {
  const defaultLaneRef = input.defaultLaneRef ?? "codex-local"
  const now = input.now ?? (() => new Date())
  const selection = (threadRef: string): string =>
    readRecords(input.file).find(record => record.threadRef === threadRef)?.laneRef ?? defaultLaneRef
  const bind = (threadRef: string, laneRef: string): ProviderLaneSelectionRecord => {
    const record = { threadRef, laneRef, updatedAt: now().toISOString() }
    writeRecords(input.file, [record, ...readRecords(input.file).filter(row => row.threadRef !== threadRef)])
    return record
  }
  return {
    listSelections: (): ReadonlyArray<ProviderLaneSelectionRecord> => readRecords(input.file),
    selection,
    bind,
    switchThread: (request: Readonly<{
      threadRef: string
      laneRef: string
      lanes: ReadonlyArray<ProviderLaneRegistryEntry>
      thread: DesktopThread | null
      requiredCapabilities?: ReadonlyArray<ProviderLaneFeatureKey>
    }>): ProviderLaneSwitchResult => {
      const lane = request.lanes.find(candidate => candidate.laneRef === request.laneRef)
      if (lane === undefined) return {
        ok: false, reason: "unknown_lane", message: "That provider lane is not registered.", missingCapabilities: [],
      }
      if (lane.admission !== "admitted") return {
        ok: false, reason: "unadmitted_peer", message: lane.reason ?? "That provider peer is not admitted.", missingCapabilities: [],
      }
      if (lane.authentication !== "ready") return {
        ok: false, reason: "missing_auth",
        message: `${lane.capabilities.displayName} has no verified authentication.`,
        missingCapabilities: [],
      }
      if (request.thread === null) return {
        ok: false, reason: "thread_not_found", message: "That thread does not exist.", missingCapabilities: [],
      }
      const required = request.requiredCapabilities ?? []
      const missing = required.filter(feature => !projectedFeature(lane.capabilities, feature))
      if (missing.length > 0) return {
        ok: false,
        reason: "capability_mismatch",
        message: `That lane cannot carry this thread (${missing.join(", ")}).`,
        missingCapabilities: missing,
      }
      const previousLaneRef = selection(request.threadRef)
      const projected = projectBoundedHistory(request.thread)
      bind(request.threadRef, request.laneRef)
      return {
        ok: true,
        threadRef: request.threadRef,
        laneRef: request.laneRef,
        previousLaneRef,
        history: projected.history,
        truncated: projected.truncated,
      }
    },
  }
}

export type ProviderLaneRegistry = ReturnType<typeof makeProviderLaneRegistry>

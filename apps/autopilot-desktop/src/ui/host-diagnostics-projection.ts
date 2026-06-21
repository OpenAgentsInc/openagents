// VCODE-14 (#5931): public-safe host diagnostics for Verse code mode.

import type { CodeModeSyncSnapshot } from "./code-mode-sync.js"
import type { VerseSceneDiagnostic } from "./verse-scene-diagnostics.js"
import type { NodeStateMessage } from "../shared/rpc.js"

export type HostDiagnosticStatus = "ok" | "info" | "warning" | "blocked"

export type HostDiagnosticSection =
  | "node"
  | "bridge"
  | "accounts"
  | "stream"
  | "transcript"
  | "scene"
  | "input"

export type HostDiagnosticRow = Readonly<{
  key: string
  section: HostDiagnosticSection
  status: HostDiagnosticStatus
  title: string
  summary: string
  detail: string
  sourceRefs: readonly string[]
}>

export type HostDiagnosticsCounters = Readonly<{
  sessions: number
  runningSessions: number
  accounts: number
  readyAccounts: number
  streamEvents: number
  persistedTranscriptSessions: number
  sceneEvents: number
  sceneRemounts: number
  blackFrameEvents: number
  cameraControlEvents: number
  localPoseEvents: number
}>

export type HostDiagnosticsExport = Readonly<{
  schema: "openagents.autopilot_desktop.host_diagnostics.v1"
  generatedAt: string
  counters: HostDiagnosticsCounters
  rows: readonly HostDiagnosticRow[]
  recentSceneEvents: readonly VerseSceneDiagnostic[]
}>

export type HostDiagnosticsPanel = Readonly<{
  rows: readonly HostDiagnosticRow[]
  counters: HostDiagnosticsCounters
  exportData: HostDiagnosticsExport
}>

export type HostDiagnosticsInput = Readonly<{
  nodeLaunchStatus: string | null
  node: NodeStateMessage | null
  sync: CodeModeSyncSnapshot | null
  sceneDiagnostics: readonly VerseSceneDiagnostic[]
  generatedAt?: string
}>

const shortRef = (value: string): string => {
  const text = value.trim()
  if (text === "") return ""
  if (text.startsWith("account.")) return `account#${text.slice(-8)}`
  if (text.startsWith("session.")) return `session:${text.slice(-8)}`
  if (text.startsWith("workspace.")) return `workspace:${text.slice(-10)}`
  if (text.length > 40) return `${text.slice(0, 18)}...${text.slice(-8)}`
  return text
}

const sanitizeText = (value: string): string =>
  value
    .replace(/\/Users\/[^\s"']+/g, "[local-path]")
    .replace(/[A-Za-z]:\\[^\s"']+/g, "[local-path]")
    .replace(/\b(account\.[A-Za-z0-9._-]{12,})\b/g, (match) => shortRef(match))
    .replace(/\b(session\.[A-Za-z0-9._-]{12,})\b/g, (match) => shortRef(match))
    .replace(/\b(sk-[A-Za-z0-9_-]{12,})\b/g, "[secret-ref]")
    .replace(/\b(xox[pbar]-[A-Za-z0-9-]{12,})\b/g, "[secret-ref]")

const sanitizeValue = (value: unknown): unknown => {
  if (typeof value === "string") return sanitizeText(value)
  if (typeof value === "number" || typeof value === "boolean" || value === null) return value
  if (Array.isArray(value)) return value.slice(0, 12).map(sanitizeValue)
  if (typeof value === "object" && value !== null) {
    const entries = Object.entries(value as Record<string, unknown>).slice(0, 16)
    return Object.fromEntries(entries.map(([key, v]) => [key, sanitizeValue(v)]))
  }
  return String(value)
}

const sanitizeSceneEvents = (
  rows: readonly VerseSceneDiagnostic[],
): readonly VerseSceneDiagnostic[] =>
  rows.slice(-24).map((row) => ({
    at: row.at,
    event: sanitizeText(row.event),
    detail: sanitizeValue(row.detail) as Record<string, unknown>,
  }))

const sourceRefs = (values: readonly (string | null | undefined)[]): readonly string[] =>
  [...new Set(values.flatMap((value) => {
    const text = value?.trim() ?? ""
    return text === "" ? [] : [shortRef(sanitizeText(text))]
  }))]

const row = (input: HostDiagnosticRow): HostDiagnosticRow => ({
  ...input,
  summary: sanitizeText(input.summary),
  detail: sanitizeText(input.detail),
  sourceRefs: sourceRefs(input.sourceRefs),
})

const nodeStatus = (input: HostDiagnosticsInput): HostDiagnosticRow => {
  const node = input.sync?.node ?? input.node
  if (node === null) {
    return row({
      key: "node.waiting",
      section: "node",
      status: "warning",
      title: "Pylon node",
      summary: "Waiting for node-state",
      detail: "No public node-state projection has reached the desktop yet.",
      sourceRefs: [],
    })
  }
  return row({
    key: node.ok ? "node.ok" : "node.blocked",
    section: "node",
    status: node.ok ? "ok" : "blocked",
    title: "Pylon node",
    summary: node.ok ? "Node-state projection is online" : "Node-state projection is blocked",
    detail: `schema ${node.schema}; ${node.sessions.length} sessions projected`,
    sourceRefs: [node.schema],
  })
}

const bridgeStatus = (input: HostDiagnosticsInput): HostDiagnosticRow => {
  const status = input.nodeLaunchStatus ?? "unknown"
  const ok = status === "online" || status === "adopted"
  const blocked = status === "failed" || status === "unavailable"
  return row({
    key: `bridge.${status}`,
    section: "bridge",
    status: ok ? "ok" : blocked ? "blocked" : "warning",
    title: "Bun bridge",
    summary: ok ? `Bridge is ${status}` : `Bridge status: ${status}`,
    detail:
      status === "unknown"
        ? "The desktop has not received a node launch lifecycle update yet."
        : `Latest node launch lifecycle status is ${status}.`,
    sourceRefs: [`bridge:${status}`],
  })
}

const accountRows = (input: HostDiagnosticsInput): readonly HostDiagnosticRow[] => {
  const accounts = input.sync?.accounts ?? (input.node?.accounts ?? []).map((account) => ({
    provider: account.provider,
    accountRef: account.accountRef,
    accountRefHash: account.accountRefHash,
    ready: account.ready,
    label: account.accountRef ?? `${account.provider} default`,
    blockerRefs: account.blockerRefs,
    priority: account.priority,
  }))
  const codex = accounts.filter((account) => account.provider === "codex")
  if (codex.length === 0) {
    return [
      row({
        key: "accounts.codex.none",
        section: "accounts",
        status: "warning",
        title: "Codex accounts",
        summary: "No Codex accounts projected",
        detail: "Open Accounts to add a managed Codex home or wait for live readiness.",
        sourceRefs: [],
      }),
    ]
  }
  return codex.map((account) =>
    row({
      key: `accounts.codex.${account.accountRef ?? shortRef(account.accountRefHash ?? "default")}`,
      section: "accounts",
      status: account.ready ? "ok" : "blocked",
      title: `Codex ${account.accountRef ?? "default"}`,
      summary: account.ready ? "Ready for routing" : "Not ready for routing",
      detail: account.ready
        ? `priority ${account.priority ?? "auto"}`
        : "Open Accounts for login/home repair details.",
      sourceRefs: account.accountRefHash === null ? [] : [account.accountRefHash],
    }),
  )
}

const streamStatus = (
  sync: CodeModeSyncSnapshot | null,
): HostDiagnosticRow => {
  const sessions = sync?.sessions ?? []
  const eventCount = sync?.counts.events ?? 0
  const running = sync?.counts.runningSessions ?? 0
  return row({
    key: "stream.poll",
    section: "stream",
    status: eventCount > 0 || running === 0 ? "ok" : "warning",
    title: "Stream / poll",
    summary:
      eventCount > 0
        ? `${eventCount} event rows projected`
        : running > 0
          ? "Running sessions are waiting for event rows"
          : "No active stream rows",
    detail: `sync source ${sync?.source ?? "none"}; ${sessions.length} sessions; polling remains the repair fallback.`,
    sourceRefs: sync?.syncRef === undefined ? [] : [sync.syncRef],
  })
}

const transcriptStatus = (
  sync: CodeModeSyncSnapshot | null,
): HostDiagnosticRow => {
  const sessionsWithEvents =
    sync?.sessionRows.filter((entry) => entry.events.length > 0).length ?? 0
  return row({
    key: "transcript.persistence",
    section: "transcript",
    status: "ok",
    title: "Transcript persistence",
    summary: `${sessionsWithEvents} sessions have event tails`,
    detail:
      "Desktop transcript persistence merges poll tails back into node-state; exports include counts and refs only.",
    sourceRefs: ["desktop-transcripts"],
  })
}

const sceneRows = (
  input: HostDiagnosticsInput,
): readonly HostDiagnosticRow[] => {
  const logs = input.sceneDiagnostics
  const remounts = logs.filter((entry) => entry.event.startsWith("verse-host.remount."))
  const blackFrames = logs.filter((entry) => entry.event.includes("black-frame"))
  const keyChanges = logs.filter((entry) => entry.event === "visualization.key_changed")
  const camera = logs.filter((entry) => entry.event === "verse-host.camera-control")
  const localPose = logs.filter((entry) => entry.event === "local-pose.cached")
  return [
    row({
      key: "scene.stability",
      section: "scene",
      status: remounts.length > 0 || blackFrames.length > 0 ? "blocked" : keyChanges.length > 2 ? "warning" : "ok",
      title: "Scene stability",
      summary: `${remounts.length} remounts, ${blackFrames.length} black-frame events`,
      detail: `${keyChanges.length} visualization key changes in the retained diagnostic buffer.`,
      sourceRefs: remounts.map((entry) => entry.event),
    }),
    row({
      key: "input.mouselook",
      section: "input",
      status: camera.length > 0 || localPose.length > 0 ? "ok" : "info",
      title: "Scene input",
      summary: `${camera.length} camera-control events, ${localPose.length} local pose samples`,
      detail:
        camera.length > 0
          ? "Mouselook / wheel controls have emitted camera diagnostics."
          : "No camera-control events observed in this buffer yet.",
      sourceRefs: camera.map((entry) => entry.event),
    }),
  ]
}

export const projectHostDiagnosticsPanel = (
  input: HostDiagnosticsInput,
): HostDiagnosticsPanel => {
  const sync = input.sync
  const scene = input.sceneDiagnostics
  const rows = [
    nodeStatus(input),
    bridgeStatus(input),
    ...accountRows(input),
    streamStatus(sync),
    transcriptStatus(sync),
    ...sceneRows(input),
  ]
  const counters = {
    sessions: sync?.counts.sessions ?? input.node?.sessions.length ?? 0,
    runningSessions: sync?.counts.runningSessions ?? 0,
    accounts: sync?.counts.accounts ?? input.node?.accounts?.length ?? 0,
    readyAccounts: sync?.counts.readyAccounts ?? input.node?.accounts?.filter((row) => row.ready).length ?? 0,
    streamEvents: sync?.counts.events ?? 0,
    persistedTranscriptSessions: sync?.sessionRows.filter((entry) => entry.events.length > 0).length ?? 0,
    sceneEvents: scene.length,
    sceneRemounts: scene.filter((entry) => entry.event.startsWith("verse-host.remount.")).length,
    blackFrameEvents: scene.filter((entry) => entry.event.includes("black-frame")).length,
    cameraControlEvents: scene.filter((entry) => entry.event === "verse-host.camera-control").length,
    localPoseEvents: scene.filter((entry) => entry.event === "local-pose.cached").length,
  }
  return {
    rows,
    counters,
    exportData: {
      schema: "openagents.autopilot_desktop.host_diagnostics.v1",
      generatedAt: input.generatedAt ?? "runtime",
      counters,
      rows,
      recentSceneEvents: sanitizeSceneEvents(scene),
    },
  }
}

export type KhalaCodeTerminalTabStatus = "running" | "exited" | "unknown"

export type KhalaCodeTerminalTab = Readonly<{
  command: string
  cwd: string | null
  outputPreview: string
  processId: string
  status: KhalaCodeTerminalTabStatus
  title: string
}>

export type KhalaCodeTerminalWorkbenchProjection = Readonly<{
  activeProcessId: string | null
  activeThreadId: string | null
  boundary: "active_thread" | "no_active_thread"
  tabs: readonly KhalaCodeTerminalTab[]
  transport: "codex_background_terminal"
}>

const asRecord = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

const stringValue = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null

const terminalItems = (response: unknown): readonly unknown[] => {
  if (Array.isArray(response)) return response
  const record = asRecord(response)
  if (Array.isArray(record.terminals)) return record.terminals
  if (Array.isArray(record.items)) return record.items
  if (Array.isArray(record.data)) return record.data
  return []
}

const statusFor = (value: unknown): KhalaCodeTerminalTabStatus => {
  const status = stringValue(value)?.toLowerCase()
  if (status === "running" || status === "active") return "running"
  if (status === "exited" || status === "closed" || status === "completed") return "exited"
  return "unknown"
}

const tabFor = (value: unknown): KhalaCodeTerminalTab | null => {
  const record = asRecord(value)
  const processId = stringValue(record.processId) ?? stringValue(record.id)
  if (processId === null) return null
  const command = stringValue(record.command) ?? stringValue(record.cmd) ?? ""
  const title = stringValue(record.title) ?? (command.length > 0 ? command : `Process ${processId}`)
  const outputPreview =
    stringValue(record.outputPreview) ??
    stringValue(record.output) ??
    stringValue(record.preview) ??
    ""
  return {
    command,
    cwd: stringValue(record.cwd),
    outputPreview,
    processId,
    status: statusFor(record.status),
    title,
  }
}

export const projectKhalaCodeTerminalWorkbench = (
  input: Readonly<{
    activeProcessId?: string | null
    activeThreadId?: string | null
    response?: unknown
  }>,
): KhalaCodeTerminalWorkbenchProjection => {
  const activeThreadId = input.activeThreadId ?? null
  if (activeThreadId === null) {
    return {
      activeProcessId: null,
      activeThreadId: null,
      boundary: "no_active_thread",
      tabs: [],
      transport: "codex_background_terminal",
    }
  }
  const tabs = terminalItems(asRecord(input.response).response ?? input.response)
    .flatMap(item => {
      const tab = tabFor(item)
      return tab === null ? [] : [tab]
    })
  const activeProcessId =
    input.activeProcessId !== undefined && tabs.some(tab => tab.processId === input.activeProcessId)
      ? input.activeProcessId
      : tabs[0]?.processId ?? null
  return {
    activeProcessId,
    activeThreadId,
    boundary: activeThreadId === null ? "no_active_thread" : "active_thread",
    tabs,
    transport: "codex_background_terminal",
  }
}

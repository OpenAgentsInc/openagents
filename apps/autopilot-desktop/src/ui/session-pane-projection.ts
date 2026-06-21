// VCODE-08 (#5925): pure session-list/detail projection for Verse code-mode panes.
//
// The UI filters sessions by bounded public fields only: status, adapter,
// account hash, and workspace ref. Labels stay compact by default; full account
// hashes are reserved for the selected-session detail pane.

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type { AccountRow } from "../shared/rpc.js"
import type { SessionAdapterFilter, SessionFilter } from "./model.js"

export const SESSION_FILTER_ALL = "all"
export const SESSION_ACCOUNT_DEFAULT = "__default_account__"
export const SESSION_WORKSPACE_NONE = "__workspace_none__"

export type SessionPaneFilters = Readonly<{
  status: SessionFilter
  adapter: SessionAdapterFilter
  account: string
  workspace: string
}>

export type SessionFilterOption = Readonly<{
  value: string
  label: string
  count: number
  title?: string
}>

export type SessionPaneProjection = Readonly<{
  sessions: ReadonlyArray<SessionSummary>
  statusOptions: ReadonlyArray<SessionFilterOption>
  adapterOptions: ReadonlyArray<SessionFilterOption>
  accountOptions: ReadonlyArray<SessionFilterOption>
  workspaceOptions: ReadonlyArray<SessionFilterOption>
}>

const increment = (map: Map<string, number>, key: string): void => {
  map.set(key, (map.get(key) ?? 0) + 1)
}

export const sessionAccountFilterValue = (session: {
  accountRefHash: string | null
}): string =>
  session.accountRefHash === null || session.accountRefHash.trim() === ""
    ? SESSION_ACCOUNT_DEFAULT
    : session.accountRefHash

export const sessionWorkspaceFilterValue = (session: {
  workspaceRef?: string | null | undefined
}): string => {
  const ref = session.workspaceRef ?? null
  return ref === null || ref.trim() === "" ? SESSION_WORKSPACE_NONE : ref
}

const shortHash = (hash: string): string =>
  hash.length <= 8 ? hash : `#${hash.slice(-8)}`

export const sessionAccountShortLabel = (
  session: Pick<SessionSummary, "adapter" | "accountRefHash">,
  accounts: ReadonlyArray<AccountRow> = [],
): string => {
  if (session.accountRefHash === null || session.accountRefHash.trim() === "") {
    return `${session.adapter} default`
  }
  const match = accounts.find((row) => row.accountRefHash === session.accountRefHash)
  if (match) {
    const suffix =
      match.selector === "default_home"
        ? "default"
        : match.accountRef?.trim() || shortHash(match.accountRefHash)
    return `${match.provider} ${suffix}`
  }
  return `${session.adapter} ${shortHash(session.accountRefHash)}`
}

export const sessionWorkspaceShortLabel = (workspaceValue: string): string => {
  if (workspaceValue === SESSION_WORKSPACE_NONE) return "no workspace"
  const parts = workspaceValue.split(/[/.]/).filter((part) => part.length > 0)
  const tail = parts.at(-1) ?? workspaceValue
  return tail.length > 18 ? `...${tail.slice(-16)}` : tail
}

const option = (
  value: string,
  label: string,
  count: number,
  title?: string,
): SessionFilterOption => ({ value, label, count, ...(title ? { title } : {}) })

const adapterLabel = (adapter: string): string =>
  adapter === "claude_agent" ? "Claude" : adapter === "apple_fm" ? "Apple FM" : "Codex"

const stateLabel = (state: string): string =>
  state === "all" ? "All" : state

const sessionMatches = (
  session: SessionSummary,
  filters: SessionPaneFilters,
): boolean =>
  (filters.status === "all" || session.state === filters.status) &&
  (filters.adapter === "all" || session.adapter === filters.adapter) &&
  (filters.account === SESSION_FILTER_ALL ||
    sessionAccountFilterValue(session) === filters.account) &&
  (filters.workspace === SESSION_FILTER_ALL ||
    sessionWorkspaceFilterValue(session) === filters.workspace)

export const projectSessionPane = (input: {
  sessions: ReadonlyArray<SessionSummary>
  accounts?: ReadonlyArray<AccountRow>
  filters: SessionPaneFilters
}): SessionPaneProjection => {
  const accounts = input.accounts ?? []
  const statusCounts = new Map<string, number>([[SESSION_FILTER_ALL, input.sessions.length]])
  const adapterCounts = new Map<string, number>([[SESSION_FILTER_ALL, input.sessions.length]])
  const accountCounts = new Map<string, number>([[SESSION_FILTER_ALL, input.sessions.length]])
  const workspaceCounts = new Map<string, number>([[SESSION_FILTER_ALL, input.sessions.length]])

  for (const session of input.sessions) {
    increment(statusCounts, session.state)
    increment(adapterCounts, session.adapter)
    increment(accountCounts, sessionAccountFilterValue(session))
    increment(workspaceCounts, sessionWorkspaceFilterValue(session))
  }

  const statusOrder: ReadonlyArray<SessionFilter> = [
    "all",
    "running",
    "queued",
    "completed",
    "failed",
    "cancelled",
  ]
  const adapterOrder: ReadonlyArray<SessionAdapterFilter> = [
    "all",
    "codex",
    "claude_agent",
    "apple_fm",
  ]

  const statusOptions = statusOrder.map((value) =>
    option(value, stateLabel(value), statusCounts.get(value) ?? 0),
  )
  const adapterOptions = adapterOrder.map((value) =>
    option(
      value,
      value === SESSION_FILTER_ALL ? "All" : adapterLabel(value),
      adapterCounts.get(value) ?? 0,
    ),
  )

  const accountValues = [...accountCounts.keys()]
    .filter((value) => value !== SESSION_FILTER_ALL)
    .sort((a, b) => {
      if (a === SESSION_ACCOUNT_DEFAULT) return -1
      if (b === SESSION_ACCOUNT_DEFAULT) return 1
      return a.localeCompare(b)
    })
  const accountOptions = [
    option(SESSION_FILTER_ALL, "All accounts", input.sessions.length),
    ...accountValues.map((value) => {
      const matchingSession = input.sessions.find(
        (session) => sessionAccountFilterValue(session) === value,
      )
      const label =
        matchingSession === undefined
          ? value === SESSION_ACCOUNT_DEFAULT
            ? "default"
            : shortHash(value)
          : sessionAccountShortLabel(matchingSession, accounts)
      return option(value, label, accountCounts.get(value) ?? 0)
    }),
  ]

  const workspaceValues = [...workspaceCounts.keys()]
    .filter((value) => value !== SESSION_FILTER_ALL)
    .sort((a, b) => {
      if (a === SESSION_WORKSPACE_NONE) return -1
      if (b === SESSION_WORKSPACE_NONE) return 1
      return sessionWorkspaceShortLabel(a).localeCompare(sessionWorkspaceShortLabel(b))
    })
  const workspaceOptions = [
    option(SESSION_FILTER_ALL, "All workspaces", input.sessions.length),
    ...workspaceValues.map((value) =>
      option(
        value,
        sessionWorkspaceShortLabel(value),
        workspaceCounts.get(value) ?? 0,
        value === SESSION_WORKSPACE_NONE ? undefined : value,
      ),
    ),
  ]

  return {
    sessions: input.sessions.filter((session) => sessionMatches(session, input.filters)),
    statusOptions,
    adapterOptions,
    accountOptions,
    workspaceOptions,
  }
}

// VCODE-12 (#5929): one typed sync projection for Verse code mode.
//
// The desktop already receives all raw ingredients through existing safe
// projections: node state, managed dev.accounts, session event tails,
// transcript/artifact summaries, and readiness/quota blobs. This module folds
// them into one de-duped, public-safe snapshot so code-mode panes converge on
// the same tick instead of each guessing independently.

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"
import type {
  AccountRow,
  AppleFmReadinessResponse,
  ApprovalRow,
  BuiltInAgentReadinessResponse,
  InferenceGatewayReadinessResponse,
  ManagedAccountRow,
  ManagedAccountsResponse,
  NodeStateMessage,
  SessionArtifactStats,
  SessionEventRow,
} from "../shared/rpc.js"

export type CodeModeSyncSource =
  | "initial"
  | "node_state"
  | "managed_accounts"
  | "account_mutation"
  | "readiness"
  | "model_tick"

export type CodeModeSyncDiagnosticSeverity = "info" | "warning" | "error"

export type CodeModeSyncDiagnostic = Readonly<{
  key: string
  severity: CodeModeSyncDiagnosticSeverity
  title: string
  body: string
  sourceRef: string | null
}>

export type CodeModeSyncAccountRow = Readonly<{
  key: string
  provider: string
  accountRef: string | null
  accountRefHash: string | null
  label: string
  selector: string
  ready: boolean
  managed: boolean
  live: boolean
  homePresent: boolean | null
  priority: number | null
  blockerRefs: readonly string[]
  source: "managed_live" | "managed_only" | "live_only" | "default_home"
}>

export type CodeModeSyncSessionRow = Readonly<{
  session: SessionSummary
  events: readonly SessionEventRow[]
  stats: SessionArtifactStats | undefined
}>

export type CodeModeSyncReadiness = Readonly<{
  nodeOk: boolean
  gatewayReady: boolean
  gatewayCreditBalance: number | null
  gatewayLowBalance: boolean
  builtInReady: boolean
  builtInQuotaRemaining: number | null
  appleFmReady: boolean
}>

export type CodeModeSyncSnapshot = Readonly<{
  syncRef: string
  source: CodeModeSyncSource
  node: NodeStateMessage | null
  sessions: readonly SessionSummary[]
  sessionRows: readonly CodeModeSyncSessionRow[]
  events: Readonly<Record<string, readonly SessionEventRow[]>>
  accounts: readonly CodeModeSyncAccountRow[]
  liveAccounts: readonly AccountRow[]
  managedAccounts: readonly ManagedAccountRow[]
  approvals: readonly ApprovalRow[]
  artifacts: Readonly<Record<string, SessionArtifactStats>>
  readiness: CodeModeSyncReadiness
  diagnostics: readonly CodeModeSyncDiagnostic[]
  counts: Readonly<{
    sessions: number
    runningSessions: number
    accounts: number
    readyAccounts: number
    events: number
    approvals: number
    artifacts: number
    diagnostics: number
  }>
}>

export type CodeModeSyncInput = Readonly<{
  source: CodeModeSyncSource
  node: NodeStateMessage | null
  managedAccounts: ManagedAccountsResponse | null
  inferenceGatewayReadiness: InferenceGatewayReadinessResponse | null
  builtInAgentReadiness: BuiltInAgentReadinessResponse | null
  appleFmReadiness: AppleFmReadinessResponse | null
  selectedSessionRef: string | null
  composerAccountRef: string | null
}>

const terminalStates = new Set(["completed", "failed", "cancelled"])

const stableString = (value: unknown): string =>
  JSON.stringify(value, (_key, v) => {
    if (v === undefined) return null
    if (v instanceof Set) return [...v].sort()
    return v
  })

const isoMs = (value: string | undefined): number => {
  if (value === undefined) return 0
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

const shortHash = (value: string | null | undefined): string | null => {
  const text = value?.trim()
  return text ? `#${text.slice(-8)}` : null
}

const accountKey = (provider: string, accountRef: string | null, hash?: string | null): string => {
  if (accountRef !== null && accountRef.trim() !== "") return `${provider}:ref:${accountRef}`
  if (hash !== null && hash !== undefined && hash.trim() !== "") return `${provider}:hash:${hash}`
  return `${provider}:default`
}

const accountLabel = (
  provider: string,
  accountRef: string | null,
  hash: string | null,
): string => {
  if (accountRef !== null && accountRef.trim() !== "") return `${provider} ${accountRef}`
  if (hash !== null) return `${provider} ${shortHash(hash) ?? "account"}`
  return `${provider} default`
}

const uniqueStrings = (values: readonly string[]): readonly string[] =>
  [...new Set(values.filter((value) => value.trim() !== ""))]

const mergeAccountRows = (
  liveAccounts: readonly AccountRow[],
  managedAccounts: readonly ManagedAccountRow[],
): readonly CodeModeSyncAccountRow[] => {
  const liveByKey = new Map<string, AccountRow>()
  for (const row of liveAccounts) {
    liveByKey.set(accountKey(row.provider, row.accountRef, row.accountRefHash), row)
  }

  const rows = new Map<string, CodeModeSyncAccountRow>()
  for (const managed of managedAccounts) {
    const key = accountKey(managed.provider, managed.ref, null)
    const live = liveByKey.get(key) ?? null
    rows.set(key, {
      key,
      provider: managed.provider,
      accountRef: managed.ref,
      accountRefHash: live?.accountRefHash ?? null,
      label: accountLabel(managed.provider, managed.ref, live?.accountRefHash ?? null),
      selector: live?.selector ?? "managed",
      ready: live?.ready ?? managed.homePresent,
      managed: true,
      live: live !== null,
      homePresent: managed.homePresent,
      priority: managed.priority,
      blockerRefs: uniqueStrings(live?.blockerRefs ?? []),
      source: live === null ? "managed_only" : "managed_live",
    })
  }

  for (const live of liveAccounts) {
    const key = accountKey(live.provider, live.accountRef, live.accountRefHash)
    if (rows.has(key)) continue
    rows.set(key, {
      key,
      provider: live.provider,
      accountRef: live.accountRef,
      accountRefHash: live.accountRefHash,
      label: accountLabel(live.provider, live.accountRef, live.accountRefHash),
      selector: live.selector,
      ready: live.ready,
      managed: false,
      live: true,
      homePresent: null,
      priority: live.priority,
      blockerRefs: uniqueStrings(live.blockerRefs),
      source: live.selector === "default_home" ? "default_home" : "live_only",
    })
  }

  return [...rows.values()].sort((a, b) => {
    const priorityA = a.priority ?? Number.POSITIVE_INFINITY
    const priorityB = b.priority ?? Number.POSITIVE_INFINITY
    if (priorityA !== priorityB) return priorityA - priorityB
    return a.label.localeCompare(b.label)
  })
}

const dedupeSessions = (sessions: readonly SessionSummary[]): readonly SessionSummary[] => {
  const byRef = new Map<string, SessionSummary>()
  for (const session of sessions) {
    const previous = byRef.get(session.sessionRef)
    if (
      previous === undefined ||
      isoMs(session.updatedAt) >= isoMs(previous.updatedAt)
    ) {
      byRef.set(session.sessionRef, session)
    }
  }
  return [...byRef.values()].sort((a, b) => {
    const byTime = isoMs(b.updatedAt) - isoMs(a.updatedAt)
    return byTime !== 0 ? byTime : a.sessionRef.localeCompare(b.sessionRef)
  })
}

const dedupeEvents = (
  events: Readonly<Record<string, readonly SessionEventRow[]>> | undefined,
): Readonly<Record<string, readonly SessionEventRow[]>> => {
  const result: Record<string, readonly SessionEventRow[]> = {}
  for (const [sessionRef, rows] of Object.entries(events ?? {})) {
    const byIndex = new Map<number, SessionEventRow>()
    for (const row of rows) {
      const previous = byIndex.get(row.eventIndex)
      if (
        previous === undefined ||
        isoMs(row.observedAt) >= isoMs(previous.observedAt)
      ) {
        byIndex.set(row.eventIndex, row)
      }
    }
    result[sessionRef] = [...byIndex.values()].sort((a, b) => a.eventIndex - b.eventIndex)
  }
  return result
}

const readinessProjection = (input: CodeModeSyncInput): CodeModeSyncReadiness => {
  const gateway = input.inferenceGatewayReadiness
  const builtIn = input.builtInAgentReadiness
  const apple = input.appleFmReadiness
  const builtInQuotaRemaining =
    builtIn === null
      ? null
      : Math.max(0, builtIn.dailySessionCap - builtIn.dailySessionsUsed)
  const gatewayLowBalance =
    gateway?.creditBalance !== null &&
    gateway?.creditBalance !== undefined &&
    gateway.creditBalance <= gateway.lowBalanceThreshold
  return {
    nodeOk: input.node?.ok ?? false,
    gatewayReady: Boolean(gateway?.ok && gateway.enabled && gateway.apiKeyPresent),
    gatewayCreditBalance: gateway?.creditBalance ?? null,
    gatewayLowBalance,
    builtInReady: Boolean(builtIn?.ok),
    builtInQuotaRemaining,
    appleFmReady: Boolean(apple?.ok && apple.available),
  }
}

const diagnosticsFor = (
  input: CodeModeSyncInput,
  accounts: readonly CodeModeSyncAccountRow[],
  sessions: readonly SessionSummary[],
  readiness: CodeModeSyncReadiness,
): readonly CodeModeSyncDiagnostic[] => {
  const diagnostics: CodeModeSyncDiagnostic[] = []
  const add = (diagnostic: CodeModeSyncDiagnostic): void => {
    diagnostics.push(diagnostic)
  }

  if (input.node === null) {
    add({
      key: "node.waiting",
      severity: "info",
      title: "Node sync pending",
      body: "Waiting for the first node-state poll before marking code mode ready.",
      sourceRef: null,
    })
  } else if (!input.node.ok) {
    add({
      key: "node.blocked",
      severity: "error",
      title: "Node sync blocked",
      body: "The latest node-state projection is not OK; coding panes should stay repairable.",
      sourceRef: input.node.schema,
    })
  }

  if (input.managedAccounts?.ok === false) {
    add({
      key: "accounts.managed.load_failed",
      severity: "error",
      title: "Account registry failed",
      body: input.managedAccounts.error ?? "Managed accounts could not be loaded.",
      sourceRef: "dev.accounts",
    })
  }

  for (const account of accounts) {
    if (!account.live && account.managed) {
      add({
        key: `account.${account.provider}.${account.accountRef ?? "default"}.missing_live`,
        severity: "warning",
        title: "Account waiting for node projection",
        body: `${account.label} is in dev.accounts but has not appeared in live node readiness yet.`,
        sourceRef: account.accountRefHash,
      })
    }
    if (!account.ready) {
      add({
        key: `account.${account.provider}.${account.accountRef ?? account.accountRefHash ?? "default"}.blocked`,
        severity: "warning",
        title: "Account blocked",
        body:
          account.blockerRefs.length > 0
            ? account.blockerRefs.join(", ")
            : `${account.label} is not ready.`,
        sourceRef: account.accountRefHash,
      })
    }
  }

  const selectedAccount = input.composerAccountRef
  if (
    selectedAccount !== null &&
    !accounts.some((row) => row.provider === "codex" && row.accountRef === selectedAccount)
  ) {
    add({
      key: "composer.account.unavailable",
      severity: "warning",
      title: "Selected account unavailable",
      body: `The selected Codex account '${selectedAccount}' is not in the synchronized account model.`,
      sourceRef: null,
    })
  }

  const selectedSession = input.selectedSessionRef
  if (selectedSession !== null && !sessions.some((row) => row.sessionRef === selectedSession)) {
    add({
      key: "session.selected.unavailable",
      severity: "warning",
      title: "Selected session unavailable",
      body: "The selected session is not present in the synchronized session list.",
      sourceRef: selectedSession,
    })
  }

  const gateway = input.inferenceGatewayReadiness
  if (gateway?.ok === false) {
    add({
      key: "readiness.gateway.failed",
      severity: "warning",
      title: "Gateway readiness failed",
      body: gateway.error ?? "Inference gateway readiness could not be refreshed.",
      sourceRef: gateway.sourceUrl,
    })
  } else if (readiness.gatewayLowBalance) {
    add({
      key: "readiness.gateway.low_balance",
      severity: "warning",
      title: "Gateway balance low",
      body: "OpenAgents inference gateway credit is at or below the low-balance threshold.",
      sourceRef: gateway?.sourceUrl ?? null,
    })
  }

  const builtIn = input.builtInAgentReadiness
  if (builtIn?.ok === false) {
    add({
      key: "readiness.builtin.failed",
      severity: "warning",
      title: "Built-in agent readiness failed",
      body:
        builtIn.error ??
        (builtIn.blockerRefs.join(", ") || "Built-in agent is blocked."),
      sourceRef: builtIn.sourceUrl,
    })
  } else if (readiness.builtInQuotaRemaining === 0) {
    add({
      key: "readiness.builtin.quota",
      severity: "warning",
      title: "Built-in agent quota exhausted",
      body: "The hosted built-in agent has no remaining daily sessions.",
      sourceRef: builtIn?.sourceUrl ?? null,
    })
  }

  if (input.appleFmReadiness?.ok === false) {
    add({
      key: "readiness.apple_fm.failed",
      severity: "info",
      title: "Apple FM unavailable",
      body:
        input.appleFmReadiness.error ??
        input.appleFmReadiness.message ??
        input.appleFmReadiness.unavailableReason ??
        "Local Apple FM is not available.",
      sourceRef: input.appleFmReadiness.sourceUrl,
    })
  }

  return diagnostics
}

export const projectCodeModeSyncSnapshot = (
  input: CodeModeSyncInput,
): CodeModeSyncSnapshot => {
  const sessions = dedupeSessions(input.node?.sessions ?? [])
  const events = dedupeEvents(input.node?.events)
  const liveAccounts = mergeLiveAccounts(input.node?.accounts ?? [])
  const managedAccounts = [...(input.managedAccounts?.accounts ?? [])].sort((a, b) => {
    const priorityA = a.priority ?? Number.POSITIVE_INFINITY
    const priorityB = b.priority ?? Number.POSITIVE_INFINITY
    if (priorityA !== priorityB) return priorityA - priorityB
    return `${a.provider}:${a.ref}`.localeCompare(`${b.provider}:${b.ref}`)
  })
  const accounts = mergeAccountRows(liveAccounts, managedAccounts)
  const artifacts = input.node?.artifacts ?? {}
  const approvals = input.node?.approvals ?? []
  const readiness = readinessProjection(input)
  const sessionRows = sessions.map((session) => ({
    session,
    events: events[session.sessionRef] ?? [],
    stats: artifacts[session.sessionRef],
  }))
  const diagnostics = diagnosticsFor(input, accounts, sessions, readiness)
  const eventCount = Object.values(events).reduce((sum, rows) => sum + rows.length, 0)
  const artifactCount = Object.keys(artifacts).length
  const runningSessions = sessions.filter((session) => !terminalStates.has(session.state)).length
  const syncRef = `code-sync:${hashStable(
    stableString({
      source: input.source,
      sessions: sessions.map((row) => [row.sessionRef, row.state, row.updatedAt]),
      events: Object.entries(events).map(([ref, rows]) => [ref, rows.map((row) => row.eventIndex)]),
      accounts: accounts.map((row) => [row.key, row.ready, row.source]),
      approvals: approvals.map((row) => row.approvalRef),
      artifacts: Object.keys(artifacts).sort(),
      diagnostics: diagnostics.map((row) => row.key),
      readiness,
    }),
  )}`

  return {
    syncRef,
    source: input.source,
    node: input.node,
    sessions,
    sessionRows,
    events,
    accounts,
    liveAccounts,
    managedAccounts,
    approvals,
    artifacts,
    readiness,
    diagnostics,
    counts: {
      sessions: sessions.length,
      runningSessions,
      accounts: accounts.length,
      readyAccounts: accounts.filter((row) => row.ready).length,
      events: eventCount,
      approvals: approvals.length,
      artifacts: artifactCount,
      diagnostics: diagnostics.length,
    },
  }
}

const mergeLiveAccounts = (accounts: readonly AccountRow[]): readonly AccountRow[] => {
  const byKey = new Map<string, AccountRow>()
  for (const account of accounts) {
    const key = accountKey(account.provider, account.accountRef, account.accountRefHash)
    const previous = byKey.get(key)
    if (
      previous === undefined ||
      (previous.ready === false && account.ready) ||
      ((previous.priority ?? Number.POSITIVE_INFINITY) >
        (account.priority ?? Number.POSITIVE_INFINITY))
    ) {
      byKey.set(key, account)
    }
  }
  return [...byKey.values()].sort((a, b) => {
    const priorityA = a.priority ?? Number.POSITIVE_INFINITY
    const priorityB = b.priority ?? Number.POSITIVE_INFINITY
    if (priorityA !== priorityB) return priorityA - priorityB
    return accountLabel(a.provider, a.accountRef, a.accountRefHash).localeCompare(
      accountLabel(b.provider, b.accountRef, b.accountRefHash),
    )
  })
}

const hashStable = (value: string): string => {
  let hash = 2166136261
  for (let i = 0; i < value.length; i += 1) {
    hash ^= value.charCodeAt(i)
    hash = Math.imul(hash, 16777619)
  }
  return (hash >>> 0).toString(16).padStart(8, "0")
}

// VCODE-13 (#5930): deterministic account routing for Verse code mode.
//
// The runtime command still only needs an accountRef, but the UI needs a public
// route explanation before spawn. This projection keeps the precedence shared
// by reducer + view + tests and keeps route evidence redacted by default.

import type { SessionSummary } from "@openagentsinc/autopilot-control-protocol"

import type { CodeModeSyncAccountRow } from "./code-mode-sync.js"

export type CodeModeSpawnAdapter = "codex" | "claude_agent"

export type CodeModeRouteSource =
  | "explicit"
  | "last_used"
  | "priority"
  | "default_home"
  | "blocked"

export type CodeModeRouteEvidence = Readonly<{
  adapter: CodeModeSpawnAdapter
  route: CodeModeRouteSource
  accountLabel: string | null
  accountHash: string | null
  workspaceRef: string | null
  sourceRef: string | null
}>

export type CodeModeAccountRoute = Readonly<{
  adapter: CodeModeSpawnAdapter
  source: CodeModeRouteSource
  accountRef: string | null
  accountRefHash: string | null
  label: string
  detail: string
  blocker: string | null
  evidence: CodeModeRouteEvidence
}>

export type CodeModeAccountRouteInput = Readonly<{
  adapter: CodeModeSpawnAdapter
  selectedAccountRef: string | null
  accounts: readonly CodeModeSyncAccountRow[]
  sessions: readonly SessionSummary[]
  workspaceRef: string | null
  allowDefaultHome: boolean
}>

export type CodeModeAccountOverride = Readonly<{
  accountRef: string | null
  label: string
}>

const adapterLabel = (adapter: CodeModeSpawnAdapter): string =>
  adapter === "codex" ? "Codex" : "Claude Agent"

export const redactedAccountHash = (value: string | null | undefined): string | null => {
  const text = value?.trim() ?? ""
  return text === "" ? null : `#${text.slice(-8)}`
}

const redactedAccountLabel = (value: string | null | undefined): string | null => {
  const text = value?.trim() ?? ""
  if (text === "") return null
  if (text.length <= 24) return text
  return `${text.slice(0, 10)}…${text.slice(-6)}`
}

const isoMs = (value: string | undefined): number => {
  if (value === undefined) return 0
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : 0
}

const providerAccounts = (
  input: CodeModeAccountRouteInput,
): readonly CodeModeSyncAccountRow[] =>
  input.accounts.filter((row) => row.provider === input.adapter)

const sortRouteAccounts = (
  rows: readonly CodeModeSyncAccountRow[],
): readonly CodeModeSyncAccountRow[] =>
  [...rows].sort((a, b) => {
    const priorityA = a.priority ?? Number.POSITIVE_INFINITY
    const priorityB = b.priority ?? Number.POSITIVE_INFINITY
    if (priorityA !== priorityB) return priorityA - priorityB
    const labelCompare = a.label.localeCompare(b.label)
    return labelCompare !== 0 ? labelCompare : a.key.localeCompare(b.key)
  })

const routeFor = (
  input: CodeModeAccountRouteInput,
  source: Exclude<CodeModeRouteSource, "blocked">,
  account: CodeModeSyncAccountRow | null,
  detail: string,
): CodeModeAccountRoute => {
  const label =
    account?.accountRef !== null && account?.accountRef !== undefined
      ? `${adapterLabel(input.adapter)} ${account.accountRef}`
      : `${adapterLabel(input.adapter)} default home`
  return {
    adapter: input.adapter,
    source,
    accountRef: account?.accountRef ?? null,
    accountRefHash: account?.accountRefHash ?? null,
    label,
    detail,
    blocker: null,
    evidence: {
      adapter: input.adapter,
      route: source,
      accountLabel: redactedAccountLabel(account?.accountRef ?? null),
      accountHash: redactedAccountHash(account?.accountRefHash ?? null),
      workspaceRef: input.workspaceRef,
      sourceRef:
        redactedAccountHash(account?.accountRefHash ?? null) ??
        redactedAccountLabel(account?.accountRef ?? null),
    },
  }
}

const blockedRoute = (
  input: CodeModeAccountRouteInput,
  detail: string,
  sourceRef: string | null = null,
): CodeModeAccountRoute => ({
  adapter: input.adapter,
  source: "blocked",
  accountRef: null,
  accountRefHash: null,
  label: `${adapterLabel(input.adapter)} route blocked`,
  detail,
  blocker: detail,
  evidence: {
    adapter: input.adapter,
    route: "blocked",
    accountLabel: redactedAccountLabel(input.selectedAccountRef),
    accountHash: null,
    workspaceRef: input.workspaceRef,
    sourceRef: sourceRef === null ? null : redactedAccountLabel(sourceRef),
  },
})

export const projectCodeModeAccountRoute = (
  input: CodeModeAccountRouteInput,
): CodeModeAccountRoute => {
  const rows = providerAccounts(input)
  const selected = input.selectedAccountRef?.trim() || null
  if (selected !== null) {
    if (rows.length === 0) {
      return {
        adapter: input.adapter,
        source: "explicit",
        accountRef: selected,
        accountRefHash: null,
        label: `${adapterLabel(input.adapter)} ${selected}`,
        detail: "selected account; waiting for live readiness",
        blocker: null,
        evidence: {
          adapter: input.adapter,
          route: "explicit",
          accountLabel: redactedAccountLabel(selected),
          accountHash: null,
          workspaceRef: input.workspaceRef,
          sourceRef: null,
        },
      }
    }
    const row = rows.find((account) => account.accountRef === selected) ?? null
    if (row === null) {
      return blockedRoute(
        input,
        `selected ${adapterLabel(input.adapter)} account "${selected}" is unavailable; choose another account`,
      )
    }
    if (!row.ready) {
      return blockedRoute(
        input,
        `selected ${adapterLabel(input.adapter)} account "${selected}" is blocked; choose another account`,
        row.key,
      )
    }
    return routeFor(input, "explicit", row, "selected account")
  }

  if (input.workspaceRef !== null) {
    const sessions = [...input.sessions].sort((a, b) => {
      const byTime = isoMs(b.updatedAt) - isoMs(a.updatedAt)
      return byTime !== 0 ? byTime : a.sessionRef.localeCompare(b.sessionRef)
    })
    for (const session of sessions) {
      if (
        session.adapter !== input.adapter ||
        (session.workspaceRef ?? null) !== input.workspaceRef ||
        session.accountRefHash === null ||
        session.accountRefHash.trim() === ""
      ) {
        continue
      }
      const account =
        rows.find(
          (row) =>
            row.ready &&
            row.accountRef !== null &&
            row.accountRefHash === session.accountRefHash,
        ) ?? null
      if (account !== null) {
        return routeFor(
          input,
          "last_used",
          account,
          `last used for ${input.workspaceRef}`,
        )
      }
    }
  }

  const priorityAccount =
    sortRouteAccounts(rows).find((row) => row.ready && row.accountRef !== null) ??
    null
  if (priorityAccount !== null) {
    const priority =
      priorityAccount.priority === null ? "automatic priority" : `priority ${priorityAccount.priority}`
    return routeFor(input, "priority", priorityAccount, priority)
  }

  if (input.allowDefaultHome) {
    const defaultHome =
      rows.find(
        (row) =>
          row.ready &&
          row.accountRef === null &&
          (row.selector === "default_home" || row.source === "default_home"),
      ) ?? null
    return routeFor(input, "default_home", defaultHome, "default home fallback")
  }

  return blockedRoute(
    input,
    `${adapterLabel(input.adapter)} has no ready account route for this workspace`,
  )
}

export const nextCodeModeAccountOverride = (
  input: CodeModeAccountRouteInput,
): CodeModeAccountOverride | null => {
  const current = projectCodeModeAccountRoute(input).accountRef
  const candidates: ReadonlyArray<string | null> = [
    ...sortRouteAccounts(providerAccounts(input))
      .filter((row) => row.ready && row.accountRef !== null)
      .map((row) => row.accountRef),
    ...(input.allowDefaultHome ? [null] : []),
  ]
  if (candidates.length === 0) return null
  const currentIndex = candidates.findIndex((ref) => ref === current)
  const next = candidates[(currentIndex + 1) % candidates.length] ?? null
  return {
    accountRef: next,
    label:
      next === null
        ? `${adapterLabel(input.adapter)} default home`
        : `${adapterLabel(input.adapter)} ${next}`,
  }
}

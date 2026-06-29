// VCODE-09 (#5926): pure approval/decision projection for Verse code mode.
//
// The node currently resolves approvals exactly once (approve/deny). This
// projection keeps that contract explicit while giving future nodes a typed,
// public-safe shape for scoped persistent approvals. Missing or unsafe scope
// always blocks "scoped always".

import type { ApprovalRow } from "../shared/rpc.js"

export type DecisionScopeKey =
  | "session"
  | "workspace"
  | "command_class"
  | "account"
  | "expiration"
  | "execution_lane"

export type DecisionScopeRow = {
  readonly key: DecisionScopeKey
  readonly label: string
  readonly value: string
  readonly published: boolean
  readonly persistentRequired: boolean
}

export type DecisionActionKind = "reject" | "allow_once" | "scoped_always"

export type DecisionAction = {
  readonly kind: DecisionActionKind
  readonly label: string
  readonly enabled: boolean
  readonly decision?: "approve" | "deny"
  readonly title: string
}

export type ApprovalDecisionProjection = {
  readonly approvalRef: string
  readonly title: string
  readonly prompt: string
  readonly createdAt: string
  readonly scopeRows: readonly DecisionScopeRow[]
  readonly actions: readonly DecisionAction[]
  readonly scopedAlwaysEnabled: boolean
  readonly scopedAlwaysBlockers: readonly string[]
}

const PUBLIC_OR_PROVIDER_LANES = new Set([
  "assignment",
  "cloud",
  "cloudrun",
  "gcp",
  "market",
  "provider",
  "public",
  "public_assignment",
])

const publicSafeRef = (value: string | null | undefined): string | null => {
  const trimmed = value?.trim()
  if (!trimmed) return null
  if (
    trimmed.startsWith("/") ||
    trimmed.startsWith("~/") ||
    /^[A-Za-z]:[\\/]/.test(trimmed) ||
    trimmed.includes("\\")
  ) {
    return null
  }
  return trimmed
}

const compactHash = (value: string): string => {
  if (value.length <= 18) return value
  const tail = value.slice(-8)
  const parts = value.split(/[.:/-]/).filter(Boolean)
  const provider = parts.find((part) => part === "codex" || part === "claude") ?? "account"
  return `${provider} ...${tail}`
}

const scopeRow = (
  key: DecisionScopeKey,
  label: string,
  value: string | null,
  fallback: string,
  persistentRequired = true,
): DecisionScopeRow => ({
  key,
  label,
  value: value ?? fallback,
  published: value !== null,
  persistentRequired,
})

const approvalTitle = (approval: ApprovalRow): string =>
  approval.prompt.trim() !== "" ? approval.prompt : approval.kind

const laneValue = (approval: ApprovalRow): string | null => {
  const explicit = publicSafeRef(approval.lane)
  if (explicit !== null) return explicit
  const source = publicSafeRef(approval.source)
  if (source !== null) return source
  return publicSafeRef(approval.assignmentPath)
}

const laneBlocksPersistentApproval = (approval: ApprovalRow): boolean => {
  const candidates = [approval.lane, approval.source, approval.assignmentPath]
    .map((value) => publicSafeRef(value)?.toLowerCase())
    .filter((value): value is string => Boolean(value))
  return candidates.some((value) =>
    value
      .split(/[^a-z0-9]+/)
      .filter(Boolean)
      .some((part) => PUBLIC_OR_PROVIDER_LANES.has(part)) ||
    PUBLIC_OR_PROVIDER_LANES.has(value),
  )
}

export const projectApprovalDecision = (
  approval: ApprovalRow,
): ApprovalDecisionProjection => {
  const session = publicSafeRef(approval.sessionRef)
  const workspace = publicSafeRef(approval.workspaceRef)
  const commandClass = publicSafeRef(approval.commandClass) ?? publicSafeRef(approval.kind)
  const account = publicSafeRef(approval.accountRefHash)
  const expiration = publicSafeRef(approval.expiresAt)
  const lane = laneValue(approval)

  const scopeRows = [
    scopeRow("session", "Session", session, "not published"),
    scopeRow("workspace", "Workspace", workspace, "not published"),
    scopeRow("command_class", "Command class", commandClass, "approval"),
    scopeRow("account", "Account", account === null ? null : compactHash(account), "not published"),
    scopeRow("expiration", "Expiration", expiration, "one decision only"),
    scopeRow("execution_lane", "Execution lane", lane, "not published"),
  ] satisfies readonly DecisionScopeRow[]

  const blockers = scopeRows
    .filter((row) => row.persistentRequired && !row.published)
    .map((row) => `${row.label.toLowerCase()} scope not published`)

  if (approval.persistentApprovalSupported !== true) {
    blockers.push("node has no persistent approval verb")
  }
  blockers.push("desktop has no persistent approval control verb")
  if (laneBlocksPersistentApproval(approval)) {
    blockers.push("public assignment, market, and provider lanes cannot use local danger modes")
  }

  const scopedAlwaysEnabled = blockers.length === 0
  const scopedAlwaysTitle = scopedAlwaysEnabled
    ? "Scoped persistent approval is available for the visible scope."
    : `Scoped always blocked: ${blockers.join("; ")}.`

  return {
    approvalRef: approval.approvalRef,
    title: approvalTitle(approval),
    prompt: approval.prompt,
    createdAt: approval.createdAt,
    scopeRows,
    actions: [
      {
        kind: "reject",
        label: "Reject",
        enabled: true,
        decision: "deny",
        title: "Reject this pending approval.",
      },
      {
        kind: "allow_once",
        label: "Allow once",
        enabled: true,
        decision: "approve",
        title: "Allow this approval one time.",
      },
      {
        kind: "scoped_always",
        label: "Scoped always",
        enabled: scopedAlwaysEnabled,
        title: scopedAlwaysTitle,
      },
    ],
    scopedAlwaysEnabled,
    scopedAlwaysBlockers: blockers,
  }
}

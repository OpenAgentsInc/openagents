import type { FleetHarnessKind } from "@openagentsinc/khala-fleet-intents"

import type {
  KhalaCodeDesktopFleetAccount,
  KhalaCodeDesktopFleetAssignment,
  KhalaCodeDesktopFleetStatus,
} from "../../shared/rpc"

// ---------------------------------------------------------------------------
// EN cockpit projection (MH-7 / EN-5)
//
// A PURE adapter from the already-existing live desktop fleet status
// (`KhalaCodeDesktopFleetStatus`, produced by `fleet-run-supervisor.ts` /
// `pylon-service.ts` and surfaced over RPC) into a small, render-ready shape
// the Effect Native cockpit view consumes. This does NOT introduce any new
// backend logic — it is a read-only reshape, the same discipline as
// `fleet-board-projection.ts`.
// ---------------------------------------------------------------------------

export type CockpitChipTone = "ok" | "warn" | "blocked" | "neutral"

export type CockpitCapacityChip = Readonly<{
  key: string
  label: string
  value: string
  tone: CockpitChipTone
}>

export type CockpitHarnessRow = Readonly<{
  key: string
  harnessKind: FleetHarnessKind
  accountRef: string
  readinessLabel: string
  tone: CockpitChipTone
  paused: boolean
}>

export type CockpitRunRow = Readonly<{
  key: string
  runRef: string | null
  assignmentRef: string
  issueRef: string
  elapsedLabel: string
  statusLabel: string
  tone: CockpitChipTone
  needsApproval: boolean
}>

export type CockpitApprovalRow = Readonly<{
  key: string
  approvalRef: string
  issueRef: string
  detail: string
}>

export type EnCockpitState = Readonly<{
  generatedAt: string
  pylonStatusLabel: string
  capacityChips: ReadonlyArray<CockpitCapacityChip>
  harnessRows: ReadonlyArray<CockpitHarnessRow>
  runRows: ReadonlyArray<CockpitRunRow>
  pendingApprovals: ReadonlyArray<CockpitApprovalRow>
  // The FleetRun the run-control buttons steer, when there is an active run.
  // `null` disables (but still renders) the pause/resume/drain/stop controls.
  runControlTargetRef: string | null
}>

export const emptyEnCockpitState: EnCockpitState = {
  generatedAt: "time.khala_fleet_cockpit.pending",
  pylonStatusLabel: "loading",
  capacityChips: [],
  harnessRows: [],
  runRows: [],
  pendingApprovals: [],
  runControlTargetRef: null,
}

const providerToHarness = (
  provider: KhalaCodeDesktopFleetAccount["provider"],
): FleetHarnessKind => (provider === "claude_agent" ? "claude" : "codex")

const accountReadinessTone = (readiness: string): CockpitChipTone => {
  const value = readiness.toLowerCase()
  if (value === "ready") return "ok"
  if (value.includes("missing")) return "blocked"
  return "warn"
}

const readinessLabel = (readiness: string): string => {
  const value = readiness.toLowerCase()
  if (value === "ready") return "ready"
  if (value.includes("missing")) return "missing sign-in"
  return readiness.replace(/_/g, " ")
}

const formatElapsed = (elapsedMs: number | null): string => {
  if (elapsedMs === null || elapsedMs < 0) return "elapsed unknown"
  const totalSeconds = Math.floor(elapsedMs / 1000)
  const minutes = Math.floor(totalSeconds / 60)
  const seconds = totalSeconds % 60
  return `${minutes}m ${String(seconds).padStart(2, "0")}s`
}

const assignmentApprovalState = (
  assignment: KhalaCodeDesktopFleetAssignment,
): string | null => assignment.workerSession?.approvalState ?? null

const assignmentNeedsApproval = (
  assignment: KhalaCodeDesktopFleetAssignment,
): boolean => {
  const state = assignmentApprovalState(assignment)
  return state === "approval_required" || state === "ready_for_review"
}

const runStatusLabel = (
  assignment: KhalaCodeDesktopFleetAssignment,
): { label: string; tone: CockpitChipTone } => {
  const review = assignment.workerSession?.reviewState
  if (assignmentNeedsApproval(assignment)) {
    return { label: "approval required", tone: "warn" }
  }
  if (review === "blocked") return { label: "blocked", tone: "blocked" }
  if (review === "ready_for_review") return { label: "ready for review", tone: "warn" }
  if (review === "pending_closeout") return { label: "closing out", tone: "neutral" }
  return { label: "active", tone: "ok" }
}

export const buildEnCockpitProjection = (
  status: KhalaCodeDesktopFleetStatus,
): EnCockpitState => {
  const readyAccounts = status.accounts.filter(
    (account) => account.readiness.toLowerCase() === "ready",
  ).length

  const pylonBlocked = status.pylon.status === "unavailable"
  const pylonStatusLabel = pylonBlocked ? "offline" : status.pylon.status

  const slotsValue =
    status.availableCodexAssignments === null ||
    status.maxCodexAssignments === null
      ? "not reported"
      : `${status.availableCodexAssignments}/${status.maxCodexAssignments} free`

  const slotsTone: CockpitChipTone =
    status.availableCodexAssignments === null || status.maxCodexAssignments === null
      ? "neutral"
      : status.availableCodexAssignments === 0
        ? "blocked"
        : "ok"

  const capacityChips: ReadonlyArray<CockpitCapacityChip> = [
    {
      key: "chip-pylon",
      label: "pylon",
      value: pylonStatusLabel,
      tone: pylonBlocked ? "blocked" : "ok",
    },
    {
      key: "chip-accounts",
      label: "accounts ready",
      value: `${readyAccounts}/${status.accounts.length}`,
      tone:
        status.accounts.length === 0
          ? "neutral"
          : readyAccounts === 0
            ? "blocked"
            : readyAccounts < status.accounts.length
              ? "warn"
              : "ok",
    },
    {
      key: "chip-slots",
      label: "codex slots",
      value: slotsValue,
      tone: slotsTone,
    },
    {
      key: "chip-runs",
      label: "active runs",
      value: String(status.activeAssignments.length),
      tone: status.activeAssignments.length > 0 ? "ok" : "neutral",
    },
  ]

  const harnessRows: ReadonlyArray<CockpitHarnessRow> = status.accounts.map(
    (account, index) => ({
      key: `harness-${index + 1}`,
      harnessKind: providerToHarness(account.provider),
      accountRef: account.accountRef,
      readinessLabel: readinessLabel(account.readiness),
      tone: accountReadinessTone(account.readiness),
      paused: account.paused === true,
    }),
  )

  const runRows: ReadonlyArray<CockpitRunRow> = status.activeAssignments.map(
    (assignment, index) => {
      const status = runStatusLabel(assignment)
      const assignmentRef =
        assignment.assignmentRef ?? `assignment.khala_fleet.pending.${index + 1}`
      return {
        key: `run-${index + 1}`,
        runRef: assignment.runRef ?? null,
        assignmentRef,
        issueRef: assignment.issueRef ?? "issue.khala_fleet.unset",
        elapsedLabel: formatElapsed(assignment.elapsedMs),
        statusLabel: status.label,
        tone: status.tone,
        needsApproval: assignmentNeedsApproval(assignment),
      }
    },
  )

  const pendingApprovals: ReadonlyArray<CockpitApprovalRow> = status.activeAssignments
    .map((assignment, index) => ({ assignment, index }))
    .filter(({ assignment }) => assignmentNeedsApproval(assignment))
    .map(({ assignment, index }) => ({
      key: `approval-${index + 1}`,
      // The assignment ref is the one approval-authority surface (Inbox).
      approvalRef:
        assignment.assignmentRef ?? `assignment.khala_fleet.pending.${index + 1}`,
      issueRef: assignment.issueRef ?? "issue.khala_fleet.unset",
      detail:
        assignmentApprovalState(assignment) === "ready_for_review"
          ? "worker is ready for review"
          : "worker is waiting on an approval decision",
    }))

  const firstRunWithRef = status.activeAssignments.find(
    (assignment) => (assignment.runRef ?? null) !== null,
  )

  return {
    generatedAt: status.observedAt,
    pylonStatusLabel,
    capacityChips,
    harnessRows,
    runRows,
    pendingApprovals,
    runControlTargetRef: firstRunWithRef?.runRef ?? null,
  }
}

import type {
  NotificationCenterView,
  SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"

export type SessionEventRow = {
  readonly eventIndex: number
  readonly phase: string
  readonly state: string
  readonly observedAt: string
  readonly detail: string
  // CL-52: full untruncated content, revealed on click-to-expand. Optional so
  // existing fixtures stay valid; the Bun side always populates it.
  readonly full?: string
}

export type AccountRow = {
  readonly provider: string
  readonly homeState: string
  readonly ready: boolean
}

export type SessionArtifactStats = {
  readonly kind: string
  readonly outcome: string | null
  readonly editedFileCount: number | null
  readonly commandCount: number | null
  readonly totalTokens: number | null
}

// CL-26 "Deploy to Cloud": read-only projection of the node's last deploy.
export type DeployStatusRow = {
  readonly state: "queued" | "building" | "deployed" | "failed" | "unknown"
  readonly url: string | null
  readonly deployedAt: string | null
  readonly message: string
}

export type DeployResultRow = {
  readonly accepted: boolean
  readonly reason: string
  readonly errors: string[]
}

export type TrainingRunState = "planned" | "active" | "sealed" | "reconciled"

export type TrainingWindowState = "planned" | "active" | "sealed" | "reconciled"

export type TrainingPublicMetric = {
  readonly provenanceLabel: string
  readonly sourceRefs: readonly string[]
  readonly value: number
}

export type TrainingRunProjectionRow = {
  readonly createdAtDisplay: string
  readonly maxAllowedStale: number
  readonly promiseRef: string
  readonly receiptRefs: readonly string[]
  readonly sealInFlight: boolean
  readonly sealPublicationCadenceWindows: number
  readonly sourceRefs: readonly string[]
  readonly state: TrainingRunState
  readonly trainingRunRef: string
  readonly updatedAtDisplay: string
}

export type TrainingWindowProjectionRow = {
  readonly datasetRefs: readonly string[]
  readonly homeworkKind: string
  readonly plannedAtDisplay: string
  readonly priority: number
  readonly receiptRefs: readonly string[]
  readonly sealMetadata: unknown | null
  readonly sourceRefs: readonly string[]
  readonly state: TrainingWindowState
  readonly trainingRunRef: string
  readonly updatedAtDisplay: string
  readonly windowRef: string
}

export type TrainingRunMetricsRow = {
  readonly activeWindowCount: TrainingPublicMetric
  readonly assignedContributorCount: TrainingPublicMetric
  readonly pendingPayoutCount: TrainingPublicMetric
  readonly plannedWindowCount: TrainingPublicMetric
  readonly providerConfirmedSettledPayoutSats: TrainingPublicMetric
  readonly receiptRefCount: TrainingPublicMetric
  readonly reconciledWindowCount: TrainingPublicMetric
  readonly rejectedWorkCount: TrainingPublicMetric
  readonly sealedWindowCount: TrainingPublicMetric
  readonly verifiedWorkCount: TrainingPublicMetric
}

export type TrainingRunRealGradientRow = {
  readonly closeoutRequirement: {
    readonly evalRef: string | null
    readonly freivaldsCommitmentRefs: readonly string[]
    readonly gradientCloseoutRefs: readonly string[]
    readonly mergeRef: string | null
    readonly provenanceLabel: string
    readonly satisfied: boolean
  }
  readonly deviceRequirement: {
    readonly observedDistinctContributorDevices: number
    readonly provenanceLabel: string
    readonly requiredDistinctContributorDevices: number
    readonly satisfied: boolean
    readonly sourceRefs: readonly string[]
  }
  readonly externalAsk: {
    readonly blockerRefs: readonly string[]
    readonly psionicLaneRef: string
    readonly requirementRefs: readonly string[]
    readonly status: string
  }
  readonly lossUnderBudget: {
    readonly budgetLabel: string
    readonly budgetRef: string | null
    readonly finalValidationLoss: number | null
    readonly maxValidationLoss: number | null
    readonly provenanceLabel: string
    readonly satisfied: boolean
    readonly sourceRefs: readonly string[]
  }
  readonly scopeBoundaryRefs: readonly string[]
}

export type TrainingRunSummaryRow = {
  readonly copyBoundaryRefs: readonly string[]
  readonly emptyState: {
    readonly idle: boolean
    readonly reason: string
  }
  readonly metrics: TrainingRunMetricsRow
  readonly realGradient: TrainingRunRealGradientRow
  readonly receiptRefs: readonly string[]
  readonly run: TrainingRunProjectionRow
  readonly sourceRefs: readonly string[]
  readonly windows: readonly TrainingWindowProjectionRow[]
}

export type TrainingRunsResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly runs: readonly TrainingRunProjectionRow[]
  readonly summaries: readonly TrainingRunSummaryRow[]
  readonly error?: string
}

// CL-47: an "ask" the owner submitted, with its ship-status round-trip state.
export type IntentRow = {
  readonly intentId: string
  readonly title: string
  readonly status: string
  readonly submittedByClientRef: string
}

// CL-48: a pending approval/decision the node is waiting on.
export type ApprovalRow = {
  readonly approvalRef: string
  readonly kind: string
  readonly prompt: string
  readonly createdAt: string
}

// CL-49: read-only MDK wallet status (no spend authority).
export type WalletStatusRow = {
  readonly configured: boolean
  readonly daemonOnline: boolean
  readonly balanceSats: number | null
  readonly receiveReady: boolean
  readonly sendReady: boolean
  readonly readiness: string
}

// CL-50: an open work-lease assignment (read-only).
export type AssignmentRow = {
  readonly assignmentRef: string
  readonly leaseRef: string
  readonly goal: string
  readonly paymentMode: string
  readonly expiresAt: string
}

export type NodeStateMessage = {
  readonly ok: boolean
  readonly schema: string
  readonly sessions: SessionSummary[]
  // CL-5: bounded recent-events tail per session for the live detail timeline.
  readonly events?: Record<string, SessionEventRow[]>
  // CL-18/CL-20: read-only provider/account readiness.
  readonly accounts?: AccountRow[]
  // CL-19: retained artifact stats per terminal session.
  readonly artifacts?: Record<string, SessionArtifactStats>
  // CL-26: read-only projection of the node's last "Deploy to Cloud".
  readonly deploy?: DeployStatusRow
  // CL-47: the owner's recent asks + their ship-status.
  readonly intents?: IntentRow[]
  // CL-48: pending approvals/decisions awaiting the owner.
  readonly approvals?: ApprovalRow[]
  // CL-49: read-only MDK wallet balance/readiness.
  readonly wallet?: WalletStatusRow | null
  // CL-50: open work-lease assignments (read-only).
  readonly assignments?: AssignmentRow[]
  // CL-51: node coordinator paused flag (null when the node doesn't expose it).
  readonly coordinatorPaused?: boolean | null
}

export type DesktopRPCSchema = {
  readonly bun: {
    readonly requests: {
      // CL-26: trigger a deploy of the node's own cloud service. The node
      // fail-safe-gates execution behind OA_DEPLOY_ENABLE=1.
      readonly deployCloud: {
        readonly params: { target: "cloudrun" | "workers"; ref: string; env?: "production" | "preview" }
        readonly response: DeployResultRow
      }
      // CL-47: submit an "ask" (work intent) to the node.
      readonly submitIntent: {
        readonly params: { title: string; body: string }
        readonly response: { ok: boolean; status: string; error?: string }
      }
      // Read public Worker-authoritative training run projections. This is a
      // read-only desktop projection; admin mutations stay out of the webview.
      readonly listTrainingRuns: {
        readonly params: Record<string, never>
        readonly response: TrainingRunsResponse
      }
      // CL-48: resolve a pending approval (approve/deny). Node enforces
      // exactly-once; a duplicate resolve returns duplicate:true.
      readonly resolveApproval: {
        readonly params: { approvalRef: string; decision: "approve" | "deny" }
        readonly response: { applied: boolean; duplicate: boolean; decision: string }
      }
      // CL-51: pause/resume the node's autonomous coordinator loop.
      readonly setCoordinatorPaused: {
        readonly params: { paused: boolean }
        readonly response: { paused: boolean }
      }
      // CL-52: cancel a running/queued session.
      readonly cancelSession: {
        readonly params: { sessionRef: string }
        readonly response: { ok: boolean; state: string }
      }
      // CL-57: directly spawn a bounded session on the node.
      readonly spawnSession: {
        readonly params: { adapter: "codex" | "claude_agent"; objective: string; verify?: string[] }
        readonly response: { ok: boolean; sessionRef: string; error?: string }
      }
    }
    readonly messages: Record<string, never>
  }
  readonly webview: {
    readonly requests: Record<string, never>
    readonly messages: {
      readonly nodeState: NodeStateMessage
      // CL-30: in-app notification center (unread count + recent items),
      // derived from newly notify-worthy sessions on the Bun side.
      readonly notifications: NotificationCenterView
    }
  }
}

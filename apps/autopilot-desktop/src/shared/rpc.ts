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

export type TrainingLeaderboardLane =
  | "a1_loss"
  | "a2_throughput"
  | "a3_isoflop"
  | "a4_eval_delta"
  | "a5_accuracy"

export type TrainingLeaderboardTopRow = {
  readonly contributorRef: string
  readonly rank: number
  readonly score: number
  readonly scoreLabel: string
  readonly settledPayoutSats: number
  readonly trainingRunRef: string
}

export type TrainingLeaderboardLaneSummary = {
  readonly blockerRefs: readonly string[]
  readonly lane: TrainingLeaderboardLane
  readonly rowCount: number
  readonly title: string
  readonly topRow: TrainingLeaderboardTopRow | null
}

export type TrainingDashboardSummaryResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly leaderboards: {
    readonly blockerRefs: readonly string[]
    readonly lanes: readonly TrainingLeaderboardLaneSummary[]
  }
  readonly a2: {
    readonly blockerRefs: readonly string[]
    readonly observedDeviceClassCount: number
    readonly observedMeasurementCount: number
    readonly verifiedMeasurementCount: number
  }
  readonly a3: {
    readonly blockerRefs: readonly string[]
    readonly cellCount: number
    readonly fitArtifactCount: number
    readonly verifiedCellCount: number
  }
  readonly a4: {
    readonly blockerRefs: readonly string[]
    readonly evalDeltaBonusBlockerRefs: readonly string[]
    readonly observedVerifiedStages: readonly string[]
    readonly requiredVerifiedStageCount: number
    readonly shardCount: number
  }
  readonly a5: {
    readonly blockerRefs: readonly string[]
    readonly evalSuiteCount: number
    readonly updateBoundaryRef: string | null
    readonly verifiedSuiteCount: number
  }
  readonly error?: string
}

export type TrainingPromiseState =
  | "degraded"
  | "green"
  | "planned"
  | "red"
  | "withdrawn"
  | "yellow"
  | "unknown"

export type TrainingPromiseSummary = {
  readonly blockerRefs: readonly string[]
  readonly claim: string
  readonly evidenceRefCount: number
  readonly productArea: string
  readonly promiseId: string
  readonly safeCopy: string
  readonly state: TrainingPromiseState
  readonly verification: string
}

export type TrainingPromiseGatesResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly registryVersion: string
  readonly sourceUrl: string
  readonly blockerRefs: readonly string[]
  readonly promises: readonly TrainingPromiseSummary[]
  readonly stateCounts: Readonly<Record<TrainingPromiseState, number>>
  readonly error?: string
}

export type TrainingOperatorReadinessPylonRefSource =
  | "env"
  | "identity"
  | "missing"

export type TrainingOperatorReadinessResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly trainingBaseUrl: string
  readonly adminEnabled: boolean
  readonly adminTokenPresent: boolean
  readonly adminReady: boolean
  readonly leaseEnabled: boolean
  readonly leaseReady: boolean
  readonly pylonRefPresent: boolean
  readonly pylonRefSource: TrainingOperatorReadinessPylonRefSource
  readonly pylonRef: string | null
  readonly pylonHomePresent: boolean
  readonly controlTokenPresent: boolean
  readonly localPylonReady: boolean
  readonly evidenceEnabled: boolean
  readonly evidencePacketPathPresent: boolean
  readonly evidenceReady: boolean
  readonly blockerRefs: readonly string[]
  readonly error?: string
}

export type TrainingEvidencePacketSummaryResponse = {
  readonly ok: boolean
  readonly configured: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly packetSource: string | null
  readonly budgetLabel: string | null
  readonly budgetRefPresent: boolean
  readonly evalRefPresent: boolean
  readonly mergeRefPresent: boolean
  readonly finalValidationLoss: number | null
  readonly maxValidationLoss: number | null
  readonly lossPointCount: number
  readonly freivaldsCommitmentRefCount: number
  readonly gradientCloseoutRefCount: number
  readonly evidenceRefCount: number
  readonly receiptRefCount: number
  readonly shardContributionCount: number
  readonly distinctPylonCount: number
  readonly blockerRefs: readonly string[]
  readonly error?: string
}

export type TrainingPlanReason =
  | "disabled"
  | "admin_token_missing"
  | "run_plan_failed"
  | "window_plan_failed"
  | "request_failed"
  | "planned"

export type TrainingPlanResponse = {
  readonly ok: boolean
  readonly enabled: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly trainingRunRef: string | null
  readonly windowRef: string | null
  readonly run: TrainingRunProjectionRow | null
  readonly window: TrainingWindowProjectionRow | null
  readonly runPlanned: boolean
  readonly windowPlanned: boolean
  readonly reason: TrainingPlanReason
  readonly message: string
  readonly error?: string
}

export type TrainingWindowActionReason =
  | "disabled"
  | "admin_token_missing"
  | "invalid_window_ref"
  | "transition_failed"
  | "request_failed"
  | "activated"
  | "reconciled"

export type TrainingWindowActionResponse = {
  readonly ok: boolean
  readonly enabled: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly windowRef: string | null
  readonly window: TrainingWindowProjectionRow | null
  readonly reason: TrainingWindowActionReason
  readonly message: string
  readonly error?: string
}

export type TrainingWindowLeaseRow = {
  readonly claimedAtDisplay: string
  readonly leaseExpiresInSeconds: number
  readonly leaseRef: string
  readonly pylonRef: string
  readonly receiptRefs: readonly string[]
  readonly state: "active" | "released"
  readonly trainingRunRef: string
  readonly windowRef: string
}

export type TrainingWindowLeaseReason =
  | "disabled"
  | "pylon_ref_missing"
  | "invalid_pylon_ref"
  | "claim_failed"
  | "request_failed"
  | "claimed"

export type TrainingWindowLeaseResponse = {
  readonly ok: boolean
  readonly enabled: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly pylonRef: string | null
  readonly lease: TrainingWindowLeaseRow | null
  readonly reason: TrainingWindowLeaseReason
  readonly message: string
  readonly error?: string
}

export type TrainingBootstrapGrantRow = {
  readonly checkpointDigestRef: string
  readonly grantRef: string
  readonly joinerReceiptRefs: readonly string[]
  readonly joinerRef: string
  readonly sealReceiptRefs: readonly string[]
  readonly sealedAtDisplay: string
  readonly sealedWindowRef: string
  readonly trainingRunRef: string
}

export type TrainingBootstrapOutcome =
  | {
      readonly kind: "granted"
      readonly grant: TrainingBootstrapGrantRow
    }
  | {
      readonly joinerRef: string
      readonly kind: "queued"
      readonly reasonCode: string
      readonly trainingRunRef: string
    }
  | {
      readonly joinerRef: string
      readonly kind: "refused"
      readonly reason: string
      readonly reasonCode: string
      readonly trainingRunRef: string
    }

export type TrainingBootstrapGrantReason =
  | "pylon_ref_missing"
  | "invalid_pylon_ref"
  | "invalid_run_ref"
  | "request_failed"
  | "granted"
  | "queued"
  | "refused"

export type TrainingBootstrapGrantResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly pylonRef: string | null
  readonly trainingRunRef: string | null
  readonly outcome: TrainingBootstrapOutcome | null
  readonly reason: TrainingBootstrapGrantReason
  readonly message: string
  readonly error?: string
}

export type TrainingEvidenceAdmissionReason =
  | "disabled"
  | "admin_token_missing"
  | "invalid_run_ref"
  | "packet_path_missing"
  | "packet_read_failed"
  | "packet_invalid"
  | "admission_failed"
  | "request_failed"
  | "admitted"

export type TrainingEvidenceAdmissionResponse = {
  readonly ok: boolean
  readonly enabled: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly trainingRunRef: string | null
  readonly packetSource: string | null
  readonly run: TrainingRunProjectionRow | null
  readonly realGradient: TrainingRunRealGradientRow | null
  readonly reason: TrainingEvidenceAdmissionReason
  readonly message: string
  readonly evidenceRefCount: number
  readonly receiptRefCount: number
  readonly shardContributionCount: number
  readonly distinctPylonCount: number
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
      readonly listTrainingDashboard: {
        readonly params: Record<string, never>
        readonly response: TrainingDashboardSummaryResponse
      }
      readonly listTrainingPromiseGates: {
        readonly params: Record<string, never>
        readonly response: TrainingPromiseGatesResponse
      }
      readonly listTrainingOperatorReadiness: {
        readonly params: Record<string, never>
        readonly response: TrainingOperatorReadinessResponse
      }
      readonly listTrainingEvidencePacketSummary: {
        readonly params: Record<string, never>
        readonly response: TrainingEvidencePacketSummaryResponse
      }
      // Admin planning stays in Bun. The webview receives only the public-safe
      // run/window refs and projections that the Worker returns.
      readonly planTrainingRunWindow: {
        readonly params: Record<string, never>
        readonly response: TrainingPlanResponse
      }
      readonly activateTrainingWindow: {
        readonly params: { windowRef: string }
        readonly response: TrainingWindowActionResponse
      }
      readonly reconcileTrainingWindow: {
        readonly params: { windowRef: string }
        readonly response: TrainingWindowActionResponse
      }
      readonly claimTrainingWindowLease: {
        readonly params: Record<string, never>
        readonly response: TrainingWindowLeaseResponse
      }
      readonly requestTrainingBootstrapGrant: {
        readonly params: { trainingRunRef: string }
        readonly response: TrainingBootstrapGrantResponse
      }
      readonly admitTrainingRealGradientEvidence: {
        readonly params: { trainingRunRef: string }
        readonly response: TrainingEvidenceAdmissionResponse
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

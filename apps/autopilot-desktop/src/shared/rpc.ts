import type {
  NotificationCenterView,
  SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import type { PublicActivityTimelineEnvelope } from "@openagentsinc/public-activity-timeline"
import type { InstallReadinessResponse } from "./install-readiness"
import type { OnboardingStatusResponse } from "./onboarding-status"
import type {
  PromiseSurfacingDraft,
  PromiseSurfacingInput,
} from "./promise-surfacing"

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
  // CS-A1: the registry/sibling account ref (null for a provider's default
  // home, which has no explicit ref). Threaded through session.spawn as
  // `accountRef` for the per-session account picker — no new control contract.
  readonly accountRef: string | null
  // Stable per-account public ref hash (refs only; never a raw path/secret).
  readonly accountRefHash: string
  // "registry_ref" (an explicit `dev.accounts` entry / discovered sibling home)
  // or "default_home" (the provider's default ~/.codex / ~/.claude home).
  readonly selector: string
  // Public-safe readiness blocker refs (e.g. home missing, login required).
  readonly blockerRefs: readonly string[]
  // CS-A1: operator-assigned dispatch priority (lower = preferred). Surfaced
  // and editable through the desktop account-management UI; persisted in the
  // node's local `dev.accounts` config. Null when the account has no explicit
  // entry (default homes / discovered siblings without a priority set).
  readonly priority: number | null
}

// CS-A1: a managed local account registry entry (read-only projection of the
// node's `dev.accounts` config the desktop account-management UI edits).
export type ManagedAccountRow = {
  readonly ref: string
  readonly provider: "codex" | "claude_agent"
  readonly homePresent: boolean
  readonly priority: number | null
}

export type ManagedAccountsResponse = {
  readonly ok: boolean
  readonly accounts: readonly ManagedAccountRow[]
  readonly error?: string
}

export type ManagedAccountMutationResponse = {
  readonly ok: boolean
  readonly accounts: readonly ManagedAccountRow[]
  readonly error?: string
}

export type SessionArtifactStats = {
  readonly kind: string
  readonly outcome: string | null
  readonly editedFileCount: number | null
  readonly commandCount: number | null
  readonly totalTokens: number | null
  // #5470: the redaction-safe, ref-only detail of the retained `session.artifact`
  // payload (proof for completed, failure for failed). Every field here is a
  // ref/digest/enum the node already deemed public-projection-safe (the proof
  // JSON passes `assertPublicProjectionSafe` + the redaction scan before it is
  // written) — never a seed, token, raw path, or raw secret. Optional /
  // back-compat: absent on older projections (which only carried the stats
  // above), so consumers must tolerate `detail === undefined`.
  readonly detail?: SessionArtifactDetail
}

// #5470: dereferenceable refs surfaced in the artifact & receipt browser. These
// mirror the proof/failure artifact JSON fields written by
// apps/pylon/src/node/control-sessions.ts (writeRetainedArtifact /
// writeFailureArtifact). The browser shows these as inspectable rows; it never
// fetches or renders the raw artifact body.
export type SessionArtifactDetail = {
  // The artifact schema id (e.g. the control-session proof/failure schema).
  readonly schema: string | null
  // The session's task refs (objective digest + verify ref).
  readonly objectiveDigestRef: string | null
  readonly verifyRef: string | null
  // The agent's response digest + any external (nested) session ref.
  readonly responseDigestRef: string | null
  readonly externalSessionRef: string | null
  // Execution provenance (path/mode/sandbox/permission/network) — bounded enums.
  readonly executionPathRef: string | null
  readonly executionMode: string | null
  readonly sandboxMode: string | null
  readonly permissionMode: string | null
  // The dev-check (verify) state the artifact recorded, and any deviation refs.
  readonly devCheckState: string | null
  readonly deviationRefs: readonly string[]
  // The redaction-scan posture the node attached to this artifact.
  readonly redactionState: string | null
  // Failure artifacts: the error class + its digest ref (no raw error text).
  readonly errorClass: string | null
  readonly errorDigestRef: string | null
  // The workspace ref the session ran in (a public ref, not a path).
  readonly workspaceRef: string | null
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

export type TrainingEvidencePacketBuildReason =
  | "disabled"
  | "invalid_run_ref"
  | "worker_receipts_path_missing"
  | "packet_path_missing"
  | "worker_receipts_read_failed"
  | "worker_receipts_invalid"
  | "packet_write_failed"
  | "packet_blocked"
  | "written"

export type TrainingEvidencePacketBuildResponse = {
  readonly ok: boolean
  readonly enabled: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly trainingRunRef: string | null
  readonly inputSource: string | null
  readonly packetSource: string | null
  readonly reason: TrainingEvidencePacketBuildReason
  readonly message: string
  readonly summary: TrainingEvidencePacketSummaryResponse | null
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

export type PublicActivityTimelineResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly envelope: PublicActivityTimelineEnvelope | null
  readonly error?: string
}

export type BuiltInAgentReadinessResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly enabled: boolean
  readonly localPylonReady: boolean
  readonly hostedComputeConfigured: boolean
  readonly userApiKeyRequired: false
  readonly lane: "cloud-gcp" | "cloud-shc"
  readonly modelSet: string
  readonly maxSessionSeconds: number
  readonly dailySessionCap: number
  readonly dailySessionsUsed: number
  readonly meteringLabel: string
  readonly worktreePathPresent: boolean
  readonly blockerRefs: readonly string[]
  readonly error?: string
}

export type BuiltInAgentStartResponse = {
  readonly ok: boolean
  readonly sessionRef: string
  readonly readiness: BuiltInAgentReadinessResponse
  readonly error?: string
}

export type AppleFmReadinessResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: "desktop:apple-fm-readiness"
  readonly localPylonReady: boolean
  readonly available: boolean
  readonly status: string
  readonly backendKind: string
  readonly profileId: string
  readonly model: string
  readonly capability: string
  readonly advertisedCapabilities: readonly string[]
  readonly baseUrl: string
  readonly platform: string | null
  readonly version: string | null
  readonly unavailableReason: string | null
  readonly message: string | null
  readonly blockerRefs: readonly string[]
  readonly error?: string
}

export type AppleFmSessionStartResponse = {
  readonly ok: boolean
  readonly sessionRef: string
  readonly readiness: AppleFmReadinessResponse
  readonly blockerRefs: readonly string[]
  readonly error?: string
}

export type { InstallReadinessResponse } from "./install-readiness"
export type {
  OnboardingStatusResponse,
  OnboardingStep,
  OnboardingStepStatus,
} from "./onboarding-status"
export type {
  PromiseSurfacingDraft,
  PromiseSurfacingInput,
} from "./promise-surfacing"

// AO-3 (#5444): public-safe first-run identity-choice projection for the
// webview. No seeds, no tokens — only detection booleans + public labels.
export type IdentityChoiceKind = "use_existing" | "create_new"
export type IdentityChoiceStateResponse = {
  readonly choiceNeeded: boolean
  readonly detected: {
    readonly present: boolean
    readonly shortLabel: string | null
    readonly npub: string | null
    readonly source: string | null
  }
  readonly chosen: {
    readonly kind: IdentityChoiceKind
    readonly displayName: string | null
  } | null
  readonly createNewAvailable: true
}

// AO-3 (#5444): the webview asks the Bun host to record the user's choice. The
// Bun host re-verifies the seed marker before adopting an existing home; the
// webview never names a raw home path for create-new (Bun owns the managed home).
export type ChooseIdentityParams =
  | { readonly kind: "use_existing" }
  | { readonly kind: "create_new"; readonly displayName: string }
export type ChooseIdentityResponse = {
  readonly ok: boolean
  readonly state: IdentityChoiceStateResponse
  readonly error?: string
}

export type PromiseSurfacingReadinessResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: "desktop:promise-surfacing-readiness"
  readonly forumSlug: "product-promises"
  readonly baseUrl: string
  readonly productPromisesUrl: string
  readonly forumTopicsUrl: string
  readonly agentTokenPresent: boolean
  readonly blockerRefs: readonly string[]
}

export type PromiseSurfacingResponse = {
  readonly ok: boolean
  readonly mode: "posted" | "drafted" | "blocked"
  readonly draft: PromiseSurfacingDraft | null
  readonly topicId?: string | null
  readonly topicUrl?: string | null
  readonly blockerRefs: readonly string[]
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
      // #5063: read the packaged no-user-key built-in agent readiness. The Bun
      // host keeps cloud control credentials; the webview sees only booleans,
      // labels, bounds, and blocker refs.
      readonly builtinAgentReadiness: {
        readonly params: Record<string, never>
        readonly response: BuiltInAgentReadinessResponse
      }
      // #5063: start the built-in hosted-compute agent from the Bun host. The
      // webview cannot choose credentials, worktree, or bounds.
      readonly startBuiltInAgent: {
        readonly params: Record<string, never>
        readonly response: BuiltInAgentStartResponse
      }
      // #5071: public-safe local Apple FM readiness. Bun/Pylon own the control
      // token and bridge details; the webview receives status, labels, and
      // blocker refs only.
      readonly appleFmReadiness: {
        readonly params: Record<string, never>
        readonly response: AppleFmReadinessResponse
      }
      // #5072: start the bounded local Apple FM chat/tool loop from Bun. The
      // webview supplies no prompt, callback URL, token, helper path, or tool
      // authority; Bun/Pylon own those details and return a normal sessionRef.
      readonly startAppleFmSession: {
        readonly params: Record<string, never>
        readonly response: AppleFmSessionStartResponse
      }
      // #5064: one public-safe first-run health projection for normal installs.
      readonly installReadiness: {
        readonly params: Record<string, never>
        readonly response: InstallReadinessResponse
      }
      // AO-4 (#5445): the live onboarding chain status for the first-run wizard
      // (identity → registered → online → wallet → payout → presence → Tassadar
      // → claimed → earned). Public-safe; reflects real node/registration state.
      readonly onboardingStatus: {
        readonly params: Record<string, never>
        readonly response: OnboardingStatusResponse
      }
      // AO-3 (#5444): read the first-run identity-choice state (detect existing
      // Pylon vs needs-choice). Public-safe (npub/refs only; never the seed).
      readonly identityChoiceState: {
        readonly params: Record<string, never>
        readonly response: IdentityChoiceStateResponse
      }
      // AO-3 (#5444): record the user's first-run identity choice. The Bun host
      // re-verifies an existing home's seed marker before adopting it and never
      // overwrites a different home.
      readonly chooseIdentity: {
        readonly params: ChooseIdentityParams
        readonly response: ChooseIdentityResponse
      }
      // #5065: build and optionally post an Orrery-style Product Promises Forum
      // report. Bun owns the registered-agent token; the webview sends only
      // public-safe report fields and receives the generated draft/result.
      readonly promiseSurfacingReadiness: {
        readonly params: Record<string, never>
        readonly response: PromiseSurfacingReadinessResponse
      }
      readonly surfacePromiseGap: {
        readonly params: PromiseSurfacingInput
        readonly response: PromiseSurfacingResponse
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
      // Read the public activity spine directly from the Worker. This is
      // observation-only and remains usable without a local node connection.
      readonly listPublicActivityTimeline: {
        readonly params: Record<string, never>
        readonly response: PublicActivityTimelineResponse
      }
      readonly buildTrainingEvidencePacket: {
        readonly params: { trainingRunRef: string }
        readonly response: TrainingEvidencePacketBuildResponse
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
      // #4998: `lane` selects the execution lane (auto|local|cloud-gcp|cloud-shc,
      // default auto = own-Pylon-first then cloud-gcp). Optional for backward
      // compat; omitted means the node defaults to auto.
      readonly spawnSession: {
        readonly params: {
          adapter: "codex" | "claude_agent"
          objective: string
          verify?: string[]
          lane?: "auto" | "local" | "cloud-gcp" | "cloud-shc"
          timeoutSeconds?: number
          worktreePath?: string
          // CS-A1: run this session under a specific provider account. The
          // node resolves it against its registry and rejects an unknown ref;
          // omitted means the node's default account selection. No new wire
          // contract — `session.spawn` already accepts `accountRef` (#4868).
          accountRef?: string
        }
        readonly response: { ok: boolean; sessionRef: string; error?: string }
      }
      // CS-A1: spawn a bounded local Apple Foundation Models coding session,
      // exposed as a spawn-adapter option alongside codex/claude. The webview
      // supplies an objective + optional worktree; Bun/Pylon own the prompt
      // policy, control token, and bridge details (the existing
      // apple_fm.session.start verb — no new contract).
      readonly spawnAppleFmSession: {
        readonly params: { objective: string; worktreePath?: string }
        readonly response: AppleFmSessionStartResponse
      }
      // CS-A1 account management — read/add/remove/set-priority against the
      // node's local `dev.accounts` config the runtime already reads. Bun owns
      // the node home + config path; the webview only sees public-safe refs.
      readonly listManagedAccounts: {
        readonly params: Record<string, never>
        readonly response: ManagedAccountsResponse
      }
      readonly addManagedAccount: {
        readonly params: {
          ref: string
          provider: "codex" | "claude_agent"
          home: string
          priority?: number
        }
        readonly response: ManagedAccountMutationResponse
      }
      readonly removeManagedAccount: {
        readonly params: { ref: string; provider: "codex" | "claude_agent" }
        readonly response: ManagedAccountMutationResponse
      }
      readonly setManagedAccountPriority: {
        readonly params: {
          ref: string
          provider: "codex" | "claude_agent"
          priority: number
        }
        readonly response: ManagedAccountMutationResponse
      }
    }
    readonly messages: Record<string, never>
  }
  readonly webview: {
    readonly requests: Record<string, never>
    readonly messages: {
      readonly nodeState: NodeStateMessage
      // #5049: public pylon-network stats snapshot (GET /api/public/pylon-stats),
      // polled on the Bun side and pushed to drive the home network scene.
      // `unknown` (opaque) — the webview projects it via projectPylonNetworkScene.
      readonly pylonStats: unknown
      // CL-30: in-app notification center (unread count + recent items),
      // derived from newly notify-worthy sessions on the Bun side.
      readonly notifications: NotificationCenterView
      // #5025: honest node-launch lifecycle status from the Bun supervisor
      // (superviseManagedNode), surfaced as a badge in the webview. Distinct
      // from the live node-state poll: this reflects whether the app launched /
      // adopted / failed to bring up the local node. No fake "online".
      readonly nodeLaunchStatus: { readonly status: NodeLaunchStatus }
    }
  }
}

// #5025: launch-lifecycle status the Bun supervisor emits. Defined here (shared,
// electrobun-free) so both the Bun launcher and the webview agree; node-launcher
// re-exports it for its existing importers.
export type NodeLaunchStatus =
  | "launching"
  | "online"
  | "adopted"
  | "failed"
  | "unavailable"

import type {
  NotificationCenterView,
  SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import type { PublicActivityTimelineEnvelope } from "@openagentsinc/public-activity-timeline"
import type { InstallReadinessResponse } from "./install-readiness.js"
import type { OnboardingStatusResponse } from "./onboarding-status.js"
import type {
  PromiseSurfacingDraft,
  PromiseSurfacingInput,
} from "./promise-surfacing.js"

// Electrobun's default RPC timeout is 1000ms, which is too short for host-side
// requests that legitimately touch the network, especially the zero-base shell
// model turn. Keep the timeout finite so a genuinely wedged bridge still
// resolves to an honest UI error.
export const DESKTOP_RPC_MAX_REQUEST_TIME_MS = 60_000

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

export type AccountWindowStatus = {
  readonly usedPercent: number
  readonly remainingPercent: number
  readonly windowMinutes: number | null
  readonly resetsAtIso: string | null
  readonly label: string
}

export type AccountStatusRow = {
  readonly provider: "codex" | "claude_agent" | string
  readonly selector: "registry_ref" | "default_home" | string
  readonly accountRef: string | null
  readonly accountRefHash: string
  readonly readiness: {
    readonly state: string
    readonly blockerRefs: readonly string[]
  }
  readonly quota: {
    readonly state: "available" | "cooldown" | "weekly_exhausted" | "limited" | string
    readonly kind: string | null
    readonly observedAt: string | null
    readonly cooldownExpiresAt: string | null
    readonly cooldownSecondsRemaining: number | null
    readonly sourceDigestRef: string | null
    readonly manualResetsRemaining: number
    readonly resetAllowed: boolean
    readonly operatorAction: "wait_for_cooldown" | "manual_recovery_available" | "none" | string
  }
  readonly capacity: {
    readonly hourly: AccountWindowStatus | null
    readonly weekly: AccountWindowStatus | null
    readonly windows: readonly AccountWindowStatus[]
  }
  readonly usage: {
    readonly observedAt: string | null
    readonly inputTokens: number | null
    readonly outputTokens: number | null
    readonly totalTokens: number | null
    readonly totalCostUsd?: number
  }
  readonly manualReset: {
    readonly performed: boolean
    readonly manualResetsRemaining: number
    readonly updatedAt: string
    readonly blockerRefs: readonly string[]
  }
  readonly blockerRefs: readonly string[]
}

export type AccountStatusResponse = {
  readonly schema?: string
  readonly observedAt?: string
  readonly accounts: readonly AccountStatusRow[]
  readonly blockerRefs?: readonly string[]
  readonly ok?: boolean
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

export type TassadarRunBulletin = {
  readonly headline?: string
  readonly latestActivity?: readonly {
    readonly label?: string
    readonly occurredAt?: string
    readonly sourceRefs?: readonly string[]
    readonly text?: string
  }[]
  readonly metrics?: {
    readonly acceptedTraceCount?: number
    readonly activePylonCount?: number
    readonly activeWindowCount?: number
    readonly realSettlementCount?: number
    readonly settledSats?: number
    readonly totalPylonCount?: number
    readonly verifiedWorkCount?: number
  }
  readonly onBoardLines?: readonly string[]
  readonly schemaVersion?: string
  readonly sourceRefs?: readonly string[]
  readonly statusLine?: string
  readonly summary?: string
  readonly title?: string
}

export type PublicTassadarRunSummary = {
  readonly bulletin?: TassadarRunBulletin
  readonly generatedAt?: string
  readonly metrics?: Readonly<Record<string, number | { readonly value?: number }>>
  readonly runLabel?: string
  readonly runRef?: string
  readonly runState?: string
  readonly sourceRefs?: readonly string[]
}

export type TrainingRunsResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  readonly runs: readonly TrainingRunProjectionRow[]
  readonly summaries: readonly TrainingRunSummaryRow[]
  readonly tassadarSummary?: PublicTassadarRunSummary | null
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

// #5485 (EPIC #5474): public-safe OpenAgents inference-gateway readiness for the
// desktop coding agent. The Bun host owns the OpenAgents API key + the gateway
// base URL and reads the user's credit balance; the webview receives only the
// server-flag state, an `apiKeyPresent` boolean, the model id, the numeric
// credit balance, and blocker refs. NEVER the raw API key. INERT-safe: `enabled`
// is false until the gateway is served server-side, so the routing decision
// keeps BYO-auth behaviour until the flag flips.
export type InferenceGatewayReadinessResponse = {
  readonly ok: boolean
  readonly fetchedAt: string
  readonly sourceUrl: string
  // Server-side flag gate: the gateway is actually served.
  readonly enabled: boolean
  // A Bun-host OpenAgents API key is configured (the raw key never crosses RPC).
  readonly apiKeyPresent: boolean
  // The chat-completions model id the gateway routes coding turns to.
  readonly model: string
  // Pay-as-you-go credit balance, or null when unknown (flag off / key missing
  // / fetch failed). The unit + ledger authority is the openagents.com Worker.
  readonly creditBalance: number | null
  // The balance at/below which the UI surfaces a low-balance hint.
  readonly lowBalanceThreshold: number
  // Public-safe readiness blocker refs (gateway disabled / key missing / etc.).
  readonly blockerRefs: readonly string[]
  readonly error?: string
}

// HUD H5 (#5503): the zero-base shell's REAL model response. The Bun host owns
// the OpenAgents agent token + the gateway base URL and calls the
// OpenAI-compatible `/v1/chat/completions` surface (Gemini 3.5 Flash on the free
// per-agent allowance); the webview receives ONLY the plain Autopilot text (or a
// clean, honest "how to configure"/error message when there is no token or the
// call fails). NEVER the raw token. `ok:false` still carries a user-facing
// `text` — it is never a fabricated model answer, just an honest plain-language
// note with no session-ref / program-step / verdict / node-state jargon.
export type ShellTurnResponse = {
  readonly ok: boolean
  readonly text: string
}

// M1 (#6009, EPIC #6017) — Lane A Cockpit. One Khala cockpit turn: the Bun host
// issues the prompt through the unified Pylon/MCP issuer by default, with the
// legacy OpenAI-compatible `/v1/chat/completions` stream available behind the
// host-side off switch. The webview receives only public-safe answer text,
// receipt projection, durable handle refs, and issuer provenance. `live` is TRUE
// only when the response carried a real receipt ref — never claim live off a
// stub/free route. The raw token never crosses here.
export type KhalaReceiptProjectionResponse = {
  readonly requestedModel: string
  readonly servedModel: string
  readonly worker: string
  readonly lane: string
  readonly verification: "none" | "test_passed" | "failed"
  readonly verified: boolean | null
  readonly receipt: string | null
  readonly receiptUrl: string | null
  readonly rubric: {
    readonly ref: string
    readonly passedChecks: readonly string[]
    readonly failedChecks: readonly string[]
  } | null
}

export type KhalaTurnResponse = {
  readonly ok: boolean
  readonly text: string
  readonly receipt: KhalaReceiptProjectionResponse | null
  readonly live: boolean
  readonly issuerPath: "legacy_gateway" | "pylon_mcp_local" | "remote_mcp"
  readonly durableRequestId: string | null
  readonly durableStreamUrl: string | null
  readonly assignmentRef: string | null
}

// #5821: the Verse chat bar talks to Tassadar/OpenAgents by default. Bun owns
// the OpenAgents token and public context fetches; the webview receives only the
// plain response, source refs, and public-safe blocker refs. No raw token, raw
// private trace, session ref, repo path, or coding-agent payload crosses here.
export type VerseTurnContextSummary = {
  readonly fetchedAt: string
  readonly sourceRefs: readonly string[]
  readonly blockerRefs: readonly string[]
  readonly pylon: {
    readonly onlineNow: number | null
    readonly assignmentReadyNow: number | null
    readonly walletReadyNow: number | null
    readonly satsSettled24h: number | null
    readonly satsSettledTotal: number | null
  }
  readonly training: {
    readonly runRef: string | null
    readonly runState: string | null
    readonly acceptedTraceCount: number | null
    readonly qualifiedContributorCount: number | null
    readonly settledPayoutSats: number | null
  }
  readonly promises: {
    readonly registryVersion: string | null
    readonly green: number | null
    readonly yellow: number | null
    readonly red: number | null
    readonly trackedTrainingPromises: number | null
  }
  readonly activity: {
    readonly eventCount: number | null
    readonly recent: readonly {
      readonly kind: string
      readonly text: string
      readonly refs: readonly string[]
    }[]
  }
}

export type VerseTurnResponse = {
  readonly ok: boolean
  readonly text: string
  readonly context: VerseTurnContextSummary
}

export type { InstallReadinessResponse } from "./install-readiness.js"
export type {
  OnboardingStatusResponse,
  OnboardingStep,
  OnboardingStepStatus,
} from "./onboarding-status.js"
export type {
  PromiseSurfacingDraft,
  PromiseSurfacingInput,
} from "./promise-surfacing.js"

// AO-3 (#5444): public-safe first-run identity-choice projection for the
// webview. No seeds, no tokens — only detection booleans + public labels.
export type IdentityChoiceKind = "use_existing" | "create_new"
export type IdentityChoiceStateResponse = {
  readonly choiceNeeded: boolean
  readonly detected: {
    readonly present: boolean
    readonly shortLabel: string | null
    readonly npub: string | null
    readonly pylonRef: string | null
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
  // #5467: the real per-intent lifecycle timeline + timestamps from the node's
  // `intent.list` projection (intent-intake `statusHistory`/`createdAt`/
  // `updatedAt`). Optional so older nodes that omit them still decode; the
  // autonomous-loop view derives the intent → plan → fanout → reconcile → ship
  // stage progression from this REAL history, not a fabricated one.
  readonly statusHistory?: ReadonlyArray<{ readonly status: string; readonly observedAt: string }>
  readonly createdAt?: string
  readonly updatedAt?: string
}

// CL-48: a pending approval/decision the node is waiting on.
export type ApprovalRow = {
  readonly approvalRef: string
  readonly kind: string
  readonly prompt: string
  readonly createdAt: string
  // VCODE-09: optional public-safe scope fields for code-mode decisions. Older
  // nodes omit these; the desktop must treat absent scope as "allow once only"
  // and never enable persistent approval.
  readonly sessionRef?: string | null
  readonly workspaceRef?: string | null
  readonly commandClass?: string | null
  readonly accountRefHash?: string | null
  readonly expiresAt?: string | null
  readonly lane?: string | null
  readonly source?: string | null
  readonly assignmentPath?: string | null
  readonly persistentApprovalSupported?: boolean
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

export type PylonFleetAssignmentState =
  | "executing"
  | "stale_unknown"
  | "marker_only"

export type PylonFleetCapacityState =
  | "verified"
  | "stale"
  | "unknown"
  | "blocked"

export type PylonFleetAssignmentRow = {
  readonly assignmentRef: string
  readonly leaseRef: string
  readonly service: "codex" | "claude" | "unknown"
  readonly state: PylonFleetAssignmentState
  readonly accountRefHash: string | null
  readonly ageSeconds: number | null
}

export type PylonFleetReconciliation = {
  readonly fetchedAt: string
  readonly pylonRefs: readonly string[]
  readonly counts: {
    readonly pylons: number
    readonly assigned: number
    readonly executing: number
    readonly stale: number
    readonly accepted: number
    readonly rejected: number
    readonly tokenFailures: number
    readonly khalaRequestWrappers: number
  }
  readonly capacity: {
    readonly state: PylonFleetCapacityState
    readonly lastHeartbeatAt: string | null
    readonly ageSeconds: number | null
    readonly availableCodexSlots: number | null
    readonly sourceRefs: readonly string[]
    readonly blockerRefs: readonly string[]
  }
  readonly assignments: readonly PylonFleetAssignmentRow[]
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
  // #7593: public-safe local Pylon/Codex assignment reconciliation. Read-only:
  // active marker counts, live child-process counts, recent closeout log
  // classification, and heartbeat freshness. Never includes raw paths,
  // prompts, command output, tokens, or secrets.
  readonly pylonFleet?: PylonFleetReconciliation
  // CL-51: node coordinator paused flag (null when the node doesn't expose it).
  readonly coordinatorPaused?: boolean | null
  // #5468 (EPIC #5461): the BOUNDED auto-approve audit trail, refs-only. Present
  // only when a session ran under `pylon sessions exec --on-approval auto`; the
  // node relays the policy's `autoApprovals[]` (approvalRef / kind / category /
  // decision / reason ref — never raw command/path/prompt text). Absent/empty
  // by default (manual approve-deny is the default and the desktop does not
  // enable auto-approve itself). The Supervise/Decisions surface shows the
  // policy bounds regardless and the trail when it exists.
  readonly autoApprovals?: readonly AutoApprovalAuditEntry[]
}

// #5468: one refs-only entry of the bounded auto-approve audit trail. Mirrors
// the Pylon exec result `autoApprovals[]` shape (apps/pylon/src/node/
// auto-approval-policy.ts → sessions-exec.ts). No raw command/path/prompt text.
export type AutoApprovalAuditEntry = {
  readonly approvalRef: string
  readonly kind: string
  readonly category: "allow" | "escalate" | "deny"
  readonly decision: string
  readonly reason: string
}

export type DesktopRPCSchema = {
  readonly bun: {
    readonly requests: {
      // Open an external URL in the system browser. The webview must NEVER
      // navigate to external URLs itself — doing so strands the user off the
      // local app (e.g. on github.com) with no way back. The webview click
      // guard routes every external anchor here instead.
      readonly openExternal: {
        readonly params: { url: string }
        readonly response: { ok: boolean }
      }
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
      // #5485: public-safe OpenAgents inference-gateway readiness (server-flag
      // state + API-key presence + credit balance) so the desktop can route
      // coding-session inference through the gateway when the user has no own
      // auth. Bun owns the API key + balance read; the webview never sees the
      // raw key. No spawn-side wire-contract change — gateway routing rides the
      // node's existing session.spawn lane selection once the flag is on.
      readonly inferenceGatewayReadiness: {
        readonly params: Record<string, never>
        readonly response: InferenceGatewayReadinessResponse
      }
      // HUD H5 (#5503): one zero-base shell turn — the Bun host sends the
      // prompt to the live OpenAgents inference gateway
      // (`POST /v1/chat/completions`, Gemini 3.5 Flash on the free per-agent
      // allowance) using the desktop's configured agent token and returns ONLY
      // the plain Autopilot text. The raw token never crosses this boundary.
      // No-token / failure returns `ok:false` with an honest plain-language
      // message (never a fabricated answer).
      readonly shellTurn: {
        readonly params: { prompt: string }
        readonly response: ShellTurnResponse
      }
      // M1 (#6009, EPIC #6017): one Khala cockpit turn. The Bun host submits the
      // prompt to the single public `openagents/khala` model on the gateway and
      // returns the plain answer PLUS the public-safe `openagents` receipt
      // projection. `live` is gated on a real receipt ref. Works against
      // local/stub or staging; the raw token never crosses this boundary. The old
      // khala-mini / khala-code split ids are deprecated (gateway returns
      // model_unavailable for them); the host normalizes any value to the single id.
      readonly khalaTurn: {
        readonly params: {
          readonly prompt: string
          readonly model?: "openagents/khala"
          // M8 streaming (#6027): correlates this turn to its live token push
          // (`webview.messages.khalaToken`). Omitted by callers that do not want
          // live tokens; the Bun host then streams server-side without pushing.
          readonly turnId?: string
        }
        readonly response: KhalaTurnResponse
      }
      // #5821: one default Verse/Tassadar turn. The Bun host builds a bounded
      // public context pack from /api/public/tassadar-run-summary,
      // /api/public/pylon-stats, /api/public/activity-timeline, and the
      // product-promise registry, then calls the same OpenAgents model gateway
      // token path as shellTurn. Missing auth/credits returns one clean Verse
      // blocker; it never spawns a Codex/Claude/session turn.
      readonly verseTurn: {
        readonly params: { prompt: string }
        readonly response: VerseTurnResponse
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
          // Desktop shell coding turns do not expose the full composer workspace
          // picker. When set, Bun may fill an omitted worktreePath with a safe
          // local git checkout from config / dev checkout discovery.
          useDefaultWorktree?: boolean
          // #5471: managed-worktree selector. Mutually exclusive with
          // `worktreePath`; when present the node's workspace-materializer
          // checks out `commitSha`. No new wire contract — `session.spawn`
          // already accepts `repoRef`.
          repoRef?: {
            provider: "github"
            visibility: "public"
            fullName: string
            branch: string
            commitSha: string
          }
          // CS-A1: run this session under a specific provider account. The
          // node resolves it against its registry and rejects an unknown ref;
          // omitted means the node's default account selection. No new wire
          // contract — `session.spawn` already accepts `accountRef` (#4868).
          accountRef?: string
        }
        readonly response: { ok: boolean; sessionRef: string; error?: string }
      }
      // #5471: resolve a managed-worktree request to a concrete repoRef. Bun
      // owns the `git ls-remote` SHA resolution; the webview only sends the
      // GitHub owner/name + base ref + stripped branch and receives a
      // public-safe repoRef it then passes to spawnSession.
      readonly resolveManagedWorktree: {
        readonly params: {
          fullName: string
          baseRef: string
          branch: string
        }
        readonly response:
          | {
              ok: true
              repoRef: {
                provider: "github"
                visibility: "public"
                fullName: string
                branch: string
                commitSha: string
              }
            }
          | { ok: false; error: string }
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
      readonly getAccountStatus: {
        readonly params: Record<string, never>
        readonly response: AccountStatusResponse
      }
      readonly resetAccountStatus: {
        readonly params: { accountRef: string }
        readonly response: AccountStatusResponse
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
      // ZERO-BASE SHELL programmatic control (owner directive, 2026-06-19): a
      // Bun→webview push that drives the dead-simple shell's text bar so an
      // operator / headless control path (autopilotctl, the Bun control surface)
      // can set the input and submit it WITHOUT a human at the keyboard. The
      // webview routes this to the SAME inbound messages the UI dispatches
      // (ChangedShellInput / SubmittedShell), so a driver sees exactly the state
      // the owner sees. Public-safe: it carries only the literal text the owner
      // could type. `set-input` replaces the bar; `submit` sends the current bar.
      readonly shellControl: {
        readonly action: "set-input" | "submit"
        readonly value?: string
      }
      // M8 streaming (#6027, EPIC #6017): Bun→webview live token push for a Khala
      // cockpit turn. The `khalaTurn` RPC submits `stream:true` and consumes the
      // SSE stream server-side (each chunk resets the Cloudflare edge idle timer —
      // the 524 fix), pushing each content delta here so the cockpit renders
      // tokens live. The terminal `openagents` receipt block is NOT pushed here;
      // it rides on the `khalaTurn` RPC response, attached on stream close.
      // `turnId` correlates a delta stream to its turn so concurrent/!stale turns
      // don't cross-render. Public-safe: token text only, no token/credentials.
      readonly khalaToken: {
        readonly turnId: string
        readonly delta: string
      }
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

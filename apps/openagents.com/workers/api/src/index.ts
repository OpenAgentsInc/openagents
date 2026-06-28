import { Container, getContainer } from '@cloudflare/containers'
import {
  type DurableObjectStateLike as DurableStreamObjectStateLike,
  handleDurableStreamAlarm,
  handleDurableStreamFetch,
} from '@openagentsinc/durable-stream'
import {
  type WorkerBindings,
  badRequest,
  cursorGap,
  jsonResponse,
  makeD1SyncOutboxRepository,
  notFound,
} from '@openagentsinc/sync-worker'
import { issuer } from '@openauthjs/openauth'
import { type Tokens, createClient } from '@openauthjs/openauth/client'
import {
  CodeProvider,
  type CodeProviderError,
} from '@openauthjs/openauth/provider/code'
import { GithubProvider } from '@openauthjs/openauth/provider/github'
import { createSubjects } from '@openauthjs/openauth/subject'
import { CodeUI } from '@openauthjs/openauth/ui/code'
import { Cause, Effect, Layer, Option, Redacted, Schema as S } from 'effect'
import { Exit } from 'effect'
import { WorkerEnvironment } from 'effect-cf'

import { handleAcceptedOutcomesPerKwhApi } from './accepted-outcomes-per-kwh-routes'
import { AdjutantEnrichmentQueueMessage } from './adjutant-enrichment-jobs'
import type { AdjutantTaskPacketRefValidationInput } from './adjutant-task-packets'
import { recordAdjutantUsageReceipt } from './adjutant-usage-receipts'
import { makeAdminOverviewHandlers } from './admin-overview-routes'
import {
  handleAgentBalanceApi,
  handleAgentBalancePreferencesApi,
} from './agent-balance-routes'
import { makeAgentGoalRoutes } from './agent-goal-routes'
import {
  handleProgrammaticAgentHome,
  handleProgrammaticAgentSelfUpdate,
} from './agent-home-routes'
import {
  makeAgentOwnerClaimRoutes,
  makeD1AgentOwnerClaimStore,
} from './agent-owner-claim-routes'
import {
  makeAgentProposalRoutes,
  makeD1AgentProposalStore,
} from './agent-proposal-routes'
import { withAgentRateLimitHeaders } from './agent-rate-limit-policy'
import { makeD1AgentRateLimitRecoveryStore } from './agent-rate-limit-recovery'
import {
  type AgentRegistrationStore,
  type AgentReissueStore,
  ProgrammaticAgentRegistrationRequest,
  type ProgrammaticAgentSession,
  ReissueAgentTokenRequest,
  authenticateProgrammaticAgent,
  createProgrammaticAgentRegistration,
  makeD1AgentRegistrationStore,
  reissueProgrammaticAgentToken,
  sha256Hex,
  timingSafeEqual,
} from './agent-registration'
import {
  makeAgentScopedGrantRoutes,
  makeD1AgentScopedGrantStore,
} from './agent-scoped-grant-routes'
import { makeAgentSearchRoutes } from './agent-search-routes'
import { makeAgentSiteRoutes } from './agent-site-routes'
import {
  AgenticLaborProductEndpoint,
  handleAgenticLaborProductApi,
  isAgenticLaborProductsEnabled,
} from './agentic-labor-product-routes'
import { makeD1ArtanisAdminCloseoutReceiptStore } from './artanis-admin-closeout-receipts'
import {
  runArtanisAdminTickScheduled,
  runArtanisCloseoutVerifierScheduled,
} from './artanis-administrator-tick'
import {
  boundedDistillationDatasetLimit,
  readArtanisDistillationDatasetReceipt,
} from './artanis-distillation-dataset-receipt'
import { deliverArtanisForumPublicationIntent } from './artanis-forum-delivery'
import { ArtanisForumPublicationIntentRecord } from './artanis-forum-publication'
import { exampleArtanisForumPublicationQueue } from './artanis-forum-publication'
import {
  ARTANIS_REGISTERED_ACTOR_REF,
  ARTANIS_RESPONDER_DEMAND_CLIENT,
  ARTANIS_RESPONDER_DEMAND_SOURCE,
  type ArtanisResponderKhalaClient,
  runArtanisResponderScanScheduled,
} from './artanis-forum-responder'
import {
  handlePublicArtanisLaborGreenReadinessApi,
  handlePublicArtanisLaborReceiptsApi,
} from './artanis-labor-receipt-routes'
import { makeD1ArtanisLaborUnattendedReceiptStore } from './artanis-labor-receipt-store'
import { ArtanisMindSmokeSystem, artanisMindComplete } from './artanis-mind'
import { makeOperatorArtanisChatRoutes } from './artanis-operator-chat-routes'
import { loadArtanisNetworkStatsFromLedger } from './artanis-network-stats-d1'
import { makeOperatorArtanisConsoleRoutes } from './artanis-operator-console-routes'
import {
  makeArtanisDispatchExecution,
  readEffectiveArtanisPylonDispatchApprovalForOwner,
} from './artanis-operator-dispatch-execution'
import { isOpenAgentsOwnerAgentOpenAuthUserId } from './artanis-owner-authority'
import {
  makeArtanisPylonAssignmentsLister,
  makeArtanisPylonJobStatusReader,
} from './artanis-operator-pylon-job-status'
import { makeArtanisGlmFleetStatusLoader } from './artanis-operator-glm-fleet-status'
import { makeArtanisKhalaFeedbackReader } from './artanis-operator-khala-feedback'
import { makeArtanisTraceReviewLoader } from './artanis-operator-trace-review'
import { makeArtanisOperatorTools } from './artanis-operator-tools'
import {
  makeArtanisUnsupportedRequestsReader,
  makeArtanisUnsupportedRequestWriter,
} from './artanis-operator-unsupported-requests'
import { saveArtanisForumPublicationIntent } from './artanis-persistence'
import { handlePublicArtanisReportApi } from './artanis-public-report-routes'
import { runArtanisComposerScheduled } from './artanis-reply-composer'
import {
  boundedResponderSupportLimit,
  readArtanisResponderSupport,
} from './artanis-responder-provenance'
import { runArtanisScheduledTickForWorker } from './artanis-scheduled-runner'
import { runArtanisSpendDecision } from './artanis-spend'
import {
  boundedTickMonitorLimit,
  readArtanisTickMonitor,
} from './artanis-tick-monitor'
import {
  boundedTickStreakLimit,
  readArtanisTickStreak,
} from './artanis-tick-streak'
import {
  ACCESS_COOKIE,
  AUTH_STATE_COOKIE,
  AUTH_STATE_MAX_AGE_SECONDS,
  REFRESH_COOKIE,
  SESSION_MAX_AGE_SECONDS,
  appendClearSessionCookies,
  appendSessionCookies,
  expiredCookie,
  parseCookies,
  serializeCookie,
} from './auth-cookies'
import {
  AUTH_EMAIL_OTP_CODE_TTL_SECONDS,
  type AuthEmailOtpRateLimitRejected,
  authEmailOtpClaimsAreFresh,
  authEmailOtpClientIp,
  authEmailOtpSendForm,
  normalizeAuthEmailOtpEmail,
  reserveAuthEmailOtpSend,
  stampAuthEmailOtpClaims,
} from './auth/email-otp-hardening'
import { makeD1Storage } from './auth/openauth-storage'
import {
  type VerifiedSession as VerifiedAuthSession,
  makeBrowserSessionBoundary,
} from './auth/session'
import {
  AutopilotComposedRunEndpoint,
  handleAutopilotComposedRunApi,
  isAutopilotComposedRunEnabled,
} from './autopilot-composed-run-routes'
import {
  listAutopilotContinuationRunCandidates,
  makeD1AutopilotContinuationStore,
  runAutopilotContinuationSweep,
} from './autopilot-continuation-policy'
import { makeAutopilotContinuationPolicyRoutes } from './autopilot-continuation-policy-routes'
import { makeAutopilotDecisionRoutes } from './autopilot-decision-routes'
import { makeHostedGeminiExecuteReadyWork } from './autopilot-hosted-gemini-executor-env'
import { makeAutopilotMorningReportRoutes } from './autopilot-morning-report-routes'
import {
  type OnboardingInferenceClient,
  OnboardingInferenceError,
  type OnboardingStreamClient,
  type OnboardingStreamSource,
} from './autopilot-onboarding-program'
import { makeAutopilotOnboardingRoutes } from './autopilot-onboarding-routes'
import {
  type AutopilotWorkOrderRecord,
  dispatchDueScheduledAutopilotWork,
  makeAutopilotWorkRoutes,
  makeD1AutopilotWorkStore,
  recordAutopilotWorkerCloseoutFromPylon,
  verifyAutopilotL402PaymentProofFromBuyerLedger,
} from './autopilot-work-routes'
import {
  type BillingSummary,
  markOutOfCreditsNotificationFailed,
  markOutOfCreditsNotificationSent,
  readBillingSummary,
  recordContainerUsageDebitForRun,
  requireMinimumRunCredits,
  reserveOutOfCreditsNotification,
  suspendBillingAccountIfOutOfCredits,
  withBillingCreditPackages,
} from './billing'
import { makeBillingApiHandlers } from './billing-routes'
import { OpenAgentsDatabase, ThreadFileArtifacts } from './bindings'
import { makeBlueprintProbeContributionRoutes } from './blueprint-probe-contribution-routes'
import { makeBlueprintRoutes } from './blueprint-routes'
import {
  listBlueprintActionSubmissions,
  recordBlueprintActionSubmissionProposal,
} from './blueprint/repositories/action-submissions'
import {
  listBlueprintProbeContributions,
  recordBlueprintProbeContribution,
} from './blueprint/repositories/probe-contributions'
import {
  listBlueprintProgramRuns,
  recordBlueprintProgramRun,
} from './blueprint/repositories/program-runs'
import { handleBusinessSignupApi } from './business-signup-routes'
import { makeD1BuyModeDispatcherStore } from './buy-mode-dispatcher'
import { buyModePaymentBridgeForEnv } from './buy-mode-http-payment-bridge'
import { buyModeEvalBridgeForEnv } from './buy-mode-live-eval-bridge'
import { buyModeRelayPublisherForEnv } from './buy-mode-live-publisher'
import { makeD1BuyerPaymentLedgerStore } from './buyer-payment-ledger'
import { makeCfBrowserSmokeHandler } from './cf-browser-smoke-routes'
import { makeCheckoutPageRoutes } from './checkout-page-routes'
// Cloud coding-session surface (autopilot.cloud_coding_sessions.v1, red) — the
// "our cloud" autonomous-execution lane. INERT behind CLOUD_CODING_SESSIONS_ENABLED
// (default off). Ships wired to the stub/accepting runtime adapter + no-op
// metering stub; the managed GCE control-plane adapter + live receipt-first
// metering plug into its seams. The promise STAYS red — no real VM, no real
// repo-edit, no paid receipt; no green flip lands here.
import {
  isCloudCodingSessionsEnabled,
  routeCloudCodingSessionRequest as routeCloudCodingSessionRequestImpl,
} from './cloud/cloud-coding-session-routes'
import { makeD1CloudPrimitiveReceiptStore } from './cloud/cloud-primitive-receipts'
// Cloud primitive SCAFFOLDS (EPIC #5510). Both flag-gated INERT by default; the
// promises `cloud.fine_tuning_service.v1` / `cloud.sandbox_compute_service.v1`
// STAY red until a dereferenceable paid receipt lands. No green flip here.
import {
  handleFineTuningJobSubmit,
  isFineTuningServiceEnabled,
} from './cloud/fine-tuning-service-routes'
import { makePublicCloudPrimitiveReceiptRoutes } from './cloud/public-cloud-primitive-receipt-routes'
import {
  handleSandboxRequest,
  isSandboxComputeServiceEnabled,
} from './cloud/sandbox-compute-service-routes'
import {
  CodingQuickWinPipelineEndpoint,
  handleCodingQuickWinPipelineApi,
} from './coding-quick-win-pipeline-routes'
import {
  type OpenAgentsWorkerConfigEnv,
  getOpenAgentsWorkerConfig,
  redactedValue,
} from './config'
import { makeCrmBatchRoutes } from './crm-batch-routes'
import { makeCrmCommandRoutes } from './crm-command-routes'
import { makeCrmEmailRoutes } from './crm-email-routes'
import { makeCrmImportRoutes } from './crm-import-routes'
import { makeCrmMcpCatalog } from './crm-mcp'
import { makeCrmMcpDiscoveryRoutes } from './crm-mcp-discovery-routes'
import {
  crmMcpAdminPrincipal,
  mcpTenantHeader,
  readMcpBearerToken,
  resolveCrmMcpGrantPrincipal,
} from './crm-mcp-grant'
import { makeCrmMcpGrantRoutes } from './crm-mcp-grant-routes'
import { makeCrmMcpRoutes } from './crm-mcp-routes'
import { isCrmResendSendEnabled, makeCrmResendSender } from './crm-resend'
import { makeCrmResendRoutes } from './crm-resend-routes'
import { makeCrmRoutes } from './crm-routes'
import { makeCrmSendRoutes } from './crm-send-routes'
import { CustomerOneCohortEndpoint } from './customer-one-cohort-projection'
import {
  handleOperatorCustomerOneCohortRowsApi,
  handlePublicCustomerOneCohortApi,
} from './customer-one-cohort-routes'
import { makeD1CustomerOneCohortRowStore } from './customer-one-cohort-store'
import { handleDemandProvenanceApi } from './demand-provenance-routes'
import { makeInMemoryEcommerceCampaignPaidDeliveryClaimStore } from './ecommerce-campaign-claim-upgrade'
import { firstPaidEcommerceCampaignDeliveryReceiptFixture } from './ecommerce-campaign-delivery-receipt-fixture'
import { makeEcommerceCampaignReceiptOperatorRoutes } from './ecommerce-campaign-receipt-operator-routes'
import { makeEcommerceCampaignReceiptRoutes } from './ecommerce-campaign-receipt-routes'
import { makeD1EcommerceCampaignReceiptStore } from './ecommerce-campaign-receipt-store'
import { makeEcommerceCampaignSelfServeRoutes } from './ecommerce-campaign-self-serve-routes'
import {
  AutopilotDecisionEmailInput,
  OrderSitesTransactionalEmailInput,
  buildOrderSitesTransactionalEmailIdempotencyKey,
  sendAutopilotDecisionEmailWithLedger,
  sendOrderSitesTransactionalEmailWithLedger,
  sendOutOfCreditsEmailWithLedger,
  sendPrivateWorkspaceInviteEmailWithLedger,
} from './email'
import {
  type EmailCampaignDispatcherResult,
  dispatchDueEmailCampaignSends,
} from './email-campaign-dispatcher'
import type { OnboardingDripOrderState } from './email-onboarding-drip'
import { makeEmailSequenceAuthoringRoutes } from './email-sequence-authoring-routes'
import { makeFirmupBitcoinSettlementRoutes } from './firmup-bitcoin-settlement-routes'
import { readFirmupSettleableEscrow } from './firmup-settleable-escrow'
import { makeForumRoutes } from './forum-routes'
import { forumWorkRequestRelayPublisherForEnv } from './forum-work-request-live-publisher'
import { archiveStaleDirectTipRecoveries } from './forum/paid-actions'
import { readForumTipRecipientReadinessForActor } from './forum/repository'
import {
  GITHUB_WRITE_REQUIRED_SCOPES,
  GitHubWriteApiFailure,
  type GitHubWriteConnectionAttemptRecord,
  type GitHubWriteConnectionBundle,
  GitHubWriteTokenStorageFailure,
  gitHubWriteConnectionMetadataJson,
  githubWriteConnectionRef,
  githubWriteSecretKey,
  githubWriteSecretRef,
  listGitHubWriteConnectionsForUser,
  makeD1GitHubWriteRepository,
  recordGitHubWriteConnectionConnected,
  requireGitHubWriteCallbackAccount,
  requireGitHubWritePermissions,
  resolveGitHubWriteGrant,
  startGitHubWriteConnectionAttempt,
} from './github-write-connections'
import {
  gitHubWriteRouteErrorMessage,
  gitHubWriteRouteErrorName,
  gitHubWriteRouteErrorStatus,
} from './github-write-route-errors'
import {
  makeMissingOpenAgentsHostedMdkClient,
  makeOpenAgentsHostedMdkRouteClient,
} from './hosted-mdk-client'
import type { ContainerPathFetch } from './http/container-fetch'
import { handleForumThreadDocument } from './http/forum-social-preview'
import {
  type DurableMdkPaymentOutcome,
  durableMdkPaymentOutcomeResponse,
  journalMdkResponseOutcome,
  mdkPaymentIdFromStatusPath,
  mdkPaymentOutcomeStorageKey,
  mdkTerminalOutcomeFromPayload,
} from './http/mdk-payment-outcome-journal'
import { fetchAppShellWithPylonStatsBootPayload } from './http/pylon-stats-boot-payload'
import {
  forbidden,
  methodNotAllowed,
  noStoreJsonResponse,
  redirectResponse,
  serverError,
  unauthorized,
} from './http/responses'
import { routeAccessResponse } from './http/route-access-response'
import { routeEffect, routeEffectOrResponse } from './http/route-effects'
import { makeD1HygieneDebtReceiptStore } from './hygiene-debt-receipt-store'
import { makeHygieneLaneSettlementRoutes } from './hygiene-lane-settlement-routes'
import { makeImageGenerationRoutes } from './image-generation-routes'
import { makeD1InferenceReceiptStore } from './inference-receipts'
import {
  isAcceptanceDispatchEnabled,
  makeD1KhalaVerificationStore,
} from './inference/acceptance-dispatch'
import {
  handleAcceptanceJobAck,
  handleAcceptanceJobLease,
} from './inference/acceptance-job-lease-routes'
import { makeD1AcceptanceJobQueueStore } from './inference/acceptance-job-queue-store'
import {
  type AcceptedOutcomeSettlementSink,
  handleAcceptanceVerdictCallback,
} from './inference/acceptance-verdict-callback-routes'
import {
  BatchJobQueueMessage,
  executeBatchJob,
} from './inference/batch-job-consumer'
import {
  handleBatchJobReceiptRead,
  handleBatchJobStatusRead,
  handleBatchJobsSubmit,
} from './inference/batch-job-routes'
import { makeD1BatchJobStore } from './inference/batch-job-store'
import { makeD1CardCreditSpendReceiptStore } from './inference/card-credit-spend-receipt-store'
import {
  handleChatCompletions,
  isInferenceDurableStreamEnabled,
  isInferenceGatewayEnabled,
  khalaRequestForAdapter,
} from './inference/chat-completions-routes'
import { handleDispatchFailureTelemetryReadout } from './inference/dispatch-failure-telemetry-routes'
import {
  type DiscoverySurfacePath,
  renderDiscoverySurface,
} from './inference/discovery-surfaces'
import { type DurableStreamNamespace } from './inference/durable-inference-do-transport'
import {
  matchDurableReadRequest,
  routeDurableInferenceReadRequest,
  routeDurableInferenceReadRequestDO,
} from './inference/durable-inference-read-routes'
import {
  KHALA_FIREWORKS_BACKING_MODEL_ID,
  fireworksAdapter,
} from './inference/fireworks-adapter'
import { runFleetBurnStallDetectorScheduled } from './inference/fleet-burn-stall-detector'
import { freeTierDataSharingDisclosure } from './inference/free-tier-data-sharing-disclosure'
import { handleFreeTierDataSharingDisclosureApi } from './inference/free-tier-data-sharing-routes'
import { handleGatewayReadiness } from './inference/gateway-readiness-routes'
import { handleGlmFleetReadiness } from './inference/glm-fleet-readiness-routes'
import {
  glmPoolHeartbeatRoutingStateOracle,
  runScheduledGlmPoolHeartbeatForD1,
} from './inference/glm-pool-heartbeat'
import { handleOperatorHarborFullTraceArchivesApi } from './inference/gym/harbor-full-trace-archive-routes'
import { makeD1R2HarborFullTraceArchiveStore } from './inference/gym/harbor-full-trace-archive-store'
import {
  handleOperatorGymRunProgressApi,
  handlePublicGymRunProgressApi,
} from './inference/gym/run-progress-routes'
import { makeD1GymRunProgressStore } from './inference/gym/run-progress-store'
import { publishGymRunProgressSnapshot } from './inference/gym/run-progress-sync'
import {
  handleOperatorGymLeaderboardApi,
  handlePublicGymLeaderboardApi,
} from './inference/gym/ladder-routes'
import { makeD1GymLadderStore } from './inference/gym/ladder-store'
import {
  handleMirrorCodeRunByIdApi,
  handleMirrorCodeRunsApi,
  matchMirrorCodeRunByIdRequest,
} from './inference/gym/mirrorcode-routes'
import { makeD1MirrorCodeRunStore } from './inference/gym/mirrorcode-store'
import {
  handleOperatorKhalaHeadToHeadApi,
  handlePublicKhalaHeadToHeadApi,
} from './inference/benchmark/head-to-head-routes'
import { makeD1KhalaHeadToHeadStore } from './inference/benchmark/head-to-head-store'
import {
  type HydraliskPoolRouteAdmissionSnapshot,
  makeHydraliskVllmAdapter,
  makeHydraliskVllmPoolRuntime,
} from './inference/hydralisk-adapter'
import {
  checkFreeAllowancePreflight,
  withFreeAllowance,
} from './inference/inference-free-allowance'
import {
  decideFreeKeyMint,
  isFreeTierEnabled,
  makeFreeTierGate,
  markAccountFreeTierAsync,
  readAccountFreeTier,
  readFreeKeyMintsToday,
  recordFreeKeyMintAsync,
  resolveFreeKeyMintCap,
  resolveFreeTierQuota,
  sanitizeFreeKeyLabel,
  withFreeTierKhala,
} from './inference/inference-free-tier-key'
import { parseInternalAccountRefs } from './inference/inference-internal-account'
import {
  isOperatorExemptionEnabled,
  makeOperatorExemptionGate,
  withOperatorCredit,
} from './inference/inference-operator-exemption'
import { makeVerifiedOwnerIdentityResolver } from './inference/inference-owner-identity'
import { makePremiumAccessGate } from './inference/inference-premium-allowlist'
import {
  isConfidentialComputeEnabled,
  makePaidPrivacyResolver,
} from './inference/inference-privacy-entitlement'
import { withReferralAccrual } from './inference/inference-referral-accrual'
import { makeInferenceReferralRoutes } from './inference/inference-referral-routes'
import {
  settleVerifiedAcceptedOutcome,
  summarizeAcceptedOutcomeSettlement,
} from './inference/khala-accepted-outcome-settlement'
import {
  emitKhalaChatTrace,
  isKhalaChatTraceEmitEnabled,
  isKhalaFreeTierTraceCaptureDefaultEnabled,
} from './inference/khala-chat-trace-emitter'
import { isComponentChannelEnabled } from './inference/khala-component-channel'
import {
  type KhalaSettlementDispatch,
  makeDryRunSettlementDispatch,
  makeKhalaLoopSettlementDispatch,
  readKhalaLoopArming,
} from './inference/khala-loop-integration'
import {
  buildKhalaTokensServedDelta,
  publishKhalaTokensServedDelta,
} from './inference/khala-tokens-served-sync'
import {
  type InternalStressSchedulerNamespace,
  makeInternalStressPreemptionCoordinatorDO,
  makeInternalStressPreemptionRegistry,
} from './inference/internal-stress-preemption'
export { GlmStressSchedulerDurableObject } from './inference/internal-stress-preemption-do'
import { makeLedgerMeteringHook } from './inference/metering-hook'
import {
  FIREWORKS_ADAPTER_ID,
  FIREWORKS_STRONG_CODING_ADAPTER_ID,
  HYDRALISK_ADAPTER_ID,
  HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
  HYDRALISK_GPT_OSS_120B_ADAPTER_ID,
  OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
  dispatchWithOverflow,
  dispatchWithOverflowWithMetadata,
  makeBoundedDispatchFailureTelemetry,
  makeKhalaBackedAdapterPlan,
} from './inference/model-router'
import {
  resolveHydraliskGlm52Reap504bArming,
  resolveSupplyLaneArming,
} from './inference/model-serving-policy'
import {
  handleModelsList,
  routeModelRetrieveRequest,
} from './inference/models-routes'
import { renderMppDiscoveryDocument } from './inference/mpp-discovery-document'
import {
  handleMppChatCompletions,
  isKhalaMppEnabled,
  isKhalaMppLightningEnabled,
} from './inference/mpp/mpp-chat-completions-routes'
import { makeFallbackLightningInvoiceIssuer } from './inference/mpp/mpp-lightning-invoice'
import {
  MDK_LIGHTNING_FALLBACK_MINT_TIMEOUT_MS,
  makeMdkLightningInvoiceIssuer,
  normalizeMdkLightningRouteUrl,
} from './inference/mpp/mpp-lightning-invoice-mdk'
import { makeSparkLightningInvoiceIssuer } from './inference/mpp/mpp-lightning-invoice-spark'
import { dispatchOnboardingStreamSource } from './inference/onboarding-stream-source'
import { makeAdmittedOpenAgentsNetworkAdapter } from './inference/openagents-network-adapter'
import {
  OPENROUTER_DEFAULT_BASE_URL,
  OPENROUTER_KHALA_FALLBACK_MODEL_ID,
  makeOpenRouterAdapter,
} from './inference/openrouter-adapter'
import {
  type PassthroughAdapterConfig,
  makePassthroughAdapter,
} from './inference/passthrough-adapter'
import {
  HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
  HYDRALISK_GPT_OSS_20B_MODEL_ID,
  HYDRALISK_GPT_OSS_120B_MODEL_ID,
  KHALA_MODEL_ID,
  normalizeKhalaModelId,
} from './inference/pricing'
import {
  InferenceAdapterError,
  type InferenceProviderAdapter,
  InferenceProviderRegistry,
  type InferenceRequest,
  type InferenceResult,
} from './inference/provider-adapter'
import { dispatchPsionicServe } from './inference/psionic-fabric-serve'
import {
  makePylonFabricHttpTransport,
  pylonFabricHttpTransportConfigFromEnv,
  pylonGatewayAdmissionFromEnv,
} from './inference/pylon-fabric-http-transport'
import { handlePylonFabricSmoke } from './inference/pylon-fabric-smoke-routes'
import { handleQuote } from './inference/quote-routes'
import {
  type ServedTokensRecorderInput,
  buildServedTokensIngestBody,
  makeD1ServedTokensRecorder,
  meterServedTokensFailSoft,
  servedTokensRowIsPublicCountable,
} from './inference/served-tokens-recorder'
import { stubEchoAdapter } from './inference/stub-echo-adapter'
import {
  VERTEX_ANTHROPIC_ADAPTER_ID,
  makeVertexAnthropicAdapter,
} from './inference/vertex-anthropic-adapter'
import {
  DEFAULT_GEMINI_MODEL_ID,
  VERTEX_GEMINI_ADAPTER_ID,
  makeVertexGeminiAdapter,
} from './inference/vertex-gemini-adapter'
import { tokenProviderFromSecret } from './inference/vertex-token'
import {
  decodeUnknownWithSchema,
  isRecord,
  nestedUnknown,
  optionalInteger,
  optionalNestedString,
  optionalString,
  readJsonObject,
  safeJsonRecord,
  stringArrayFromUnknown,
} from './json-boundary'
import type { KhalaChatStreamClient } from './khala-chat-program'
import { makeKhalaChatRoutes } from './khala-chat-routes'
import {
  handleKhalaFeedbackSubmit,
  handleOperatorKhalaFeedback,
  makeD1KhalaFeedbackStore,
} from './khala-feedback-routes'
import {
  handleOperatorKhalaTraceReview,
  makeD1KhalaTraceReviewStore,
} from './khala-trace-review-routes'
import {
  handleOperatorKhalaUnsupportedRequests,
  makeD1KhalaUnsupportedRequestStore,
} from './khala-unsupported-request-routes'
import {
  combineMcpCatalogs,
  khalaDurableRequestIsLinkedToPrincipal,
  khalaMcpAgentPrincipal,
  makeKhalaMcpCatalog,
} from './khala-mcp'
import { makeOpenAgentsL402HmacSigningBoundary } from './l402-credential-service'
import { handlePublicLaborEarningsApi } from './labor-earnings-routes'
import { handleSelfServeLaborPayoutApi } from './labor-self-serve-earning-payout-routes'
import { makeInMemoryMarketingAgencyPaidDeliveryClaimStore } from './marketing-agency-claim-upgrade'
import { makeMarketingAgencyReceiptPublicRoutes } from './marketing-agency-receipt-public-routes'
import { makeInMemoryMarketingAgencySelfServeClaimStore } from './marketing-agency-self-serve-claim-upgrade'
import { makeMarketingAgencySelfServePublicRoutes } from './marketing-agency-self-serve-public-routes'
import {
  MarketplaceComposeListEndpoint,
  handleMarketplaceCompositionApi,
  isMarketplaceComposeAndListEnabled,
} from './marketplace-composition-routes'
import {
  MarketplaceWorkClassCatalogEndpoint,
  handleMarketplaceWorkClassCatalogApi,
} from './marketplace-work-class-catalog-routes'
import {
  mdkContainerEnvVars,
  optionalMdkContainerSecret,
} from './mdk-container-env'
import { hostedMdkDirectPayoutDisabledGate } from './mdk-payout-mode-gate'
import {
  MobileWorkroomApprovalProjectionEndpoint,
  handleMobileWorkroomApprovalProjectionApi,
  isMobileWorkroomApprovalProjectionEnabled,
} from './mobile-workroom-approval-projection-routes'
import { makeMulletRoutes } from './mullet/routes'
import { makeNativeListsService } from './native-lists'
import { makeNativeListsRoutes } from './native-lists-routes'
import { makeNexusPylonVisibilityRoutes } from './nexus-pylon-visibility-routes'
import { makeD1NexusTreasuryPayoutLedgerStore } from './nexus-treasury-payout-ledger'
import { makeD1Nip90MarketReceiptStore } from './nip90-market-receipts'
import {
  logWorkerRouteError,
  logWorkerRouteInfo,
  logWorkerRouteWarning,
  observedEffect,
  observedPromise,
} from './observability'
import { handleOmniApiSdkSeedApi } from './omni-api-sdk-seed-routes'
import { makeOmniBundleRoutes } from './omni-bundle-routes'
import {
  OmniClientDeliveryProjectionEndpoint,
  handleOmniClientDeliveryProjectionApi,
  isOmniClientDeliveryProjectionEnabled,
} from './omni-client-delivery-projection-routes'
import { handleOmniContributorAccrualBundleApi } from './omni-contributor-accrual-bundle-routes'
import { readOmniEvidenceBundleById } from './omni-evidence-bundles'
import { makeOmniHandlers } from './omni-handlers'
import { makeOmniHandoffRoutes } from './omni-handoff-routes'
import { readOmniPublicProofBundleById } from './omni-public-proof-bundles'
import { makeOmniRoutes } from './omni-routes'
import {
  type AgentRunBundle,
  type AgentRunRecord,
  type OmniEventRecord,
  cancelActiveAgentRunsForBillingExhaustion,
  cancelAgentRunOnShc,
  listActiveAgentRunsForBilling,
  makeD1OmniRunStore,
} from './omni-runs'
import { makeOmniWorkroomLifecycleRoutes } from './omni-workroom-lifecycle-routes'
import { makeOmniWorkroomRoutes } from './omni-workroom-routes'
import { githubIdentityTokenKey } from './onboarding/github'
import { readOnboardingStatusForUser } from './onboarding/repository'
import { makeOnboardingRoutes } from './onboarding/routes'
import {
  handleLiquidityMarketSkeletonApi,
  handleOpenMarketsSurfaceApi,
  handleRiskMarketSkeletonApi,
} from './open-markets-routes'
import {
  handleOpenAgentsAgentOnboarding,
  handleOpenAgentsCompanionFile,
} from './openagents-agent-onboarding-routes'
import { handleOpenAgentsCapabilityManifestApi } from './openagents-capability-manifest-routes'
import { handleOpenAgentsOpenApi } from './openagents-openapi-routes'
import {
  executeQueuedAdjutantEnrichmentJob,
  makeOperatorAdjutantRoutes,
} from './operator-adjutant-routes'
import { makeOperatorBillingHandlers } from './operator-billing-routes'
import { makeOperatorBuyModeRoutes } from './operator-buy-mode-routes'
import { makeOperatorEmailInspectionRoutes } from './operator-email-inspection-routes'
import { makeOperatorOrderTriageRoutes } from './operator-order-triage-routes'
import { makeOperatorProviderAccountRoutes } from './operator-provider-account-routes'
import { makeOperatorPylonMarketplaceRoutes } from './operator-pylon-marketplace-routes'
import { makeOperatorSitesRoutes } from './operator-sites-routes'
import {
  type OperatorTargetUser,
  readOperatorTargetUser,
  readSelectedInferenceCreditTargetUser as readSelectedInferenceCreditTargetUserBase,
} from './operator-targets'
import { makePartnerAgreementRoutes } from './partner-agreement-routes'
import { PartnerPayoutDispatchError } from './partner-payout-dispatch'
import { makePartnerPayoutLedgerRoutes } from './partner-payout-ledger-routes'
import { handlePartnerPayoutsPublicApi } from './partner-payout-public-routes'
import { makeD1PartnerPayoutReceiptStore } from './partner-payout-receipts'
import { readAgentBalance } from './payments-ledger'
import { makePrefilledWorkspaceService } from './prefilled-workspace'
import { makePrefilledWorkspaceRoutes } from './prefilled-workspace-routes'
import {
  makeD1PrivateProjectWorkspaceStore,
  makePrivateProjectWorkspaceRoutes,
} from './private-project-workspace-routes'
import { publicProductPromisesDocument } from './product-promises'
import { handlePublicPromiseAuditApi } from './promise-transition-audit-routes'
import {
  handleOperatorPromiseTransitionApi,
  handlePublicPromiseTransitionsApi,
  lastVerifiedAtByPromise,
  makeD1PromiseTransitionReceiptStore,
} from './promise-transition-receipt-routes'
import { probeProviderApiKey } from './provider-account-api-key'
import { makeProviderAccountBrowserHandlers } from './provider-account-browser-routes'
import { makeProviderAccountPoolRoutes } from './provider-account-pool-routes'
import { makeProviderAccountPylonHandlers } from './provider-account-pylon-routes'
import { makeProviderAccountRoutes } from './provider-account-routes'
import { makeProviderAccountServiceHandlers } from './provider-account-service-routes'
import { makeProviderAccountUsageRoutes } from './provider-account-usage-routes'
import {
  type CodexOAuthAuth,
  type ProviderAccountBundle,
  listProviderAccountsForUser,
  makeD1ProviderAccountRepository,
} from './provider-accounts'
import {
  handlePublicActivityTimelineApiForEnv,
  handlePublicActivityTimelineStreamApiForEnv,
} from './public-activity-timeline-routes'
import { handlePublicAdjutantActivityApi } from './public-adjutant-activity-routes'
import { makePublicCardCreditSpendReceiptRoutes } from './public-card-credit-spend-receipt-routes'
import { handlePublicForumActivityApiForEnv } from './public-forum-activity-routes'
import { makePublicInferenceReceiptRoutes } from './public-inference-receipt-routes'
import { handlePublicKhalaTokensServedHistoryApi } from './public-khala-tokens-served-history-routes'
import { handlePublicKhalaTokensServedModelMixApi } from './public-khala-tokens-served-model-mix-routes'
import { handlePublicKhalaTokensServedApi } from './public-khala-tokens-served-routes'
import { recordPublicKhalaChatServedTokens } from './public-khala-chat-served-tokens'
import { handlePublicLaunchDashboardApi } from './public-launch-dashboard-routes'
import { makePublicNip90MarketReceiptRoutes } from './public-nip90-market-receipt-routes'
import { handlePublicOtecProofApi } from './public-otec-proof-routes'
import { makePublicPartnerPayoutReceiptRoutes } from './public-partner-payout-receipt-routes'
import { handlePublicProofReplayBundleRequest } from './public-proof-replay-routes'
import { handlePublicPylonStatsApi } from './public-pylon-stats-routes'
import { makePublicSiteReferralPayoutReceiptRoutes } from './public-site-referral-payout-receipt-routes'
import { makePublicStripeCheckoutReceiptRoutes } from './public-stripe-checkout-receipt-routes'
import { buildPublicTassadarRunSummaryEnvelopeForRequest } from './public-tassadar-run-summary-routes'
import {
  makeD1PylonApiStore,
  makeD1PylonSparkPayoutTargetStore,
  resolveSparkPayoutDestination,
} from './pylon-api'
import { makePylonApiRoutes } from './pylon-api-routes'
import {
  handlePylonCapacityFunnelApi,
  handlePylonCapacityFunnelHistoryApi,
  makeD1PylonCapacityFunnelSnapshotStore,
  recordPylonCapacityFunnelSnapshots,
} from './pylon-capacity-funnel-live-routes'
import {
  PYLON_CLAUDE_TURN_INGEST_PATH,
  PYLON_CODEX_ASSIGNMENT_PROOF_PATH,
  PYLON_CODEX_ASSIGNMENT_TRACE_STATUS_PATH,
  PYLON_CODEX_EVENT_CHUNK_INGEST_PATH,
  PYLON_CODEX_TURN_INGEST_PATH,
  makeD1PylonCodexAssignmentProofStore,
  makeD1R2PylonCodexRawEventChunkStore,
  makeD1R2PylonCodexRawEventStore,
  makePylonCodexTurnIngestRoutes,
} from './pylon-codex-turn-ingest-routes'
import {
  PylonLargestDecentralizedTrainingClaimEndpoint,
  handlePylonLargestDecentralizedTrainingClaimStatusApi,
} from './pylon-largest-decentralized-training-claim-status-routes'
import { makeD1PylonMarketplaceJobStore } from './pylon-marketplace-service'
import {
  PylonMultiEarningNodeEndpoint,
  handlePylonMultiEarningNodeApi,
  isPylonMultiEarningProjectionEnabled,
} from './pylon-multi-earning-node-routes'
import { makePylonOpenAgentsAuthHandlers } from './pylon-openagents-auth-routes'
import {
  type RelayHealthFetch,
  canonicalMarketRelayUrl,
  makeD1RelayHealthStore,
  runRelayHealthProbeTick,
} from './relay-health'
import { handlePublicRelayHealthApi } from './relay-health-routes'
import { handleResendWebhook } from './resend-webhooks'
import { makeExactRouteRegistry } from './routing/exact-routes'
import {
  cleanProductRouteRedirectLocation,
  githubWriteResultRedirectLocation,
} from './routing/redirect-policy'
import {
  OpenAgentsWorkerRequest,
  WorkerRequestLayer,
  openAgentsDatabase,
  scheduleBackgroundWork,
} from './runtime'
import {
  compactRandomId,
  currentDate,
  currentEpochMillis,
  currentIsoTimestamp,
  epochMillisToIsoTimestamp,
  isoTimestampAfter,
  randomUuid,
} from './runtime-primitives'
import {
  SelfServeFanoutEndpoint,
  handleSelfServeFanoutApi,
  isSelfServeFanoutEnabled,
} from './self-serve-fanout-routes'
import { makeShareRoutes } from './share-routes'
import { handleSignaturePackageValidationApi } from './signature-package-validation-routes'
import {
  SignatureUsageMeteringEndpoint,
  handleSignatureUsageMeteringApi,
  isSignatureUsageMeteringEnabled,
} from './signature-usage-metering-routes'
import { makeD1SiteCommerceReviewStore } from './site-commerce-review'
import { makeSiteCommerceRoutes } from './site-commerce-routes'
import { resolveSiteFormSpec } from './site-form-spec-registry'
import { makeD1SiteMdkAccountBindingStore } from './site-mdk-account-bindings'
import { makeD1SiteMdkCheckoutIntentStore } from './site-mdk-checkout-intents'
import { omegaMdkDemoSitePaymentCatalog } from './site-mdk-demo-product'
import {
  isSiteFormCaptureEnabled,
  makeSitePageFormCaptureRoutes,
} from './site-page-form-capture-routes'
import {
  type ReferralConsumptionResult,
  consumePendingReferralForUser,
} from './site-referral-attribution-consumption'
import { makeSiteReferralInspectionRoutes } from './site-referral-inspection-routes'
import { sendSiteReferralOnboardingForConsumption } from './site-referral-onboarding'
import { makeSiteReferralPayoutAdapter } from './site-referral-payout-adapter'
import { makeSiteReferralPayoutLedgerRoutes } from './site-referral-payout-ledger-routes'
import { handleSiteReferralPayoutsPublicApi } from './site-referral-payout-public-routes'
import { makeD1SiteReferralPayoutReceiptStore } from './site-referral-payout-receipts'
import { makeSiteReferralRoutes } from './site-referral-routes'
import { PENDING_REFERRAL_COOKIE } from './site-referrals'
import { makeSiteRuntimeRoutes } from './site-runtime-routes'
import { makeSitesOrchestrationRoutes } from './sites-orchestration-routes'
import { readBillingCreditPackages } from './stripe-billing'
import { makeD1StripeCheckoutReceiptStore } from './stripe-checkout-receipts'
import {
  decideHighFrequencyBroadcast,
  highFrequencyBroadcastLastAtStorageKey,
} from './sync-broadcast-throttle'
import {
  type SyncNotificationContext,
  notifyAgentRunSyncScopes,
  notifySyncScopes,
  publishTeamChatMessageSync,
  publishTeamThreadFileSync,
} from './sync-notifier'
import { type ParsedSyncPath, makeSyncRoutes } from './sync-routes'
import { autoSettleVerifiedPair } from './tassadar-auto-settlement'
import {
  TASSADAR_COMPILED_MODULE_MARKETPLACE_ROUTE,
  buildPublicTassadarCompiledModuleMarketplaceEnvelope,
} from './tassadar-compiled-module-marketplace'
import {
  TassadarPerceptaArchitectureReceiptsEndpoint,
  handleTassadarPerceptaArchitectureReceiptsApi,
} from './tassadar-percepta-architecture-receipts-routes'
import {
  TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
  handleTassadarPerceptaCpuTransformTrainingReceiptsApi,
} from './tassadar-percepta-cpu-transform-training-receipts-routes'
import {
  TassadarReplayRequest,
  runTassadarReplayValidation,
} from './tassadar-replay-validator'
import {
  readTassadarRealSettlementGate,
  tassadarRealSettledSatsForDay,
  tassadarRealSettlementUtcDayKey,
} from './tassadar-run-settlement-gate'
import {
  buildSettledFeedEvents,
  publishSettledFeedEvents,
} from './tassadar-settled-feed-sync'
import { makeD1TrainingTraceContributionStore } from './tassadar-trace-contribution-authority'
import { makeTassadarTraceContributionRoutes } from './tassadar-trace-contribution-routes'
import { runTassadarTracePairingScheduled } from './tassadar-trace-pairing'
import {
  type TeamChatMessage,
  type TeamChatRunSummary,
  insertTeamChatMessage,
  listTeamChatMessages,
  makeTeamChatMessageId,
  makeTeamChatThreadId,
  readTeamChatMessageByAgentRunId,
  readTeamChatMessageById,
  teamChatLaunchErrorFromResponse,
  updateTeamChatMessageRunSummary,
} from './team-chat'
import { makeTeamChatRoutes } from './team-chat-routes'
import {
  type UserTeam,
  type UserTeamProject,
  readActiveTeamMembershipRole,
  readActiveTeamProject,
  readTeamsForUser,
} from './team-repository'
import { makeTeamWorkspaceInviteRoutes } from './team-workspace-invite-routes'
import { makeD1TeamWorkspaceInviteStore } from './team-workspace-invites'
import { makeTenantClientRoutes } from './tenant-client-routes'
import { makeTenantHostnameSelfServeRoutes } from './tenant-custom-hostname-self-serve-routes'
import { makeTenantCustomHostnames } from './tenant-custom-hostnames'
import {
  type RouteAccessError,
  RouteAccessForbidden,
  RouteAccessNotFound,
  ThreadAccessService,
} from './thread-access'
import { makeThreadFileRoutes } from './thread-file-routes'
import {
  type PublicThreadFile,
  type ThreadFileRow,
  insertThreadFileMessageReferences,
  listTeamThreadFiles,
  readThreadFileById,
} from './thread-files'
import {
  type BufferPayFn,
  checkTipsBufferBackingInvariant,
  reconcileForwardingBufferPayments,
  runTipsSweepScheduled,
} from './tips-sweep'
import {
  type AutopilotTokenLeaderboards,
  TokenUsageLeaderboards,
} from './token-usage'
import { makeD1TokenUsageLedger } from './token-usage-ledger'
import { makeTokenUsageLedgerRoutes } from './token-usage-ledger-routes'
import {
  makeD1TraceStore,
  makeR2TraceMediaBlobStore,
  makeR2TraceTrajectoryBlobStore,
} from './trace-store-d1'
import { makeTraceStoreRoutes } from './trace-store-routes'
import {
  TrainingAblationDeriskingLedgerEndpoint,
  handleTrainingAblationDeriskingLedgerApi,
} from './training-ablation-derisking-ledger-routes'
import {
  TrainingFullPipelineProgramEndpoint,
  handleTrainingFullPipelineProgramApi,
} from './training-full-pipeline-program-routes'
import {
  TrainingMarathonOperationsEndpoint,
  handleTrainingMarathonOperationsApi,
} from './training-marathon-operations-routes'
import {
  TrainingModelLadderRungsEndpoint,
  handleTrainingModelLadderRungsApi,
} from './training-model-ladder-rungs-routes'
import {
  TrainingPostTrainingDpoPreferenceWorkloadEndpoint,
  handleTrainingPostTrainingDpoPreferenceWorkloadApi,
} from './training-post-training-dpo-preference-workload-routes'
import {
  TrainingPostTrainingInstructSftEndpoint,
  handleTrainingPostTrainingInstructSftApi,
} from './training-post-training-instruct-sft-routes'
import {
  TrainingPostTrainingVibeTestRubricEndpoint,
  handleTrainingPostTrainingVibeTestRubricApi,
} from './training-post-training-vibe-test-rubric-routes'
import {
  TrainingPublicDistributedRunScaleEndpoint,
  handleTrainingPublicDistributedRunScaleApi,
} from './training-public-distributed-run-scale-routes'
import {
  TrainingPublicGradientWindowsEndpoint,
  handleTrainingPublicGradientWindowsApi,
} from './training-public-gradient-windows-routes'
import {
  buildTrainingWindowRecord,
  makeD1TrainingAuthorityStore,
  transitionTrainingWindowRecord,
} from './training-run-window-authority'
import {
  dispatchRealRunSettlementCore,
  makeTrainingRunWindowRoutes,
} from './training-run-window-routes'
import {
  buildTrainingVerificationChallengeRecord,
  finalizeTrainingVerificationChallengeRecord,
  leaseTrainingVerificationChallengeRecord,
  makeD1TrainingVerificationStore,
  runTrainingVerificationClass,
} from './training-verification'
import { makeTrainingVerificationRoutes } from './training-verification-routes'
import {
  makeD1TreasuryTransactionStore,
  makeTreasuryPageRoutes,
} from './treasury-page-routes'
import { makeTreasuryPaymentAuthority } from './treasury-payment-authority'
import { makeHostedMdkPayoutAdapter } from './treasury-payment-hosted-mdk-payout-adapter'
import { makeSparkTreasuryPayoutAdapter } from './treasury-payment-spark-payout-adapter'
import {
  TREASURY_SERVICE_TOKEN_HEADER,
  handleOperatorSparkTreasuryFundingDestinationApi,
  handleOperatorSparkTreasuryFundingInvoiceApi,
  handleOperatorTreasuryFundingDestinationApi,
  handleOperatorTreasuryPayoutApi,
  handleOperatorTreasuryRecipientConfirmationApi,
  handleOperatorTreasuryRecipientReportApi,
  handleOperatorTreasuryStatusApi,
  handleOperatorTreasuryTransactionReconcileApi,
  handlePublicTreasuryLaunchStatusApi,
  reconcilePendingTreasuryTransactions,
} from './treasury-routes'
import {
  type ViralAgentFunnelEventKind,
  recordViralAgentFunnelEvent,
} from './viral-agent-funnel'
import {
  VoiceProgramIngestEndpoint,
  handleVoiceProgramIngestApi,
  isVoiceProgramIngestEnabled,
} from './voice-program-ingest-routes'
import { makeWorkerRouteRequest } from './worker-routes'
import {
  makeD1XClaimRewardTreasuryDispatchStore,
  readXClaimRewardTreasuryDispatchConfig,
  runXClaimRewardTreasuryDispatchScheduled,
  xClaimRewardDispatchDayStartIso,
} from './x-claim-reward-treasury-dispatcher'

export type Env = WorkerBindings & OpenAgentsWorkerConfigEnv

type EmailCampaignDispatcherBindings = WorkerBindings &
  OpenAgentsWorkerConfigEnv
export {
  SESSION_MAX_AGE_SECONDS,
  appendClearSessionCookies,
  appendSessionCookies,
} from './auth-cookies'
export {
  publishTeamChatMessageSync,
  publishTeamThreadFileSync,
  teamChatMessageSyncValue,
  threadFileSyncValue,
} from './sync-notifier'

export const OPENAGENTS_ADMIN_EMAILS = ['chris@openagents.com'] as const
const OPENAGENTS_CORE_TEAM_ID = 'team_openagents_core'
const MDK_SIDECAR_INSTANCE_NAME = 'openagents-mdk-sidecar-20260611-5'
const MDK_TREASURY_INSTANCE_NAME = 'openagents-mdk-treasury-20260610-4'
const SIMPLE_EMAIL_PATTERN = /^[^\s@]+@[^\s@]+\.[^\s@]+$/
const workerRuntime = {
  makeUuid: randomUuid,
  now: currentDate,
  nowMs: currentEpochMillis,
  nowIso: currentIsoTimestamp,
} as const
const AUTH_EMAIL_OTP_SEND_UNAVAILABLE_MESSAGE =
  "We couldn't send a sign-in code right now. Try again in a minute."
const invalidAuthEmailOtpClaim = (email: string): CodeProviderError => ({
  key: 'email',
  type: 'invalid_claim',
  value: email,
})

class MdkSidecarUnavailable extends S.TaggedErrorClass<MdkSidecarUnavailable>()(
  'MdkSidecarUnavailable',
  {
    error: S.Defect,
  },
) {}
class UnsupportedAuthProvider extends S.TaggedErrorClass<UnsupportedAuthProvider>()(
  'UnsupportedAuthProvider',
  {
    provider: S.String,
  },
) {}

class AuthSignInError extends S.TaggedErrorClass<AuthSignInError>()(
  'AuthSignInError',
  {
    reason: S.String,
  },
) {}

export class MdkSidecarContainer extends Container<Env> {
  override defaultPort = 8080
  override sleepAfter = '30m'
  override pingEndpoint = 'localhost:8080/healthz'

  constructor(ctx: DurableObjectState<{}>, environment: Env) {
    super(ctx, environment)
    this.envVars = mdkContainerEnvVars(environment)
    this.labels = {
      service: 'openagents-mdk-sidecar',
    }
  }
}

class DurableMdkOutcomeContainer extends Container<Env> {
  private async readPaymentOutcome(
    paymentId: string,
  ): Promise<DurableMdkPaymentOutcome | null> {
    const value = await this.ctx.storage.get<DurableMdkPaymentOutcome>(
      mdkPaymentOutcomeStorageKey(paymentId),
    )

    return value === undefined ? null : value
  }

  private async writePaymentOutcome(
    paymentId: string,
    outcome: DurableMdkPaymentOutcome,
  ): Promise<void> {
    await this.ctx.storage.put(mdkPaymentOutcomeStorageKey(paymentId), outcome)
  }

  override async fetch(request: Request) {
    const url = new URL(request.url)

    if (request.method === 'POST' && url.pathname === '/pay') {
      const response = await this.containerFetch(request)
      await journalMdkResponseOutcome(response, (paymentId, outcome) =>
        this.writePaymentOutcome(paymentId, outcome),
      )

      return response
    }

    if (request.method === 'GET') {
      const paymentId = mdkPaymentIdFromStatusPath(url.pathname)

      if (paymentId !== null) {
        try {
          const response = await this.containerFetch(request)
          const payload = await response
            .clone()
            .json()
            .catch(() => null)
          const outcome = mdkTerminalOutcomeFromPayload(
            payload,
            currentIsoTimestamp(),
          )

          if (outcome !== null) {
            await this.writePaymentOutcome(paymentId, outcome)

            return response
          }

          const stored = await this.readPaymentOutcome(paymentId)

          return stored === null
            ? response
            : durableMdkPaymentOutcomeResponse(paymentId, stored)
        } catch (error) {
          const stored = await this.readPaymentOutcome(paymentId)

          if (stored !== null) {
            return durableMdkPaymentOutcomeResponse(paymentId, stored)
          }

          throw error
        }
      }
    }

    return this.containerFetch(request)
  }
}

const mdkTreasuryContainerEnvVars = (
  environment: OpenAgentsWorkerConfigEnv,
): Record<string, string> => {
  const accessToken = optionalMdkContainerSecret(
    environment.MDK_TREASURY_ACCESS_TOKEN,
  )
  const mnemonic = optionalMdkContainerSecret(environment.MDK_TREASURY_MNEMONIC)
  const serviceToken = optionalMdkContainerSecret(
    environment.MDK_TREASURY_SERVICE_TOKEN,
  )
  const sparkApiKey = optionalMdkContainerSecret(
    environment.SPARK_TREASURY_API_KEY ?? environment.OPENAGENTS_SPARK_API_KEY,
  )
  const sparkMnemonic = optionalMdkContainerSecret(
    environment.SPARK_TREASURY_MNEMONIC ?? environment.MDK_TREASURY_MNEMONIC,
  )
  const sparkNetwork = optionalMdkContainerSecret(
    environment.SPARK_TREASURY_NETWORK,
  )
  const sparkStorageDir = optionalMdkContainerSecret(
    environment.SPARK_TREASURY_STORAGE_DIR,
  )
  const sparkTimeoutMs = optionalMdkContainerSecret(
    environment.SPARK_TREASURY_TIMEOUT_MS,
  )

  return {
    ...(accessToken === undefined
      ? {}
      : { MDK_TREASURY_ACCESS_TOKEN: accessToken }),
    ...(mnemonic === undefined ? {} : { MDK_TREASURY_MNEMONIC: mnemonic }),
    ...(serviceToken === undefined
      ? {}
      : { MDK_TREASURY_SERVICE_TOKEN: serviceToken }),
    ...(sparkApiKey === undefined
      ? {}
      : { SPARK_TREASURY_API_KEY: sparkApiKey }),
    ...(sparkMnemonic === undefined
      ? {}
      : { SPARK_TREASURY_MNEMONIC: sparkMnemonic }),
    ...(sparkNetwork === undefined
      ? {}
      : { SPARK_TREASURY_NETWORK: sparkNetwork }),
    ...(sparkStorageDir === undefined
      ? {}
      : { SPARK_TREASURY_STORAGE_DIR: sparkStorageDir }),
    ...(sparkTimeoutMs === undefined
      ? {}
      : { SPARK_TREASURY_TIMEOUT_MS: sparkTimeoutMs }),
  }
}

const MDK_TREASURY_CONTAINER_GENERATION_KEY =
  'openagents.mdk_treasury.container_generation'
const MDK_TREASURY_CONTAINER_GENERATION =
  '2026-06-17.spark-treasury-funding-invoice'

export class MdkTreasuryContainer extends DurableMdkOutcomeContainer {
  override defaultPort = 8080
  override sleepAfter = '30m'
  override pingEndpoint = 'localhost:8080/healthz'

  constructor(ctx: DurableObjectState<{}>, environment: Env) {
    super(ctx, environment)
    this.envVars = mdkTreasuryContainerEnvVars(environment)
    this.labels = {
      service: 'openagents-mdk-treasury',
    }
  }

  private async ensureCurrentContainerGeneration(): Promise<void> {
    const current = await this.ctx.storage.get<string>(
      MDK_TREASURY_CONTAINER_GENERATION_KEY,
    )

    if (current === MDK_TREASURY_CONTAINER_GENERATION) {
      return
    }

    const container = this.ctx.container
    if (container?.running) {
      await container.destroy('openagents-mdk-treasury-generation-bump')
    }

    await this.ctx.storage.put(
      MDK_TREASURY_CONTAINER_GENERATION_KEY,
      MDK_TREASURY_CONTAINER_GENERATION,
    )
  }

  override async fetch(request: Request) {
    await this.ensureCurrentContainerGeneration()

    return super.fetch(request)
  }
}

const fetchMdkTreasuryPath = (
  environment: Env,
): ContainerPathFetch | undefined => {
  const namespace = environment.MDK_TREASURY as
    | DurableObjectNamespace<MdkTreasuryContainer>
    | undefined

  if (namespace === undefined) {
    return undefined
  }

  const serviceToken = optionalMdkContainerSecret(
    environment.MDK_TREASURY_SERVICE_TOKEN,
  )

  return (path, init) =>
    getContainer(namespace, MDK_TREASURY_INSTANCE_NAME).fetch(
      new Request(`http://mdk-treasury${path}`, {
        ...(init?.body === undefined ? {} : { body: init.body }),
        headers: {
          'content-type': 'application/json',
          ...(serviceToken === undefined
            ? {}
            : { [TREASURY_SERVICE_TOKEN_HEADER]: serviceToken }),
        },
        method: init?.method ?? 'GET',
        ...(init?.signal === undefined ? {} : { signal: init.signal }),
      }),
    )
}

// Build the accepted-outcome settlement sink (#6011, EPIC #6017) the verdict-callback
// route fires when a VERIFIED + EXECUTED accepted outcome backfills for the first time.
// REUSES the proven Spark dispatch core (`dispatchRealRunSettlementCore` +
// `makeSparkTreasuryPayoutAdapter` + `resolveSparkPayoutDestination`) and the owner
// real-settlement gate (`readTassadarRealSettlementGate`) — NO parallel money path.
//
// INERT BY DEFAULT, double-gated:
//   - returns `undefined` (no settlement attempted at all) unless the KHALA loop-arming
//     flag is armed (`readKhalaLoopArming`) — the FIRST default-OFF gate; and
//   - even when armed, every per-party leg independently fail-closes on the owner
//     real-settlement gate (default OFF), the per-payout cap, the daily budget, and a
//     registered Spark destination — the SECOND gate stack, inside the engine.
// So arming a real accepted-outcome payout requires BOTH the loop flag AND the owner
// gate; with either OFF no sats move. The serving-run ref is derived from the
// verification receipt ref so the gate allowlist enrolls a specific accepted outcome.
const makeAcceptedOutcomeSettlementSink = (
  env: Env,
): AcceptedOutcomeSettlementSink | undefined => {
  // FIRST gate: the loop-arming flag. OFF (default) => no sink => no settlement.
  if (
    !readKhalaLoopArming(env as unknown as Record<string, unknown>).loopArmed
  ) {
    return undefined
  }

  const db = openAgentsDatabase(env)
  const ledger = makeD1NexusTreasuryPayoutLedgerStore(db)
  const sparkTargetStore = makeD1PylonSparkPayoutTargetStore(db)
  const contributionStore = makeD1TrainingTraceContributionStore(db)

  // Owner resolver for a contributor's Spark payout destination — same shape as the
  // Tassadar autostream: direct registered-pylon lookup, then the owner-scoped fallback
  // via the contributor's most-recent worker pylon. Never crosses agent ownership.
  const resolveContributorOwnerAgentUserId = async (
    contributorRef: string,
  ): Promise<string | undefined> => {
    const pylonApiStore = makeD1PylonApiStore(db)
    const direct = await pylonApiStore
      .readRegistration(contributorRef)
      .then(registration => registration?.ownerAgentUserId)
    if (direct !== undefined && direct.trim() !== '') {
      return direct
    }
    const pylonRefForDevice =
      await contributionStore.readMostRecentPylonRefByDeviceRef(contributorRef)
    if (pylonRefForDevice === undefined) {
      return undefined
    }
    return pylonApiStore
      .readRegistration(pylonRefForDevice)
      .then(registration => registration?.ownerAgentUserId)
  }

  // The REAL Spark dispatch (the proven receipt-first idempotent core). Performs a
  // real send once every gate downstream authorizes it. The Khala records are
  // structurally identical to the Tassadar run-settlement records the core takes.
  const realDispatch: KhalaSettlementDispatch = dispatchInput =>
    dispatchRealRunSettlementCore<WorkerBindings>(
      {
        env,
        makeSettlementPaymentAuthority: (authorityEnv, context) =>
          makeTreasuryPaymentAuthority({
            adapters: [
              makeSparkTreasuryPayoutAdapter({
                fetchTreasury: fetchMdkTreasuryPath(authorityEnv),
                providerRef: context.providerRef,
                resolveDestination: () =>
                  Effect.succeed(context.privatePayoutDestination),
              }),
            ],
            ledgerStore: context.ledgerStore,
          }),
        readSettlementWalletReadiness: async authorityEnv => {
          const fetchTreasury = fetchMdkTreasuryPath(authorityEnv)
          if (fetchTreasury === undefined) {
            return 'absent'
          }
          try {
            const response = await fetchTreasury('/spark/balance')
            return response.ok ? 'ready' : 'absent'
          } catch {
            return 'absent'
          }
        },
        resolveSettlementPayoutDestination: (_authorityEnv, ref) =>
          resolveSparkPayoutDestination(
            sparkTargetStore,
            ref,
            resolveContributorOwnerAgentUserId,
          ),
      },
      {
        contributorRef: dispatchInput.contributorRef,
        ledger,
        settlement: dispatchInput.settlement,
      },
    )

  // The DRY-RUN dispatch (records the dereferenceable receipt, NEVER a real send).
  // It is the fail-closed fallback the gated selector routes to whenever the loop
  // flag or the owner gate does not authorize a real payout for this exact outcome.
  const dryRunDispatch = makeDryRunSettlementDispatch({
    readReceiptByRef: ref => ledger.readPaymentAuthorityReceiptByRef(ref),
    recordReceipt: record => ledger.createPaymentAuthorityReceipt(record),
  })

  return outcome => {
    // The gate allowlist enrolls a specific accepted outcome by its derived run ref.
    const settlementRunRef = `accepted_outcome.${outcome.verificationReceiptRef
      .replace(/[^A-Za-z0-9_.:/-]/g, '_')
      .slice(0, 180)}`
    // GATED dispatch: real Spark send ONLY when the loop flag is armed AND the
    // owner real-settlement gate authorizes THIS payout (adapter + cap + run
    // allowlist + daily cap); otherwise the dry-run. This is a SECOND, independent
    // fail-closed gate evaluation at the dispatch boundary, on top of the engine's
    // own GATE 3, so a real send is unreachable without the owner's gate JSON + flag.
    const gatedDispatch = makeKhalaLoopSettlementDispatch({
      arming: readKhalaLoopArming(env as unknown as Record<string, unknown>),
      dryRunDispatch,
      readGate: () => readTassadarRealSettlementGate(env),
      realDispatch,
      settlementRunRef,
    })
    return settleVerifiedAcceptedOutcome(
      {
        dispatchRealSettlement: gatedDispatch,
        ledger,
        nowIso: currentIsoTimestamp(),
        readGate: () => readTassadarRealSettlementGate(env),
        resolvePayoutDestination: ref =>
          resolveSparkPayoutDestination(
            sparkTargetStore,
            ref,
            resolveContributorOwnerAgentUserId,
          ),
        settlementRunRef,
      },
      outcome,
    ).pipe(
      Effect.map(result => summarizeAcceptedOutcomeSettlement(outcome, result)),
    )
  }
}

const TIPS_BUFFER_SERVICE_TOKEN_HEADER = 'x-tips-buffer-service-token'
const MDK_TIPS_BUFFER_INSTANCE_NAME = 'openagents-mdk-tips-buffer-20260610-1'

const mdkTipsBufferContainerEnvVars = (
  environment: OpenAgentsWorkerConfigEnv,
): Record<string, string> => {
  const accessToken = optionalMdkContainerSecret(
    environment.MDK_TIPS_BUFFER_ACCESS_TOKEN,
  )
  const mnemonic = optionalMdkContainerSecret(
    environment.MDK_TIPS_BUFFER_MNEMONIC,
  )
  const serviceToken = optionalMdkContainerSecret(
    environment.MDK_TIPS_BUFFER_SERVICE_TOKEN,
  )

  return {
    ...(accessToken === undefined
      ? {}
      : { MDK_TIPS_BUFFER_ACCESS_TOKEN: accessToken }),
    ...(mnemonic === undefined ? {} : { MDK_TIPS_BUFFER_MNEMONIC: mnemonic }),
    ...(serviceToken === undefined
      ? {}
      : { MDK_TIPS_BUFFER_SERVICE_TOKEN: serviceToken }),
  }
}

export class MdkTipsBufferContainer extends DurableMdkOutcomeContainer {
  override defaultPort = 8080
  override sleepAfter = '30m'
  override pingEndpoint = 'localhost:8080/healthz'

  constructor(ctx: DurableObjectState<{}>, environment: Env) {
    super(ctx, environment)
    this.envVars = mdkTipsBufferContainerEnvVars(environment)
    this.labels = {
      service: 'openagents-mdk-tips-buffer',
    }
  }
}

const fetchMdkTipsBufferPath = (
  environment: Env,
): ContainerPathFetch | undefined => {
  const namespace = (
    environment as {
      MDK_TIPS_BUFFER?: DurableObjectNamespace<MdkTipsBufferContainer>
    }
  ).MDK_TIPS_BUFFER

  if (
    namespace === undefined ||
    optionalMdkContainerSecret(environment.MDK_TIPS_BUFFER_MNEMONIC) ===
      undefined
  ) {
    return undefined
  }

  const serviceToken = optionalMdkContainerSecret(
    environment.MDK_TIPS_BUFFER_SERVICE_TOKEN,
  )

  return (path, init) =>
    getContainer(namespace, MDK_TIPS_BUFFER_INSTANCE_NAME).fetch(
      new Request(`http://mdk-tips-buffer${path}`, {
        ...(init?.body === undefined ? {} : { body: init.body }),
        headers: {
          'content-type': 'application/json',
          ...(serviceToken === undefined
            ? {}
            : { [TIPS_BUFFER_SERVICE_TOKEN_HEADER]: serviceToken }),
        },
        method: init?.method ?? 'GET',
        ...(init?.signal === undefined ? {} : { signal: init.signal }),
      }),
    )
}

const runArtanisForumRouteEffect = async (
  effect: ReturnType<typeof forumRoutes.routeForumRequest> | undefined,
) => (effect === undefined ? undefined : Effect.runPromise(effect))

const artanisComposerForumPostForEnv =
  (environment: Env) =>
  async (input: {
    topicId: string
    bodyText: string
    idempotencyKey: string
  }): Promise<{ postId: string } | { error: string }> => {
    const token = (environment as { ARTANIS_AGENT_TOKEN?: string })
      .ARTANIS_AGENT_TOKEN
    if (token === undefined || token === '') {
      return { error: 'artanis_agent_token_missing' }
    }
    try {
      const response = await runArtanisForumRouteEffect(
        forumRoutes.routeForumRequest(
          new Request(
            `https://openagents.com/api/forum/topics/${input.topicId}/posts`,
            {
              body: JSON.stringify({ bodyText: input.bodyText }),
              headers: {
                Authorization: `Bearer ${token}`,
                'Content-Type': 'application/json',
                'Idempotency-Key': input.idempotencyKey,
              },
              method: 'POST',
            },
          ),
          openAgentsDatabase(environment),
          {
            agentStore: makeD1AgentRegistrationStore(
              openAgentsDatabase(environment),
            ),
          },
        ),
      )
      if (response === undefined) {
        return { error: 'forum_route_unmatched' }
      }
      const payload = (await response.json()) as {
        error?: string
        post?: { postId?: string }
      }
      return payload.post?.postId === undefined
        ? { error: String(payload.error ?? response.status) }
        : { postId: payload.post.postId }
    } catch (error) {
      return {
        error:
          error instanceof Error ? error.message.slice(0, 120) : 'post_failed',
      }
    }
  }

const artanisComposerTipForEnv =
  (environment: Env) =>
  async (input: {
    postId: string
    amountSat: number
    idempotencyKey: string
    publicReceiptRef: string
  }): Promise<
    | {
        ladderReason: string
        payInId: string
        receiptRef: string
        rung: string
      }
    | { error: string }
  > => {
    const token = (environment as { ARTANIS_AGENT_TOKEN?: string })
      .ARTANIS_AGENT_TOKEN
    if (token === undefined || token === '') {
      return { error: 'artanis_agent_token_missing' }
    }
    try {
      const response = await runArtanisForumRouteEffect(
        forumRoutes.routeForumRequest(
          new Request(
            `https://openagents.com/api/forum/posts/${input.postId}/tips/ladder`,
            {
              body: JSON.stringify({
                amountSat: input.amountSat,
                publicReceiptRef: input.publicReceiptRef,
              }),
              headers: {
                Authorization: `Bearer ${token}`,
                'Content-Type': 'application/json',
                'Idempotency-Key': input.idempotencyKey,
              },
              method: 'POST',
            },
          ),
          openAgentsDatabase(environment),
          {
            agentStore: makeD1AgentRegistrationStore(
              openAgentsDatabase(environment),
            ),
            tipsBufferPay: tipsBufferPayFnForEnv(environment),
          },
        ),
      )
      if (response === undefined) {
        return { error: 'forum_route_unmatched' }
      }
      const payload = (await response.json()) as {
        error?: string
        ladderReason?: string
        payInId?: string
        receiptRef?: string
        rung?: string
      }
      return payload.rung === undefined ||
        payload.receiptRef === undefined ||
        payload.payInId === undefined ||
        payload.ladderReason === undefined
        ? { error: String(payload.error ?? response.status) }
        : {
            ladderReason: payload.ladderReason,
            payInId: payload.payInId,
            receiptRef: payload.receiptRef,
            rung: payload.rung,
          }
    } catch (error) {
      return {
        error:
          error instanceof Error ? error.message.slice(0, 120) : 'tip_failed',
      }
    }
  }

const tipsBufferPayFnForEnv = (environment: Env): BufferPayFn | null => {
  const fetchBuffer = fetchMdkTipsBufferPath(environment)

  if (fetchBuffer === undefined) {
    return null
  }

  return async ({ amountSat, destination }) => {
    const attempt = async (destination: string) => {
      const response = await fetchBuffer('/pay', {
        body: JSON.stringify({ amountSat, destination }),
        method: 'POST',
      })
      const result = (await response.json()) as {
        error?: string
        paymentId?: string
        status?: string
      }

      if (response.ok && result.status === 'pending' && result.paymentId) {
        return {
          ok: false as const,
          paymentId: String(result.paymentId),
          pending: true as const,
        }
      }

      if (!response.ok || result.status !== 'succeeded') {
        return {
          ok: false as const,
          reason: String(result.error ?? result.status ?? response.status),
        }
      }

      return {
        ok: true as const,
        paymentRef: `payment.tips_buffer.${String(result.paymentId ?? '').slice(0, 12)}`,
      }
    }

    try {
      return await attempt(destination)
    } catch (error) {
      return {
        ok: false as const,
        reason:
          error instanceof Error ? error.message.slice(0, 80) : 'fetch_failed',
      }
    }
  }
}

const fetchMdkSidecarRequest = async (request: Request, environment: Env) => {
  if (environment.MDK_SIDECAR === undefined) {
    return noStoreJsonResponse(
      { error: 'mdk_sidecar_unconfigured' },
      { status: 503 },
    )
  }

  try {
    return await getContainer(
      environment.MDK_SIDECAR as DurableObjectNamespace<MdkSidecarContainer>,
      MDK_SIDECAR_INSTANCE_NAME,
    ).fetch(request)
  } catch {
    return noStoreJsonResponse(
      { error: 'mdk_sidecar_unavailable' },
      { status: 503 },
    )
  }
}

const routeMdkSidecarRequest = (request: Request, environment: Env) =>
  Effect.tryPromise({
    catch: error => new MdkSidecarUnavailable({ error }),
    try: () => fetchMdkSidecarRequest(request, environment),
  }).pipe(
    Effect.catchTag('MdkSidecarUnavailable', () =>
      Effect.succeed(
        noStoreJsonResponse(
          { error: 'mdk_sidecar_unavailable' },
          { status: 503 },
        ),
      ),
    ),
  )
const TrimmedString = S.Trim
const NonEmptyTrimmedString = TrimmedString.check(S.isNonEmpty())
const EmailString = NonEmptyTrimmedString.check(
  S.isPattern(SIMPLE_EMAIL_PATTERN),
)

const UserSubject = S.Struct({
  userId: NonEmptyTrimmedString,
  // 'github' carries githubId/login; 'email' (one-time code) has neither.
  provider: S.Literals(['github', 'email']),
  githubId: S.optionalKey(NonEmptyTrimmedString),
  login: S.optionalKey(NonEmptyTrimmedString),
  email: EmailString,
  name: NonEmptyTrimmedString,
  avatarUrl: S.String,
})

type UserSubject = typeof UserSubject.Type

const subjects = createSubjects({
  user: S.toStandardSchemaV1(UserSubject),
})

export const isOpenAgentsAdminEmail = (email: string): boolean =>
  OPENAGENTS_ADMIN_EMAILS.some(
    adminEmail => adminEmail === email.trim().toLowerCase(),
  )

const GitHubUser = S.Struct({
  id: S.Union([S.Number, S.String]),
  login: NonEmptyTrimmedString,
  name: S.optionalKey(S.NullOr(S.String)),
  avatar_url: S.optionalKey(S.NullOr(S.String)),
})

const GitHubEmail = S.Struct({
  email: EmailString,
  primary: S.Boolean,
  verified: S.Boolean,
})

const GitHubEmails = S.Array(GitHubEmail)

const GitHubOAuthToken = S.Struct({
  access_token: NonEmptyTrimmedString,
  scope: S.optionalKey(TrimmedString),
  token_type: S.optionalKey(TrimmedString),
})

const GITHUB_LOGIN_SCOPES = ['read:user', 'user:email', 'repo'] as const
const LOGIN_ORIGIN_COOKIE = 'oa_login_origin'
const LOGIN_RETURN_TO_COOKIE = 'oa_login_return_to'
const LOGIN_ERROR_COOKIE = 'oa_login_error'
const LOGIN_ERROR_MAX_AGE_SECONDS = 60

type GitHubUser = typeof GitHubUser.Type
type GitHubEmail = typeof GitHubEmail.Type
type GitHubOAuthToken = typeof GitHubOAuthToken.Type

type UserKindTotals = Readonly<{
  admins: number
  adminEmails: ReadonlyArray<string>
  humans: number
  agents: number
  total: number
}>

type TeamAutopilotContextMessage = Readonly<{
  authorName: string
  authorUserId: string
  body: string
  createdAt: string
}>

type TeamAutopilotContextFile = Pick<
  PublicThreadFile,
  'contentType' | 'createdAt' | 'filename' | 'id' | 'sizeBytes'
>

type TeamAutopilotSelectedFile = TeamAutopilotContextFile &
  Readonly<{ excerpt?: string }>

export type TeamAutopilotContextBundle = Readonly<{
  normalizedPrompt: string
  parentProjectId?: string
  parentProjectName?: string
  parentTeamChatMessageId: string
  parentTeamId: string
  recentMessages: ReadonlyArray<TeamAutopilotContextMessage>
  selectedFiles: ReadonlyArray<TeamAutopilotSelectedFile>
  selectedTeamFileIds: ReadonlyArray<string>
}>

const uuidPattern =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

const optionalUuid = (value: unknown): string | undefined => {
  const text = optionalString(value)

  return text !== undefined && uuidPattern.test(text)
    ? text.toLowerCase()
    : undefined
}

type AuthenticatedActor =
  | Readonly<{
      kind: 'human'
      user: UserSubject
      tokens?: Tokens
    }>
  | Readonly<{
      kind: 'agent'
      agent: ProgrammaticAgentSession
    }>

const teamChatThreadId = (teamId: string): string => `team:${teamId}:chat`

const teamProjectChatThreadId = (teamId: string, projectId: string): string =>
  `team:${teamId}:project:${projectId}:chat`

const compactTeamContextText = (value: string, maxLength: number): string => {
  const compact = value.replace(/\s+/g, ' ').trim()

  return compact.length <= maxLength
    ? compact
    : `${compact.slice(0, Math.max(0, maxLength - 3))}...`
}

export const selectedTeamFileIdsForAutopilotPrompt = (
  input: Readonly<{
    files: ReadonlyArray<TeamAutopilotContextFile>
    prompt: string
    requestedFileIds?: ReadonlyArray<string>
  }>,
): ReadonlyArray<string> => {
  const teamFileIds = new Set(input.files.map(file => file.id))
  const requested = (input.requestedFileIds ?? []).filter(fileId =>
    teamFileIds.has(fileId),
  )

  return [...new Set(requested)].slice(0, 8)
}

export const teamAutopilotContextBundle = (
  input: Readonly<{
    files: ReadonlyArray<TeamAutopilotContextFile>
    messages: ReadonlyArray<TeamChatMessage>
    project?: UserTeamProject
    parentTeamChatMessageId: string
    prompt: string
    requestedFileIds?: ReadonlyArray<string>
    teamId: string
  }>,
): TeamAutopilotContextBundle => {
  const selectedTeamFileIds = selectedTeamFileIdsForAutopilotPrompt({
    files: input.files,
    prompt: input.prompt,
    ...(input.requestedFileIds === undefined
      ? {}
      : { requestedFileIds: input.requestedFileIds }),
  })
  const fileById = new Map(input.files.map(file => [file.id, file]))

  return {
    normalizedPrompt: compactTeamContextText(input.prompt, 4_000),
    ...(input.project === undefined
      ? {}
      : {
          parentProjectId: input.project.id,
          parentProjectName: input.project.name,
        }),
    parentTeamChatMessageId: input.parentTeamChatMessageId,
    parentTeamId: input.teamId,
    recentMessages: input.messages.slice(-8).map(message => ({
      authorName: message.author.name,
      authorUserId: message.author.userId,
      body: compactTeamContextText(message.body, 320),
      createdAt: message.createdAt,
    })),
    selectedFiles: selectedTeamFileIds.flatMap(fileId => {
      const file = fileById.get(fileId)

      return file === undefined ? [] : [file]
    }),
    selectedTeamFileIds,
  }
}

const teamAutopilotContextLine = (
  message: TeamAutopilotContextMessage,
): string =>
  `- ${message.createdAt} ${message.authorName} (${message.authorUserId}): ${message.body}`

const teamAutopilotSelectedFileLines = (
  file: TeamAutopilotSelectedFile,
): ReadonlyArray<string> => [
  `- ${file.filename} (${file.contentType}, ${file.sizeBytes} bytes, id ${file.id})`,
  ...(file.excerpt === undefined ? [] : [`  excerpt: ${file.excerpt}`]),
]

export const teamAutopilotChildRunGoal = (
  bundle: TeamAutopilotContextBundle,
): string =>
  [
    bundle.normalizedPrompt,
    '',
    'Team room context for this Autopilot run:',
    `parentTeamId: ${bundle.parentTeamId}`,
    ...(bundle.parentProjectId === undefined
      ? []
      : [
          `parentProjectId: ${bundle.parentProjectId}`,
          `parentProjectName: ${bundle.parentProjectName ?? 'unknown project'}`,
        ]),
    `parentTeamChatMessageId: ${bundle.parentTeamChatMessageId}`,
    `normalizedPrompt: ${bundle.normalizedPrompt}`,
    `selectedTeamFileIds: ${
      bundle.selectedTeamFileIds.length === 0
        ? 'none'
        : bundle.selectedTeamFileIds.join(', ')
    }`,
    'selectedTeamFiles:',
    ...(bundle.selectedFiles.length === 0
      ? ['- none']
      : bundle.selectedFiles.flatMap(teamAutopilotSelectedFileLines)),
    'recentTeamConversation:',
    ...(bundle.recentMessages.length === 0
      ? ['- none']
      : bundle.recentMessages.map(teamAutopilotContextLine)),
    '',
    'Answer the team room directly. Use the context above only to orient the run; do not narrate internal dispatch or writeback mechanics to the user.',
  ].join('\n')

const getAppOrigin = (env: OpenAgentsWorkerConfigEnv): string =>
  getOpenAgentsWorkerConfig(env).app.origin

const getResendEmailConfig = (env: EmailCampaignDispatcherBindings) =>
  getOpenAgentsWorkerConfig(env).email.resend

const getRunnerBackendConfig = (env: OpenAgentsWorkerConfigEnv) =>
  getOpenAgentsWorkerConfig(env).runnerBackends

const getIssuerOrigin = (env: Env): string =>
  getOpenAgentsWorkerConfig(env).openauth.issuerOrigin

const readBearerToken = (request: Request): string | undefined => {
  const authorization = request.headers.get('authorization')

  if (authorization === null) {
    return undefined
  }

  const [scheme, token] = authorization.split(' ')

  if (scheme?.toLowerCase() !== 'bearer' || token === undefined) {
    return undefined
  }

  return token
}

const getAdminApiToken = (env: Env): string | undefined => {
  const token = redactedValue(getOpenAgentsWorkerConfig(env).adminApiToken)

  if (token === undefined || token.trim() === '') {
    return undefined
  }

  return token
}

const errorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const errorName = (error: unknown): string =>
  error instanceof Error ? error.name : typeof error

class GitHubFetchFailure extends Error {
  override name = 'GitHubFetchFailure'
}

const isUniqueConstraintError = (error: unknown): boolean =>
  errorMessage(error).includes('UNIQUE constraint failed')

const fetchGitHubJson = async <T>(
  schema: S.Decoder<T>,
  url: string,
  accessToken: string,
): Promise<T> => {
  const response = await fetch(url, {
    headers: {
      Authorization: `Bearer ${accessToken}`,
      Accept: 'application/vnd.github+json',
      'User-Agent': 'OpenAgents',
      'X-GitHub-Api-Version': '2022-11-28',
    },
  })

  if (!response.ok) {
    throw new GitHubFetchFailure(
      `GitHub request failed with ${response.status}`,
    )
  }

  return decodeUnknownWithSchema(schema, await response.json())
}

const parseGitHubScopeHeader = (
  value: string | undefined,
): ReadonlyArray<string> =>
  value === undefined
    ? []
    : value
        .split(',')
        .map(scope => scope.trim())
        .filter(scope => scope !== '')

const gitHubWriteRedirectUri = (env: Env): string =>
  `${getIssuerOrigin(env)}/github/callback`

const gitHubWriteAuthorizeUrl = (
  env: Env,
  state: string,
  scopes: ReadonlyArray<string>,
): string => {
  const config = getOpenAgentsWorkerConfig(env)
  const url = new URL('https://github.com/login/oauth/authorize')
  url.searchParams.set('client_id', config.github.clientId)
  url.searchParams.set('redirect_uri', gitHubWriteRedirectUri(env))
  url.searchParams.set('response_type', 'code')
  url.searchParams.set('scope', scopes.join(' '))
  url.searchParams.set('state', state)

  return url.toString()
}

const exchangeGitHubOAuthCode = async (
  env: Env,
  code: string,
): Promise<GitHubOAuthToken> => {
  const config = getOpenAgentsWorkerConfig(env)
  const response = await fetch('https://github.com/login/oauth/access_token', {
    method: 'POST',
    headers: {
      Accept: 'application/json',
      'content-type': 'application/x-www-form-urlencoded',
      'User-Agent': 'OpenAgents',
    },
    body: new URLSearchParams({
      client_id: config.github.clientId,
      client_secret: redactedValue(config.github.clientSecret) ?? '',
      code,
      redirect_uri: gitHubWriteRedirectUri(env),
    }).toString(),
  })

  if (!response.ok) {
    throw new GitHubWriteApiFailure({
      operation: 'oauth_token_exchange',
      status: response.status,
      message: `GitHub OAuth token exchange failed with ${response.status}`,
    })
  }

  return decodeUnknownWithSchema(GitHubOAuthToken, await response.json())
}

type GitHubWriteTokenStorage = Readonly<{
  AUTH_STORAGE: KVNamespace
}>

const storeGitHubWriteAccessToken = async (
  storage: GitHubWriteTokenStorage,
  connectionRef: string,
  accessToken: string,
): Promise<void> => {
  try {
    await storage.AUTH_STORAGE.put(
      githubWriteSecretKey(connectionRef),
      accessToken,
    )
  } catch (error) {
    throw new GitHubWriteTokenStorageFailure({
      operation: 'put',
      message:
        error instanceof Error
          ? error.message
          : 'GitHub write token storage failed.',
    })
  }
}

const getPrimaryVerifiedEmail = (
  emails: ReadonlyArray<GitHubEmail>,
): GitHubEmail => {
  const primary = emails.find(email => email.primary)

  if (primary === undefined) {
    throw new Error('No primary GitHub email found')
  }

  if (!primary.verified) {
    throw new Error('Primary GitHub email is not verified')
  }

  return primary
}

const githubUserToSubject = (
  user: GitHubUser,
  primaryEmail: GitHubEmail,
): UserSubject => {
  const githubId = String(user.id)
  const login = user.login

  return {
    userId: `github:${githubId}`,
    provider: 'github',
    githubId,
    login,
    email: primaryEmail.email,
    name: user.name ?? login,
    avatarUrl: user.avatar_url ?? '',
  }
}

const upsertGitHubUser = async (
  db: D1Database,
  user: UserSubject,
): Promise<void> => {
  if (user.githubId === undefined || user.login === undefined) {
    throw new AuthSignInError({
      reason: 'upsertGitHubUser requires a GitHub identity',
    })
  }
  const githubId = user.githubId
  const login = user.login
  const now = workerRuntime.nowIso()

  await db.batch([
    db
      .prepare(
        `INSERT INTO users
          (id, kind, display_name, primary_email, avatar_url, status, created_at, updated_at)
         VALUES (?, 'human', ?, ?, ?, 'active', ?, ?)
         ON CONFLICT(id) DO UPDATE SET
          display_name = excluded.display_name,
          primary_email = excluded.primary_email,
          avatar_url = excluded.avatar_url,
          status = 'active',
          updated_at = excluded.updated_at,
          deleted_at = NULL`,
      )
      .bind(user.userId, user.name, user.email, user.avatarUrl, now, now),
    db
      .prepare(
        `INSERT INTO auth_identities
          (id, user_id, provider, provider_subject, provider_username, email, created_at, updated_at)
         VALUES (?, ?, 'github', ?, ?, ?, ?, ?)
         ON CONFLICT(provider, provider_subject) DO UPDATE SET
          user_id = excluded.user_id,
          provider_username = excluded.provider_username,
          email = excluded.email,
          updated_at = excluded.updated_at,
          deleted_at = NULL`,
      )
      .bind(
        `auth_identity_github_${githubId}`,
        user.userId,
        githubId,
        login,
        user.email,
        now,
        now,
      ),
  ])
}

const normalizeEmail = (email: string): string => email.trim().toLowerCase()

// Build a session subject for an email (one-time-code) login. No GitHub identity;
// the userId namespaces email accounts so they never collide with `github:` ids.
const emailToSubject = (rawEmail: string): UserSubject => {
  const email = normalizeEmail(rawEmail)
  const localPart = email.split('@')[0] ?? email

  return {
    userId: `email:${email}`,
    provider: 'email',
    email,
    name: localPart,
    avatarUrl: '',
  }
}

const upsertEmailUser = async (
  db: D1Database,
  user: UserSubject,
): Promise<void> => {
  const now = workerRuntime.nowIso()

  await db.batch([
    db
      .prepare(
        `INSERT INTO users
          (id, kind, display_name, primary_email, avatar_url, status, created_at, updated_at)
         VALUES (?, 'human', ?, ?, ?, 'active', ?, ?)
         ON CONFLICT(id) DO UPDATE SET
          display_name = excluded.display_name,
          primary_email = excluded.primary_email,
          status = 'active',
          updated_at = excluded.updated_at,
          deleted_at = NULL`,
      )
      .bind(user.userId, user.name, user.email, user.avatarUrl, now, now),
    db
      .prepare(
        `INSERT INTO auth_identities
          (id, user_id, provider, provider_subject, provider_username, email, created_at, updated_at)
         VALUES (?, ?, 'email', ?, ?, ?, ?, ?)
         ON CONFLICT(provider, provider_subject) DO UPDATE SET
          user_id = excluded.user_id,
          email = excluded.email,
          updated_at = excluded.updated_at,
          deleted_at = NULL`,
      )
      .bind(
        `auth_identity_email_${user.email}`,
        user.userId,
        user.email,
        user.name,
        user.email,
        now,
        now,
      ),
  ])
}

// Persist a session subject regardless of provider (session refresh paths can
// carry either a GitHub or an email user).
const upsertUser = async (db: D1Database, user: UserSubject): Promise<void> =>
  user.provider === 'email'
    ? upsertEmailUser(db, user)
    : upsertGitHubUser(db, user)

// Send the one-time sign-in code via Resend directly (auth email stays decoupled
// from the CRM/marketing email-intent machinery). The auth OTP guard owns the
// separate abuse/cost throttle for this path.
const sendSignInCodeEmail = async (
  env: Env,
  rawEmail: string,
  code: string,
): Promise<void> => {
  const config = getOpenAgentsWorkerConfig(env)
  const resend = config.email.resend

  if (resend === undefined) {
    throw new AuthSignInError({
      reason: 'Resend is not configured; cannot send sign-in code',
    })
  }

  const email = normalizeEmail(rawEmail)
  const apiKey = String(Redacted.value(resend.apiKey))
  const from = String(resend.fromEmail)
  const replyTo =
    resend.replyToEmail === undefined ? undefined : String(resend.replyToEmail)

  const response = await fetch('https://api.resend.com/emails', {
    body: JSON.stringify({
      from,
      html: signInCodeEmailHtml(code),
      ...(replyTo === undefined ? {} : { reply_to: replyTo }),
      subject: 'Your OpenAgents sign-in code',
      text: `Your OpenAgents sign-in code is ${code}.\n\nEnter it on the sign-in screen to continue. It expires in ${AUTH_EMAIL_OTP_CODE_TTL_SECONDS / 60} minutes. If you didn't request this, you can ignore this email.`,
      to: email,
    }),
    headers: {
      authorization: `Bearer ${apiKey}`,
      'content-type': 'application/json',
    },
    method: 'POST',
  })

  if (!response.ok) {
    throw new AuthSignInError({
      reason: `Resend sign-in code send failed: ${response.status}`,
    })
  }
}

const signInCodeEmailHtml = (code: string): string =>
  `<!doctype html><html><body style="margin:0;background:#000;color:#f1efe8;font-family:ui-sans-serif,system-ui,sans-serif;padding:40px 0">
    <table role="presentation" width="100%" cellpadding="0" cellspacing="0"><tr><td align="center">
      <table role="presentation" width="420" cellpadding="0" cellspacing="0" style="max-width:420px;width:100%">
        <tr><td style="font-size:18px;font-weight:600;padding-bottom:20px">OpenAgents</td></tr>
        <tr><td style="font-size:15px;line-height:1.6;color:#c9c6bd;padding-bottom:24px">Use this one-time code to finish signing in:</td></tr>
        <tr><td style="font-size:34px;font-weight:700;letter-spacing:8px;font-family:ui-monospace,SFMono-Regular,Menlo,monospace;color:#d6f6ff;padding-bottom:24px">${code}</td></tr>
        <tr><td style="font-size:13px;line-height:1.6;color:#8b8880">This code expires in ${AUTH_EMAIL_OTP_CODE_TTL_SECONDS / 60} minutes. If you didn't request it, you can safely ignore this email.</td></tr>
      </table>
    </td></tr></table>
  </body></html>`

const AUTH_EMAIL_OTP_HTML_ESCAPES: Readonly<Record<string, string>> = {
  '&': '&amp;',
  '"': '&quot;',
  "'": '&#39;',
  '<': '&lt;',
  '>': '&gt;',
}

const authEmailOtpEscapeHtml = (value: string): string =>
  value.replace(
    /[&<>"']/g,
    character => AUTH_EMAIL_OTP_HTML_ESCAPES[character] ?? '&#39;',
  )

const authEmailOtpPrefersJson = (request: Request): boolean => {
  const accept = request.headers.get('accept') ?? ''

  return accept.includes('application/json') && !accept.includes('text/html')
}

const authEmailOtpProblemResponse = (
  request: Request,
  input: Readonly<{
    error: string
    message: string
    retryAfterSeconds?: number
    status: number
  }>,
) => {
  const headers = new Headers({
    'cache-control': 'no-store',
  })

  if (input.retryAfterSeconds !== undefined) {
    headers.set('retry-after', String(input.retryAfterSeconds))
  }

  if (authEmailOtpPrefersJson(request)) {
    return noStoreJsonResponse(
      {
        error: input.error,
        message: input.message,
        retryAfterSeconds: input.retryAfterSeconds,
      },
      { headers, status: input.status },
    )
  }

  return new Response(
    `<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>OpenAgents sign-in</title></head><body style="margin:0;background:#050505;color:#f1efe8;font-family:ui-sans-serif,system-ui,sans-serif;display:grid;min-height:100vh;place-items:center"><main style="width:min(420px,calc(100vw - 48px));border:1px solid #2b2a26;padding:28px;background:#0b0b0a"><p style="margin:0 0 10px;color:#8b8880;font-size:12px;text-transform:uppercase;letter-spacing:.12em">OpenAgents</p><h1 style="margin:0 0 12px;font-size:22px">Sign-in code unavailable</h1><p style="margin:0 0 22px;color:#c9c6bd;line-height:1.55">${authEmailOtpEscapeHtml(input.message)}</p><a href="/login" style="color:#d6f6ff">Back to login</a></main></body></html>`,
    {
      headers: {
        ...Object.fromEntries(headers),
        'content-type': 'text/html; charset=utf-8',
      },
      status: input.status,
    },
  )
}

const authEmailOtpRateLimitResponse = (
  request: Request,
  decision: AuthEmailOtpRateLimitRejected,
) =>
  authEmailOtpProblemResponse(request, {
    error: 'auth_email_otp_rate_limited',
    message: `Too many sign-in code requests. Try again in ${decision.retryAfterSeconds} seconds.`,
    retryAfterSeconds: decision.retryAfterSeconds,
    status: 429,
  })

const maybeAuthEmailOtpGuardResponse = async (request: Request, env: Env) => {
  const url = new URL(request.url)

  if (request.method !== 'POST' || url.pathname !== '/code/authorize') {
    return undefined
  }

  const formData = await request
    .clone()
    .formData()
    .catch((): undefined => undefined)
  const sendForm =
    formData === undefined ? undefined : authEmailOtpSendForm(formData)

  if (sendForm === undefined || !SIMPLE_EMAIL_PATTERN.test(sendForm.email)) {
    return undefined
  }

  const decision = await reserveAuthEmailOtpSend(
    openAgentsDatabase(env),
    {
      email: sendForm.email,
      ipAddress: authEmailOtpClientIp(request),
    },
    workerRuntime,
  ).catch(error => {
    logWorkerRouteError('auth_email_otp_rate_limit_failed', error, {
      errorName: errorName(error),
    })

    return undefined
  })

  if (decision === undefined) {
    return authEmailOtpProblemResponse(request, {
      error: 'auth_email_otp_temporarily_unavailable',
      message: AUTH_EMAIL_OTP_SEND_UNAVAILABLE_MESSAGE,
      status: 503,
    })
  }

  return decision._tag === 'RateLimited'
    ? authEmailOtpRateLimitResponse(request, decision)
    : undefined
}

const readUserKindTotals = async (db: D1Database): Promise<UserKindTotals> => {
  const rows = await db
    .prepare(
      `SELECT kind, COUNT(*) AS count
       FROM users
       WHERE status = 'active'
         AND deleted_at IS NULL
       GROUP BY kind`,
    )
    .all<Readonly<{ kind: 'human' | 'agent'; count: number }>>()
  const countFor = (kind: 'human' | 'agent') =>
    rows.results.find(row => row.kind === kind)?.count ?? 0
  const humans = countFor('human')
  const agents = countFor('agent')

  return {
    admins: OPENAGENTS_ADMIN_EMAILS.length,
    adminEmails: [...OPENAGENTS_ADMIN_EMAILS],
    humans,
    agents,
    total: humans + agents,
  }
}

const TEAM_AUTOPILOT_COMMAND = '@autopilot'
const TEAM_ADJUTANT_COMMAND = '@adjutant'
const ADJUTANT_PROJECT_ID = 'project_adjutant'
const ADJUTANT_INTENT_SCHEMA_VERSION = 'openagents.team_chat.adjutant_intent.v1'
const softwareOrderIdPattern = /^software_order_[A-Za-z0-9_-]+$/
const siteIdPattern = /^site_project_[A-Za-z0-9_-]+$/
const taskSpecPathPattern =
  /^docs\/autopilot-tasks\/[A-Za-z0-9][A-Za-z0-9._/-]*\.md$/
const softwareOrderTokenPattern =
  /(?:^|[\s,;])(?:softwareOrderId|software_order_id|orderId|order)\s*[:=]\s*(software_order_[A-Za-z0-9_-]+)/gi
const siteTokenPattern =
  /(?:^|[\s,;])(?:siteId|site_id|site)\s*[:=]\s*(site_project_[A-Za-z0-9_-]+)/gi
const taskSpecTokenPattern =
  /(?:^|[\s,;])(?:taskSpecPath|task_spec_path|taskPacketPath|task_packet_path|task)\s*[:=]\s*(docs\/autopilot-tasks\/[A-Za-z0-9][A-Za-z0-9._/-]*\.md)/gi

export type TeamAdjutantIntent = Readonly<{
  schemaVersion: typeof ADJUTANT_INTENT_SCHEMA_VERSION
  prompt: string
  softwareOrderId?: string | undefined
  siteId?: string | undefined
  taskSpecPath?: string | undefined
}>

export const teamAutopilotPromptFromBody = (messageBody: string): string => {
  const trimmed = messageBody.trim()
  const lower = trimmed.toLowerCase()
  const prefix = `${TEAM_AUTOPILOT_COMMAND} `

  if (lower.startsWith(prefix)) {
    const prompt = trimmed.slice(TEAM_AUTOPILOT_COMMAND.length).trim()

    return prompt === '' ? messageBody : prompt
  }

  const lines = messageBody.split(/\r?\n/)
  const commandLineIndex = lines.findIndex(
    line => line.trim().toLowerCase() === TEAM_AUTOPILOT_COMMAND,
  )

  if (commandLineIndex !== -1) {
    const prompt = [
      ...lines.slice(0, commandLineIndex),
      ...lines.slice(commandLineIndex + 1),
    ]
      .join('\n')
      .trim()

    return prompt === '' ? messageBody : prompt
  }

  const suffix = ` ${TEAM_AUTOPILOT_COMMAND}`

  if (lower.endsWith(suffix)) {
    const prompt = trimmed.slice(0, -TEAM_AUTOPILOT_COMMAND.length).trim()

    return prompt === '' ? messageBody : prompt
  }

  return messageBody
}

const exactTeamCommandPromptFromBody = (
  messageBody: string,
  command: string,
): string | undefined => {
  const trimmed = messageBody.trim()
  const lower = trimmed.toLowerCase()
  const prefix = `${command} `

  if (lower.startsWith(prefix)) {
    return trimmed.slice(command.length).trim()
  }

  const lines = messageBody.split(/\r?\n/)
  const commandLineIndex = lines.findIndex(
    line => line.trim().toLowerCase() === command,
  )

  if (commandLineIndex !== -1) {
    return [
      ...lines.slice(0, commandLineIndex),
      ...lines.slice(commandLineIndex + 1),
    ]
      .join('\n')
      .trim()
  }

  const suffix = ` ${command}`

  if (lower.endsWith(suffix)) {
    return trimmed.slice(0, -command.length).trim()
  }

  return undefined
}

const exactTeamAdjutantPromptFromBody = (
  messageBody: string,
): string | undefined =>
  exactTeamCommandPromptFromBody(messageBody, TEAM_AUTOPILOT_COMMAND) ??
  exactTeamCommandPromptFromBody(messageBody, TEAM_ADJUTANT_COMMAND)

const firstRegexCapture = (
  pattern: RegExp,
  value: string,
): string | undefined => {
  pattern.lastIndex = 0
  const match = pattern.exec(value)
  pattern.lastIndex = 0

  return match?.[1]
}

const cleanAdjutantPrompt = (prompt: string): string =>
  prompt
    .replace(softwareOrderTokenPattern, ' ')
    .replace(siteTokenPattern, ' ')
    .replace(taskSpecTokenPattern, ' ')
    .replace(/[ \t]{2,}/g, ' ')
    .replace(/\n{3,}/g, '\n\n')
    .trim()

const boundedTaskSpecPath = (value: string | undefined): string | undefined =>
  value !== undefined &&
  taskSpecPathPattern.test(value) &&
  !value.includes('..') &&
  !value.includes('//')
    ? value
    : undefined

const explicitAdjutantContextString = (
  value: unknown,
  pattern: RegExp,
): string | undefined => {
  const text = optionalString(value)

  return text !== undefined && pattern.test(text) ? text : undefined
}

export const teamAdjutantIntentFromBody = (
  messageBody: string,
): TeamAdjutantIntent | undefined => {
  const prompt = exactTeamAdjutantPromptFromBody(messageBody)

  if (prompt === undefined) {
    return undefined
  }

  const softwareOrderId = firstRegexCapture(softwareOrderTokenPattern, prompt)
  const siteId = firstRegexCapture(siteTokenPattern, prompt)
  const taskSpecPath = boundedTaskSpecPath(
    firstRegexCapture(taskSpecTokenPattern, prompt),
  )
  const cleanPrompt = cleanAdjutantPrompt(prompt)

  return {
    schemaVersion: ADJUTANT_INTENT_SCHEMA_VERSION,
    prompt: cleanPrompt === '' ? prompt : cleanPrompt,
    ...(softwareOrderId === undefined ? {} : { softwareOrderId }),
    ...(siteId === undefined ? {} : { siteId }),
    ...(taskSpecPath === undefined ? {} : { taskSpecPath }),
  }
}

const teamAutopilotAnswerBackId = (runId: string): string =>
  `team_chat_answer_${runId}`

const genericAutopilotEventText = new Set([
  'Assistant message completed.',
  'Codex one-shot run completed.',
  'Codex one-shot turn completed.',
  'Codex run resource usage receipt emitted.',
  'Codex workspace removed.',
  'Closeout receipt emitted.',
  'OpenCode run completed and closeout manifest submitted.',
  'OpenCode/Codex one-shot run completed.',
  'OpenCode/Codex one-shot turn completed.',
  'OpenCode/Codex run finished with status completed.',
  'Codex VM artifact captured.',
  'Codex VM receipt emitted.',
  'Runner event received.',
  'stdout JSON event captured.',
])

const internalAutopilotAnswerPhrases = [
  'closeout receipt',
  'closeout manifest',
  'completion artifact',
  'completion artifacts',
  'github-writeback.json',
  'local artifact',
  'local artifacts',
  'record the requested summary',
  'result.md',
  'run artifact',
  'run artifacts',
  'run outcome',
  'usage receipt',
  'workspace removed',
]

const isVisibleAutopilotAnswerText = (text: string): boolean => {
  const compact = compactTeamContextText(text, 3_800)
  const normalized = compact.toLowerCase()

  if (genericAutopilotEventText.has(compact)) {
    return false
  }

  if (
    internalAutopilotAnswerPhrases.some(phrase => normalized.includes(phrase))
  ) {
    return false
  }

  if (
    normalized.includes('artifact') &&
    (normalized.includes('required') ||
      normalized.includes('prepare') ||
      normalized.includes('write') ||
      normalized.includes('adding') ||
      normalized.includes('record'))
  ) {
    return false
  }

  return true
}

const eventPayloadRecord = (
  event: Readonly<{ payloadJson: string | null }>,
): Record<string, unknown> | undefined => safeJsonRecord(event.payloadJson)

const rawEventPayloadRecord = (
  event: Readonly<{ payloadJson: string | null }>,
): Record<string, unknown> | undefined => {
  const payload = eventPayloadRecord(event)
  const dataJson = optionalString(payload?.dataJson)
  const rawPayloadJson =
    optionalString(payload?.rawPayloadJson) ??
    optionalString(payload?.raw_payload_json)

  return safeJsonRecord(dataJson) ?? safeJsonRecord(rawPayloadJson) ?? payload
}

const eventRawPart = (
  event: Readonly<{ payloadJson: string | null }>,
): Record<string, unknown> | undefined => {
  const raw = rawEventPayloadRecord(event)
  const direct = raw?.part
  const propertiesPart = nestedUnknown(raw, ['properties', 'part'])

  return isRecord(direct)
    ? direct
    : isRecord(propertiesPart)
      ? propertiesPart
      : undefined
}

const eventLooksLikeToolCall = (
  event: Readonly<{ payloadJson: string | null; type: string }>,
): boolean => {
  const raw = rawEventPayloadRecord(event)
  const rawType = optionalString(raw?.type)
  const part = eventRawPart(event)

  return (
    event.type.includes('tool') ||
    rawType === 'tool_use' ||
    rawType === 'tool_result' ||
    optionalString(raw?.tool) !== undefined ||
    optionalString(part?.tool) !== undefined
  )
}

const eventTokenTotal = (
  event: Readonly<{ payloadJson: string | null }>,
): number => {
  const raw = rawEventPayloadRecord(event)
  const total =
    optionalInteger(nestedUnknown(raw, ['usage', 'totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'total_tokens'])) ??
    optionalInteger(nestedUnknown(raw, ['tokenUsage', 'totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['token_usage', 'total_tokens'])) ??
    optionalInteger(nestedUnknown(raw, ['totalTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['total_tokens']))

  if (total !== undefined) {
    return total
  }

  const input =
    optionalInteger(nestedUnknown(raw, ['usage', 'inputTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'input_tokens'])) ??
    0
  const output =
    optionalInteger(nestedUnknown(raw, ['usage', 'outputTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'output_tokens'])) ??
    0
  const reasoning =
    optionalInteger(nestedUnknown(raw, ['usage', 'reasoningTokens'])) ??
    optionalInteger(nestedUnknown(raw, ['usage', 'reasoning_tokens'])) ??
    0

  return input + output + reasoning
}

const durationSecondsForRun = (
  run: Pick<
    AgentRunRecord,
    | 'canceledAt'
    | 'completedAt'
    | 'createdAt'
    | 'failedAt'
    | 'startedAt'
    | 'updatedAt'
  >,
): number | null => {
  const startedAt = Date.parse(run.startedAt ?? run.createdAt)
  const endedAt = Date.parse(
    run.completedAt ?? run.failedAt ?? run.canceledAt ?? run.updatedAt,
  )

  return Number.isFinite(startedAt) && Number.isFinite(endedAt)
    ? Math.max(0, Math.round((endedAt - startedAt) / 1000))
    : null
}

const teamChatRunSummaryFromBundle = (
  bundle: AgentRunBundle,
): TeamChatRunSummary => ({
  runId: bundle.run.id,
  status: bundle.run.status,
  runtime: bundle.run.runtime,
  backend: bundle.run.backend,
  repository: `${bundle.run.repository.owner}/${bundle.run.repository.repo}@${bundle.run.repository.ref}`,
  eventCount: bundle.events.length,
  toolCallCount: bundle.events.filter(eventLooksLikeToolCall).length,
  tokenTotal: bundle.events.reduce(
    (total, event) => total + eventTokenTotal(event),
    0,
  ),
  durationSeconds: durationSecondsForRun(bundle.run),
  updatedAt: bundle.run.updatedAt,
})

const assistantAnswerTextFromEvent = (
  event: Readonly<{
    payloadJson: string | null
    summary: string
    type: string
  }>,
): string | undefined => {
  const payload = rawEventPayloadRecord(event)
  const rawType =
    optionalString(payload?.type) ??
    optionalString(payload?.event) ??
    event.type

  if (
    event.type !== 'message.completed' &&
    event.type !== 'message.part.updated' &&
    rawType !== 'message.completed' &&
    rawType !== 'message.part.updated' &&
    rawType !== 'text'
  ) {
    return undefined
  }

  const propertiesPart = nestedUnknown(payload, ['properties', 'part'])
  const directPart = nestedUnknown(payload, ['part'])
  const part = isRecord(propertiesPart)
    ? propertiesPart
    : isRecord(directPart)
      ? directPart
      : undefined
  const text =
    (rawType === 'message.part.updated' || rawType === 'text') &&
    (optionalString(part?.type) ?? 'text') === 'text'
      ? (optionalString(part?.text) ?? optionalString(part?.content))
      : (optionalNestedString(payload, [
          ['finalAnswer'],
          ['final_answer'],
          ['answer'],
          ['text'],
          ['detail'],
          ['message'],
          ['content'],
          ['output'],
          ['response', 'output_text'],
          ['properties', 'part', 'text'],
          ['properties', 'part', 'content'],
          ['part', 'text'],
          ['part', 'content'],
          ['item', 'text'],
          ['item', 'message'],
          ['item', 'content', '0', 'text'],
        ]) ?? (event.type === 'message.completed' ? event.summary : undefined))
  const compact =
    text === undefined ? undefined : compactTeamContextText(text, 3_800)

  return compact === undefined || !isVisibleAutopilotAnswerText(compact)
    ? undefined
    : compact
}

export const teamAutopilotAnswerBackDraft = (
  bundle: Pick<AgentRunBundle, 'events'>,
): Readonly<{ body: string; sourceEventId: string | null }> => {
  const chronological = [...bundle.events].sort((left, right) =>
    left.sequence === right.sequence
      ? Date.parse(left.createdAt) - Date.parse(right.createdAt)
      : left.sequence - right.sequence,
  )
  const candidates = chronological
    .map(event => ({
      body: assistantAnswerTextFromEvent(event),
      sourceEventId: event.id,
    }))
    .filter(
      (value): value is Readonly<{ body: string; sourceEventId: string }> =>
        value.body !== undefined,
    )
  const candidate = candidates[candidates.length - 1]

  return candidate === undefined
    ? {
        body: 'Autopilot completed the run. Open the linked thread for the full transcript.',
        sourceEventId: null,
      }
    : candidate
}

const resultArtifactEventId = (
  events: ReadonlyArray<OmniEventRecord>,
): string | undefined => {
  const candidates = [...events]
    .sort((left, right) =>
      left.sequence === right.sequence
        ? Date.parse(left.createdAt) - Date.parse(right.createdAt)
        : left.sequence - right.sequence,
    )
    .filter(event => {
      const raw = rawEventPayloadRecord(event)
      const detail = optionalString(raw?.detail)
      const artifactName = optionalString(raw?.artifactName)
      const filename = optionalString(raw?.filename)

      return (
        event.type === 'artifact.created' &&
        (detail === 'result.md' ||
          artifactName === 'result.md' ||
          filename === 'result.md')
      )
    })

  return candidates[candidates.length - 1]?.id
}

const githubContentsUrl = (
  repository: AgentRunRecord['repository'],
  branchName: string,
  path: string,
): string | undefined => {
  if (repository.provider !== 'github') {
    return undefined
  }

  const owner = encodeURIComponent(repository.owner)
  const repo = encodeURIComponent(repository.repo)
  const encodedPath = path.split('/').map(encodeURIComponent).join('/')
  const ref = encodeURIComponent(branchName)

  return `https://api.github.com/repos/${owner}/${repo}/contents/${encodedPath}?ref=${ref}`
}

const textFromGithubContentsResponse = (
  payload: unknown,
): string | undefined => {
  if (!isRecord(payload)) {
    return undefined
  }

  const encoding = optionalString(payload.encoding)
  const content = optionalString(payload.content)

  if (encoding !== 'base64' || content === undefined) {
    return undefined
  }

  try {
    const decoded = atob(content.replace(/\s+/g, ''))
    const bytes = Uint8Array.from(decoded, char => char.charCodeAt(0))

    return new TextDecoder().decode(bytes)
  } catch {
    return undefined
  }
}

export const teamAutopilotResultArtifactAnswerBackDraft = async (
  bundle: AgentRunBundle,
  options: Readonly<{
    githubAccessToken?: string | undefined
  }> = {},
): Promise<Readonly<{ body: string; sourceEventId: string }> | undefined> => {
  const sourceEventId = resultArtifactEventId(bundle.events)
  const branchName = bundle.run.assignment.githubWorkOrder?.branchName

  if (sourceEventId === undefined || branchName === undefined) {
    return undefined
  }

  const url = githubContentsUrl(bundle.run.repository, branchName, 'result.md')

  if (url === undefined) {
    return undefined
  }

  const response = await fetch(url, {
    headers: {
      accept: 'application/vnd.github+json',
      ...(options.githubAccessToken === undefined
        ? {}
        : { authorization: `Bearer ${options.githubAccessToken}` }),
      'user-agent': 'openagents-autopilot-worker',
    },
  }).catch((): Response | undefined => undefined)

  if (response === undefined || !response.ok) {
    return undefined
  }

  const payload = await response.json().catch((): unknown => undefined)
  const text = textFromGithubContentsResponse(payload)
  const body =
    text === undefined ? undefined : compactTeamContextText(text, 3_800)

  return body === undefined || !isVisibleAutopilotAnswerText(body)
    ? undefined
    : {
        body,
        sourceEventId,
      }
}

export const teamAutopilotAnswerBackDraftForBundle = async (
  bundle: AgentRunBundle,
  options: Readonly<{
    githubAccessToken?: string | undefined
  }> = {},
): Promise<Readonly<{ body: string; sourceEventId: string | null }>> =>
  (await teamAutopilotResultArtifactAnswerBackDraft(bundle, options)) ??
  teamAutopilotAnswerBackDraft(bundle)

const selectedFileIdsFromTeamMessageMetadata = (
  metadataJson: string,
): ReadonlyArray<string> => {
  const metadata = safeJsonRecord(metadataJson)
  const direct = stringArrayFromUnknown(metadata?.selectedTeamFileIds)
  const nestedIds = stringArrayFromUnknown(
    nestedUnknown(metadata, ['context', 'selectedTeamFileIds']),
  )

  return direct.length > 0 ? direct : nestedIds
}

const appendTeamAutopilotAnswerBack = async (
  env: Env,
  ctx: SyncNotificationContext,
  runId: string,
): Promise<void> => {
  const parent = await readTeamChatMessageByAgentRunId(
    openAgentsDatabase(env),
    runId,
  )

  if (parent === undefined) {
    return
  }

  const answerId = teamAutopilotAnswerBackId(runId)
  const existing = await readTeamChatMessageById(
    openAgentsDatabase(env),
    answerId,
  )

  if (existing !== undefined) {
    return
  }

  const bundle = await makeD1OmniRunStore(
    openAgentsDatabase(env),
  ).findAgentRunForUser(parent.message.author.userId, runId)

  if (bundle === undefined || bundle.run.status !== 'completed') {
    return
  }

  const updatedParent = await updateTeamChatMessageRunSummary(
    openAgentsDatabase(env),
    {
      messageId: parent.message.id,
      metadataJson: parent.metadataJson,
      runSummary: teamChatRunSummaryFromBundle(bundle),
    },
  )

  if (updatedParent !== undefined) {
    await publishTeamChatMessageSync(
      env,
      ctx,
      updatedParent,
      parent.message.author.userId,
    )
  }

  const githubAccessToken =
    bundle.run.assignment.githubWriteConnectionRef === undefined
      ? null
      : await env.AUTH_STORAGE.get(
          githubWriteSecretKey(bundle.run.assignment.githubWriteConnectionRef),
        )
  const draft = await teamAutopilotAnswerBackDraftForBundle(bundle, {
    ...(githubAccessToken === null ? {} : { githubAccessToken }),
  })
  const selectedTeamFileIds = selectedFileIdsFromTeamMessageMetadata(
    parent.metadataJson,
  )
  const message = await insertTeamChatMessage(openAgentsDatabase(env), {
    agentRunId: runId,
    authorUserId: parent.message.author.userId,
    body: draft.body,
    id: answerId,
    kind: 'system',
    metadataJson: JSON.stringify({
      agentRunId: runId,
      kind: 'autopilot_answer_back',
      parentTeamChatMessageId: parent.message.id,
      selectedTeamFileIds,
      sourceEventId: draft.sourceEventId,
    }),
    ...(parent.message.autopilotThreadId === null
      ? {}
      : { autopilotThreadId: parent.message.autopilotThreadId }),
    ...(parent.message.projectId === null
      ? {}
      : { projectId: parent.message.projectId }),
    teamId: parent.message.teamId,
  }).catch(async error => {
    const replayed = await readTeamChatMessageById(
      openAgentsDatabase(env),
      answerId,
    )

    if (replayed !== undefined) {
      return undefined
    }

    throw error
  })

  if (message === undefined) {
    return
  }

  await insertThreadFileMessageReferences(openAgentsDatabase(env), {
    fileIds: selectedTeamFileIds,
    messageId: message.id,
    referenceKind: 'autopilot_answer',
    teamId: parent.message.teamId,
    threadId:
      parent.message.autopilotThreadId ??
      (parent.message.projectId === null
        ? teamChatThreadId(parent.message.teamId)
        : teamProjectChatThreadId(
            parent.message.teamId,
            parent.message.projectId,
          )),
  })

  await publishTeamChatMessageSync(
    env,
    ctx,
    message,
    parent.message.author.userId,
  )
}

const TEAM_AUTOPILOT_FILE_CONTEXT_MAX_BYTES = 256 * 1024
const TEAM_AUTOPILOT_FILE_CONTEXT_MAX_CHARS = 24_000

const threadFileLooksTextLike = (row: ThreadFileRow): boolean => {
  const contentType = row.content_type.toLowerCase()
  const filename = row.filename.toLowerCase()

  return (
    contentType.startsWith('text/') ||
    contentType.includes('json') ||
    contentType.includes('markdown') ||
    contentType.includes('xml') ||
    contentType.includes('yaml') ||
    contentType.includes('csv') ||
    /\.(txt|md|markdown|json|jsonl|csv|tsv|xml|yaml|yml|log)$/i.test(filename)
  )
}

const teamAutopilotFileExcerpt = async (
  env: Env,
  row: ThreadFileRow,
): Promise<string | undefined> => {
  if (
    !threadFileLooksTextLike(row) ||
    row.size_bytes > TEAM_AUTOPILOT_FILE_CONTEXT_MAX_BYTES
  ) {
    return undefined
  }

  return Effect.runPromise(
    Effect.gen(function* () {
      const artifacts = yield* ThreadFileArtifacts
      const maybeObject = yield* artifacts.get(row.object_key)
      const object = Option.getOrUndefined(maybeObject)

      if (object === undefined) {
        return undefined
      }

      const text = yield* object.text

      return compactTeamContextText(text, TEAM_AUTOPILOT_FILE_CONTEXT_MAX_CHARS)
    }).pipe(
      Effect.provide(
        ThreadFileArtifacts.layer({ binding: 'ARTIFACTS' }).pipe(
          Layer.provide(Layer.succeed(WorkerEnvironment, env)),
        ),
      ),
      Effect.catchTag('R2OperationError', () =>
        Effect.sync((): undefined => undefined),
      ),
    ),
  )
}

const hydrateTeamAutopilotContextFileExcerpts = async (
  env: Env,
  bundle: TeamAutopilotContextBundle,
): Promise<TeamAutopilotContextBundle> => {
  const selectedFiles = await Promise.all(
    bundle.selectedFiles.map(async file => {
      const row = await readThreadFileById(openAgentsDatabase(env), file.id)
      const excerpt =
        row === undefined
          ? undefined
          : await teamAutopilotFileExcerpt(env, row).catch(() => undefined)

      return excerpt === undefined ? file : { ...file, excerpt }
    }),
  )

  return { ...bundle, selectedFiles }
}

export const authIssuerAllowsRedirectHostname = (hostname: string): boolean =>
  hostname === 'openagents.com' ||
  hostname === 'auth.openagents.com' ||
  // Isolated staging Worker. WIDEN-ONLY: this lets the prod issuer accept the
  // staging-origin auth callback so a human can sign in on staging and exercise
  // the billing/credit flow. The staging Worker delegates auth to this same
  // prod issuer (OPENAUTH_ISSUER_URL=auth.openagents.com), so the allowlist must
  // live here. Prod hosts above are unchanged.
  hostname === 'openagents-staging.openagents.workers.dev' ||
  hostname === 'localhost' ||
  hostname === '127.0.0.1'

const makeAuthIssuer = (env: Env) => {
  const config = getOpenAgentsWorkerConfig(env)
  const emailCodeUi = CodeUI({
    copy: {
      code_info: 'We sent a one-time sign-in code to your email.',
      email_invalid: AUTH_EMAIL_OTP_SEND_UNAVAILABLE_MESSAGE,
      email_placeholder: 'you@example.com',
    },
    sendCode: async () => undefined,
  })

  return issuer({
    // Full OpenAgents-branded auth theme (replaces the OpenAuth defaults: the grid
    // logo, openauth.js.org favicon, IBM Plex font, and #6772e5 purple accent).
    theme: {
      title: 'OpenAgents',
      radius: 'none' as const,
      primary: { light: '#0a0a0a', dark: '#f1efe8' },
      background: { light: '#ffffff', dark: '#000000' },
      font: {
        family: 'ui-monospace, SFMono-Regular, Menlo, Consolas, monospace',
      },
      logo: {
        light:
          'data:image/svg+xml,%3Csvg%20xmlns%3D%27http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%27%20width%3D%27240%27%20height%3D%2740%27%20viewBox%3D%270%200%20240%2040%27%3E%3Ctext%20x%3D%27120%27%20y%3D%2730%27%20text-anchor%3D%27middle%27%20font-family%3D%27ui-monospace%2CSFMono-Regular%2CMenlo%2CConsolas%2Cmonospace%27%20font-size%3D%2729%27%20font-weight%3D%27700%27%20letter-spacing%3D%27-1.5%27%20fill%3D%27%230a0a0a%27%3EOpenAgents%3C%2Ftext%3E%3C%2Fsvg%3E',
        dark: 'data:image/svg+xml,%3Csvg%20xmlns%3D%27http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%27%20width%3D%27240%27%20height%3D%2740%27%20viewBox%3D%270%200%20240%2040%27%3E%3Ctext%20x%3D%27120%27%20y%3D%2730%27%20text-anchor%3D%27middle%27%20font-family%3D%27ui-monospace%2CSFMono-Regular%2CMenlo%2CConsolas%2Cmonospace%27%20font-size%3D%2729%27%20font-weight%3D%27700%27%20letter-spacing%3D%27-1.5%27%20fill%3D%27%23f1efe8%27%3EOpenAgents%3C%2Ftext%3E%3C%2Fsvg%3E',
      },
      favicon:
        'data:image/svg+xml,%3Csvg%20xmlns%3D%27http%3A%2F%2Fwww.w3.org%2F2000%2Fsvg%27%20width%3D%2764%27%20height%3D%2764%27%20viewBox%3D%270%200%2064%2064%27%3E%3Crect%20width%3D%2764%27%20height%3D%2764%27%20fill%3D%27%23000000%27%2F%3E%3Ctext%20x%3D%2732%27%20y%3D%2744%27%20text-anchor%3D%27middle%27%20font-family%3D%27ui-monospace%2CSFMono-Regular%2CMenlo%2CConsolas%2Cmonospace%27%20font-size%3D%2734%27%20font-weight%3D%27700%27%20letter-spacing%3D%27-2%27%20fill%3D%27%23f1efe8%27%3Eoa%3C%2Ftext%3E%3C%2Fsvg%3E',
    },
    providers: {
      github: GithubProvider({
        clientID: config.github.clientId,
        clientSecret: redactedValue(config.github.clientSecret) ?? '',
        scopes: [...GITHUB_LOGIN_SCOPES],
      }),
      code: CodeProvider({
        ...emailCodeUi,
        sendCode: async (claims, code) => {
          const email =
            typeof claims.email === 'string'
              ? normalizeAuthEmailOtpEmail(claims.email)
              : ''
          if (email === '' || !SIMPLE_EMAIL_PATTERN.test(email)) {
            return invalidAuthEmailOtpClaim(email)
          }
          claims.email = email
          stampAuthEmailOtpClaims(claims, workerRuntime)

          return sendSignInCodeEmail(env, email, code)
            .then(() => undefined)
            .catch(error => {
              logWorkerRouteError('auth_email_otp_send_failed', error, {
                errorName: errorName(error),
              })

              return invalidAuthEmailOtpClaim(email)
            })
        },
      }),
    },
    storage: makeD1Storage(openAgentsDatabase(env)),
    subjects,
    allow: async ({ redirectURI }) => {
      const hostname = new URL(redirectURI).hostname

      return authIssuerAllowsRedirectHostname(hostname)
    },
    success: async (ctx, response) => {
      if (response.provider === 'code') {
        const claimedEmail =
          typeof response.claims.email === 'string' ? response.claims.email : ''
        if (claimedEmail === '') {
          throw new UnsupportedAuthProvider({ provider: 'code' })
        }
        if (!authEmailOtpClaimsAreFresh(response.claims, workerRuntime)) {
          throw new AuthSignInError({
            reason: 'Email sign-in code expired',
          })
        }
        const subject = emailToSubject(claimedEmail)
        await upsertEmailUser(openAgentsDatabase(env), subject)

        return ctx.subject('user', subject, {
          subject: subject.userId,
          ttl: {
            access: SESSION_MAX_AGE_SECONDS,
            refresh: SESSION_MAX_AGE_SECONDS,
          },
        })
      }

      // Only the github + code providers are registered; code handled above.
      const [user, emails] = await Promise.all([
        fetchGitHubJson(
          GitHubUser,
          'https://api.github.com/user',
          response.tokenset.access,
        ),
        fetchGitHubJson(
          GitHubEmails,
          'https://api.github.com/user/emails',
          response.tokenset.access,
        ),
      ])

      const subject = githubUserToSubject(user, getPrimaryVerifiedEmail(emails))
      await upsertGitHubUser(openAgentsDatabase(env), subject)
      await env.AUTH_STORAGE.put(
        githubIdentityTokenKey(subject.userId),
        response.tokenset.access,
        { expirationTtl: SESSION_MAX_AGE_SECONDS },
      )

      return ctx.subject('user', subject, {
        subject: subject.userId,
        ttl: {
          access: SESSION_MAX_AGE_SECONDS,
          refresh: SESSION_MAX_AGE_SECONDS,
        },
      })
    },
  })
}

const makeIssuerAwareFetch =
  (env: Env, ctx: ExecutionContext) =>
  async (
    input: Parameters<typeof fetch>[0],
    init?: Parameters<typeof fetch>[1],
  ): Promise<Response> => {
    const request = new Request(input, init)
    const url = new URL(request.url)
    const issuerHost = new URL(getIssuerOrigin(env)).hostname

    if (url.hostname === issuerHost) {
      return routeAuthIssuerRequest(request, env, ctx)
    }

    return fetch(request)
  }

const makeAuthClient = (env: Env, ctx: ExecutionContext) => {
  const config = getOpenAgentsWorkerConfig(env)

  return createClient({
    clientID: config.openauth.clientId,
    issuer: getIssuerOrigin(env),
    fetch: makeIssuerAwareFetch(env, ctx),
  })
}

type VerifiedSession = VerifiedAuthSession<UserSubject>

const verifySession = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<VerifiedSession | undefined> => {
  const cookies = parseCookies(request)
  const access = cookies.get(ACCESS_COOKIE)

  if (access === undefined) {
    return undefined
  }

  const refresh = cookies.get(REFRESH_COOKIE)
  const verified = await observedPromise('Auth.verifySession', () =>
    refresh === undefined
      ? makeAuthClient(env, ctx).verify(subjects, access)
      : makeAuthClient(env, ctx).verify(subjects, access, { refresh }),
  ).catch(error => {
    logWorkerRouteError('auth_session_verify_failed', error)

    return undefined
  })

  if (verified === undefined) {
    return undefined
  }

  if (verified.err !== undefined) {
    return undefined
  }

  if (verified.subject.type !== 'user') {
    return undefined
  }

  if (verified.tokens === undefined) {
    return { user: verified.subject.properties }
  }

  return { user: verified.subject.properties, tokens: verified.tokens }
}

const scheduleSiteReferralOnboardingEmail = (
  ctx: ExecutionContext,
  env: EmailCampaignDispatcherBindings,
  session: VerifiedSession,
  referralResult: ReferralConsumptionResult,
  orderState: OnboardingDripOrderState,
): void => {
  if (referralResult._tag !== 'consumed') {
    return
  }

  scheduleBackgroundWork(
    ctx,
    sendSiteReferralOnboardingForConsumption(openAgentsDatabase(env), {
      appOrigin: getAppOrigin(env),
      displayName: session.user.name,
      email: session.user.email,
      orderState,
      referralResult,
      resend: getResendEmailConfig(env),
      userId: session.user.userId,
    }).catch(error =>
      logWorkerRouteError('site_referral_onboarding_failed', error, {
        userId: session.user.userId,
      }),
    ),
  )
}

const { appendRefreshedSessionCookies, requireBrowserSession } =
  makeBrowserSessionBoundary<UserSubject, Env>({
    persistUser: (env, user) => upsertUser(openAgentsDatabase(env), user),
    verifySession,
  })

const authenticateRequestActor = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<AuthenticatedActor | undefined> => {
  const bearerToken = readBearerToken(request)

  if (bearerToken !== undefined) {
    const agent = await authenticateProgrammaticAgent(
      makeD1AgentRegistrationStore(openAgentsDatabase(env)),
      bearerToken,
    )

    if (agent !== undefined) {
      return { kind: 'agent', agent }
    }
  }

  const session = await verifySession(request, env, ctx)

  if (session === undefined) {
    return undefined
  }

  await upsertUser(openAgentsDatabase(env), session.user)

  if (session.tokens === undefined) {
    return {
      kind: 'human',
      user: session.user,
    }
  }

  return {
    kind: 'human',
    user: session.user,
    tokens: session.tokens,
  }
}

const actorJson = (actor: AuthenticatedActor) => {
  if (actor.kind === 'agent') {
    return {
      kind: actor.kind,
      userId: actor.agent.user.id,
      displayName: actor.agent.user.displayName,
      credentialId: actor.agent.credential.id,
      tokenPrefix: actor.agent.credential.tokenPrefix,
    }
  }

  return {
    kind: actor.kind,
    userId: actor.user.userId,
    login: actor.user.login,
    email: actor.user.email,
    name: actor.user.name,
  }
}

const cloneResponseWithHeaders = (
  response: Response,
  mutateHeaders: (headers: Headers) => void,
): Response => {
  const headers = new Headers(response.headers)
  mutateHeaders(headers)

  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  })
}

const handleAppShellPage = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  const cookies = parseCookies(request)
  const hadSessionCookie =
    cookies.has(ACCESS_COOKIE) || cookies.has(REFRESH_COOKIE)
  const session = await verifySession(request, env, ctx)
  const tokens = session?.tokens
  const assetResponse = await fetchAppShellWithPylonStatsBootPayload(
    request,
    env,
  )

  if (tokens !== undefined) {
    return cloneResponseWithHeaders(assetResponse, headers => {
      appendSessionCookies(headers, tokens)
    })
  }

  if (hadSessionCookie && session === undefined) {
    return cloneResponseWithHeaders(assetResponse, headers => {
      appendClearSessionCookies(headers, new URL(request.url).hostname)
    })
  }

  return assetResponse
}

const readAuthenticatedPageContext = async (
  env: Env,
  session: VerifiedSession,
): Promise<
  Readonly<{
    totals: UserKindTotals | undefined
    teams: ReadonlyArray<UserTeam>
    providerAccounts: ProviderAccountBundle
    githubWriteConnections: GitHubWriteConnectionBundle
    tokenLeaderboards: AutopilotTokenLeaderboards
    billing: BillingSummary
    onboarding: Awaited<ReturnType<typeof readOnboardingStatusForUser>>
  }>
> => {
  await upsertUser(openAgentsDatabase(env), session.user)

  const providerAccountRepository = makeD1ProviderAccountRepository(
    openAgentsDatabase(env),
  )
  const githubWriteRepository = makeD1GitHubWriteRepository(
    openAgentsDatabase(env),
  )
  const [
    maybeTotals,
    teams,
    providerAccounts,
    githubWriteConnections,
    tokenLeaderboards,
    billing,
    onboarding,
  ] = await Promise.all([
    isOpenAgentsAdminEmail(session.user.email)
      ? readUserKindTotals(openAgentsDatabase(env))
      : Promise.resolve(undefined),
    readTeamsForUser(openAgentsDatabase(env), session.user.userId),
    listProviderAccountsForUser(providerAccountRepository, session.user.userId),
    listGitHubWriteConnectionsForUser(
      githubWriteRepository,
      session.user.userId,
    ),
    readTokenUsageLeaderboardsForUser(env, session.user.userId),
    readBillingSummary(openAgentsDatabase(env), session.user.userId),
    readOnboardingStatusForUser(env, session.user.userId),
  ])

  return {
    totals: maybeTotals,
    teams,
    providerAccounts,
    githubWriteConnections,
    tokenLeaderboards,
    // Attach the purchasable credit catalog from the server Stripe config so the
    // billing page renders buy buttons whose packageId the checkout endpoint
    // accepts on first render, before any client-side billing refresh.
    billing: withBillingCreditPackages(billing, readBillingCreditPackages(env)),
    onboarding,
  }
}

const handleHomePage = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  const shellResponse = await handleAppShellPage(request, env, ctx)
  const headers = new Headers(shellResponse.headers)
  headers.append(
    'Link',
    '<https://openagents.com/api/public/home>; rel="alternate"; type="application/json"; title="OpenAgents homepage JSON"',
  )
  headers.append(
    'Link',
    '<https://openagents.com/.well-known/openagents.json>; rel="service-desc"; type="application/json"; title="OpenAgents capability manifest"',
  )
  headers.set(
    'X-OpenAgents-Homepage-Json',
    'https://openagents.com/api/public/home',
  )

  return new Response(shellResponse.body, {
    headers,
    status: shellResponse.status,
    statusText: shellResponse.statusText,
  })
}

const handlePublicHomeApi = (request: Request) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.succeed(
        noStoreJsonResponse({
          schemaVersion: 'openagents.public_home.v1',
          page: {
            canonicalUrl: 'https://openagents.com/',
            htmlUrl: 'https://openagents.com/',
            title: 'OpenAgents',
          },
          agentDiscovery: {
            homepageJson: 'https://openagents.com/api/public/home',
            productPromises:
              'https://openagents.com/api/public/product-promises',
            capabilityManifest:
              'https://openagents.com/.well-known/openagents.json',
            agentInstructions: 'https://openagents.com/AGENTS.md',
            openApi: 'https://openagents.com/api/openapi.json',
          },
          homepageData: [
            {
              id: 'capability_manifest',
              href: 'https://openagents.com/.well-known/openagents.json',
              method: 'GET',
              description:
                'Machine-readable OpenAgents capability manifest for agents and operators.',
            },
            {
              id: 'openapi',
              href: 'https://openagents.com/api/openapi.json',
              method: 'GET',
              description:
                'Machine-readable OpenAPI contract for public-safe API routes.',
            },
            {
              id: 'pylon_stats',
              href: 'https://openagents.com/api/public/pylon-stats',
              method: 'GET',
              description:
                'Pylon heartbeat, readiness, and receipt-gated accepted-work counters shown on the homepage.',
            },
            {
              id: 'product_promises',
              href: 'https://openagents.com/api/public/product-promises',
              method: 'GET',
              description:
                'Versioned OpenAgents product-promise registry for agents and users, including live, scoped, gated, degraded, and planned claim states.',
            },
            {
              id: 'forum_tip_leaderboards',
              href: 'https://openagents.com/api/forum/tip-leaderboards?limit=10',
              method: 'GET',
              description:
                'Public Forum tip paid and settled evidence rows shown on the homepage.',
            },
            {
              id: 'forum_launch_status',
              href: 'https://openagents.com/api/forum/launch-status',
              method: 'GET',
              description:
                'Forum posting, moderation/reporting, and tipping launch gates shown on the homepage.',
            },
            {
              id: 'public_adjutant_activity',
              href: 'https://openagents.com/api/public/adjutant/activity',
              method: 'GET',
              description:
                'Public Autopilot activity projection linked from the homepage.',
            },
          ],
          notes: [
            'This endpoint is public and safe for agents to crawl.',
            'Use the listed hrefs for the live JSON data behind the homepage.',
            'This endpoint is discovery only and grants no write authority.',
          ],
        }),
      )

const handlePublicProductPromisesApi = (request: Request, db: D1Database) =>
  request.method !== 'GET'
    ? Effect.succeed(methodNotAllowed(['GET']))
    : Effect.promise(async () => {
        const document = publicProductPromisesDocument()
        const receipts = await makeD1PromiseTransitionReceiptStore(db)
          .listReceipts(200)
          .catch(() => [])
        const verifiedAt = lastVerifiedAtByPromise(receipts)

        return noStoreJsonResponse({
          ...document,
          promises: document.promises.map(promise => ({
            ...promise,
            lastVerifiedAt: verifiedAt.get(promise.promiseId) ?? null,
          })),
        })
      })

const handleThreadPage = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  threadId: string,
): Promise<Response> => {
  const session = await verifySession(request, env, ctx)

  if (session !== undefined) {
    const accessResult = await threadRouteAccessBundle(
      env,
      session.user.userId,
      threadId,
    )

    if (isRouteAccessError(accessResult)) {
      const response = routeAccessResponse(accessResult, {
        href: '/',
        surface: 'product',
      })

      if (session.tokens !== undefined) {
        appendSessionCookies(response.headers, session.tokens)
      }

      return response
    }
  }

  return handleAppShellPage(request, env, ctx)
}

const isAgentClaimReturnPath = (pathname: string): boolean =>
  /^\/agents\/claims\/[^/]+$/.test(pathname)

const isForumReturnPath = (pathname: string): boolean =>
  pathname === '/forum' ||
  /^\/forum\/(?:f|t)\/[^/]+$/.test(pathname) ||
  /^\/forum\/receipts\/[^/]+$/.test(pathname)

const serializeBrowserReadableCookie = (
  name: string,
  value: string,
  maxAgeSeconds: number,
  path = '/',
): string =>
  [
    `${name}=${encodeURIComponent(value)}`,
    `Max-Age=${Math.max(0, Math.floor(maxAgeSeconds))}`,
    `Path=${path}`,
    'Secure',
    'SameSite=Lax',
  ].join('; ')

const expiredBrowserReadableCookie = (name: string, path = '/'): string =>
  serializeBrowserReadableCookie(name, '', 0, path)

const loginFailedCookie = (): string =>
  serializeBrowserReadableCookie(
    LOGIN_ERROR_COOKIE,
    'github_login_failed',
    LOGIN_ERROR_MAX_AGE_SECONDS,
  )

const cleanLoginReturnPath = (value: string | null): string | undefined => {
  const raw = value?.trim()

  if (
    raw === undefined ||
    raw === '' ||
    !raw.startsWith('/') ||
    raw.startsWith('//') ||
    raw.includes('\n') ||
    raw.includes('\r')
  ) {
    return undefined
  }

  try {
    const url = new URL(raw, 'https://openagents.local')

    if (
      url.origin !== 'https://openagents.local' ||
      url.pathname === '/auth/callback' ||
      url.pathname.startsWith('/login')
    ) {
      return undefined
    }

    if (url.pathname === '/api/team-workspace-invites/accept') {
      const token = url.searchParams.get('token')?.trim()

      return token === undefined || token === ''
        ? undefined
        : `${url.pathname}?token=${encodeURIComponent(token)}`
    }

    if (url.pathname === '/api/pylon/auth/openagents/device/verify') {
      const attempt = url.searchParams.get('attempt')?.trim()
      const code = url.searchParams.get('code')?.trim().toUpperCase()

      return attempt === undefined ||
        code === undefined ||
        !/^pylon_openauth_[A-Za-z0-9_-]+$/.test(attempt) ||
        !/^[A-Z0-9]{4}-[A-Z0-9]{4}$/.test(code)
        ? undefined
        : `${url.pathname}?attempt=${encodeURIComponent(attempt)}&code=${encodeURIComponent(code)}`
    }

    const isAgentClaimReturn = isAgentClaimReturnPath(url.pathname)

    if (
      url.pathname === '/' ||
      url.pathname === '/billing' ||
      url.pathname === '/onboarding' ||
      url.pathname === '/order' ||
      isForumReturnPath(url.pathname) ||
      isAgentClaimReturn ||
      url.pathname.startsWith('/orders/') ||
      url.pathname.startsWith('/share/')
    ) {
      return isAgentClaimReturn ? url.pathname : `${url.pathname}${url.search}`
    }

    return undefined
  } catch {
    return undefined
  }
}

const handleLoginStart = async (
  request: Request,
  env: Env,
  provider: 'github' | 'code',
) => {
  const config = getOpenAgentsWorkerConfig(env)
  const redirectUri = `${getAppOrigin(env)}/auth/callback`
  const { challenge, url } = await createClient({
    clientID: config.openauth.clientId,
    issuer: getIssuerOrigin(env),
  }).authorize(redirectUri, 'code', {
    provider,
  })
  const requestUrl = new URL(request.url)
  const maybeReturnTo = cleanLoginReturnPath(
    requestUrl.searchParams.get('returnTo') ??
      requestUrl.searchParams.get('return_to'),
  )
  const cookies = [
    serializeCookie(
      AUTH_STATE_COOKIE,
      challenge.state,
      AUTH_STATE_MAX_AGE_SECONDS,
      '/auth',
    ),
    serializeCookie(
      LOGIN_ORIGIN_COOKIE,
      requestUrl.origin,
      AUTH_STATE_MAX_AGE_SECONDS,
      '/auth',
    ),
    maybeReturnTo === undefined
      ? expiredCookie(LOGIN_RETURN_TO_COOKIE, '/auth')
      : serializeCookie(
          LOGIN_RETURN_TO_COOKIE,
          maybeReturnTo,
          AUTH_STATE_MAX_AGE_SECONDS,
          '/auth',
        ),
  ]

  return redirectResponse(url, cookies)
}

const handleGitHubStart = (request: Request, env: Env) =>
  handleLoginStart(request, env, 'github')

const handleEmailStart = (request: Request, env: Env) =>
  handleLoginStart(request, env, 'code')

const githubWriteResultRedirect = (env: Env): Response =>
  redirectResponse(githubWriteResultRedirectLocation(getAppOrigin(env)))

const handleGitHubWriteStart = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return redirectResponse(getAppOrigin(env))
  }

  // The GitHub-write connection binds to the signed-in GitHub identity. Email
  // (one-time-code) accounts have none, so there is nothing to connect here.
  if (session.user.githubId === undefined || session.user.login === undefined) {
    return redirectResponse(getAppOrigin(env))
  }

  const attempt = await startGitHubWriteConnectionAttempt(
    makeD1GitHubWriteRepository(openAgentsDatabase(env)),
    {
      expectedGithubId: session.user.githubId,
      expectedGithubLogin: session.user.login,
      redirectAfter: '/',
      scopes: GITHUB_WRITE_REQUIRED_SCOPES,
      userId: session.user.userId,
    },
  )

  return redirectResponse(
    gitHubWriteAuthorizeUrl(env, attempt.state, attempt.scopes),
  )
}

const handleGitHubWriteCallback = async (
  request: Request,
  env: Env,
  attempt: GitHubWriteConnectionAttemptRecord | undefined,
): Promise<Response> => {
  if (attempt === undefined) {
    return githubWriteResultRedirect(env)
  }

  const repository = makeD1GitHubWriteRepository(openAgentsDatabase(env))
  const url = new URL(request.url)
  const now = workerRuntime.now()
  const nowIso = now.toISOString()
  const fail = async (
    status: 'denied' | 'expired' | 'failed',
    reason: string,
  ): Promise<Response> => {
    await repository
      .markAttemptFailed(attempt, status, reason, nowIso)
      .catch(error => {
        logWorkerRouteError('github_write_attempt_mark_failed', error, {
          attemptId: attempt.id,
          errorName: errorName(error),
          status,
        })
      })

    return githubWriteResultRedirect(env)
  }

  if (attempt.status !== 'pending') {
    return githubWriteResultRedirect(env)
  }

  if (Date.parse(attempt.expiresAt) <= now.getTime()) {
    return fail('expired', 'GitHub write connection state expired.')
  }

  const error = url.searchParams.get('error')

  if (error !== null) {
    return fail(error === 'access_denied' ? 'denied' : 'failed', error)
  }

  const code = url.searchParams.get('code')

  if (code === null) {
    return fail('failed', 'GitHub write OAuth code is missing.')
  }

  try {
    const token = await exchangeGitHubOAuthCode(env, code)
    const scopes = parseGitHubScopeHeader(token.scope)
    const githubUser = await fetchGitHubJson(
      GitHubUser,
      'https://api.github.com/user',
      token.access_token,
    ).catch(error => {
      throw new GitHubWriteApiFailure({
        operation: 'fetch_authenticated_user',
        status: 0,
        message: errorMessage(error),
      })
    })
    const githubId = String(githubUser.id)

    try {
      requireGitHubWriteCallbackAccount(attempt, githubId)
      requireGitHubWritePermissions(scopes)
    } catch (error) {
      return fail('failed', gitHubWriteRouteErrorMessage(error))
    }

    const connectionRef = githubWriteConnectionRef(workerRuntime.makeUuid())
    const secretRef = githubWriteSecretRef(connectionRef)

    await storeGitHubWriteAccessToken(env, connectionRef, token.access_token)

    try {
      await recordGitHubWriteConnectionConnected(repository, {
        attempt,
        connectionRef,
        githubId,
        githubLogin: githubUser.login,
        scopes,
        secretRef,
      })
    } catch (error) {
      await env.AUTH_STORAGE.delete(githubWriteSecretKey(connectionRef))
      throw error
    }

    return githubWriteResultRedirect(env)
  } catch (error) {
    logWorkerRouteError('github_write_callback_failed', error, {
      attemptId: attempt.id,
      errorName: errorName(error),
    })

    return fail('failed', 'GitHub write connection failed.')
  }
}

const handleGitHubWriteConnectionsApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const githubWriteConnections = await listGitHubWriteConnectionsForUser(
    makeD1GitHubWriteRepository(openAgentsDatabase(env)),
    session.user.userId,
  )

  return appendRefreshedSessionCookies(
    noStoreJsonResponse({ githubWriteConnections }),
    session,
  )
}

const handleGitHubWriteDisconnectApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  connectionRef: string,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const session = await requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  if (session.user.login === undefined) {
    return noStoreJsonResponse({ error: 'no_github_identity' }, { status: 400 })
  }

  const now = workerRuntime.nowIso()
  const repository = makeD1GitHubWriteRepository(openAgentsDatabase(env))
  const connection = await repository.disconnectConnection({
    connectionRef,
    metadataJson: gitHubWriteConnectionMetadataJson({
      githubLogin: session.user.login,
      scopes: [],
      source: 'browser_disconnect',
      status: 'disconnected',
    }),
    now,
    userId: session.user.userId,
  })

  if (connection === undefined) {
    return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
  }

  await env.AUTH_STORAGE.delete(githubWriteSecretKey(connectionRef))

  return appendRefreshedSessionCookies(
    noStoreJsonResponse({
      connection,
    }),
    session,
  )
}

const runnerResolvedGitHubWriteGrantJson = (
  grant: Awaited<ReturnType<typeof resolveGitHubWriteGrant>>,
  accessToken: string,
) => {
  if (grant === undefined) {
    return undefined
  }

  const expiresAt = Date.parse(grant.expiresAt)

  if (!Number.isFinite(expiresAt)) {
    throw new Error('Resolved GitHub write grant expiry is invalid.')
  }

  return {
    connectionRef: grant.connectionRef,
    credential: {
      accessToken,
      provider: 'github',
      scopes: grant.scopes,
      tokenType: 'oauth',
    },
    expiresAt,
    githubLogin: grant.githubLogin,
    grantRef: grant.grantRef,
    materialization: {
      authRef: grant.secretRef,
      gitCredentialEnv: 'GITHUB_TOKEN',
      provider: 'github',
      remoteUrlMode: 'https_token',
      scrubAfterCloseout: true,
    },
    requestedAction: grant.requestedAction,
    runnerSessionId: grant.runnerSessionId,
    status: 'issued',
  }
}

const redactedGrantErrorMessage = (error: unknown): string =>
  gitHubWriteRouteErrorMessage(error)

const grantResolveErrorStatus = (error: unknown): number => {
  return gitHubWriteRouteErrorStatus(error)
}

const handleGitHubWriteGrantResolveApi = async (
  request: Request,
  env: Env,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const actor = await requireProviderServiceActor(request, env)

  if (actor === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )
  const grantRef =
    optionalString(body.githubWriteGrantRef) ?? optionalString(body.grantRef)

  if (grantRef === undefined) {
    return noStoreJsonResponse(
      { error: 'bad_request', reason: 'githubWriteGrantRef is required' },
      { status: 400 },
    )
  }

  const runnerSessionId =
    optionalString(body.runnerSessionId) ?? optionalString(body.runId)

  try {
    const grant = await resolveGitHubWriteGrant(
      makeD1GitHubWriteRepository(openAgentsDatabase(env)),
      {
        grantRef,
        ...(runnerSessionId === undefined ? {} : { runnerSessionId }),
      },
    )

    if (grant === undefined) {
      return noStoreJsonResponse({ error: 'not_found' }, { status: 404 })
    }

    const accessToken = await env.AUTH_STORAGE.get(
      githubWriteSecretKey(grant.connectionRef),
    )

    if (accessToken === null) {
      return noStoreJsonResponse(
        {
          error: 'github_write_secret_missing',
          message: 'GitHub write credential is not available.',
        },
        { status: 409 },
      )
    }

    return noStoreJsonResponse({
      grant: runnerResolvedGitHubWriteGrantJson(grant, accessToken),
    })
  } catch (error) {
    logWorkerRouteError('github_write_grant_resolve_failed', error, {
      errorName: gitHubWriteRouteErrorName(error),
      grantRef,
      runnerSessionId,
    })

    return noStoreJsonResponse(
      {
        error: 'github_write_grant_resolve_failed',
        message: redactedGrantErrorMessage(error),
      },
      { status: grantResolveErrorStatus(error) },
    )
  }
}

const handleAuthCallback = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  const url = new URL(request.url)
  const error = url.searchParams.get('error')
  const cookies = parseCookies(request)
  const cleanupCookies = [
    expiredCookie(AUTH_STATE_COOKIE, '/auth'),
    expiredCookie(LOGIN_ORIGIN_COOKIE, '/auth'),
    expiredCookie(LOGIN_RETURN_TO_COOKIE, '/auth'),
    expiredBrowserReadableCookie(LOGIN_ERROR_COOKIE),
  ]
  const maybeReturnTo = cleanLoginReturnPath(
    cookies.get(LOGIN_RETURN_TO_COOKIE) ?? null,
  )

  if (error !== null) {
    return redirectResponse(maybeReturnTo ?? '/', [
      ...cleanupCookies,
      loginFailedCookie(),
    ])
  }

  const code = url.searchParams.get('code')
  const state = url.searchParams.get('state')
  const expectedState = cookies.get(AUTH_STATE_COOKIE)

  if (
    code === null ||
    state === null ||
    expectedState === undefined ||
    state !== expectedState
  ) {
    return redirectResponse('/', cleanupCookies)
  }

  const redirectUri = `${getAppOrigin(env)}/auth/callback`
  const exchanged = await observedPromise('Auth.exchangeCode', () =>
    makeAuthClient(env, ctx).exchange(code, redirectUri),
  ).catch(error => {
    logWorkerRouteError('auth_code_exchange_failed', error, {
      errorName: errorName(error),
    })

    return undefined
  })

  if (exchanged === undefined) {
    return redirectResponse('/', cleanupCookies)
  }

  if (exchanged.err !== false) {
    return redirectResponse('/', cleanupCookies)
  }

  const response = redirectResponse(maybeReturnTo ?? '/', cleanupCookies)
  appendSessionCookies(response.headers, exchanged.tokens)

  return response
}

const handleLogout = (request: Request): Response => {
  const requestUrl = new URL(request.url)
  const maybeReturnTo = cleanLoginReturnPath(
    requestUrl.searchParams.get('returnTo') ??
      requestUrl.searchParams.get('return_to'),
  )
  const response = redirectResponse(maybeReturnTo ?? '/')
  appendClearSessionCookies(response.headers, new URL(request.url).hostname)

  return response
}

const handleSessionApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const session = await verifySession(request, env, ctx)

  if (session === undefined) {
    const response = noStoreJsonResponse(
      { authenticated: false },
      { status: 200 },
    )

    const cookies = parseCookies(request)

    if (cookies.has(ACCESS_COOKIE) || cookies.has(REFRESH_COOKIE)) {
      appendClearSessionCookies(response.headers, new URL(request.url).hostname)
    }

    return response
  }

  await upsertUser(openAgentsDatabase(env), session.user)
  const referralResult = await consumePendingReferralForUser(
    openAgentsDatabase(env),
    workerRuntime,
    {
      pendingAttributionId: parseCookies(request).get(PENDING_REFERRAL_COOKIE),
      userId: session.user.userId,
    },
  ).catch(error => {
    logWorkerRouteError('site_referral_session_consumption_failed', error)

    return { _tag: 'none' as const }
  })
  const accountContext = await readAuthenticatedPageContext(env, session)
  scheduleSiteReferralOnboardingEmail(ctx, env, session, referralResult, 'none')

  const response = noStoreJsonResponse({
    authenticated: true,
    bootstrap: {
      session: {
        userId: session.user.userId,
        email: session.user.email,
        name: session.user.name,
        login: session.user.login,
        avatarUrl: session.user.avatarUrl,
        provider: session.user.provider,
        githubId: session.user.githubId,
      },
      teams: accountContext.teams,
      tokenLeaderboards: accountContext.tokenLeaderboards,
      billing: accountContext.billing,
      onboarding: accountContext.onboarding,
      providerAccounts: accountContext.providerAccounts,
      isAdmin: isOpenAgentsAdminEmail(session.user.email),
    },
  })

  if (session.tokens !== undefined) {
    appendSessionCookies(response.headers, session.tokens)
  }

  if (referralResult._tag !== 'none') {
    response.headers.append(
      'set-cookie',
      expiredCookie(PENDING_REFERRAL_COOKIE),
    )
  }

  return response
}

const handleAuthTotalsApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const actor = await authenticateRequestActor(request, env, ctx)

  if (actor === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  if (actor.kind !== 'human' || !isOpenAgentsAdminEmail(actor.user.email)) {
    return forbidden()
  }

  const response = noStoreJsonResponse({
    authenticated: true,
    actor: actorJson(actor),
    totals: await readUserKindTotals(openAgentsDatabase(env)),
  })

  if (actor.kind === 'human' && actor.tokens !== undefined) {
    appendSessionCookies(response.headers, actor.tokens)
  }

  return response
}

const handleAuthTeamsApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  if (request.method !== 'GET') {
    return methodNotAllowed(['GET'])
  }

  const actor = await authenticateRequestActor(request, env, ctx)

  if (actor === undefined) {
    return unauthorized()
  }

  const teams = await readTeamsForUser(
    openAgentsDatabase(env),
    actor.kind === 'human' ? actor.user.userId : actor.agent.user.id,
  )
  const response = jsonResponse({
    authenticated: true,
    actor: actorJson(actor),
    teams,
  })

  if (actor.kind === 'human' && actor.tokens !== undefined) {
    appendSessionCookies(response.headers, actor.tokens)
  }

  return response
}

type PostedTeamChatMessage = Readonly<{
  payload: Record<string, unknown>
  status: number
}>

const adjutantIntentContextFromRequest = (
  input: Record<string, unknown>,
): Partial<Omit<TeamAdjutantIntent, 'prompt' | 'schemaVersion'>> => {
  const softwareOrderId = explicitAdjutantContextString(
    input.softwareOrderId ?? input.software_order_id ?? input.orderId,
    softwareOrderIdPattern,
  )
  const siteId = explicitAdjutantContextString(
    input.siteId ?? input.site_id,
    siteIdPattern,
  )
  const taskSpecPath = boundedTaskSpecPath(
    optionalString(
      input.taskSpecPath ?? input.task_spec_path ?? input.taskPacketPath,
    ),
  )

  return {
    ...(softwareOrderId === undefined ? {} : { softwareOrderId }),
    ...(siteId === undefined ? {} : { siteId }),
    ...(taskSpecPath === undefined ? {} : { taskSpecPath }),
  }
}

const mergeAdjutantIntentContext = (
  bodyIntent: TeamAdjutantIntent,
  requestContext: Partial<Omit<TeamAdjutantIntent, 'prompt' | 'schemaVersion'>>,
  requestedPrompt: string | undefined,
): TeamAdjutantIntent => ({
  schemaVersion: ADJUTANT_INTENT_SCHEMA_VERSION,
  prompt: requestedPrompt ?? bodyIntent.prompt,
  softwareOrderId: requestContext.softwareOrderId ?? bodyIntent.softwareOrderId,
  siteId: requestContext.siteId ?? bodyIntent.siteId,
  taskSpecPath: requestContext.taskSpecPath ?? bodyIntent.taskSpecPath,
})

const adjutantIntentHasContext = (intent: TeamAdjutantIntent): boolean =>
  intent.softwareOrderId !== undefined ||
  intent.siteId !== undefined ||
  intent.taskSpecPath !== undefined

const postTeamChatMessageForUser = async (
  env: Env,
  ctx: ExecutionContext,
  input: Readonly<{
    body: Record<string, unknown>
    project?: UserTeamProject
    roomThreadId: string
    teamId: string
    userId: string
  }>,
): Promise<PostedTeamChatMessage> => {
  const messageBody = optionalString(input.body.body ?? input.body.message)
  const kind = optionalUserWritableTeamChatKind(input.body.kind) ?? 'message'

  if (messageBody === undefined) {
    return {
      payload: { error: 'bad_request', reason: 'body is required' },
      status: 400,
    }
  }

  if (messageBody.length > 4000) {
    return {
      payload: {
        error: 'bad_request',
        reason: 'body must be 4000 characters or fewer',
      },
      status: 400,
    }
  }

  const bodyAdjutantIntent =
    kind === 'adjutant_intent'
      ? teamAdjutantIntentFromBody(messageBody)
      : undefined
  const adjutantIntent =
    bodyAdjutantIntent === undefined
      ? undefined
      : mergeAdjutantIntentContext(
          bodyAdjutantIntent,
          adjutantIntentContextFromRequest(input.body),
          optionalString(input.body.prompt),
        )

  if (
    kind === 'adjutant_intent' &&
    (input.project === undefined || input.project.id !== ADJUTANT_PROJECT_ID)
  ) {
    return {
      payload: {
        error: 'adjutant_project_context_required',
        reason: '@autopilot requires the Autopilot project room',
      },
      status: 400,
    }
  }

  if (kind === 'adjutant_intent' && adjutantIntent === undefined) {
    return {
      payload: {
        error: 'adjutant_command_required',
        reason: 'Autopilot messages must include an exact @autopilot tag',
      },
      status: 400,
    }
  }

  if (
    kind === 'adjutant_intent' &&
    adjutantIntent !== undefined &&
    !adjutantIntentHasContext(adjutantIntent)
  ) {
    return {
      payload: {
        error: 'adjutant_context_required',
        reason:
          '@autopilot requires softwareOrderId, siteId, or taskSpecPath context',
      },
      status: 400,
    }
  }

  const autopilotThreadId =
    kind === 'autopilot_intent'
      ? (optionalUuid(input.body.threadId) ?? makeTeamChatThreadId())
      : undefined
  const executionPrompt =
    kind === 'autopilot_intent'
      ? (optionalString(input.body.prompt) ??
        teamAutopilotPromptFromBody(messageBody))
      : messageBody
  const requestedFileIds = stringArrayFromUnknown(input.body.fileIds)
  const messageId =
    kind === 'autopilot_intent' ? makeTeamChatMessageId() : undefined
  const teamAutopilotContext =
    kind === 'autopilot_intent' && messageId !== undefined
      ? await hydrateTeamAutopilotContextFileExcerpts(
          env,
          teamAutopilotContextBundle({
            files: await listTeamThreadFiles(openAgentsDatabase(env), {
              teamId: input.teamId,
              threadId: input.roomThreadId,
            }),
            messages: await listTeamChatMessages(
              openAgentsDatabase(env),
              input.teamId,
              12,
              undefined,
              undefined,
              input.project?.id ?? null,
            ),
            ...(input.project === undefined ? {} : { project: input.project }),
            parentTeamChatMessageId: messageId,
            prompt: executionPrompt,
            requestedFileIds,
            teamId: input.teamId,
          }),
        )
      : undefined
  const missionLaunch =
    kind === 'autopilot_intent' && teamAutopilotContext !== undefined
      ? await launchUserAutopilotMission(env, ctx, {
          selector: {
            ...input.body,
            autopilotThreadId,
            dispatchGoal: teamAutopilotChildRunGoal(teamAutopilotContext),
            goal: teamAutopilotContext.normalizedPrompt,
            parentTeamChatMessageId:
              teamAutopilotContext.parentTeamChatMessageId,
            prompt: teamAutopilotContext.normalizedPrompt,
            ...(input.project === undefined
              ? {}
              : { projectId: input.project.id }),
            selectedTeamFileIds: teamAutopilotContext.selectedTeamFileIds,
            teamId: input.teamId,
          },
          userId: input.userId,
        })
      : undefined
  const launchError =
    missionLaunch?.ok === false
      ? await teamChatLaunchErrorFromResponse(missionLaunch.response)
      : undefined

  const message = await insertTeamChatMessage(openAgentsDatabase(env), {
    ...(missionLaunch === undefined || missionLaunch.ok === false
      ? {}
      : {
          agentRunId: missionLaunch.launch.runId,
          ...(autopilotThreadId === undefined ? {} : { autopilotThreadId }),
          metadataJson: JSON.stringify({
            mission: missionLaunch.launch.payload.mission,
            runSummary: missionLaunch.launch.payload.runSummary,
            statusUrl: missionLaunch.launch.payload.statusUrl,
            streamUrl: missionLaunch.launch.payload.streamUrl,
            context: teamAutopilotContext,
            selectedTeamFileIds:
              teamAutopilotContext?.selectedTeamFileIds ?? [],
          }),
        }),
    ...(launchError === undefined
      ? {}
      : {
          ...(autopilotThreadId === undefined ? {} : { autopilotThreadId }),
          metadataJson: JSON.stringify({
            context: teamAutopilotContext,
            launchError,
            launchStatus:
              missionLaunch?.ok === false
                ? missionLaunch.response.status
                : undefined,
            selectedTeamFileIds:
              teamAutopilotContext?.selectedTeamFileIds ?? [],
          }),
        }),
    ...(adjutantIntent === undefined
      ? {}
      : {
          metadataJson: JSON.stringify({
            adjutantIntent: {
              schemaVersion: adjutantIntent.schemaVersion,
              prompt: adjutantIntent.prompt,
              softwareOrderId: adjutantIntent.softwareOrderId ?? null,
              siteId: adjutantIntent.siteId ?? null,
              taskSpecPath: adjutantIntent.taskSpecPath ?? null,
            },
          }),
        }),
    authorUserId: input.userId,
    body: messageBody,
    ...(messageId === undefined ? {} : { id: messageId }),
    kind,
    ...(input.project === undefined ? {} : { projectId: input.project.id }),
    teamId: input.teamId,
  })
  const selectedTeamFileIds =
    kind === 'autopilot_intent'
      ? (teamAutopilotContext?.selectedTeamFileIds ?? [])
      : requestedFileIds

  await insertThreadFileMessageReferences(openAgentsDatabase(env), {
    fileIds: selectedTeamFileIds,
    messageId: message.id,
    referenceKind:
      kind === 'autopilot_intent' ? 'autopilot_input' : 'message_attachment',
    teamId: input.teamId,
    threadId: message.autopilotThreadId ?? input.roomThreadId,
  })

  await publishTeamChatMessageSync(env, ctx, message, input.userId)

  return {
    payload: {
      ...(missionLaunch === undefined || missionLaunch.ok === false
        ? {}
        : missionLaunch.launch.payload),
      ...(launchError === undefined ? {} : { launchError }),
      ...(autopilotThreadId === undefined ||
      missionLaunch === undefined ||
      missionLaunch.ok === false
        ? {}
        : {
            threadId: autopilotThreadId,
            threadUrl: `/t/${missionLaunch.launch.runId}`,
          }),
      message,
      projectId: input.project?.id ?? null,
      teamId: input.teamId,
    },
    status:
      kind === 'autopilot_intent' && launchError === undefined ? 202 : 201,
  }
}

const handleTeamChatMessagesApi = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  teamId: string,
  projectId?: string,
): Promise<Response> => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }

  const session = await requireBrowserSession(request, env, ctx)

  if (session === undefined) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const role = await readActiveTeamMembershipRole(
    openAgentsDatabase(env),
    teamId,
    session.user.userId,
  )

  if (role === undefined) {
    return forbidden()
  }

  const project =
    projectId === undefined
      ? undefined
      : await readActiveTeamProject(openAgentsDatabase(env), teamId, projectId)

  if (projectId !== undefined && project === undefined) {
    return notFound()
  }

  const roomThreadId =
    project === undefined
      ? teamChatThreadId(teamId)
      : teamProjectChatThreadId(teamId, project.id)

  if (request.method === 'GET') {
    const url = new URL(request.url)
    const requestedLimit = optionalInteger(url.searchParams.get('limit')) ?? 50
    const limit = Math.min(Math.max(requestedLimit, 1), 100)
    const kind = optionalUserWritableTeamChatKind(url.searchParams.get('kind'))
    const autopilotThreadId = optionalUuid(url.searchParams.get('threadId'))
    const response = noStoreJsonResponse({
      messages: await listTeamChatMessages(
        openAgentsDatabase(env),
        teamId,
        limit,
        kind,
        autopilotThreadId,
        project?.id ?? null,
      ),
      projectId: project?.id ?? null,
      teamId,
    })

    return appendRefreshedSessionCookies(response, session)
  }

  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )
  const posted = await postTeamChatMessageForUser(env, ctx, {
    body,
    ...(project === undefined ? {} : { project }),
    roomThreadId,
    teamId,
    userId: session.user.userId,
  })

  const response = noStoreJsonResponse(posted.payload, {
    status: posted.status,
  })

  return appendRefreshedSessionCookies(response, session)
}

const optionalUserWritableTeamChatKind = (
  value: unknown,
): 'message' | 'autopilot_intent' | 'adjutant_intent' | undefined =>
  value === 'message' ||
  value === 'autopilot_intent' ||
  value === 'adjutant_intent'
    ? value
    : undefined

const requireProviderServiceActor = async (
  request: Request,
  env: Env,
): Promise<ProgrammaticAgentSession | undefined> => {
  const bearerToken = readBearerToken(request)

  if (bearerToken === undefined) {
    return undefined
  }

  return authenticateProgrammaticAgent(
    makeD1AgentRegistrationStore(openAgentsDatabase(env)),
    bearerToken,
  )
}

const requireRunnerCallbackAuth = async (
  request: Request,
  env: Env,
): Promise<boolean> => {
  const expected = redactedValue(
    getOpenAgentsWorkerConfig(env).shc.runnerCallbackToken,
  )
  const actual = readBearerToken(request)

  if (
    expected === undefined ||
    expected.trim() === '' ||
    actual === undefined
  ) {
    return false
  }

  return timingSafeEqual(actual, expected)
}

const requireAdminApiToken = async (
  request: Request,
  env: Env,
): Promise<boolean> => {
  const expected = getAdminApiToken(env)
  const actual = readBearerToken(request)

  if (expected === undefined || actual === undefined) {
    return false
  }

  return timingSafeEqual(actual, expected)
}

const shcDispatchConfig = (env: Env) => {
  const config = getOpenAgentsWorkerConfig(env)

  return {
    controlApiBearerToken: redactedValue(config.shc.controlApiBearerToken),
    controlApiUrl: config.shc.controlApiUrl,
    dispatchMode: config.shc.dispatchMode,
  }
}

const cleanupCanceledAgentRunOnShc = async (
  env: Env,
  run: AgentRunRecord,
): Promise<void> => {
  try {
    const result = await cancelAgentRunOnShc(run, {
      ...shcDispatchConfig(env),
      reason: 'Credits exhausted; OpenAgents stopped the run.',
    })

    if (!result.ok) {
      logWorkerRouteWarning('shc_billing_cleanup_not_acknowledged', {
        error: result.error,
        runId: run.id,
        status: result.status,
        targetPath: result.targetPath,
      })
    }
  } catch (error) {
    logWorkerRouteWarning('shc_billing_cleanup_request_failed', {
      error: errorMessage(error),
      runId: run.id,
    })
  }
}

const sendAutopilotDecisionRequiredEmailOnce = async (
  env: Env,
  record: AutopilotWorkOrderRecord,
): Promise<void> => {
  const resend = getOpenAgentsWorkerConfig(env).email.resend

  if (resend === undefined) {
    logWorkerRouteWarning('autopilot_decision_email_config_missing', {
      workOrderRef: record.workOrderRef,
    })

    return
  }

  const contact = await openAgentsDatabase(env)
    .prepare(
      `SELECT display_name, primary_email
       FROM users
       WHERE id = ?`,
    )
    .bind(record.ownerUserId)
    .first<Readonly<{ display_name: string; primary_email: string | null }>>()
  const email = contact?.primary_email?.trim()

  if (email === undefined || email === '') {
    logWorkerRouteWarning('autopilot_decision_email_missing_recipient', {
      workOrderRef: record.workOrderRef,
    })

    return
  }

  const delivery = await observedEffect(
    'Email.sendAutopilotDecisionEmailWithLedger',
    sendAutopilotDecisionEmailWithLedger(
      openAgentsDatabase(env),
      resend,
      new AutopilotDecisionEmailInput({
        appOrigin: getAppOrigin(env),
        displayName: contact?.display_name.trim() || 'there',
        idempotencyKey: `autopilot:decision_required:${record.workOrderRef}`,
        kind: 'decision_required',
        to: email,
        workOrderRef: record.workOrderRef,
      }),
      {
        sourceAuthorityRef: 'system.autopilot_decision_notification.v1',
        targetUserId: record.ownerUserId,
      },
    ),
  )

  if (!delivery.ok) {
    logWorkerRouteWarning('autopilot_decision_email_failed', {
      error: delivery.errorMessage,
      workOrderRef: record.workOrderRef,
    })
  }
}

const sendOutOfCreditsNotificationOnce = async (
  env: Env,
  input: Readonly<{
    balanceCents: number
    balanceFormatted: string
    userId: string
  }>,
): Promise<void> => {
  const reservation = await reserveOutOfCreditsNotification(
    openAgentsDatabase(env),
    input,
  )

  if (!reservation.ok) {
    return
  }

  const resend = getOpenAgentsWorkerConfig(env).email.resend
  const delivery =
    resend === undefined
      ? {
          errorMessage: 'Resend email configuration is not set.',
          errorName: 'resend_config_missing',
          ok: false as const,
        }
      : await observedEffect(
          'Email.sendOutOfCreditsEmailWithLedger',
          sendOutOfCreditsEmailWithLedger(
            openAgentsDatabase(env),
            resend,
            {
              appOrigin: getAppOrigin(env),
              balanceFormatted: input.balanceFormatted,
              displayName: reservation.displayName,
              idempotencyKey: reservation.idempotencyKey,
              to: reservation.email,
            },
            {
              metadata: {
                balanceCents: input.balanceCents,
                existingReservationTable: 'billing_credit_notifications',
              },
              sourceAuthorityRef: 'system.billing_out_of_credits.v1',
              targetUserId: input.userId,
            },
          ),
        )

  if (delivery.ok) {
    await markOutOfCreditsNotificationSent(openAgentsDatabase(env), {
      resendEmailId: delivery.id,
      userId: input.userId,
    })

    return
  }

  await markOutOfCreditsNotificationFailed(openAgentsDatabase(env), {
    errorMessage: delivery.errorMessage,
    userId: input.userId,
  })
  logWorkerRouteWarning('out_of_credits_email_failed', {
    error: delivery.errorMessage,
    userId: input.userId,
  })
}

const handleEmailResendWebhookApi = async (
  request: Request,
  env: Parameters<typeof getResendEmailConfig>[0],
) => {
  try {
    if (request.method !== 'POST') {
      return methodNotAllowed(['POST'])
    }

    const config = getOpenAgentsWorkerConfig(env)
    const result = await handleResendWebhook(openAgentsDatabase(env), {
      body: await request.text(),
      headers: request.headers,
      secret: config.email.resendWebhookSecret,
    })

    return noStoreJsonResponse(
      {
        duplicate: result.duplicate,
        eventType: result.eventType,
        ok: result.status === 'accepted',
        providerEventId: result.providerEventId,
        status: result.status,
      },
      { status: result.status === 'unauthorized' ? 401 : 200 },
    )
  } catch (error) {
    return noStoreJsonResponse(
      {
        error: 'resend_webhook_error',
        message:
          error instanceof Error
            ? error.message
            : 'Resend webhook processing failed.',
      },
      { status: 400 },
    )
  }
}

type SiteCustomerNotificationRow = Readonly<{
  display_name: string | null
  order_id: string | null
  primary_email: string | null
  site_title: string | null
  target_user_id: string | null
}>

type SiteCustomerNotificationOutcome = Readonly<{
  emailMessageId: string | null
  emailStatus: 'accepted' | 'failed' | 'skipped'
  providerMessageId?: string | null | undefined
  skipReason?: string | undefined
}>

type ReviewReadyNotificationRow = Readonly<{
  assignment_current_run_id: string | null
  assignment_goal_id: string | null
  assignment_id: string | null
  assignment_visibility: 'private' | 'team' | 'public' | null
  deployment_id: string | null
  display_name: string | null
  order_id: string | null
  primary_email: string | null
  site_id: string
  site_title: string | null
  site_url: string | null
  target_user_id: string | null
  version_id: string
}>

type ReviewReadyArtifactNotificationRow = Readonly<{
  artifact_id: string
  artifact_title: string
  artifact_url: string | null
  assignment_current_run_id: string | null
  assignment_goal_id: string | null
  assignment_id: string | null
  assignment_visibility: 'private' | 'team' | 'public' | null
  display_name: string | null
  kind: string
  order_id: string
  primary_email: string | null
  repository_full_name: string | null
  target_user_id: string | null
}>

type AdjutantNotificationAssignmentRow = Readonly<{
  current_run_id: string | null
  goal_id: string | null
  id: string
  software_order_id: string | null
  visibility: 'private' | 'team' | 'public'
}>

const notificationPayloadJson = (
  input: Readonly<{
    deploymentId: string
    emailMessageId: string | null
    emailStatus: string
    providerMessageId?: string | null | undefined
    siteId: string
    siteUrl: string
    skipReason?: string | undefined
    softwareOrderId: string | null
    stage: 'deployed'
  }>,
): string =>
  JSON.stringify({
    deploymentId: input.deploymentId,
    emailMessageId: input.emailMessageId,
    emailStatus: input.emailStatus,
    providerMessageId: input.providerMessageId ?? null,
    siteId: input.siteId,
    siteUrl: input.siteUrl,
    skipReason: input.skipReason ?? null,
    softwareOrderId: input.softwareOrderId,
    stage: input.stage,
  })

const reviewReadyNotificationPayloadJson = (
  input: Readonly<{
    deploymentId: string | null
    emailMessageId: string | null
    emailStatus: string
    providerMessageId?: string | null | undefined
    siteId: string
    siteUrl: string | null
    skipReason?: string | undefined
    softwareOrderId: string | null
    stage: 'review_ready'
    versionId: string
  }>,
): string =>
  JSON.stringify({
    deploymentId: input.deploymentId,
    emailMessageId: input.emailMessageId,
    emailStatus: input.emailStatus,
    providerMessageId: input.providerMessageId ?? null,
    siteId: input.siteId,
    siteUrl: input.siteUrl,
    skipReason: input.skipReason ?? null,
    softwareOrderId: input.softwareOrderId,
    stage: input.stage,
    versionId: input.versionId,
  })

const reviewReadyArtifactNotificationPayloadJson = (
  input: Readonly<{
    artifactId: string
    artifactUrl: string | null
    emailMessageId: string | null
    emailStatus: string
    providerMessageId?: string | null | undefined
    skipReason?: string | undefined
    softwareOrderId: string
    stage: 'review_ready'
  }>,
): string =>
  JSON.stringify({
    artifactId: input.artifactId,
    artifactUrl: input.artifactUrl,
    emailMessageId: input.emailMessageId,
    emailStatus: input.emailStatus,
    providerMessageId: input.providerMessageId ?? null,
    skipReason: input.skipReason ?? null,
    softwareOrderId: input.softwareOrderId,
    stage: input.stage,
  })

const siteRevisionUrl = (
  siteUrl: string | null,
  versionId: string | null,
): string | null =>
  siteUrl === null || versionId === null
    ? null
    : `${siteUrl.replace(/\/+$/, '')}/versions/${encodeURIComponent(versionId)}`

const notifyCustomerSiteDeployed = async (
  env: Env,
  input: Readonly<{
    actorUserId: string
    deploymentId: string
    siteId: string
    siteUrl: string
  }>,
): Promise<SiteCustomerNotificationOutcome> => {
  const db = openAgentsDatabase(env)
  const existingSiteEvent = await db
    .prepare(
      `SELECT id
         FROM site_events
        WHERE site_id = ?
          AND deployment_id = ?
          AND type = 'adjutant.notification.deployed'
        LIMIT 1`,
    )
    .bind(input.siteId, input.deploymentId)
    .first<Readonly<{ id: string }>>()

  if (existingSiteEvent !== null) {
    return {
      emailMessageId: null,
      emailStatus: 'skipped',
      skipReason: 'notification_event_already_recorded',
    }
  }

  const target = await db
    .prepare(
      `SELECT software_orders.id AS order_id,
              users.id AS target_user_id,
              users.display_name,
              users.primary_email,
              site_projects.title AS site_title
         FROM site_projects
         LEFT JOIN software_orders
           ON software_orders.id = site_projects.software_order_id
          AND software_orders.archived_at IS NULL
         LEFT JOIN users
           ON users.id = software_orders.user_id
          AND users.deleted_at IS NULL
        WHERE site_projects.id = ?
          AND site_projects.archived_at IS NULL
        LIMIT 1`,
    )
    .bind(input.siteId)
    .first<SiteCustomerNotificationRow>()
  const assignment = await db
    .prepare(
      `SELECT id,
              software_order_id,
              goal_id,
              current_run_id,
              visibility
         FROM adjutant_assignments
        WHERE site_id = ?
          AND archived_at IS NULL
        ORDER BY updated_at DESC
        LIMIT 1`,
    )
    .bind(input.siteId)
    .first<AdjutantNotificationAssignmentRow>()
  const email = target?.primary_email?.trim()
  const resend = getResendEmailConfig(env)
  const notification =
    await (async (): Promise<SiteCustomerNotificationOutcome> => {
      if (
        target === null ||
        target.order_id === null ||
        target.target_user_id === null
      ) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_order_notification_target',
        }
      }

      if (email === undefined || email === '') {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_customer_email',
        }
      }

      if (resend === undefined) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'email_config_missing',
        }
      }

      const notificationInput = new OrderSitesTransactionalEmailInput({
        appOrigin: getAppOrigin(env),
        ...(assignment === null ? {} : { assignmentId: assignment.id }),
        artifactLabel: null,
        artifactUrl: null,
        customerSafeStatus: 'deployed',
        displayName: target.display_name ?? email,
        eventRef: input.deploymentId,
        lifecycleKind: 'site_deployed',
        nextAction:
          'Review the deployed Site and reply with any requested adjustment.',
        orderId: target.order_id,
        revisionUrl: null,
        safeReason: null,
        siteId: input.siteId,
        siteTitle: target.site_title,
        siteUrl: input.siteUrl,
        sourceAuthorityRefs: [
          'docs/2026-06-05-adjutant-sites-supervisor-audit.md#16',
        ],
        targetRefs: [
          target.order_id,
          input.siteId,
          input.deploymentId,
          ...(assignment === null ? [] : [assignment.id]),
        ],
        to: email,
      })

      const result = await observedEffect(
        'Email.sendOrderSitesTransactionalEmailWithLedger',
        sendOrderSitesTransactionalEmailWithLedger(
          db,
          resend,
          new OrderSitesTransactionalEmailInput({
            ...notificationInput,
            idempotencyKey:
              buildOrderSitesTransactionalEmailIdempotencyKey(
                notificationInput,
              ),
          }),
          {
            actorUserId: input.actorUserId,
            metadata: {
              assignmentId: assignment?.id ?? null,
              deploymentId: input.deploymentId,
              eventSource: 'operator_sites_deploy',
              lifecycleKind: 'site_deployed',
              siteId: input.siteId,
              softwareOrderId: target.order_id,
              stage: 'deployed',
            },
            sourceAuthorityRef: 'system.order_sites_lifecycle_email.v1',
            targetUserId: target.target_user_id,
          },
        ),
      )

      return result.ok
        ? {
            emailMessageId: result.emailMessageId,
            emailStatus: 'accepted',
            providerMessageId: result.providerMessageId,
          }
        : {
            emailMessageId: result.emailMessageId,
            emailStatus: 'failed',
            skipReason: result.errorMessage,
          }
    })()
  const now = currentIsoTimestamp()
  const payload = notificationPayloadJson({
    deploymentId: input.deploymentId,
    emailMessageId: notification.emailMessageId,
    emailStatus: notification.emailStatus,
    providerMessageId: notification.providerMessageId,
    siteId: input.siteId,
    siteUrl: input.siteUrl,
    skipReason: notification.skipReason,
    softwareOrderId: assignment?.software_order_id ?? target?.order_id ?? null,
    stage: 'deployed',
  })
  const summary =
    notification.emailStatus === 'accepted'
      ? 'Autopilot customer deployed email notification was accepted.'
      : notification.emailStatus === 'failed'
        ? 'Autopilot customer deployed email notification failed.'
        : 'Autopilot customer deployed email notification is needed.'

  if (assignment !== null) {
    await observedEffect(
      'AdjutantUsageReceipts.recordHosting',
      recordAdjutantUsageReceipt(db, {
        assignmentId: assignment.id,
        billingMode: 'public_beta_free',
        category: 'hosting',
        idempotencyKey: [
          'adjutant_usage',
          assignment.id,
          input.deploymentId,
          'hosting',
        ].join(':'),
        publicDetails: {
          billingNote: 'Public beta Site hosting is free.',
          siteUrl: input.siteUrl,
        },
        quantity: 1,
        runId: assignment.current_run_id,
        siteId: input.siteId,
        softwareOrderId: assignment.software_order_id,
        summary: 'Autopilot activated public Site hosting.',
        teamDetails: {
          billingPolicy: 'public_beta_free',
          deploymentId: input.deploymentId,
          siteId: input.siteId,
          siteUrl: input.siteUrl,
        },
        unit: 'deployment',
        visibility: assignment.visibility,
      }),
    )

    await db
      .prepare(
        `INSERT INTO adjutant_assignment_events
           (id,
            assignment_id,
            software_order_id,
            site_id,
            goal_id,
            run_id,
            event_type,
            visibility,
            summary,
            actor_user_id,
            payload_json,
            email_message_id,
            created_at)
         VALUES (?, ?, ?, ?, ?, ?, 'adjutant.notification.deployed', ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        compactRandomId('adjutant_assignment_event'),
        assignment.id,
        assignment.software_order_id,
        input.siteId,
        assignment.goal_id,
        assignment.current_run_id,
        assignment.visibility,
        summary,
        input.actorUserId,
        payload,
        notification.emailMessageId,
        now,
      )
      .run()
  }

  await db
    .prepare(
      `INSERT INTO site_events
         (id,
          site_id,
          version_id,
          deployment_id,
          type,
          summary,
          actor_user_id,
          actor_run_id,
          payload_json,
          email_message_id,
          created_at)
       VALUES (?, ?, NULL, ?, 'adjutant.notification.deployed', ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      compactRandomId('site_event'),
      input.siteId,
      input.deploymentId,
      summary,
      input.actorUserId,
      assignment?.current_run_id ?? null,
      payload,
      notification.emailMessageId,
      now,
    )
    .run()

  return notification
}

const readPendingReviewReadyNotifications = async (
  db: D1Database,
): Promise<ReadonlyArray<ReviewReadyNotificationRow>> =>
  db
    .prepare(
      `SELECT site_projects.id AS site_id,
              site_projects.title AS site_title,
              site_projects.active_version_id AS version_id,
              site_projects.active_deployment_id AS deployment_id,
              site_deployments.url AS site_url,
              software_orders.id AS order_id,
              users.id AS target_user_id,
              users.display_name,
              users.primary_email,
              adjutant_assignments.id AS assignment_id,
              adjutant_assignments.goal_id AS assignment_goal_id,
              adjutant_assignments.current_run_id AS assignment_current_run_id,
              adjutant_assignments.visibility AS assignment_visibility
         FROM site_projects
         JOIN site_versions
           ON site_versions.id = site_projects.active_version_id
          AND site_versions.site_id = site_projects.id
         LEFT JOIN site_deployments
           ON site_deployments.id = site_projects.active_deployment_id
          AND site_deployments.site_id = site_projects.id
         LEFT JOIN software_orders
           ON software_orders.id = site_projects.software_order_id
          AND software_orders.archived_at IS NULL
         LEFT JOIN users
           ON users.id = software_orders.user_id
          AND users.deleted_at IS NULL
         LEFT JOIN adjutant_assignments
           ON adjutant_assignments.id = (
                SELECT id
                  FROM adjutant_assignments AS assignment
                 WHERE assignment.site_id = site_projects.id
                   AND assignment.archived_at IS NULL
                 ORDER BY assignment.updated_at DESC
                 LIMIT 1
              )
        WHERE site_projects.archived_at IS NULL
          AND site_projects.active_version_id IS NOT NULL
          AND json_extract(site_versions.metadata_json, '$.customerReviewState') = 'customer_review_ready'
          AND NOT EXISTS (
                SELECT 1
                  FROM site_events
                  JOIN email_messages
                    ON email_messages.id = site_events.email_message_id
                 WHERE site_events.site_id = site_projects.id
                   AND site_events.version_id = site_projects.active_version_id
                   AND site_events.type = 'adjutant.notification.review_ready'
                   AND email_messages.status = 'accepted'
              )
        ORDER BY site_versions.created_at ASC
        LIMIT 10`,
    )
    .all<ReviewReadyNotificationRow>()
    .then(result => result.results)

const sendReviewReadySiteNotification = async (
  env: Parameters<typeof getResendEmailConfig>[0],
  row: ReviewReadyNotificationRow,
): Promise<SiteCustomerNotificationOutcome> => {
  const db = openAgentsDatabase(env)
  const email = row.primary_email?.trim()
  const resend = getResendEmailConfig(env)
  const notification =
    await (async (): Promise<SiteCustomerNotificationOutcome> => {
      if (row.order_id === null || row.target_user_id === null) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_order_notification_target',
        }
      }

      if (email === undefined || email === '') {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_customer_email',
        }
      }

      if (resend === undefined) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'email_config_missing',
        }
      }

      const notificationInput = new OrderSitesTransactionalEmailInput({
        appOrigin: getAppOrigin(env),
        ...(row.assignment_id === null
          ? {}
          : { assignmentId: row.assignment_id }),
        artifactLabel: null,
        artifactUrl: null,
        customerSafeStatus: 'Ready for review',
        displayName: row.display_name ?? email,
        eventRef: row.version_id,
        lifecycleKind: 'review_ready',
        nextAction:
          'Open your order status page, review the latest Site revision, and send any follow-up comment.',
        orderId: row.order_id,
        revisionUrl: siteRevisionUrl(row.site_url, row.version_id),
        safeReason: null,
        siteId: row.site_id,
        siteTitle: row.site_title,
        siteUrl: row.site_url,
        sourceAuthorityRefs: [
          'docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md#email-and-drip-campaign-plan',
          'system.review_ready_site_notification_reconciler.v1',
        ],
        targetRefs: [
          row.order_id,
          row.site_id,
          row.version_id,
          ...(row.deployment_id === null ? [] : [row.deployment_id]),
          ...(row.assignment_id === null ? [] : [row.assignment_id]),
        ],
        to: email,
      })

      const result = await observedEffect(
        'Email.sendReviewReadySiteNotification',
        sendOrderSitesTransactionalEmailWithLedger(
          db,
          resend,
          new OrderSitesTransactionalEmailInput({
            ...notificationInput,
            idempotencyKey:
              buildOrderSitesTransactionalEmailIdempotencyKey(
                notificationInput,
              ),
          }),
          {
            actorUserId: 'system:review-ready-reconciler',
            metadata: {
              assignmentId: row.assignment_id,
              deploymentId: row.deployment_id,
              eventSource: 'review_ready_site_notification_reconciler',
              lifecycleKind: 'review_ready',
              siteId: row.site_id,
              softwareOrderId: row.order_id,
              stage: 'review_ready',
              versionId: row.version_id,
            },
            sourceAuthorityRef:
              'system.review_ready_site_notification_reconciler.v1',
            targetUserId: row.target_user_id,
          },
        ),
      )

      return result.ok
        ? {
            emailMessageId: result.emailMessageId,
            emailStatus: 'accepted',
            providerMessageId: result.providerMessageId,
          }
        : {
            emailMessageId: result.emailMessageId,
            emailStatus: 'failed',
            skipReason: result.errorMessage,
          }
    })()
  const now = currentIsoTimestamp()
  const payload = reviewReadyNotificationPayloadJson({
    deploymentId: row.deployment_id,
    emailMessageId: notification.emailMessageId,
    emailStatus: notification.emailStatus,
    providerMessageId: notification.providerMessageId,
    siteId: row.site_id,
    siteUrl: row.site_url,
    skipReason: notification.skipReason,
    softwareOrderId: row.order_id,
    stage: 'review_ready',
    versionId: row.version_id,
  })
  const summary =
    notification.emailStatus === 'accepted'
      ? 'Autopilot customer review-ready email notification was accepted.'
      : notification.emailStatus === 'failed'
        ? 'Autopilot customer review-ready email notification failed.'
        : 'Autopilot customer review-ready email notification is needed.'

  if (row.assignment_id !== null) {
    await db
      .prepare(
        `INSERT INTO adjutant_assignment_events
           (id,
            assignment_id,
            software_order_id,
            site_id,
            goal_id,
            run_id,
            event_type,
            visibility,
            summary,
            actor_user_id,
            payload_json,
            email_message_id,
            created_at)
         VALUES (?, ?, ?, ?, ?, ?, 'adjutant.notification.review_ready', ?, ?, ?, ?, ?, ?)`,
      )
      .bind(
        compactRandomId('adjutant_assignment_event'),
        row.assignment_id,
        row.order_id,
        row.site_id,
        row.assignment_goal_id,
        row.assignment_current_run_id,
        row.assignment_visibility ?? 'public',
        summary,
        'system:review-ready-reconciler',
        payload,
        notification.emailMessageId,
        now,
      )
      .run()
  }

  await db
    .prepare(
      `INSERT INTO site_events
         (id,
          site_id,
          version_id,
          deployment_id,
          type,
          summary,
          actor_user_id,
          actor_run_id,
          payload_json,
          email_message_id,
          created_at)
       VALUES (?, ?, ?, ?, 'adjutant.notification.review_ready', ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      compactRandomId('site_event'),
      row.site_id,
      row.version_id,
      row.deployment_id,
      summary,
      'system:review-ready-reconciler',
      row.assignment_current_run_id,
      payload,
      notification.emailMessageId,
      now,
    )
    .run()

  return notification
}

const sendPendingReviewReadySiteNotifications = async (
  env: Parameters<typeof getResendEmailConfig>[0],
): Promise<void> => {
  if (getResendEmailConfig(env) === undefined) {
    return
  }

  const pending = await readPendingReviewReadyNotifications(
    openAgentsDatabase(env),
  )

  await Promise.all(
    pending.map(row =>
      sendReviewReadySiteNotification(env, row).catch(error => {
        logWorkerRouteWarning('review_ready_site_notification_failed', {
          error: errorMessage(error),
          siteId: row.site_id,
          versionId: row.version_id,
        })
      }),
    ),
  )
}

const readPendingReviewReadyArtifactNotifications = async (
  db: D1Database,
): Promise<ReadonlyArray<ReviewReadyArtifactNotificationRow>> =>
  db
    .prepare(
      `SELECT order_fulfillment_artifacts.id AS artifact_id,
              order_fulfillment_artifacts.title AS artifact_title,
              order_fulfillment_artifacts.url AS artifact_url,
              order_fulfillment_artifacts.kind AS kind,
              order_fulfillment_artifacts.repository_full_name AS repository_full_name,
              software_orders.id AS order_id,
              users.id AS target_user_id,
              users.display_name,
              users.primary_email,
              adjutant_assignments.id AS assignment_id,
              adjutant_assignments.goal_id AS assignment_goal_id,
              adjutant_assignments.current_run_id AS assignment_current_run_id,
              adjutant_assignments.visibility AS assignment_visibility
         FROM order_fulfillment_artifacts
         JOIN software_orders
           ON software_orders.id = order_fulfillment_artifacts.software_order_id
          AND software_orders.archived_at IS NULL
         LEFT JOIN users
           ON users.id = software_orders.user_id
          AND users.deleted_at IS NULL
         LEFT JOIN adjutant_assignments
           ON adjutant_assignments.id = (
                SELECT id
                  FROM adjutant_assignments AS assignment
                 WHERE assignment.software_order_id = software_orders.id
                   AND assignment.archived_at IS NULL
                 ORDER BY assignment.updated_at DESC
                 LIMIT 1
              )
        WHERE order_fulfillment_artifacts.archived_at IS NULL
          AND order_fulfillment_artifacts.visibility = 'public'
          AND order_fulfillment_artifacts.status = 'customer_review_ready'
          AND adjutant_assignments.id IS NOT NULL
          AND NOT EXISTS (
                SELECT 1
                  FROM adjutant_assignment_events
                  JOIN email_messages
                    ON email_messages.id = adjutant_assignment_events.email_message_id
                 WHERE adjutant_assignment_events.software_order_id = software_orders.id
                   AND adjutant_assignment_events.event_type = 'adjutant.notification.review_ready_artifact'
                   AND json_extract(adjutant_assignment_events.payload_json, '$.artifactId') = order_fulfillment_artifacts.id
                   AND email_messages.status = 'accepted'
              )
        ORDER BY order_fulfillment_artifacts.created_at ASC
        LIMIT 10`,
    )
    .all<ReviewReadyArtifactNotificationRow>()
    .then(result => result.results)

const sendReviewReadyArtifactNotification = async (
  env: Parameters<typeof getResendEmailConfig>[0],
  row: ReviewReadyArtifactNotificationRow,
): Promise<SiteCustomerNotificationOutcome> => {
  const db = openAgentsDatabase(env)
  const email = row.primary_email?.trim()
  const resend = getResendEmailConfig(env)
  const notification =
    await (async (): Promise<SiteCustomerNotificationOutcome> => {
      if (row.target_user_id === null) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_order_notification_target',
        }
      }

      if (row.assignment_id === null) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_assignment',
        }
      }

      if (email === undefined || email === '') {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'missing_customer_email',
        }
      }

      if (resend === undefined) {
        return {
          emailMessageId: null,
          emailStatus: 'skipped',
          skipReason: 'email_config_missing',
        }
      }

      const artifactLabel =
        row.kind === 'pull_request'
          ? 'Review pull request'
          : row.kind === 'diff'
            ? 'Review diff'
            : 'Review artifact'
      const notificationInput = new OrderSitesTransactionalEmailInput({
        appOrigin: getAppOrigin(env),
        ...(row.assignment_id === null
          ? {}
          : { assignmentId: row.assignment_id }),
        artifactLabel,
        artifactUrl: row.artifact_url,
        customerSafeStatus: 'Ready for review',
        displayName: row.display_name ?? email,
        eventRef: row.artifact_id,
        lifecycleKind: 'review_ready',
        nextAction:
          'Open your order status page, review the latest deliverable, and send any follow-up comment.',
        orderId: row.order_id,
        revisionUrl: null,
        safeReason: null,
        siteTitle: row.artifact_title,
        siteUrl: null,
        sourceAuthorityRefs: [
          'docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md#email-and-drip-campaign-plan',
          'system.review_ready_artifact_notification_reconciler.v1',
        ],
        targetRefs: [
          row.order_id,
          row.artifact_id,
          ...(row.assignment_id === null ? [] : [row.assignment_id]),
        ],
        to: email,
      })

      const result = await observedEffect(
        'Email.sendReviewReadyArtifactNotification',
        sendOrderSitesTransactionalEmailWithLedger(
          db,
          resend,
          new OrderSitesTransactionalEmailInput({
            ...notificationInput,
            idempotencyKey:
              buildOrderSitesTransactionalEmailIdempotencyKey(
                notificationInput,
              ),
          }),
          {
            actorUserId: 'system:review-ready-artifact-reconciler',
            metadata: {
              artifactId: row.artifact_id,
              assignmentId: row.assignment_id,
              eventSource: 'review_ready_artifact_notification_reconciler',
              lifecycleKind: 'review_ready',
              softwareOrderId: row.order_id,
              stage: 'review_ready',
            },
            sourceAuthorityRef:
              'system.review_ready_artifact_notification_reconciler.v1',
            targetUserId: row.target_user_id,
          },
        ),
      )

      return result.ok
        ? {
            emailMessageId: result.emailMessageId,
            emailStatus: 'accepted',
            providerMessageId: result.providerMessageId,
          }
        : {
            emailMessageId: result.emailMessageId,
            emailStatus: 'failed',
            skipReason: result.errorMessage,
          }
    })()
  const now = currentIsoTimestamp()
  const payload = reviewReadyArtifactNotificationPayloadJson({
    artifactId: row.artifact_id,
    artifactUrl: row.artifact_url,
    emailMessageId: notification.emailMessageId,
    emailStatus: notification.emailStatus,
    providerMessageId: notification.providerMessageId,
    skipReason: notification.skipReason,
    softwareOrderId: row.order_id,
    stage: 'review_ready',
  })
  const summary =
    notification.emailStatus === 'accepted'
      ? 'Autopilot customer artifact review-ready email notification was accepted.'
      : notification.emailStatus === 'failed'
        ? 'Autopilot customer artifact review-ready email notification failed.'
        : 'Autopilot customer artifact review-ready email notification is needed.'

  await db
    .prepare(
      `INSERT INTO adjutant_assignment_events
         (id,
          assignment_id,
          software_order_id,
          site_id,
          goal_id,
          run_id,
          event_type,
          visibility,
          summary,
          actor_user_id,
          payload_json,
          email_message_id,
          created_at)
       VALUES (?, ?, ?, NULL, ?, ?, 'adjutant.notification.review_ready_artifact', ?, ?, ?, ?, ?, ?)`,
    )
    .bind(
      compactRandomId('adjutant_assignment_event'),
      row.assignment_id,
      row.order_id,
      row.assignment_goal_id,
      row.assignment_current_run_id,
      row.assignment_visibility ?? 'public',
      summary,
      'system:review-ready-artifact-reconciler',
      payload,
      notification.emailMessageId,
      now,
    )
    .run()

  return notification
}

const sendPendingReviewReadyArtifactNotifications = async (
  env: Parameters<typeof getResendEmailConfig>[0],
): Promise<void> => {
  if (getResendEmailConfig(env) === undefined) {
    return
  }

  const pending = await readPendingReviewReadyArtifactNotifications(
    openAgentsDatabase(env),
  )

  await Promise.all(
    pending.map(row =>
      sendReviewReadyArtifactNotification(env, row).catch(error => {
        logWorkerRouteWarning('review_ready_artifact_notification_failed', {
          artifactId: row.artifact_id,
          error: errorMessage(error),
          orderId: row.order_id,
        })
      }),
    ),
  )
}

const enforceOutOfCreditsPolicy = async (
  env: Env,
  ctx: ExecutionContext | undefined,
  userId: string,
): Promise<void> => {
  const billing = await suspendBillingAccountIfOutOfCredits(
    openAgentsDatabase(env),
    userId,
  )

  if (!billing.exhausted) {
    return
  }

  const canceledRuns = await cancelActiveAgentRunsForBillingExhaustion(
    openAgentsDatabase(env),
    userId,
    {
      balanceCents: billing.balanceCents,
      balanceFormatted: billing.balanceFormatted,
    },
  )

  await Promise.all(
    canceledRuns.map(item => notifyAgentRunSyncScopes(env, item.run.id)),
  )

  const cleanup = Promise.all(
    canceledRuns.map(item => cleanupCanceledAgentRunOnShc(env, item.run)),
  ).then(() => undefined)
  const notify = sendOutOfCreditsNotificationOnce(env, {
    balanceCents: billing.balanceCents,
    balanceFormatted: billing.balanceFormatted,
    userId,
  })

  if (ctx === undefined) {
    await Promise.all([cleanup, notify])

    return
  }

  scheduleBackgroundWork(ctx, cleanup)
  scheduleBackgroundWork(ctx, notify)
}

const makeBillingAwareOmniRunStore = (env: Env, ctx?: ExecutionContext) =>
  makeD1OmniRunStore(openAgentsDatabase(env), {
    afterAgentRunMetered: run =>
      enforceOutOfCreditsPolicy(env, ctx, run.userId),
  })

const tokenUsageLeaderboardsLayer = (env: Env) =>
  TokenUsageLeaderboards.effectCfLayer().pipe(
    Layer.provide(OpenAgentsDatabase.layer),
    Layer.provide(Layer.succeed(WorkerEnvironment, env)),
  )

const emptyEmailCampaignDispatcherResult =
  (): EmailCampaignDispatcherResult => ({
    claimed: 0,
    failed: 0,
    retried: 0,
    sent: 0,
    skipped: 0,
    suppressed: 0,
  })

const emailCampaignDispatcherLayer = (env: EmailCampaignDispatcherBindings) =>
  OpenAgentsDatabase.layer.pipe(
    Layer.provide(Layer.succeed(WorkerEnvironment, env)),
  )

const dispatchDueEmailCampaignSendsScheduled = (
  env: EmailCampaignDispatcherBindings,
): Effect.Effect<EmailCampaignDispatcherResult, never> =>
  Effect.gen(function* () {
    const config = getOpenAgentsWorkerConfig(env)
    const db = yield* OpenAgentsDatabase

    return yield* dispatchDueEmailCampaignSends(db, {
      appOrigin: config.app.origin,
      resend: config.email.resend,
    })
  }).pipe(
    Effect.provide(emailCampaignDispatcherLayer(env)),
    Effect.catch(() => Effect.succeed(emptyEmailCampaignDispatcherResult())),
  )

const runArtanisScheduledTickScheduled = (
  db: D1Database,
  scheduledRunnerEnabled: boolean,
  scheduledTime: number,
): Effect.Effect<void, never> =>
  runArtanisScheduledTickForWorker({
    db,
    khalaUnsupportedTriage: {
      feedbackStore: makeD1KhalaFeedbackStore(db),
      traceReviewStore: makeD1KhalaTraceReviewStore(db),
      unsupportedRequestStore: makeD1KhalaUnsupportedRequestStore(db),
    },
    scheduledRunnerEnabled,
    scheduledTime,
  }).pipe(
    Effect.asVoid,
    Effect.catch(() => Effect.void),
  )

const recordPylonCapacityFunnelSnapshotsScheduled = (
  db: D1Database,
  scheduledTime: number,
): Effect.Effect<void, never> =>
  Effect.tryPromise({
    catch: () => 'pylon_capacity_funnel_snapshot_failed' as const,
    try: () =>
      recordPylonCapacityFunnelSnapshots({
        nowIso: epochMillisToIsoTimestamp(scheduledTime),
        snapshotStore: makeD1PylonCapacityFunnelSnapshotStore(db),
        store: makeD1PylonApiStore(db),
      }),
  }).pipe(
    Effect.asVoid,
    Effect.catch(() => Effect.void),
  )

// Public relay health probe (#4865): scheduled NIP-11 + websocket
// REQ/EOSE probe of the canonical market relay. The tick guards its own
// 5-minute cadence internally because the worker cron fires every minute,
// and the probe timestamp authority is the scheduled controller time.
const runRelayHealthProbeScheduled = (
  env: Env,
  scheduledTime: number,
): Effect.Effect<void, never> =>
  Effect.tryPromise({
    catch: () => 'relay_health_probe_failed' as const,
    try: () =>
      runRelayHealthProbeTick({
        // Service binding preferred for the NIP-11 leg: same-zone plain-GET
        // subrequests to the relay's custom domain fail from inside this
        // worker; the binding invokes the relay worker directly (#4865).
        fetchFn: env.MARKET_RELAY_SERVICE
          ? (((url, init) =>
              (env.MARKET_RELAY_SERVICE as Fetcher).fetch(
                url,
                init,
              )) as RelayHealthFetch)
          : undefined,
        makeId: randomUuid,
        relayUrl: canonicalMarketRelayUrl(env),
        scheduledTimeMs: scheduledTime,
        store: makeD1RelayHealthStore(openAgentsDatabase(env)),
      }),
  }).pipe(
    Effect.asVoid,
    Effect.catch(() => Effect.void),
  )

// Self-serve open-window producer (#5396). Keeps a small, hard-capped pool of
// openly-claimable `auto_starter` windows on the live Tassadar run so a fresh
// contributor's `pylon training claim` finds work instead of "no claimable
// window". listClaimableWindows applies the exact active+unleased claimability
// filter, so we only ever top up to TARGET claimable windows — bounding spend:
// each window can settle at most one verified worker+validator pair (5+5 sats)
// within the armed settlement-gate caps before it is consumed and replenished.
const SELF_SERVE_WINDOW_RUN_REF = 'run.tassadar.executor.20260615'
const SELF_SERVE_WINDOW_TARGET = 2

const runSelfServeWindowProducerScheduled = (
  env: Env,
  scheduledTime: number,
): Effect.Effect<void, never> =>
  Effect.tryPromise({
    catch: () => 'self_serve_window_producer_failed' as const,
    try: async () => {
      const store = makeD1TrainingAuthorityStore(openAgentsDatabase(env))
      const nowIso = epochMillisToIsoTimestamp(scheduledTime)
      const claimable = await store.listClaimableWindows(nowIso, 50)
      const openSelfServe = claimable.filter(
        window =>
          window.trainingRunRef === SELF_SERVE_WINDOW_RUN_REF &&
          window.homeworkKind === 'auto_starter',
      )
      const toCreate = Math.max(
        0,
        SELF_SERVE_WINDOW_TARGET - openSelfServe.length,
      )

      for (let index = 0; index < toCreate; index += 1) {
        const planned = await store.planWindow(
          buildTrainingWindowRecord({
            makeId: randomUuid,
            nowIso,
            request: {
              datasetRefs: ['dataset.public.tassadar.kernel_trace'],
              homeworkKind: 'auto_starter',
              priority: 1,
              receiptRefs: [
                'receipt.public.tassadar.window.self_serve_open.producer.plan',
              ],
              sourceRefs: ['source.public.tassadar.executor.self_serve_open'],
              trainingRunRef: SELF_SERVE_WINDOW_RUN_REF,
            },
          }),
        )
        const transitioned = transitionTrainingWindowRecord({
          actorRef: 'operator.openagents.self_serve_window_producer',
          eventId: randomUuid(),
          nextState: 'active',
          nowIso,
          receiptRef: `receipt.public.tassadar.window.self_serve_open.producer.activate.${planned.id}`,
          transitionKind: 'window_activate',
          window: planned,
        })
        await store.transitionWindow(transitioned.window, transitioned.event)
      }
    },
  }).pipe(
    Effect.asVoid,
    Effect.catch(() => Effect.void),
  )

const readTokenUsageLeaderboardsForUser = (
  env: Env,
  userId: string,
): Promise<AutopilotTokenLeaderboards> =>
  Effect.runPromise(
    Effect.gen(function* () {
      const tokenUsageLeaderboards = yield* TokenUsageLeaderboards

      return yield* Effect.promise(() =>
        tokenUsageLeaderboards.readForUser(userId),
      )
    }).pipe(Effect.provide(tokenUsageLeaderboardsLayer(env))),
  )

const readSelectedOperatorTargetUser = (
  db: D1Database,
  selector: Record<string, unknown>,
): Promise<OperatorTargetUser | undefined> =>
  readOperatorTargetUser(db, selector, OPENAGENTS_ADMIN_EMAILS[0])

// Kind-agnostic target resolver for the inference-credit grant (human OR agent
// account) — the bridge funds `agent:<userId>` for either, and an agent account
// under test is a valid target.
const readSelectedInferenceCreditTargetUser = (
  db: D1Database,
  selector: Record<string, unknown>,
): Promise<OperatorTargetUser | undefined> =>
  readSelectedInferenceCreditTargetUserBase(
    db,
    selector,
    OPENAGENTS_ADMIN_EMAILS[0],
  )

const sweepActiveAgentRunBilling = async (
  env: Env,
  ctx?: ExecutionContext,
): Promise<void> => {
  const billUntil = workerRuntime.nowIso()
  const activeRuns = await listActiveAgentRunsForBilling(
    openAgentsDatabase(env),
    100,
  )

  for (const run of activeRuns) {
    await recordContainerUsageDebitForRun(openAgentsDatabase(env), run, {
      billUntil,
    })
    await enforceOutOfCreditsPolicy(env, ctx, run.userId)
  }
}

const handleAdminSyncNotifyApi = async (
  request: Request,
  env: Env,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  if (!(await requireAdminApiToken(request, env))) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }

  const body = await readJsonObject(request).catch(
    (): Record<string, unknown> => ({}),
  )
  const scopes = [
    ...stringArrayFromUnknown(body.scopes),
    ...(optionalString(body.scope) === undefined
      ? []
      : [optionalString(body.scope)]),
  ].filter((scope): scope is string => scope !== undefined)

  if (scopes.length === 0) {
    return noStoreJsonResponse(
      { error: 'bad_request', reason: 'scope or scopes is required' },
      { status: 400 },
    )
  }

  await notifySyncScopes(env, scopes)

  return noStoreJsonResponse({
    ok: true,
    scopes: [...new Set(scopes)],
  })
}

const providerDeviceLoginSecretKey = (attemptId: string): string =>
  `provider-device-login:${attemptId}`

const providerAuthSecretKey = (providerAccountRef: string): string =>
  `provider-auth:${providerAccountRef}`

const providerSecretRef = (providerAccountRef: string): string =>
  `codex-auth://${providerAccountRef}`

const startedDeviceLoginTtlSeconds = (expiresAt: string): number => {
  const milliseconds = Date.parse(expiresAt) - workerRuntime.now().getTime()

  return Math.max(60, Math.ceil(milliseconds / 1000))
}

const storeStartedCodexDeviceLogin =
  (kv: KVNamespace) =>
  async (
    input: Readonly<{
      attemptId: string
      deviceAuthId: string
      userCode: string
      expiresAt: string
    }>,
  ): Promise<void> => {
    await kv.put(
      providerDeviceLoginSecretKey(input.attemptId),
      JSON.stringify({
        deviceAuthId: input.deviceAuthId,
        userCode: input.userCode,
      }),
      { expirationTtl: startedDeviceLoginTtlSeconds(input.expiresAt) },
    )
  }

const readStartedCodexDeviceLogin =
  (kv: KVNamespace) =>
  async (
    attemptId: string,
  ): Promise<
    Readonly<{ deviceAuthId: string; userCode: string }> | undefined
  > => {
    const value = await kv.get(providerDeviceLoginSecretKey(attemptId), 'json')

    if (
      !isRecord(value) ||
      typeof value.deviceAuthId !== 'string' ||
      typeof value.userCode !== 'string'
    ) {
      return undefined
    }

    return {
      deviceAuthId: value.deviceAuthId,
      userCode: value.userCode,
    }
  }

const deleteStartedCodexDeviceLogin =
  (kv: KVNamespace) =>
  async (attemptId: string): Promise<void> => {
    await kv.delete(providerDeviceLoginSecretKey(attemptId))
  }

const storeConnectedProviderApiKey =
  (kv: KVNamespace) =>
  async (
    input: Readonly<{
      providerAccountRef: string
      provider: 'anthropic_claude' | 'google_gemini'
      apiKey: string
    }>,
  ): Promise<void> => {
    const providerField =
      input.provider === 'anthropic_claude' ? 'anthropic' : 'google'

    await kv.put(
      providerAuthSecretKey(input.providerAccountRef),
      JSON.stringify({
        [providerField]: {
          type: 'api_key',
          key: input.apiKey,
        },
      }),
    )
  }

const storeConnectedCodexAuth =
  (kv: KVNamespace) =>
  async (
    input: Readonly<{
      providerAccountRef: string
      auth: CodexOAuthAuth
    }>,
  ): Promise<string> => {
    const secretRef = providerSecretRef(input.providerAccountRef)

    await kv.put(
      providerAuthSecretKey(input.providerAccountRef),
      JSON.stringify({
        openai: input.auth,
      }),
    )

    return secretRef
  }

const readConnectedCodexAuthMaterial = async (
  kv: KVNamespace,
  providerAccountRef: string,
): Promise<
  | Readonly<{
      authContentEnv: 'OPENCODE_AUTH_CONTENT'
      authContentJson: string
    }>
  | undefined
> => {
  const raw = await kv.get(providerAuthSecretKey(providerAccountRef), 'text')

  if (raw === null) {
    return undefined
  }

  const parsed = safeJsonRecord(raw)

  if (!isRecord(parsed) || !isRecord(parsed.openai)) {
    return undefined
  }

  const openai = parsed.openai

  if (
    optionalString(openai.type) !== 'oauth' ||
    optionalString(openai.access) === undefined ||
    optionalString(openai.refresh) === undefined
  ) {
    return undefined
  }

  return {
    authContentEnv: 'OPENCODE_AUTH_CONTENT',
    authContentJson: JSON.stringify(parsed),
  }
}

export const handleProgrammaticAgentRegistration = async (
  request: Request,
  env: Env,
  agentRegistrationStore?: AgentRegistrationStore,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const body = await request.json().catch(error => ({
    parseError: errorMessage(error),
  }))
  let parsed: typeof ProgrammaticAgentRegistrationRequest.Type

  try {
    parsed = decodeUnknownWithSchema(ProgrammaticAgentRegistrationRequest, body)
  } catch (error) {
    return withAgentRateLimitHeaders(badRequest(errorMessage(error)))
  }

  const store =
    agentRegistrationStore ??
    makeD1AgentRegistrationStore(openAgentsDatabase(env))

  try {
    const registration = await createProgrammaticAgentRegistration(
      store,
      parsed,
    )

    const autoClaimSparkAddress =
      typeof parsed.sparkAddress === 'string' &&
      parsed.sparkAddress.trim() !== ''
        ? parsed.sparkAddress.trim()
        : null
    const autoClaimLightningAddress =
      typeof parsed.lightningAddress === 'string' &&
      parsed.lightningAddress.trim() !== ''
        ? parsed.lightningAddress.trim()
        : null
    const autoClaimBolt12Offer =
      typeof parsed.bolt12Offer === 'string' && parsed.bolt12Offer.trim() !== ''
        ? parsed.bolt12Offer.trim()
        : null

    if (
      autoClaimSparkAddress !== null ||
      autoClaimLightningAddress !== null ||
      autoClaimBolt12Offer !== null
    ) {
      // Automatically register the tip wallet so the user doesn't have to call
      // claim-tip-wallet. Native Spark (offline-receive `spark1...` address) is
      // the preferred default agent rail; a Spark Lightning Address is the
      // online-receive path; BOLT 12 remains accepted for legacy registrations.
      const { upsertForumTipRecipientWallet } =
        await import('./forum/repository')
      const db = openAgentsDatabase(env)
      const sparkAddressPrimary = autoClaimSparkAddress !== null
      const lightningAddressPrimary =
        !sparkAddressPrimary && autoClaimLightningAddress !== null

      const readinessRefs = sparkAddressPrimary
        ? [
            'readiness.public.spark_address.offline_receive_ready',
            'readiness.public.spark_primary.agent_balance',
          ]
        : lightningAddressPrimary
        ? [
            'readiness.public.spark_lightning_address.receive_ready',
            'readiness.public.spark_primary.agent_balance',
          ]
        : [
            'readiness.public.mdk_agent.daemon_running',
            'readiness.public.mdk_agent.receive_ready',
            'readiness.public.mdk_agent.setup_present',
          ]
      const custodyPolicyRefs = sparkAddressPrimary
        ? [
            'policy.public.forum_tip_recipient.self_custody_mdk_agent_wallet',
            'policy.public.forum_tip_recipient.spark_self_custody',
          ]
        : lightningAddressPrimary
        ? ['policy.public.forum_tip_recipient.spark_self_custody']
        : ['policy.public.forum_tip_recipient.self_custody_mdk_agent_wallet']

      await Effect.runPromise(
        upsertForumTipRecipientWallet(db, {
          actorRef: `agent:${registration.user.id}`,
          sparkAddress: autoClaimSparkAddress,
          bolt12Offer: autoClaimBolt12Offer,
          lightningAddress: autoClaimLightningAddress,
          caveatRefs: [
            'caveat.public.forum_tip_recipient.creator_settlement_pending',
          ],
          claimPolicyRefs: [
            'policy.public.forum_tip_recipient.agent_registration_auto_claimed',
          ],
          custodyPolicyRefs,
          disabledAt: null,
          id: `forum_tip_recipient_wallet.user_${registration.user.id}.auto_claim`,
          payoutTargetApprovalRef: null,
          providerClass: lightningAddressPrimary
            ? 'external_lightning'
            : 'mdk_agent_wallet',
          readinessRefs,
          receiveCapabilityRef: `receive_capability.public.auto_${registration.user.id}.redacted`,
          sourceRef:
            'source.public.forum_tip_recipient.agent_registration_auto_claim',
          state: 'ready',
          walletRef: `wallet.public.auto_${registration.user.id}.redacted`,
        }),
      )
    }

    return withAgentRateLimitHeaders(
      jsonResponse(registration, { status: 201 }),
    )
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      return withAgentRateLimitHeaders(
        jsonResponse({ error: 'agent_registration_conflict' }, { status: 409 }),
      )
    }

    return serverError()
  }
}

// #6370: admin-only override to re-issue a fresh token for an EXISTING forum
// agent identity (dead-token recovery). Authorized EXACTLY like other admin
// routes: a valid admin API token OR an admin browser session
// (isOpenAgentsAdminEmail). It mints a NEW active credential bound to the SAME
// agent user/entity (same userId, slug, actorRef) — it never creates a new
// agent and never changes the slug/displayName. The raw token is returned to
// the admin caller once and is never logged.
export const handleAdminReissueAgentToken = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
  options?: Readonly<{
    agentRegistrationStore?: AgentReissueStore
    authorize?: (
      request: Request,
      env: Env,
      ctx: ExecutionContext,
    ) => Promise<boolean>
  }>,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }

  const authorize =
    options?.authorize ??
    (async (authRequest, authEnv, authCtx): Promise<boolean> => {
      if (await requireAdminApiToken(authRequest, authEnv)) {
        return true
      }

      const session = await requireBrowserSession(authRequest, authEnv, authCtx)

      return (
        session !== undefined && isOpenAgentsAdminEmail(session.user.email)
      )
    })

  if (!(await authorize(request, env, ctx))) {
    return forbidden()
  }

  const body = await request.json().catch(error => ({
    parseError: errorMessage(error),
  }))
  let parsed: typeof ReissueAgentTokenRequest.Type

  try {
    parsed = decodeUnknownWithSchema(ReissueAgentTokenRequest, body)
  } catch (error) {
    return badRequest(errorMessage(error))
  }

  const selector =
    parsed.slug !== undefined
      ? ({ slug: parsed.slug } as const)
      : parsed.externalId !== undefined
      ? ({ externalId: parsed.externalId } as const)
      : undefined

  if (selector === undefined) {
    return badRequest('slug or externalId is required')
  }

  const store =
    options?.agentRegistrationStore ??
    makeD1AgentRegistrationStore(openAgentsDatabase(env))

  try {
    const reissue = await reissueProgrammaticAgentToken(store, selector)

    if (reissue === undefined) {
      return jsonResponse({ error: 'agent_not_found' }, { status: 404 })
    }

    return jsonResponse(
      {
        token: reissue.token,
        tokenPrefix: reissue.tokenPrefix,
        slug: reissue.slug,
        actorRef: reissue.actorRef,
      },
      { status: 201 },
    )
  } catch {
    return serverError()
  }
}

// Khala FREE API MODE self-serve mint (issue #6228). Mints a rate-limited FREE
// `oa_agent_` API key that may call the single public model `openagents/khala`
// (own-infra GPT-OSS / Gemini Flash) with NO funded balance, within a per-key
// daily quota (the balance-gate bypass + zero-debit metering live in
// inference-free-tier-key.ts). It REUSES the existing agent-registration auth +
// token plumbing — a free key is a normal agent credential, just tagged free
// tier — so there is no parallel auth/inference stack. ABUSE-RESISTANT: minting
// is bounded per client IP per UTC day (no unbounded minting). The simplest safe
// option is anonymous + IP-rate-limited (no email required); an optional label is
// only a display name. The raw IP is hashed (SHA-256), never stored or logged.
//
// INERT until INFERENCE_FREE_TIER_ENABLED is on: the endpoint returns 404 so the
// surface is honestly absent until free mode is armed.
export const handleFreeKeyMint = async (
  request: Request,
  env: Env,
  agentRegistrationStore?: AgentRegistrationStore,
): Promise<Response> => {
  if (request.method !== 'POST') {
    return methodNotAllowed(['POST'])
  }
  if (!isFreeTierEnabled(env.INFERENCE_FREE_TIER_ENABLED)) {
    return notFound()
  }

  const body = (await request.json().catch(() => ({}))) as {
    label?: unknown
  }
  const label = sanitizeFreeKeyLabel(
    typeof body?.label === 'string' ? body.label : null,
  )

  const db = openAgentsDatabase(env)
  const clientIp =
    request.headers.get('cf-connecting-ip') ??
    request.headers.get('x-forwarded-for')?.split(',')[0]?.trim() ??
    'unknown'
  const ipHash = await sha256Hex(clientIp)
  const nowIso = currentIsoTimestamp()
  const mintDay = nowIso.slice(0, 10)

  // ABUSE GUARD: bound the number of free keys one IP can mint per UTC day.
  // Env-overridable (FREE_KEY_MAX_MINTS_PER_IP_PER_DAY) so the cap can be raised
  // without a deploy if ops/canaries need fresh keys during an incident (AAR
  // 2026-06-25).
  const mintsToday = await readFreeKeyMintsToday(db, ipHash, mintDay)
  const mintGate = decideFreeKeyMint({
    mintsToday,
    maxMintsPerDay: resolveFreeKeyMintCap(env),
  })
  if (!mintGate.allowed) {
    return withAgentRateLimitHeaders(
      jsonResponse(
        {
          error: 'free_key_mint_rate_limited',
          maxMintsPerDay: mintGate.maxMintsPerDay,
          reason: mintGate.reasonRef,
        },
        { status: 429 },
      ),
    )
  }

  const store = agentRegistrationStore ?? makeD1AgentRegistrationStore(db)

  try {
    const registration = await createProgrammaticAgentRegistration(store, {
      displayName: label,
    })
    const accountRef = `agent:${registration.user.id}`
    await markAccountFreeTierAsync(db, {
      accountRef,
      mintSource: 'self_serve_anonymous',
    })
    await recordFreeKeyMintAsync(db, { ipHash, mintDay })

    return withAgentRateLimitHeaders(
      jsonResponse(
        {
          tier: 'free',
          model: 'openagents/khala',
          credential: {
            token: registration.credential.token,
            tokenPrefix: registration.credential.tokenPrefix,
            createdAt: registration.credential.createdAt,
          },
          quota: {
            maxRequestsPerDay: resolveFreeTierQuota(env).maxRequestsPerDay,
            maxTokensPerDay: resolveFreeTierQuota(env).maxTokensPerDay,
            window: 'utc_day',
          },
          usage:
            'Send this token as the Authorization: Bearer credential to POST /api/v1/chat/completions with {"model":"openagents/khala"}. Free within the daily quota; beyond it, add credits.',
          // DATA-SHARING DISCLOSURE (#6296). Honest, code-accurate terms for the
          // free tier: free usage is captured by default as redacted, private
          // (owner_only) traces that may be used to improve/train models; pay for
          // privacy (or use confidential compute) to opt out; public sharing is
          // opt-in only. The same canonical disclosure is served at
          // GET /api/public/free-tier-data-sharing for agents.
          dataSharing: freeTierDataSharingDisclosure(),
        },
        { status: 201 },
      ),
    )
  } catch (error) {
    if (isUniqueConstraintError(error)) {
      return withAgentRateLimitHeaders(
        jsonResponse({ error: 'free_key_mint_conflict' }, { status: 409 }),
      )
    }
    return serverError()
  }
}

const agentBalanceAuthForStore =
  (store: ReturnType<typeof makeD1AgentRegistrationStore>) =>
  async (request: Request): Promise<{ actorRef: string } | undefined> => {
    const bearerToken = readBearerToken(request)
    if (bearerToken === undefined) {
      return undefined
    }
    const session = await authenticateProgrammaticAgent(store, bearerToken)
    return session === undefined
      ? undefined
      : { actorRef: `agent:${session.user.id}` }
  }

const handleProgrammaticAgentMe = async (
  request: Request,
  env: Env,
): Promise<Response> => {
  // #5333: self-serve agent displayName rename lives on PATCH /api/agents/me,
  // the same agent-self surface that GET reads from. GET keeps returning the
  // identity projection; PATCH updates the agent's own user row.
  if (request.method === 'PATCH') {
    return handleProgrammaticAgentSelfUpdate(request, openAgentsDatabase(env))
  }

  if (request.method !== 'GET') {
    return methodNotAllowed(['GET', 'PATCH'])
  }

  const bearerToken = readBearerToken(request)

  if (bearerToken === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  const session = await authenticateProgrammaticAgent(
    makeD1AgentRegistrationStore(openAgentsDatabase(env)),
    bearerToken,
  )

  if (session === undefined) {
    return withAgentRateLimitHeaders(unauthorized())
  }

  return withAgentRateLimitHeaders(
    jsonResponse({ authenticated: true, agent: session }),
  )
}

const routeAuthHostRequest = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> => {
  const url = new URL(request.url)

  if (url.pathname === '/' || url.pathname === '/login') {
    return redirectResponse(getAppOrigin(env))
  }

  if (url.pathname === '/github/callback') {
    const state = url.searchParams.get('state')

    if (state !== null) {
      const attempt = await makeD1GitHubWriteRepository(
        openAgentsDatabase(env),
      ).findAttemptByState(state)

      if (attempt !== undefined) {
        return handleGitHubWriteCallback(request, env, attempt)
      }
    }
  }

  return routeAuthIssuerRequest(request, env, ctx)
}

const routeAuthIssuerRequest = async (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
) => {
  const maybeGuardResponse = await maybeAuthEmailOtpGuardResponse(request, env)

  return maybeGuardResponse ?? makeAuthIssuer(env).fetch(request, env, ctx)
}

export { findAuthorizedAgentRunBundle } from './thread-access'

const isRouteAccessError = (
  value: AgentRunBundle | RouteAccessError,
): value is RouteAccessError =>
  value instanceof RouteAccessForbidden || value instanceof RouteAccessNotFound

const threadRouteAccessBundle = (
  env: Env,
  userId: string,
  routeId: string,
): Promise<AgentRunBundle | RouteAccessError> =>
  Effect.runPromise(
    Effect.gen(function* () {
      const threadAccess = yield* ThreadAccessService

      return yield* threadAccess.findAuthorizedBundle({ routeId, userId }).pipe(
        Effect.match({
          onFailure: error => error,
          onSuccess: bundle => bundle,
        }),
      )
    }).pipe(Effect.provide(ThreadAccessService.layer(env))),
  )

const threadRouteAccessError = async (
  env: Env,
  userId: string,
  routeId: string,
): Promise<RouteAccessError | undefined> => {
  const accessResult = await threadRouteAccessBundle(env, userId, routeId)

  return isRouteAccessError(accessResult) ? accessResult : undefined
}

const authorizeSyncPath = async (
  env: Env,
  session: VerifiedSession,
  syncPath: ParsedSyncPath,
): Promise<RouteAccessError | undefined> => {
  if (syncPath.kind === 'workspace') {
    return syncPath.id === session.user.userId
      ? undefined
      : new RouteAccessForbidden({ routeId: syncPath.id })
  }

  if (syncPath.kind === 'team') {
    const role =
      (await readActiveTeamMembershipRole(
        openAgentsDatabase(env),
        syncPath.id,
        session.user.userId,
      )) ?? undefined

    return role === undefined
      ? new RouteAccessForbidden({ routeId: syncPath.id })
      : undefined
  }

  if (syncPath.kind === 'thread' || syncPath.kind === 'agent-run') {
    return threadRouteAccessError(env, session.user.userId, syncPath.id)
  }

  return new RouteAccessForbidden({ routeId: syncPath.id })
}

const threadFileRoutes = makeThreadFileRoutes({
  appendRefreshedSessionCookies,
  publishTeamThreadFileSync,
  readActiveTeamMembershipRole,
  requireBrowserSession,
})

const agentSiteRoutes = makeAgentSiteRoutes({
  agentStoreForEnv: env =>
    makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  appendRefreshedSessionCookies,
  artifactsForEnv: env => env.ARTIFACTS,
  dbForEnv: openAgentsDatabase,
  isAdminEmail: isOpenAgentsAdminEmail,
  requireBrowserSession,
})

// Trace store + ingest/read API (openagents #6208/#6212, epic #6206): the
// shareable `/trace/{uuid}` surface. Agent-bearer ingest, visibility-gated read.
const traceStoreRoutes = makeTraceStoreRoutes({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  appendRefreshedSessionCookies,
  // Data-market revshare stub (#6221): INERT and owner-gated. Default OFF —
  // even when armed, the recorded marker is eligible-only with a TBD amount and
  // moves no money. Arm via TRACE_DATA_MARKET_REWARD_ENABLED.
  dataMarketRewardArmed: env =>
    isInferenceGatewayEnabled(
      (env as { TRACE_DATA_MARKET_REWARD_ENABLED?: string })
        .TRACE_DATA_MARKET_REWARD_ENABLED,
    ),
  isAdminEmail: isOpenAgentsAdminEmail,
  makeStore: env => makeD1TraceStore(openAgentsDatabase(env)),
  // Large-trajectory R2 offload (#6221): a multi-MB real agent session exceeds
  // D1's ~1MB value cap, so the public-safe trajectory JSON is stored in the
  // shared ARTIFACTS bucket with only a pointer kept in D1.
  trajectoryBlobStore: env => makeR2TraceTrajectoryBlobStore(env.ARTIFACTS),
  // Media blobs (#6223): the trace's playable recording + screenshots live in
  // the same ARTIFACTS bucket under `trace-blobs/{uuid}/{r2Key}` so the
  // `/trace/{uuid}` page serves its own media (never a GitHub attachment).
  mediaBlobStore: env => makeR2TraceMediaBlobStore(env.ARTIFACTS),
  requireBrowserSession,
})

const pylonCodexTurnIngestRoutes = makePylonCodexTurnIngestRoutes<Env>({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  ledger: env => makeD1TokenUsageLedger(openAgentsDatabase(env)),
  pylonStore: env => makeD1PylonApiStore(openAgentsDatabase(env)),
  proofStore: env => makeD1PylonCodexAssignmentProofStore(openAgentsDatabase(env)),
  traceStatusStore: env =>
    makeD1PylonCodexAssignmentProofStore(openAgentsDatabase(env)),
  rawEventChunkStore: env =>
    makeD1R2PylonCodexRawEventChunkStore(
      openAgentsDatabase(env),
      env.ARTIFACTS,
    ),
  rawEventStore: env =>
    makeD1R2PylonCodexRawEventStore(openAgentsDatabase(env), env.ARTIFACTS),
  publishDelta: (env, delta) =>
    Effect.promise(() =>
      publishKhalaTokensServedDelta(env, buildKhalaTokensServedDelta(delta)),
    ),
  traceStore: env => makeD1TraceStore(openAgentsDatabase(env)),
})

const hostedMdkClientForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) => {
  const checkout = getOpenAgentsWorkerConfig(env).mdk.checkout
  const routeSecret = redactedValue(checkout.routeSecret)

  if (
    !checkout.configured ||
    checkout.routeUrl === undefined ||
    routeSecret === undefined
  ) {
    return makeMissingOpenAgentsHostedMdkClient(checkout.providerRef)
  }

  return makeOpenAgentsHostedMdkRouteClient(
    {
      configRef: checkout.configRef,
      credentialBindingRef: checkout.credentialBindingRef,
      environment: checkout.environment,
      providerRef: checkout.providerRef,
      webhookBindingRef: checkout.webhookBindingRef,
    },
    {
      checkoutPathBase: checkout.checkoutPathBase,
      ...(checkout.routeKind === 'self_hosted_mdkd_sidecar'
        ? {
            fetch: (input, init) => {
              const request =
                input instanceof Request ? input : new Request(input, init)

              return fetchMdkSidecarRequest(request, env)
            },
          }
        : {}),
      routeSecret,
      routeUrl: checkout.routeUrl,
    },
  )
}

// The PRIMARY Spark-backed BOLT11 invoice issuer for the Lightning MPP rail
// (EPIC #6049). Owner directive: Spark is the primary rail for all agent/MPP
// payments (it supports OFFLINE RECEIVES). Returns undefined when the Spark
// treasury container is not reachable (the `MDK_TREASURY` binding is absent), so
// the selector can fall back to MDK. POSTs `/spark/funding-invoice` to the SAME
// `MDK_TREASURY` container the Spark payout/balance paths already reach
// (`fetchMdkTreasuryPath` → the `@breeztech/breez-sdk-spark` SDK) and reads the
// RAW bolt11 + decoded paymentHash. Both are public (they go into the 402
// challenge); the preimage is never seen here (verified LOCALLY in the Worker).
const sparkLightningInvoiceIssuerForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) => {
  const fetchTreasury = fetchMdkTreasuryPath(env)
  if (fetchTreasury === undefined) {
    return undefined
  }

  const post = async (
    body: Readonly<Record<string, unknown>>,
  ): Promise<Readonly<{ ok: boolean; status: number; payload: unknown }>> => {
    const response = await fetchTreasury('/spark/funding-invoice', {
      body: JSON.stringify(body),
      method: 'POST',
    })
    const payload = await response.json().catch(() => ({}))
    return { ok: response.ok, payload, status: response.status }
  }

  return makeSparkLightningInvoiceIssuer(post)
}

// The FALLBACK MDK-backed BOLT11 invoice issuer for the Lightning MPP rail
// (EPIC #6049). MDK is permitted ONLY as an explicit fallback Lightning issuer
// (never primary) and remains checkouts-only otherwise. Returns undefined when
// no MDK route is configured (route URL + secret). POSTs `create_checkout` to the
// SAME route/sidecar the Forum L402 flow uses (the `self_hosted_mdkd_sidecar`
// route kind goes through the MDK_SIDECAR container) and reads the RAW bolt11 +
// paymentHash. Uses the tighter FALLBACK mint timeout so a Spark primary timeout
// plus this MDK attempt together stay under the route's per-rail guard (#6149).
const mdkLightningInvoiceIssuerForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) => {
  const checkout = getOpenAgentsWorkerConfig(env).mdk.checkout
  const routeSecret = redactedValue(checkout.routeSecret)

  if (
    !checkout.configured ||
    checkout.routeUrl === undefined ||
    routeSecret === undefined
  ) {
    return undefined
  }

  const isSidecar = checkout.routeKind === 'self_hosted_mdkd_sidecar'
  const routeUrl = normalizeMdkLightningRouteUrl(checkout.routeUrl, {
    sidecar: isSidecar,
  })

  const post = async (
    body: Readonly<Record<string, unknown>>,
    options?: Readonly<{ signal?: AbortSignal | undefined }>,
  ): Promise<Readonly<{ ok: boolean; status: number; payload: unknown }>> => {
    const request = new Request(routeUrl, {
      body: JSON.stringify(body),
      headers: {
        'content-type': 'application/json',
        'x-moneydevkit-webhook-secret': routeSecret,
      },
      method: 'POST',
      ...(options?.signal === undefined ? {} : { signal: options.signal }),
    })
    const response = isSidecar
      ? await fetchMdkSidecarRequest(request, env)
      : await fetch(request)
    const payload = await response.json().catch(() => ({}))
    return { ok: response.ok, payload, status: response.status }
  }

  return makeMdkLightningInvoiceIssuer(
    post,
    MDK_LIGHTNING_FALLBACK_MINT_TIMEOUT_MS,
  )
}

// The Lightning MPP invoice issuer for this env: SPARK PRIMARY, MDK FALLBACK
// (EPIC #6049, owner directive). Tries Spark first; if Spark is
// unavailable/unconfigured/slow, falls back to the MDK Lightning issuer. Returns
// undefined only when NEITHER is reachable, so the Lightning rail is never
// offered without a working invoice issuer (honesty gate). The combined issuer
// keeps each leg's bounded mint timeout and stays under the route's per-rail
// guard (#6149) so a slow/failed issuer can only ever drop the Lightning rail.
const lightningInvoiceIssuerForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) =>
  makeFallbackLightningInvoiceIssuer(
    sparkLightningInvoiceIssuerForEnv(env),
    mdkLightningInvoiceIssuerForEnv(env),
  )

const forumL402SigningBoundaryForEnv = async (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) => {
  const checkout = getOpenAgentsWorkerConfig(env).mdk.checkout
  const routeSecret = redactedValue(checkout.routeSecret)

  if (
    !checkout.configured ||
    checkout.credentialBindingRef === null ||
    routeSecret === undefined
  ) {
    return null
  }

  return makeOpenAgentsL402HmacSigningBoundary({
    secretKeyMaterial: routeSecret,
    signerRef: checkout.credentialBindingRef,
  })
}

const hostedMdkWebhookConfigForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) => {
  const checkout = getOpenAgentsWorkerConfig(env).mdk.checkout
  const webhookSecret = redactedValue(checkout.webhookSecret)

  if (webhookSecret === undefined || checkout.webhookBindingRef === null) {
    return undefined
  }

  return {
    bindingRef: checkout.webhookBindingRef,
    secret: webhookSecret,
    source: checkout.webhookSource,
  }
}

const siteCommerceRoutesForEnv = (
  env: WorkerBindings & OpenAgentsWorkerConfigEnv,
) =>
  makeSiteCommerceRoutes({
    authorizeCommerceReviewDecision: request =>
      requireAdminApiToken(request, env),
    authorizePaidActionAgent: async request => {
      const bearerToken = readBearerToken(request)

      if (bearerToken === undefined) {
        return false
      }

      const session = await authenticateProgrammaticAgent(
        makeD1AgentRegistrationStore(openAgentsDatabase(env)),
        bearerToken,
      )

      return session !== undefined
    },
    authorizeMdkAccountBinding: request => requireAdminApiToken(request, env),
    authorizePayoutBridge: request => requireAdminApiToken(request, env),
    buyerPaymentLedgerStore: makeD1BuyerPaymentLedgerStore(
      openAgentsDatabase(env),
    ),
    challengeExpiresAt: () => isoTimestampAfter(currentDate(), 10 * 60_000),
    checkoutCatalog: omegaMdkDemoSitePaymentCatalog,
    checkoutIntentStore: makeD1SiteMdkCheckoutIntentStore(
      openAgentsDatabase(env),
    ),
    hostedMdkClient: hostedMdkClientForEnv(env),
    mdkWebhookConfig: hostedMdkWebhookConfigForEnv(env),
    mdkAccountBindingStore: makeD1SiteMdkAccountBindingStore(
      openAgentsDatabase(env),
    ),
    nowEpochMillis: () => currentDate().getTime(),
    nowIso: () => currentDate().toISOString(),
    payoutLedgerStore: makeD1NexusTreasuryPayoutLedgerStore(
      openAgentsDatabase(env),
    ),
    reviewStore: makeD1SiteCommerceReviewStore(openAgentsDatabase(env)),
  })

const siteReferralRoutes = makeSiteReferralRoutes()

const syncDependencyErrorReason = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

const syncRoutes = makeSyncRoutes({
  appendRefreshedSessionCookies,
  authorizeSyncPath: (env, session, syncPath) =>
    Effect.tryPromise({
      catch: syncDependencyErrorReason,
      try: () => authorizeSyncPath(env, session, syncPath),
    }),
  requireBrowserSession: (request, env, ctx) =>
    Effect.tryPromise({
      catch: syncDependencyErrorReason,
      try: () => requireBrowserSession(request, env, ctx),
    }),
})

const providerAccountBrowserHandlers = makeProviderAccountBrowserHandlers({
  appendRefreshedSessionCookies,
  deleteStartedCodexDeviceLogin,
  probeProviderApiKey: probeProviderApiKey(),
  providerAuthSecretKey,
  readStartedCodexDeviceLogin,
  requireBrowserSession,
  storeConnectedCodexAuth,
  storeConnectedProviderApiKey,
  storeStartedCodexDeviceLogin,
})

const providerAccountPylonHandlers = makeProviderAccountPylonHandlers({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  deleteStartedCodexDeviceLogin,
  readStartedCodexDeviceLogin,
  storeConnectedCodexAuth,
  storeStartedCodexDeviceLogin,
})

const pylonOpenAgentsAuthHandlers = makePylonOpenAgentsAuthHandlers({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  appendRefreshedSessionCookies,
  requireBrowserSession,
})

const providerAccountServiceHandlers = makeProviderAccountServiceHandlers({
  readConnectedCodexAuthMaterial,
  requireProviderServiceActor,
})

const providerAccountPoolRoutes = makeProviderAccountPoolRoutes({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  appendRefreshedSessionCookies,
  requireBrowserSession,
})

const providerAccountUsageRoutes = makeProviderAccountUsageRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

const operatorProviderAccountRoutes = makeOperatorProviderAccountRoutes({
  deleteStartedCodexDeviceLogin,
  readConnectedCodexAuthMaterial,
  readSelectedOperatorTargetUser,
  readStartedCodexDeviceLogin,
  requireAdminApiToken,
  storeConnectedCodexAuth,
  storeStartedCodexDeviceLogin,
})

const adminOverviewHandlers = makeAdminOverviewHandlers({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

// Admin-gated Cloudflare Browser Rendering smoke (#6205). Reuses the SAME admin
// mechanism as every other `/api/admin/*` route. `launch` defaults to the real
// `@cloudflare/playwright`; `env.BROWSER` is read off the runtime binding (not on
// the statically-typed `Env`).
const handleCfBrowserSmokeApi = makeCfBrowserSmokeHandler({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

const tokenUsageLedgerRoutes = makeTokenUsageLedgerRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireAdminApiToken,
  requireBrowserSession,
})

const mulletRoutes = makeMulletRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

const providerAccountRoutes = makeProviderAccountRoutes({
  handleGitHubWriteDisconnectApi: (request, env, ctx, connectionRef) =>
    routeEffect('handle_github_write_disconnect_api', () =>
      handleGitHubWriteDisconnectApi(request, env, ctx, connectionRef),
    ),
  handleProviderAccountDisconnectApi: (request, env, ctx, providerAccountRef) =>
    routeEffect('handle_provider_account_disconnect_api', () =>
      providerAccountBrowserHandlers.handleProviderAccountDisconnectApi(
        request,
        env,
        ctx,
        providerAccountRef,
      ),
    ),
  handleProviderAccountGrantIssueApi: (request, env, ctx, providerAccountRef) =>
    routeEffect('handle_provider_account_grant_issue_api', () =>
      providerAccountBrowserHandlers.handleProviderAccountGrantIssueApi(
        request,
        env,
        ctx,
        providerAccountRef,
      ),
    ),
  handleProviderAccountGrantResolveApi: (request, env) =>
    routeEffect('handle_provider_account_grant_resolve_api', () =>
      providerAccountServiceHandlers.handleProviderAccountGrantResolveApi(
        request,
        env,
      ),
    ),
  handleGoogleGeminiGrantResolveApi: (request, env) =>
    routeEffect('handle_google_gemini_grant_resolve_api', () =>
      providerAccountServiceHandlers.handleGoogleGeminiGrantResolveApi(
        request,
        env,
      ),
    ),
  handleGoogleGeminiBuiltinGrantApi: (request, env) =>
    routeEffect('handle_google_gemini_builtin_grant_api', () =>
      providerAccountServiceHandlers.handleGoogleGeminiBuiltinGrantApi(
        request,
        env,
      ),
    ),
  handleGoogleGeminiGenerateContentApi: (request, env, ctx, model) =>
    routeEffect('handle_google_gemini_generate_content_api', () =>
      providerAccountServiceHandlers.handleGoogleGeminiGenerateContentApi(
        request,
        env,
        ctx,
        model,
      ),
    ),
  handleProviderApiKeyConnectApi: (request, env, ctx, providerRouteSegment) =>
    routeEffect('handle_provider_api_key_connect_api', () =>
      providerAccountBrowserHandlers.handleProviderApiKeyConnectApi(
        request,
        env,
        ctx,
        providerRouteSegment,
      ),
    ),
  handleProviderAccountHealthApi: (request, env, providerAccountRef) =>
    routeEffect('handle_provider_account_health_api', () =>
      providerAccountServiceHandlers.handleProviderAccountHealthApi(
        request,
        env,
        providerAccountRef,
      ),
    ),
  handleProviderAccountPoolApi: (request, env, ctx) =>
    providerAccountPoolRoutes.handleProviderAccountPoolApi(request, env, ctx),
  handleProviderAccountUsageApi: (request, env, ctx) =>
    providerAccountUsageRoutes.handleProviderAccountUsageApi(request, env, ctx),
  handleProviderAccountsListApi: (request, env, ctx) =>
    routeEffect('handle_provider_accounts_list_api', () =>
      providerAccountBrowserHandlers.handleProviderAccountsListApi(
        request,
        env,
        ctx,
      ),
    ),
  handleProviderDeviceLoginConnectedApi: (request, env, attemptId) =>
    routeEffect('handle_provider_device_login_connected_api', () =>
      providerAccountServiceHandlers.handleProviderDeviceLoginConnectedApi(
        request,
        env,
        attemptId,
      ),
    ),
  handleProviderDeviceLoginFailedApi: (request, env, attemptId) =>
    routeEffect('handle_provider_device_login_failed_api', () =>
      providerAccountServiceHandlers.handleProviderDeviceLoginFailedApi(
        request,
        env,
        attemptId,
      ),
    ),
  handleProviderDeviceLoginStartApi: (request, env, ctx) =>
    routeEffect('handle_provider_device_login_start_api', () =>
      providerAccountBrowserHandlers.handleProviderDeviceLoginStartApi(
        request,
        env,
        ctx,
      ),
    ),
  handleProviderDeviceLoginStatusApi: (request, env, ctx, attemptId) =>
    routeEffect('handle_provider_device_login_status_api', () =>
      providerAccountBrowserHandlers.handleProviderDeviceLoginStatusApi(
        request,
        env,
        ctx,
        attemptId,
      ),
    ),
  handlePylonProviderDeviceLoginStartApi: (request, env) =>
    routeEffect('handle_pylon_provider_device_login_start_api', () =>
      providerAccountPylonHandlers.handlePylonProviderDeviceLoginStartApi(
        request,
        env,
      ),
    ),
  handlePylonProviderDeviceLoginStatusApi: (request, env, attemptId) =>
    routeEffect('handle_pylon_provider_device_login_status_api', () =>
      providerAccountPylonHandlers.handlePylonProviderDeviceLoginStatusApi(
        request,
        env,
        attemptId,
      ),
    ),
  handlePylonProviderLocalCodexAuthImportApi: (request, env) =>
    routeEffect('handle_pylon_provider_local_codex_auth_import_api', () =>
      providerAccountPylonHandlers.handlePylonProviderLocalCodexAuthImportApi(
        request,
        env,
      ),
    ),
  handlePylonOpenAgentsAuthStartApi: (request, env) =>
    pylonOpenAgentsAuthHandlers.handlePylonOpenAgentsAuthStartApi(request, env),
  handlePylonOpenAgentsAuthStatusApi: (request, env, attemptId) =>
    pylonOpenAgentsAuthHandlers.handlePylonOpenAgentsAuthStatusApi(
      request,
      env,
      attemptId,
    ),
  handlePylonOpenAgentsAuthVerifyApi: (request, env, ctx) =>
    pylonOpenAgentsAuthHandlers.handlePylonOpenAgentsAuthVerifyApi(
      request,
      env,
      ctx,
    ),
})

const billingApiHandlers = makeBillingApiHandlers({
  appendRefreshedSessionCookies,
  requireBrowserSession,
})

const onboardingRoutes = makeOnboardingRoutes({
  appendRefreshedSessionCookies,
  requireBrowserSession,
  siteReferralOnboarding: ({ env, orderState, referralResult, session }) =>
    sendSiteReferralOnboardingForConsumption(openAgentsDatabase(env), {
      appOrigin: getAppOrigin(env),
      displayName: session.user.name,
      email: session.user.email,
      orderState,
      referralResult,
      resend: getResendEmailConfig(env),
      userId: session.user.userId,
    }),
})

const siteReferralInspectionRoutes = makeSiteReferralInspectionRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

// Referral payout settlement adapter (RL-1 settle wire, #5511). This is the
// PRODUCTION hosted-MDK programmatic-payout adapter the shared dispatcher
// (`dispatchReferralPayoutSettlement`) invokes to record a `settled` referral
// payout with a real, redacted, dereferenceable receipt ref -- replacing the
// throwing placeholder that could never pay.
//
// OWNER-ARMED / INERT (the #5512 boundary): two independent gates keep this
// inert until the owner arms live payouts:
//   1. `readReadiness` below is the OWNER-ARMED OFF gate
//      (`hostedMdkDirectPayoutDisabledGate` -> `livePayoutClaimAllowed: false`),
//      so the dispatcher REFUSES before ever reaching the adapter; and
//   2. the adapter's `client` is null (no funded programmatic-payout client
//      armed) and its destination resolver returns null (no registered referrer
//      destination), so even if reached it FAILS CLOSED (throws) -- no money
//      moves and NO settled state is recorded.
// Arming the gate + configuring a funded client + a registered destination is
// the owner step (#5512); the rail itself is now wired and ready.
const referralPayoutSettlementAdapter = makeSiteReferralPayoutAdapter({
  // Not armed: no funded hosted-MDK programmatic-payout client is wired into the
  // referral rail yet. Fail closed until the owner arms it (#5512).
  client: null,
  // Not armed: referrer payout-destination registration is owner-gated (#5512).
  // Returns null so the adapter fails closed if ever reached.
  resolveDestination: async () => null,
})

const siteReferralPayoutLedgerRoutes = makeSiteReferralPayoutLedgerRoutes({
  dispatchDependencies: {
    adapter: referralPayoutSettlementAdapter,
    nowIso: currentIsoTimestamp,
    readReadiness: async () => hostedMdkDirectPayoutDisabledGate(),
  },
  nowIso: currentIsoTimestamp,
  requireAdminApiToken,
})

// Inference referral revshare routes (sub-EPIC #5475: #5491 dashboard read +
// #5490 dispatch). The dispatch readiness gate defaults to the OWNER-ARMED OFF
// gate, so the first real referral payout is owner-armed: dispatch REFUSES (no
// money moves, the adapter is never reached) until the owner arms a live payout
// mode. The adapter is now the real hosted-MDK rail (`referralPayoutSettlement
// Adapter`), which also fails closed (unconfigured client) on the not-armed
// path -- so the placeholder that could never pay is replaced by a real,
// readiness-gated rail.
const inferenceReferralRoutes = makeInferenceReferralRoutes({
  appendRefreshedSessionCookies,
  dispatchDependencies: {
    adapter: referralPayoutSettlementAdapter,
    nowIso: currentIsoTimestamp,
    readReadiness: async () => hostedMdkDirectPayoutDisabledGate(),
  },
  requireAdminApiToken,
  requireBrowserSession,
})

const agentGoalRoutes = makeAgentGoalRoutes({
  appendRefreshedSessionCookies,
  authenticateRequestActor,
  readActiveTeamMembershipRole,
  readActiveTeamProject,
  requireAdminApiToken,
  requireBrowserSession,
})

const autopilotOnboardingRoutes = makeAutopilotOnboardingRoutes<WorkerBindings>(
  {
    makeInferenceClient: env => makeOnboardingInferenceClient(env),
    makeStreamClient: env => makeOnboardingStreamClient(env),
    // DURABLE ONBOARDING STREAM (#6154 item 4). The per-request Durable Object
    // namespace, resolved only when the durable-stream flag is on AND the binding
    // is wired; absent => the onboarding stream stays the non-durable SSE
    // (fail-safe). Keyed by the stable id `onboarding:{sessionId}:{turnIndex}` so
    // an unauthenticated browser can resume a dropped turn by offset.
    resolveDurableStream: env => {
      // `env` is the full Worker `Env` at call time; the route's generic narrows it
      // to `WorkerBindings`, so read the config flag through the broader Env shape.
      const fullEnv = env as Env
      return isInferenceDurableStreamEnabled(
        fullEnv.INFERENCE_DURABLE_STREAM_ENABLED,
      ) && fullEnv.INFERENCE_DURABLE_STREAM !== undefined
        ? (fullEnv.INFERENCE_DURABLE_STREAM as unknown as DurableStreamNamespace)
        : undefined
    },
  },
)

const checkoutPageRoutes = makeCheckoutPageRoutes<WorkerBindings>({
  hostedMdkClient: env => hostedMdkClientForEnv(env),
})

const agentOwnerClaimRoutes = makeAgentOwnerClaimRoutes({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  appOrigin: getAppOrigin,
  appendRefreshedSessionCookies,
  makeStore: env => makeD1AgentOwnerClaimStore(openAgentsDatabase(env)),
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  requireBrowserSession,
})

const agentProposalRoutes = makeAgentProposalRoutes({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  appOrigin: getAppOrigin,
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  makeStore: env => makeD1AgentProposalStore(openAgentsDatabase(env)),
  recoveryStore: env =>
    makeD1AgentRateLimitRecoveryStore(openAgentsDatabase(env)),
  requireAdminApiToken,
  requireBrowserSession,
})

const agentSearchRoutes = makeAgentSearchRoutes({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
})

const autopilotWorkRouteDependencies = {
  agentStore: (env: WorkerBindings) =>
    makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  // Hosted Gemini executor binding (api.hosted_gemini.v1, yellow). DOUBLE-gated
  // and INERT by default: resolves an executor ONLY when
  // HOSTED_GEMINI_EXECUTOR_ENABLED is armed AND VERTEX_SA_KEY is present;
  // otherwise it resolves `undefined` (no execution, no closeout) — exactly the
  // prior behaviour when no executor was wired. Reads the config off the live
  // `Env` the route already receives; carries no secret into any closeout.
  executeReadyWork: makeHostedGeminiExecuteReadyWork(),
  l402SigningBoundary: (env: WorkerBindings) =>
    forumL402SigningBoundaryForEnv(env),
  makeBuyerPaymentLedgerStore: (env: WorkerBindings) =>
    makeD1BuyerPaymentLedgerStore(openAgentsDatabase(env)),
  makePylonApiStore: (env: WorkerBindings) =>
    makeD1PylonApiStore(openAgentsDatabase(env)),
  makeStore: (env: WorkerBindings) =>
    makeD1AutopilotWorkStore(openAgentsDatabase(env)),
  // Feed the registered-pylon registry into the work-order placement selector
  // so an owner's online, heartbeat-fresh Pylon is eligible for `requester_pylon`
  // placement (own jobs run on the owner's own node). Without this the selector
  // only ever sees an empty list and every order falls back to the SHC lane,
  // which is what blocked the spare-capacity provider from picking up its
  // owner's job (#4782). The selector itself enforces owner-match + active +
  // fresh-heartbeat eligibility.
  pylonRegistrations: (env: WorkerBindings) =>
    makeD1PylonApiStore(openAgentsDatabase(env)).listRegistrations(100),
  requireBrowserSession,
  verifyL402PaymentProof: (
    env: WorkerBindings,
    input: Parameters<typeof verifyAutopilotL402PaymentProofFromBuyerLedger>[1],
  ) =>
    verifyAutopilotL402PaymentProofFromBuyerLedger(
      makeD1BuyerPaymentLedgerStore(openAgentsDatabase(env)),
      input,
    ),
}

const autopilotWorkRoutes = makeAutopilotWorkRoutes<Env>(
  autopilotWorkRouteDependencies,
)

const autopilotContinuationPolicyRoutes =
  makeAutopilotContinuationPolicyRoutes<WorkerBindings>({
    agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
    makeStore: env => makeD1AutopilotContinuationStore(openAgentsDatabase(env)),
    requireBrowserSession,
  })

const autopilotMorningReportRoutes =
  makeAutopilotMorningReportRoutes<WorkerBindings>({
    agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
    makeContinuationStore: env =>
      makeD1AutopilotContinuationStore(openAgentsDatabase(env)),
    makeWorkStore: env => makeD1AutopilotWorkStore(openAgentsDatabase(env)),
    requireBrowserSession,
  })

const autopilotDecisionRoutes = makeAutopilotDecisionRoutes<WorkerBindings>({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  makeStore: env => makeD1AutopilotWorkStore(openAgentsDatabase(env)),
  requireBrowserSession,
})

const omniWorkroomRoutes = makeOmniWorkroomRoutes<WorkerBindings>({
  db: env => openAgentsDatabase(env),
  requireBrowserSession,
})

const omniWorkroomLifecycleRoutes =
  makeOmniWorkroomLifecycleRoutes<WorkerBindings>({
    makeDb: env => openAgentsDatabase(env),
    requireBrowserSession,
    requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  })

const omniBundleRoutes = makeOmniBundleRoutes<WorkerBindings>({
  db: env => openAgentsDatabase(env),
  requireOperator: (request, env) => requireAdminApiToken(request, env),
  readEvidenceBundle: (db, id) => readOmniEvidenceBundleById(db, id),
  readProofBundle: (db, id) => readOmniPublicProofBundleById(db, id),
})

const omniHandoffRoutes = makeOmniHandoffRoutes<WorkerBindings>({
  db: env => openAgentsDatabase(env),
  requireOperator: (request, env) => requireAdminApiToken(request, env),
})

const nativeListsRoutes = makeNativeListsRoutes<WorkerBindings>({
  makeStore: env => makeNativeListsService(openAgentsDatabase(env)),
  requireOperator: (request, env) => requireAdminApiToken(request, env),
})

// Site page form-capture wiring (#5523 / DE-9 #5532; promise
// autopilot_sites.native_email_sequences.v1, yellow). Default OFF via
// SITE_FORM_CAPTURE_ENABLED: when the flag is unset the route returns undefined
// for every request and the omni chain falls through exactly as today. When
// armed it resolves a page's FormCaptureSpec from the active site version's
// metadata_json (via site-form-spec-registry) and persists captured leads
// through the native-lists addSubscriber sink. The registry is the authority on
// whether a formId is published (spec.id === formId); the SQL only narrows
// candidate active versions whose metadata_json mentions the form key.
const sitePageFormCaptureRoutes = makeSitePageFormCaptureRoutes<WorkerBindings>(
  {
    isEnabled: env =>
      isSiteFormCaptureEnabled(
        (env as unknown as { SITE_FORM_CAPTURE_ENABLED?: string })
          .SITE_FORM_CAPTURE_ENABLED,
      ),
    makeSink: env => ({
      addSubscriber: makeNativeListsService(openAgentsDatabase(env))
        .addSubscriber,
    }),
    readSiteFormMetadata: async (env, formId) => {
      const row = await openAgentsDatabase(env)
        .prepare(
          `SELECT site_versions.metadata_json AS metadata_json
           FROM site_projects
           JOIN site_versions
             ON site_versions.id = site_projects.active_version_id
            AND site_versions.site_id = site_projects.id
          WHERE site_projects.archived_at IS NULL
            AND site_projects.active_version_id IS NOT NULL
            AND json_extract(site_versions.metadata_json, '$.formSpecs')
                IS NOT NULL
            AND instr(site_versions.metadata_json, ?1) > 0
          ORDER BY site_versions.created_at DESC
          LIMIT 25`,
        )
        .bind(formId)
        .all<{ metadata_json: string }>()
        .then(result => result.results)

      // The registry validates spec.id === formId and decodes defensively, so the
      // first candidate whose metadata actually publishes this formId wins; a
      // metadata blob that merely mentions the id as a substring resolves to
      // undefined here and is skipped.
      for (const candidate of row) {
        const spec = resolveSiteFormSpec(candidate.metadata_json, formId)
        if (spec !== undefined) {
          return candidate.metadata_json
        }
      }

      return undefined
    },
  },
)

const prefilledWorkspaceRoutes = makePrefilledWorkspaceRoutes<WorkerBindings>({
  makeStore: env => makePrefilledWorkspaceService(openAgentsDatabase(env)),
  requireHolderUserId: async (request, env, ctx) => {
    const session = await requireBrowserSession(request, env, ctx)

    return session?.user.userId
  },
  requireOperator: (request, env) => requireAdminApiToken(request, env),
})

const teamWorkspaceInviteRoutes = makeTeamWorkspaceInviteRoutes<
  WorkerBindings,
  VerifiedSession
>({
  appendRefreshedSessionCookies,
  appOrigin: getAppOrigin,
  getResendEmailConfig,
  makeStore: env => makeD1TeamWorkspaceInviteStore(openAgentsDatabase(env)),
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  requireBrowserSession,
  sendInviteEmailWithLedger: (env, config, input) =>
    sendPrivateWorkspaceInviteEmailWithLedger(
      openAgentsDatabase(env),
      config,
      input,
    ),
})

const privateProjectWorkspaceRoutes =
  makePrivateProjectWorkspaceRoutes<WorkerBindings>({
    appOrigin: getAppOrigin,
    getResendEmailConfig,
    makeInviteStore: env =>
      makeD1TeamWorkspaceInviteStore(openAgentsDatabase(env)),
    makePrivateProjectStore: env =>
      makeD1PrivateProjectWorkspaceStore(openAgentsDatabase(env)),
    makeWorkspaceStore: env =>
      makePrefilledWorkspaceService(openAgentsDatabase(env)),
    requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
    sendInviteEmailWithLedger: (env, config, input) =>
      sendPrivateWorkspaceInviteEmailWithLedger(
        openAgentsDatabase(env),
        config,
        input,
      ),
  })

const tenantClientRoutes = makeTenantClientRoutes({
  database: (env: WorkerBindings) => openAgentsDatabase(env),
  requireBrowserSession,
  resolveTenant: async (request: Request, env: WorkerBindings) => {
    const host = request.headers.get('Host') ?? ''
    const tenant = await Effect.runPromise(
      makeTenantCustomHostnames(
        openAgentsDatabase(env),
      ).resolveTenantByHostname(host),
    )
    return tenant ?? undefined
  },
})

// CUSTOMER self-serve custom-hostname routes (#4988 follow-up). Browser-session
// + team-role gated; writes only the tenant_custom_hostnames table (pending
// rows), never live DNS/SSL/origin binding/spend. Live provisioning to `active`
// stays the owner-gated provisioning core's job (default-OFF Cloudflare
// secrets), so config stays INERT here (servingLive=false, no live DNS check).
const tenantHostnameSelfServeRoutes = makeTenantHostnameSelfServeRoutes({
  database: (env: WorkerBindings) => openAgentsDatabase(env),
  requireBrowserSession,
  readTeamRole: (db, teamId, userId) =>
    readActiveTeamMembershipRole(db, teamId, userId),
})

const emailSequenceAuthoringRoutes = makeEmailSequenceAuthoringRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireAdminApiToken: (request: Request, env: WorkerBindings) =>
    requireAdminApiToken(request, env),
  requireBrowserSession,
})

const sitesOrchestrationRoutes = makeSitesOrchestrationRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireBrowserSession,
})

const partnerPayoutLedgerRoutes = makePartnerPayoutLedgerRoutes<WorkerBindings>(
  {
    dispatchDependencies: {
      adapter: {
        adapterKind: 'owner_armed_partner_payout',
        dispatch: async () => {
          throw new PartnerPayoutDispatchError(
            'partner_payout_adapter_unconfigured: owner has not armed a live partner payout rail',
          )
        },
      },
      nowIso: currentIsoTimestamp,
      readReadiness: async () => hostedMdkDirectPayoutDisabledGate(),
    },
    nowIso: currentIsoTimestamp,
    requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  },
)

const partnerAgreementRoutes = makePartnerAgreementRoutes<WorkerBindings>({
  nowIso: currentIsoTimestamp,
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
})

const crmRoutes = makeCrmRoutes<WorkerBindings>({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
})

const crmImportRoutes = makeCrmImportRoutes<WorkerBindings>({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
})

const crmEmailRoutes = makeCrmEmailRoutes<WorkerBindings>({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
})

const resolveCrmResendDeps = (env: WorkerBindings) => {
  const enabled = isCrmResendSendEnabled(
    (env as { CRM_RESEND_SEND_ENABLED?: string | undefined })
      .CRM_RESEND_SEND_ENABLED,
  )
  const resend = getResendEmailConfig(env)
  if (resend === undefined) {
    return { enabled, fromEmail: null, sender: null }
  }
  return {
    enabled,
    fromEmail: resend.fromEmail,
    sender: makeCrmResendSender({
      apiKey: resend.apiKey,
      replyTo: resend.replyToEmail,
    }),
  }
}

const crmResendRoutes = makeCrmResendRoutes<WorkerBindings>({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  resolveResendDeps: resolveCrmResendDeps,
})

const crmSendRoutes = makeCrmSendRoutes<WorkerBindings>({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  resolveResendDeps: resolveCrmResendDeps,
})

const crmCommandRoutes = makeCrmCommandRoutes<WorkerBindings>({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  resolveResendDeps: resolveCrmResendDeps,
})

const crmBatchRoutes = makeCrmBatchRoutes<WorkerBindings>({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  resolveResendDeps: resolveCrmResendDeps,
})

type KhalaMcpTokensServedDelta = Readonly<{
  eventRef: string
  observedAt: string
  tokensServedDelta: number
}>

const makeKhalaMcpServedTokensRecorder = (
  db: D1Database,
  options: Readonly<{
    nowIso?: () => string
    publishDelta?: (input: KhalaMcpTokensServedDelta) => Promise<void>
  }> = {},
): ((input: ServedTokensRecorderInput) => Promise<void>) => {
  const nowIso = options.nowIso ?? currentIsoTimestamp

  return async input => {
    const inputTokens = Math.max(0, Math.trunc(input.usage.promptTokens))
    const outputTokens = Math.max(0, Math.trunc(input.usage.completionTokens))
    const tokensServedDelta = inputTokens + outputTokens
    if (tokensServedDelta <= 0) {
      return
    }

    const observedAt = nowIso()
    const body = buildServedTokensIngestBody({
      accountRef: input.accountRef,
      adapterId: input.adapterId,
      observedAt,
      requestAttribution: input.requestAttribution,
      requestId: input.requestId,
      requestedModel: input.requestedModel,
      servedModel: input.servedModel,
      usage: input.usage,
    })
    const safeMetadataJson = JSON.stringify(body.safeMetadata ?? {})

    try {
      const result = await db
        .prepare(
          `INSERT OR IGNORE INTO token_usage_events (
            id,
            idempotency_key,
            observed_at,
            ingested_at,
            producer_system,
            source_route,
            actor_user_id,
            actor_team_id,
            account_ref,
            anonymized_source_ref,
            run_ref,
            session_ref,
            task_ref,
            repository_ref,
            provider,
            model,
            backend_profile,
            input_tokens,
            output_tokens,
            reasoning_tokens,
            cache_read_tokens,
            cache_write_5m_tokens,
            cache_write_1h_tokens,
            total_tokens,
            usage_truth,
            cost_amount,
            currency,
            demand_kind,
            demand_source,
            demand_client,
            leaderboard_eligible,
            privacy_opt_out,
            safe_metadata_json
          ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
        )
        .bind(
          body.eventId,
          body.idempotencyKey,
          body.observedAt,
          nowIso(),
          body.producerSystem,
          body.sourceRoute,
          null,
          null,
          body.actor?.accountRef ?? null,
          null,
          null,
          null,
          null,
          null,
          body.provider ?? null,
          body.model ?? null,
          body.backendProfile ?? null,
          body.tokenCounts.inputTokens,
          body.tokenCounts.outputTokens,
          body.tokenCounts.reasoningTokens,
          body.tokenCounts.cacheReadTokens,
          body.tokenCounts.cacheWrite5mTokens,
          body.tokenCounts.cacheWrite1hTokens,
          body.tokenCounts.totalTokens,
          body.usageTruth,
          body.cost?.amount ?? null,
          body.cost?.currency ?? null,
          body.demand?.demandKind ?? 'unlabeled',
          body.demand?.demandSource ?? null,
          body.demand?.demandClient ?? null,
          body.privacy?.leaderboardEligible === false ? 0 : 1,
          body.privacy?.privacyOptOut === true ? 1 : 0,
          safeMetadataJson,
        )
        .run()

      const inserted =
        Number((result.meta as D1Meta & { changes?: number }).changes ?? 0) > 0
      if (
        inserted &&
        options.publishDelta !== undefined &&
        servedTokensRowIsPublicCountable(input.requestAttribution)
      ) {
        await options
          .publishDelta({
            eventRef: body.eventId,
            observedAt,
            tokensServedDelta,
          })
          .catch(() => undefined)
      }
    } catch {
      return
    }
  }
}

// CRM MCP server (epic #5991): read-only catalog (#5993) + resources (#5994),
// authenticated to a bound principal — admin token = full CRM authority on the
// header/default tenant; a scoped grant (#5995) = its declared authorities +
// bound tenant. The catalog filters tools/resources by the principal's grants.
const crmMcpRoutes = makeCrmMcpRoutes<WorkerBindings>({
  authenticate: async (request, env) => {
    const isAdmin = await requireAdminApiToken(request, env)
    if (isAdmin) {
      return crmMcpAdminPrincipal(
        mcpTenantHeader(request),
        currentIsoTimestamp(),
      )
    }
    const token = readMcpBearerToken(request)
    if (token === undefined) {
      return null
    }
    const crmPrincipal = await resolveCrmMcpGrantPrincipal(
      openAgentsDatabase(env),
      token,
      currentIsoTimestamp(),
    )
    if (crmPrincipal !== null) {
      return crmPrincipal
    }
    const session = await authenticateProgrammaticAgent(
      makeD1AgentRegistrationStore(openAgentsDatabase(env)),
      token,
    )
    return session === undefined
      ? null
      : khalaMcpAgentPrincipal(session, currentIsoTimestamp())
  },
  catalog: combineMcpCatalogs<WorkerBindings>([
    makeCrmMcpCatalog<WorkerBindings>({
      resolveResendDeps: resolveCrmResendDeps,
    }),
    makeKhalaMcpCatalog<WorkerBindings>({
      agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
      pylonStore: env => makeD1PylonApiStore(openAgentsDatabase(env)),
      recordTokensServed: env =>
        makeKhalaMcpServedTokensRecorder(openAgentsDatabase(env), {
          publishDelta: delta =>
            publishKhalaTokensServedDelta(
              env,
              buildKhalaTokensServedDelta(delta),
            ).catch(() => undefined),
        }),
    }),
  ]),
})

const crmMcpGrantRoutes = makeCrmMcpGrantRoutes<WorkerBindings>({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
})

const crmMcpDiscoveryRoutes = makeCrmMcpDiscoveryRoutes()

const agentScopedGrantRoutes = makeAgentScopedGrantRoutes({
  requireAdminApiToken: (request, env) => requireAdminApiToken(request, env),
  appOrigin: getAppOrigin,
  appendRefreshedSessionCookies,
  makeStore: env => makeD1AgentScopedGrantStore(openAgentsDatabase(env)),
  requireBrowserSession,
})

const shareRoutes = makeShareRoutes({
  appendRefreshedSessionCookies,
  appOrigin: getAppOrigin,
  authenticateRequestActor,
  isAdminEmail: isOpenAgentsAdminEmail,
  readSelectedOperatorTargetUser,
  requireAdminApiToken,
  requireBrowserSession,
})

const operatorBillingHandlers = makeOperatorBillingHandlers({
  readSelectedInferenceCreditTargetUser,
  readSelectedOperatorTargetUser,
  requireAdminApiToken,
})

const operatorBuyModeRoutes = makeOperatorBuyModeRoutes<Env>({
  makeEvalBridge: env => buyModeEvalBridgeForEnv(env),
  makePaymentBridge: env => buyModePaymentBridgeForEnv(env),
  makeRelayPublisher: env => buyModeRelayPublisherForEnv(env),
  makeStore: env => makeD1BuyModeDispatcherStore(openAgentsDatabase(env)),
  requireAdminApiToken,
})

const ecommerceCampaignSelfServeRoutes =
  makeEcommerceCampaignSelfServeRoutes<Env>({
    makeStore: env => makePrefilledWorkspaceService(openAgentsDatabase(env)),
    enabled: true, // INERT self-serve enabled
  })

const ecommerceCampaignReceiptRoutes = makeEcommerceCampaignReceiptRoutes<Env>({
  makeStore: env =>
    makeD1EcommerceCampaignReceiptStore(
      openAgentsDatabase(env),
      currentIsoTimestamp,
    ),
  makeClaimStore: () =>
    makeInMemoryEcommerceCampaignPaidDeliveryClaimStore([
      {
        document: firstPaidEcommerceCampaignDeliveryReceiptFixture,
        receiptRef:
          firstPaidEcommerceCampaignDeliveryReceiptFixture.receipt.workItemRef,
        ownerSignOffRef: 'owner.signoff.fixture.1',
      },
    ]),
})

const ecommerceCampaignReceiptOperatorRoutes =
  makeEcommerceCampaignReceiptOperatorRoutes<Env>({
    makeStore: env =>
      makeD1EcommerceCampaignReceiptStore(
        openAgentsDatabase(env),
        currentIsoTimestamp,
      ),
    requireAdminApiToken: requireAdminApiToken,
  })

const publicNip90MarketReceiptRoutes = makePublicNip90MarketReceiptRoutes<Env>({
  makeStore: env => makeD1Nip90MarketReceiptStore(openAgentsDatabase(env)),
})

const publicInferenceReceiptRoutes = makePublicInferenceReceiptRoutes<Env>({
  makeStore: env => makeD1InferenceReceiptStore(openAgentsDatabase(env)),
  nowIso: currentIsoTimestamp,
})

// Dereferenceable PAID receipt read for sellable Cloud primitives (sandbox
// compute #5517 / fine-tuning #5516). Public proof read only — it derefs the
// metered-charge `pay_ins` row the cloud-metering seam already wrote; it grants
// no authority and asserts no promise is green.
const publicCloudPrimitiveReceiptRoutes =
  makePublicCloudPrimitiveReceiptRoutes<Env>({
    makeStore: env => makeD1CloudPrimitiveReceiptStore(openAgentsDatabase(env)),
    nowIso: currentIsoTimestamp,
  })

const marketingAgencyReceiptPublicRoutes =
  makeMarketingAgencyReceiptPublicRoutes<Env>({
    makeClaimStore: _env =>
      makeInMemoryMarketingAgencyPaidDeliveryClaimStore([]),
  })
const marketingAgencySelfServePublicRoutes =
  makeMarketingAgencySelfServePublicRoutes<Env>({
    makeClaimStore: _env => makeInMemoryMarketingAgencySelfServeClaimStore([]),
  })

const publicCardCreditSpendReceiptRoutes =
  makePublicCardCreditSpendReceiptRoutes<Env>({
    makeStore: env =>
      makeD1CardCreditSpendReceiptStore(openAgentsDatabase(env)),
    nowIso: currentIsoTimestamp,
  })

const publicStripeCheckoutReceiptRoutes =
  makePublicStripeCheckoutReceiptRoutes<Env>({
    makeStore: env => makeD1StripeCheckoutReceiptStore(openAgentsDatabase(env)),
    nowIso: currentIsoTimestamp,
  })

const publicSiteReferralPayoutReceiptRoutes =
  makePublicSiteReferralPayoutReceiptRoutes<Env>({
    makeStore: env =>
      makeD1SiteReferralPayoutReceiptStore(openAgentsDatabase(env)),
    nowIso: currentIsoTimestamp,
  })

const publicPartnerPayoutReceiptRoutes =
  makePublicPartnerPayoutReceiptRoutes<Env>({
    makeStore: env => makeD1PartnerPayoutReceiptStore(openAgentsDatabase(env)),
    nowIso: currentIsoTimestamp,
  })

const blueprintRoutes = makeBlueprintRoutes<Env>({
  listActionSubmissions: env =>
    listBlueprintActionSubmissions(openAgentsDatabase(env)),
  listProgramRuns: env => listBlueprintProgramRuns(openAgentsDatabase(env)),
  recordActionSubmissionProposal: (env, input) =>
    recordBlueprintActionSubmissionProposal(openAgentsDatabase(env), input),
  recordProgramRun: (env, input) =>
    recordBlueprintProgramRun(openAgentsDatabase(env), input),
  requireAdminApiToken,
  requireActionSubmissionIntake: async (request, env) =>
    (await requireRunnerCallbackAuth(request, env)) ||
    (await requireAdminApiToken(request, env)),
  requireProgramRunEvidenceIntake: async (request, env) =>
    (await requireRunnerCallbackAuth(request, env)) ||
    (await requireAdminApiToken(request, env)),
})

const blueprintProbeContributionRoutes =
  makeBlueprintProbeContributionRoutes<Env>({
    listContributions: env =>
      listBlueprintProbeContributions(openAgentsDatabase(env)),
    recordContribution: (env, input) =>
      recordBlueprintProbeContribution(openAgentsDatabase(env), input),
    requireAdminApiToken,
    requireContributionIntake: async (request, env) =>
      (await requireRunnerCallbackAuth(request, env)) ||
      (await requireAdminApiToken(request, env)),
  })

const operatorSitesRoutes = makeOperatorSitesRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  notifyCustomerSiteDeployed,
  requireBrowserSession,
})

const operatorOrderTriageRoutes = makeOperatorOrderTriageRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireAdminApiToken,
  requireBrowserSession,
})

const operatorEmailInspectionRoutes = makeOperatorEmailInspectionRoutes({
  appendRefreshedSessionCookies,
  getAppOrigin,
  getResendEmailConfig,
  isOpenAgentsAdminEmail,
  requireAdminApiToken,
  requireBrowserSession,
})

const githubRawContentUrl = ({
  commitSha,
  path,
  repositoryName,
  repositoryOwner,
}: AdjutantTaskPacketRefValidationInput): string => {
  const encodedPath = path.split('/').map(encodeURIComponent).join('/')

  return `https://raw.githubusercontent.com/${encodeURIComponent(repositoryOwner)}/${encodeURIComponent(repositoryName)}/${encodeURIComponent(commitSha)}/${encodedPath}`
}

const githubApiContentUrl = ({
  commitSha,
  path,
  repositoryName,
  repositoryOwner,
}: AdjutantTaskPacketRefValidationInput): string => {
  const encodedPath = path.split('/').map(encodeURIComponent).join('/')
  const url = new URL(
    `https://api.github.com/repos/${encodeURIComponent(repositoryOwner)}/${encodeURIComponent(repositoryName)}/contents/${encodedPath}`,
  )
  url.searchParams.set('ref', commitSha)

  return url.toString()
}

const validateAdjutantTaskPacketRef = async (
  input: AdjutantTaskPacketRefValidationInput,
): Promise<boolean> => {
  const githubAccessToken = input.githubAccessToken?.trim()

  if (githubAccessToken !== undefined && githubAccessToken !== '') {
    const apiResponse = await fetch(githubApiContentUrl(input), {
      headers: {
        accept: 'application/vnd.github+json',
        authorization: `Bearer ${githubAccessToken}`,
        'cache-control': 'no-store',
        'user-agent': 'OpenAgents',
        'x-github-api-version': '2022-11-28',
      },
      method: 'GET',
    })

    if (apiResponse.ok) {
      return true
    }
  }

  const url = githubRawContentUrl(input)
  const response = await fetch(url, {
    headers: {
      'cache-control': 'no-store',
      'user-agent': 'OpenAgents',
    },
    method: 'HEAD',
  })

  if (response.status !== 405) {
    return response.ok
  }

  const fallback = await fetch(url, {
    headers: {
      'cache-control': 'no-store',
      range: 'bytes=0-0',
      'user-agent': 'OpenAgents',
    },
    method: 'GET',
  })

  return fallback.ok
}

const omniHandlers = makeOmniHandlers({
  actorJson,
  appendRefreshedSessionCookies,
  appendTeamAutopilotAnswerBack,
  authenticateRequestActor,
  getAppOrigin,
  getResendEmailConfig,
  getRunnerBackendConfig,
  isOpenAgentsAdminEmail,
  isRouteAccessError,
  makeBillingAwareOmniRunStore,
  postTeamChatMessageForUser,
  readSelectedOperatorTargetUser,
  readTokenUsageLeaderboardsForUser,
  requireAdminApiToken,
  requireBrowserSession,
  requireRunnerCallbackAuth,
  shcDispatchConfig,
  threadRouteAccessBundle,
})

const launchUserAutopilotMission = omniHandlers.launchUserAutopilotMission

const operatorAdjutantRoutes = makeOperatorAdjutantRoutes({
  appendRefreshedSessionCookies,
  buildOperatorAutopilotPreflightPayload:
    omniHandlers.buildOperatorAutopilotPreflightPayload,
  continueUserAutopilotRun: omniHandlers.continueUserAutopilotRun,
  isOpenAgentsAdminEmail,
  launchUserAutopilotMission,
  requireAdminApiToken,
  requireBrowserSession,
  validateAdjutantTaskPacketRef,
})

const operatorArtanisConsoleRoutes = makeOperatorArtanisConsoleRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  requireAdminApiToken,
  requireBrowserSession,
})

// Owner-only Artanis operator chat channel (#6363). Artanis's reasoning is
// powered ONLY by the Khala API — `makeArtanisResponderKhalaClient` dogfoods the
// `openagents/khala` pool and meters the call as Khala usage, so this channel
// never calls a provider directly. `makeKhalaClient` is invoked at request time,
// so referencing the (later-declared) builder here is safe.
const operatorArtanisChatRoutes = makeOperatorArtanisChatRoutes({
  appendRefreshedSessionCookies,
  // Inject the LIVE daily token-pace block into EVERY Artanis turn (epic #6359)
  // so he sees whether today is on track for the target (at least 4x the prior
  // day, goal 10x) without calling a tool. Read-only, fail-soft: an unreachable
  // public stats endpoint degrades the pace block to null, never an error.
  awarenessReaders: env => ({
    readTokenPace: () =>
      loadArtanisNetworkStatsFromLedger(
        makeD1TokenUsageLedger(openAgentsDatabase(env)),
      ).then(
        stats => stats.pace,
        () => null,
      ),
  }),
  isOpenAgentsAdminEmail,
  makeKhalaClient: env => makeArtanisResponderKhalaClient(env),
  // #6366 follow-up: wire the gated Codex dispatch tool to LIVE execution. The
  // owner SESSION is threaded in so the execution seam is owner-scoped:
  // own-capacity only (the owner's own linked Pylons via `delegateCodingWorkflow`),
  // no-spend (`unpaid_smoke`), and gated behind an effective `pylon_job_dispatch`
  // owner approval. With no effective approval (the default today), the tool
  // returns the plan and defers — it never fires.
  makeOperatorTools: (env, session) => {
    // Owner-scope resolver shared by the gated dispatch seam and the
    // owner-scoped Pylon job-status reader: resolve the owner's linked agent
    // user ids (their Pylon-owning credentials) so both stay strictly
    // own-capacity / owner-scoped.
    const listLinkedAgentUserIds = async (ownerOpenAuthUserId: string) => {
      const agentStore = makeD1AgentRegistrationStore(openAgentsDatabase(env))
      if (agentStore.listLinkedAgentsForOpenAuthUser === undefined) {
        return []
      }
      const linked = await agentStore.listLinkedAgentsForOpenAuthUser(
        ownerOpenAuthUserId,
        100,
      )
      const ids = linked.map(agent => agent.agentUserId)
      // Owner-promoted operator agent (Artanis, owner-directed 2026-06-27) owns
      // his own Pylon AS HIMSELF (registrations keyed by his own agent user id),
      // and his credential carries no OpenAuth link, so the link query above
      // returns nothing for him. Include his own agent user id so his
      // own-capacity Codex dispatch can resolve his own linked Pylon. This stays
      // strictly own-capacity: only the owner-promoted agent's OWN id is added,
      // never another account's.
      if (
        isOpenAgentsOwnerAgentOpenAuthUserId(ownerOpenAuthUserId) &&
        !ids.includes(ownerOpenAuthUserId)
      ) {
        ids.push(ownerOpenAuthUserId)
      }
      return ids
    }
    return makeArtanisOperatorTools({
      defaultBranch: 'main',
      // get_network_stats reads the token-usage ledger directly (the worker
      // cannot reliably HTTP-fetch its own public /stats zone).
      networkStats: {
        loadStats: () =>
          loadArtanisNetworkStatsFromLedger(
            makeD1TokenUsageLedger(openAgentsDatabase(env)),
          ),
      },
      // iteration-7: the live GLM inference-fleet readiness READ tool. Reads the
      // SAME public-safe fleet readiness projection the
      // GET /v1/gateway/glm-fleet/readiness route serves, IN-WORKER (the Worker
      // cannot reliably HTTP-fetch its own public zone), so Artanis can gate
      // synthetic-load and Codex-dispatch decisions on healthy capacity.
      // Read-only, side-effect-free, public-safe (aggregate counts + status only).
      glmFleetStatus: {
        loadFleetStatus: makeArtanisGlmFleetStatusLoader({
          db: openAgentsDatabase(env),
          env,
        }),
      },
      // iteration-12: the ACTIVE synthetic-load run READ tool
      // (get_synthetic_load_status) — the read half of the plan-only
      // `trigger_synthetic_load` pair, so Artanis can see runs already in flight
      // before planning a new burn. Synthetic-load runs are plan-only /
      // owner-gated today and there is no live run registry yet, so we wire NO
      // reader: the tool reports an honest "(no active synthetic-load runs)"
      // rather than inventing one. When a real own-capacity synthetic-load run
      // registry lands, wire its owner-scoped reader here.
      // iteration-3: the owner-scoped Pylon job-status read tool. Reads the
      // public-safe closeout/proof status of ONE of the owner's own linked-Pylon
      // assignments. Read-only, owner-scoped, no spend/authority.
      pylonJobStatus: {
        reader: makeArtanisPylonJobStatusReader({
          listLinkedAgentUserIds,
          nowIso: currentIsoTimestamp,
          ownerOpenAuthUserId: session.user.userId,
          pylonStore: makeD1PylonApiStore(openAgentsDatabase(env)),
        }),
      },
      // iteration-5: the owner-scoped bulk Pylon assignments LIST tool. Reads the
      // public-safe summaries of ALL of the owner's own linked-Pylon assignments
      // in one call so Artanis can scan the burndown, spot failed/stalled runs,
      // and queue parallel retries. Read-only, owner-scoped, no spend/authority.
      pylonAssignments: {
        lister: makeArtanisPylonAssignmentsLister({
          listLinkedAgentUserIds,
          nowIso: currentIsoTimestamp,
          ownerOpenAuthUserId: session.user.userId,
          pylonStore: makeD1PylonApiStore(openAgentsDatabase(env)),
        }),
      },
      // iteration-6: the owner-scoped Khala CLI feedback READ tool. Reads the most
      // recent user feedback submitted through the Khala CLI /feedback command
      // (the same admin-gated `khala_feedback` store the
      // GET /api/operator/khala/feedback route uses) so Artanis can hear directly
      // from users, spot gaps/bugs/style preferences, and triage them.
      // Read-only, side-effect-free, no spend/authority.
      khalaFeedback: {
        reader: makeArtanisKhalaFeedbackReader({
          store: makeD1KhalaFeedbackStore(openAgentsDatabase(env)),
        }),
      },
      // iteration-11: the owner-scoped Khala trace-review READ tool. Reads the
      // SAME public-safe report the GET /api/operator/khala/trace-review route
      // serves (#6356), IN-WORKER (the Worker cannot reliably HTTP-fetch its own
      // admin-gated zone), so Artanis can spot recurring failure modes and unmet
      // user intents in-loop, triage them into the unsupported-request ledger,
      // and plan targeted Codex burndown at the gaps that block adoption.
      // Read-only, side-effect-free, public-safe (aggregate counts + bounded
      // buckets only; no raw trajectories, prompts, or private refs).
      traceReview: {
        loadReport: makeArtanisTraceReviewLoader({
          nowIso: currentIsoTimestamp,
          store: makeD1KhalaTraceReviewStore(openAgentsDatabase(env)),
        }),
      },
      // iteration-8: the owner-scoped unsupported-request ledger READ tool. Reads
      // the live `khala_unsupported_requests` ledger of user-facing capability
      // gaps that block Khala adoption (#6357) — the same admin-gated store the
      // GET /api/operator/khala/unsupported-requests route uses — so Artanis can
      // see exactly which gaps suppress usage, match them to open issues, and
      // target Codex dispatch / forum mobilization at the highest-leverage gaps.
      // Read-only, side-effect-free, no spend/authority.
      unsupportedRequests: {
        reader: makeArtanisUnsupportedRequestsReader({
          store: makeD1KhalaUnsupportedRequestStore(openAgentsDatabase(env)),
        }),
      },
      // iteration-9: the owner-scoped unsupported-request ledger WRITE/TRIAGE
      // tool. Lets Artanis move a capability-gap entry through its lifecycle
      // (e.g. needs_issue -> issue_opened -> closed), set its triage kind, and
      // link the GitHub issue dispatched to fix it — in the SAME turn he reads
      // the gap (#6357). Owner-scoped, internal-ledger-only: no spend, payout,
      // deploy, delete, or outward action.
      unsupportedRequestWriter: makeArtanisUnsupportedRequestWriter({
        nowIso: currentIsoTimestamp,
        store: makeD1KhalaUnsupportedRequestStore(openAgentsDatabase(env)),
      }),
      dispatchExecution: makeArtanisDispatchExecution({
        listLinkedAgentUserIds,
        makeId: () => compactRandomId('artanis_dispatch'),
        nowIso: currentIsoTimestamp,
        ownerOpenAuthUserId: session.user.userId,
        pylonStore: makeD1PylonApiStore(openAgentsDatabase(env)),
        readEffectivePylonDispatchApproval: () =>
          // Owner-promotion-aware (owner-directed 2026-06-27): owner-Artanis
          // carries a STANDING owner approval for his own pylon_job_dispatch, so
          // his own-capacity no-spend Codex dispatch EXECUTES without a
          // separately-armed gate. Any other owner still needs an effective
          // armed approval gate. Money-movement kinds stay gated regardless.
          readEffectiveArtanisPylonDispatchApprovalForOwner(
            openAgentsDatabase(env),
            currentIsoTimestamp(),
            session.user.userId,
          ),
        // Resolve the current branch tip so a pinned-workspace dispatch uses a
        // real commit; on any failure the dispatch falls back to the bounded
        // public fixture. Public repo only, public-safe.
        resolveCommitSha: async branch => {
          try {
            const response = await fetch(
              `https://api.github.com/repos/OpenAgentsInc/openagents/commits/${encodeURIComponent(branch)}`,
              {
                headers: {
                  Accept: 'application/vnd.github+json',
                  'User-Agent': 'artanis-operator',
                },
              },
            )
            if (!response.ok) {
              return undefined
            }
            const body = (await response.json()) as { sha?: unknown }
            return typeof body.sha === 'string' ? body.sha : undefined
          } catch {
            return undefined
          }
        },
      }),
    })
  },
  requireAdminApiToken,
  requireBrowserSession,
  // Accept an `oa_agent_` bearer (the Khala CLI's token from `khala login`) when
  // its linked OpenAuth account email is an OpenAgents admin. Resolves the agent
  // credential -> its linked owner user id -> that user's email -> admin check.
  resolveOwnerAgentBearer: async (request, env) => {
    const bearer = readBearerToken(request)
    if (bearer === undefined) {
      return undefined
    }
    const agent = await authenticateProgrammaticAgent(
      makeD1AgentRegistrationStore(openAgentsDatabase(env)),
      bearer,
    )
    // The owner-promoted operator agent (Artanis, owner-directed 2026-06-27) is
    // admitted by his OWN agent identity (his agent user id forms his actorRef
    // `agent:<userId>`), independent of any linked OpenAuth account — his
    // credential carries no OpenAuth link. Every other owner uses the original
    // Khala-CLI owner path: a linked OpenAuth account whose email is an
    // OpenAgents admin.
    const agentOwnUserId = agent?.user.id
    if (agentOwnUserId === undefined) {
      return undefined
    }
    const linkedOpenAuthUserId = agent?.credential.openauthUserId ?? null

    const isOwnerAgent =
      isOpenAgentsOwnerAgentOpenAuthUserId(agentOwnUserId) ||
      isOpenAgentsOwnerAgentOpenAuthUserId(linkedOpenAuthUserId)

    // Original admin-email path: only consulted when there is a linked OpenAuth
    // account to resolve an email for.
    let adminEmail: string | undefined
    if (linkedOpenAuthUserId !== null) {
      const row = await openAgentsDatabase(env)
        .prepare(`SELECT primary_email FROM users WHERE id = ?`)
        .bind(linkedOpenAuthUserId)
        .first<Readonly<{ primary_email: string | null }>>()
      const email = row?.primary_email?.trim().toLowerCase()
      if (email !== undefined && email !== '' && isOpenAgentsAdminEmail(email)) {
        adminEmail = email
      }
    }

    if (!isOwnerAgent && adminEmail === undefined) {
      return undefined
    }

    // Owner-scope user id: for the owner-promoted agent key on his OWN
    // owner-promoted user id (so the standing pylon_job_dispatch approval and his
    // own-capacity resolution key consistently on his agent user id); for the
    // admin-email path preserve the original linked-OpenAuth owner id.
    const ownerScopeUserId = isOwnerAgent
      ? agentOwnUserId
      : (linkedOpenAuthUserId ?? agentOwnUserId)

    // The Artanis route only reads user.email + user.userId, but the inferred
    // Session is the full human-session shape; fill the unused fields so the
    // owner-agent-bearer return type matches the browser-session return type.
    const sessionEmail =
      adminEmail ??
      agent?.user.primaryEmail?.trim().toLowerCase() ??
      'artanis@agents.openagents.com'
    return {
      user: {
        avatarUrl: '',
        email: sessionEmail,
        name: sessionEmail,
        provider: 'github' as const,
        userId: ownerScopeUserId,
      },
    }
  },
})

const operatorPylonMarketplaceRoutes = makeOperatorPylonMarketplaceRoutes({
  appendRefreshedSessionCookies,
  isOpenAgentsAdminEmail,
  makeStore: env => makeD1PylonMarketplaceJobStore(openAgentsDatabase(env)),
  requireAdminApiToken,
  requireBrowserSession,
})

const nexusPylonVisibilityRoutes = makeNexusPylonVisibilityRoutes({
  appendRefreshedSessionCookies,
  currentIsoTimestamp,
  isOpenAgentsAdminEmail,
  makeArtanisAdminCloseoutReceiptStore: env =>
    makeD1ArtanisAdminCloseoutReceiptStore(openAgentsDatabase(env)),
  makeLedgerStore: env =>
    makeD1NexusTreasuryPayoutLedgerStore(openAgentsDatabase(env)),
  makePaymentAuthority: (env, context) => {
    const config = getOpenAgentsWorkerConfig(env)
    const ledgerStore = makeD1NexusTreasuryPayoutLedgerStore(
      openAgentsDatabase(env),
    )

    return makeTreasuryPaymentAuthority({
      adapters:
        context.adapterKind === 'hosted_mdk'
          ? [
              makeHostedMdkPayoutAdapter({
                accessToken: redactedValue(config.mdk.accessToken),
                providerRef: context.providerRef,
                resolveDestination: () =>
                  Effect.succeed(context.privatePayoutDestination ?? ''),
              }),
            ]
          : context.adapterKind === 'spark_treasury'
            ? [
                makeSparkTreasuryPayoutAdapter({
                  fetchTreasury: fetchMdkTreasuryPath(env),
                  providerRef: context.providerRef,
                  resolveDestination: () =>
                    Effect.succeed(context.privatePayoutDestination ?? ''),
                }),
              ]
            : [],
      ledgerStore,
    })
  },
  makePylonApiStore: env => makeD1PylonApiStore(openAgentsDatabase(env)),
  makeTipRecipientReadinessReader: env => ({
    readForActor: actorRef =>
      readForumTipRecipientReadinessForActor(openAgentsDatabase(env), actorRef),
  }),
  requireAdminApiToken,
  requireBrowserSession,
})

const pylonApiRoutes = makePylonApiRoutes<WorkerBindings>({
  agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
  makeStore: env => makeD1PylonApiStore(openAgentsDatabase(env)),
  // #5252: private operator-only store for raw Spark payout targets.
  makeSparkPayoutTargetStore: env =>
    makeD1PylonSparkPayoutTargetStore(openAgentsDatabase(env)),
  recordAutopilotWorkerCloseout: async (env, input) => {
    const delivered = await recordAutopilotWorkerCloseoutFromPylon(
      makeD1AutopilotWorkStore(openAgentsDatabase(env)),
      input,
    )

    if (delivered?.state === 'delivered') {
      try {
        await sendAutopilotDecisionRequiredEmailOnce(env, delivered)
      } catch (error) {
        logWorkerRouteWarning('autopilot_decision_email_failed', {
          error: errorMessage(error),
          workOrderRef: delivered.workOrderRef,
        })
      }
    }

    return delivered
  },
  requireAdminApiToken,
  requireBrowserSession,
})

const trainingRunWindowRoutes = makeTrainingRunWindowRoutes<WorkerBindings>({
  createVerificationChallenge: (env, request) => {
    const built = buildTrainingVerificationChallengeRecord({
      makeId: randomUuid,
      nowIso: currentIsoTimestamp(),
      request,
    })

    return makeD1TrainingVerificationStore(
      openAgentsDatabase(env),
    ).createChallenge(built.challenge, built.event)
  },
  makePayoutLedgerStore: env =>
    makeD1NexusTreasuryPayoutLedgerStore(openAgentsDatabase(env)),
  // REAL Bitcoin settlement wiring (openagents #5232, Gate 2). INERT by default:
  // these are only consulted on the real branch, which is unreachable unless the
  // owner sets OPENAGENTS_REAL_SETTLEMENT_GATE (enabled + allowlisted + capped).
  // The rail is the proven Spark treasury payout adapter driven through the
  // treasury payment authority (idempotency-keyed dispatch, dedupe, redaction,
  // pause/cap/wallet-readiness gates).
  makeSettlementPaymentAuthority: (env, context) =>
    makeTreasuryPaymentAuthority({
      adapters: [
        makeSparkTreasuryPayoutAdapter({
          fetchTreasury: fetchMdkTreasuryPath(env),
          providerRef: context.providerRef,
          resolveDestination: () =>
            Effect.succeed(context.privatePayoutDestination),
        }),
      ],
      ledgerStore: context.ledgerStore,
    }),
  // Wallet readiness for the gated payout: the treasury Spark rail is ready only
  // when its container is reachable and reports a spendable balance. Any failure
  // fails closed to 'absent' (no payout).
  readSettlementWalletReadiness: async env => {
    const fetchTreasury = fetchMdkTreasuryPath(env)

    if (fetchTreasury === undefined) {
      return 'absent'
    }

    try {
      const response = await fetchTreasury('/spark/balance')

      return response.ok ? 'ready' : 'absent'
    } catch {
      return 'absent'
    }
  },
  // #5252: resolve the (private, never-projected) payout destination for the
  // gated recipient (the contributor at lease.pylonRef) from the recipient's
  // OWN registered raw Spark address. The raw `spark1…` lives only in the
  // private operator store keyed to its pylonRef; we return it here as the
  // native Spark send destination so #5232's real settlement (and #5225 native
  // send) pay it natively over Spark. The destination never enters any receipt
  // projection — only the adapter's redacted refs do.
  //
  // Fails closed: when the recipient has no registered Spark target (or the
  // store read fails), this returns undefined and the real settlement branch
  // does not send. The owner may later add a BOLT12/Lightning-Address fallback
  // resolver here; until then, no vetted Spark target == no native send.
  resolveSettlementPayoutDestination: (env, contributorRef) =>
    resolveSparkPayoutDestination(
      makeD1PylonSparkPayoutTargetStore(openAgentsDatabase(env)),
      contributorRef,
      pylonRef =>
        makeD1PylonApiStore(openAgentsDatabase(env))
          .readRegistration(pylonRef)
          .then(registration => registration?.ownerAgentUserId),
    ),
  makeStore: env => makeD1TrainingAuthorityStore(openAgentsDatabase(env)),
  requireAdminApiToken,
})

// Honest hygiene-lane settlement DISPATCH route (openagents #5372, EPIC #5335).
// Settles ONE merged, benchmark-verified hygiene debt receipt to the
// contributor's registered Spark target through the SAME proven #5232 Spark
// treasury rail and the SAME owner gate as the Tassadar run settlement, but with
// an HONEST `hygiene_merged_reviewed` verification basis (merged PR + reviewer
// acceptance + debt receipt) instead of a fabricated exact_trace_replay verdict.
//
// INERT by default: with the owner gate OFF (the default everywhere) every
// settle resolves to the simulation chain. The real branch is unreachable until
// the owner arms OPENAGENTS_REAL_SETTLEMENT_GATE with the hygiene run-ref.
//
// Create-side (#5335 process step 1): POST /api/hygiene-lane/debt-receipts is
// admin-only and persists a PAYABLE funded debt receipt in the durable D1 store
// (one row per DebtReceiptKey, #5340). `resolveDebtReceiptProjection` reads that
// store as the single source of truth for payability — an operator cannot assert
// payability through the settle request body. It is fail-closed: no row, or a
// retired row, yields a non-payable projection so the route reports
// `debt_receipt_not_found` / `duplicate_replay` and never pays. Once real bitcoin
// moves, the settle route marks the key retired, so a second settle on the same
// key is `duplicate_replay`.
const hygieneLaneSettlementRoutes =
  makeHygieneLaneSettlementRoutes<WorkerBindings>({
    makePayoutLedgerStore: env =>
      makeD1NexusTreasuryPayoutLedgerStore(openAgentsDatabase(env)),
    // REAL Bitcoin settlement wiring (openagents #5232): the SAME proven Spark
    // treasury rail the Tassadar run settlement uses. INERT unless the gate is
    // armed.
    makeSettlementPaymentAuthority: (env, context) =>
      makeTreasuryPaymentAuthority({
        adapters: [
          makeSparkTreasuryPayoutAdapter({
            fetchTreasury: fetchMdkTreasuryPath(env),
            providerRef: context.providerRef,
            resolveDestination: () =>
              Effect.succeed(context.privatePayoutDestination),
          }),
        ],
        ledgerStore: context.ledgerStore,
      }),
    readSettlementWalletReadiness: async env => {
      const fetchTreasury = fetchMdkTreasuryPath(env)

      if (fetchTreasury === undefined) {
        return 'absent'
      }

      try {
        const response = await fetchTreasury('/spark/balance')

        return response.ok ? 'ready' : 'absent'
      } catch {
        return 'absent'
      }
    },
    resolveSettlementPayoutDestination: (env, contributorRef) =>
      resolveSparkPayoutDestination(
        makeD1PylonSparkPayoutTargetStore(openAgentsDatabase(env)),
        contributorRef,
        pylonRef =>
          makeD1PylonApiStore(openAgentsDatabase(env))
            .readRegistration(pylonRef)
            .then(registration => registration?.ownerAgentUserId),
      ),
    // Durable, payable debt-receipt store (#5335 process step 1, #5372). The
    // admin create endpoint (POST /api/hygiene-lane/debt-receipts) persists a
    // payable funded receipt here; the settle route reads payability from it
    // and marks it retired once real bitcoin moves, so a second settle on the
    // same DebtReceiptKey reprojects to duplicate_replay.
    makeDebtReceiptStore: env =>
      makeD1HygieneDebtReceiptStore(openAgentsDatabase(env)),
    // The settle route's source of truth for payability: the durable store.
    // Fail-closed — no row (or a retired row) yields a non-payable projection,
    // so the operator cannot assert payability through the request body.
    resolveDebtReceiptProjection: (env, debtReceiptKeyRef) =>
      makeD1HygieneDebtReceiptStore(openAgentsDatabase(env)).resolveProjection(
        debtReceiptKeyRef,
      ),
    requireAdminApiToken,
  })

// Firm-up escrow -> real Bitcoin settlement DISPATCH route (openagents #5459,
// EPIC #5457). Settles ONE firmed-up, EXECUTED-verified labor job to the
// worker's registered Spark target through the SAME proven #5232 Spark treasury
// rail and the SAME owner gate as the Tassadar + hygiene lanes, but against an
// EXECUTED verification verdict (not a manual attestation).
//
// INERT by default: with the owner gate OFF (the default everywhere) every
// settle resolves to the simulation chain. The real branch is unreachable until
// the owner deliberately arms OPENAGENTS_REAL_SETTLEMENT_GATE with a firm-up
// run-ref (run.firmup.lane.YYYYMMDD). No firm-up run-ref is armed by this code;
// the first real firm-up payout is a separate, deliberate prod event.
//
// `resolveSettleableEscrow` is the SOURCE OF TRUTH for settleability: it reads
// the escrow + acceptance + work request server-side. Fail-closed — the escrow
// must be a `reserved` firm-up escrow with an accepted offer (the provider) and
// a declared verification command. The operator cannot assert settleability
// through the request body.
const firmupBitcoinSettlementRoutes =
  makeFirmupBitcoinSettlementRoutes<WorkerBindings>({
    makePayoutLedgerStore: env =>
      makeD1NexusTreasuryPayoutLedgerStore(openAgentsDatabase(env)),
    resolveSettleableEscrow: (env, escrowRef) =>
      readFirmupSettleableEscrow(openAgentsDatabase(env), escrowRef),
    // REAL Bitcoin settlement wiring (openagents #5232): the SAME proven Spark
    // treasury rail. INERT unless the gate is armed.
    makeSettlementPaymentAuthority: (env, context) =>
      makeTreasuryPaymentAuthority({
        adapters: [
          makeSparkTreasuryPayoutAdapter({
            fetchTreasury: fetchMdkTreasuryPath(env),
            providerRef: context.providerRef,
            resolveDestination: () =>
              Effect.succeed(context.privatePayoutDestination),
          }),
        ],
        ledgerStore: context.ledgerStore,
      }),
    readSettlementWalletReadiness: async env => {
      const fetchTreasury = fetchMdkTreasuryPath(env)

      if (fetchTreasury === undefined) {
        return 'absent'
      }

      try {
        const response = await fetchTreasury('/spark/balance')

        return response.ok ? 'ready' : 'absent'
      } catch {
        return 'absent'
      }
    },
    resolveSettlementPayoutDestination: (env, contributorRef) =>
      resolveSparkPayoutDestination(
        makeD1PylonSparkPayoutTargetStore(openAgentsDatabase(env)),
        contributorRef,
        pylonRef =>
          makeD1PylonApiStore(openAgentsDatabase(env))
            .readRegistration(pylonRef)
            .then(registration => registration?.ownerAgentUserId),
      ),
    requireAdminApiToken,
  })

// #5052 (epic #5051): agent-gated worker -> validator executor-trace completion
// routes. These add the contributor-callable submit/verify path; they are inert
// with respect to existing admin/closeout/settlement behavior until the pairing
// orchestration (#5053) and Pylon client (#5054) wire them.
const tassadarTraceContributionRoutes =
  makeTassadarTraceContributionRoutes<WorkerBindings>({
    agentStore: env => makeD1AgentRegistrationStore(openAgentsDatabase(env)),
    createVerificationChallenge: async (env, input) => {
      const store = makeD1TrainingVerificationStore(openAgentsDatabase(env))
      const built = buildTrainingVerificationChallengeRecord({
        makeId: randomUuid,
        nowIso: currentIsoTimestamp(),
        request: input.request,
      })
      const created = await store.createChallenge(built.challenge, built.event)
      const leased = leaseTrainingVerificationChallengeRecord({
        challenge: created,
        eventId: randomUuid(),
        nowIso: currentIsoTimestamp(),
        request: {
          leaseSeconds: 60,
          validatorRef: input.validatorDeviceRef,
        },
      })
      const storedLeased = await store.leaseChallenge(
        leased.challenge,
        leased.event,
      )
      const verdict = await runTrainingVerificationClass({
        challenge: storedLeased,
      })
      const finalized = finalizeTrainingVerificationChallengeRecord({
        challenge: storedLeased,
        eventId: randomUuid(),
        nowIso: currentIsoTimestamp(),
        request: { receiptRefs: [] },
        validatorRef: input.validatorDeviceRef,
        verdict,
      })

      return store.transitionChallenge(finalized.challenge, finalized.event)
    },
    makeContributionStore: env =>
      makeD1TrainingTraceContributionStore(openAgentsDatabase(env)),
    makeStore: env => makeD1TrainingAuthorityStore(openAgentsDatabase(env)),
    // Hands-off auto-stream of the real per-window reward on each Verified
    // exact_trace_replay pair (openagents #5309 + #5310): worker 5 sats AND
    // validator 5 sats, NO operator POST. INERT until the owner arms
    // OPENAGENTS_REAL_SETTLEMENT_GATE — every leg resolves to skip while the
    // gate is OFF (the default everywhere). FAIL-SOFT: the verdict route wraps
    // this in catchAll so a blocked/failed settlement never breaks the verdict.
    onVerifiedExactTraceReplayPair: (env, input) =>
      Effect.gen(function* () {
        const db = openAgentsDatabase(env)
        const ledger = makeD1NexusTreasuryPayoutLedgerStore(db)
        const sparkTargetStore = makeD1PylonSparkPayoutTargetStore(db)
        const contributionStore = makeD1TrainingTraceContributionStore(db)
        const run = yield* Effect.promise(() =>
          makeD1TrainingAuthorityStore(db).readRun(input.lease.trainingRunRef),
        )

        if (run === undefined) {
          return
        }

        // Owner resolver for the Spark payout destination (#5306/#5310). The
        // WORKER leg's contributorRef is the verified registered `pylonRef`, so
        // the direct registration lookup resolves its owner. The VALIDATOR leg's
        // contributorRef is the validator's device-ref (its nodeId) — NOT a
        // `pylonRef` — so the direct lookup misses; we then map that device-ref
        // to the most recent `pylon_ref` it acted as a worker under and resolve
        // THAT pylon's owner. This binds the validator strictly to its own
        // owning agent (its own historical worker pylon), never crosses agent
        // ownership, and arms no new authority — it only lets the owner-scoped
        // `readByOwner` fallback in resolveSparkPayoutDestination find the
        // validator's OWN registered Spark target so the autostream pays it with
        // NO operator step.
        const resolveContributorOwnerAgentUserId = async (
          contributorRef: string,
        ): Promise<string | undefined> => {
          const pylonApiStore = makeD1PylonApiStore(db)
          const direct = await pylonApiStore
            .readRegistration(contributorRef)
            .then(registration => registration?.ownerAgentUserId)

          if (direct !== undefined && direct.trim() !== '') {
            return direct
          }

          const pylonRefForDevice =
            await contributionStore.readMostRecentPylonRefByDeviceRef(
              contributorRef,
            )

          if (pylonRefForDevice === undefined) {
            return undefined
          }

          return pylonApiStore
            .readRegistration(pylonRefForDevice)
            .then(registration => registration?.ownerAgentUserId)
        }

        const settlementOutcome = yield* autoSettleVerifiedPair<WorkerBindings>(
          {
            dispatchRealSettlement: dispatchInput =>
              dispatchRealRunSettlementCore<WorkerBindings>(
                {
                  env,
                  makeSettlementPaymentAuthority: (authorityEnv, context) =>
                    makeTreasuryPaymentAuthority({
                      adapters: [
                        makeSparkTreasuryPayoutAdapter({
                          fetchTreasury: fetchMdkTreasuryPath(authorityEnv),
                          providerRef: context.providerRef,
                          resolveDestination: () =>
                            Effect.succeed(context.privatePayoutDestination),
                        }),
                      ],
                      ledgerStore: context.ledgerStore,
                    }),
                  readSettlementWalletReadiness: async authorityEnv => {
                    const fetchTreasury = fetchMdkTreasuryPath(authorityEnv)

                    if (fetchTreasury === undefined) {
                      return 'absent'
                    }

                    try {
                      const response = await fetchTreasury('/spark/balance')

                      return response.ok ? 'ready' : 'absent'
                    } catch {
                      return 'absent'
                    }
                  },
                  resolveSettlementPayoutDestination: (_authorityEnv, ref) =>
                    resolveSparkPayoutDestination(
                      sparkTargetStore,
                      ref,
                      resolveContributorOwnerAgentUserId,
                    ),
                },
                {
                  contributorRef: dispatchInput.contributorRef,
                  ledger,
                  settlement: dispatchInput.settlement,
                },
              ),
            ledger,
            nowIso: currentIsoTimestamp(),
            readGate: () => readTassadarRealSettlementGate(env),
            resolvePayoutDestination: ref =>
              resolveSparkPayoutDestination(
                sparkTargetStore,
                ref,
                resolveContributorOwnerAgentUserId,
              ),
            run,
          },
          {
            challenge: input.challenge,
            lease: input.lease,
            validatorContributorRef: input.validatorContributorRef,
          },
        )

        // ADDITIVE + FAIL-SOFT live settled feed (openagents #5311): broadcast
        // ONE public-safe event per actually-settled leg onto the public sync
        // room so the homepage updates in real-time as sats stream. This never
        // touches the settlement dispatch above; any failure is swallowed so a
        // broadcast problem can never break or slow settlement.
        const settledLegs = settlementOutcome.legs.filter(leg => leg.settled)

        if (settledLegs.length > 0) {
          const settledAt = currentIsoTimestamp()
          const workerContributorRef = input.lease.pylonRef.trim()
          const contributorRefForParty = (party: 'validator' | 'worker') =>
            party === 'worker'
              ? workerContributorRef
              : input.validatorContributorRef.trim()
          const dayReceipts = yield* Effect.tryPromise({
            catch: () => [],
            try: () => ledger.listPaymentAuthorityReceipts(5000),
          }).pipe(Effect.orElseSucceed(() => []))
          const priorSettledSats =
            tassadarRealSettledSatsForDay(
              dayReceipts,
              tassadarRealSettlementUtcDayKey(settledAt),
            ) - settledLegs.reduce((sum, leg) => sum + leg.amountSats, 0)
          const events = buildSettledFeedEvents({
            legs: settledLegs.map(leg => ({
              amountSats: leg.amountSats,
              challengeRef: input.challenge.challengeRef,
              contributorRef: contributorRefForParty(leg.party),
              party: leg.party,
              runRef: run.trainingRunRef,
              windowRef: input.lease.windowRef,
            })),
            priorCount: 0,
            priorSettledSats: Math.max(0, priorSettledSats),
            settledAt,
          })

          yield* Effect.promise(() =>
            publishSettledFeedEvents(env, events).catch(() => undefined),
          )
        }
      }),
    resolvePylonOwnerUserId: async (env, pylonRef) => {
      const registration = await makeD1PylonApiStore(
        openAgentsDatabase(env),
      ).readRegistration(pylonRef)

      return registration?.ownerAgentUserId
    },
  })

const trainingVerificationRoutes =
  makeTrainingVerificationRoutes<WorkerBindings>({
    makeStore: env => makeD1TrainingVerificationStore(openAgentsDatabase(env)),
    requireAdminApiToken,
  })

const omniRoutes = makeOmniRoutes({
  handleAutopilotFleetApi: (request, env, ctx) =>
    routeEffect('handle_autopilot_fleet_api', () =>
      omniHandlers.handleAutopilotFleetApi(request, env, ctx),
    ),
  handleAutopilotTokenLeaderboardsApi: (request, env, ctx) =>
    routeEffect('handle_autopilot_token_leaderboards_api', () =>
      omniHandlers.handleAutopilotTokenLeaderboardsApi(request, env, ctx),
    ),
  handleBillingCheckoutApi: (request, env, ctx) =>
    routeEffect('handle_billing_checkout_api', () =>
      billingApiHandlers.handleBillingCheckoutApi(request, env, ctx),
    ),
  handleBillingInferenceCreditApi: (request, env, ctx) =>
    routeEffect('handle_billing_inference_credit_api', () =>
      billingApiHandlers.handleBillingInferenceCreditApi(request, env, ctx),
    ),
  handleBillingAutoTopUpPolicyApi: (request, env, ctx) =>
    routeEffect('handle_billing_auto_top_up_policy_api', () =>
      billingApiHandlers.handleBillingAutoTopUpPolicyApi(request, env, ctx),
    ),
  handleBillingAutoTopUpRunApi: (request, env, ctx) =>
    routeEffect('handle_billing_auto_top_up_run_api', () =>
      billingApiHandlers.handleBillingAutoTopUpRunApi(request, env, ctx),
    ),
  handleBillingCouponRedeemApi: (request, env, ctx) =>
    routeEffect('handle_billing_coupon_redeem_api', () =>
      billingApiHandlers.handleBillingCouponRedeemApi(request, env, ctx),
    ),
  handleBillingSummaryApi: (request, env, ctx) =>
    routeEffect('handle_billing_summary_api', () =>
      billingApiHandlers.handleBillingSummaryApi(request, env, ctx),
    ),
  handleBillingStripeCheckoutReturnApi: (request, environment) =>
    routeEffect('handle_billing_stripe_checkout_return_api', () =>
      billingApiHandlers.handleBillingStripeCheckoutReturnApi(
        request,
        environment,
      ),
    ),
  handleBillingStripeSetupIntentApi: (request, env, ctx) =>
    routeEffect('handle_billing_stripe_setup_intent_api', () =>
      billingApiHandlers.handleBillingStripeSetupIntentApi(request, env, ctx),
    ),
  handleBillingStripeSetupIntentSaveApi: (request, env, ctx) =>
    routeEffect('handle_billing_stripe_setup_intent_save_api', () =>
      billingApiHandlers.handleBillingStripeSetupIntentSaveApi(
        request,
        env,
        ctx,
      ),
    ),
  handleBillingStripeWebhookApi: (request, environment) =>
    routeEffect('handle_billing_stripe_webhook_api', () =>
      billingApiHandlers.handleBillingStripeWebhookApi(request, environment),
    ),
  handleEmailResendWebhookApi: (request, environment) =>
    routeEffect('handle_email_resend_webhook_api', () =>
      handleEmailResendWebhookApi(request, environment),
    ),
  handleOmniAgentRunDetailApi: (request, env, ctx, runId) =>
    routeEffect('handle_omni_agent_run_detail_api', () =>
      omniHandlers.handleOmniAgentRunDetailApi(request, env, ctx, runId),
    ),
  handleOmniAgentRunEventsApi: (request, env, ctx, runId) =>
    routeEffect('handle_omni_agent_run_events_api', () =>
      omniHandlers.handleOmniAgentRunEventsApi(request, env, ctx, runId),
    ),
  handleOmniAgentRunsApi: (request, env, ctx) =>
    routeEffect('handle_omni_agent_runs_api', () =>
      omniHandlers.handleOmniAgentRunsApi(request, env, ctx),
    ),
  handleOmniDeploymentDetailApi: (request, env, ctx, deployId) =>
    routeEffect('handle_omni_deployment_detail_api', () =>
      omniHandlers.handleOmniDeploymentDetailApi(request, env, ctx, deployId),
    ),
  handleOmniDeploymentEventsApi: (request, env, ctx, deployId) =>
    routeEffect('handle_omni_deployment_events_api', () =>
      omniHandlers.handleOmniDeploymentEventsApi(request, env, ctx, deployId),
    ),
  handleOmniDeploymentsApi: (request, env, ctx) =>
    routeEffect('handle_omni_deployments_api', () =>
      omniHandlers.handleOmniDeploymentsApi(request, env, ctx),
    ),
  handleOmniOperatorAgentRunDetailApi: (request, env, runId) =>
    routeEffect('handle_omni_operator_agent_run_detail_api', () =>
      omniHandlers.handleOmniOperatorAgentRunDetailApi(request, env, runId),
    ),
  handleOmniOperatorAgentRunsApi: (request, env) =>
    routeEffect('handle_omni_operator_agent_runs_api', () =>
      omniHandlers.handleOmniOperatorAgentRunsApi(request, env),
    ),
  handleOmniOperatorBillingCreditsApi: (request, env) =>
    routeEffect('handle_omni_operator_billing_credits_api', () =>
      operatorBillingHandlers.handleOmniOperatorBillingCreditsApi(request, env),
    ),
  handleOmniOperatorInferenceCreditApi: (request, env) =>
    routeEffect('handle_omni_operator_inference_credit_api', () =>
      operatorBillingHandlers.handleOmniOperatorInferenceCreditApi(
        request,
        env,
      ),
    ),
  handleOmniOperatorDeploymentsApi: (request, env) =>
    routeEffect('handle_omni_operator_deployments_api', () =>
      omniHandlers.handleOmniOperatorDeploymentsApi(request, env),
    ),
  handleOmniOperatorFleetApi: (request, env) =>
    routeEffect('handle_omni_operator_fleet_api', () =>
      omniHandlers.handleOmniOperatorFleetApi(request, env),
    ),
  handleOmniOperatorTeamChatMessagesApi: (request, env, ctx) =>
    routeEffect('handle_omni_operator_team_chat_messages_api', () =>
      omniHandlers.handleOmniOperatorTeamChatMessagesApi(request, env, ctx),
    ),
})

const teamChatRoutes = makeTeamChatRoutes({
  handleTeamChatMessagesApi: (request, env, ctx, teamId, projectId) =>
    routeEffect('handle_team_chat_messages_api', () =>
      handleTeamChatMessagesApi(request, env, ctx, teamId, projectId),
    ),
})

const forumRoutes = makeForumRoutes()

const imageGenerationRoutes = makeImageGenerationRoutes({
  appUrl: env => getOpenAgentsWorkerConfig(env).app.url,
  appendRefreshedSessionCookies,
  requireOperatorAccess: async (env, session) =>
    (await readActiveTeamMembershipRole(
      openAgentsDatabase(env),
      OPENAGENTS_CORE_TEAM_ID,
      session.user.userId,
    )) !== undefined,
  requireBrowserSession,
})

const siteRuntimeRoutes = makeSiteRuntimeRoutes({
  sitesHost: 'sites.openagents.com',
})

const recordPublicAgentFunnelRead = (
  request: Request,
  db: D1Database,
  ctx: ExecutionContext,
  eventKind: ViralAgentFunnelEventKind,
  route: string,
): void => {
  scheduleBackgroundWork(
    ctx,
    recordViralAgentFunnelEvent(db, request, {
      eventKind,
      proofRef: route === '/api/public/proof/otec' ? 'proof:otec' : null,
      route,
      siteSlug: route === '/api/public/proof/otec' ? 'otec' : null,
    }),
  )
}

// Inference gateway provider registry (EPIC #5474, #5476). Seeded with the
// stub/echo adapter so the route works end-to-end while the gateway is inert.
// Phase-2 provider issues register their adapter exactly once here:
//   #5479 Fireworks, #5480 Vertex Anthropic, #5481 partner passthrough.
const inferenceProviderRegistry = new InferenceProviderRegistry()
inferenceProviderRegistry.register(stubEchoAdapter)
inferenceProviderRegistry.register(fireworksAdapter)
// Strong-coding alias of the Fireworks adapter for the internal MirrorCode
// frontier-coding gym rung. It serves the frontier GLM coding model (the chat
// route rewrites the request model via `khalaStrongCodingRequestForAdapter`) and
// FORCES every failure to be retryable so dispatch always overflows to the
// proven Fireworks Khala backing if the frontier coding model is unavailable —
// the strong lane is best-effort, never a hard-fail.
const toRetryableStrongCodingError = (
  error: InferenceAdapterError,
): InferenceAdapterError =>
  error.retryable
    ? error
    : new InferenceAdapterError({
        adapterId: FIREWORKS_STRONG_CODING_ADAPTER_ID,
        kind: error.kind ?? 'strong_coding_lane_unavailable',
        reason: error.reason,
        retryable: true,
        ...(error.httpStatus === undefined
          ? {}
          : { httpStatus: error.httpStatus }),
      })
const fireworksStrongCodingAdapter: InferenceProviderAdapter = {
  complete: request =>
    fireworksAdapter
      .complete(request)
      .pipe(Effect.mapError(toRetryableStrongCodingError)),
  id: FIREWORKS_STRONG_CODING_ADAPTER_ID,
  stream: request =>
    fireworksAdapter
      .stream(request)
      .pipe(Effect.mapError(toRetryableStrongCodingError)),
  ...(fireworksAdapter.streamSse === undefined
    ? {}
    : {
        streamSse: request =>
          fireworksAdapter
            .streamSse!(request)
            .pipe(Effect.mapError(toRetryableStrongCodingError)),
      }),
}
inferenceProviderRegistry.register(fireworksStrongCodingAdapter)

// Partner passthrough adapters (#5481). Registered from Worker secrets at
// request time (env is per-request in Workers); `register` replaces by id, so
// repeated calls are idempotent. INERT under the flag: when a partner secret is
// absent the adapter is never registered, and even when registered the route is
// only reachable with INFERENCE_GATEWAY_ENABLED on. The redacted key never
// leaves this closure except onto the outbound partner request header.
const passthroughAdaptersRegistered = new WeakSet<object>()

// Only the partner-secret slice of the Worker env is read here, so we accept a
// narrow shape rather than the full Cloudflare `Env` (which the zero-debt check
// keeps off new business surfaces).
type PassthroughSecretsEnv = Readonly<{
  ANTHROPIC_API_KEY?: string | undefined
  ANTHROPIC_BASE_URL?: string | undefined
  OPENAI_API_KEY?: string | undefined
  OPENAI_BASE_URL?: string | undefined
}>

const registerPassthroughAdapters = (
  registry: InferenceProviderRegistry,
  env: PassthroughSecretsEnv,
): void => {
  if (passthroughAdaptersRegistered.has(env)) {
    return
  }
  passthroughAdaptersRegistered.add(env)

  const anthropicKey = env.ANTHROPIC_API_KEY?.trim()
  if (anthropicKey !== undefined && anthropicKey !== '') {
    const config: PassthroughAdapterConfig = {
      apiKey: Redacted.make(anthropicKey),
      baseUrl: env.ANTHROPIC_BASE_URL?.trim() || 'https://api.anthropic.com',
      id: 'passthrough-anthropic',
      wireFormat: 'anthropic',
    }
    registry.register(makePassthroughAdapter(config))
  }

  const openaiKey = env.OPENAI_API_KEY?.trim()
  if (openaiKey !== undefined && openaiKey !== '') {
    const config: PassthroughAdapterConfig = {
      apiKey: Redacted.make(openaiKey),
      baseUrl: env.OPENAI_BASE_URL?.trim() || 'https://api.openai.com',
      id: 'passthrough-openai',
      wireFormat: 'openai',
    }
    registry.register(makePassthroughAdapter(config))
  }
}

// Hydralisk GPT-OSS lanes (#6155 + 120B follow-on). Registered only when the
// presence-derived serving policy arms the specific model, so catalog/quote/chat
// all agree: an armed 20B L4 host never accidentally advertises 120B.
const hydraliskAdaptersRegistered = new WeakSet<object>()
const hydraliskGlm52PoolRuntimes = new WeakMap<
  object,
  ReturnType<typeof makeHydraliskVllmPoolRuntime>
>()

type HydraliskServeEnv = OpenAgentsWorkerConfigEnv

const registerConfiguredHydraliskAdapter = (
  registry: InferenceProviderRegistry,
  input: Readonly<{
    adapterId: string
    baseUrl: string | undefined
    bearerToken: string | undefined
    upstreamModel: string
  }>,
): void => {
  const bearerToken = input.bearerToken?.trim()
  const baseUrl = input.baseUrl?.trim()
  if (
    bearerToken === undefined ||
    bearerToken === '' ||
    baseUrl === undefined ||
    baseUrl === ''
  ) {
    return
  }
  registry.register(
    makeHydraliskVllmAdapter({
      apiKey: Redacted.make(bearerToken),
      baseUrl,
      id: input.adapterId,
      upstreamModel: input.upstreamModel,
    }),
  )
}

const registerHydraliskAdapter = (
  registry: InferenceProviderRegistry,
  env: HydraliskServeEnv,
): void => {
  if (hydraliskAdaptersRegistered.has(env)) {
    return
  }
  hydraliskAdaptersRegistered.add(env)
  const arming = resolveSupplyLaneArming(env)
  const glm52 = resolveHydraliskGlm52Reap504bArming(env)
  if (glm52.replicas.length > 0) {
    const runtime = makeHydraliskVllmPoolRuntime({
      id: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      replicas: glm52.replicas.map(replica => ({
        apiKey: Redacted.make(replica.bearerToken),
        baseUrl: replica.baseUrl,
        benchmarkReserved: replica.benchmarkReserved,
        costProfileRef: replica.costProfileRef,
        draining: replica.draining,
        evidenceRefs: replica.evidenceRefs,
        id: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
        maxInflight: replica.maxInflight,
        profileRef: replica.profileRef,
        replicaId: replica.replicaId,
        upstreamModel: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
      })),
      routingStateOracle: glmPoolHeartbeatRoutingStateOracle,
      upstreamModel: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
    })
    hydraliskGlm52PoolRuntimes.set(env, runtime)
    registry.register(runtime.adapter)
  } else if (
    arming.hydraliskModels?.[HYDRALISK_GLM_52_REAP_504B_MODEL_ID] === true
  ) {
    hydraliskGlm52PoolRuntimes.delete(env)
    registerConfiguredHydraliskAdapter(registry, {
      adapterId: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      baseUrl: env.HYDRALISK_GLM_52_REAP_504B_BASE_URL,
      bearerToken: env.HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN,
      upstreamModel: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
    })
  } else {
    hydraliskGlm52PoolRuntimes.delete(env)
  }
  if (arming.hydraliskModels?.[HYDRALISK_GPT_OSS_20B_MODEL_ID] === true) {
    registerConfiguredHydraliskAdapter(registry, {
      adapterId: HYDRALISK_ADAPTER_ID,
      baseUrl: env.HYDRALISK_BASE_URL,
      bearerToken: env.HYDRALISK_BEARER_TOKEN,
      upstreamModel: HYDRALISK_GPT_OSS_20B_MODEL_ID,
    })
  }
  if (arming.hydraliskModels?.[HYDRALISK_GPT_OSS_120B_MODEL_ID] === true) {
    registerConfiguredHydraliskAdapter(registry, {
      adapterId: HYDRALISK_GPT_OSS_120B_ADAPTER_ID,
      baseUrl: env.HYDRALISK_GPT_OSS_120B_BASE_URL,
      bearerToken: env.HYDRALISK_GPT_OSS_120B_BEARER_TOKEN,
      upstreamModel: HYDRALISK_GPT_OSS_120B_MODEL_ID,
    })
  }
}

const hydraliskGlm52RouteAdmissionForEnv = (
  env: HydraliskServeEnv,
): HydraliskPoolRouteAdmissionSnapshot | undefined =>
  hydraliskGlm52PoolRuntimes.get(env)?.routeAdmission()

const openRouterAdaptersRegistered = new WeakSet<object>()

type OpenRouterServeEnv = Readonly<{
  OPENROUTER_API_KEY?: string | undefined
  OPENROUTER_BASE_URL?: string | undefined
  OPENROUTER_KHALA_FALLBACK_MODEL?: string | undefined
}>

const registerOpenRouterAdapter = (
  registry: InferenceProviderRegistry,
  env: OpenRouterServeEnv,
): void => {
  if (openRouterAdaptersRegistered.has(env)) {
    return
  }
  openRouterAdaptersRegistered.add(env)
  const apiKey = env.OPENROUTER_API_KEY?.trim()
  const baseUrl = env.OPENROUTER_BASE_URL?.trim() || OPENROUTER_DEFAULT_BASE_URL
  if (
    apiKey === undefined ||
    apiKey === ''
  ) {
    return
  }
  registry.register(
    makeOpenRouterAdapter({
      apiKey: Redacted.make(apiKey),
      baseUrl,
      id: OPENROUTER_KHALA_FALLBACK_ADAPTER_ID,
      upstreamModel: OPENROUTER_KHALA_FALLBACK_MODEL_ID,
    }),
  )
}

// OpenAgents serving-fabric lane (#5483 / Khala M4 #6012 / #6089) — the
// `openagents-network` adapter wired to a Psionic serve transport. The lane is
// routed AHEAD of passthrough for the OPEN class (model-router.ts), but stays
// INERT unless the live deploy has BOTH:
//   1. the public route/evidence arming refs checked by model-serving-policy,
//   2. a secret HTTP endpoint + bearer token for the actual Pylon proxy.
//
// The endpoint must speak the Psionic serve response contract, not raw vLLM
// OpenAI output: the admitted adapter still checks Pylon admission before
// dispatch and requires parity + canary + replay + payout-eligibility evidence
// before paid routing clears. With any piece absent, no adapter is registered,
// so dispatch skips the lane and overflows to the existing cloud paths.
type FabricServeEnv = OpenAgentsWorkerConfigEnv

const registerFabricServeAdapter = (
  registry: InferenceProviderRegistry,
  env: FabricServeEnv,
): void => {
  if (!resolveSupplyLaneArming(env)['openagents-network']) {
    return
  }
  const transportConfig = pylonFabricHttpTransportConfigFromEnv(env)
  if (transportConfig === undefined) {
    return
  }
  const transport = makePylonFabricHttpTransport(transportConfig)
  registry.register(
    makeAdmittedOpenAgentsNetworkAdapter({
      admission: () => pylonGatewayAdmissionFromEnv(env, currentEpochMillis()),
      dispatch: dispatchPsionicServe({ transport }),
    }),
  )
}

const makeConfiguredFabricServeAdapter = (env: FabricServeEnv) => {
  if (!resolveSupplyLaneArming(env)['openagents-network']) {
    return undefined
  }
  const transportConfig = pylonFabricHttpTransportConfigFromEnv(env)
  if (transportConfig === undefined) {
    return undefined
  }
  const transport = makePylonFabricHttpTransport(transportConfig)
  return makeAdmittedOpenAgentsNetworkAdapter({
    admission: () => pylonGatewayAdmissionFromEnv(env, currentEpochMillis()),
    dispatch: dispatchPsionicServe({ transport }),
  })
}

// Per-request env holder for env-dependent inference adapters. A Cloudflare
// Worker has no env at module scope, so the module-level adapter registry reads
// the live env (set by the /v1/chat/completions handler before dispatch) when
// it needs to mint credentials. Within a single request the isolate is
// single-threaded, so this is not racy.
// Typed as OpenAgentsWorkerConfigEnv (a subset of the full worker env) so this
// holder reads only the VERTEX_* config fields it needs, not the full
// Cloudflare binding surface.
let inferenceAdapterEnv: OpenAgentsWorkerConfigEnv | undefined
const setInferenceAdapterEnv = (env: OpenAgentsWorkerConfigEnv): void => {
  inferenceAdapterEnv = env
}

// Async batch-job consumer wiring (Khala, EPIC #6017 / #6028). Assembles the
// consumer deps from the live env using the SAME seams the interactive
// /v1/chat/completions handler uses: the module-level provider-adapter registry
// (with partner/fabric adapters + the per-request env captured first), the live
// ledger metering hook (so each batch item decrements credits exactly as an
// interactive completion would), and the cheapest-viable lane plan with
// bounded-backoff overflow. INERT by default — the queue handler only calls
// `executeBatchJob` with these deps when INFERENCE_BATCH_JOBS_ENABLED is on, so
// nothing here changes prod behaviour until the path is explicitly armed.
//
// Reads only the narrow slices it needs — the D1 binding plus the inference
// config/secret fields (`OpenAgentsWorkerConfigEnv` already carries the
// passthrough/Vertex secrets) — rather than a raw Cloudflare `Env`, keeping the
// worker off the raw-Env ratchet.
type BatchJobConsumerEnv = OpenAgentsWorkerConfigEnv &
  Pick<WorkerBindings, 'OPENAGENTS_DB'>
const makeBatchJobConsumerDeps = (env: BatchJobConsumerEnv) => {
  registerPassthroughAdapters(inferenceProviderRegistry, env)
  registerHydraliskAdapter(inferenceProviderRegistry, env)
  registerOpenRouterAdapter(inferenceProviderRegistry, env)
  registerFabricServeAdapter(inferenceProviderRegistry, env)
  setInferenceAdapterEnv(env)
  const laneArming = resolveSupplyLaneArming(env)
  return {
    dispatch: {
      plan: makeKhalaBackedAdapterPlan(laneArming.khalaBacking),
      registry: inferenceProviderRegistry,
    },
    meteringHook: makeLedgerMeteringHook({ db: openAgentsDatabase(env) }),
    // Book P0-3 (#6086): the consumer stamps the start-of-processing time (the
    // END of the batch wait) with this clock so the closeout receipt can disclose
    // an honest `batchWaitMs`.
    nowIso: currentIsoTimestamp,
    store: makeD1BatchJobStore(openAgentsDatabase(env), currentIsoTimestamp),
  }
}

// Onboarding program inference client (EPIC #6123, #6126). Builds an
// `OnboardingInferenceClient` that calls the Khala orchestrator
// (`openagents/khala-mini`) through the SAME provider-adapter registry +
// cheapest-viable overflow dispatch the interactive /v1/chat/completions handler
// uses — an INTERNAL call, no external HTTP hop. It first registers the
// per-request env (partner/fabric adapters + Vertex config) exactly like the
// batch consumer, so khala-mini routes to its live supply lane. When no lane is
// configured (e.g. no provider secrets / inert env) the dispatch fails with a
// typed adapter error, which the onboarding route maps to a stable
// inference_unavailable response.
type OnboardingInferenceEnv = OpenAgentsWorkerConfigEnv
const makeOnboardingInferenceClient = (
  env: OnboardingInferenceEnv,
): OnboardingInferenceClient => {
  registerPassthroughAdapters(inferenceProviderRegistry, env)
  registerHydraliskAdapter(inferenceProviderRegistry, env)
  registerOpenRouterAdapter(inferenceProviderRegistry, env)
  registerFabricServeAdapter(inferenceProviderRegistry, env)
  setInferenceAdapterEnv(env)
  const laneArming = resolveSupplyLaneArming(env)
  return (request: InferenceRequest) =>
    dispatchWithOverflow<InferenceResult>(
      request,
      (adapter, req) => adapter.complete(req),
      {
        plan: makeKhalaBackedAdapterPlan(laneArming.khalaBacking),
        registry: inferenceProviderRegistry,
      },
    ).pipe(
      Effect.map(result => result.content),
      Effect.mapError(
        error => new OnboardingInferenceError({ reason: error.reason }),
      ),
    )
}

// STREAMING onboarding client (issue #6123 UI follow-up; #6154 incremental).
// The same provider-adapter registry + overflow dispatch as the buffered client,
// but it prefers the adapter's TRUE incremental `streamSse` so the onboarding
// reply streams token-by-token — one `event: delta` per upstream Gemini fragment
// — instead of one buffered reply materialized server-side. Adapters without a
// `streamSse` (stub/echo, simple test adapters) fall back to the buffered chunk
// `stream`. Overflow happens at source-open time exactly like the `/v1` gateway:
// the dispatched Effect resolves once the upstream stream HEAD is accepted (a
// non-2xx surfaces as a retryable adapter error BEFORE any frame is consumed), so
// a failing lane overflows to the next without buffering the body.
//
// The source's `final()` returns '' (no content re-buffering): the route already
// accumulates the deltas it emits and persists that accumulation, so the lazy
// frames stay the single source of truth (receipt-first, matching the gateway).
// The dispatch operation + source shaping live in `onboarding-stream-source.ts`
// so the prefer-streamSse / fall-back-to-stream behavior is unit-testable.
const makeOnboardingStreamClient = (
  env: OnboardingInferenceEnv,
): OnboardingStreamClient => {
  registerPassthroughAdapters(inferenceProviderRegistry, env)
  registerHydraliskAdapter(inferenceProviderRegistry, env)
  registerOpenRouterAdapter(inferenceProviderRegistry, env)
  registerFabricServeAdapter(inferenceProviderRegistry, env)
  setInferenceAdapterEnv(env)
  const laneArming = resolveSupplyLaneArming(env)
  return (request: InferenceRequest) =>
    dispatchWithOverflowWithMetadata<OnboardingStreamSource>(
      request,
      dispatchOnboardingStreamSource,
      {
        plan: makeKhalaBackedAdapterPlan(laneArming.khalaBacking),
        registry: inferenceProviderRegistry,
      },
    ).pipe(
      Effect.map(result => ({
        ...result.value,
        metadata: () => ({
          ...(result.value.metadata?.() ?? {}),
          fallbackReason: result.route.fallbackReason,
          primaryAdapterId: result.route.primaryAdapterId,
          requestedModel: request.model,
          servedAdapterId: result.route.servedAdapterId,
        }),
      })),
      Effect.mapError(
        error => new OnboardingInferenceError({ reason: error.reason }),
      ),
    )
}

// GENERIC PUBLIC KHALA CHAT stream client (the `/khala` chat demo). Identical
// wiring to `makeOnboardingStreamClient`: the SAME provider-adapter registry +
// overflow dispatch + incremental `dispatchOnboardingStreamSource` bridge the
// gateway and the onboarding route use (no auth/credit gate, no external HTTP
// hop). The difference is the caller (the generic stateless `/api/khala/chat`
// route) and the system prompt (Khala identity + generic chat instruction,
// assembled in `khala-chat-program.ts`) — NOT the concierge intake program.
const khalaPublicChatRequestForAdapter = (
  request: InferenceRequest,
  adapterId: string,
): InferenceRequest => {
  if (normalizeKhalaModelId(request.model) !== KHALA_MODEL_ID) {
    return request
  }
  switch (adapterId) {
    case VERTEX_GEMINI_ADAPTER_ID:
      return { ...request, model: DEFAULT_GEMINI_MODEL_ID }
    case FIREWORKS_ADAPTER_ID:
      return { ...request, model: KHALA_FIREWORKS_BACKING_MODEL_ID }
    case HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID:
      return { ...request, model: HYDRALISK_GLM_52_REAP_504B_MODEL_ID }
    default:
      return request
  }
}

const makeKhalaChatStreamClient = (
  env: OnboardingInferenceEnv,
): KhalaChatStreamClient => {
  registerPassthroughAdapters(inferenceProviderRegistry, env)
  registerHydraliskAdapter(inferenceProviderRegistry, env)
  registerOpenRouterAdapter(inferenceProviderRegistry, env)
  registerFabricServeAdapter(inferenceProviderRegistry, env)
  setInferenceAdapterEnv(env)
  const laneArming = resolveSupplyLaneArming(env)
  return (request: InferenceRequest) =>
    dispatchWithOverflowWithMetadata<OnboardingStreamSource>(
      request,
      dispatchOnboardingStreamSource,
      {
        plan: makeKhalaBackedAdapterPlan(laneArming.khalaBacking),
        registry: inferenceProviderRegistry,
        requestForAdapter: khalaPublicChatRequestForAdapter,
      },
    ).pipe(
      Effect.map(result => ({
        ...result.value,
        metadata: () => ({
          ...(result.value.metadata?.() ?? {}),
          fallbackReason: result.route.fallbackReason,
          primaryAdapterId: result.route.primaryAdapterId,
          requestedModel: request.model,
          servedAdapterId: result.route.servedAdapterId,
        }),
      })),
      Effect.mapError(
        error => new OnboardingInferenceError({ reason: error.reason }),
      ),
    )
}

const makeArtanisResponderKhalaClient = (
  env: OnboardingInferenceEnv &
    Pick<WorkerBindings, 'OPENAGENTS_DB' | 'SYNC_ROOM'>,
): ArtanisResponderKhalaClient => {
  registerPassthroughAdapters(inferenceProviderRegistry, env)
  registerHydraliskAdapter(inferenceProviderRegistry, env)
  registerOpenRouterAdapter(inferenceProviderRegistry, env)
  registerFabricServeAdapter(inferenceProviderRegistry, env)
  setInferenceAdapterEnv(env)
  const laneArming = resolveSupplyLaneArming(env)
  const recordTokensServed = makeD1ServedTokensRecorder(
    openAgentsDatabase(env),
    {
      publishDelta: delta =>
        Effect.promise(() =>
          publishKhalaTokensServedDelta(
            env,
            buildKhalaTokensServedDelta(delta),
          ).catch(() => undefined),
        ),
    },
  )

  return (request: InferenceRequest) =>
    Effect.gen(function* () {
      const responseId = randomUuid()
      const result = yield* dispatchWithOverflowWithMetadata<InferenceResult>(
        request,
        (adapter, req) => adapter.complete(req),
        {
          plan: makeKhalaBackedAdapterPlan(laneArming.khalaBacking),
          registry: inferenceProviderRegistry,
          // Map `openagents/khala` to each lane's BACKING model before dispatch,
          // exactly like the public completions route (#6363). The conversational
          // Khala plan fans out across Vertex Gemini / Fireworks / GLM /
          // OpenRouter, each of which rejects the `openagents/khala` alias; without
          // this mapping every lane failed and the operator channel 503'd.
          requestForAdapter: khalaRequestForAdapter,
        },
      )
      const served = result.value
      // FAIL-SOFT METERING (issue #6363). The Khala dispatch already produced
      // the served reply; recording the served-token ledger row + publishing the
      // live-counter delta is a downstream PROJECTION and must NEVER fail the
      // turn. The Artanis operator core wraps this whole client in `Effect.exit`,
      // so a metering FAILURE *or DEFECT* (D1 write, sync push, ingest-body
      // build) would otherwise surface to the owner as a 503
      // `artanis_operator_mind_unavailable` even though Khala served the answer.
      // `meterServedTokensFailSoft` swallows both failures and defects (mirrors
      // the public completions path's fail-soft metering) so Artanis always
      // returns the served reply.
      yield* meterServedTokensFailSoft(recordTokensServed, {
        accountRef: ARTANIS_REGISTERED_ACTOR_REF,
        adapterId: result.route.servedAdapterId,
        requestAttribution: {
          demandClient: ARTANIS_RESPONDER_DEMAND_CLIENT,
          demandKind: 'internal',
          demandSource: ARTANIS_RESPONDER_DEMAND_SOURCE,
        },
        requestId: responseId,
        requestMetrics: {
          fallbackReason: result.route.fallbackReason,
          requestClass: 'async_job',
          supplyLane: result.route.servedAdapterId,
        },
        requestedModel: request.model,
        servedModel: served.servedModel,
        streamed: false,
        usage: served.usage,
      })
      return served
    })
}

const khalaChatRoutes = makeKhalaChatRoutes({
  makeStreamClient: env =>
    makeKhalaChatStreamClient(env as OnboardingInferenceEnv),
  recordServedTokens: recordPublicKhalaChatServedTokens,
})

// PRODUCER seam for the async batch-job submit route (Khala, #6028 / EPIC
// #6017). Returns an `enqueueBatchJob` that sends the executable
// `BatchJobQueueMessage` onto the batch-job queue so the consumer (the `queue`
// handler below) runs it OFF the request path. INERT BY DEFAULT and FAIL-SAFE:
//   - when `INFERENCE_BATCH_JOBS_ENABLED` is off, returns `undefined` so the
//     submit route persists the pending row + returns the receipt exactly as
//     before, with nothing queued (no behaviour change to the existing route);
//   - when the queue binding is absent (not provisioned), also returns
//     `undefined` rather than throwing, so arming the flag without the binding
//     degrades to accept-only instead of erroring the request.
// The job id is the idempotency unit (the consumer no-ops a redelivered/dup
// message), so a duplicate enqueue is always safe.
type BatchJobProducerEnv = OpenAgentsWorkerConfigEnv &
  Readonly<{ INFERENCE_BATCH_JOBS_QUEUE?: Queue | undefined }>
const makeBatchJobEnqueue = (
  env: BatchJobProducerEnv,
): ((message: BatchJobQueueMessage) => Effect.Effect<void>) | undefined => {
  if (!isInferenceGatewayEnabled(env.INFERENCE_BATCH_JOBS_ENABLED)) {
    return undefined
  }
  const queue = env.INFERENCE_BATCH_JOBS_QUEUE
  if (queue === undefined) {
    return undefined
  }
  return message =>
    Effect.tryPromise(() => queue.send(message)).pipe(Effect.orDie)
}

// #5480 Vertex Anthropic (Claude lane) — registered exactly once. INERT until
// the VERTEX_SA_KEY Worker secret is present: with no key, `tokenProvider` is
// undefined and every call returns a typed non-retryable error (mapped by the
// route to a stable provider_error). The route itself stays flag-gated off via
// INFERENCE_GATEWAY_ENABLED. Project/location/key are read from the live env at
// call time via the per-request holder above.
inferenceProviderRegistry.register(
  makeVertexAnthropicAdapter({
    // Lane defaults: project openagentsgemini, global location (broadest
    // shared-lineage quota, no regional premium — gateway business doc §3a).
    // VERTEX_PROJECT_ID / VERTEX_LOCATION env overrides are reserved for a
    // follow-up; the module-level registry is constructed once, before any env
    // is captured, so it pins these defaults.
    location: 'global',
    project: 'openagentsgemini',
    resolveModelId: undefined,
    tokenProvider: () => {
      const env = inferenceAdapterEnv
      const provider =
        env === undefined
          ? undefined
          : tokenProviderFromSecret(env.VERTEX_SA_KEY)
      return provider === undefined
        ? Effect.fail(
            new InferenceAdapterError({
              adapterId: VERTEX_ANTHROPIC_ADAPTER_ID,
              reason:
                'Vertex Anthropic adapter is not configured (missing VERTEX_SA_KEY).',
              retryable: false,
            }),
          )
        : provider()
    },
  }),
)

// Vertex Gemini (Google's own model) — the default/free-tier lane (Gemini 3.5
// Flash). Registered exactly once, sharing the SAME VERTEX_SA_KEY token path as
// the Anthropic lane. INERT until the secret is present: with no key the adapter
// returns a typed non-retryable error, and the route stays flag-gated off via
// INFERENCE_GATEWAY_ENABLED. Project/location pin the same lane defaults.
inferenceProviderRegistry.register(
  makeVertexGeminiAdapter({
    location: 'global',
    project: 'openagentsgemini',
    resolveModelId: undefined,
    tokenProvider: () => {
      const env = inferenceAdapterEnv
      const provider =
        env === undefined
          ? undefined
          : tokenProviderFromSecret(env.VERTEX_SA_KEY)
      return provider === undefined
        ? Effect.fail(
            new InferenceAdapterError({
              adapterId: VERTEX_GEMINI_ADAPTER_ID,
              reason:
                'Vertex Gemini adapter is not configured (missing VERTEX_SA_KEY).',
              retryable: false,
            }),
          )
        : provider()
    },
  }),
)

const dispatchFailureTelemetry = makeBoundedDispatchFailureTelemetry({
  maxEvents: 256,
  nowMs: currentEpochMillis,
  windowMs: 15 * 60 * 1_000,
})

const internalStressPreemption = makeInternalStressPreemptionRegistry()

const exactRouteRegistry = makeExactRouteRegistry<Env>([
  {
    path: '/',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleHomePage(request, env, ctx)),
  },
  {
    path: '/api/public/home',
    handler: request => handlePublicHomeApi(request),
  },
  {
    path: '/api/khala/feedback',
    handler: (request, env) =>
      handleKhalaFeedbackSubmit(request, {
        store: makeD1KhalaFeedbackStore(openAgentsDatabase(env)),
      }),
  },
  {
    path: '/api/khala/tokens',
    handler: (request, env) => handlePublicKhalaTokensServedApi(request, env),
  },
  {
    path: '/api/operator/khala/feedback',
    handler: (request, env) =>
      handleOperatorKhalaFeedback(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1KhalaFeedbackStore(openAgentsDatabase(env)),
      }),
  },
  {
    path: '/api/operator/khala/trace-review',
    handler: (request, env) =>
      handleOperatorKhalaTraceReview(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1KhalaTraceReviewStore(openAgentsDatabase(env)),
      }),
  },
  {
    path: '/api/operator/khala/unsupported-requests',
    handler: (request, env) =>
      handleOperatorKhalaUnsupportedRequests(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1KhalaUnsupportedRequestStore(openAgentsDatabase(env)),
      }),
  },
  {
    path: '/api/public/business-signup',
    handler: (request, env) =>
      handleBusinessSignupApi(request, openAgentsDatabase(env)),
  },
  {
    path: '/api/public/tassadar-run-summary',
    handler: (request, env) =>
      Effect.promise(() =>
        buildPublicTassadarRunSummaryEnvelopeForRequest(request, env),
      ).pipe(Effect.map(envelope => noStoreJsonResponse(envelope))),
  },
  {
    path: TrainingPublicDistributedRunScaleEndpoint,
    handler: (request, env) =>
      handleTrainingPublicDistributedRunScaleApi(request, env),
  },
  {
    path: PylonLargestDecentralizedTrainingClaimEndpoint,
    handler: (request, env) =>
      handlePylonLargestDecentralizedTrainingClaimStatusApi(request, env),
  },
  {
    path: '/api/public/activity-timeline',
    handler: (request, env) =>
      handlePublicActivityTimelineApiForEnv(request, env),
  },
  {
    path: '/api/public/activity-timeline/stream',
    handler: (request, env) =>
      handlePublicActivityTimelineStreamApiForEnv(request, env),
  },
  {
    path: '/api/public/forum-activity',
    handler: (request, env) => handlePublicForumActivityApiForEnv(request, env),
  },
  {
    path: TASSADAR_COMPILED_MODULE_MARKETPLACE_ROUTE,
    handler: () =>
      Effect.promise(() =>
        buildPublicTassadarCompiledModuleMarketplaceEnvelope(),
      ).pipe(Effect.map(envelope => noStoreJsonResponse(envelope))),
  },
  {
    path: '/api/public/tassadar-replays/first-real-settlement',
    handler: (request, env) =>
      Effect.promise(() => handlePublicProofReplayBundleRequest(request, env)),
  },
  {
    path: '/api/public/proof-replays',
    handler: (request, env) =>
      Effect.promise(() => handlePublicProofReplayBundleRequest(request, env)),
  },
  {
    path: '/api/public/product-promises',
    handler: (request, env) =>
      handlePublicProductPromisesApi(request, openAgentsDatabase(env)),
  },
  {
    // Free-API data-sharing terms / consent disclosure (#6296). Public,
    // agent-readable: the honest free-tier terms (captured by default, redacted,
    // private owner_only, may improve/train; pay-for-privacy opt-out; public
    // sharing opt-in only; reward marker inert). Same canonical disclosure the
    // free-key mint response embeds. Read-only, no auth, no DB, no secrets.
    path: '/api/public/free-tier-data-sharing',
    handler: request => handleFreeTierDataSharingDisclosureApi(request),
  },
  {
    path: '/api/public/product-promises/transitions',
    handler: (request, env) =>
      handlePublicPromiseTransitionsApi(request, {
        store: makeD1PromiseTransitionReceiptStore(openAgentsDatabase(env)),
      }),
  },
  {
    // Enterprise claim-upgrade audit projection (proof.claim_upgrade_receipts.v1).
    // Read-only: joins the transition-receipt feed against the live registry so
    // a third party can audit every green flip (promiseId, from->to,
    // registryVersion, receiptRef, lastVerifiedAt) with filtering + summary.
    path: '/api/public/product-promises/audit',
    handler: (request, env) =>
      handlePublicPromiseAuditApi(request, {
        store: makeD1PromiseTransitionReceiptStore(openAgentsDatabase(env)),
      }),
  },
  {
    path: '/api/public/metrics/accepted-outcomes-per-kwh',
    handler: request => handleAcceptedOutcomesPerKwhApi(request),
  },
  {
    // Public-safe live Gym / Harbor run progress (#6261). web_authorized runs
    // render live counts/denominator/pass-rate-over-completed/freshness with the
    // in-progress + decisionGrade:false markers; local_only runs degrade to an
    // honest awaiting-authorization marker. No raw prompts/responses/logs/keys.
    path: '/api/public/gym/run-progress',
    handler: (request, env) =>
      handlePublicGymRunProgressApi(request, {
        store: makeD1GymRunProgressStore(openAgentsDatabase(env)),
      }),
  },
  {
    // Public, dereferenceable Gym benchmark LADDER leaderboard (#6309, GTM §4).
    // The three rungs (Big Pickle -> free models -> paid frontier) on the same
    // OpenCode coding surface + our axes (cost-per-accepted-outcome,
    // verified-rate, tool-call completion). Returns the latest owner-armed
    // decision-grade published snapshot; when none exists it serves the honest
    // empty ladder shape (all rungs awaiting_owner with their owner-gate refs)
    // so the surface never fabricates a measurement. Read-only, no auth.
    path: '/api/public/gym/leaderboard',
    handler: (request, env) =>
      handlePublicGymLeaderboardApi(request, {
        store: makeD1GymLadderStore(openAgentsDatabase(env)),
      }),
  },
  {
    // Public, dereferenceable Khala external HEAD-TO-HEAD quality bar (#6308,
    // GTM §4). Khala vs the tools/models a developer would otherwise reach for
    // (default coding model -> free/open -> paid frontier), each matchup scored
    // on solve-rate AND cost-per-accepted-outcome. Returns the latest owner-armed
    // decision-grade published snapshot; when none exists it serves the honest
    // empty shape (all matchups awaiting_owner with their owner-gate refs) so the
    // surface never fabricates a measurement. Read-only, no auth.
    path: '/api/public/khala/head-to-head',
    handler: (request, env) =>
      handlePublicKhalaHeadToHeadApi(request, {
        store: makeD1KhalaHeadToHeadStore(openAgentsDatabase(env)),
      }),
  },
  {
    // Contributor accrual bundle dereference, addressed by accepted-outcome
    // economics id (?economicsId=...) for payments.accepted_outcome_economics.v1
    // (blocker.product_promises.contributor_ledger_missing). Read-only public
    // projection: returns the reconciled gross-margin receipt + contributor
    // accrual ledger with lifecycle/evidence labels visible and internal monetary
    // figures dropped. No dispatch, spend, settlement, or payout — every entry's
    // payable/settlement state stays honestly not_yet_evidenced.
    path: '/api/public/payments/contributor-accrual-bundle',
    handler: (request, env) =>
      handleOmniContributorAccrualBundleApi(request, openAgentsDatabase(env)),
  },
  {
    // Full training-pipeline program status (#5523 / DE-5 #5528; promise
    // training.full_pipeline_program.v1, planned). Read-only stage map: exposes
    // current stage receipt surfaces and blockers while keeping the umbrella
    // blocker active. No dispatch, spend, settlement, model promotion, or green
    // claim.
    path: TrainingFullPipelineProgramEndpoint,
    handler: request => handleTrainingFullPipelineProgramApi(request),
  },
  {
    // Marathon-operations status projection (#5523 / DE-5 #5528; promise
    // training.marathon_operations.v1, planned). Read-only status surface:
    // exposes durable-seal and standby predicates while real checkpoint
    // read-back, standby-promotion, and curtailment-drill receipts remain false.
    path: TrainingMarathonOperationsEndpoint,
    handler: request => handleTrainingMarathonOperationsApi(request),
  },
  {
    // Model-ladder rung status projection (#5523 / DE-5 #5528; promise
    // training.model_ladder.v1, planned). Read-only ladder surface: exposes R0,
    // the published R1 closeout criteria, and the economics-gate format while
    // keeping R1/R2 closeout receipts and green authority false.
    path: TrainingModelLadderRungsEndpoint,
    handler: request => handleTrainingModelLadderRungsApi(request),
  },
  {
    // Public gradient-window status projection (#5523 / DE-5 #5528; promise
    // training.public_gradient_windows.v1, planned). Read-only regime/receipt
    // surface: exposes the gate and promoted-window receipt emitter while no
    // live public window, promotion receipt, settlement, or green claim exists.
    path: TrainingPublicGradientWindowsEndpoint,
    handler: request => handleTrainingPublicGradientWindowsApi(request),
  },
  {
    // Training ablation derisking ledger projection (#5523 / DE-5 #5528;
    // promise training.ablation_system.v1, planned). Read-only candidate
    // ledger: clears the projection + one-delta harness + eval-reproduction
    // blockers while paid dispatch and verdict gates remain false. No ablation
    // execution, spend, settlement, model promotion, or green claim.
    path: TrainingAblationDeriskingLedgerEndpoint,
    handler: request => handleTrainingAblationDeriskingLedgerApi(request),
  },
  {
    // Post-training instruct SFT lane receipt (#5523 / DE-5 #5528;
    // promise training.post_training_arc.v1, planned). Read-only fixture-scale
    // lane receipt: clears only the generic instruct-SFT lane blocker while
    // paid dispatch, preference rollout, and vibe-test gates remain false. No
    // assignment, spend, settlement, model promotion, service, or green claim.
    path: TrainingPostTrainingInstructSftEndpoint,
    handler: request => handleTrainingPostTrainingInstructSftApi(request),
  },
  {
    // Post-training DPO preference workload projection (#5523 / DE-5 #5528;
    // promise training.post_training_arc.v1, planned). Read-only deterministic
    // reference grading receipt: exposes the DPO pair workload while paid
    // preference dispatch, real log-probs, settlement, and green gates remain
    // false. No assignment, spend, model update, service, or green claim.
    path: TrainingPostTrainingDpoPreferenceWorkloadEndpoint,
    handler: request =>
      handleTrainingPostTrainingDpoPreferenceWorkloadApi(request),
  },
  {
    // Post-training vibe-test rubric projection (#5523 / DE-5 #5528; promise
    // training.post_training_arc.v1, planned). Read-only deterministic rubric
    // and fixture closeout digest; real model transcripts, reviewer signature,
    // promotion, service, and green gates remain false. No assignment, spend,
    // settlement, model promotion, reviewed artifact, or green claim.
    path: TrainingPostTrainingVibeTestRubricEndpoint,
    handler: request => handleTrainingPostTrainingVibeTestRubricApi(request),
  },
  {
    // Tassadar Percepta executor architecture receipts (#5523 / DE-5 #5528;
    // promise models.tassadar_percepta_executor.v1, planned). Read-only refs and
    // digest projection: clears only the architecture-receipt blocker while
    // Pylon CPU-transform training receipts remain missing. No trained model,
    // inference endpoint, spend, settlement, promotion, or green claim.
    path: TassadarPerceptaArchitectureReceiptsEndpoint,
    handler: request => handleTassadarPerceptaArchitectureReceiptsApi(request),
  },
  {
    // Tassadar Percepta CPU-transform training receipt status (#5523 / DE-5
    // #5528; promise models.tassadar_percepta_executor.v1, planned). Read-only
    // status projection: exposes the architecture and Artanis dataset inputs
    // while every real training receipt gate remains false.
    path: TassadarPerceptaCpuTransformTrainingReceiptsEndpoint,
    handler: request =>
      handleTassadarPerceptaCpuTransformTrainingReceiptsApi(request),
  },
  {
    path: '/api/public/demand-provenance',
    handler: request => handleDemandProvenanceApi(request),
  },
  {
    path: '/api/public/markets/open-markets',
    handler: request => handleOpenMarketsSurfaceApi(request),
  },
  {
    path: '/api/public/markets/liquidity/skeleton',
    handler: request => handleLiquidityMarketSkeletonApi(request),
  },
  {
    path: '/api/public/markets/risk/skeleton',
    handler: request => handleRiskMarketSkeletonApi(request),
  },
  {
    // Compose-and-list marketplace MVP listing surface (#5510, #5515). INERT:
    // the store is empty unless MARKETPLACE_COMPOSE_AND_LIST_ENABLED is armed,
    // and the response always reports inert/planned. Read-only.
    path: MarketplaceComposeListEndpoint,
    handler: (request, env) =>
      handleMarketplaceCompositionApi(request, {
        enabled: isMarketplaceComposeAndListEnabled(
          env.MARKETPLACE_COMPOSE_AND_LIST_ENABLED,
        ),
      }),
  },
  {
    // Signature usage-metering surface (#5523 / DE-6 #5529; promise
    // marketplace.signature_monetization.v1, red). INERT: the store is empty
    // unless SIGNATURE_USAGE_METERING_ENABLED is armed, and the response always
    // reports inert/red with the settlement blocker still open. It PRODUCES the
    // public-safe usage-evidence refs the signature revenue gate consumes
    // (clearing blocker.product_promises.signature_usage_metering_missing) and
    // makes no live-revenue or settlement claim. Read-only.
    path: SignatureUsageMeteringEndpoint,
    handler: (request, env) =>
      handleSignatureUsageMeteringApi(request, {
        enabled: isSignatureUsageMeteringEnabled(
          env.SIGNATURE_USAGE_METERING_ENABLED,
        ),
      }),
  },
  {
    // Pylon multi-earning-node projection (#5523 / DE-4 #5527; promise
    // pylon.v0_3_multi_earning_node.v1, red). INERT: the store is empty unless
    // PYLON_MULTI_EARNING_PROJECTION_ENABLED is armed, and the response always
    // reports inert/red. It is the safe public projection deliverable — it
    // distinguishes modeled/observed/pending/paid/settled amounts per earning
    // mode (clearing blocker.product_promises.safe_public_projection_missing)
    // and surfaces the install/receipt/settlement blockers as still owner-gated.
    // It records no earnings, moves no money, and makes no install-closed or
    // live-earning claim. Read-only.
    path: PylonMultiEarningNodeEndpoint,
    handler: (request, env) =>
      handlePylonMultiEarningNodeApi(request, {
        enabled: isPylonMultiEarningProjectionEnabled(
          env.PYLON_MULTI_EARNING_PROJECTION_ENABLED,
        ),
      }),
  },
  {
    // Mobile workroom approval projection (promise
    // mobile.voice_approval_companion.v1, yellow). INERT by default: the store
    // is empty unless MOBILE_WORKROOM_APPROVAL_PROJECTION_ENABLED is armed. When
    // armed it returns the existing read-only mobile approval-card projection:
    // no approval, execution, notification, payment, provider mutation, runner
    // launch, or public-claim upgrade. This clears ONLY
    // blocker.product_promises.mobile_projection_missing; voice-command
    // approval receipts and cross-device sync stay open, and the promise stays
    // yellow. GET only.
    path: MobileWorkroomApprovalProjectionEndpoint,
    handler: (request, env) =>
      handleMobileWorkroomApprovalProjectionApi(request, {
        enabled: isMobileWorkroomApprovalProjectionEnabled(
          env.MOBILE_WORKROOM_APPROVAL_PROJECTION_ENABLED,
        ),
        nowIso: currentIsoTimestamp,
      }),
  },
  {
    // Omni client-delivery business-object projection (DE-9 / EPIC #5532;
    // promise workrooms.omni_client_delivery_workrooms.v1, yellow). INERT by
    // default: the store is empty unless OMNI_CLIENT_DELIVERY_PROJECTION_ENABLED
    // is armed. When armed it projects the existing source-authorized
    // business-object delivery seam (buildOmniBusinessObjectDeliveryPlan) over
    // an injected client-delivery workroom store: per-write approval-gated
    // decisions plus the integration gate verdict. It applies no write, sends,
    // settles, spends, mutates a connector, notifies, launches a runner, or
    // upgrades a public claim (effectsApplied is always false). This clears ONLY
    // blocker.product_promises.omni_client_delivery_projection_missing; the
    // live-integration, owner-sign-off, and closeout-receipt blockers stay
    // owner-gated and the promise stays yellow. GET only.
    path: OmniClientDeliveryProjectionEndpoint,
    handler: (request, env) =>
      Effect.succeed(
        handleOmniClientDeliveryProjectionApi(request, {
          enabled: isOmniClientDeliveryProjectionEnabled(
            env.OMNI_CLIENT_DELIVERY_PROJECTION_ENABLED,
          ),
          nowIso: currentIsoTimestamp,
        }),
      ),
  },
  {
    // Voice-session transcript ingestion endpoint (#5523 / DE-7 #5530; promise
    // mobile.voice_session_evidence_transcript_ingest.v1, red). INERT by
    // default: when VOICE_PROGRAM_INGEST_ENABLED is OFF the endpoint returns an
    // honest inert/red payload and never runs the ingest core. When armed it
    // decodes already-transcribed, redacted, ref-only segments and runs the
    // existing pure buildVoiceProgramIngestProposal core to return an
    // approval-gated program-input proposal (no STT, no audio capture, no
    // mutation, no execution, no settlement). This clears ONLY
    // blocker.product_promises.voice_ingestion_endpoint_missing; the
    // transcription-service and approval-UI blockers stay owner/product-gated
    // and the promise stays red. POST only.
    path: VoiceProgramIngestEndpoint,
    handler: (request, env) =>
      Effect.promise(() =>
        handleVoiceProgramIngestApi(request, {
          enabled: isVoiceProgramIngestEnabled(
            env.VOICE_PROGRAM_INGEST_ENABLED,
          ),
        }),
      ),
  },
  {
    // Autopilot all-in-one composed-run scaffold (#5510, #5519). INERT: the
    // store is empty unless AUTOPILOT_COMPOSED_RUN_ENABLED is armed, and the
    // response always reports inert/planned over BOTH capstone promises
    // (autopilot.all_in_one_business_system.v1 + cloud.primitives_suite.v1). It
    // shows the composition shape (one balance, one receipt envelope across the
    // primitive scaffolds) and makes no live/billable claim. Read-only.
    path: AutopilotComposedRunEndpoint,
    handler: (request, env) =>
      handleAutopilotComposedRunApi(request, {
        enabled: isAutopilotComposedRunEnabled(
          env.AUTOPILOT_COMPOSED_RUN_ENABLED,
        ),
      }),
  },
  {
    // Agentic labor-product flow scaffold (promise
    // autopilot.agentic_labor_products.v1, yellow). INERT: the store is empty
    // unless AGENTIC_LABOR_PRODUCTS_ENABLED is armed, and the response always
    // reports inert/yellow. It models the end-to-end labor-product flow (post ->
    // order -> dispatch -> deliver -> settle) with a settlement receipt seam that
    // is flag-gated INERT and owner-gated; it makes no live-sale claim.
    // GET lists flows (read-only); GET ?receiptRef= dereferences a published
    // settlement receipt (empty store in prod => receipt:null, INERT);
    // GET ?view=real-sale-claims surfaces the claim-upgrade verdict over
    // published evidence bundles (empty store in prod => nothing substantiated,
    // blocker surfaced, never flips a promise). POST is
    // the SELF-SERVE order-planning path: a buyer/agent posts a listing and
    // orders it in one request and gets back the typed `ordered`-stage flow plan
    // with no operator staging (still INERT — dispatches nothing, debits nothing,
    // settles nothing). POST returns 503 unless the flag is armed.
    path: AgenticLaborProductEndpoint,
    handler: (request, env) =>
      handleAgenticLaborProductApi(request, {
        enabled: isAgenticLaborProductsEnabled(
          env.AGENTIC_LABOR_PRODUCTS_ENABLED,
        ),
      }),
  },
  {
    // Coding quick win self-serve pipeline orchestrator (promise business.coding_quick_win.v1).
    // POST is the self-serve path: accepts step-by-step evidence (scope,
    // provisioning, invocation, delivery, acceptance, payment) and returns
    // a valid receipt. INERT: it creates no new state, moves no money, and
    // serves strictly to orchestrate verifiable structures into a pipeline.
    path: CodingQuickWinPipelineEndpoint,
    handler: request => handleCodingQuickWinPipelineApi(request),
  },
  {
    // Self-serve control-center fanout scaffold (promise
    // autopilot.control_center_fanout_marketplace.v1, yellow). INERT: the store
    // is empty unless SELF_SERVE_FANOUT_ENABLED is armed, and the response
    // always reports inert/yellow/selfServe with workClass code_task. It models
    // a customer-initiated single-action fanout plan (gate decision + the linked
    // market work-request the fanout would list) over the existing lane-C gate;
    // the dispatch seam (dispatchSelfServeFanout) lists nothing. It clears only
    // the self-serve blocker (the plugin-marketplace-beyond-code_task blocker
    // stays uncleared) and makes no broad-live-marketplace claim. Read-only.
    path: SelfServeFanoutEndpoint,
    handler: (request, env) =>
      handleSelfServeFanoutApi(request, {
        enabled: isSelfServeFanoutEnabled(env.SELF_SERVE_FANOUT_ENABLED),
      }),
  },
  {
    // Marketplace work-class catalog (promise
    // autopilot.control_center_fanout_marketplace.v1, yellow). Read-only registry
    // view: it lists every registered work class with its status, names the single
    // live class (code_task), and always reports the still-uncleared
    // plugin-marketplace-beyond-code_task blocker. No flag, no store, nothing
    // executable — the projection's assertCatalogInvariants throws rather than let
    // any plugin class silently flip live. Makes no broad-live-marketplace claim.
    path: MarketplaceWorkClassCatalogEndpoint,
    handler: request => handleMarketplaceWorkClassCatalogApi(request),
  },
  {
    path: CustomerOneCohortEndpoint,
    handler: (request, env) =>
      handlePublicCustomerOneCohortApi(request, {
        store: makeD1CustomerOneCohortRowStore(openAgentsDatabase(env)),
      }),
  },
  {
    // Scoped operator surface for live Gym / Harbor runs (#6261, #6271).
    //   GET  returns every progress object, including local_only runs not yet
    //        authorized for web publication. Still public-safe; "scoped" gates
    //        visibility, not fields.
    //   POST ingests a Harbor-side pushed snapshot: it is REBUILT through
    //        buildGymRunProgress + checkGymRunProgressPublicSafety (rejecting any
    //        prompts/responses/logs/trajectories/keys/private endpoints with a
    //        typed 400) and upserted by runRef into D1.
    path: '/api/operator/gym/run-progress',
    handler: (request, env, ctx) =>
      handleOperatorGymRunProgressApi(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1GymRunProgressStore(openAgentsDatabase(env)),
        // Realtime push (#6261): after the upsert lands, publish the public-safe
        // projected snapshot to the live `public-gym-run-progress` sync scope so
        // the `/gym` follow-along updates the instant the snapshot is ingested.
        // Fail-soft and off the customer path via the execution context.
        publishProgress: progress =>
          publishGymRunProgressSnapshot(env, progress, { ctx }),
      }),
  },
  {
    // Recurring publish boundary for the Gym benchmark LADDER (#6309). The
    // operator (or the recurring scheduler) POSTs the decision-grade
    // GymLeaderboardReportInput[] from an owner-armed real sweep; the Worker
    // re-builds the ladder via buildGymLadderLeaderboard (decision-grade +
    // public-safety-checked rows only) and upserts the public-safe ladder by
    // ladderRef. GET returns the current published ladder. Admin-bearer gated.
    path: '/api/operator/gym/leaderboard',
    handler: (request, env) =>
      handleOperatorGymLeaderboardApi(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1GymLadderStore(openAgentsDatabase(env)),
      }),
  },
  {
    // Recurring publish boundary for the Khala external HEAD-TO-HEAD (#6308). The
    // operator (or the recurring scheduler) POSTs the decision-grade
    // GymLeaderboardReportInput[] from an owner-armed real sweep; the Worker
    // re-builds the bar via buildKhalaHeadToHead (decision-grade +
    // public-safety-checked rows only) and upserts the public-safe artifact by
    // headToHeadRef. GET returns the current published bar. Admin-bearer gated.
    path: '/api/operator/khala/head-to-head',
    handler: (request, env) =>
      handleOperatorKhalaHeadToHeadApi(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1KhalaHeadToHeadStore(openAgentsDatabase(env)),
      }),
  },
  {
    // MirrorCode-as-a-service demo runs (#6378, epic #6376). GET is the
    // public-safe leaderboard/list (stored Khala runs + LABELED illustrative
    // paper-reference comparators); POST is the owner-gated (admin bearer)
    // launch/record path that rebuilds each run through the no-task-contents /
    // no-canary public-safety boundary before upserting by runId. The
    // path-param `/api/gym/mirrorcode/runs/{id}` read is wired in the worker
    // route cascade (the exact-route registry cannot match a path param).
    path: '/api/gym/mirrorcode/runs',
    handler: (request, env) =>
      handleMirrorCodeRunsApi(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1MirrorCodeRunStore(openAgentsDatabase(env)),
      }),
  },
  {
    // Operator-only Harbor full trace archive (#6253). Stores raw Harbor job
    // tarballs in private R2 with D1 metadata. Unlike `/api/traces`, this is
    // NOT a public-safe ATIF projection and never appears on public `/gym`.
    path: '/api/operator/gym/full-trace-archives',
    handler: (request, env) =>
      handleOperatorHarborFullTraceArchivesApi(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1R2HarborFullTraceArchiveStore(
          openAgentsDatabase(env),
          env.ARTIFACTS,
        ),
      }),
  },
  {
    path: '/api/operator/customer-one-cohort/rows',
    handler: (request, env) =>
      handleOperatorCustomerOneCohortRowsApi(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        store: makeD1CustomerOneCohortRowStore(openAgentsDatabase(env)),
      }),
  },
  {
    path: '/api/operator/product-promises/transitions',
    handler: (request, env) =>
      handleOperatorPromiseTransitionApi(request, {
        requireAdminApiToken: () => requireAdminApiToken(request, env),
        store: makeD1PromiseTransitionReceiptStore(openAgentsDatabase(env)),
      }),
  },
  {
    path: '/api/operator/artanis/mind/smoke',
    handler: (request, env) =>
      Effect.gen(function* () {
        if (request.method !== 'POST') {
          return noStoreJsonResponse(
            { error: 'method_not_allowed' },
            { status: 405 },
          )
        }
        const authorized = yield* Effect.promise(() =>
          requireAdminApiToken(request, env),
        )
        if (!authorized) {
          return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
        }
        const apiKey = (env as { GEMINI_API_KEY?: string }).GEMINI_API_KEY
        if (apiKey === undefined || apiKey === '') {
          return noStoreJsonResponse(
            { error: 'gemini_api_key_missing' },
            { status: 503 },
          )
        }
        const body = yield* Effect.promise(async () => {
          try {
            return (await request.json()) as {
              forumPost?: boolean
              gatewayId?: string
              model?: string
              prompt?: string
            }
          } catch {
            return {}
          }
        })
        const prompt =
          body.prompt ??
          'State in one sentence what the Artanis administrator should verify before dispatching executor-trace work to an idle Pylon.'
        const gatewayToken = (env as { CF_AIG_TOKEN?: string }).CF_AIG_TOKEN
        const result = yield* Effect.promise(() =>
          artanisMindComplete({
            apiKey,
            ...(body.gatewayId === undefined
              ? {}
              : { gatewayId: body.gatewayId }),
            ...(gatewayToken === undefined || gatewayToken === ''
              ? {}
              : { gatewayToken }),
            ...(body.model === undefined ? {} : { model: body.model }),
            prompt,
            system: ArtanisMindSmokeSystem,
          }),
        )
        if ('error' in result) {
          return noStoreJsonResponse(result, { status: 502 })
        }
        let forumPost: { postRef?: string; error?: string } | null = null
        if (body.forumPost === true) {
          // In-process delivery through the shipped Artanis publication
          // queue (never fetch-to-self): the mind's decision lands as an
          // Artanis status post in forum.public.artanis.
          const nowIso = currentIsoTimestamp()
          const suffix = nowIso.replace(/[-:]/g, '').slice(0, 13)
          const intent = new ArtanisForumPublicationIntentRecord({
            ...exampleArtanisForumPublicationQueue().intents[0]!,
            artifactRefs: ['artifact.public.artanis.mind_smoke'],
            bodyText: [
              'Automated update from the Artanis cloud mind running inside the OpenAgents worker.',
              `Inference served via ${result.servedVia}${result.gatewayId === null ? '' : ` (gateway ${result.gatewayId})`}, model ${result.model}.`,
              `Decision sample: ${result.text.slice(0, 400)}`,
              'Boundary: the mind proposes; typed schemas validate; approval gates hold.',
            ].join(' '),
            createdAtIso: nowIso,
            deliveredAtIso: null,
            deliveryReceiptRefs: [],
            deliveryState: 'ready' as const,
            goalRefs: ['goal.public.artanis.cloud_mind_smoke'],
            idempotencyKey: `artanis-forum:mind-smoke:${suffix}:v1`,
            intentRef: `forum.public.artanis.mind_smoke_intent.${suffix}`,
            postRef: null,
            receiptRefs: ['receipt.public.artanis.mind_smoke'],
            updatedAtIso: nowIso,
          })
          forumPost = yield* saveArtanisForumPublicationIntent(
            openAgentsDatabase(env),
            intent,
            nowIso,
          ).pipe(
            Effect.flatMap(() =>
              deliverArtanisForumPublicationIntent(
                openAgentsDatabase(env),
                intent,
              ),
            ),
            Effect.map((post): { postRef?: string; error?: string } => ({
              postRef: post.postRef,
            })),
            Effect.catch(error =>
              Effect.succeed({
                error: `forum_delivery_failed: ${String(
                  (error as { reason?: string }).reason ?? error,
                )}`.slice(0, 160),
              }),
            ),
          )
        }
        return noStoreJsonResponse({
          forumPost,
          gatewayId: result.gatewayId,
          model: result.model,
          promptChars: result.promptChars,
          responseChars: result.responseChars,
          servedVia: result.servedVia,
          text: result.text.slice(0, 600),
        })
      }),
  },
  {
    path: '/api/operator/tassadar/replay',
    handler: (request, env) =>
      Effect.promise(async () => {
        if (request.method !== 'POST') {
          return noStoreJsonResponse(
            { error: 'method_not_allowed' },
            { status: 405 },
          )
        }
        if (!(await requireAdminApiToken(request, env))) {
          return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
        }
        try {
          const body = S.decodeUnknownSync(TassadarReplayRequest)(
            await request.json(),
          )
          const verdict = await runTassadarReplayValidation(body)
          return noStoreJsonResponse({ verdict })
        } catch (error) {
          return noStoreJsonResponse(
            {
              error: 'bad_request',
              reason: error instanceof Error ? error.message : String(error),
            },
            { status: 400 },
          )
        }
      }),
  },
  {
    path: '/api/operator/buy-mode',
    handler: (request, env) =>
      operatorBuyModeRoutes.handleOperatorBuyModeStatusApi(request, env),
  },
  {
    path: '/api/operator/buy-mode/start',
    handler: (request, env) =>
      operatorBuyModeRoutes.handleOperatorBuyModeStartApi(request, env),
  },
  {
    path: '/api/operator/buy-mode/stop',
    handler: (request, env) =>
      operatorBuyModeRoutes.handleOperatorBuyModeStopApi(request, env),
  },
  {
    path: '/api/operator/buy-mode/dispatch',
    handler: (request, env) =>
      operatorBuyModeRoutes.handleOperatorBuyModeDispatchApi(request, env),
  },
  {
    path: '/api/operator/buy-mode/eval',
    handler: (request, env) =>
      operatorBuyModeRoutes.handleOperatorBuyModeEvalApi(request, env),
  },
  {
    path: '/api/operator/buy-mode/results/settle',
    handler: (request, env) =>
      operatorBuyModeRoutes.handleOperatorBuyModeSettleApi(request, env),
  },
  {
    path: '/chat',
    handler: () => Effect.succeed(notFound()),
  },
  {
    path: '/discord',
    handler: () =>
      Effect.succeed(redirectResponse('https://discord.gg/4RrjGCuQAZ')),
  },
  // NOTE: `/login` is intentionally NOT an exact route. It is a served document
  // route (see `knownDocumentPathPatterns` in worker-routes.ts) so the SPA renders
  // the real login page (`apps/web/src/page/login.ts`). A prior exact-route entry
  // here hard-redirected `/login` -> `/`, which dead-ended sign-in. The query-
  // string cleanup for post-OAuth `/login?...` still lives in
  // `cleanProductRouteRedirectLocation`.
  {
    path: '/login/email',
    handler: (request, env) =>
      Effect.promise(() => handleEmailStart(request, env)),
  },
  {
    path: '/login/github',
    handler: (request, env) =>
      Effect.promise(() => handleGitHubStart(request, env)),
  },
  {
    path: '/auth/github/write/start',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleGitHubWriteStart(request, env, ctx)),
  },
  {
    path: '/auth/callback',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleAuthCallback(request, env, ctx)),
  },
  {
    path: '/auth/logout',
    handler: request => Effect.succeed(handleLogout(request)),
  },
  {
    path: '/logout',
    handler: request => Effect.succeed(handleLogout(request)),
  },
  {
    path: '/api/auth/session',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleSessionApi(request, env, ctx)),
  },
  {
    path: '/api/auth/totals',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleAuthTotalsApi(request, env, ctx)),
  },
  {
    path: '/api/mdk',
    handler: (request, env) => routeMdkSidecarRequest(request, env),
  },
  {
    path: '/api/admin/overview',
    handler: (request, env, ctx) =>
      adminOverviewHandlers.handleAdminOverviewApi(request, env, ctx),
  },
  {
    // Admin-gated Cloudflare Browser Rendering smoke (#6205). Proves the real
    // `env.BROWSER` binding from a deployed Worker; honest `{ ok:false, reason }`
    // if the binding is absent or Browser Rendering errors.
    path: '/api/admin/cf-browser-smoke',
    handler: (request, env, ctx) => handleCfBrowserSmokeApi(request, env, ctx),
  },
  {
    path: '/api/stats/token-usage/events',
    handler: (request, env) =>
      tokenUsageLedgerRoutes.handleTokenUsageEventsApi(request, env),
  },
  {
    path: '/api/stats/token-usage/aggregate',
    handler: (request, env, ctx) =>
      tokenUsageLedgerRoutes.handleTokenUsageAggregateApi(request, env, ctx),
  },
  {
    // OWNER-GATED inference cost / provider-lane analytics (#6232). Aggregate
    // token + cost rollups (byProvider, byModel, byRoute, byDay, totals) over a
    // window. Admin/owner session only — provider ids + cost are internal.
    path: '/api/admin/inference-analytics',
    handler: (request, env, ctx) =>
      tokenUsageLedgerRoutes.handleInferenceAnalyticsApi(request, env, ctx),
  },
  {
    path: '/api/stats/token-usage/leaderboards',
    handler: (request, env, ctx) =>
      tokenUsageLedgerRoutes.handleTokenUsageLeaderboardsApi(request, env, ctx),
  },
  {
    path: '/api/stats/token-usage/leaderboard-preference',
    handler: (request, env, ctx) =>
      tokenUsageLedgerRoutes.handleTokenUsageLeaderboardPreferenceApi(
        request,
        env,
        ctx,
      ),
  },
  {
    path: '/api/auth/teams',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleAuthTeamsApi(request, env, ctx)),
  },
  {
    path: '/api/autopilot/continuation-policy',
    handler: (request, env, ctx) =>
      autopilotContinuationPolicyRoutes.routeAutopilotContinuationPolicyRequest(
        request,
        env,
        ctx,
      ),
  },
  {
    path: '/api/autopilot/morning-report',
    handler: (request, env, ctx) =>
      autopilotMorningReportRoutes.routeAutopilotMorningReportRequest(
        request,
        env,
        ctx,
      ),
  },
  {
    path: '/api/public/pylon-stats',
    handler: (request, env) => handlePublicPylonStatsApi(request, env),
  },
  {
    path: '/api/public/khala-tokens-served',
    handler: (request, env) => handlePublicKhalaTokensServedApi(request, env),
  },
  {
    path: '/api/public/khala-tokens-served/history',
    handler: (request, env) =>
      handlePublicKhalaTokensServedHistoryApi(request, env),
  },
  {
    path: '/api/public/khala-tokens-served/model-mix',
    handler: (request, env) =>
      handlePublicKhalaTokensServedModelMixApi(request, env),
  },
  {
    path: PYLON_CODEX_TURN_INGEST_PATH,
    handler: (request, env) =>
      pylonCodexTurnIngestRoutes.handlePylonCodexTurnIngestApi(request, env),
  },
  {
    path: PYLON_CLAUDE_TURN_INGEST_PATH,
    handler: (request, env) =>
      pylonCodexTurnIngestRoutes.handlePylonClaudeTurnIngestApi(request, env),
  },
  {
    path: PYLON_CODEX_EVENT_CHUNK_INGEST_PATH,
    handler: (request, env) =>
      pylonCodexTurnIngestRoutes.handlePylonCodexEventChunkIngestApi(
        request,
        env,
      ),
  },
  {
    path: PYLON_CODEX_ASSIGNMENT_PROOF_PATH,
    handler: (request, env) =>
      pylonCodexTurnIngestRoutes.handlePylonCodexAssignmentProofApi(
        request,
        env,
      ),
  },
  {
    path: PYLON_CODEX_ASSIGNMENT_TRACE_STATUS_PATH,
    handler: (request, env) =>
      pylonCodexTurnIngestRoutes.handlePylonCodexAssignmentTraceStatusApi(
        request,
        env,
      ),
  },
  {
    path: '/api/public/pylon-capacity-funnel',
    handler: (request, env) => handlePylonCapacityFunnelApi(request, env),
  },
  {
    path: '/api/public/pylon-capacity-funnel/history',
    handler: (request, env) =>
      handlePylonCapacityFunnelHistoryApi(request, env),
  },
  {
    path: '/api/public/site-referral-payouts',
    handler: (request, env) => handleSiteReferralPayoutsPublicApi(request, env),
  },
  {
    path: '/api/public/partner-payouts',
    handler: (request, env) => handlePartnerPayoutsPublicApi(request, env),
  },
  {
    path: '/api/public/relay-health',
    handler: (request, env) =>
      handlePublicRelayHealthApi(request, {
        relayUrl: canonicalMarketRelayUrl(env),
        store: makeD1RelayHealthStore(openAgentsDatabase(env)),
      }),
  },
  {
    path: '/api/public/launch-dashboard',
    handler: (request, env) => handlePublicLaunchDashboardApi(request, env),
  },
  {
    path: '/api/public/treasury/launch-status',
    handler: (request, env) =>
      handlePublicTreasuryLaunchStatusApi(request, {
        fetchTreasury: fetchMdkTreasuryPath(env),
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
      }),
  },
  {
    path: '/api/operator/treasury/status',
    handler: (request, env) =>
      handleOperatorTreasuryStatusApi(request, {
        fetchTreasury: fetchMdkTreasuryPath(env),
        readRewardDispatchStats: () => {
          const nowIso = currentIsoTimestamp()
          const dispatchConfig = readXClaimRewardTreasuryDispatchConfig(
            env,
            nowIso,
          )

          return makeD1XClaimRewardTreasuryDispatchStore(
            openAgentsDatabase(env),
          ).readDispatchStats(
            xClaimRewardDispatchDayStartIso(nowIso),
            dispatchConfig,
          )
        },
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
      }),
  },
  {
    path: '/api/operator/treasury/funding-destination',
    handler: (request, env) =>
      handleOperatorTreasuryFundingDestinationApi(request, {
        fetchTreasury: fetchMdkTreasuryPath(env),
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
      }),
  },
  {
    path: '/api/operator/treasury/spark-funding-destination',
    handler: (request, env) =>
      handleOperatorSparkTreasuryFundingDestinationApi(request, {
        fetchSparkTreasury: fetchMdkTreasuryPath(env),
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
      }),
  },
  {
    path: '/api/operator/treasury/spark-funding-invoice',
    handler: (request, env) =>
      handleOperatorSparkTreasuryFundingInvoiceApi(request, {
        fetchSparkTreasury: fetchMdkTreasuryPath(env),
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
      }),
  },
  {
    path: '/api/operator/treasury/transactions/reconcile',
    handler: (request, env) =>
      handleOperatorTreasuryTransactionReconcileApi(request, {
        fetchTipsBuffer: fetchMdkTipsBufferPath(env),
        fetchTreasury: fetchMdkTreasuryPath(env),
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        transactionStore: makeD1TreasuryTransactionStore(
          openAgentsDatabase(env),
        ),
      }),
  },
  {
    path: '/api/operator/treasury/recipient-report',
    handler: (request, env) =>
      handleOperatorTreasuryRecipientReportApi(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        transactionStore: makeD1TreasuryTransactionStore(
          openAgentsDatabase(env),
        ),
      }),
  },
  {
    path: '/api/operator/treasury/recipient-confirmations',
    handler: (request, env) =>
      handleOperatorTreasuryRecipientConfirmationApi(request, {
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        transactionStore: makeD1TreasuryTransactionStore(
          openAgentsDatabase(env),
        ),
      }),
  },
  {
    path: '/api/operator/tips-buffer/status',
    handler: (request, env) =>
      handleOperatorTreasuryStatusApi(request, {
        fetchTreasury: fetchMdkTipsBufferPath(env),
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        serviceLabel: 'mdk_tips_buffer',
      }),
  },
  {
    path: '/api/operator/tips-buffer/funding-destination',
    handler: (request, env) =>
      handleOperatorTreasuryFundingDestinationApi(request, {
        fetchTreasury: fetchMdkTipsBufferPath(env),
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
        serviceLabel: 'mdk_tips_buffer',
      }),
  },
  {
    path: '/api/operator/artanis/spend-decision',
    handler: (request, env) =>
      Effect.gen(function* () {
        if (request.method !== 'POST') {
          return noStoreJsonResponse(
            { error: 'method_not_allowed' },
            { status: 405 },
          )
        }
        const authorized = yield* Effect.promise(() =>
          requireAdminApiToken(request, env),
        )
        if (!authorized) {
          return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
        }
        const body = yield* Effect.promise(async () => {
          try {
            return (await request.json()) as {
              recipientActorRef?: string
              context?: string
              suggestedMaxSat?: number
            }
          } catch {
            return {}
          }
        })
        if (
          typeof body.recipientActorRef !== 'string' ||
          typeof body.context !== 'string' ||
          typeof body.suggestedMaxSat !== 'number'
        ) {
          return noStoreJsonResponse({ error: 'bad_request' }, { status: 400 })
        }
        // Registered destinations only: the recipient's tip-recipient
        // wallet claim is the public-safe source of the offer.
        const wallet = yield* Effect.promise(
          async () =>
            (await openAgentsDatabase(env)
              .prepare(
                `SELECT wallet_ref, bolt12_offer, lightning_address FROM forum_tip_recipient_wallets
               WHERE actor_ref = ? AND state = 'ready' AND archived_at IS NULL
                 AND (lightning_address IS NOT NULL OR bolt12_offer IS NOT NULL)`,
              )
              .bind(body.recipientActorRef)
              .first()) as {
              wallet_ref: string
              bolt12_offer: string | null
              lightning_address: string | null
            } | null,
        )
        if (wallet === null) {
          return noStoreJsonResponse(
            { error: 'recipient_destination_not_registered' },
            { status: 409 },
          )
        }
        const outcome = yield* Effect.promise(() =>
          runArtanisSpendDecision(openAgentsDatabase(env), {
            candidate: {
              destination: wallet.lightning_address ?? wallet.bolt12_offer!,
              context: body.context!,
              destinationSourceRef: wallet.wallet_ref,
              recipientRef: body.recipientActorRef!,
              suggestedMaxSat: Math.floor(body.suggestedMaxSat!),
            },
            gatewayToken: (env as { CF_AIG_TOKEN?: string }).CF_AIG_TOKEN,
            geminiApiKey:
              (env as { GEMINI_API_KEY?: string }).GEMINI_API_KEY ?? null,
            nowIso: currentIsoTimestamp(),
            treasury: {
              fetchSparkTreasury: fetchMdkTreasuryPath(env),
              fetchTreasury: fetchMdkTreasuryPath(env),
              recordPayoutTransaction: async input => {
                await makeD1TreasuryTransactionStore(
                  openAgentsDatabase(env),
                ).insert({
                  amountSat: input.amountSat,
                  bolt11: null,
                  createdAt: currentIsoTimestamp(),
                  direction: 'out',
                  expiresAt: null,
                  failureReasonRef: input.failureReasonRef ?? null,
                  id: randomUuid(),
                  owedRef: input.owedRef ?? null,
                  owedSat: input.owedSat ?? null,
                  paymentRef: input.paymentRef,
                  recipientConfirmationRef: null,
                  recipientConfirmationState: 'unconfirmed',
                  recipientConfirmedAt: null,
                  recipientRef: input.recipientRef ?? null,
                  redactedDestinationRef: input.redactedDestinationRef ?? null,
                  settledAt: input.settled ? currentIsoTimestamp() : null,
                  state:
                    input.failureReasonRef !== undefined &&
                    input.failureReasonRef !== null
                      ? 'failed'
                      : input.settled
                        ? 'settled'
                        : 'pending',
                })
              },
              requireAdminApiToken: async () => true,
            },
          }),
        )
        return noStoreJsonResponse({ outcome })
      }),
  },
  {
    path: '/api/operator/treasury/payout',
    handler: (request, env) =>
      handleOperatorTreasuryPayoutApi(request, {
        fetchSparkTreasury: fetchMdkTreasuryPath(env),
        fetchTreasury: fetchMdkTreasuryPath(env),
        recordPayoutTransaction: async input => {
          await makeD1TreasuryTransactionStore(openAgentsDatabase(env)).insert({
            amountSat: input.amountSat,
            bolt11: null,
            createdAt: currentIsoTimestamp(),
            direction: 'out',
            expiresAt: null,
            failureReasonRef: input.failureReasonRef ?? null,
            id: `treasury_payout_${randomUuid()}`,
            owedRef: input.owedRef ?? null,
            owedSat: input.owedSat ?? null,
            paymentRef: input.paymentRef,
            recipientConfirmationRef: null,
            recipientConfirmationState: 'unconfirmed',
            recipientConfirmedAt: null,
            recipientRef: input.recipientRef ?? null,
            redactedDestinationRef: input.redactedDestinationRef ?? null,
            settledAt: input.settled ? currentIsoTimestamp() : null,
            state:
              input.failureReasonRef !== undefined &&
              input.failureReasonRef !== null
                ? 'failed'
                : input.settled
                  ? 'settled'
                  : 'pending',
          })
        },
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
      }),
  },
  {
    // Operator payout from the tips-buffer wallet (mirrors the treasury payout
    // against the tips-buffer MDK wallet). Used to pay BOLT12 offers from the
    // buffer directly — e.g. operator-directed recognition rewards — without the
    // tip-ladder's per-sender ledger-balance requirement.
    path: '/api/operator/tips-buffer/payout',
    handler: (request, env) =>
      handleOperatorTreasuryPayoutApi(request, {
        fetchTreasury: fetchMdkTipsBufferPath(env),
        recordPayoutTransaction: async input => {
          await makeD1TreasuryTransactionStore(openAgentsDatabase(env)).insert({
            amountSat: input.amountSat,
            bolt11: null,
            createdAt: currentIsoTimestamp(),
            direction: 'out',
            expiresAt: null,
            failureReasonRef: input.failureReasonRef ?? null,
            id: `tips_buffer_payout_${randomUuid()}`,
            owedRef: input.owedRef ?? null,
            owedSat: input.owedSat ?? null,
            paymentRef: input.paymentRef,
            recipientConfirmationRef: null,
            recipientConfirmationState: 'unconfirmed',
            recipientConfirmedAt: null,
            recipientRef: input.recipientRef ?? null,
            redactedDestinationRef: input.redactedDestinationRef ?? null,
            settledAt: input.settled ? currentIsoTimestamp() : null,
            state:
              input.failureReasonRef !== undefined &&
              input.failureReasonRef !== null
                ? 'failed'
                : input.settled
                  ? 'settled'
                  : 'pending',
          })
        },
        requireAdminApiToken: adminRequest =>
          requireAdminApiToken(adminRequest, env),
      }),
  },
  {
    path: '/api/public/artanis/report',
    handler: (request, env) => handlePublicArtanisReportApi(request, env),
  },
  {
    path: '/api/public/labor-earnings',
    handler: (request, env) =>
      handlePublicLaborEarningsApi(request, {
        db: openAgentsDatabase(env),
      }),
  },
  {
    path: '/api/public/labor-earnings/payout',
    handler: (request, env) =>
      handleSelfServeLaborPayoutApi(request, {
        db: openAgentsDatabase(env),
        authenticate: agentBalanceAuthForStore(
          makeD1AgentRegistrationStore(openAgentsDatabase(env)),
        ),
        // INERT flag: defaults to false so it plans but lists nothing.
        enabled: env.LABOR_SELF_SERVE_PAYOUT_ENABLED === 'true',
      }),
  },
  {
    path: '/api/public/artanis/labor-receipts',
    handler: (request, env) =>
      handlePublicArtanisLaborReceiptsApi(request, {
        nowIso: currentIsoTimestamp,
        store: makeD1ArtanisLaborUnattendedReceiptStore(
          openAgentsDatabase(env),
          currentIsoTimestamp,
        ),
      }),
  },
  {
    path: '/api/public/artanis/labor-green-readiness',
    handler: (request, env) =>
      handlePublicArtanisLaborGreenReadinessApi(request, {
        nowIso: currentIsoTimestamp,
        store: makeD1ArtanisLaborUnattendedReceiptStore(
          openAgentsDatabase(env),
          currentIsoTimestamp,
        ),
      }),
  },
  {
    path: '/api/public/artanis/admin-ticks',
    handler: (request, env) =>
      Effect.promise(async () => {
        if (request.method !== 'GET') {
          return Response.json({ error: 'method_not_allowed' }, { status: 405 })
        }
        const monitor = await readArtanisTickMonitor(openAgentsDatabase(env), {
          limit: boundedTickMonitorLimit(
            new URL(request.url).searchParams.get('limit'),
          ),
          nowIso: currentIsoTimestamp(),
        })
        return Response.json(monitor, {
          headers: { 'cache-control': 'no-store' },
        })
      }),
  },
  {
    path: '/api/public/artanis/tick-streak',
    handler: (request, env) =>
      Effect.promise(async () => {
        if (request.method !== 'GET') {
          return Response.json({ error: 'method_not_allowed' }, { status: 405 })
        }
        const streak = await readArtanisTickStreak(openAgentsDatabase(env), {
          limit: boundedTickStreakLimit(
            new URL(request.url).searchParams.get('limit'),
          ),
          nowIso: currentIsoTimestamp(),
        })
        return Response.json(streak, {
          headers: { 'cache-control': 'no-store' },
        })
      }),
  },
  {
    path: '/api/public/artanis/tassadar-distillation-dataset',
    handler: (request, env) =>
      Effect.promise(async () => {
        if (request.method !== 'GET') {
          return Response.json({ error: 'method_not_allowed' }, { status: 405 })
        }
        const receipt = await readArtanisDistillationDatasetReceipt(
          openAgentsDatabase(env),
          {
            limit: boundedDistillationDatasetLimit(
              new URL(request.url).searchParams.get('limit'),
            ),
            nowIso: currentIsoTimestamp(),
          },
        )
        return Response.json(receipt, {
          headers: { 'cache-control': 'no-store' },
        })
      }),
  },
  {
    path: '/api/public/artanis/responder-support',
    handler: (request, env) =>
      Effect.promise(async () => {
        if (request.method !== 'GET') {
          return Response.json({ error: 'method_not_allowed' }, { status: 405 })
        }
        const projection = await readArtanisResponderSupport(
          openAgentsDatabase(env),
          {
            limit: boundedResponderSupportLimit(
              new URL(request.url).searchParams.get('limit'),
            ),
            nowIso: currentIsoTimestamp(),
          },
        )
        return Response.json(projection, {
          headers: { 'cache-control': 'no-store' },
        })
      }),
  },
  {
    path: '/api/blueprint/program-registry',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintProgramRegistryApi(request, env),
  },
  {
    path: '/api/blueprint/program-runs',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintProgramRunEvidenceApi(request, env),
  },
  {
    path: '/api/blueprint/action-submissions',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintActionSubmissionsApi(request, env),
  },
  {
    path: '/api/blueprint/contributions',
    handler: (request, env) =>
      blueprintProbeContributionRoutes.handleBlueprintProbeContributionsApi(
        request,
        env,
      ),
  },
  {
    path: '/api/blueprint/contracts',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintContractExportApi(request, env),
  },
  {
    path: '/api/blueprint/tassadar-modules',
    handler: (request, env) =>
      blueprintRoutes.handleBlueprintTassadarModuleRegistryApi(request, env),
  },
  {
    path: '/.well-known/openagents.json',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'capability_manifest_read',
        '/.well-known/openagents.json',
      )

      return handleOpenAgentsCapabilityManifestApi(request)
    },
  },
  {
    path: '/AGENTS-CORE.md',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'agent_doc_read',
        '/AGENTS-CORE.md',
      )

      return handleOpenAgentsCompanionFile(
        request,
        env.ASSETS,
        '/AGENTS-CORE.md',
      )
    },
  },
  {
    path: '/AGENTS.md',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'agent_doc_read',
        '/AGENTS.md',
      )

      return handleOpenAgentsAgentOnboarding(request, env.ASSETS)
    },
  },
  {
    path: '/HEARTBEAT.md',
    handler: (request, env) =>
      handleOpenAgentsCompanionFile(request, env.ASSETS, '/HEARTBEAT.md'),
  },
  {
    path: '/RULES.md',
    handler: (request, env) =>
      handleOpenAgentsCompanionFile(request, env.ASSETS, '/RULES.md'),
  },
  {
    path: '/skill.json',
    handler: (request, env) =>
      handleOpenAgentsCompanionFile(request, env.ASSETS, '/skill.json'),
  },
  // Agent-discovery surfaces for Khala + the OpenAgents Agent Cloud (EPIC #6049,
  // Phase 1). Ship-ready, UNCONDITIONAL (no flag): plain-language, machine-
  // readable docs that describe the live Khala inference API so agents — and the
  // Stripe Directory crawler (StripeBot) — can find and understand it. They make
  // no money claim and require no payment config; they only describe the API and
  // forward-reference the (flagged) MPP endpoint. Crawlable (public, cacheable,
  // no auth, no robots block). Mirrors the live PostalForm directory shape.
  ...(['/llms.txt', '/agents.md', '/ai.md', '/skill.md'] as const).map(
    surfacePath => ({
      handler: (request: Request) =>
        renderDiscoverySurface(request, surfacePath as DiscoverySurfacePath),
      path: surfacePath,
    }),
  ),
  {
    // MPP service-discovery document (EPIC #6049). The Machine Payments Protocol
    // registries (MPPScan, mpp.dev/services) crawl `GET /openapi.json` to light
    // the Machine Payments badge for Khala. OpenAPI 3.1 with `x-service-info`
    // (root) + per-operation `x-payment-info` (canonical multi-offer form).
    //
    // HONESTY GATE: the paid `/mpp/v1/chat/completions` path's offers + 402 are
    // emitted ONLY when the MPP endpoint is actually armed — the SAME
    // KHALA_MPP_ENABLED flag the route reads from config.ts (and the card offer
    // only when STRIPE_MPP_NETWORK_PROFILE_ID is set, the same condition that
    // arms the card rail). Inert => the document omits the paid path and
    // advertises nothing payable. The document itself is always served (free
    // description + x-service-info) so registries can still discover the service.
    path: '/openapi.json',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'openapi_read',
        '/openapi.json',
      )

      return renderMppDiscoveryDocument(request, {
        cardRailEnabled:
          env.STRIPE_MPP_NETWORK_PROFILE_ID !== undefined &&
          env.STRIPE_MPP_NETWORK_PROFILE_ID.trim() !== '',
        // Bitcoin-first Lightning offer: armed flag AND a working Lightning
        // invoice issuer present (Spark primary, MDK fallback — the SAME
        // condition that arms the rail in the route). Honesty gate: omitted when
        // we cannot actually mint an invoice.
        lightningRailEnabled:
          isKhalaMppLightningEnabled(env.KHALA_MPP_LIGHTNING_ENABLED) &&
          lightningInvoiceIssuerForEnv(env) !== undefined,
        mppEnabled: isKhalaMppEnabled(env.KHALA_MPP_ENABLED),
      })
    },
  },
  {
    path: '/api/openapi.json',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'openapi_read',
        '/api/openapi.json',
      )

      return handleOpenAgentsOpenApi(request)
    },
  },
  {
    path: '/api/omni/sdk-seed',
    handler: request => handleOmniApiSdkSeedApi(request),
  },
  {
    path: '/api/developer/signature-packages/validate',
    handler: request => handleSignaturePackageValidationApi(request),
  },
  {
    path: '/api/public/adjutant/activity',
    handler: (request, env) => handlePublicAdjutantActivityApi(request, env),
  },
  {
    path: '/api/public/proof/otec',
    handler: (request, env, ctx) => {
      recordPublicAgentFunnelRead(
        request,
        openAgentsDatabase(env),
        ctx,
        'public_proof_read',
        '/api/public/proof/otec',
      )

      return handlePublicOtecProofApi(request, env)
    },
  },
  {
    path: '/api/github-write/connections',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleGitHubWriteConnectionsApi(request, env, ctx)),
  },
  {
    path: '/api/github-write/grants/resolve',
    handler: (request, env) =>
      Effect.promise(() => handleGitHubWriteGrantResolveApi(request, env)),
  },
  {
    path: '/api/admin/sync/notify',
    handler: (request, env) =>
      Effect.promise(() => handleAdminSyncNotifyApi(request, env)),
  },
  {
    path: '/api/agents/register',
    handler: (request, env) =>
      Effect.promise(() => handleProgrammaticAgentRegistration(request, env)),
  },
  {
    // #6370: admin-only dead-token recovery — mint a fresh credential for an
    // EXISTING agent identity (same entity), admin-gated like every /api/admin/*
    // route.
    path: '/api/admin/agents/reissue-token',
    handler: (request, env, ctx) =>
      Effect.promise(() => handleAdminReissueAgentToken(request, env, ctx)),
  },
  {
    // Khala FREE API MODE self-serve mint (issue #6228). Mints a rate-limited
    // free `oa_agent_` key for the free `openagents/khala` lane. INERT (404)
    // until INFERENCE_FREE_TIER_ENABLED is on.
    path: '/api/keys/free',
    handler: (request, env) =>
      Effect.promise(() => handleFreeKeyMint(request, env)),
  },
  {
    path: '/api/agents/me',
    handler: (request, env) =>
      Effect.promise(() => handleProgrammaticAgentMe(request, env)),
  },
  {
    path: '/api/agents/me/balance',
    handler: (request, env) =>
      handleAgentBalanceApi(request, {
        authenticate: agentBalanceAuthForStore(
          makeD1AgentRegistrationStore(openAgentsDatabase(env)),
        ),
        db: openAgentsDatabase(env),
      }),
  },
  {
    path: '/api/agents/me/balance/preferences',
    handler: (request, env) =>
      handleAgentBalancePreferencesApi(request, {
        authenticate: agentBalanceAuthForStore(
          makeD1AgentRegistrationStore(openAgentsDatabase(env)),
        ),
        db: openAgentsDatabase(env),
      }),
  },
  {
    path: '/api/agents/home',
    handler: (request, env) =>
      Effect.promise(() =>
        handleProgrammaticAgentHome(request, openAgentsDatabase(env)),
      ),
  },
  {
    path: '/api/public/inference/batch-job-receipts/:receiptRef',
    handler: (request, env) =>
      handleBatchJobReceiptRead(request, {
        authenticate: async () => undefined,
        db: openAgentsDatabase(env),
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        nowIso: currentIsoTimestamp,
      }),
  },
  {
    path: '/v1/inference/batches',
    handler: (request, env) =>
      handleBatchJobsSubmit(request, {
        authenticate: async authRequest => {
          const token = readBearerToken(authRequest)
          if (token === undefined) {
            return undefined
          }
          const session = await authenticateProgrammaticAgent(
            makeD1AgentRegistrationStore(openAgentsDatabase(env)),
            token,
          )
          return session === undefined
            ? undefined
            : { accountRef: `agent:${session.user.id}` }
        },
        db: openAgentsDatabase(env),
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        // Inert producer by default: undefined unless the batch-jobs flag is on
        // AND the queue binding is provisioned (see makeBatchJobEnqueue).
        enqueueBatchJob: makeBatchJobEnqueue(env),
        nowIso: currentIsoTimestamp,
      }),
  },
  {
    path: '/v1/inference/batches/:jobId',
    handler: (request, env) =>
      handleBatchJobStatusRead(request, {
        authenticate: async authRequest => {
          const token = readBearerToken(authRequest)
          if (token === undefined) {
            return undefined
          }
          const session = await authenticateProgrammaticAgent(
            makeD1AgentRegistrationStore(openAgentsDatabase(env)),
            token,
          )
          return session === undefined
            ? undefined
            : { accountRef: `agent:${session.user.id}` }
        },
        db: openAgentsDatabase(env),
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        nowIso: currentIsoTimestamp,
      }),
  },
  {
    // Inference gateway (EPIC #5474, #5476). INERT by default: gated behind
    // INFERENCE_GATEWAY_ENABLED (default off). Ships wired to the stub/echo
    // adapter + no-op metering stub; Phase-2 issues register real adapters
    // (#5479/#5480/#5481), routing (#5482), and live metering/credits (#5477).
    path: '/v1/chat/completions',
    handler: (request, env) => {
      registerPassthroughAdapters(inferenceProviderRegistry, env)
      registerHydraliskAdapter(inferenceProviderRegistry, env)
      registerOpenRouterAdapter(inferenceProviderRegistry, env)
      // Serving-fabric lane (#5483 / Khala M4 #6012). No-op today (no live
      // Psionic serve transport bound); keeps the lane honestly skipped until a
      // transport lands, with no routing change required to activate it.
      registerFabricServeAdapter(inferenceProviderRegistry, env)
      // Capture the live env so env-dependent adapters (Vertex #5480) can mint
      // credentials from Worker secrets at call time. INERT regardless: the
      // gateway is gated by INFERENCE_GATEWAY_ENABLED below.
      setInferenceAdapterEnv(env)
      // Free-tier identity resolver (EPIC #5474 §1/§2): maps an authenticated
      // account ref to its VERIFIED owner-claim identity (the SAME surface the
      // #5486 light-KYC gate reads), so the Sybil-resistant free pool and the
      // premium allowlist both key on one owner across all of that owner's
      // accounts. INERT regardless — only reached when the gateway is enabled.
      const ownerClaimStore = makeD1AgentOwnerClaimStore(
        openAgentsDatabase(env),
      )
      const resolveOwnerIdentity = makeVerifiedOwnerIdentityResolver(
        ownerClaimStore.readVerifiedPublicIdentityForAgentUserId,
      )
      // OWNER BALANCE-GATE EXEMPTION (issue #6180). Armed ONLY when
      // INFERENCE_OPERATOR_EXEMPTION_ENABLED is on (fail-closed, default OFF). When
      // armed, an EXEMPT verified owner (the `inference_operator_exemption` store)
      // may call our OWN non-premium lanes (e.g. `openagents/khala`) with a zero
      // balance: the balance-gate seam admits it and `withOperatorCredit` records
      // it as `operator_credit` (zero debit + receipt, no referral). A PREMIUM
      // model is NEVER exempt; Khala stays paid for the public (non-exempt keys
      // still 402). Inert with the flag off — both seams are simply not wired.
      const operatorExemptionEnabled = isOperatorExemptionEnabled(
        env.INFERENCE_OPERATOR_EXEMPTION_ENABLED,
      )
      // KHALA FREE API MODE (issue #6228). Armed ONLY when
      // INFERENCE_FREE_TIER_ENABLED is on (fail-closed, default OFF). When armed,
      // a self-serve FREE-TIER key (minted at POST /api/keys/free, marked in
      // `inference_free_tier_keys`) may call the single public model
      // `openagents/khala` (own-infra GPT-OSS / Gemini Flash) with a zero balance
      // WITHIN its per-key daily quota: the balance-gate seam admits it and
      // `withFreeTierKhala` records it as a zero-debit free receipt + accrues the
      // quota. A PREMIUM model is NEVER free; over-quota / non-free-tier keys
      // still 402, so paid Khala behavior for funded keys is unchanged. Inert with
      // the flag off — both seams are simply not wired.
      const freeTierEnabled = isFreeTierEnabled(env.INFERENCE_FREE_TIER_ENABLED)
      const freeTierQuota = resolveFreeTierQuota(env)
      // INTERNAL/OPS ACCOUNT ALLOWLIST (#6232 / #6298). Parsed ONCE here so the
      // demand-attribution rule, the free-tier balance-gate quota exemption, and
      // the free-tier metering-wrapper quota exemption all read the SAME set.
      // Internal testing accounts on this allowlist are quota-EXEMPT on the free
      // Khala lane (never hit the per-key daily quota -> never 402 on quota
      // grounds); external free keys keep the unchanged daily limit. Empty
      // (unset/blank) => pure no-op.
      const internalAccountRefs = parseInternalAccountRefs(
        env.INFERENCE_INTERNAL_ACCOUNT_REFS,
      )
      const laneArming = resolveSupplyLaneArming(env)
      const routeAdmission = hydraliskGlm52RouteAdmissionForEnv(env)
      const internalStressCoordinator =
        env.GLM_STRESS_SCHEDULER === undefined
          ? undefined
          : makeInternalStressPreemptionCoordinatorDO(
              env.GLM_STRESS_SCHEDULER as unknown as InternalStressSchedulerNamespace,
            )
      return handleChatCompletions(request, {
        authenticate: async authRequest => {
          const token = readBearerToken(authRequest)
          if (token === undefined) {
            return undefined
          }
          const session = await authenticateProgrammaticAgent(
            makeD1AgentRegistrationStore(openAgentsDatabase(env)),
            token,
          )
          return session === undefined
            ? undefined
            : { accountRef: `agent:${session.user.id}` }
        },
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        // Live credit metering (#5477): decrement the account's balance from
        // real provider usage through the existing PayIn-shaped credit ledger.
        // The route never reaches the hook on the inert (flag-off) path, so this
        // is safe to wire unconditionally; it only fires when the gateway is on
        // AND a real adapter served a completion.
        //
        // Referral accrual on ALL inference (#5488): wrap the live ledger hook so
        // that AFTER a real, non-zero charge settles, the referrer's ongoing cut
        // (the referrer share of the three-way margin split, #5489) is recorded
        // into the existing RL-1 referral payout ledger — ongoing/indefinite, one
        // accrual per paid request, idempotent, and never failing the inference
        // call. INERT when no real charge occurred (stub hook / zero charge / flag
        // off) and a no-op when the account was not referred.
        //
        // Free-allowance gate (EPIC #5474 §1): the OUTERMOST wrapper. For a
        // free-eligible model (Gemini Flash) whose priced charge fits under the
        // resolving owner's Sybil-resistant free pool ($10/verified owner, ~$0.50
        // taste/unclaimed account, + earned allowance), it EATS the cost and
        // short-circuits BEFORE the referral+ledger inner hook — no credit
        // decrement, no referral accrual. Over allowance (or any non-free model)
        // it falls through to the normal referral+ledger path. INERT on the
        // flag-off path (the route never reaches the hook) and idempotent per
        // request.
        // OWNER OPERATOR-CREDIT WRAPPER (issue #6180) is the OUTERMOST metering
        // wrapper, ONLY when armed. For an EXEMPT verified owner on a non-premium
        // model it records `operator_credit` (zero debit, receipt, no referral)
        // and short-circuits BEFORE the free-allowance/referral/ledger inner
        // hooks. Premium served models + non-exempt owners always fall through to
        // the normal path. Inert (identity) when the flag is off.
        //
        // KHALA FREE-TIER WRAPPER (issue #6228), ONLY when armed. For a FREE-TIER
        // key on the free Khala lane within its daily quota it records a zero-
        // debit free receipt + accrues the quota and short-circuits BEFORE the
        // referral/ledger inner hooks. Premium served models, non-free-tier keys,
        // non-Khala models, and over-quota requests fall through to the normal
        // path. Inert (identity) when the flag is off. Each decorator acts only on
        // its own lane, so the order among the free wrappers is independent.
        meteringHook: (baseHook =>
          operatorExemptionEnabled
            ? withOperatorCredit(baseHook, {
                db: openAgentsDatabase(env),
                resolveOwnerIdentity,
              })
            : baseHook)(
          withFreeAllowance(
            (innerHook =>
              freeTierEnabled
                ? withFreeTierKhala(innerHook, {
                    db: openAgentsDatabase(env),
                    quota: freeTierQuota,
                    internalAccountRefs,
                  })
                : innerHook)(
              withReferralAccrual(
                makeLedgerMeteringHook({ db: openAgentsDatabase(env) }),
                { db: openAgentsDatabase(env) },
              ),
            ),
            { db: openAgentsDatabase(env), resolveOwnerIdentity },
          ),
        ),
        // SERVED-TOKENS COUNTER (issue #6227/#6358). Records one canonical
        // `token_usage_events` row per SERVED completion so the public "Khala
        // Tokens Served" counter (GET /api/public/khala-tokens-served) reflects
        // every real Khala served-token row — paid, free-tier, own-capacity,
        // internal_stress, and internal dogfood. Demand labels remain exact in
        // the ledger but do not leak through the aggregate public projection.
        // Idempotent per request; never fails the completion. INERT regardless —
        // only reached when the gateway is enabled.
        recordTokensServed: makeD1ServedTokensRecorder(
          openAgentsDatabase(env),
          {
            // Live-counter push (#6231): on a REAL new served-tokens row, push the
            // public-safe delta onto the tokens-served sync scope so the homepage
            // odometer rolls up instantly. Fail-soft: never breaks the completion.
            publishDelta: delta =>
              Effect.promise(() =>
                publishKhalaTokensServedDelta(
                  env,
                  buildKhalaTokensServedDelta(delta),
                ).catch(() => undefined),
              ),
          },
        ),
        // INTERNAL/OPS ACCOUNT DEMAND ALLOWLIST (#6298 follow-up). Parsed once
        // from the worker var; traffic from a listed account is auto-classified
        // `demand_kind=internal` (header-independent), keeping our own dogfood
        // out of the external trace corpus + demand ledger. Empty => no-op.
        internalAccountRefs,
        codingDelegation: {
          agentStore: makeD1AgentRegistrationStore(openAgentsDatabase(env)),
          pylonStore: makeD1PylonApiStore(openAgentsDatabase(env)),
          resolveOpenAuthUserId: async accountRef => {
            const agentUserId = accountRef.startsWith('agent:')
              ? accountRef.slice('agent:'.length)
              : undefined
            if (agentUserId === undefined || agentUserId === '') {
              return undefined
            }
            const row = await openAgentsDatabase(env)
              .prepare(
                `SELECT openauth_user_id
                   FROM agent_credentials
                  WHERE user_id = ?
                    AND openauth_user_id IS NOT NULL
                    AND status = 'active'
                    AND revoked_at IS NULL
                  ORDER BY last_used_at DESC
                  LIMIT 1`,
              )
              .bind(agentUserId)
              .first<{ openauth_user_id: string | null }>()
            if (
              row?.openauth_user_id !== null &&
              row?.openauth_user_id !== undefined
            ) {
              return row.openauth_user_id
            }
            const link = await openAgentsDatabase(env)
              .prepare(
                `SELECT openauth_user_id
                   FROM openauth_agent_links
                  WHERE agent_user_id = ?
                    AND status = 'active'
                    AND revoked_at IS NULL
                  ORDER BY updated_at DESC
                  LIMIT 1`,
              )
              .bind(agentUserId)
              .first<{ openauth_user_id: string | null }>()

            return link?.openauth_user_id ?? undefined
          },
        },
        readAvailableMsat: async accountRef => {
          const balance = await readAgentBalance(
            openAgentsDatabase(env),
            accountRef,
          )
          return balance === null ? 0 : balance.availableMsat
        },
        // FREE-ALLOWANCE PRE-FLIGHT (EPIC #5474 §1): read-only mirror of the
        // gate inside `withFreeAllowance` (wired just above as the metering
        // hook). It lets the balance gate admit a zero-balance account when the
        // (account, model) is free-eligible and the resolving owner still has
        // remaining free allowance, so a genuinely-free request (Gemini Flash
        // under the owner's Sybil-resistant pool) is reachable WITHOUT a funded
        // balance — the metering hook then eats and accrues the cost. Uses the
        // SAME owner-identity resolver as the metering hook so the bypass and
        // the accrual agree on the owner/pool.
        checkFreeAllowance: checkFreeAllowancePreflight({
          db: openAgentsDatabase(env),
          resolveOwnerIdentity,
        }),
        // Routing & supply selection (#5482): cheapest-viable lane plan per
        // model with bounded-backoff overflow to the next viable lane on a
        // retryable provider failure (429 / 503 / 5xx / transport). INERT
        // regardless — the gateway is gated by INFERENCE_GATEWAY_ENABLED above.
        lanePlan: makeKhalaBackedAdapterPlan(laneArming.khalaBacking),
        dispatch: {
          failureTelemetry: dispatchFailureTelemetry.record,
        },
        ...(routeAdmission === undefined ? {} : { routeAdmission }),
        internalStressPreemption,
        ...(internalStressCoordinator === undefined
          ? {}
          : { internalStressCoordinator }),
        // Provider serving policy (public_paid_model_gateway_missing on
        // api.hosted_gemini.v1): the SAME presence-derived lane arming the
        // public catalog (/v1/models) and the pre-purchase quote (/v1/quote)
        // are gated on. A request for a KNOWN model whose supply lane is not
        // armed (e.g. an absent VERTEX_SA_KEY / FIREWORKS_API_KEY) is rejected
        // with a clean model_unavailable before dispatch, so the LIVE gateway
        // serves exactly what it advertises and quotes. INERT regardless — the
        // gateway is gated by INFERENCE_GATEWAY_ENABLED above.
        laneArming,
        // Abuse / fair-share / spend-cap gates (#5486): the route exposes
        // `checkFairShare` and `checkSpendCap` seams whose pure deciders live in
        // inference-abuse-controls.ts (`decideFairShare` / `decideSpendCap`).
        // They are deliberately LEFT UNWIRED here (=> the gate is OPEN / no-op)
        // until a per-account rolling-window counter store (D1/KV keyed by account
        // + window bucket) lands; wiring them then is a one-line dep add with no
        // route change. The enforceable money-side abuse control (chargeback /
        // refund credit clawback, `clawbackInferenceCredits`) hangs off the
        // Stripe dispute/refund webhook path, not this hot route.
        //
        // Premium-model owner-grant gate (EPIC #5474 §2): premium models (Claude
        // / GPT / partner passthrough) require the requesting account's resolved
        // OWNER identity to be on the owner-controlled allowlist
        // (`inference_premium_allowlist`); a non-allowlisted premium request is
        // denied (403) with an actionable message before any dispatch. Non-premium
        // models (the Gemini free default, Fireworks open) always pass. INERT on
        // the flag-off path.
        checkPremiumAccess: makePremiumAccessGate({
          db: openAgentsDatabase(env),
          resolveOwnerIdentity,
        }),
        // OWNER BALANCE-GATE EXEMPTION SEAM (issue #6180). Wired ONLY when armed
        // (INFERENCE_OPERATOR_EXEMPTION_ENABLED). Lets an EXEMPT verified owner on
        // a non-premium / own-infra model bypass the 402 with a zero balance; the
        // `withOperatorCredit` wrapper above then records the request as
        // `operator_credit`. The gate refuses premium models + unclaimed accounts,
        // so the 402 stands for them and Khala stays paid for the public. Left
        // UNWIRED (=> the gate is closed, normal 402) when the flag is off.
        ...(operatorExemptionEnabled
          ? {
              checkOperatorExemption: makeOperatorExemptionGate({
                db: openAgentsDatabase(env),
                resolveOwnerIdentity,
              }),
            }
          : {}),
        // KHALA FREE-TIER SEAM (issue #6228). Wired ONLY when armed
        // (INFERENCE_FREE_TIER_ENABLED). Lets a self-serve FREE-TIER key on the
        // free Khala lane within its per-key daily quota bypass the 402 with a
        // zero balance; the `withFreeTierKhala` wrapper above then records the
        // request as a zero-debit free receipt + accrues the quota. The gate
        // refuses premium models, non-free-tier accounts, non-Khala models, and
        // over-quota keys, so the 402 stands for them and paid Khala for funded
        // keys is unchanged. Left UNWIRED (=> closed, normal 402) when the flag is
        // off. Uses the SAME store + quota + UTC-day bucket as the metering
        // wrapper so the bypass and the zero-debit accrual agree.
        ...(freeTierEnabled
          ? {
              checkFreeTier: makeFreeTierGate({
                db: openAgentsDatabase(env),
                quota: freeTierQuota,
                internalAccountRefs,
              }),
            }
          : {}),
        // ACCEPTANCE-DISPATCH (EPIC #6017): when khala-code produces an executable
        // artifact, enqueue an out-of-Worker verification job (a node-side runner —
        // Pylon / sandbox / Cloud Run — runs the headless suite and posts the verdict
        // back to the callback below, which backfills the receipt). DEFAULT OFF +
        // UNWIRED: `queue` is left undefined until a runner host is deployed, so even
        // with the flag on nothing is enqueued. Chromium never runs in the Worker.
        // Wiring the queue producer here (and the runner host) is the remaining
        // step to make verified khala-code true in prod.
        acceptanceDispatch: {
          enabled: isAcceptanceDispatchEnabled(
            env.KHALA_ACCEPTANCE_DISPATCH_ENABLED,
          ),
          // NEEDS-OWNER: bind a Cloudflare Queue producer + an R2 artifact store
          // here once a runner host exists. Until then the seam is inert.
          queue: undefined,
        },
        // DURABLE-STREAM RANK-1 (#6058, EPIC #6056). LIVE when
        // INFERENCE_DURABLE_STREAM_ENABLED is on AND the DO binding is wired:
        // every streamed completion is teed into the per-request Durable Object
        // (`DurableInferenceStreamObject`, keyed `getByName(responseId)`) over the
        // `/v1/stream/{id}` HTTP contract, so a client disconnect mid-generation
        // can be resumed by offset via the durable read route. The in-memory
        // `durableStream` (synchronous `StreamStore`) factory stays undefined here
        // — it is the test/contract substrate; production uses the DO namespace.
        // Metering settles EXACTLY ONCE on the real upstream EOF and NEVER on a
        // replay (the resume route has no metering hook). FAIL-SAFE: with the flag
        // off OR the binding absent, `durableStreamNamespace` is undefined and the
        // streaming path is byte-for-byte today's pure pass-through.
        durableStream: undefined,
        durableStreamNamespace:
          isInferenceDurableStreamEnabled(
            env.INFERENCE_DURABLE_STREAM_ENABLED,
          ) && env.INFERENCE_DURABLE_STREAM !== undefined
            ? (env.INFERENCE_DURABLE_STREAM as unknown as DurableStreamNamespace)
            : undefined,
        durableStreamEnabled: isInferenceDurableStreamEnabled(
          env.INFERENCE_DURABLE_STREAM_ENABLED,
        ),
        // TYPED COMPONENT CHANNEL (EPIC #6123, issue #6127). Flag-gated INERT by
        // default: with KHALA_COMPONENT_CHANNEL_ENABLED off the `oa.component` SSE
        // channel never activates and `/v1/chat/completions` is byte-for-byte
        // today's text-only stream. Even when the flag is on, the channel only
        // activates for a request that explicitly opts in (`x-oa-component-channel`
        // header / `oa_component_channel` body field) AND targets a Khala model.
        // `repairReask` (the ONE bounded repair turn) is left undefined here for
        // now — an invalid card is dropped without a repair attempt (still never
        // shipped). NEEDS-OWNER: wire `repairReask` to a single non-streaming Khala
        // call once the onboarding program lands, to enable the bounded repair.
        componentChannel: {
          enabled: isComponentChannelEnabled(
            env.KHALA_COMPONENT_CHANNEL_ENABLED,
          ),
        },
        // CROSS-APP TRACE EMISSION (#6214, epic #6206). Flag-gated INERT by
        // default: with KHALA_CHAT_TRACE_EMIT_ENABLED off, a completed Khala chat
        // session is NEVER emitted and `/v1/chat/completions` is byte-for-byte
        // today's behaviour. When the flag is on, a session is emitted as a
        // shareable `/trace/{uuid}` ONLY for a request that explicitly opts in
        // (`x-oa-emit-trace` header / `oa_emit_trace` body field). The emitter
        // persists through the SAME D1 trace store the `POST /api/traces` ingest
        // uses and reuses its validator + public-safety tripwire (never bypassed).
        // The owner is the requesting agent (`agent:<id>` from the bearer); the
        // emitted model id is the gateway projection `openagents/khala`, never a
        // raw backend. Autopilot/Pylon emission are explicit follow-ups (#6214).
        traceEmit: {
          enabled: isKhalaChatTraceEmitEnabled(
            env.KHALA_CHAT_TRACE_EMIT_ENABLED,
          ),
          // DEFAULT-ON FREE-TIER CAPTURE (#6293). SEPARATE staged flag (default
          // OFF in code): merging this does NOT auto-capture in prod — the
          // coordinator flips KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT only after
          // the redaction tests are confirmed + the migration is applied.
          captureDefaultEnabled: isKhalaFreeTierTraceCaptureDefaultEnabled(
            env.KHALA_FREE_TIER_TRACE_CAPTURE_DEFAULT,
          ),
          // CAPTURE-DEFAULT resolver: `freeTier.free && !paidPrivacy` (#6293 +
          // #6295). Free-tier is the existing self-serve free signal; paid-privacy
          // is the confidential-compute / per-account opt-OUT, FAIL-CLOSED-TO-
          // PRIVATE (an unsafe read => paidPrivacy => NOT captured). The resolver
          // itself is wrapped fail-soft at the call site (errors => not captured).
          resolveCaptureDefault: async (accountRef, _model) => {
            const db = openAgentsDatabase(env)
            const free = await readAccountFreeTier(db, accountRef)
            if (!free) {
              return false
            }
            const paidPrivacy = await makePaidPrivacyResolver({
              db,
              confidentialComputeEnabled: isConfidentialComputeEnabled(
                env.INFERENCE_CONFIDENTIAL_COMPUTE_ENABLED,
              ),
            })(accountRef)
            return !paidPrivacy.enabled
          },
          emit: async input => {
            // The accountRef is `agent:<id>`; the trace owner is that agent.
            const ownerUserId = input.accountRef.startsWith('agent:')
              ? input.accountRef.slice('agent:'.length)
              : input.accountRef
            return await emitKhalaChatTrace(
              {
                requestedModel: input.requestedModel,
                requestMessages: input.requestMessages,
                result: input.result,
                responseId: input.responseId,
              },
              {
                enabled: isKhalaChatTraceEmitEnabled(
                  env.KHALA_CHAT_TRACE_EMIT_ENABLED,
                ),
                optedIn: input.optedIn,
                // DEFAULT-ON CAPTURE (#6293): persist even without an explicit
                // opt-in when the call site resolved captureDefault. The emitter
                // stores an auto-capture (captureDefault && !optedIn) as
                // owner_only (private-by-default).
                captureDefault: input.captureDefault,
                store: makeD1TraceStore(openAgentsDatabase(env)),
                owner: {
                  ownerUserId,
                  agentRef: input.accountRef,
                  uploadSource: 'agent',
                },
                // DEMAND-ORIGIN (#6298): the SAME attribution the recorder got
                // for this request (kind + source), so the captured trace and
                // its token-ledger event always agree. Absent => `unlabeled`.
                ...(input.requestAttribution === undefined
                  ? {}
                  : {
                      demandAttribution: {
                        demandKind: input.requestAttribution.demandKind,
                        ...(input.requestAttribution.demandSource === undefined
                          ? {}
                          : {
                              demandSource:
                                input.requestAttribution.demandSource,
                            }),
                      },
                    }),
              },
            )
          },
          recordRedactionMetrics: event => {
            logWorkerRouteInfo('khala_trace_redaction_metrics', {
              emitted: event.emitted,
              reason: event.reason,
              redactionTotal: event.redactionTotal,
              redactionCounts: JSON.stringify(event.redactionCounts),
              residualTripwireCount: event.residualTripwireCount,
            })
          },
        },
        registry: inferenceProviderRegistry,
      })
    },
  },
  {
    // Machine-payable (MPP / x402) Khala endpoint (EPIC #6049, Phase 2 + 3).
    // 402-gated: a request with no payment credential returns 402 + a payment
    // challenge (USDC crypto; card/SPT when a network profile id is configured);
    // a verified credential mints Khala credits (Phase 3, reuses the USD-origin
    // credit-grant seam) and runs the SAME Khala completion path + metering +
    // receipt, so a paid call lands in the one-balance, two-inbound-rails loop
    // with contributor payout still Bitcoin/Spark.
    //
    // FAIL-SAFE INERT: with KHALA_MPP_ENABLED off OR no STRIPE_API_KEY the
    // endpoint returns a clean "not configured" 503 and NEVER constructs a
    // charge. A missing STRIPE_MPP_NETWORK_PROFILE_ID disables only the card
    // rail; the crypto rail still works.
    path: '/mpp/v1/chat/completions',
    handler: (request, env) => {
      registerPassthroughAdapters(inferenceProviderRegistry, env)
      registerHydraliskAdapter(inferenceProviderRegistry, env)
      registerOpenRouterAdapter(inferenceProviderRegistry, env)
      registerFabricServeAdapter(inferenceProviderRegistry, env)
      setInferenceAdapterEnv(env)
      const ownerClaimStore = makeD1AgentOwnerClaimStore(
        openAgentsDatabase(env),
      )
      const resolveOwnerIdentity = makeVerifiedOwnerIdentityResolver(
        ownerClaimStore.readVerifiedPublicIdentityForAgentUserId,
      )
      // Bitcoin-first Lightning rail (EPIC #6049). Offered FIRST when armed:
      // KHALA_MPP_LIGHTNING_ENABLED on AND a Lightning invoice issuer is
      // configured (Spark primary via the MDK_TREASURY container, MDK fallback
      // via the route/sidecar wallet binding). With either absent the issuer is
      // undefined and the rail is never advertised (honesty gate).
      const lightningEnabled = isKhalaMppLightningEnabled(
        env.KHALA_MPP_LIGHTNING_ENABLED,
      )
      const mintLightningInvoice = lightningEnabled
        ? lightningInvoiceIssuerForEnv(env)
        : undefined
      const laneArming = resolveSupplyLaneArming(env)
      return handleMppChatCompletions(request, {
        db: openAgentsDatabase(env),
        enabled: isKhalaMppEnabled(env.KHALA_MPP_ENABLED),
        lightningEnabled,
        ...(mintLightningInvoice === undefined ? {} : { mintLightningInvoice }),
        signingSecret: env.KHALA_MPP_SIGNING_SECRET,
        stripeNetworkProfileId: env.STRIPE_MPP_NETWORK_PROFILE_ID,
        stripeSecretKey: env.STRIPE_API_KEY,
        // The underlying Khala completion reuses the SAME registry, metering
        // hook, receipt, lane plan, serving policy, and premium gate as the keyed
        // `/v1/chat/completions` route — only auth + balance are replaced by the
        // payer-bound account + minted MPP credit inside the MPP handler. The
        // completion still runs gated by INFERENCE_GATEWAY_ENABLED.
        completionDeps: {
          enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
          meteringHook: withFreeAllowance(
            withReferralAccrual(
              makeLedgerMeteringHook({ db: openAgentsDatabase(env) }),
              { db: openAgentsDatabase(env) },
            ),
            { db: openAgentsDatabase(env), resolveOwnerIdentity },
          ),
          // SERVED-TOKENS COUNTER (issue #6227): the MPP (machine-payable) Khala
          // completion path lands its served tokens in the SAME canonical ledger
          // the public counter sums, so x402/MPP traffic counts too.
          recordTokensServed: makeD1ServedTokensRecorder(
            openAgentsDatabase(env),
            {
              // Live-counter push (#6231): MPP traffic rolls the homepage
              // odometer up instantly too. Fail-soft.
              publishDelta: delta =>
                Effect.promise(() =>
                  publishKhalaTokensServedDelta(
                    env,
                    buildKhalaTokensServedDelta(delta),
                  ).catch(() => undefined),
                ),
            },
          ),
          // Same internal/ops account demand allowlist on the MPP path (#6298
          // follow-up), so the resolver stays uniform across both gateways.
          internalAccountRefs: parseInternalAccountRefs(
            env.INFERENCE_INTERNAL_ACCOUNT_REFS,
          ),
          lanePlan: makeKhalaBackedAdapterPlan(laneArming.khalaBacking),
          laneArming,
          checkPremiumAccess: makePremiumAccessGate({
            db: openAgentsDatabase(env),
            resolveOwnerIdentity,
          }),
          acceptanceDispatch: {
            enabled: isAcceptanceDispatchEnabled(
              env.KHALA_ACCEPTANCE_DISPATCH_ENABLED,
            ),
            queue: undefined,
          },
          registry: inferenceProviderRegistry,
        },
      })
    },
  },
  {
    // Verdict-callback ingest (EPIC #6017). A node-side runner posts its executed
    // `AcceptanceVerdict` here; the authenticated route backfills the khala-code
    // verification verdict (`unverified` -> `test_passed`/`failed`). INERT by
    // default: gated by INFERENCE_GATEWAY_ENABLED AND closed unless
    // ACCEPTANCE_VERDICT_CALLBACK_TOKEN is configured (every verdict rejected).
    path: '/v1/inference/acceptance-verdicts',
    handler: (request, env) =>
      handleAcceptanceVerdictCallback(request, {
        callbackToken: env.ACCEPTANCE_VERDICT_CALLBACK_TOKEN,
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        nowIso: currentIsoTimestamp,
        store: makeD1KhalaVerificationStore(openAgentsDatabase(env)),
        // Accepted-outcome settlement (#6011): fires worker+validator Bitcoin payout on
        // the first VERIFIED+EXECUTED backfill. Double-gated + inert by default (undefined
        // unless the KHALA loop flag is armed; even then the owner real-settlement gate +
        // caps + destination fail-close inside the engine). NEEDS-OWNER to arm real money.
        ...(sink => (sink === undefined ? {} : { settlement: sink }))(
          makeAcceptedOutcomeSettlementSink(env),
        ),
      }),
  },
  {
    // Out-of-Worker runner JOB LEASE (EPIC #6017). The runner CANNOT be a Cloudflare
    // Queue consumer (a consumer is a Worker; chromium never runs in a Worker), so it
    // PULLS work here: an authenticated GET leases the next pending acceptance job (or
    // 204 when idle). The runner runs the headless suite OUT of the Worker and POSTs the
    // verdict to the callback above. INERT by default: gated by INFERENCE_GATEWAY_ENABLED
    // AND closed unless ACCEPTANCE_VERDICT_CALLBACK_TOKEN is configured (every request
    // 401), AND empty until the dispatch producer enqueues into the pull queue (a
    // NEEDS-OWNER step owned by the dispatch lane). The lease uses the SAME bearer token
    // as the verdict callback — one secret authenticates the whole runner<->gateway
    // channel.
    path: '/v1/inference/acceptance-jobs/lease',
    handler: (request, env) =>
      handleAcceptanceJobLease(request, {
        callbackToken: env.ACCEPTANCE_VERDICT_CALLBACK_TOKEN,
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        newLeaseId: randomUuid,
        nowIso: currentIsoTimestamp,
        store: makeD1AcceptanceJobQueueStore(
          openAgentsDatabase(env),
          currentIsoTimestamp,
        ),
      }),
  },
  {
    // Out-of-Worker runner JOB ACK (EPIC #6017). The runner reports a leased job's
    // terminal outcome: delivered (the verdict was posted + the receipt backfilled) =>
    // remove the job; retryable (delivery failed) => return it to pending for re-lease.
    // Same INERT gate + bearer auth as the lease route.
    path: '/v1/inference/acceptance-jobs/ack',
    handler: (request, env) =>
      handleAcceptanceJobAck(request, {
        callbackToken: env.ACCEPTANCE_VERDICT_CALLBACK_TOKEN,
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        newLeaseId: randomUuid,
        nowIso: currentIsoTimestamp,
        store: makeD1AcceptanceJobQueueStore(
          openAgentsDatabase(env),
          currentIsoTimestamp,
        ),
      }),
  },
  {
    // Public model catalog (OpenAI-compatible GET /v1/models) for the inference
    // gateway. INERT by default: gated behind the SAME INFERENCE_GATEWAY_ENABLED
    // flag as /v1/chat/completions, so it 404s when the gateway is off. Public,
    // unauthenticated, public-safe (published sell prices + free-key tier policy
    // only, no prompts/credentials/balances) — the pre-purchase discovery surface a
    // credits customer reads to learn what each model costs before funding a
    // balance. Derived from the SAME pricing table the metering hook charges
    // against, so the published price cannot drift from the billed price. No
    // promise state changes; the paid loop is still secrets-gated.
    path: '/v1/models',
    handler: (request, env) =>
      handleModelsList(request, {
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        freeTierEnabled: isFreeTierEnabled(env.INFERENCE_FREE_TIER_ENABLED),
        freeTierQuota: resolveFreeTierQuota(env),
        laneArming: resolveSupplyLaneArming(env),
      }),
  },
  {
    // Pre-purchase cost quote (POST /v1/quote) for the inference gateway. INERT
    // by default: gated behind the SAME INFERENCE_GATEWAY_ENABLED flag as
    // /v1/chat/completions and /v1/models, so it 404s when the gateway is off.
    // Public, unauthenticated, public-safe — like /v1/models it reads only the
    // published catalog price (the estimator omits our cost basis / margin),
    // moves no money, and writes no ledger row. It returns the exact
    // credit/USD/msat charge the metering hook WOULD settle for a given model +
    // token estimate + funding rail (`isEstimate: true`), so a credits customer
    // can size a deliberate spend before funding a balance. Additively, when the
    // body carries `budgetCredits` it answers the INVERSE affordability question
    // ("how many such requests does N credits buy?") via `estimateBudgetCapacity`,
    // embedding the same per-request estimate under `perRequest`. Computed from the
    // SAME pricing engine (`priceRequest`) the metering hook bills against, so a
    // quote cannot drift from the eventual billed charge. No promise state
    // changes; the paid loop is still secrets-gated.
    path: '/v1/quote',
    handler: (request, env) =>
      handleQuote(request, {
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        laneArming: resolveSupplyLaneArming(env),
      }),
  },
  {
    // Public gateway readiness summary (GET /v1/gateway/readiness). INERT by
    // default: gated behind the SAME INFERENCE_GATEWAY_ENABLED flag as the rest
    // of the gateway, so it 404s when the gateway is off. Public, unauthenticated,
    // public-safe — it exposes the SINGLE readiness fact projected from the SAME
    // catalog + serving policy the /v1/models, /v1/quote, and /v1/chat/completions
    // surfaces gate on (servable/hidden model COUNTS + per-lane arming booleans +
    // dereferenceable reason refs only; no prompts/credentials/prices/balances).
    // So an operator (or the launch dashboard) can verify "can the paid gateway
    // serve anything right now, and how degraded is its catalog?" in one read
    // instead of replaying each surface. No promise state changes.
    path: '/v1/gateway/readiness',
    handler: (request, env) =>
      handleGatewayReadiness(request, {
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        laneArming: resolveSupplyLaneArming(env),
      }),
  },
  {
    // Public GLM fleet readiness summary (GET /v1/gateway/glm-fleet/readiness).
    // Read-only and public-safe: stable replica refs plus aggregate warm, ready,
    // reclaimed, disabled, and unavailable counts. It reuses configured GLM
    // replica arming and the latest in-memory heartbeat projection; it never
    // probes hosts, returns raw origins, or changes replica state.
    path: '/v1/gateway/glm-fleet/readiness',
    handler: (request, env) =>
      handleGlmFleetReadiness(request, {
        db: openAgentsDatabase(env),
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        env,
      }),
  },
  {
    // Public gateway dispatch-failure telemetry (GET
    // /v1/gateway/dispatch-failures). Live-at-read, in-memory, bounded-window
    // counters for the typed router failure classes. The recent event sample is
    // redacted to classifier/stage/status-class only; no raw adapter ids, prompts,
    // completions, URLs, credentials, balances, or provider payloads are exposed.
    path: '/v1/gateway/dispatch-failures',
    handler: (request, env) =>
      handleDispatchFailureTelemetryReadout(request, {
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        nowMs: currentEpochMillis,
        telemetry: dispatchFailureTelemetry,
      }),
  },
  {
    // Operator-only Pylon fabric canary (#6089). This is deliberately separate
    // from /v1/chat/completions so the public model catalog stays collapsed to
    // `openagents/khala`; the route runs one fixed known-answer prompt through
    // the secret-backed admitted Pylon adapter and returns public-safe status
    // only. It never reads customer input, debits credits, or exposes endpoint
    // URLs / bearer material.
    path: '/api/operator/inference/pylon-fabric/smoke',
    handler: (request, env) =>
      handlePylonFabricSmoke(request, {
        adapter: makeConfiguredFabricServeAdapter(env),
        enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
        nowIso: currentIsoTimestamp,
        requireOperator: () => requireAdminApiToken(request, env),
      }),
  },
  {
    // Fine-tuning service (EPIC #5510, #5516) — sellable Cloud primitive
    // SCAFFOLD. INERT by default: gated behind CLOUD_FINE_TUNING_ENABLED
    // (default off). Ships wired to the stub/accepting runtime adapter + no-op
    // metering stub; #5516 registers the real training-lane runtime adapter and
    // live credit metering. The promise `cloud.fine_tuning_service.v1` STAYS red
    // — this surface produces no paid/servable result and no green flip lands
    // without a dereferenceable paid receipt.
    path: '/v1/fine_tuning/jobs',
    handler: (request, env) =>
      handleFineTuningJobSubmit(request, {
        authenticate: async authRequest => {
          const token = readBearerToken(authRequest)
          if (token === undefined) {
            return undefined
          }
          const session = await authenticateProgrammaticAgent(
            makeD1AgentRegistrationStore(openAgentsDatabase(env)),
            token,
          )
          return session === undefined
            ? undefined
            : { accountRef: `agent:${session.user.id}` }
        },
        enabled: isFineTuningServiceEnabled(env.CLOUD_FINE_TUNING_ENABLED),
      }),
  },
  {
    // Sandbox compute service (EPIC #5510, #5517) — sellable Cloud primitive
    // SCAFFOLD. INERT by default: gated behind CLOUD_SANDBOX_COMPUTE_ENABLED
    // (default off). Ships wired to the stub/accepting runtime adapter + no-op
    // metering stub; #5517 registers the real isolated-session runtime adapter
    // and live credit metering. The promise `cloud.sandbox_compute_service.v1`
    // STAYS red — this surface provisions no real sandbox and no green flip
    // lands without a dereferenceable paid receipt.
    path: '/v1/sandboxes',
    handler: (request, env) =>
      handleSandboxRequest(request, {
        authenticate: async authRequest => {
          const token = readBearerToken(authRequest)
          if (token === undefined) {
            return undefined
          }
          const session = await authenticateProgrammaticAgent(
            makeD1AgentRegistrationStore(openAgentsDatabase(env)),
            token,
          )
          return session === undefined
            ? undefined
            : { accountRef: `agent:${session.user.id}` }
        },
        enabled: isSandboxComputeServiceEnabled(
          env.CLOUD_SANDBOX_COMPUTE_ENABLED,
        ),
      }),
  },
])

export const exactRoutePathManifest = exactRouteRegistry.paths

const routeRequest = makeWorkerRouteRequest({
  cleanProductRouteRedirectLocation,
  exactRoutes: exactRouteRegistry.routes,
  handleAppShellPage: (request, env, ctx) =>
    routeEffect('handle_app_shell_page', () =>
      handleAppShellPage(request, env, ctx),
    ),
  handleAssetRequest: (request, env) =>
    routeEffect('handle_asset_request', () => env.ASSETS.fetch(request)),
  handleThreadPage: (request, env, ctx, threadId) =>
    routeEffect('handle_thread_page', () =>
      handleThreadPage(request, env, ctx, threadId),
    ),
  handleForumThreadPage: (request, env, ctx, topicId) =>
    routeEffect('handle_forum_thread_page', () =>
      handleForumThreadDocument({
        db: openAgentsDatabase(env),
        fetchAppShell: () => handleAppShellPage(request, env, ctx),
        topicId,
      }),
    ),
  optionalUuid,
  routeAutopilotWorkRequest: (request, env, ctx) =>
    autopilotDecisionRoutes.routeAutopilotDecisionRequest(request, env, ctx) ??
    autopilotWorkRoutes.routeAutopilotWorkRequest(request, env, ctx),
  // Cloud coding-session surface (autopilot.cloud_coding_sessions.v1, red).
  // INERT behind CLOUD_CODING_SESSIONS_ENABLED (default off). Wired to the same
  // programmatic-agent auth the inference gateway / sandbox / fine-tuning
  // surfaces use; ships defaulted to the stub runtime adapter + no-op metering
  // stub, so on prod every route returns 404 and nothing is provisioned or
  // billed. The managed GCE control-plane adapter + live receipt-first metering
  // hook plug into the module's seams when the EPIC lands.
  routeCloudCodingSessionRequest: (request, env) =>
    routeCloudCodingSessionRequestImpl(request, {
      authenticate: async authRequest => {
        const token = readBearerToken(authRequest)
        if (token === undefined) {
          return undefined
        }
        const session = await authenticateProgrammaticAgent(
          makeD1AgentRegistrationStore(openAgentsDatabase(env)),
          token,
        )
        return session === undefined
          ? undefined
          : { accountRef: `agent:${session.user.id}` }
      },
      enabled: isCloudCodingSessionsEnabled(env.CLOUD_CODING_SESSIONS_ENABLED),
    }),
  routeAgentGoalRequest: agentGoalRoutes.routeAgentGoalRequest,
  routeAutopilotOnboardingTurnRequest: (request, env) =>
    autopilotOnboardingRoutes.routeOnboardingTurnRequest(request, env),
  routeKhalaChatRequest: (request, env) =>
    khalaChatRoutes.routeKhalaChatRequest(request, env),
  routeAgentOwnerClaimRequest:
    agentOwnerClaimRoutes.routeAgentOwnerClaimRequest,
  routeCheckoutPageRequest: (request, env) =>
    checkoutPageRoutes.routeCheckoutPageRequest(request, env),
  routeTreasuryPageRequest: (request, env) =>
    makeTreasuryPageRoutes({
      fetchTreasury: fetchMdkTreasuryPath(env),
      makeUuid: randomUuid,
      nowIso: currentIsoTimestamp,
      store: makeD1TreasuryTransactionStore(openAgentsDatabase(env)),
    }).routeTreasuryPageRequest(request),
  routeAgentProposalRequest: agentProposalRoutes.routeAgentProposalRequest,
  routeAgentSearchRequest: agentSearchRoutes.routeAgentSearchRequest,
  routeAgentScopedGrantRequest:
    agentScopedGrantRoutes.routeAgentScopedGrantRequest,
  routeAgentSiteRequest: agentSiteRoutes.routeAgentSiteRequest,
  routeForumRequest: (request, env, ctx) =>
    forumRoutes.routeForumRequest(request, openAgentsDatabase(env), {
      tipsBufferPay: tipsBufferPayFnForEnv(env),
      agentStore: makeD1AgentRegistrationStore(openAgentsDatabase(env)),
      ...(() => {
        const forumWorkRequestRelayPublisher =
          forumWorkRequestRelayPublisherForEnv(env)

        return forumWorkRequestRelayPublisher === undefined
          ? {}
          : { forumWorkRequestRelayPublisher }
      })(),
      hostedMdkClient: hostedMdkClientForEnv(env),
      l402SigningBoundary: () => forumL402SigningBoundaryForEnv(env),
      mdkWebhookConfig: hostedMdkWebhookConfigForEnv(env),
      publicIdentityClaimStore: makeD1AgentOwnerClaimStore(
        openAgentsDatabase(env),
      ),
      pylonApiStore: makeD1PylonApiStore(openAgentsDatabase(env)),
      pylonSparkPayoutTargetStore: makeD1PylonSparkPayoutTargetStore(
        openAgentsDatabase(env),
      ),
      resolveModeratorActor: async request => {
        const session = await requireBrowserSession(request, env, ctx)

        if (session === undefined) {
          return undefined
        }

        if (!isOpenAgentsAdminEmail(session.user.email)) {
          return {
            _tag: 'Forbidden' as const,
            reason: 'Forum moderation requires an OpenAgents admin session.',
          }
        }

        return {
          _tag: 'Moderator' as const,
          actor: {
            displayName: session.user.name,
            operatorId: session.user.userId,
            slug: session.user.login ?? session.user.userId,
          },
        }
      },
    }),
  routeImageGenerationRequest:
    imageGenerationRoutes.routeImageGenerationRequest,
  // OpenAI-compatible GET /v1/models/{model} retrieve. Gated by the SAME
  // INFERENCE_GATEWAY_ENABLED flag as the list and chat-completions routes, so
  // it 404s when the gateway is off. Public + unauthenticated (published price
  // and free-key tier policy only — public-safe pre-purchase discovery), serving
  // prices derived from the same pricing table the metering hook charges
  // against. No promise state changes; the paid loop is still secrets-gated.
  routeModelRetrieveRequest: (request, env) =>
    routeModelRetrieveRequest(request, {
      enabled: isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED),
      freeTierEnabled: isFreeTierEnabled(env.INFERENCE_FREE_TIER_ENABLED),
      freeTierQuota: resolveFreeTierQuota(env),
      laneArming: resolveSupplyLaneArming(env),
    }),
  // MirrorCode demo: GET /api/gym/mirrorcode/runs/{id} (#6378). The public-safe
  // single-run read — the path-param surface the exact-route registry cannot
  // match. Returns undefined for any non-matching path so the cascade falls
  // through; the base /api/gym/mirrorcode/runs list+launch is an exact route.
  routeMirrorCodeRunByIdRequest: (request, env) => {
    const runId = matchMirrorCodeRunByIdRequest(request)
    if (runId === undefined) {
      return undefined
    }
    return handleMirrorCodeRunByIdApi(request, runId, {
      store: makeD1MirrorCodeRunStore(openAgentsDatabase(env)),
    })
  },
  // Durable inference resume read GET /v1/chat/completions/durable/{requestId}
  // (durable-stream Rank-1, #6058). Reads stored bytes only — NEVER meters.
  // Shares the gateway flag AND the durable-stream flag. LIVE when both are on
  // AND the DO binding is wired: resumes from the per-request Durable Object the
  // chat route's producer teed into. FAIL-SAFE: with the flag off OR the binding
  // absent, returns an honest 404 (the synchronous fallback path).
  routeDurableInferenceReadRequest: (request, env) => {
    const matched = matchDurableReadRequest(request)
    if (matched === undefined) {
      return undefined
    }

    const enabled =
      isInferenceGatewayEnabled(env.INFERENCE_GATEWAY_ENABLED) &&
      isInferenceDurableStreamEnabled(env.INFERENCE_DURABLE_STREAM_ENABLED)
    const namespace =
      enabled && env.INFERENCE_DURABLE_STREAM !== undefined
        ? (env.INFERENCE_DURABLE_STREAM as unknown as DurableStreamNamespace)
        : undefined
    const authorizeKhalaAssignmentRead = async (): Promise<
      Response | undefined
    > => {
      const db = openAgentsDatabase(env)
      const pylonStore = makeD1PylonApiStore(db)
      let khalaAssignmentExists = false
      try {
        khalaAssignmentExists =
          (await pylonStore.readAssignmentByIdempotencyKeyHash(
            `khala-coding:${matched.requestId}`,
          )) !== undefined
      } catch {
        return noStoreJsonResponse(
          { error: 'durable_request_authorization_unavailable' },
          { status: 503 },
        )
      }
      if (!khalaAssignmentExists) {
        return undefined
      }

      const token = readBearerToken(request)
      if (token === undefined) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const agentStore = makeD1AgentRegistrationStore(db)
      const session = await authenticateProgrammaticAgent(agentStore, token)
      if (session === undefined) {
        return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
      }

      const authorized = await khalaDurableRequestIsLinkedToPrincipal({
        agentStore,
        durableRequestId: matched.requestId,
        principal: khalaMcpAgentPrincipal(session, currentIsoTimestamp()),
        pylonStore,
      })
      return authorized
        ? undefined
        : noStoreJsonResponse(
            {
              error: 'durable_request_not_authorized',
              reason:
                'The durable Khala stream is attached to an assignment outside the caller-owned linked Pylon set.',
            },
            { status: 403 },
          )
    }
    // Production DO-backed resume (async): when the binding is wired, a durable
    // read URL resolves to an Effect that reads the per-request DO; a non-durable
    // URL falls through (undefined). The URL match is synchronous, so the
    // dispatcher contract (`Effect<Response> | undefined`) is honored: undefined
    // only for a non-match.
    if (namespace !== undefined) {
      return Effect.promise(async () => {
        const denial = await authorizeKhalaAssignmentRead()
        if (denial !== undefined) {
          return denial
        }
        return routeDurableInferenceReadRequestDO(request, {
          enabled,
          namespace,
        }).then(
          // A matched durable URL with the namespace present always yields a
          // Response; the `?? notFound` is a defensive total fallback.
          response =>
            response ??
            new Response(JSON.stringify({ error: 'not_found' }), {
              headers: {
                'cache-control': 'no-store',
                'content-type': 'application/json',
              },
              status: 404,
            }),
        )
      })
    }
    // Fail-safe synchronous path (binding absent / flag off): honest 404.
    const response = routeDurableInferenceReadRequest(request, {
      durableStream: undefined,
      enabled,
      nowEpochMillis: currentEpochMillis,
    })
    return response === undefined ? undefined : Effect.succeed(response)
  },
  routeMulletRequest: mulletRoutes.routeMulletRequest,
  routeOmniRequest: (request, env, ctx) =>
    omniRoutes.routeOmniRequest(request, env, ctx) ??
    omniWorkroomRoutes.routeOmniWorkroomRequest(request, env, ctx) ??
    omniWorkroomLifecycleRoutes.routeOmniWorkroomLifecycleRequest(
      request,
      env,
      ctx,
    ) ??
    omniBundleRoutes.routeOmniBundleRequest(request, env, ctx) ??
    omniHandoffRoutes.routeOmniHandoffRequest(request, env, ctx) ??
    nativeListsRoutes.routeNativeListsRequest(request, env, ctx) ??
    sitePageFormCaptureRoutes.routeSitePageFormCaptureRequest(
      request,
      env,
      ctx,
    ) ??
    prefilledWorkspaceRoutes.routePrefilledWorkspaceRequest(
      request,
      env,
      ctx,
    ) ??
    privateProjectWorkspaceRoutes.routePrivateProjectWorkspaceRequest(
      request,
      env,
      ctx,
    ) ??
    teamWorkspaceInviteRoutes.routeTeamWorkspaceInviteRequest(
      request,
      env,
      ctx,
    ) ??
    tenantClientRoutes.routeTenantClientRequest(request, env, ctx) ??
    tenantHostnameSelfServeRoutes.routeTenantHostnameSelfServeRequest(
      request,
      env,
      ctx,
    ) ??
    emailSequenceAuthoringRoutes.routeEmailSequenceAuthoringRequest(
      request,
      env,
      ctx,
    ) ??
    sitesOrchestrationRoutes.routeSitesOrchestrationRequest(
      request,
      env,
      ctx,
    ) ??
    partnerPayoutLedgerRoutes.routePartnerPayoutLedgerRequest(
      request,
      env,
      ctx,
    ) ??
    partnerAgreementRoutes.routePartnerAgreementRequest(request, env, ctx) ??
    crmImportRoutes.routeCrmImportRequest(request, env, ctx) ??
    crmEmailRoutes.routeCrmEmailRequest(request, env, ctx) ??
    crmResendRoutes.routeCrmResendRequest(request, env, ctx) ??
    crmSendRoutes.routeCrmSendRequest(request, env, ctx) ??
    crmCommandRoutes.routeCrmCommandRequest(request, env, ctx) ??
    crmBatchRoutes.routeCrmBatchRequest(request, env, ctx) ??
    crmMcpDiscoveryRoutes.routeCrmMcpDiscoveryRequest(request, env, ctx) ??
    crmMcpGrantRoutes.routeCrmMcpGrantRequest(request, env, ctx) ??
    crmMcpRoutes.routeCrmMcpRequest(request, env, ctx) ??
    crmRoutes.routeCrmRequest(request, env, ctx),
  routeOnboardingRequest: onboardingRoutes.routeOnboardingRequest,
  routeNexusPylonVisibilityRequest:
    nexusPylonVisibilityRoutes.routeNexusPylonVisibilityRequest,
  routePublicCardCreditSpendReceiptRequest:
    publicCardCreditSpendReceiptRoutes.routePublicCardCreditSpendReceiptRequest,
  routePublicInferenceReceiptRequest:
    publicInferenceReceiptRoutes.routePublicInferenceReceiptRequest,
  routePublicCloudPrimitiveReceiptRequest:
    publicCloudPrimitiveReceiptRoutes.routePublicCloudPrimitiveReceiptRequest,
  routePublicNip90MarketReceiptRequest:
    publicNip90MarketReceiptRoutes.routePublicNip90MarketReceiptRequest,
  routePublicPartnerPayoutReceiptRequest:
    publicPartnerPayoutReceiptRoutes.routePublicPartnerPayoutReceiptRequest,
  routePublicSiteReferralPayoutReceiptRequest:
    publicSiteReferralPayoutReceiptRoutes.routePublicSiteReferralPayoutReceiptRequest,
  routePublicStripeCheckoutReceiptRequest:
    publicStripeCheckoutReceiptRoutes.routePublicStripeCheckoutReceiptRequest,
  routeEcommerceCampaignReceiptRequest:
    ecommerceCampaignReceiptRoutes.routeEcommerceCampaignReceiptRequest,
  routeEcommerceCampaignReceiptOperatorRequest:
    ecommerceCampaignReceiptOperatorRoutes.routeEcommerceCampaignReceiptOperatorRequest,
  routeEcommerceCampaignSelfServeRequest:
    ecommerceCampaignSelfServeRoutes.routeEcommerceCampaignSelfServeRequest,
  routeMarketingAgencyReceiptRequest:
    marketingAgencyReceiptPublicRoutes.routeMarketingAgencyReceiptRequest,
  routeMarketingAgencySelfServeRequest:
    marketingAgencySelfServePublicRoutes.routeMarketingAgencySelfServeRequest,
  routePylonApiRequest: pylonApiRoutes.routePylonApiRequest,
  routeSiteCommerceRequest: (request, _env, _ctx) =>
    siteCommerceRoutesForEnv(_env).routeSiteCommerceRequest(request),
  routeSiteReferralInspectionRequest:
    siteReferralInspectionRoutes.routeSiteReferralInspectionRequest,
  routeSiteReferralPayoutLedgerRequest:
    siteReferralPayoutLedgerRoutes.routeSiteReferralPayoutLedgerRequest,
  routeInferenceReferralRequest:
    inferenceReferralRoutes.routeInferenceReferralRequest,
  routeSiteReferralRequest: siteReferralRoutes.routeSiteReferralRequest,
  routeOperatorAdjutantRequest:
    operatorAdjutantRoutes.routeOperatorAdjutantRequest,
  routeOperatorArtanisChatRequest:
    operatorArtanisChatRoutes.routeOperatorArtanisChatRequest,
  routeOperatorArtanisConsoleRequest:
    operatorArtanisConsoleRoutes.routeOperatorArtanisConsoleRequest,
  routeOperatorEmailInspectionRequest:
    operatorEmailInspectionRoutes.routeOperatorEmailInspectionRequest,
  routeOperatorOrderTriageRequest:
    operatorOrderTriageRoutes.routeOperatorOrderTriageRequest,
  routeOperatorPylonMarketplaceRequest:
    operatorPylonMarketplaceRoutes.routeOperatorPylonMarketplaceRequest,
  routeOperatorProviderAccountRequest: (request, env) => {
    const response =
      operatorProviderAccountRoutes.routeOperatorProviderAccountRequest(
        request,
        env,
      )

    return response === undefined
      ? undefined
      : routeEffectOrResponse(
          routeEffect(
            'route_operator_provider_account_request',
            () => response,
          ),
        )
  },
  routeOperatorSitesRequest: operatorSitesRoutes.routeOperatorSitesRequest,
  routeProviderAccountRequest:
    providerAccountRoutes.routeProviderAccountRequest,
  routeShareRequest: shareRoutes.routeShareRequest,
  routeSyncRequest: syncRoutes.routeSyncRequest,
  routeTeamChatRequest: teamChatRoutes.routeTeamChatRequest,
  routeThreadFileRequest: threadFileRoutes.routeThreadFileRequest,
  routeHygieneLaneSettlementRequest: (request, env) =>
    hygieneLaneSettlementRoutes.routeHygieneLaneSettlementRequest(request, env),
  routeFirmupLaneSettlementRequest: (request, env) =>
    firmupBitcoinSettlementRoutes.routeFirmupLaneSettlementRequest(
      request,
      env,
    ),
  routeTassadarTraceContributionRequest:
    tassadarTraceContributionRoutes.routeTassadarTraceContributionRequest,
  routeTraceRequest: traceStoreRoutes.routeTraceRequest,
  routeTrainingRunWindowRequest:
    trainingRunWindowRoutes.routeTrainingRunWindowRequest,
  routeTrainingVerificationRequest:
    trainingVerificationRoutes.routeTrainingVerificationRequest,
})

type SyncSocketAttachment = Readonly<{
  cursor: number
  scope: string
}>

const syncSocketClose = (
  socket: WebSocket,
  code: number,
  reason: string,
): void => {
  try {
    socket.close(code, reason)
  } catch {
    // The browser may already have closed the WebSocket while replay is pending.
  }
}

const syncSocketSendJson = (socket: WebSocket, value: unknown): boolean => {
  try {
    socket.send(JSON.stringify(value))
    return true
  } catch {
    syncSocketClose(socket, 1011, 'sync replay failed')
    return false
  }
}

const syncSocketAttachment = (
  value: unknown,
): SyncSocketAttachment | undefined => {
  if (!isRecord(value)) {
    return undefined
  }

  return typeof value.scope === 'string' && typeof value.cursor === 'number'
    ? { cursor: value.cursor, scope: value.scope }
    : undefined
}

const syncCursorFromUrl = (url: URL): number => {
  const cursor = Number(url.searchParams.get('cursor') ?? '0')

  return Number.isInteger(cursor) && cursor >= 0 ? cursor : 0
}

const syncScopeFromRequest = (request: Request, url: URL): string | undefined =>
  request.headers.get('x-openagents-sync-scope') ??
  url.searchParams.get('scope') ??
  undefined

// Durable-stream Rank-1 resumable inference DO (#6058, EPIC #6056). One DO
// instance per request id (`idFromName(requestId)`) holds that completion's
// durable offset log in SQLite (the `@openagentsinc/durable-stream`
// `SqliteStreamStore`). The gateway tees the upstream token stream in as the
// producer; a dropped client resumes by reading `/v1/stream/{requestId}?offset=`.
// TTL/expiry is driven by DO alarms (storage bounded). INERT unless the
// INFERENCE_DURABLE_STREAM_ENABLED flag is on AND this binding is wired.
export class DurableInferenceStreamObject {
  // The durable offset log is entirely self-contained in this DO's SQLite
  // storage, so the DO needs no `Env` — it does not read bindings/secrets. The
  // constructor takes only the DO state (Cloudflare passes `(state, env)`; the
  // unused `env` is intentionally not bound).
  constructor(private readonly state: DurableObjectState) {}

  async fetch(request: Request): Promise<Response> {
    return handleDurableStreamFetch(
      this.state as unknown as DurableStreamObjectStateLike,
      request,
    )
  }

  async alarm(): Promise<void> {
    handleDurableStreamAlarm(
      this.state as unknown as DurableStreamObjectStateLike,
    )
  }
}

// High-frequency public tokens-served broadcast throttle (openagents #6324).
// The throttle DECISION + interval + scope predicate now live in the pure,
// unit-tested module `./sync-broadcast-throttle`. This DO only persists the
// durable `lastBroadcastAt` and performs the fanout. See that module for the
// hibernation/freeze-then-jump root cause and the leading-edge fix rationale.
export class SyncRoomDurableObject {
  constructor(
    private readonly state: DurableObjectState,
    private readonly env: Env,
  ) {}

  private async replaySocket(
    socket: WebSocket,
    scope: string,
    cursor: number,
  ): Promise<void> {
    try {
      const repository = makeD1SyncOutboxRepository(
        openAgentsDatabase(this.env),
      )
      const snapshot = await repository.readSnapshot(scope)

      if (cursor > snapshot.cursor) {
        if (
          syncSocketSendJson(socket, cursorGap(scope, snapshot.cursor, cursor))
        ) {
          socket.serializeAttachment({ cursor: snapshot.cursor, scope })
        }
        return
      }

      const patches = await repository.readChangesAfter(scope, cursor)
      let nextCursor = cursor

      for (const patch of patches) {
        if (!syncSocketSendJson(socket, patch)) {
          return
        }

        nextCursor = patch.seq
      }

      socket.serializeAttachment({ cursor: nextCursor, scope })
    } catch (error) {
      logWorkerRouteError('sync_replay_failed', error, {
        errorName: errorName(error),
        scope,
      })
      syncSocketClose(socket, 1011, 'sync replay failed')
    }
  }

  private async broadcastScope(scope: string): Promise<void> {
    await Promise.all(
      this.state.getWebSockets(scope).map(async socket => {
        const attachment = syncSocketAttachment(socket.deserializeAttachment())

        await this.replaySocket(socket, scope, attachment?.cursor ?? 0)
      }),
    )
  }

  // Hibernation-safe, burst-safe leading-edge throttle for the high-frequency
  // public tokens-served scope (openagents #6324). The decision is made by the
  // pure `decideHighFrequencyBroadcast` helper against a DURABLE `lastBroadcastAt`
  // read from `state.storage`, so it survives DO hibernation between hibernatable
  // WebSocket events and never depends on a sub-second alarm. Non-throttled scopes
  // always broadcast immediately. A skipped intermediate poke loses nothing: the
  // authoritative running total rides every event + the summary row, so the next
  // poke ~334ms later carries the latest total — the counter never freezes-then-
  // jumps, it advances in steady ≤3/sec steps.
  private async notifyScopeThrottled(scope: string): Promise<void> {
    const storageKey = highFrequencyBroadcastLastAtStorageKey(scope)
    const lastBroadcastAtMs =
      (await this.state.storage.get<number>(storageKey)) ?? null

    const decision = decideHighFrequencyBroadcast({
      scope,
      nowMs: currentEpochMillis(),
      lastBroadcastAtMs,
    })

    if (!decision.broadcast) {
      return
    }

    if (decision.persistLastBroadcastAtMs !== undefined) {
      await this.state.storage.put(
        storageKey,
        decision.persistLastBroadcastAtMs,
      )
    }

    await this.broadcastScope(scope)
  }

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)
    const scope = syncScopeFromRequest(request, url)

    if (url.pathname === '/__sync/notify' && request.method === 'POST') {
      if (scope === undefined) {
        return badRequest('scope is required')
      }

      this.state.waitUntil(this.notifyScopeThrottled(scope))

      return jsonResponse({ ok: true, scope })
    }

    if (request.headers.get('upgrade')?.toLowerCase() !== 'websocket') {
      return badRequest('websocket upgrade is required')
    }

    if (scope === undefined) {
      return badRequest('scope is required')
    }

    const pair = new WebSocketPair()
    const client = pair[0]
    const server = pair[1]
    const cursor = syncCursorFromUrl(url)

    server.serializeAttachment({ cursor, scope })
    this.state.acceptWebSocket(server, [scope])
    this.state.waitUntil(this.replaySocket(server, scope, cursor))

    return new Response(null, {
      status: 101,
      webSocket: client,
    })
  }

  async webSocketMessage(socket: WebSocket, message: string): Promise<void> {
    if (message !== 'replay') {
      return
    }

    const attachment = syncSocketAttachment(socket.deserializeAttachment())

    if (attachment === undefined) {
      syncSocketClose(socket, 1008, 'missing sync attachment')
      return
    }

    await this.replaySocket(socket, attachment.scope, attachment.cursor)
  }

  webSocketClose(
    socket: WebSocket,
    _code: number,
    _reason: string,
    _wasClean: boolean,
  ): void {
    socket.serializeAttachment(null)
  }

  webSocketError(socket: WebSocket): void {
    syncSocketClose(socket, 1011, 'sync socket error')
  }
}

const workerFetchProgram = Effect.gen(function* () {
  const { ctx, env, request, url } = yield* OpenAgentsWorkerRequest

  return yield* Effect.gen(function* () {
    const siteRuntimeResponse = siteRuntimeRoutes.routeSiteRuntimeRequest(
      request,
      env,
    )

    if (siteRuntimeResponse !== undefined) {
      return yield* siteRuntimeResponse
    }

    if (url.hostname === 'auth.openagents.com') {
      return yield* Effect.promise(() =>
        routeAuthHostRequest(request, env, ctx),
      )
    }

    return yield* routeRequest()
  }).pipe(
    Effect.catchCause(cause =>
      Effect.sync(() => {
        logWorkerRouteError('worker_unhandled_exception', Cause.pretty(cause))

        if (url.hostname === 'auth.openagents.com') {
          return redirectResponse(getAppOrigin(env))
        }

        return serverError()
      }),
    ),
  )
})

const runWorkerFetch = (
  request: Request,
  env: Env,
  ctx: ExecutionContext,
): Promise<Response> =>
  Effect.runPromise(
    workerFetchProgram.pipe(
      Effect.provide(WorkerRequestLayer({ ctx, env, request })),
    ),
  )

export default {
  fetch: runWorkerFetch,
  queue: async (batch, env, ctx): Promise<void> => {
    // Whether the async batch-job consumer is armed. Default OFF: batch-job
    // messages are routed to `executeBatchJob` ONLY when this flag is on, so the
    // queue handler's behaviour for every existing message type is unchanged in
    // prod until the path is explicitly armed.
    const batchJobsEnabled = isInferenceGatewayEnabled(
      env.INFERENCE_BATCH_JOBS_ENABLED,
    )

    for (const message of batch.messages) {
      // Discriminate by the message's stable schema version so a batch-job
      // payload (Khala, EPIC #6017 / #6028) is routed to the inference batch
      // consumer OFF the request path, while every other payload keeps flowing
      // to the adjutant-enrichment executor exactly as before.
      const body = message.body
      const schemaVersion =
        typeof body === 'object' && body !== null && 'schemaVersion' in body
          ? (body as { schemaVersion?: unknown }).schemaVersion
          : undefined

      if (schemaVersion === 'openagents.inference.batch_job.v1') {
        // Inert unless armed: a batch-job message that arrives while the flag is
        // off is acked without execution (the submitted job stays pending, no
        // credit decrement, no behaviour change). When armed, the consumer runs
        // the job to completion against the gateway and writes the
        // dereferenceable closeout receipt.
        if (batchJobsEnabled) {
          const decoded = S.decodeUnknownSync(BatchJobQueueMessage)(body)
          const exit = await Effect.runPromiseExit(
            executeBatchJob(makeBatchJobConsumerDeps(env), decoded),
          )
          if (Exit.isFailure(exit)) {
            throw exit.cause
          }
        }
        message.ack()
        continue
      }

      const decoded = S.decodeUnknownSync(AdjutantEnrichmentQueueMessage)(body)

      const exit = await Effect.runPromiseExit(
        executeQueuedAdjutantEnrichmentJob(env, decoded),
      )

      if (Exit.isFailure(exit)) {
        throw exit.cause
      }

      message.ack()
    }
    void ctx
  },
  scheduled: async (event, env, ctx): Promise<void> => {
    const config = getOpenAgentsWorkerConfig(env)

    const glmPoolHeartbeatReport = await observedEffect(
      'HydraliskGlmPoolHeartbeat.run',
      runScheduledGlmPoolHeartbeatForD1({
        db: openAgentsDatabase(env),
        env,
        scheduledTimeMs: event.scheduledTime,
      }),
    )
    if (glmPoolHeartbeatReport.persistenceFailures.length > 0) {
      logWorkerRouteWarning('glm_pool_heartbeat_persistence_blocked', {
        blockerRef:
          'blocker.public.inference.glm_pool_heartbeat.token_usage_events_persistence_failed',
        failureCount: glmPoolHeartbeatReport.persistenceFailures.length,
        failureErrorTags: glmPoolHeartbeatReport.persistenceFailures
          .map(failure => failure.errorTag)
          .join(','),
        failureStages: glmPoolHeartbeatReport.persistenceFailures
          .map(failure => failure.stage)
          .join(','),
        runRef: glmPoolHeartbeatReport.runRef,
      })
    }

    await Promise.all([
      sweepActiveAgentRunBilling(env, ctx),
      sendPendingReviewReadyArtifactNotifications(env),
      sendPendingReviewReadySiteNotifications(env),
      // #6408: "fleet never silently stalls" watchdog. Every minute, measure the
      // own-capacity Codex burn rate vs active coding leases; on a stall (burn
      // below threshold WHILE work is leased) write a loud fleet_alerts row and
      // auto-flush abandoned leases that poison the dispatch gate. Idle-no-work
      // and healthy ticks are silent. Does NOT touch the dispatch-gate dedup.
      observedEffect(
        'FleetBurnStallDetector.tick',
        Effect.promise(() =>
          runFleetBurnStallDetectorScheduled(
            openAgentsDatabase(env),
            env,
            { scheduledTimeMs: event.scheduledTime },
            (line, fields) =>
              logWorkerRouteWarning('fleet_burn_stall_watchdog', {
                line,
                ...fields,
              }),
          ),
        ),
      ),
      observedEffect(
        'ArtanisScheduledRunner.runTick',
        runArtanisScheduledTickScheduled(
          openAgentsDatabase(env),
          config.artanis.scheduledRunnerEnabled,
          event.scheduledTime,
        ),
      ),
      observedEffect(
        'PylonCapacityFunnel.recordSnapshots',
        recordPylonCapacityFunnelSnapshotsScheduled(
          openAgentsDatabase(env),
          event.scheduledTime,
        ),
      ),
      observedEffect(
        'RelayHealth.probeTick',
        runRelayHealthProbeScheduled(env, event.scheduledTime),
      ),
      observedEffect(
        'SelfServeWindowProducer.topUp',
        runSelfServeWindowProducerScheduled(env, event.scheduledTime),
      ),
      observedEffect(
        'EmailCampaignDispatcher.dispatchDue',
        dispatchDueEmailCampaignSendsScheduled(env),
      ),
      observedEffect(
        'AutopilotScheduledLaunches.dispatchDue',
        dispatchDueScheduledAutopilotWork(autopilotWorkRouteDependencies, env, {
          nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
        }),
      ),
      observedEffect(
        'AutopilotContinuationPolicy.sweep',
        runAutopilotContinuationSweep({
          billingAllowsContinuation: async userId => {
            const result = await requireMinimumRunCredits(
              openAgentsDatabase(env),
              userId,
            )

            return result.ok
              ? { ok: true, reasonRef: 'continuation.billing_ok' }
              : {
                  ok: false,
                  reasonRef: 'continuation.skipped.billing_blocked',
                }
          },
          dispatchFollowUpTurn: async candidate => {
            const result = await omniHandlers.continueUserAutopilotRun(
              env,
              ctx,
              {
                prompt:
                  'Continue the active OpenAgents goal from the latest durable run state.',
                runId: candidate.runId,
                userId: candidate.userId,
              },
            )

            return result.ok
              ? {
                  ok: true,
                  reasonRef: 'continuation.dispatched.follow_up_turn',
                }
              : {
                  ok: false,
                  reasonRef: 'continuation.failed.follow_up_turn',
                }
          },
          dispatchGoalContinuation: async candidate => {
            await omniHandlers.requestGoalContinuationAfterCompletedRun(
              env,
              ctx,
              candidate.runId,
            )

            return {
              ok: true,
              reasonRef: 'continuation.dispatched.goal_continuation',
            }
          },
          listStoppedRunsForUser: (userId, sinceIso, limit) =>
            listAutopilotContinuationRunCandidates(openAgentsDatabase(env), {
              limit,
              sinceIso,
              userId,
            }),
          nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
          store: makeD1AutopilotContinuationStore(openAgentsDatabase(env)),
        }),
      ),
      observedEffect(
        'TipsSweep.runTick',
        runTipsSweepScheduled(openAgentsDatabase(env), {
          makeId: randomUuid,
          nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
          payFromBuffer: tipsBufferPayFnForEnv(env),
        }),
      ),
      observedEffect(
        'XClaimRewardTreasuryDispatcher.runTick',
        runXClaimRewardTreasuryDispatchScheduled(openAgentsDatabase(env), {
          config: readXClaimRewardTreasuryDispatchConfig(
            env,
            epochMillisToIsoTimestamp(event.scheduledTime),
          ),
          fetchTreasury: fetchMdkTreasuryPath(env),
        }),
      ),
      observedEffect(
        'ArtanisResponder.scan',
        runArtanisResponderScanScheduled(openAgentsDatabase(env), {
          artanisActorRefs: [ARTANIS_REGISTERED_ACTOR_REF],
          enabled:
            (env as { ARTANIS_FORUM_RESPONDER_ENABLED?: string })
              .ARTANIS_FORUM_RESPONDER_ENABLED === 'true',
          gatewayToken: (env as { CF_AIG_TOKEN?: string }).CF_AIG_TOKEN,
          geminiApiKey:
            (env as { GEMINI_API_KEY?: string }).GEMINI_API_KEY ?? null,
          khalaClient: makeArtanisResponderKhalaClient(env),
          nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
        }),
      ),
      observedEffect(
        'ArtanisAdmin.tick',
        runArtanisAdminTickScheduled(openAgentsDatabase(env), {
          dispatch: async body => {
            const adminToken = (env as { OPENAGENTS_ADMIN_API_TOKEN?: string })
              .OPENAGENTS_ADMIN_API_TOKEN
            if (adminToken === undefined) {
              return { detail: 'admin_token_missing', ok: false }
            }
            const response = await runArtanisForumRouteEffect(
              pylonApiRoutes.routePylonApiRequest(
                new Request(
                  'https://openagents.com/api/operator/pylons/assignments',
                  {
                    body: JSON.stringify(body),
                    headers: {
                      Authorization: `Bearer ${adminToken}`,
                      'Content-Type': 'application/json',
                      'Idempotency-Key': String(
                        (body as { assignmentRef?: string }).assignmentRef ??
                          'artanis-admin-dispatch',
                      ),
                    },
                    method: 'POST',
                  },
                ),
                env,
                ctx,
              ),
            )
            if (response === undefined) {
              return { detail: 'route_unmatched', ok: false }
            }
            const detail = (await response.text()).slice(0, 200)
            return { detail, ok: response.ok }
          },
          enabled:
            (env as { ARTANIS_ADMIN_TICK_ENABLED?: string })
              .ARTANIS_ADMIN_TICK_ENABLED === 'true',
          gatewayToken: (env as { CF_AIG_TOKEN?: string }).CF_AIG_TOKEN,
          geminiApiKey:
            (env as { GEMINI_API_KEY?: string }).GEMINI_API_KEY ?? null,
          nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
        }),
      ),
      observedEffect(
        'ArtanisAdmin.closeoutVerifier',
        runArtanisCloseoutVerifierScheduled(openAgentsDatabase(env), {
          accept: async input => {
            const adminToken = (env as { OPENAGENTS_ADMIN_API_TOKEN?: string })
              .OPENAGENTS_ADMIN_API_TOKEN
            if (adminToken === undefined) {
              return { detail: 'admin_token_missing', ok: false }
            }
            const response = await runArtanisForumRouteEffect(
              pylonApiRoutes.routePylonApiRequest(
                new Request(
                  `https://openagents.com/api/operator/pylons/assignments/${encodeURIComponent(input.assignmentRef)}/closeout`,
                  {
                    body: JSON.stringify({
                      accepted: input.accepted,
                      acceptedWorkRefs: input.accepted ? input.refs : [],
                      closeoutRefs: input.refs,
                      rejectionRefs: input.accepted ? [] : input.refs,
                    }),
                    headers: {
                      Authorization: `Bearer ${adminToken}`,
                      'Content-Type': 'application/json',
                      'Idempotency-Key': `artanis-closeout-${input.assignmentRef}`,
                    },
                    method: 'POST',
                  },
                ),
                env,
                ctx,
              ),
            )
            if (response === undefined) {
              return { detail: 'route_unmatched', ok: false }
            }
            return {
              detail: (await response.text()).slice(0, 200),
              ok: response.ok,
            }
          },
          enabled:
            (env as { ARTANIS_ADMIN_TICK_ENABLED?: string })
              .ARTANIS_ADMIN_TICK_ENABLED === 'true',
          nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
          replay: async input =>
            runTassadarReplayValidation({
              assignmentRef: input.assignmentRef,
              claimedTraceDigest: input.claimedTraceDigest,
              pylonDeviceRef: input.pylonDeviceRef,
              workload: input.workload,
            } as never),
        }),
      ),
      observedEffect(
        // #5053 (epic #5051): worker -> validator pairing orchestration
        // (Artanis-first, design §4.3 option B). INERT BY DEFAULT: enabled only
        // when TASSADAR_TRACE_PAIRING === '1', so this changes no live tick
        // behavior until the #5061 dry-run deliberately enables it. The candidate
        // resolver yields no validator devices yet (the #5061 dry-run supplies the
        // distinct-device replay evidence); device-distinctness + no-double-pair
        // are enforced by the orchestration and the conditional store update.
        'TassadarTracePairing.tick',
        runTassadarTracePairingScheduled({
          createVerificationChallenge: request => {
            const built = buildTrainingVerificationChallengeRecord({
              makeId: randomUuid,
              nowIso: currentIsoTimestamp(),
              request,
            })

            return makeD1TrainingVerificationStore(
              openAgentsDatabase(env),
            ).createChallenge(built.challenge, built.event)
          },
          enabled:
            (env as { TASSADAR_TRACE_PAIRING?: string })
              .TASSADAR_TRACE_PAIRING === '1',
          nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
          // Intentionally empty (#5121). The trust anchor is a separate-device
          // replay, so the server must NEVER fabricate a validator's replay
          // digest — a server-assigned candidate would have to carry a digest it
          // did not compute. Verdicts are produced by the validator PUSH path
          // instead: a validator node auto-discovers the next unpaired
          // contribution (GET /api/training/contributions/next-unpaired), replays
          // the committed fixture on its own distinct device, and POSTs the
          // digest to /replay-verdict (which pairs + builds the exact_trace_replay
          // challenge). This scheduled tick stays a no-op pairer by design; do not
          // wire it to a digest source.
          resolveValidatorCandidates: async () => [],
          store: makeD1TrainingTraceContributionStore(openAgentsDatabase(env)),
        }),
      ),
      observedEffect(
        'ArtanisResponder.compose',
        runArtanisComposerScheduled(openAgentsDatabase(env), {
          artanisActorRef: ARTANIS_REGISTERED_ACTOR_REF,
          enabled:
            (env as { ARTANIS_FORUM_RESPONDER_ENABLED?: string })
              .ARTANIS_FORUM_RESPONDER_ENABLED === 'true',
          forumPost: artanisComposerForumPostForEnv(env),
          gatewayToken: (env as { CF_AIG_TOKEN?: string }).CF_AIG_TOKEN,
          geminiApiKey:
            (env as { GEMINI_API_KEY?: string }).GEMINI_API_KEY ?? null,
          nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
          tip: artanisComposerTipForEnv(env),
        }),
      ),
      observedEffect(
        'TipsBuffer.reconcileForwarding',
        Effect.promise(() =>
          reconcileForwardingBufferPayments(openAgentsDatabase(env), {
            fetchBufferPaymentStatus: async paymentId => {
              const fetchBuffer = fetchMdkTipsBufferPath(env)
              if (fetchBuffer === undefined) {
                return 'pending'
              }
              try {
                const response = await fetchBuffer(
                  `/payments/${encodeURIComponent(paymentId)}`,
                )
                const body = (await response.json()) as { status?: string }
                return body.status === 'succeeded'
                  ? 'succeeded'
                  : body.status === 'failed'
                    ? 'failed'
                    : 'pending'
              } catch {
                return 'pending'
              }
            },
            makeId: randomUuid,
            nowIso: epochMillisToIsoTimestamp(event.scheduledTime),
          }),
        ),
      ),
      observedEffect(
        'TreasuryTransactions.reconcilePending',
        Effect.promise(() =>
          reconcilePendingTreasuryTransactions({
            fetchTipsBuffer: fetchMdkTipsBufferPath(env),
            fetchTreasury: fetchMdkTreasuryPath(env),
            limit: 25,
            transactionStore: makeD1TreasuryTransactionStore(
              openAgentsDatabase(env),
            ),
          }),
        ),
      ),
      observedEffect(
        'ForumDirectTips.archiveStaleRecoveries',
        Effect.promise(() =>
          archiveStaleDirectTipRecoveries(
            openAgentsDatabase(env),
            epochMillisToIsoTimestamp(event.scheduledTime),
          ),
        ),
      ),
      observedEffect(
        'TipsBuffer.backingInvariant',
        Effect.promise(() =>
          checkTipsBufferBackingInvariant(openAgentsDatabase(env), async () => {
            const fetchBuffer = fetchMdkTipsBufferPath(env)
            if (fetchBuffer === undefined) {
              return null
            }
            try {
              const response = await fetchBuffer('/balance')
              const body = (await response.json()) as {
                maxSendableSat?: number
                balanceSat?: number
              }
              return Number(body.maxSendableSat ?? body.balanceSat ?? 0)
            } catch {
              return null
            }
          }),
        ),
      ),
    ])
  },
} satisfies ExportedHandler<Env>

import { CursorGap, SyncPatch } from '@openagentsinc/sync-schema'
import { Schema as S } from 'effect'
import { m } from 'foldkit/message'

import { OnboardingTurnResponse } from '../autopilot-onboarding/flow'
import {
  OnboardingSessionResponse,
  StoredOnboardingSession,
} from '../autopilot-onboarding/persistence'
import {
  OnboardingStep,
  PublicAdjutantActivity,
  PublicAgentGoalResponse,
  PublicArtanisReport,
  PublicForumLaunchStatus,
  PublicForumTipLeaderboards,
  PublicKhalaTokensServed,
  PublicKhalaTokensServedHistory,
  PublicKhalaTokensServedModelMix,
  PublicProductPromises,
  PublicPromiseTransitions,
  PublicPylonStats,
  PublicTrainingRunsResponse,
  ShareProjectionResponse,
} from './model'
import { BlobRef as AtifBlobRef, Trajectory as AtifTrajectory } from '../trace/atif'
import {
  GymCoordinatorCandidateRef,
  GymFanoutMode,
  GymLaneRef,
  GymModuleCompositionMode,
  GymReasoningEffort,
  GymSequenceShapeRef,
  GymToolSetRef,
  GymTransport,
} from './gym/flow'
import { GymRunProgressPublicProjection } from './gym/runProgress'
import { MirrorCodeRunsResponse } from './mirrorcode/runs'

// MESSAGE

export const ClickedCopyShareLink = m('ClickedCopyShareLink', {
  url: S.String,
})
export const ClickedEnterKhala = m('ClickedEnterKhala')
export const CompletedNavigateToKhala = m('CompletedNavigateToKhala')
export const ClickedExitKhala = m('ClickedExitKhala')
export const ClickedEnterTassadar = m('ClickedEnterTassadar')
export const CompletedNavigateToTassadar = m('CompletedNavigateToTassadar')
export const ClickedCopyAgentInstructions = m('ClickedCopyAgentInstructions', {
  text: S.String,
})
export const CompletedCopyAgentInstructions = m(
  'CompletedCopyAgentInstructions',
)
export const CompletedNavigateToLanding = m('CompletedNavigateToLanding')
// Log out from the chrome-less homepage hero's floating avatar menu. Reuses the
// exact same logout behavior as the public header: clear the cached session,
// then full-page navigate to `/auth/logout`. `CompletedLandingLogout` is a
// benign no-op completion that the full-page navigation effectively preempts.
export const RequestedLandingLogout = m('RequestedLandingLogout')
export const CompletedLandingLogout = m('CompletedLandingLogout')
export const ClickedOnboardingStep = m('ClickedOnboardingStep', {
  step: OnboardingStep,
})
export const SelectedOnboardingRepository = m('SelectedOnboardingRepository', {
  repository: S.String,
})
export const SkippedOnboardingRepository = m('SkippedOnboardingRepository')
export const UpdatedOnboardingFundingAmount = m(
  'UpdatedOnboardingFundingAmount',
  { value: S.String },
)
export const ToggledOnboardingCoupon = m('ToggledOnboardingCoupon')
export const UpdatedOnboardingCouponCode = m('UpdatedOnboardingCouponCode', {
  value: S.String,
})
export const ToggledGymLane = m('ToggledGymLane', {
  lane: GymLaneRef,
})
export const UpdatedGymFanoutMode = m('UpdatedGymFanoutMode', {
  mode: GymFanoutMode,
})
export const UpdatedGymConcurrency = m('UpdatedGymConcurrency', {
  value: S.String,
})
export const UpdatedGymToolSet = m('UpdatedGymToolSet', {
  tools: GymToolSetRef,
})
export const UpdatedGymModuleComposition = m(
  'UpdatedGymModuleComposition',
  {
    mode: GymModuleCompositionMode,
  },
)
export const ToggledGymCoordinator = m('ToggledGymCoordinator', {
  candidate: GymCoordinatorCandidateRef,
})
export const UpdatedGymTemperature = m('UpdatedGymTemperature', {
  value: S.String,
})
export const UpdatedGymReasoningEffort = m('UpdatedGymReasoningEffort', {
  reasoningEffort: GymReasoningEffort,
})
export const UpdatedGymMaxTokens = m('UpdatedGymMaxTokens', {
  value: S.String,
})
export const UpdatedGymTransport = m('UpdatedGymTransport', {
  transport: GymTransport,
})
export const ToggledGymSequenceShape = m('ToggledGymSequenceShape', {
  shape: GymSequenceShapeRef,
})
export const UpdatedGymSamplesPerCell = m('UpdatedGymSamplesPerCell', {
  value: S.String,
})
export const SucceededLoadPublicAgentGoal = m('SucceededLoadPublicAgentGoal', {
  agentRef: S.String,
  response: PublicAgentGoalResponse,
})
export const FailedLoadPublicAgentGoal = m('FailedLoadPublicAgentGoal', {
  agentRef: S.String,
  error: S.String,
})
export const SucceededLoadPublicArtanisReport = m(
  'SucceededLoadPublicArtanisReport',
  {
    report: PublicArtanisReport,
  },
)
export const FailedLoadPublicArtanisReport = m(
  'FailedLoadPublicArtanisReport',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicAdjutantActivity = m(
  'SucceededLoadPublicAdjutantActivity',
  {
    activity: PublicAdjutantActivity,
  },
)
export const FailedLoadPublicAdjutantActivity = m(
  'FailedLoadPublicAdjutantActivity',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicPylonStats = m(
  'SucceededLoadPublicPylonStats',
  {
    stats: PublicPylonStats,
  },
)
export const FailedLoadPublicPylonStats = m('FailedLoadPublicPylonStats', {
  error: S.String,
})
// "Khala Tokens Served" homepage counter (#6227). The poll subscription fires
// RequestedPollKhalaTokensServed on a short interval; the command resolves to
// Succeeded/Failed and the odometer animates between fetched totals.
export const RequestedPollKhalaTokensServed = m(
  'RequestedPollKhalaTokensServed',
)
export const SucceededLoadPublicKhalaTokensServed = m(
  'SucceededLoadPublicKhalaTokensServed',
  {
    served: PublicKhalaTokensServed,
  },
)
export const FailedLoadPublicKhalaTokensServed = m(
  'FailedLoadPublicKhalaTokensServed',
  {
    error: S.String,
  },
)
// "Khala Tokens Served" snapshot seed (#6231 follow-up). One snapshot read of the
// public sync scope returns the AUTHORITATIVE running total (the room's `summary`
// record) + the cursor. The client seeds from this and subscribes strictly from
// that cursor, so events already baked into the seed are never replayed-and-added
// — no double-count, no backward jump.
export const SucceededLoadKhalaTokensServedSnapshot = m(
  'SucceededLoadKhalaTokensServedSnapshot',
  {
    cursor: S.Number,
    summary: S.NullOr(
      S.Struct({
        observedAt: S.String,
        tokensServedTotal: S.Number,
      }),
    ),
  },
)
export const FailedLoadKhalaTokensServedSnapshot = m(
  'FailedLoadKhalaTokensServedSnapshot',
  {
    error: S.String,
  },
)
// "Khala Tokens Served" history (#6227). The same poll subscription tick that
// re-fetches the scalar counter also re-fetches the per-day history series for
// the /stats chart; the command resolves to Succeeded/Failed.
export const RequestedPollKhalaTokensServedHistory = m(
  'RequestedPollKhalaTokensServedHistory',
)
export const SucceededLoadPublicKhalaTokensServedHistory = m(
  'SucceededLoadPublicKhalaTokensServedHistory',
  {
    history: PublicKhalaTokensServedHistory,
  },
)
export const FailedLoadPublicKhalaTokensServedHistory = m(
  'FailedLoadPublicKhalaTokensServedHistory',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicKhalaTokensServedModelMix = m(
  'SucceededLoadPublicKhalaTokensServedModelMix',
  {
    mix: PublicKhalaTokensServedModelMix,
  },
)
export const FailedLoadPublicKhalaTokensServedModelMix = m(
  'FailedLoadPublicKhalaTokensServedModelMix',
  {
    error: S.String,
  },
)
// Live Gym / Harbor run-progress follow-along (#6261). PUSH is the primary path:
// the panel seeds its run cards + cursor from ONE sync snapshot read on `/gym`
// entry, then upserts each run card the instant a snapshot is ingested over the
// `public-gym-run-progress` scope. The poll below is now only a SLOW socket-down
// reconcile/fallback; the model holds its last loaded runs between ticks (no
// flash to Loading).
export const RequestedPollGymRunProgress = m('RequestedPollGymRunProgress')
export const SucceededLoadPublicGymRunProgress = m(
  'SucceededLoadPublicGymRunProgress',
  {
    runs: S.Array(GymRunProgressPublicProjection),
  },
)
export const FailedLoadPublicGymRunProgress = m(
  'FailedLoadPublicGymRunProgress',
  {
    error: S.String,
  },
)
// One-shot seed of the gym run-progress run cards + cursor from the public sync
// snapshot. Subscribing strictly from that cursor means a put already baked into
// the seed is never replayed into a duplicate card.
export const SucceededLoadGymRunProgressSnapshot = m(
  'SucceededLoadGymRunProgressSnapshot',
  {
    cursor: S.Number,
    runs: S.Array(GymRunProgressPublicProjection),
  },
)
export const FailedLoadGymRunProgressSnapshot = m(
  'FailedLoadGymRunProgressSnapshot',
  {
    error: S.String,
  },
)
// Live gym run-progress delta stream (#6261). The `/gym` panel subscribes to a
// public sync scope and replaces each run card the instant a public-safe
// projected snapshot is ingested — no per-12s poll.
export const OpenedGymRunProgressStream = m('OpenedGymRunProgressStream')
export const ClosedGymRunProgressStream = m('ClosedGymRunProgressStream')
export const FailedGymRunProgressStream = m('FailedGymRunProgressStream', {
  error: S.String,
})
export const ReceivedGymRunProgressPatch = m('ReceivedGymRunProgressPatch', {
  patch: SyncPatch,
})
export const ReceivedGymRunProgressCursorGap = m(
  'ReceivedGymRunProgressCursorGap',
  {
    gap: CursorGap,
  },
)
// MirrorCode runs (#6378). The `/mirrorcode` route cold-reads
// `GET /api/gym/mirrorcode/runs` once on entry; these carry the decoded
// projection or a public-safe error string.
export const SucceededLoadMirrorCodeRuns = m('SucceededLoadMirrorCodeRuns', {
  response: MirrorCodeRunsResponse,
})
export const FailedLoadMirrorCodeRuns = m('FailedLoadMirrorCodeRuns', {
  error: S.String,
})
export const SucceededLoadPublicForumLaunchStatus = m(
  'SucceededLoadPublicForumLaunchStatus',
  {
    status: PublicForumLaunchStatus,
  },
)
export const FailedLoadPublicForumLaunchStatus = m(
  'FailedLoadPublicForumLaunchStatus',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicForumTipLeaderboards = m(
  'SucceededLoadPublicForumTipLeaderboards',
  {
    leaderboards: PublicForumTipLeaderboards,
  },
)
export const FailedLoadPublicForumTipLeaderboards = m(
  'FailedLoadPublicForumTipLeaderboards',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicProductPromises = m(
  'SucceededLoadPublicProductPromises',
  {
    promises: PublicProductPromises,
  },
)
export const FailedLoadPublicProductPromises = m(
  'FailedLoadPublicProductPromises',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicPromiseTransitions = m(
  'SucceededLoadPublicPromiseTransitions',
  {
    transitions: PublicPromiseTransitions,
  },
)
export const FailedLoadPublicPromiseTransitions = m(
  'FailedLoadPublicPromiseTransitions',
  {
    error: S.String,
  },
)
export const SucceededLoadPublicTrainingRuns = m(
  'SucceededLoadPublicTrainingRuns',
  {
    response: PublicTrainingRunsResponse,
    selectedRunId: S.NullOr(S.String),
  },
)
export const FailedLoadPublicTrainingRuns = m('FailedLoadPublicTrainingRuns', {
  error: S.String,
  runId: S.NullOr(S.String),
})
export const SucceededLoadShareProjection = m('SucceededLoadShareProjection', {
  response: ShareProjectionResponse,
  shareId: S.String,
})
export const FailedLoadShareProjection = m('FailedLoadShareProjection', {
  error: S.String,
  shareId: S.String,
  status: S.Int,
})
export const CompletedCopyShareLink = m('CompletedCopyShareLink', {
  url: S.String,
})
// Live `/trace/{uuid}` read (issue #6209). The success carries the decoded
// public-safe ATIF trajectory; the failure carries the HTTP `status` so a clean
// 404 (not found / not public) is distinguished from a transport/decode error.
export const SucceededLoadTrace = m('SucceededLoadTrace', {
  uuid: S.String,
  trajectory: AtifTrajectory,
  // Public-safe envelope blob refs (#6223) for the trace's R2 media.
  blobRefs: S.optionalKey(S.Array(AtifBlobRef)),
})
export const FailedLoadTrace = m('FailedLoadTrace', {
  uuid: S.String,
  error: S.String,
  status: S.Int,
})
// Live settled feed stream (openagents #5311).
export const SucceededLoadSettledFeedSnapshot = m(
  'SucceededLoadSettledFeedSnapshot',
  {
    cursor: S.Number,
    summary: S.NullOr(
      S.Struct({
        totalSettledCount: S.Number,
        totalSettledSats: S.Number,
      }),
    ),
  },
)
export const FailedLoadSettledFeedSnapshot = m(
  'FailedLoadSettledFeedSnapshot',
  {
    error: S.String,
  },
)
export const OpenedSettledFeedStream = m('OpenedSettledFeedStream')
export const ClosedSettledFeedStream = m('ClosedSettledFeedStream')
export const FailedSettledFeedStream = m('FailedSettledFeedStream', {
  error: S.String,
})
export const ReceivedSettledFeedPatch = m('ReceivedSettledFeedPatch', {
  patch: SyncPatch,
})
export const ReceivedSettledFeedCursorGap = m('ReceivedSettledFeedCursorGap', {
  gap: CursorGap,
})
// Live "Khala Tokens Served" delta stream (openagents #6231). The homepage
// subscribes to a public sync scope and rolls the odometer up instantly as each
// served completion pushes a public-safe delta — no per-second poll/SUM.
export const OpenedKhalaTokensServedStream = m('OpenedKhalaTokensServedStream')
export const ClosedKhalaTokensServedStream = m('ClosedKhalaTokensServedStream')
export const FailedKhalaTokensServedStream = m(
  'FailedKhalaTokensServedStream',
  {
    error: S.String,
  },
)
export const ReceivedKhalaTokensServedPatch = m(
  'ReceivedKhalaTokensServedPatch',
  {
    patch: SyncPatch,
  },
)
export const ReceivedKhalaTokensServedCursorGap = m(
  'ReceivedKhalaTokensServedCursorGap',
  {
    gap: CursorGap,
  },
)

// /autopilot onboarding conversation (#6129).
export const UpdatedAutopilotOnboardingComposer = m(
  'UpdatedAutopilotOnboardingComposer',
  { value: S.String },
)
export const SubmittedAutopilotOnboardingTurn = m(
  'SubmittedAutopilotOnboardingTurn',
)
export const OpenedAutopilotOnboardingStream = m(
  'OpenedAutopilotOnboardingStream',
  { turnId: S.String },
)
export const ReceivedAutopilotOnboardingDelta = m(
  'ReceivedAutopilotOnboardingDelta',
  { turnId: S.String, text: S.String },
)
export const SucceededAutopilotOnboardingTurn = m(
  'SucceededAutopilotOnboardingTurn',
  { response: OnboardingTurnResponse },
)
export const FailedAutopilotOnboardingTurn = m(
  'FailedAutopilotOnboardingTurn',
  { reason: S.String },
)
export const ClickedAutopilotOnboardingCreditKickoff = m(
  'ClickedAutopilotOnboardingCreditKickoff',
)
export const CompletedAutopilotOnboardingCreditKickoff = m(
  'CompletedAutopilotOnboardingCreditKickoff',
)
export const CompletedScrollAutopilotOnboardingThread = m(
  'CompletedScrollAutopilotOnboardingThread',
)

// /autopilot onboarding — browser persistence + durable resume (#6154 tier 4).
// The live-stream handshake frame (`event: stream`) carries the durable cursor
// so a reload can resume the in-flight turn from the durable log.
export const ReceivedAutopilotOnboardingStreamHandshake = m(
  'ReceivedAutopilotOnboardingStreamHandshake',
  {
    turnId: S.String,
    streamId: S.String,
    sessionId: S.String,
    turnIndex: S.Int,
  },
)
// Rehydrate from localStorage on mount: restore the saved transcript/spec/cursor
// immediately (no blank flash) before reconciling with the server.
export const LoadedStoredAutopilotOnboarding = m(
  'LoadedStoredAutopilotOnboarding',
  { session: StoredOnboardingSession },
)
// Reconcile with the authoritative server session (covers a turn that completed
// while the tab was gone). 404 => expired/unknown => clear + fresh start.
export const SucceededReconcileAutopilotOnboardingSession = m(
  'SucceededReconcileAutopilotOnboardingSession',
  { response: OnboardingSessionResponse },
)
export const FailedReconcileAutopilotOnboardingSession = m(
  'FailedReconcileAutopilotOnboardingSession',
  { status: S.Int },
)
// Resume read (durable replay) of an in-flight turn on reload. Deltas REPLACE
// the bubble (the replay re-streams the whole turn from the offset), advancing
// the persisted offset from the `stream-next-offset` header.
export const ReceivedAutopilotOnboardingResumeReply = m(
  'ReceivedAutopilotOnboardingResumeReply',
  { turnIndex: S.Int, reply: S.String, nextOffset: S.NullOr(S.String) },
)
export const SucceededResumeAutopilotOnboardingTurn = m(
  'SucceededResumeAutopilotOnboardingTurn',
  { response: OnboardingTurnResponse },
)
export const ClosedAutopilotOnboardingResumeStream = m(
  'ClosedAutopilotOnboardingResumeStream',
  { turnIndex: S.Int },
)
// The resume read is unavailable (durable log gone / TTL expired / 404): fall
// back to the reconciled transcript without leaving a stuck half-bubble.
export const FailedAutopilotOnboardingResume = m(
  'FailedAutopilotOnboardingResume',
  { turnIndex: S.Int },
)
// The "start over" affordance: drop the in-memory flow + the stored session.
export const ClickedAutopilotOnboardingStartOver = m(
  'ClickedAutopilotOnboardingStartOver',
)
// Fire-and-forget completion of a localStorage persist/clear side effect.
export const CompletedPersistAutopilotOnboarding = m(
  'CompletedPersistAutopilotOnboarding',
)

// GENERIC /khala CHAT (a minimal stateless streaming chat — NOT the concierge
// intake). The composer + turn lifecycle mirror the onboarding stream messages,
// minus the persistence/resume/output-spec machinery. The info-popup toggle is
// its own pair.
export const UpdatedKhalaChatComposer = m('UpdatedKhalaChatComposer', {
  value: S.String,
})
export const SubmittedKhalaChatTurn = m('SubmittedKhalaChatTurn')
export const OpenedKhalaChatStream = m('OpenedKhalaChatStream', {
  turnId: S.String,
})
export const ReceivedKhalaChatDelta = m('ReceivedKhalaChatDelta', {
  turnId: S.String,
  text: S.String,
})
export const SucceededKhalaChatTurn = m('SucceededKhalaChatTurn', {
  turnId: S.String,
})
export const FailedKhalaChatTurn = m('FailedKhalaChatTurn', {
  reason: S.String,
})
export const CompletedScrollKhalaChatThread = m('CompletedScrollKhalaChatThread')
export const OpenedKhalaChatInfo = m('OpenedKhalaChatInfo')
export const ClosedKhalaChatInfo = m('ClosedKhalaChatInfo')

export const Message = S.Union([
  ClickedCopyShareLink,
  ClickedEnterKhala,
  CompletedNavigateToKhala,
  ClickedExitKhala,
  ClickedEnterTassadar,
  CompletedNavigateToTassadar,
  ClickedCopyAgentInstructions,
  CompletedCopyAgentInstructions,
  CompletedNavigateToLanding,
  RequestedLandingLogout,
  CompletedLandingLogout,
  ClickedOnboardingStep,
  SelectedOnboardingRepository,
  SkippedOnboardingRepository,
  UpdatedOnboardingFundingAmount,
  ToggledOnboardingCoupon,
  UpdatedOnboardingCouponCode,
  ToggledGymLane,
  UpdatedGymFanoutMode,
  UpdatedGymConcurrency,
  UpdatedGymToolSet,
  UpdatedGymModuleComposition,
  ToggledGymCoordinator,
  UpdatedGymTemperature,
  UpdatedGymReasoningEffort,
  UpdatedGymMaxTokens,
  UpdatedGymTransport,
  ToggledGymSequenceShape,
  UpdatedGymSamplesPerCell,
  SucceededLoadPublicAgentGoal,
  FailedLoadPublicAgentGoal,
  SucceededLoadPublicArtanisReport,
  FailedLoadPublicArtanisReport,
  SucceededLoadPublicAdjutantActivity,
  FailedLoadPublicAdjutantActivity,
  SucceededLoadPublicPylonStats,
  FailedLoadPublicPylonStats,
  RequestedPollKhalaTokensServed,
  SucceededLoadPublicKhalaTokensServed,
  FailedLoadPublicKhalaTokensServed,
  SucceededLoadKhalaTokensServedSnapshot,
  FailedLoadKhalaTokensServedSnapshot,
  RequestedPollKhalaTokensServedHistory,
  SucceededLoadPublicKhalaTokensServedHistory,
  FailedLoadPublicKhalaTokensServedHistory,
  SucceededLoadPublicKhalaTokensServedModelMix,
  FailedLoadPublicKhalaTokensServedModelMix,
  RequestedPollGymRunProgress,
  SucceededLoadPublicGymRunProgress,
  FailedLoadPublicGymRunProgress,
  SucceededLoadGymRunProgressSnapshot,
  FailedLoadGymRunProgressSnapshot,
  OpenedGymRunProgressStream,
  ClosedGymRunProgressStream,
  FailedGymRunProgressStream,
  ReceivedGymRunProgressPatch,
  ReceivedGymRunProgressCursorGap,
  SucceededLoadMirrorCodeRuns,
  FailedLoadMirrorCodeRuns,
  SucceededLoadPublicForumLaunchStatus,
  FailedLoadPublicForumLaunchStatus,
  SucceededLoadPublicForumTipLeaderboards,
  FailedLoadPublicForumTipLeaderboards,
  SucceededLoadPublicProductPromises,
  FailedLoadPublicProductPromises,
  SucceededLoadPublicPromiseTransitions,
  FailedLoadPublicPromiseTransitions,
  SucceededLoadPublicTrainingRuns,
  FailedLoadPublicTrainingRuns,
  SucceededLoadShareProjection,
  FailedLoadShareProjection,
  SucceededLoadTrace,
  FailedLoadTrace,
  CompletedCopyShareLink,
  SucceededLoadSettledFeedSnapshot,
  FailedLoadSettledFeedSnapshot,
  OpenedSettledFeedStream,
  ClosedSettledFeedStream,
  FailedSettledFeedStream,
  ReceivedSettledFeedPatch,
  ReceivedSettledFeedCursorGap,
  OpenedKhalaTokensServedStream,
  ClosedKhalaTokensServedStream,
  FailedKhalaTokensServedStream,
  ReceivedKhalaTokensServedPatch,
  ReceivedKhalaTokensServedCursorGap,
  UpdatedAutopilotOnboardingComposer,
  SubmittedAutopilotOnboardingTurn,
  OpenedAutopilotOnboardingStream,
  ReceivedAutopilotOnboardingDelta,
  SucceededAutopilotOnboardingTurn,
  FailedAutopilotOnboardingTurn,
  ClickedAutopilotOnboardingCreditKickoff,
  CompletedAutopilotOnboardingCreditKickoff,
  CompletedScrollAutopilotOnboardingThread,
  ReceivedAutopilotOnboardingStreamHandshake,
  LoadedStoredAutopilotOnboarding,
  SucceededReconcileAutopilotOnboardingSession,
  FailedReconcileAutopilotOnboardingSession,
  ReceivedAutopilotOnboardingResumeReply,
  SucceededResumeAutopilotOnboardingTurn,
  ClosedAutopilotOnboardingResumeStream,
  FailedAutopilotOnboardingResume,
  ClickedAutopilotOnboardingStartOver,
  CompletedPersistAutopilotOnboarding,
  UpdatedKhalaChatComposer,
  SubmittedKhalaChatTurn,
  OpenedKhalaChatStream,
  ReceivedKhalaChatDelta,
  SucceededKhalaChatTurn,
  FailedKhalaChatTurn,
  CompletedScrollKhalaChatThread,
  OpenedKhalaChatInfo,
  ClosedKhalaChatInfo,
])
export type Message = typeof Message.Type

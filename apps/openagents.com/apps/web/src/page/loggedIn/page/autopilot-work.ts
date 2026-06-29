// Forge cockpit — the operator surface for software-factory work.
//
// This view is a reframing of the prior operator UI onto the shared
// `@openagentsinc/ui` library (the `AiElements` family) plus the Forge object
// model from `products/forge.md`:
//
//   - work orders / sessions   -> Runs (one execution attempt against a request)
//   - the provider-account pool -> the compute / routing layer
//   - the review/accept action  -> the accepted-outcome receipt
//   - node placement / runner   -> where the work runs (local / cloud node)
//
// The control logic (messages, model fields, routes, composer + receipt
// actions) is unchanged. Only the rendering and the operator-facing language
// are reframed in Forge terms and rebuilt on `Ui.AiElements`.

import { Match as M } from 'effect'
import type { Html } from 'foldkit/html'
import { html } from 'foldkit/html'

import { formatIsoDateTime } from '../../../time-format'
import { autopilotWorkDetailRouter, autopilotWorkRouter } from '../../../route'
import * as Ui from '../../../ui'
import {
  type ForgeContextDirtyState,
  type ForgeContextFreshness,
  type ForgeContextRefGroupInput,
  type ForgeContextSnapshotInput,
  type ForgeContextSnapshotStatus,
  projectForgeContextSnapshot,
} from '../autopilot-work/context-snapshot'
import {
  type ForgeDiffArtifactDrilldownFileGroup,
  type ForgeDiffArtifactDrilldownStatus,
  type ForgeDiffReviewStatus,
  type ForgeDiffReviewView,
  projectForgeDiffArtifactDrilldown,
  projectForgeDiffReview,
} from '../autopilot-work/diff-review'
import {
  type ForgePlanMutationReceiptItem,
  type ForgePlanMutationReceiptsStatus,
  projectForgePlanMutationReceipts,
} from '../autopilot-work/plan-mutation-receipts'
import {
  type ForgeErrorRecoveryErrorItem,
  type ForgeErrorRecoveryEventItem,
  type ForgeErrorRecoveryStatus,
  buildForgeErrorRecoveryInput,
  projectForgeErrorRecovery,
} from '../autopilot-work/error-recovery'
import {
  type ForgeCompactionBoundaryItem,
  type ForgeCompactionSummaryStatus,
  buildForgeCompactionSummaryInput,
  projectForgeCompactionSummary,
} from '../autopilot-work/compaction-summary'
import {
  type ForgeBrowserDesktopIntegrationItem,
  type ForgeBrowserDesktopIntegrationStatus,
  buildForgeBrowserDesktopIntegrationInput,
  projectForgeBrowserDesktopIntegration,
} from '../autopilot-work/browser-desktop-integration'
import {
  type ForgeUsageBudgetStatus,
  type ForgeUsageBudgetThreshold,
  buildForgeUsageBudgetInput,
  projectForgeUsageBudget,
} from '../autopilot-work/usage-budget'
import {
  type ForgeModelProviderStatus,
  buildForgeModelProviderInput,
  projectForgeModelProvider,
} from '../autopilot-work/model-provider'
import {
  type ForgeMultimodalInputItem,
  type ForgeMultimodalInputStatus,
  buildForgeMultimodalInputInput,
  projectForgeMultimodalInput,
} from '../autopilot-work/multimodal-input'
import {
  type ForgeInstructionLayerItem,
  type ForgeInstructionLayeringStatus,
  buildForgeInstructionLayeringInput,
  projectForgeInstructionLayering,
} from '../autopilot-work/instruction-layering'
import {
  type ForgeSessionMemoryEntryItem,
  type ForgeSessionMemoryStatus,
  buildForgeSessionMemoryInput,
  projectForgeSessionMemory,
} from '../autopilot-work/session-memory'
import {
  type ForgeRemoteSessionBridgeItem,
  type ForgeRemoteSessionBridgeStatus,
  buildForgeRemoteSessionBridgeInput,
  projectForgeRemoteSessionBridge,
} from '../autopilot-work/remote-session-bridge'
import {
  type ForgeCompanionSurfaceItem,
  type ForgeCompanionSurfaceStatus,
  buildForgeCompanionSurfaceInput,
  projectForgeCompanionSurface,
} from '../autopilot-work/companion-surface'
import {
  type ForgeTeamSharedMemoryItem,
  type ForgeTeamSharedMemoryStatus,
  buildForgeTeamSharedMemoryInput,
  projectForgeTeamSharedMemory,
} from '../autopilot-work/team-shared-memory'
import {
  type ForgeMultiAgentCoordinationItem,
  type ForgeMultiAgentCoordinationStatus,
  buildForgeMultiAgentCoordinationInput,
  projectForgeMultiAgentCoordination,
} from '../autopilot-work/multi-agent-coordination'
import {
  type ForgeExternalWorkIntakeItem,
  type ForgeExternalWorkIntakeStatus,
  buildForgeExternalWorkIntakeInput,
  projectForgeExternalWorkIntake,
} from '../autopilot-work/external-work-intake'
import {
  type ForgeArtifactReceiptArtifactItem,
  type ForgeArtifactReceiptIndexStatus,
  type ForgeArtifactReceiptReceiptItem,
  buildForgeArtifactReceiptIndexInput,
  projectForgeArtifactReceiptIndex,
} from '../autopilot-work/artifact-receipt-index'
import {
  type ForgeSchedulingCronItem,
  type ForgeSchedulingCronViewStatus,
  buildForgeSchedulingCronInput,
  projectForgeSchedulingCron,
} from '../autopilot-work/scheduling-cron'
import {
  type ForgeStructuredEventLogItem,
  type ForgeStructuredEventLogStatus,
  buildForgeStructuredEventLogInput,
  projectForgeStructuredEventLog,
} from '../autopilot-work/structured-event-log'
import {
  type ForgeTelemetryPrivacyItem,
  type ForgeTelemetryPrivacyStatus,
  buildForgeTelemetryPrivacyInput,
  projectForgeTelemetryPrivacy,
} from '../autopilot-work/telemetry-privacy'
import {
  type ForgePerformanceDiagnosticsItem,
  type ForgePerformanceDiagnosticsStatus,
  buildForgePerformanceDiagnosticsInput,
  projectForgePerformanceDiagnostics,
} from '../autopilot-work/performance-diagnostics'
import {
  type ForgeUpdateReleaseItem,
  type ForgeUpdateReleaseStatus,
  buildForgeUpdateReleaseInput,
  projectForgeUpdateRelease,
} from '../autopilot-work/update-release'
import {
  type ForgeMigrationEvidenceItem,
  type ForgeMigrationEvidenceStatus,
  buildForgeMigrationEvidenceInput,
  projectForgeMigrationEvidence,
} from '../autopilot-work/migration-evidence'
import {
  type ForgeTestingSmokeEvidenceItem,
  type ForgeTestingSmokeEvidenceStatus,
  buildForgeTestingSmokeEvidenceInput,
  projectForgeTestingSmokeEvidence,
} from '../autopilot-work/testing-smoke-evidence'
import {
  type ForgeEvaluationRegressionEvidenceItem,
  type ForgeEvaluationRegressionEvidenceStatus,
  buildForgeEvaluationRegressionEvidenceInput,
  projectForgeEvaluationRegressionEvidence,
} from '../autopilot-work/evaluation-regression-evidence'
import {
  type ForgeSecurityReviewEvidenceItem,
  type ForgeSecurityReviewEvidenceStatus,
  buildForgeSecurityReviewEvidenceInput,
  projectForgeSecurityReviewEvidence,
} from '../autopilot-work/security-review-evidence'
import {
  type ForgeDataRetentionDeletionItem,
  type ForgeDataRetentionDeletionStatus,
  buildForgeDataRetentionDeletionInput,
  projectForgeDataRetentionDeletionEvidence,
} from '../autopilot-work/data-retention-deletion-evidence'
import {
  type ForgeOnboardingCapabilityItem,
  type ForgeOnboardingCapabilityStatus,
  buildForgeOnboardingCapabilityInput,
  projectForgeOnboardingCapabilityEvidence,
} from '../autopilot-work/onboarding-capability-evidence'
import {
  type ForgeOutputStylePersonaItem,
  type ForgeOutputStylePersonaStatus,
  buildForgeOutputStylePersonaInput,
  projectForgeOutputStylePersonaEvidence,
} from '../autopilot-work/output-style-persona-evidence'
import {
  type ForgePromptSuggestionItem,
  type ForgePromptSuggestionsStatus,
  buildForgePromptSuggestionsInput,
  projectForgePromptSuggestionsEvidence,
} from '../autopilot-work/prompt-suggestions-evidence'
import {
  type ForgeTipsEducationItem,
  type ForgeTipsEducationStatus,
  buildForgeTipsEducationInput,
  projectForgeTipsEducationEvidence,
} from '../autopilot-work/tips-education-evidence'
import {
  type ForgeThemeVisualItem,
  type ForgeThemeVisualStatus,
  buildForgeThemeVisualInput,
  projectForgeThemeVisualEvidence,
} from '../autopilot-work/theme-visual-evidence'
import {
  type ForgeAccessibilityNonInteractiveItem,
  type ForgeAccessibilityNonInteractiveStatus,
  buildForgeAccessibilityNonInteractiveInput,
  projectForgeAccessibilityNonInteractiveEvidence,
} from '../autopilot-work/accessibility-non-interactive-evidence'
import {
  type ForgeLocalizationBoundaryItem,
  type ForgeLocalizationBoundaryStatus,
  buildForgeLocalizationBoundaryInput,
  projectForgeLocalizationBoundaryEvidence,
} from '../autopilot-work/localization-boundary-evidence'
import {
  type ForgeEnterpriseManagedPolicyItem,
  type ForgeEnterpriseManagedPolicyStatus,
  buildForgeEnterpriseManagedPolicyInput,
  projectForgeEnterpriseManagedPolicyEvidence,
} from '../autopilot-work/enterprise-managed-policy-evidence'
import {
  type ForgeDiagnosticEntryItem,
  type ForgeDiagnosticsStatus,
  buildForgeDiagnosticsInput,
  projectForgeDiagnostics,
} from '../autopilot-work/diagnostics'
import {
  type ForgeEditorIntegrationItem,
  type ForgeEditorIntegrationStatus,
  buildForgeEditorIntegrationInput,
  projectForgeEditorIntegration,
} from '../autopilot-work/editor-integration'
import {
  type ForgeHelpDoctorDebugItem,
  type ForgeHelpDoctorDebugStatus,
  buildForgeHelpDoctorDebugInput,
  projectForgeHelpDoctorDebug,
} from '../autopilot-work/help-doctor-debug'
import {
  type ForgeMcpServerExportItem,
  type ForgeMcpServerExportStatus,
  buildForgeMcpServerExportInput,
  projectForgeMcpServerExport,
} from '../autopilot-work/mcp-server-export'
import {
  type ForgeSettingsConfigurationItem,
  type ForgeSettingsConfigurationStatus,
  buildForgeSettingsConfigurationInput,
  projectForgeSettingsConfiguration,
} from '../autopilot-work/settings-configuration'
import {
  type ForgeTerminalSurfaceItem,
  type ForgeTerminalUiShellStatus,
  buildForgeTerminalUiShellInput,
  projectForgeTerminalUiShell,
} from '../autopilot-work/terminal-ui-shell'
import {
  type ForgeInputKeybindingItem,
  type ForgeInputKeybindingStatus,
  buildForgeInputKeybindingInput,
  projectForgeInputKeybinding,
} from '../autopilot-work/input-keybinding'
import {
  type ForgeCommandItem,
  type ForgeCommandSystemStatus,
  buildForgeCommandSystemInput,
  projectForgeCommandSystem,
} from '../autopilot-work/command-system'
import {
  type ForgeCredentialStorageItem,
  type ForgeCredentialStorageStatus,
  buildForgeCredentialStorageInput,
  projectForgeCredentialStorage,
} from '../autopilot-work/credential-storage'
import {
  type ForgeGitWorkflowItem,
  type ForgeGitWorkflowStatus,
  buildForgeGitWorkflowInput,
  projectForgeGitWorkflow,
} from '../autopilot-work/git-workflow'
import {
  type ForgeAttentionItem,
  type ForgeNotificationAttentionStatus,
  buildForgeNotificationAttentionInput,
  projectForgeNotificationAttention,
} from '../autopilot-work/notification-attention'
import {
  type ForgeExtensibilityExecutionReceipt,
  type ForgeExtensibilityExecutionReceiptsInput,
  type ForgeExtensibilityExecutionReceiptsView,
  projectForgeExtensibilityExecutionReceipts,
} from '../autopilot-work/extensibility-execution-receipts'
import {
  type ForgeRunProgressItem,
  type ForgeRunProgressItemStatus,
  type ForgeRunProgressStatus,
  projectForgeRunProgress,
} from '../autopilot-work/progress-view'
import {
  type ForgeRetrievalCandidate,
  type ForgeRetrievalFreshness,
  type ForgeRetrievalPlanInput,
  type ForgeRetrievalPlanStatus,
  type ForgeRetrievalSkippedCandidate,
  projectForgeRetrievalPlan,
} from '../autopilot-work/retrieval-plan'
import { buildForgeLiveRetrievalPlanInput } from '../autopilot-work/live-retrieval-adapters'
import {
  type ForgeRepositoryMemoryProfile,
  type ForgeRepositoryMemoryProfileInput,
  type ForgeRepositoryMemoryProfileStatus,
  projectForgeRepositoryMemoryProfile,
} from '../autopilot-work/repository-memory-profile'
import {
  type ForgeSessionControlReceiptItem,
  type ForgeSessionNavigationAction,
  type ForgeSessionNavigationItem,
  type ForgeSessionNavigationStatus,
  projectForgeSessionNavigation,
} from '../autopilot-work/session-navigation'
import {
  type ForgeDoctorCheckItem,
  type ForgeDoctorSeverity,
  type ForgeSupportDiagnosticsStatus,
  type ForgeSupportExportReadiness,
  projectForgeSupportDiagnostics,
} from '../autopilot-work/support-diagnostics'
import {
  Message,
  RequestedLoadAutopilotWorkDetail,
  RequestedLoadAutopilotWorkList,
  SubmittedAutopilotWorkComposer,
  SubmittedAutopilotWorkReview,
  UpdatedAutopilotWorkComposerField,
} from '../message'
import type {
  AutopilotMissionBriefing,
  AutopilotMorningReport,
  AutopilotMorningReportGroup,
  AutopilotWorkEvent,
  AutopilotWorkProjection,
  AutopilotWorkReviewAction,
  AutopilotWorkState,
  AutopilotWorkSummary,
  Model,
} from '../model'

const stateLabel = (state: AutopilotWorkState): string =>
  state.replaceAll('_', ' ')

const stateTone = (
  state: AutopilotWorkState,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  M.value(state).pipe(
    M.when('access_required', () => 'warning' as const),
    M.when('accepted', () => 'positive' as const),
    M.when('accepted_free_slice', () => 'accent' as const),
    M.when('blocked', () => 'negative' as const),
    M.when('delivered', () => 'positive' as const),
    M.when('invalid', () => 'negative' as const),
    M.when('paid_ready', () => 'accent' as const),
    M.when('payment_required', () => 'warning' as const),
    M.when('queued_or_running', () => 'info' as const),
    M.when('rejected', () => 'negative' as const),
    M.when('revision_required', () => 'warning' as const),
    M.when('scheduled', () => 'info' as const),
    M.exhaustive,
  )

const badge = (label: string, tone: ReturnType<typeof stateTone>): Html => {
  const h = html<Message>()
  const color =
    tone === 'positive'
      ? 'border-[#1b5e20] text-[#7ccf8a]'
      : tone === 'warning'
        ? 'border-[#5a3b00] text-[#ffb400]'
        : tone === 'negative'
          ? 'border-[#5c1f1f] text-[#ff8a80]'
          : tone === 'info'
            ? 'border-[#1d3d63] text-[#8ab4ff]'
            : 'border-[#333] text-white/65'

  return h.span(
    [
      Ui.className<Message>(
        `inline-flex min-h-7 items-center border px-2 text-[0.6875rem] uppercase ${color}`,
      ),
    ],
    [label],
  )
}

const ageLabel = (iso: string, generatedAt: string): string => {
  const then = Date.parse(iso)
  const now = Date.parse(generatedAt)

  if (!Number.isFinite(then) || !Number.isFinite(now)) {
    return 'Unknown age'
  }

  const minutes = Math.max(0, Math.floor((now - then) / 60_000))

  if (minutes < 60) {
    return `${minutes}m`
  }

  const hours = Math.floor(minutes / 60)

  return hours < 48 ? `${hours}h` : `${Math.floor(hours / 24)}d`
}

const issueRefs = (summary: AutopilotWorkSummary): ReadonlyArray<string> =>
  summary.issueRefs ?? []

const issueText = (refs: ReadonlyArray<string>): string =>
  refs.length === 0
    ? 'No issue ref'
    : refs.map(ref => ref.replace(/^github\.issue\./, '#')).join(', ')

const loadingView = (label: string): Html => {
  const h = html<Message>()

  return h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [label])
}

const errorView = (error: string): Html => {
  const h = html<Message>()

  return h.p([Ui.className<Message>('m-0 text-sm text-[#ff8a80]')], [error])
}

const emptyView = (): Html => {
  const h = html<Message>()

  return h.div(
    [Ui.className<Message>('border border-[#222] bg-[#080808] p-5')],
    [
      h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
        'No Runs',
      ]),
      h.p([Ui.className<Message>('m-0 mt-2 text-sm/6 text-white/50')], [
        'No Runs are visible for this owner yet. Submit a Run to put work into the factory.',
      ]),
    ],
  )
}

const recordFromUnknown = (value: unknown): Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
    ? Object.fromEntries(Object.entries(value))
    : {}

const stringFromUnknown = (value: unknown): string | undefined =>
  typeof value === 'string' && value.trim() !== '' ? value : undefined

const accessRequirementLabels = (
  work: AutopilotWorkProjection,
): ReadonlyArray<string> =>
  work.accessRequirements
    .map(recordFromUnknown)
    .map(record =>
      [
        stringFromUnknown(record.kind),
        stringFromUnknown(record.grantAction),
      ].filter((value): value is string => value !== undefined).join(' / ')
    )
    .filter(label => label !== '')

// The compute / routing decision: which node the Run is placed on. Reframes the
// runner-selection projection as the "where work runs" line.
const placementSummary = (work: AutopilotWorkProjection): string => {
  const placement = recordFromUnknown(work.placementDecision)
  const selected = stringFromUnknown(placement.selectedRunnerKind)
  const fallback = stringFromUnknown(placement.fallbackRunnerKind)

  return selected ?? fallback ?? 'No node selected'
}

const composerStatusView = (model: Model): Html | null => {
  const h = html<Message>()

  return M.value(model.autopilotWorkComposer).pipe(
    M.tags({
      AutopilotWorkComposerIdle: () => null,
      AutopilotWorkComposerSubmitting: () =>
        h.p([Ui.className<Message>('m-0 text-sm text-white/50')], [
          'Submitting Run...',
        ]),
      AutopilotWorkComposerFailed: ({ error }) => errorView(error),
      AutopilotWorkComposerSucceeded: ({ response }) => {
        const access = accessRequirementLabels(response.work)

        return h.div(
          [Ui.className<Message>('grid gap-2 border border-[#222] bg-[#050505] p-3 text-sm text-white/65')],
          [
            h.div([Ui.className<Message>('font-medium text-white/80')], [
              `${response.work.workOrderRef} - ${stateLabel(response.work.state)}`,
            ]),
            h.div([], [`Next: ${response.work.nextAction.state}`]),
            h.div([], [`Runs on: ${placementSummary(response.work)}`]),
            access.length === 0
              ? null
              : h.div([], [`Needs: ${access.join(', ')}`]),
          ].filter((node): node is Html => node !== null),
        )
      },
    }),
    M.exhaustive,
  )
}

const composerView = (model: Model): Html => {
  const h = html<Message>()
  const draft = model.autopilotWorkComposerDraft
  const submitting =
    model.autopilotWorkComposer._tag === 'AutopilotWorkComposerSubmitting'

  return h.form(
    [
      Ui.className<Message>('grid gap-3 border border-[#222] bg-black p-5'),
      h.OnSubmit(SubmittedAutopilotWorkComposer()),
    ],
    [
      h.div([Ui.className<Message>(Ui.eyebrowClass)], ['New Run']),
      h.label([Ui.className<Message>('grid gap-2')], [
        h.span([Ui.className<Message>('text-sm font-medium text-white/80')], [
          'Objective',
        ]),
        h.textarea(
          [
            h.Name('objective'),
            h.Value(draft.objective),
            h.Rows(4),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({ field: 'objective', value })
            ),
            Ui.className<Message>(
              'min-h-28 resize-y border border-[#333] bg-[#050505] p-3 text-base/7 text-white/85 outline-none focus:border-white/45 sm:text-sm/6',
            ),
          ],
          [],
        ),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-[minmax(0,1.2fr)_8rem_10rem]')], [
        h.label([Ui.className<Message>('grid gap-2')], [
          h.span([Ui.className<Message>('text-xs uppercase text-white/40')], [
            'Repository',
          ]),
          h.input([
            h.Name('repository'),
            h.Value(draft.repositoryFullName),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({
                field: 'repositoryFullName',
                value,
              })
            ),
            Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
          ]),
        ]),
        h.label([Ui.className<Message>('grid gap-2')], [
          h.span([Ui.className<Message>('text-xs uppercase text-white/40')], [
            'Branch',
          ]),
          h.input([
            h.Name('branch'),
            h.Value(draft.branch),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({ field: 'branch', value })
            ),
            Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
          ]),
        ]),
        h.label([Ui.className<Message>('grid gap-2')], [
          h.span([Ui.className<Message>('text-xs uppercase text-white/40')], [
            'Budget cents',
          ]),
          h.input([
            h.Name('budget'),
            h.Type('number'),
            h.Value(draft.maxSpendCents),
            h.OnInput(value =>
              UpdatedAutopilotWorkComposerField({
                field: 'maxSpendCents',
                value,
              })
            ),
            Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
          ]),
        ]),
      ]),
      h.label([Ui.className<Message>('grid gap-2')], [
        h.span([Ui.className<Message>('text-xs uppercase text-white/40')], [
          'Verification command',
        ]),
        h.input([
          h.Name('verification'),
          h.Value(draft.verificationCommand),
          h.OnInput(value =>
            UpdatedAutopilotWorkComposerField({
              field: 'verificationCommand',
              value,
            })
          ),
          Ui.className<Message>(`${Ui.inputClass} max-sm:text-base`),
        ]),
      ]),
      composerStatusView(model),
      Ui.button<Message>({
        attrs: [h.Type('submit'), ...(submitting ? [h.Disabled(true)] : [])],
        label: submitting ? 'Submitting...' : 'Submit Run',
        size: 'sm',
        variant: 'primary',
      }),
    ].filter((node): node is Html => node !== null),
  )
}

const morningReportGroupLabel = (
  group: AutopilotMorningReportGroup,
): string =>
  M.value(group).pipe(
    M.when('awaiting_decision', () => 'Awaiting decision'),
    M.when('blocked', () => 'Blocked'),
    M.when('launched', () => 'Launched'),
    M.when('reviewed', () => 'Reviewed'),
    M.when('running', () => 'Running'),
    M.when('scheduled', () => 'Scheduled'),
    M.exhaustive,
  )

const morningReportGroupTone = (
  group: AutopilotMorningReportGroup,
): ReturnType<typeof stateTone> =>
  M.value(group).pipe(
    M.when('awaiting_decision', () => 'warning' as const),
    M.when('blocked', () => 'negative' as const),
    M.when('launched', () => 'info' as const),
    M.when('reviewed', () => 'positive' as const),
    M.when('running', () => 'info' as const),
    M.when('scheduled', () => 'accent' as const),
    M.exhaustive,
  )

const morningReportItemRow = (
  item: AutopilotMorningReport['workItems'][number],
): Html => {
  const h = html<Message>()
  const href = autopilotWorkDetailRouter({ workOrderRef: item.workOrderRef })

  return h.a(
    [
      h.Href(href),
      Ui.className<Message>(
        'grid gap-2 border-b border-[#222] px-4 py-3 text-left no-underline last:border-b-0 hover:bg-[#080808] md:grid-cols-[9rem_minmax(0,1.4fr)_10rem] md:items-center',
      ),
    ],
    [
      h.div([], [
        badge(morningReportGroupLabel(item.group), morningReportGroupTone(item.group)),
      ]),
      h.div(
        [
          Ui.className<Message>(
            'overflow-hidden text-ellipsis whitespace-nowrap text-sm text-white/75',
          ),
        ],
        [item.workOrderRef],
      ),
      h.div([Ui.className<Message>('text-xs text-white/45 md:text-right')], [
        item.scheduledLaunchAt === null
          ? formatIsoDateTime(item.updatedAt)
          : `Launch ${formatIsoDateTime(item.scheduledLaunchAt)}`,
      ]),
    ],
  )
}

const morningReportContinuationRow = (
  continuation: AutopilotMorningReport['continuations'][number],
): Html => {
  const h = html<Message>()

  return h.div(
    [
      Ui.className<Message>(
        'grid gap-2 border-b border-[#222] px-4 py-3 last:border-b-0 md:grid-cols-[9rem_minmax(0,1.4fr)_10rem] md:items-center',
      ),
    ],
    [
      h.div([], [
        badge(
          continuation.decision === 'dispatched' ? 'Resumed' : 'Resume failed',
          continuation.decision === 'dispatched' ? 'positive' : 'negative',
        ),
      ]),
      h.div(
        [
          Ui.className<Message>(
            'overflow-hidden text-ellipsis whitespace-nowrap text-sm text-white/75',
          ),
        ],
        [`${continuation.runId} - attempt ${continuation.attempt}`],
      ),
      h.div([Ui.className<Message>('text-xs text-white/45 md:text-right')], [
        formatIsoDateTime(continuation.occurredAt),
      ]),
    ],
  )
}

const morningReportPanel = (report: AutopilotMorningReport): Html => {
  const h = html<Message>()
  const rows = [
    ...report.workItems.map(morningReportItemRow),
    ...report.continuations.map(morningReportContinuationRow),
  ]

  return h.section([Ui.className<Message>('grid gap-3')], [
    h.div([Ui.className<Message>('grid gap-1')], [
      h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
        'While you were away',
      ]),
      h.p([Ui.className<Message>('m-0 text-sm/6 text-white/45')], [
        `Since ${formatIsoDateTime(report.sinceIso)} - ${report.counts.awaitingDecision} awaiting decision, ${report.counts.blocked} blocked, ${report.counts.scheduled} scheduled, ${report.counts.continuations} resumed`,
      ]),
    ]),
    rows.length === 0
      ? h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
          'Nothing ran in this window.',
        ])
      : h.div([Ui.className<Message>('overflow-hidden border border-[#222]')], rows),
  ])
}

const morningReportView = (model: Model): Html =>
  M.value(model.autopilotMorningReport).pipe(
    M.tags({
      AutopilotMorningReportIdle: () =>
        html<Message>().span([Ui.className<Message>('hidden')], []),
      AutopilotMorningReportLoading: () =>
        loadingView('Loading overnight summary...'),
      AutopilotMorningReportFailed: ({ error }) => errorView(error),
      AutopilotMorningReportLoaded: ({ response }) =>
        morningReportPanel(response.report),
    }),
    M.exhaustive,
  )

const workRow = (
  summary: AutopilotWorkSummary,
  generatedAt: string,
): Html => {
  const h = html<Message>()
  const href = autopilotWorkDetailRouter({
    workOrderRef: summary.workOrderRef,
  })

  return h.a(
    [
      h.Href(href),
      Ui.className<Message>(
        'grid gap-3 border-b border-[#222] px-4 py-4 text-left no-underline last:border-b-0 hover:bg-[#080808] md:grid-cols-[minmax(0,1.4fr)_9rem_7rem_7rem] md:items-center',
      ),
    ],
    [
      h.div([Ui.className<Message>('min-w-0')], [
        h.div(
          [
            Ui.className<Message>(
              'overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/85',
            ),
          ],
          [summary.workOrderRef],
        ),
        h.div(
          [
            Ui.className<Message>(
              'mt-1 overflow-hidden text-ellipsis whitespace-nowrap text-xs text-white/35',
            ),
          ],
          [summary.taskRefs?.[0] ?? summary.promiseRef.promiseId],
        ),
      ]),
      h.div([Ui.className<Message>('text-xs text-white/55')], [
        summary.promiseRef.promiseId,
      ]),
      h.div([Ui.className<Message>('text-xs text-white/55')], [
        issueText(issueRefs(summary)),
      ]),
      h.div([Ui.className<Message>('flex items-center gap-2 md:justify-end')], [
        badge(stateLabel(summary.state), stateTone(summary.state)),
        h.span([Ui.className<Message>('text-xs text-white/35')], [
          ageLabel(summary.createdAt, summary.generatedAt ?? generatedAt),
        ]),
      ]),
    ],
  )
}

const listLoadedView = (
  model: Model,
  workOrders: ReadonlyArray<AutopilotWorkSummary>,
  generatedAt: string,
): Html => {
  const h = html<Message>()

  return h.section([Ui.className<Message>('grid gap-4')], [
    composerView(model),
    h.div([Ui.className<Message>('flex flex-wrap items-end justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.div([Ui.className<Message>(Ui.eyebrowClass)], ['Forge cockpit']),
        h.h1([Ui.className<Message>('m-0 text-2xl font-semibold text-white')], [
          'Runs',
        ]),
        h.p([Ui.className<Message>('m-0 text-sm/6 text-white/50')], [
          `Generated ${formatIsoDateTime(generatedAt)}`,
        ]),
      ]),
      Ui.button<Message>({
        attrs: [
          html<Message>().Type('button'),
          html<Message>().OnClick(RequestedLoadAutopilotWorkList()),
        ],
        label: 'Refresh',
        size: 'sm',
        variant: 'secondary',
      }),
    ]),
    morningReportView(model),
    workOrders.length === 0
      ? emptyView()
      : h.div(
          [Ui.className<Message>('overflow-hidden border border-[#222]')],
          [
            h.div(
              [
                Ui.className<Message>(
                  'hidden border-b border-[#222] px-4 py-2 text-[0.6875rem] uppercase text-white/35 md:grid md:grid-cols-[minmax(0,1.4fr)_9rem_7rem_7rem]',
                ),
              ],
              [
                h.div([], ['Run']),
                h.div([], ['Lane']),
                h.div([], ['Issue']),
                h.div([Ui.className<Message>('text-right')], ['Status']),
              ],
            ),
            ...workOrders.map(workOrder => workRow(workOrder, generatedAt)),
          ],
        ),
  ])
}

export const listView = (model: Model): Html =>
  M.value(model.autopilotWorkList).pipe(
    M.tags({
      AutopilotWorkListIdle: () =>
        html<Message>().section([Ui.className<Message>('grid gap-4')], [
          composerView(model),
          loadingView('Runs have not loaded.'),
        ]),
      AutopilotWorkListLoading: () =>
        html<Message>().section([Ui.className<Message>('grid gap-4')], [
          composerView(model),
          loadingView('Loading Runs...'),
        ]),
      AutopilotWorkListFailed: ({ error }) =>
        html<Message>().section([Ui.className<Message>('grid gap-4')], [
          composerView(model),
          errorView(error),
        ]),
      AutopilotWorkListLoaded: ({ response }) =>
        listLoadedView(model, response.workOrders, response.generatedAt),
    }),
    M.exhaustive,
  )

const refChips = (refs: ReadonlyArray<string>): ReadonlyArray<Html> => {
  const h = html<Message>()

  return refs.length === 0
    ? [
        h.span([Ui.className<Message>('text-xs text-white/35')], [
          'No refs',
        ]),
      ]
    : refs.map(ref =>
        h.span(
          [
            Ui.className<Message>(
              'min-w-0 max-w-full overflow-hidden text-ellipsis whitespace-nowrap border border-[#222] px-2 py-1 text-xs text-white/55',
            ),
          ],
          [ref],
        ),
      )
}

const refSection = (title: string, refs: ReadonlyArray<string>): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-2')], [
    h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/75')], [
      title,
    ]),
    h.div([Ui.className<Message>('flex flex-wrap gap-2')], refChips(refs)),
  ])
}

// The Run lifecycle, expressed as an `AiElements` task list: each recorded
// runtime event is a step, toned by its Run state.
const eventTaskItem = (
  event: AutopilotWorkEvent,
): Ui.AiElements.TaskItemProps => {
  const tone = stateTone(event.state)
  const status: Ui.AiElements.TaskItemStatus =
    tone === 'positive'
      ? 'done'
      : tone === 'negative'
        ? 'failed'
        : tone === 'info'
          ? 'active'
          : 'queued'

  return {
    label: `${event.eventKind.replaceAll('_', ' ')} — ${formatIsoDateTime(event.occurredAt)} (seq ${event.sequence})`,
    status,
  }
}

const eventsPanel = (model: Model): Html => {
  const h = html<Message>()

  return h.section([Ui.className<Message>('grid gap-3 border-t border-[#222] pt-5')], [
    M.value(model.autopilotWorkEvents).pipe(
      M.tags({
        AutopilotWorkEventsIdle: () => loadingView('Lifecycle has not loaded.'),
        AutopilotWorkEventsLoading: () => loadingView('Loading lifecycle...'),
        AutopilotWorkEventsFailed: ({ error }) => errorView(error),
        AutopilotWorkEventsLoaded: ({ response }) =>
          Ui.AiElements.task<Message>({
            props: {
              title: 'Run lifecycle',
              open: true,
              items: response.events.map(eventTaskItem),
            },
          }),
      }),
      M.exhaustive,
    ),
  ])
}

const loadedEvents = (model: Model): ReadonlyArray<AutopilotWorkEvent> | null =>
  model.autopilotWorkEvents._tag === 'AutopilotWorkEventsLoaded'
    ? model.autopilotWorkEvents.response.events
    : null

const progressStatusTone = (
  status: ForgeRunProgressStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'reviewed' || status === 'delivered'
    ? 'positive'
    : status === 'failed'
      ? 'negative'
      : status === 'blocked'
        ? 'warning'
        : status === 'running'
          ? 'info'
          : 'accent'

const progressTaskStatus = (
  status: ForgeRunProgressItemStatus,
): Ui.AiElements.TaskItemStatus =>
  status === 'completed'
    ? 'done'
    : status === 'failed' || status === 'blocked'
      ? 'failed'
      : status === 'active'
        ? 'active'
        : 'queued'

const progressRefPreview = (refs: ReadonlyArray<string>): string =>
  refs.length === 0
    ? ''
    : ` - ${refs.slice(0, 2).join(', ')}${refs.length > 2 ? ` (+${refs.length - 2})` : ''}`

const progressTaskItem = (
  item: ForgeRunProgressItem,
): Ui.AiElements.TaskItemProps => ({
  label: `${item.label}${progressRefPreview(item.refs)}`,
  status: progressTaskStatus(item.status),
})

const planMutationStatusTone = (
  status: ForgePlanMutationReceiptsStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'applied'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'requested'
          ? 'info'
          : 'accent'

const planMutationReceiptPanel = (
  item: ForgePlanMutationReceiptItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.div(
          [Ui.className<Message>('text-sm font-medium text-white/75')],
          [`${item.action} / ${item.state}`],
        ),
        h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
          `${item.generatedAt} - ${item.actorRef}`,
        ]),
      ]),
      badge(item.state, planMutationStatusTone(item.state)),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Plan item', [item.itemRef]),
      refSection('Request', [item.requestRef]),
      refSection(
        'Receipt',
        item.receiptRef === null ? [] : [item.receiptRef],
      ),
      refSection('Provenance', item.provenanceRefs),
      refSection('Mutation blockers', item.blockerRefs),
    ]),
  ])
}

const progressPanel = (
  model: Model,
  work: AutopilotWorkProjection,
): Html => {
  const h = html<Message>()
  const progress = projectForgeRunProgress(work, loadedEvents(model))
  const planReceipts = projectForgePlanMutationReceipts(work)

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Run progress',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Typed progress projection from Run state, lifecycle events, next action, and closeout evidence.',
        ]),
      ]),
      badge(progress.status.replaceAll('_', ' '), progressStatusTone(progress.status)),
    ]),
    Ui.AiElements.task<Message>({
      props: {
        title: `Progress for ${progress.workOrderRef}`,
        open: true,
        items: progress.items.map(progressTaskItem),
      },
    }),
    refSection('Progress blockers', progress.blockerRefs),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/80')], [
            'Plan mutation receipts',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Requested and runtime-applied plan/todo changes. These receipts do not mark a Run complete without closeout evidence.',
          ]),
        ]),
        badge(
          planReceipts.status,
          planMutationStatusTone(planReceipts.status),
        ),
      ]),
      planReceipts.items.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No plan mutation receipts available yet.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...planReceipts.items.map(planMutationReceiptPanel),
          ]),
      refSection('Plan mutation blockers', planReceipts.blockerRefs),
      planReceipts.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${planReceipts.omittedUnsafeRefCount} unsafe plan mutation ref(s) were omitted before rendering.`,
          ]),
    ]),
    progress.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${progress.omittedUnsafeRefCount} unsafe progress ref(s) were omitted before rendering.`,
    ]),
  ])
}

const notificationAttentionStatusTone = (
  status: ForgeNotificationAttentionStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'attention' || status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const attentionSeverityTone = (
  severity: ForgeAttentionItem['severity'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  severity === 'critical'
    ? 'negative'
    : severity === 'warning'
      ? 'warning'
      : 'info'

const attentionEntryPanel = (entry: ForgeAttentionItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.attentionRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.severity, attentionSeverityTone(entry.severity)),
          badge(entry.state, entry.state === 'resolved' ? 'positive' : 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Attention ref', [entry.attentionRef]),
      refSection('Notifications', entry.notificationRefs),
      refSection('Channels', entry.channelRefs),
      refSection('Deliveries', entry.deliveryRefs),
      refSection('Decisions', entry.decisionRefs),
      refSection('Actions', entry.actionRefs),
      refSection('Dedupe refs', entry.dedupeRefs),
      refSection('Invalidations', entry.invalidationRefs),
      refSection('Resolutions', entry.resolutionRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Attention blockers', entry.blockerRefs),
    ]),
  ])
}

const notificationAttentionPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeNotificationAttention(
    buildForgeNotificationAttentionInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Notifications and attention',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Attention, notification, delivery, decision, and resolution refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          notificationAttentionStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Attention', String(view.counts.total)),
        compactionMetric('Active', String(view.counts.active)),
        compactionMetric('Waiting', String(view.counts.waiting)),
        compactionMetric('Critical', String(view.counts.critical)),
        compactionMetric('Delivered', String(view.counts.delivered)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Attention blockers', view.blockerRefs),
      ]),
      view.attention.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No attention snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.attention.map(attentionEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe notification/attention ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const errorRecoveryStatusTone = (
  status: ForgeErrorRecoveryStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'recovered'
    ? 'positive'
    : status === 'failed_closed' || status === 'blocked'
      ? 'negative'
      : status === 'recovering'
        ? 'info'
        : 'accent'

const errorSeverityTone = (
  severity: ForgeErrorRecoveryErrorItem['severity'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  severity === 'fatal'
    ? 'negative'
    : severity === 'error'
      ? 'warning'
      : severity === 'warning'
        ? 'warning'
        : 'accent'

const errorRetryabilityTone = (
  retryability: ForgeErrorRecoveryErrorItem['retryability'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  retryability === 'retryable'
    ? 'info'
    : retryability === 'conditional'
      ? 'warning'
      : 'accent'

const errorRecoveryTaskStatus = (
  event: ForgeErrorRecoveryEventItem,
): Ui.AiElements.TaskItemStatus =>
  event.kind === 'recovery.succeeded'
    ? 'done'
    : event.kind === 'recovery.failed' || event.kind === 'run.failed_closed'
      ? 'failed'
      : event.kind === 'error.recorded'
        ? 'queued'
        : 'active'

const errorRecoveryTaskItem = (
  event: ForgeErrorRecoveryEventItem,
): Ui.AiElements.TaskItemProps => ({
  label: `${event.kind} - ${formatIsoDateTime(
    event.occurredAt,
  )}${progressRefPreview([event.eventRef, ...event.receiptRefs])}`,
  status: errorRecoveryTaskStatus(event),
})

const errorRecoveryErrorPanel = (
  item: ForgeErrorRecoveryErrorItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            item.category,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${item.occurredAt === null ? 'Unknown time' : formatIsoDateTime(item.occurredAt)} - ${item.originServiceRef ?? 'unknown origin'}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(item.severity, errorSeverityTone(item.severity)),
          badge(
            item.retryability.replaceAll('_', ' '),
            errorRetryabilityTone(item.retryability),
          ),
          badge(
            item.recoveryStrategy.replaceAll('_', ' '),
            errorRetryabilityTone(item.retryability),
          ),
        ]),
      ],
    ),
    item.publicMessage === null
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-white/55')], [
          item.publicMessage,
        ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Error ref', [item.errorRef]),
      refSection(
        'Diagnostic ref',
        item.diagnosticRef === null ? [] : [item.diagnosticRef],
      ),
      refSection('Cause ref', item.causeRef === null ? [] : [item.causeRef]),
      refSection('Related refs', item.relatedRefs),
    ]),
  ])
}

const errorRecoveryEventPanel = (
  item: ForgeErrorRecoveryEventItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            item.kind,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            formatIsoDateTime(item.occurredAt),
          ]),
        ]),
        badge(
          errorRecoveryTaskStatus(item),
          errorRecoveryStatusTone(
            item.kind === 'recovery.succeeded'
              ? 'recovered'
              : item.kind === 'recovery.failed'
                ? 'blocked'
                : item.kind === 'run.failed_closed'
                  ? 'failed_closed'
                  : 'recovering',
          ),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Event ref', [item.eventRef]),
      refSection('Error ref', item.errorRef === null ? [] : [item.errorRef]),
      refSection('Receipt refs', item.receiptRefs),
      refSection('Event blockers', item.blockerRefs),
    ]),
  ])
}

const errorRecoveryPanel = (
  model: Model,
  work: AutopilotWorkProjection,
): Html => {
  const h = html<Message>()
  const view = projectForgeErrorRecovery(
    buildForgeErrorRecoveryInput(work, loadedEvents(model)),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Error recovery',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Failure category, retry posture, recovery events, and blockers.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          errorRecoveryStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Recovery ref', [view.recoveryRef]),
        refSection('Recovery blockers', view.blockerRefs),
      ]),
      view.errors.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No typed recovery errors available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.errors.map(errorRecoveryErrorPanel),
          ]),
      view.events.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No recovery events available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            Ui.AiElements.task<Message>({
              props: {
                title: `Recovery events for ${view.workOrderRef}`,
                open: true,
                items: view.events.map(errorRecoveryTaskItem),
              },
            }),
            ...view.events.map(errorRecoveryEventPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe error recovery ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const compactionStatusTone = (
  status: ForgeCompactionSummaryStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'compacted'
    ? 'positive'
    : status === 'blocked' || status === 'failed'
      ? 'negative'
      : status === 'pending'
        ? 'info'
        : status === 'cancelled'
          ? 'warning'
          : 'accent'

const compactionBoundaryTone = (
  state: ForgeCompactionBoundaryItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'compacted'
    ? 'positive'
    : state === 'failed'
      ? 'negative'
      : state === 'pending'
        ? 'info'
        : state === 'cancelled'
          ? 'warning'
          : 'accent'

const compactionMetric = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-1 border border-[#222] p-3')], [
    h.div([Ui.className<Message>('text-[0.6875rem] uppercase text-white/35')], [
      label,
    ]),
    h.div(
      [Ui.className<Message>('min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/75')],
      [value],
    ),
  ])
}

const compactionEstimateLabel = (
  estimate: ForgeCompactionBoundaryItem['preEstimate'],
): string => {
  if (estimate === null) {
    return 'No estimate'
  }

  const tokens =
    estimate.tokenCount === null ? 'unknown tokens' : `${estimate.tokenCount} tokens`
  const messages =
    estimate.messageCount === null
      ? 'unknown messages'
      : `${estimate.messageCount} messages`

  return `${tokens} / ${messages}`
}

const compactionToolPairRefs = (
  boundary: ForgeCompactionBoundaryItem,
): ReadonlyArray<string> =>
  boundary.preservedToolPairs.flatMap(pair => [
    pair.requestRef,
    ...(pair.resultRef === null ? [] : [pair.resultRef]),
    ...(pair.summaryRef === null ? [] : [pair.summaryRef]),
  ])

const compactionBoundaryPanel = (
  boundary: ForgeCompactionBoundaryItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-4 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            `${boundary.trigger.replaceAll('_', ' ')} / ${boundary.strategy.replaceAll('_', ' ')}`,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            formatIsoDateTime(boundary.generatedAt),
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(boundary.state, compactionBoundaryTone(boundary.state)),
          badge(
            `${boundary.automaticFailureCount} failure(s)`,
            boundary.automaticFailureCount > 1 ? 'negative' : 'accent',
          ),
        ]),
      ],
    ),
    boundary.publicMessage === null
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-white/55')], [
          boundary.publicMessage,
        ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-3')], [
      compactionMetric('Before', compactionEstimateLabel(boundary.preEstimate)),
      compactionMetric('After', compactionEstimateLabel(boundary.postEstimate)),
      compactionMetric(
        'Context window',
        boundary.preEstimate?.contextWindow === null ||
          boundary.preEstimate?.contextWindow === undefined
          ? 'unknown'
          : String(boundary.preEstimate.contextWindow),
      ),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Boundary ref', [boundary.boundaryRef]),
      refSection('Summary sources', boundary.summarySourceRefs),
      refSection('Preserved messages', boundary.preservedRecentMessageRefs),
      refSection('Preserved tool pairs', compactionToolPairRefs(boundary)),
      refSection('Preserved tasks', boundary.preservedTaskRefs),
      refSection('Preserved plans', boundary.preservedPlanRefs),
      refSection('Restored files', boundary.restoredFileRefs),
      refSection('Restored adapters', boundary.restoredAdapterRefs),
      refSection('Restored skills', boundary.restoredSkillRefs),
      refSection('Policy refs', boundary.policyRefs),
      refSection('Hook refs', boundary.hookRefs),
      refSection('Failure refs', boundary.failureRefs),
      refSection('Retry refs', boundary.retryRefs),
      refSection('Boundary blockers', boundary.blockerRefs),
    ]),
  ])
}

const compactionSummaryPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeCompactionSummary(
    buildForgeCompactionSummaryInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Compaction',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Context boundary state, preserved refs, restored refs, and blockers.',
          ]),
        ]),
        badge(view.status, compactionStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Compaction ref', [view.compactionRef]),
        refSection('Compaction blockers', view.blockerRefs),
      ]),
      view.boundaries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No compaction boundaries available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.boundaries.map(compactionBoundaryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe compaction ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const usageBudgetStatusTone = (
  status: ForgeUsageBudgetStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'within'
    ? 'positive'
    : status === 'blocked' || status === 'exceeded'
      ? 'negative'
      : status === 'near_limit'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const usageBudgetThresholdTone = (
  state: ForgeUsageBudgetThreshold['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'within'
    ? 'positive'
    : state === 'exceeded' || state === 'blocked'
      ? 'negative'
      : state === 'near_limit'
        ? 'warning'
        : 'accent'

const usageNumber = (value: number | null): string =>
  value === null ? 'unknown' : String(value)

const centsLabel = (value: number | null, currency: string | null): string =>
  value === null ? 'unknown' : `${currency ?? 'currency'} ${value / 100}`

const usageBudgetThresholdPanel = (
  threshold: ForgeUsageBudgetThreshold,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            `${threshold.action} / ${threshold.state}`,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `tokens ${usageNumber(threshold.limitTokens)} - cost cents ${usageNumber(threshold.limitCostCents)}`,
          ]),
        ]),
        badge(threshold.state, usageBudgetThresholdTone(threshold.state)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Budget ref', [threshold.budgetRef]),
      refSection('Policy refs', threshold.policyRefs),
    ]),
  ])
}

const usageBudgetPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeUsageBudget(buildForgeUsageBudgetInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Usage and budget',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Context headroom, token usage, cost estimate, and budget blockers.',
          ]),
        ]),
        badge(view.status, usageBudgetStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-3')], [
        compactionMetric('Context tokens', usageNumber(view.tokenCounts.contextWindowTokens)),
        compactionMetric('Total tokens', usageNumber(view.tokenCounts.totalTokens)),
        compactionMetric(
          'Estimated cost',
          centsLabel(
            view.costEstimate?.estimatedCostCents ?? null,
            view.costEstimate?.currency ?? null,
          ),
        ),
        compactionMetric('Input tokens', usageNumber(view.tokenCounts.inputTokens)),
        compactionMetric('Output tokens', usageNumber(view.tokenCounts.outputTokens)),
        compactionMetric(
          'Cache tokens',
          `${usageNumber(view.tokenCounts.cacheReadTokens)} read / ${usageNumber(view.tokenCounts.cacheWriteTokens)} write`,
        ),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Usage ref', view.usageRef === null ? [] : [view.usageRef]),
        refSection(
          'Context estimate',
          view.contextEstimateRef === null ? [] : [view.contextEstimateRef],
        ),
        refSection('Provider ref', view.providerRef === null ? [] : [view.providerRef]),
        refSection('Model ref', view.modelRef === null ? [] : [view.modelRef]),
        refSection(
          'Cost ref',
          view.costEstimate === null ? [] : [view.costEstimate.costRef],
        ),
        refSection(
          'Pricing ref',
          view.costEstimate?.pricingRef === null ||
            view.costEstimate?.pricingRef === undefined
            ? []
            : [view.costEstimate.pricingRef],
        ),
        refSection('Rate limits', view.rateLimitRefs),
        refSection('Quota blockers', view.quotaBlockerRefs),
        refSection('Usage blockers', view.blockerRefs),
      ]),
      view.budgetThresholds.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No budget thresholds available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.budgetThresholds.map(usageBudgetThresholdPanel),
          ]),
      view.costEstimate?.pricingState === 'unknown'
        ? h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            'Pricing is unknown; cost is not exact.',
          ])
        : h.span([Ui.className<Message>('hidden')], []),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe usage/budget ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const modelProviderStatusTone = (
  status: ForgeModelProviderStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'selected'
    ? 'positive'
    : status === 'blocked' || status === 'unavailable'
      ? 'negative'
      : status === 'fallback_selected'
        ? 'warning'
        : 'accent'

const boolCapability = (value: boolean | null): string =>
  value === null ? 'unknown' : value ? 'yes' : 'no'

const modelProviderPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeModelProvider(buildForgeModelProviderInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Model provider',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Resolved model, provider, capabilities, validation, and fallback refs.',
          ]),
        ]),
        badge(view.status.replaceAll('_', ' '), modelProviderStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-3')], [
        compactionMetric(
          'Context window',
          usageNumber(view.capabilities.contextWindowTokens),
        ),
        compactionMetric('Max output', usageNumber(view.capabilities.maxOutputTokens)),
        compactionMetric('Tool calls', boolCapability(view.capabilities.toolCallSupport)),
        compactionMetric(
          'Structured output',
          boolCapability(view.capabilities.structuredOutputSupport),
        ),
        compactionMetric('Vision', boolCapability(view.capabilities.visionSupport)),
        compactionMetric('Cache', boolCapability(view.capabilities.cacheSupport)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection(
          'Resolution ref',
          view.resolutionRef === null ? [] : [view.resolutionRef],
        ),
        refSection(
          'Requested alias',
          view.requestedAliasRef === null ? [] : [view.requestedAliasRef],
        ),
        refSection('Provider ref', view.providerRef === null ? [] : [view.providerRef]),
        refSection('Model ref', view.modelRef === null ? [] : [view.modelRef]),
        refSection(
          'Provider model',
          view.providerFacingModelRef === null ? [] : [view.providerFacingModelRef],
        ),
        refSection('Capability refs', view.capabilityRefs),
        refSection('Entitlement refs', view.entitlementRefs),
        refSection('Validation refs', view.validationRefs),
        refSection('Fallback refs', view.fallbackRefs),
        refSection('Policy refs', view.policyRefs),
        refSection('Pricing refs', view.pricingRefs),
        refSection('Privacy refs', view.privacyRefs),
        refSection('Telemetry refs', view.telemetryRefs),
        refSection('Provider blockers', view.blockerRefs),
      ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe model provider ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const instructionLayeringStatusTone = (
  status: ForgeInstructionLayeringStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const instructionLayerStateTone = (
  state: ForgeInstructionLayerItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'applied' || state === 'appended'
    ? 'positive'
    : state === 'replaced'
      ? 'warning'
      : 'accent'

const instructionLayerPanel = (layer: ForgeInstructionLayerItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            `${layer.precedence}. ${layer.kind.replaceAll('_', ' ')}`,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${layer.freshness} - ${usageNumber(layer.tokenEstimate)} token estimate`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(layer.state, instructionLayerStateTone(layer.state)),
          badge(layer.redactionClass.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Layer ref', [layer.layerRef]),
      refSection('Sources', layer.sourceRefs),
      refSection('Metadata refs', layer.metadataRefs),
      refSection('Policy refs', layer.policyRefs),
      refSection('Allowed tools', layer.allowedToolRefs),
      refSection('Capability deltas', layer.capabilityDeltaRefs),
      refSection(
        'Replacement source',
        layer.replacementSourceRef === null ? [] : [layer.replacementSourceRef],
      ),
      refSection('Layer blockers', layer.blockerRefs),
    ]),
  ])
}

const instructionLayeringPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeInstructionLayering(
    buildForgeInstructionLayeringInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Instruction layering',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Instruction snapshot, precedence, metadata, policy, and capability refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          instructionLayeringStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
        compactionMetric('Layers', String(view.counts.total)),
        compactionMetric('Applied', String(view.counts.applied)),
        compactionMetric('Appended', String(view.counts.appended)),
        compactionMetric('Skipped', String(view.counts.skipped)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Projection ref', view.projectionRef === null ? [] : [view.projectionRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Instruction blockers', view.blockerRefs),
      ]),
      view.layers.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No instruction snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.layers.map(instructionLayerPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe instruction ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const sessionMemoryStatusTone = (
  status: ForgeSessionMemoryStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'conflicted'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const sessionMemoryLifecycleTone = (
  state: ForgeSessionMemoryEntryItem['lifecycleState'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'active'
    ? 'positive'
    : state === 'superseded'
      ? 'warning'
      : state === 'expired' || state === 'forgotten'
        ? 'accent'
        : 'info'

const sessionMemoryEntryPanel = (entry: ForgeSessionMemoryEntryItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            `${entry.scope.replaceAll('_', ' ')} / ${entry.kind.replaceAll('_', ' ')}`,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.freshness} - ${entry.retentionClass.replaceAll('_', ' ')} retention`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.lifecycleState, sessionMemoryLifecycleTone(entry.lifecycleState)),
          badge(entry.redactionClass.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Entry ref', [entry.entryRef]),
      refSection('Sources', entry.sourceRefs),
      refSection('Summaries', entry.summaryRefs),
      refSection('Retrieval refs', entry.retrievalRefs),
      refSection('Compaction refs', entry.compactionRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Conflict refs', entry.conflictRefs),
      refSection('Entry blockers', entry.blockerRefs),
    ]),
  ])
}

const sessionMemoryPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeSessionMemory(buildForgeSessionMemoryInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Session memory',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Memory snapshot, lifecycle, freshness, retention, and evidence refs.',
          ]),
        ]),
        badge(view.status.replaceAll('_', ' '), sessionMemoryStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
        compactionMetric('Entries', String(view.counts.total)),
        compactionMetric('Active', String(view.counts.active)),
        compactionMetric('Stale', String(view.counts.stale)),
        compactionMetric('Conflicts', String(view.counts.conflicted)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Projection ref', view.projectionRef === null ? [] : [view.projectionRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Session memory blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No session memory snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(sessionMemoryEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe session-memory ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const diagnosticsStatusTone = (
  status: ForgeDiagnosticsStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const diagnosticSeverityTone = (
  severity: ForgeDiagnosticEntryItem['severity'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  severity === 'error'
    ? 'negative'
    : severity === 'warning'
      ? 'warning'
      : severity === 'info'
        ? 'info'
        : 'accent'

const diagnosticEntryPanel = (entry: ForgeDiagnosticEntryItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.diagnosticRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.freshness} - ${entry.languageServerRef ?? 'language server unknown'}`,
          ]),
        ]),
        badge(entry.severity, diagnosticSeverityTone(entry.severity)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Diagnostic ref', [entry.diagnosticRef]),
      refSection(
        'Language server',
        entry.languageServerRef === null ? [] : [entry.languageServerRef],
      ),
      refSection('Sources', entry.sourceRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Remediation refs', entry.remediationRefs),
      refSection('Diagnostic blockers', entry.blockerRefs),
    ]),
  ])
}

const diagnosticsPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeDiagnostics(buildForgeDiagnosticsInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Diagnostics',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Language-server readiness, diagnostics freshness, and remediation refs.',
          ]),
        ]),
        badge(view.status.replaceAll('_', ' '), diagnosticsStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Total', String(view.counts.total)),
        compactionMetric('Errors', String(view.counts.errors)),
        compactionMetric('Warnings', String(view.counts.warnings)),
        compactionMetric('Info', String(view.counts.info)),
        compactionMetric('Hints', String(view.counts.hints)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Indexed-at ref', view.indexedAtRef === null ? [] : [view.indexedAtRef]),
        refSection('Workspace boundary', view.workspaceBoundaryRefs),
        refSection('Language servers', view.languageServerRefs),
        refSection('Sources', view.sourceRefs),
        refSection('Diagnostic refs', view.diagnosticRefs),
        refSection('Skipped diagnostics', view.skippedDiagnosticRefs),
        refSection('Policy refs', view.policyRefs),
        refSection('Remediation refs', view.remediationRefs),
        refSection('Diagnostics blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No diagnostics snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(diagnosticEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe diagnostics ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const helpDoctorDebugStatusTone = (
  status: ForgeHelpDoctorDebugStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'warning'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const helpDoctorDebugSeverityTone = (
  severity: ForgeHelpDoctorDebugItem['severity'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  severity === 'critical' || severity === 'error'
    ? 'negative'
    : severity === 'warning'
      ? 'warning'
      : 'info'

const helpDoctorDebugStateTone = (
  state: ForgeHelpDoctorDebugItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'passed'
    ? 'positive'
    : state === 'blocked' || state === 'failed'
      ? 'negative'
      : state === 'warning'
        ? 'warning'
        : 'info'

const helpDoctorDebugEntryPanel = (entry: ForgeHelpDoctorDebugItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.surfaceRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.severity, helpDoctorDebugSeverityTone(entry.severity)),
          badge(entry.state, helpDoctorDebugStateTone(entry.state)),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Surface ref', [entry.surfaceRef]),
      refSection('Help topics', entry.helpTopicRefs),
      refSection('Doctor checks', entry.doctorCheckRefs),
      refSection('Diagnostics', entry.diagnosticRefs),
      refSection('Debug bundles', entry.debugBundleRefs),
      refSection('Remediations', entry.remediationRefs),
      refSection('Sources', entry.sourceRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Help/doctor blockers', entry.blockerRefs),
    ]),
  ])
}

const helpDoctorDebugPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeHelpDoctorDebug(buildForgeHelpDoctorDebugInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Help, doctor, and debug',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Help topic, doctor check, diagnostic, debug bundle, and remediation refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          helpDoctorDebugStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Checks', String(view.counts.total)),
        compactionMetric('Passed', String(view.counts.passed)),
        compactionMetric('Warnings', String(view.counts.warnings)),
        compactionMetric('Failed', String(view.counts.failed)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Help/doctor blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No help/doctor/debug snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(helpDoctorDebugEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe help/doctor/debug ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const mcpServerExportStatusTone = (
  status: ForgeMcpServerExportStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'warning'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const mcpServerExportStateTone = (
  state: ForgeMcpServerExportItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'exposed'
    ? 'positive'
    : state === 'internal_only'
      ? 'accent'
      : state === 'blocked'
        ? 'negative'
        : state === 'planned'
          ? 'warning'
          : state === 'unknown'
            ? 'info'
            : 'accent'

const mcpServerExportEntryPanel = (entry: ForgeMcpServerExportItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.serverRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state.replaceAll('_', ' '), mcpServerExportStateTone(entry.state)),
          badge(entry.freshness, entry.freshness === 'stale' ? 'warning' : 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Server ref', [entry.serverRef]),
      refSection('Capabilities', entry.capabilityRefs),
      refSection('Exported tools', entry.exportedToolRefs),
      refSection('Exported resources', entry.exportedResourceRefs),
      refSection('Exported prompts', entry.exportedPromptRefs),
      refSection('Schemas', entry.schemaRefs),
      refSection('Transports', entry.transportRefs),
      refSection('Auth policies', entry.authPolicyRefs),
      refSection('Audiences', entry.audienceRefs),
      refSection('Trust tiers', entry.trustTierRefs),
      refSection('Invocation receipts', entry.invocationReceiptRefs),
      refSection('Sources', entry.sourceRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('MCP server blockers', entry.blockerRefs),
    ]),
  ])
}

const mcpServerExportPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeMcpServerExport(buildForgeMcpServerExportInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'MCP server export',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Server export, schema, transport, auth, audience, and invocation refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          mcpServerExportStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Servers', String(view.counts.total)),
        compactionMetric('Exposed', String(view.counts.exposed)),
        compactionMetric('Internal', String(view.counts.internalOnly)),
        compactionMetric('Planned', String(view.counts.planned)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('MCP server blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No MCP server export snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(mcpServerExportEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe MCP server ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const settingsConfigurationStatusTone = (
  status: ForgeSettingsConfigurationStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'warning'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const settingsConfigurationStateTone = (
  state: ForgeSettingsConfigurationItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'enabled' || state === 'defaulted'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'overridden'
        ? 'warning'
        : 'info'

const settingsConfigurationEntryPanel = (
  entry: ForgeSettingsConfigurationItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.settingRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state, settingsConfigurationStateTone(entry.state)),
          badge(entry.redactionClass.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Setting ref', [entry.settingRef]),
      refSection('Scopes', entry.scopeRefs),
      refSection('Sources', entry.sourceRefs),
      refSection('Defaults', entry.defaultRefs),
      refSection('Overrides', entry.overrideRefs),
      refSection('Effective values', entry.effectiveValueRefs),
      refSection('Validation refs', entry.validationRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Redaction refs', entry.redactionRefs),
      refSection('Settings blockers', entry.blockerRefs),
    ]),
  ])
}

const settingsConfigurationPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeSettingsConfiguration(
    buildForgeSettingsConfigurationInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Settings and configuration',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Setting, source, effective value, validation, policy, and redaction refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          settingsConfigurationStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Settings', String(view.counts.total)),
        compactionMetric('Enabled', String(view.counts.enabled)),
        compactionMetric('Defaulted', String(view.counts.defaulted)),
        compactionMetric('Overridden', String(view.counts.overridden)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Settings blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No settings/configuration snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(settingsConfigurationEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe settings/configuration ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const credentialStorageStatusTone = (
  status: ForgeCredentialStorageStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'warning'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const credentialStorageStateTone = (
  state: ForgeCredentialStorageItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'usable'
    ? 'positive'
    : state === 'blocked' || state === 'missing'
      ? 'negative'
      : state === 'expired' || state === 'revoked'
        ? 'warning'
        : 'info'

const credentialStorageEntryPanel = (
  entry: ForgeCredentialStorageItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.credentialRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.kind.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state, credentialStorageStateTone(entry.state)),
          badge(entry.redactionClass.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Credential ref', [entry.credentialRef]),
      refSection('Accounts', entry.accountRefs),
      refSection('Storage backends', entry.storageBackendRefs),
      refSection('Scopes', entry.scopeRefs),
      refSection('Entitlements', entry.entitlementRefs),
      refSection('Leases', entry.leaseRefs),
      refSection('Sessions', entry.sessionRefs),
      refSection('Validations', entry.validationRefs),
      refSection('Rotations', entry.rotationRefs),
      refSection('Revocations', entry.revocationRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Redaction refs', entry.redactionRefs),
      refSection('Credential blockers', entry.blockerRefs),
    ]),
  ])
}

const credentialStoragePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeCredentialStorage(buildForgeCredentialStorageInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Authentication and credential storage',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Credential, storage, validation, scope, entitlement, and policy refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          credentialStorageStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Credentials', String(view.counts.total)),
        compactionMetric('Usable', String(view.counts.usable)),
        compactionMetric('Expired', String(view.counts.expired)),
        compactionMetric('Revoked', String(view.counts.revoked)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Credential blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No credential storage snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(credentialStorageEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe credential ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const gitWorkflowStatusTone = (
  status: ForgeGitWorkflowStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'warning'
        ? 'warning'
        : status === 'waiting' || status === 'unknown'
          ? 'info'
          : 'accent'

const gitWorkflowStateTone = (
  state: ForgeGitWorkflowItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'pr_ready' ||
  state === 'review_ready' ||
  state === 'writeback_ready'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'checks_pending'
        ? 'warning'
        : 'info'

const gitWorkflowEntryPanel = (entry: ForgeGitWorkflowItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.workflowRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state.replaceAll('_', ' '), gitWorkflowStateTone(entry.state)),
          badge(entry.freshness, entry.freshness === 'stale' ? 'warning' : 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Workflow ref', [entry.workflowRef]),
      refSection('Repositories', entry.repositoryRefs),
      refSection('Branches', entry.branchRefs),
      refSection('Worktrees', entry.worktreeRefs),
      refSection('Commits', entry.commitRefs),
      refSection('Diffs', entry.diffRefs),
      refSection('Pull requests', entry.prRefs),
      refSection('Issues', entry.issueRefs),
      refSection('Reviews', entry.reviewRefs),
      refSection('Checks', entry.checkRefs),
      refSection('Statuses', entry.statusRefs),
      refSection('Writebacks', entry.writebackRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Git workflow blockers', entry.blockerRefs),
    ]),
  ])
}

const gitWorkflowPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeGitWorkflow(buildForgeGitWorkflowInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Git and GitHub workflow',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Repository, branch, diff, PR, review, check, status, and writeback refs.',
          ]),
        ]),
        badge(view.status.replaceAll('_', ' '), gitWorkflowStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Workflows', String(view.counts.total)),
        compactionMetric('PR ready', String(view.counts.prReady)),
        compactionMetric('Review ready', String(view.counts.reviewReady)),
        compactionMetric('Writeback', String(view.counts.writebackReady)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Git workflow blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No Git/GitHub workflow snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(gitWorkflowEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe Git/GitHub ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const editorIntegrationStatusTone = (
  status: ForgeEditorIntegrationStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'warning'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const editorIntegrationStateTone = (
  state: ForgeEditorIntegrationItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'ready' || state === 'connected'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'disconnected'
        ? 'warning'
        : 'info'

const editorIntegrationEntryPanel = (
  entry: ForgeEditorIntegrationItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.integrationRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state, editorIntegrationStateTone(entry.state)),
          badge(entry.freshness, entry.freshness === 'stale' ? 'warning' : 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Integration ref', [entry.integrationRef]),
      refSection('Editors', entry.editorRefs),
      refSection('Workspaces', entry.workspaceRefs),
      refSection('Extensions', entry.extensionRefs),
      refSection('Commands', entry.commandRefs),
      refSection('Diagnostics', entry.diagnosticRefs),
      refSection('Diagnostic handoffs', entry.diagnosticHandoffRefs),
      refSection('File-open refs', entry.fileOpenRefs),
      refSection('Selections', entry.selectionRefs),
      refSection('Deep links', entry.deepLinkRefs),
      refSection('Statuses', entry.statusRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Editor blockers', entry.blockerRefs),
    ]),
  ])
}

const editorIntegrationPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeEditorIntegration(
    buildForgeEditorIntegrationInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'IDE and editor integration',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Editor, workspace, extension, command, diagnostic, deep-link, and policy refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          editorIntegrationStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Integrations', String(view.counts.total)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Connected', String(view.counts.connected)),
        compactionMetric('Disconnected', String(view.counts.disconnected)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Editor blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No editor integration snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(editorIntegrationEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe editor ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const browserDesktopIntegrationStatusTone = (
  status: ForgeBrowserDesktopIntegrationStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'warning'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const browserDesktopIntegrationStateTone = (
  state: ForgeBrowserDesktopIntegrationItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'ready' || state === 'connected' || state === 'installed'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'unavailable'
        ? 'warning'
        : 'info'

const browserDesktopIntegrationEntryPanel = (
  entry: ForgeBrowserDesktopIntegrationItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.integrationRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state, browserDesktopIntegrationStateTone(entry.state)),
          badge(entry.freshness, entry.freshness === 'stale' ? 'warning' : 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Integration ref', [entry.integrationRef]),
      refSection('Surfaces', entry.surfaceRefs),
      refSection('Browsers', entry.browserRefs),
      refSection('Desktop apps', entry.desktopAppRefs),
      refSection('Extensions', entry.extensionRefs),
      refSection('Deep links', entry.deepLinkRefs),
      refSection('Permissions', entry.permissionRefs),
      refSection('Notifications', entry.notificationRefs),
      refSection('Companions', entry.companionRefs),
      refSection('Installs', entry.installRefs),
      refSection('Updates', entry.updateRefs),
      refSection('Statuses', entry.statusRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Browser/desktop blockers', entry.blockerRefs),
    ]),
  ])
}

const browserDesktopIntegrationPanel = (
  work: AutopilotWorkProjection,
): Html => {
  const h = html<Message>()
  const view = projectForgeBrowserDesktopIntegration(
    buildForgeBrowserDesktopIntegrationInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Browser and desktop integration',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Browser, desktop app, extension, deep-link, permission, notification, and policy refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          browserDesktopIntegrationStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Surfaces', String(view.counts.total)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Connected', String(view.counts.connected)),
        compactionMetric('Installed', String(view.counts.installed)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Browser/desktop blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No browser/desktop integration snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(browserDesktopIntegrationEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe browser/desktop ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const multimodalInputStatusTone = (
  status: ForgeMultimodalInputStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'waiting' || status === 'unknown'
          ? 'info'
          : 'accent'

const multimodalInputStateTone = (
  state: ForgeMultimodalInputItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'ingested' || state === 'capture_ready'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'pending'
        ? 'warning'
        : 'info'

const multimodalInputEntryPanel = (entry: ForgeMultimodalInputItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.inputRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.modality} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state.replaceAll('_', ' '), multimodalInputStateTone(entry.state)),
          badge(entry.modality, 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Input ref', [entry.inputRef]),
      refSection('Capture surfaces', entry.captureSurfaceRefs),
      refSection('Attachments', entry.attachmentRefs),
      refSection('Transcripts', entry.transcriptRefs),
      refSection('VAD/endpoint refs', [...entry.vadRefs, ...entry.endpointRefs]),
      refSection('Consent refs', entry.consentRefs),
      refSection('Redaction refs', entry.redactionRefs),
      refSection('Context ingestion', entry.contextIngestionRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Multimodal blockers', entry.blockerRefs),
    ]),
  ])
}

const multimodalInputPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeMultimodalInput(buildForgeMultimodalInputInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Voice and multimodal input',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Capture, attachment, transcript, VAD, consent, redaction, and ingestion refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          multimodalInputStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Inputs', String(view.counts.total)),
        compactionMetric('Ingested', String(view.counts.ingested)),
        compactionMetric('Capture', String(view.counts.captureReady)),
        compactionMetric('Pending', String(view.counts.pending)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Multimodal blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No voice/multimodal input snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(multimodalInputEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe multimodal ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const remoteSessionBridgeStatusTone = (
  status: ForgeRemoteSessionBridgeStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'reconnecting' || status === 'unknown'
          ? 'info'
          : 'accent'

const remoteSessionBridgeStateTone = (
  state: ForgeRemoteSessionBridgeItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'ready' || state === 'connected'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'reconnecting'
        ? 'warning'
        : 'info'

const remoteSessionBridgeEntryPanel = (
  entry: ForgeRemoteSessionBridgeItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.bridgeRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state, remoteSessionBridgeStateTone(entry.state)),
          badge(entry.freshness, entry.freshness === 'stale' ? 'warning' : 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Bridge ref', [entry.bridgeRef]),
      refSection('Sessions', entry.sessionRefs),
      refSection('Transports', entry.transportRefs),
      refSection('Protocols', entry.protocolRefs),
      refSection('Controllers', entry.controllerRefs),
      refSection('Heartbeats', entry.heartbeatRefs),
      refSection('Reconnects', entry.reconnectRefs),
      refSection('Permissions', entry.permissionRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Remote bridge blockers', entry.blockerRefs),
    ]),
  ])
}

const remoteSessionBridgePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeRemoteSessionBridge(
    buildForgeRemoteSessionBridgeInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Remote Session Bridge',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Bridge, session, transport, protocol, controller, heartbeat, reconnect, and policy refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          remoteSessionBridgeStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Bridges', String(view.counts.total)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Connected', String(view.counts.connected)),
        compactionMetric('Reconnect', String(view.counts.reconnecting)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Remote bridge blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No remote session bridge snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(remoteSessionBridgeEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe remote bridge ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const companionSurfaceStatusTone = (
  status: ForgeCompanionSurfaceStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale' || status === 'lagged'
        ? 'warning'
        : status === 'waiting'
          ? 'accent'
          : 'info'

const companionSurfaceStateTone = (
  state: ForgeCompanionSurfaceItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'ready'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'waiting'
        ? 'accent'
        : state === 'offline'
          ? 'warning'
          : 'info'

const companionSurfaceEntryPanel = (
  entry: ForgeCompanionSurfaceItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.companionRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.state.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.state.replaceAll('_', ' '),
            companionSurfaceStateTone(entry.state),
          ),
          badge(
            entry.freshness,
            entry.freshness === 'stale' || entry.freshness === 'lagged'
              ? 'warning'
              : 'accent',
          ),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Companion ref', [entry.companionRef]),
      refSection('Surfaces', entry.surfaceRefs),
      refSection('Pairings', entry.pairingRefs),
      refSection('Streams', entry.streamRefs),
      refSection('Cursors', entry.cursorRefs),
      refSection('Runs', entry.runRefs),
      refSection('Sessions', entry.sessionRefs),
      refSection('Decisions', entry.decisionRefs),
      refSection('Notifications', entry.notificationRefs),
      refSection('Artifacts', entry.artifactRefs),
      refSection('Closeouts', entry.closeoutRefs),
      refSection('Progress', entry.progressRefs),
      refSection('Budgets', entry.budgetRefs),
      refSection('Actions', entry.actionRefs),
      refSection('Capabilities', entry.capabilityRefs),
      refSection('Idempotency', entry.idempotencyRefs),
      refSection('Delivery tiers', entry.deliveryTierRefs),
      refSection('Receipts', entry.receiptRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Lag refs', entry.lagRefs),
      refSection('Companion blockers', entry.blockerRefs),
    ]),
  ])
}

const companionSurfacePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeCompanionSurface(buildForgeCompanionSurfaceInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Mobile and web companion',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Companion, pairing, stream, cursor, decision, notification, artifact, action, capability, and receipt refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          companionSurfaceStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-6')], [
        compactionMetric('Companions', String(view.counts.total)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Waiting', String(view.counts.waiting)),
        compactionMetric('Read-only', String(view.counts.readOnly)),
        compactionMetric('Offline', String(view.counts.offline)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Companion blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No companion surface snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(companionSurfaceEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe companion ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const teamSharedMemoryStatusTone = (
  status: ForgeTeamSharedMemoryStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'pending_review'
          ? 'accent'
          : 'info'

const teamSharedMemoryReviewTone = (
  state: ForgeTeamSharedMemoryItem['reviewState'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'accepted'
    ? 'positive'
    : state === 'deleted' || state === 'rejected'
      ? 'negative'
      : state === 'pending_review' || state === 'tentative'
        ? 'warning'
        : 'info'

const teamSharedMemoryEntryPanel = (
  entry: ForgeTeamSharedMemoryItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.memoryRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.scope} / ${entry.kind.replaceAll('_', ' ')} / ${entry.visibility}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.reviewState.replaceAll('_', ' '),
            teamSharedMemoryReviewTone(entry.reviewState),
          ),
          badge(
            entry.freshness,
            entry.freshness === 'stale' ? 'warning' : 'accent',
          ),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Memory ref', [entry.memoryRef]),
      refSection('Owners', entry.ownerRefs),
      refSection('Teams', entry.teamRefs),
      refSection('Evidence', entry.evidenceRefs),
      refSection('Retrieval policies', entry.retrievalPolicyRefs),
      refSection('Typed queries', entry.typedQueryRefs),
      refSection('Semantic queries', entry.semanticQueryRefs),
      refSection('Application receipts', entry.applicationReceiptRefs),
      refSection('Deletion receipts', entry.deletionReceiptRefs),
      refSection('Tombstones', entry.tombstoneRefs),
      refSection('Consent refs', entry.consentRefs),
      refSection('Promotions', entry.promotionRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Review refs', entry.reviewRefs),
      refSection('Expiry refs', entry.expiryRefs),
      refSection('Shared memory blockers', entry.blockerRefs),
    ]),
  ])
}

const teamSharedMemoryPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeTeamSharedMemory(
    buildForgeTeamSharedMemoryInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Team and shared memory',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Scoped memory, evidence, retrieval policy, application, deletion, consent, promotion, and visibility refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          teamSharedMemoryStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-6')], [
        compactionMetric('Memories', String(view.counts.total)),
        compactionMetric('Accepted', String(view.counts.accepted)),
        compactionMetric('Pending', String(view.counts.pendingReview)),
        compactionMetric('Team', String(view.counts.teamVisible)),
        compactionMetric('Public', String(view.counts.publicVisible)),
        compactionMetric('Stale', String(view.counts.stale)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Projection ref', view.projectionRef === null ? [] : [view.projectionRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Shared memory blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No team shared memory snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(teamSharedMemoryEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe shared-memory ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const multiAgentCoordinationStatusTone = (
  status: ForgeMultiAgentCoordinationStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'running'
          ? 'accent'
          : 'info'

const multiAgentCoordinationStateTone = (
  state: ForgeMultiAgentCoordinationItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'completed' || state === 'merged'
    ? 'positive'
    : state === 'blocked' || state === 'failed'
      ? 'negative'
      : state === 'running'
        ? 'accent'
        : state === 'waiting' || state === 'planned'
          ? 'warning'
          : 'info'

const multiAgentCoordinationEntryPanel = (
  entry: ForgeMultiAgentCoordinationItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.laneRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.criticality} / ${entry.kind} / ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.state.replaceAll('_', ' '),
            multiAgentCoordinationStateTone(entry.state),
          ),
          badge(entry.criticality, entry.criticality === 'mandatory' ? 'accent' : 'info'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Lane ref', [entry.laneRef]),
      refSection('Assignments', entry.assignmentRefs),
      refSection('Dependencies', entry.dependencyRefs),
      refSection('Budget caps', entry.budgetCapRefs),
      refSection('Providers', entry.providerRefs),
      refSection('Adapters', entry.adapterRefs),
      refSection('Capabilities', entry.capabilityRefs),
      refSection('Artifacts', entry.artifactRefs),
      refSection('Receipts', entry.receiptRefs),
      refSection('Conflicts', entry.conflictRefs),
      refSection('Merge strategies', entry.mergeStrategyRefs),
      refSection('Lane inbox', entry.inboxRefs),
      refSection('Steering receipts', entry.steeringReceiptRefs),
      refSection('Closeouts', entry.closeoutRefs),
      refSection('Acceptance policies', entry.acceptancePolicyRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Coordination blockers', entry.blockerRefs),
    ]),
  ])
}

const multiAgentCoordinationPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeMultiAgentCoordination(
    buildForgeMultiAgentCoordinationInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Multi-agent coordination',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Coordination plan, lane, assignment, dependency, budget, provider, capability, artifact, receipt, conflict, inbox, and closeout refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          multiAgentCoordinationStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-6')], [
        compactionMetric('Lanes', String(view.counts.total)),
        compactionMetric('Mandatory', String(view.counts.mandatory)),
        compactionMetric('Running', String(view.counts.running)),
        compactionMetric('Completed', String(view.counts.completed)),
        compactionMetric('Failed', String(view.counts.failedMandatory)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Plan ref', view.planRef === null ? [] : [view.planRef]),
        refSection('Parent run', view.parentRunRef === null ? [] : [view.parentRunRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Coordination blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No multi-agent coordination snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(multiAgentCoordinationEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe coordination ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const externalWorkIntakeStatusTone = (
  status: ForgeExternalWorkIntakeStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'admitted' || status === 'routed' || status === 'delivered'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
    : status === 'expired' || status === 'rejected' || status === 'stale'
        ? 'warning'
        : status === 'pending'
          ? 'accent'
          : 'info'

const externalWorkIntakeEntryPanel = (
  entry: ForgeExternalWorkIntakeItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.intakeRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.channel.replaceAll('_', ' ')} / ${entry.workKind.replaceAll('_', ' ')} / ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.status.replaceAll('_', ' '),
            externalWorkIntakeStatusTone(entry.status),
          ),
          badge(entry.budgetRequired ? 'budget required' : 'budget optional', 'info'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Intake ref', [entry.intakeRef]),
      refSection('Requests', entry.requestRefs),
      refSection('Requesters', entry.requesterRefs),
      refSection('Accounts', entry.accountRefs),
      refSection('Scopes', entry.scopeRefs),
      refSection('Data classes', entry.dataClassificationRefs),
      refSection('Capabilities', entry.capabilityRefs),
      refSection('Adapter preferences', entry.adapterPreferenceRefs),
      refSection('Budgets', entry.budgetRefs),
      refSection('Payments', entry.paymentRefs),
      refSection('Verification', entry.verificationRefs),
      refSection('Acceptance policies', entry.acceptancePolicyRefs),
      refSection('Review policies', entry.reviewPolicyRefs),
      refSection('Idempotency', entry.idempotencyRefs),
      refSection('Admission receipts', entry.admissionReceiptRefs),
      refSection('Rejection receipts', entry.rejectionReceiptRefs),
      refSection('Routing receipts', entry.routingReceiptRefs),
      refSection('Work orders', entry.workOrderRefs),
      refSection('Status receipts', entry.statusReceiptRefs),
      refSection('Delivery receipts', entry.deliveryReceiptRefs),
      refSection('API parity', entry.apiParityRefs),
      refSection('Expiration', entry.expirationRefs),
      refSection('Policies', entry.policyRefs),
      refSection('Intake blockers', entry.blockerRefs),
    ]),
  ])
}

const externalWorkIntakePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeExternalWorkIntake(
    buildForgeExternalWorkIntakeInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'External work intake',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Request, requester, account, scope, capability, adapter, budget, payment, admission, routing, work-order, and API parity refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          externalWorkIntakeStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-6')], [
        compactionMetric('Intakes', String(view.counts.total)),
        compactionMetric('Pending', String(view.counts.pending)),
        compactionMetric('Admitted', String(view.counts.admitted)),
        compactionMetric('Routed', String(view.counts.routed)),
        compactionMetric('Rejected', String(view.counts.rejected)),
        compactionMetric('Delivered', String(view.counts.delivered)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Intake blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No external work intake snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(externalWorkIntakeEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe intake ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const artifactReceiptIndexStatusTone = (
  status: ForgeArtifactReceiptIndexStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const artifactReceiptFreshnessTone = (
  freshness: ForgeArtifactReceiptArtifactItem['freshness'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  freshness === 'fresh'
    ? 'positive'
    : freshness === 'stale'
      ? 'warning'
      : 'info'

const artifactReceiptArtifactPanel = (
  artifact: ForgeArtifactReceiptArtifactItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            artifact.artifactRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${artifact.kind.replaceAll('_', ' ')} / ${artifact.visibility} / ${artifact.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(artifact.freshness, artifactReceiptFreshnessTone(artifact.freshness)),
          badge(artifact.redactionClass.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Artifact ref', [artifact.artifactRef]),
      refSection('Digest refs', artifact.digestRefs),
      refSection('Media refs', artifact.mediaTypeRefs),
      refSection('Size refs', artifact.sizeRefs),
      refSection('Summary refs', artifact.summaryRefs),
      refSection('Subject refs', artifact.subjectRefs),
      refSection('Producer refs', artifact.producerRefs),
      refSection('Related receipts', artifact.relatedReceiptRefs),
      refSection('Run refs', artifact.runRefs),
      refSection('Work orders', artifact.workOrderRefs),
      refSection('Assignments', artifact.assignmentRefs),
      refSection('Lanes', artifact.laneRefs),
      refSection('Missions', artifact.missionRefs),
      refSection('Retention refs', artifact.retentionRefs),
      refSection('Policy refs', artifact.policyRefs),
      refSection('Artifact blockers', artifact.blockerRefs),
    ]),
  ])
}

const artifactReceiptReceiptPanel = (
  receipt: ForgeArtifactReceiptReceiptItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            receipt.receiptRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${receipt.transitionKind.replaceAll('_', ' ')} / ${receipt.freshness}`,
          ]),
        ]),
        badge(receipt.freshness, artifactReceiptFreshnessTone(receipt.freshness)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Receipt ref', [receipt.receiptRef]),
      refSection('Subjects', receipt.subjectRefs),
      refSection('Actors', receipt.actorRefs),
      refSection('Services', receipt.serviceRefs),
      refSection('Idempotency', receipt.idempotencyRefs),
      refSection('Inputs', receipt.inputRefs),
      refSection('Outputs', receipt.outputRefs),
      refSection('Policies', receipt.policyRefs),
      refSection('Verification', receipt.verificationRefs),
      refSection('Caveats', receipt.caveatRefs),
      refSection('Claim requirements', receipt.claimRequirementRefs),
      refSection('Satisfying receipts', receipt.satisfyingReceiptRefs),
      refSection('Receipt blockers', receipt.blockerRefs),
    ]),
  ])
}

const artifactReceiptIndexPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeArtifactReceiptIndex(
    buildForgeArtifactReceiptIndexInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Artifact and receipt index',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Artifact, digest, visibility, producer, related receipt, transition, idempotency, policy, verification, caveat, and claim requirement refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          artifactReceiptIndexStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
        compactionMetric('Artifacts', String(view.counts.artifacts)),
        compactionMetric('Public', String(view.counts.publicArtifacts)),
        compactionMetric('Receipts', String(view.counts.receipts)),
        compactionMetric('Stale', String(view.counts.stale)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Index blockers', view.blockerRefs),
      ]),
      view.artifacts.length === 0 && view.receipts.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No artifact and receipt index snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-4')], [
            h.div([Ui.className<Message>('grid gap-3')], [
              h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/65')], [
                'Artifacts',
              ]),
              view.artifacts.length === 0
                ? h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
                    'No artifact refs available.',
                  ])
                : h.div([Ui.className<Message>('grid gap-3')], [
                    ...view.artifacts.map(artifactReceiptArtifactPanel),
                  ]),
            ]),
            h.div([Ui.className<Message>('grid gap-3')], [
              h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/65')], [
                'Receipts',
              ]),
              view.receipts.length === 0
                ? h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
                    'No receipt refs available.',
                  ])
                : h.div([Ui.className<Message>('grid gap-3')], [
                    ...view.receipts.map(artifactReceiptReceiptPanel),
                  ]),
            ]),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe artifact/receipt ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const schedulingCronStatusTone = (
  status: ForgeSchedulingCronViewStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const schedulingCronScheduleStatusTone = (
  status: ForgeSchedulingCronItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'active' || status === 'fired'
    ? 'positive'
    : status === 'blocked' || status === 'failed'
      ? 'negative'
      : status === 'skipped' || status === 'cancelled'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const schedulingCronSchedulePanel = (
  schedule: ForgeSchedulingCronItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            schedule.scheduleRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${schedule.triggerKind.replaceAll('_', ' ')} / ${schedule.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(schedule.status, schedulingCronScheduleStatusTone(schedule.status)),
          badge(schedule.triggerKind.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Schedule ref', [schedule.scheduleRef]),
      refSection('Owners', schedule.ownerRefs),
      refSection('Teams', schedule.teamRefs),
      refSection('Work templates', schedule.workOrderTemplateRefs),
      refSection('Workspaces', schedule.workspaceRefs),
      refSection('Repos', schedule.repoRefs),
      refSection('Timezone refs', schedule.timezoneRefs),
      refSection('Next run refs', schedule.nextRunRefs),
      refSection('Last run refs', schedule.lastRunRefs),
      refSection('Budget policies', schedule.budgetPolicyRefs),
      refSection('Permission policies', schedule.permissionPolicyRefs),
      refSection('Provider preferences', schedule.providerPreferenceRefs),
      refSection('Adapter preferences', schedule.adapterPreferenceRefs),
      refSection('Notification policies', schedule.notificationPolicyRefs),
      refSection('Retention policies', schedule.retentionPolicyRefs),
      refSection('Continuation policies', schedule.continuationPolicyRefs),
      refSection('Fire receipts', schedule.fireReceiptRefs),
      refSection('Run receipts', schedule.runReceiptRefs),
      refSection('Skip receipts', schedule.skipReceiptRefs),
      refSection('Failure receipts', schedule.failureReceiptRefs),
      refSection('Cancel receipts', schedule.cancelReceiptRefs),
      refSection('No double-fire receipts', schedule.noDoubleFireReceiptRefs),
      refSection('Schedule blockers', schedule.blockerRefs),
    ]),
  ])
}

const schedulingCronPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeSchedulingCron(buildForgeSchedulingCronInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Scheduling and cron',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Schedule, trigger, timezone, budget, permission, provider, notification, continuation, receipt, and no-double-fire refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          schedulingCronStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-7')], [
        compactionMetric('Schedules', String(view.counts.schedules)),
        compactionMetric('Active', String(view.counts.active)),
        compactionMetric('Paused', String(view.counts.paused)),
        compactionMetric('Fired', String(view.counts.fired)),
        compactionMetric('Skipped', String(view.counts.skipped)),
        compactionMetric('Failed', String(view.counts.failed)),
        compactionMetric('Stale', String(view.counts.stale)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Schedule blockers', view.blockerRefs),
      ]),
      view.schedules.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No scheduling and cron snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.schedules.map(schedulingCronSchedulePanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe scheduling/cron ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const structuredEventLogStatusTone = (
  status: ForgeStructuredEventLogStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const structuredEventStateTone = (
  status: ForgeStructuredEventLogItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'appended' || status === 'projected' || status === 'replayed'
    ? 'positive'
    : status === 'failed'
      ? 'negative'
      : status === 'skipped'
        ? 'warning'
        : 'info'

const structuredEventPanel = (event: ForgeStructuredEventLogItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            `${event.sequence}. ${event.eventRef}`,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${event.eventKind.replaceAll('_', ' ')} / ${event.visibility} / ${event.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(event.status, structuredEventStateTone(event.status)),
          badge(event.redactionClass.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Event ref', [event.eventRef]),
      refSection('Sequence ref', event.sequenceRef === null ? [] : [event.sequenceRef]),
      refSection('Run refs', event.runRefs),
      refSection('Subjects', event.subjectRefs),
      refSection('Actors', event.actorRefs),
      refSection('Services', event.serviceRefs),
      refSection('Timestamp refs', event.timestampRefs),
      refSection('Schema versions', event.payloadSchemaVersionRefs),
      refSection('Idempotency', event.idempotencyRefs),
      refSection('Parents', event.parentRefs),
      refSection('Correlations', event.correlationRefs),
      refSection('Replay refs', event.replayRefs),
      refSection('Projection refs', event.projectionRefs),
      refSection('Export refs', event.exportRefs),
      refSection('Retention refs', event.retentionRefs),
      refSection('Policy refs', event.policyRefs),
      refSection('Event blockers', event.blockerRefs),
    ]),
  ])
}

const structuredEventLogPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeStructuredEventLog(
    buildForgeStructuredEventLogInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Structured event log',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Append-only event stream, sequence, schema, idempotency, replay, export, retention, visibility, and redaction refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          structuredEventLogStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-6')], [
        compactionMetric('Events', String(view.counts.events)),
        compactionMetric('Public', String(view.counts.publicEvents)),
        compactionMetric('Team', String(view.counts.teamEvents)),
        compactionMetric('Private', String(view.counts.privateEvents)),
        compactionMetric('Failed', String(view.counts.failed)),
        compactionMetric('Stale', String(view.counts.stale)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Event streams', view.eventStreamRefs),
        refSection('Replay refs', view.replayRefs),
        refSection('Projection refs', view.projectionRefs),
        refSection('Export refs', view.exportRefs),
        refSection('Retention refs', view.retentionRefs),
        refSection('Policy refs', view.policyRefs),
        refSection('Event-log blockers', view.blockerRefs),
      ]),
      view.events.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No structured event-log snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.events.map(structuredEventPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe structured event ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const telemetryPrivacyStatusTone = (
  status: ForgeTelemetryPrivacyStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready' || status === 'disabled'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const telemetryClassStatusTone = (
  status: ForgeTelemetryPrivacyItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'enabled' || status === 'disabled'
    ? 'positive'
    : status === 'failed' || status === 'blocked'
      ? 'negative'
      : 'info'

const telemetryPrivacyClassPanel = (
  item: ForgeTelemetryPrivacyItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            item.telemetryRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${item.classKind.replaceAll('_', ' ')} / ${item.mode.replaceAll('_', ' ')} / ${item.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(item.status, telemetryClassStatusTone(item.status)),
          badge(item.classKind.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Telemetry ref', [item.telemetryRef]),
      refSection('Sinks', item.sinkRefs),
      refSection('Visibility refs', item.visibilityRefs),
      refSection('Retention refs', item.retentionRefs),
      refSection('Exportability refs', item.exportabilityRefs),
      refSection('Opt-out refs', item.optOutRefs),
      refSection('Policy refs', item.policyRefs),
      refSection('Privacy filters', item.privacyFilterRefs),
      refSection('Redaction scans', item.redactionScanRefs),
      refSection('Aggregates', item.aggregateRefs),
      refSection('Diagnostic bundles', item.diagnosticBundleRefs),
      refSection('Delivery refs', item.deliveryRefs),
      refSection('Failure refs', item.failureRefs),
      refSection('Telemetry blockers', item.blockerRefs),
    ]),
  ])
}

const telemetryPrivacyPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeTelemetryPrivacy(
    buildForgeTelemetryPrivacyInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Telemetry and privacy',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Telemetry mode, class, sink, opt-out, policy, redaction, aggregate, export, and retention refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          telemetryPrivacyStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-6')], [
        compactionMetric('Classes', String(view.counts.telemetryClasses)),
        compactionMetric('Enabled', String(view.counts.enabled)),
        compactionMetric('Disabled', String(view.counts.disabled)),
        compactionMetric('Product', String(view.counts.product)),
        compactionMetric('Failed', String(view.counts.failed)),
        compactionMetric('Stale', String(view.counts.stale)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Mode refs', view.modeRefs),
        refSection('Sinks', view.sinkRefs),
        refSection('Opt-out refs', view.optOutRefs),
        refSection('Policy refs', view.policyRefs),
        refSection('Privacy filters', view.privacyFilterRefs),
        refSection('Redaction scans', view.redactionScanRefs),
        refSection('Retention refs', view.retentionRefs),
        refSection('Telemetry blockers', view.blockerRefs),
      ]),
      view.items.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No telemetry/privacy snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.items.map(telemetryPrivacyClassPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe telemetry/privacy ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const performanceDiagnosticsStatusTone = (
  status: ForgePerformanceDiagnosticsStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'slow' || status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const performanceEntryStatusTone = (
  status: ForgePerformanceDiagnosticsItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ok'
    ? 'positive'
    : status === 'blocked' || status === 'failed'
      ? 'negative'
      : status === 'slow' || status === 'truncated'
        ? 'warning'
        : 'info'

const performanceDiagnosticsEntryPanel = (
  entry: ForgePerformanceDiagnosticsItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.spanRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.resourceClass.replaceAll('_', ' ')} / ${entry.latencyClass} / ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status, performanceEntryStatusTone(entry.status)),
          badge(entry.resourceClass.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Span ref', [entry.spanRef]),
      refSection('Counters', entry.counterRefs),
      refSection('Runs', entry.runRefs),
      refSection('Backpressure refs', entry.backpressureRefs),
      refSection('Timeout refs', entry.timeoutRefs),
      refSection('Output volume refs', entry.outputVolumeRefs),
      refSection('Truncation refs', entry.truncationRefs),
      refSection('Preserved artifacts', entry.artifactRefs),
      refSection('Provider rate limits', entry.providerRateLimitRefs),
      refSection('Budget stops', entry.budgetStopRefs),
      refSection('Local pressure', entry.localResourcePressureRefs),
      refSection('Profile refs', entry.profileRefs),
      refSection('Redaction refs', entry.redactionRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Performance blockers', entry.blockerRefs),
    ]),
  ])
}

const performanceDiagnosticsPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgePerformanceDiagnostics(
    buildForgePerformanceDiagnosticsInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Performance diagnostics',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Latency, throughput, queue, backpressure, timeout, output volume, truncation, profile, rate-limit, and resource-pressure refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          performanceDiagnosticsStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Entries', String(view.counts.entries)),
        compactionMetric('Slow', String(view.counts.slow)),
        compactionMetric('Blocked', String(view.counts.blocked)),
        compactionMetric('Truncated', String(view.counts.truncated)),
        compactionMetric('Stale', String(view.counts.stale)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Profile refs', view.profileRefs),
        refSection('Performance blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No performance diagnostics snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(performanceDiagnosticsEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe performance ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const updateReleaseStatusTone = (
  status: ForgeUpdateReleaseStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'current'
    ? 'positive'
    : status === 'blocked' || status === 'failed' || status === 'required'
      ? 'negative'
      : status === 'update_available'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const updateReleaseEntryTone = (
  status: ForgeUpdateReleaseItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'current'
    ? 'positive'
    : status === 'blocked' || status === 'failed' || status === 'required'
      ? 'negative'
      : status === 'available' || status === 'recommended'
        ? 'warning'
        : 'info'

const updateReleaseEntryPanel = (entry: ForgeUpdateReleaseItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.releaseRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.channel} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status.replaceAll('_', ' '), updateReleaseEntryTone(entry.status)),
          badge(entry.channel, 'accent'),
          entry.managedOverride ? badge('managed pin', 'info') : h.span([Ui.className<Message>('hidden')], []),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Release ref', [entry.releaseRef]),
      refSection('Version ref', [entry.versionRef]),
      refSection('Channel refs', entry.channelRefs),
      refSection('Manifest refs', entry.manifestRefs),
      refSection('Artifacts', entry.artifactRefs),
      refSection('Checksums', entry.checksumRefs),
      refSection('Signatures', entry.signatureRefs),
      refSection('Platforms', entry.platformRefs),
      refSection('Compatibility', entry.compatibilityRefs),
      refSection('Runtime requirements', entry.runtimeRequirementRefs),
      refSection('Migration refs', entry.migrationRefs),
      refSection('Restore points', entry.restorePointRefs),
      refSection('Rollback refs', entry.rollbackRefs),
      refSection('Smoke receipts', entry.smokeReceiptRefs),
      refSection('Release notes', entry.releaseNoteRefs),
      refSection('Rollout refs', entry.rolloutRefs),
      refSection('Managed pins', entry.managedPinRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Active runs', entry.activeRunRefs),
      refSection('Safe update windows', entry.safeUpdateWindowRefs),
      refSection('Deprecation refs', entry.deprecationRefs),
      refSection('Support refs', entry.supportRefs),
      refSection('Known blockers', entry.knownBlockerRefs),
      refSection('Release blockers', entry.blockerRefs),
    ]),
  ])
}

const updateReleasePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeUpdateRelease(buildForgeUpdateReleaseInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Update and release',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Manifest, channel, platform, integrity, compatibility, smoke, rollback, migration, managed-policy, and support refs.',
          ]),
        ]),
        badge(view.status.replaceAll('_', ' '), updateReleaseStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Entries', String(view.counts.entries)),
        compactionMetric('Required', String(view.counts.required)),
        compactionMetric('Available', String(view.counts.available)),
        compactionMetric('Managed', String(view.counts.managed)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Manifest refs', view.manifestRefs),
        refSection('Policy refs', view.policyRefs),
        refSection('Update blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No update/release snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(updateReleaseEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe update/release ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const migrationEvidenceStatusTone = (
  status: ForgeMigrationEvidenceStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked' || status === 'failed' || status === 'required'
      ? 'negative'
      : status === 'unknown'
        ? 'info'
        : 'accent'

const migrationEvidenceEntryTone = (
  status: ForgeMigrationEvidenceItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'completed' || status === 'skipped'
    ? 'positive'
    : status === 'blocked' || status === 'failed' || status === 'required'
      ? 'negative'
      : status === 'pending' || status === 'rebuildable'
        ? 'warning'
        : 'info'

const migrationEvidenceEntryPanel = (
  entry: ForgeMigrationEvidenceItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.domainRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.domain.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status.replaceAll('_', ' '), migrationEvidenceEntryTone(entry.status)),
          badge(entry.domain.replaceAll('_', ' '), 'accent'),
          entry.required ? badge('required', 'negative') : h.span([Ui.className<Message>('hidden')], []),
          entry.optionalCache ? badge('optional cache', 'info') : h.span([Ui.className<Message>('hidden')], []),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Domain ref', [entry.domainRef]),
      refSection('Schema from', [entry.schemaFromRef]),
      refSection('Schema to', [entry.schemaToRef]),
      refSection('Registry refs', entry.registryRefs),
      refSection('Migration steps', entry.migrationRefs),
      refSection('Idempotency refs', entry.idempotencyRefs),
      refSection('Restore points', entry.restorePointRefs),
      refSection('Rollback boundaries', entry.rollbackBoundaryRefs),
      refSection('Validation refs', entry.validationRefs),
      refSection('Receipt refs', entry.receiptRefs),
      refSection('Optional cache rebuilds', entry.optionalCacheRebuildRefs),
      refSection('Downgrade refs', entry.downgradeRefs),
      refSection('Recovery refs', entry.recoveryRefs),
      refSection('Redaction refs', entry.redactionRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Migration blockers', entry.blockerRefs),
    ]),
  ])
}

const migrationEvidencePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeMigrationEvidence(
    buildForgeMigrationEvidenceInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Migration evidence',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Domain, schema, idempotency, restore, rollback, validation, receipt, cache rebuild, downgrade, recovery, redaction, and policy refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          migrationEvidenceStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Domains', String(view.counts.domains)),
        compactionMetric('Completed', String(view.counts.completed)),
        compactionMetric('Required', String(view.counts.required)),
        compactionMetric('Rebuildable', String(view.counts.rebuildable)),
        compactionMetric('Failed', String(view.counts.failed)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Registry refs', view.registryRefs),
        refSection('Migration blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No migration evidence snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(migrationEvidenceEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe migration ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const testingSmokeEvidenceStatusTone = (
  status: ForgeTestingSmokeEvidenceStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked' || status === 'failed'
      ? 'negative'
      : status === 'pending'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const testingSmokeEvidenceEntryTone = (
  status: ForgeTestingSmokeEvidenceItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'passed' || status === 'skipped'
    ? 'positive'
    : status === 'blocked' || status === 'failed' || status === 'stale'
      ? 'negative'
      : status === 'pending'
        ? 'warning'
        : 'info'

const testingSmokeEvidenceEntryPanel = (
  entry: ForgeTestingSmokeEvidenceItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.testRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.layer.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status, testingSmokeEvidenceEntryTone(entry.status)),
          badge(entry.layer.replaceAll('_', ' '), 'accent'),
          ...entry.classifications.map(classification =>
            badge(classification.replaceAll('_', ' '), 'info'),
          ),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Test ref', [entry.testRef]),
      refSection('Commands', entry.commandRefs),
      refSection('Fixtures', entry.fixtureRefs),
      refSection('Environment refs', entry.environmentRefs),
      refSection('Version refs', entry.versionRefs),
      refSection('Proof boundaries', entry.proofBoundaryRefs),
      refSection('Product claims', entry.productClaimRefs),
      refSection('Smoke receipts', entry.smokeReceiptRefs),
      refSection('Redaction scans', entry.redactionScanRefs),
      refSection('Adapter availability', entry.adapterAvailabilityRefs),
      refSection('Workspace availability', entry.workspaceAvailabilityRefs),
      refSection('Provider availability', entry.providerAvailabilityRefs),
      refSection('Credential availability', entry.credentialAvailabilityRefs),
      refSection('Approval refs', entry.approvalRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Failure refs', entry.failureRefs),
      refSection('Testing blockers', entry.blockerRefs),
    ]),
  ])
}

const testingSmokeEvidencePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeTestingSmokeEvidence(
    buildForgeTestingSmokeEvidenceInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Testing and smoke',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Test layer, command, fixture, environment, proof-boundary, smoke receipt, redaction, approval, policy, and failure refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          testingSmokeEvidenceStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Entries', String(view.counts.entries)),
        compactionMetric('Passed', String(view.counts.passed)),
        compactionMetric('Failed', String(view.counts.failed)),
        compactionMetric('Live', String(view.counts.live)),
        compactionMetric('Paid', String(view.counts.paid)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Testing blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No testing/smoke snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(testingSmokeEvidenceEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe testing/smoke ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const evaluationRegressionStatusTone = (
  status: ForgeEvaluationRegressionEvidenceStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'passed'
    ? 'positive'
    : status === 'blocked' || status === 'failed' || status === 'regressed'
      ? 'negative'
      : status === 'pending'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const evaluationRegressionEntryTone = (
  status: ForgeEvaluationRegressionEvidenceItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'passed'
    ? 'positive'
    : status === 'blocked' || status === 'failed' || status === 'regressed'
      ? 'negative'
      : status === 'pending'
        ? 'warning'
        : 'info'

const evaluationRegressionEntryPanel = (
  entry: ForgeEvaluationRegressionEvidenceItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.evaluationRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.status} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status, evaluationRegressionEntryTone(entry.status)),
          entry.publicReportRefs.length > 0
            ? badge('public report', 'info')
            : h.span([Ui.className<Message>('hidden')], []),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Evaluation ref', [entry.evaluationRef]),
      refSection('Suites', entry.suiteRefs),
      refSection('Fixtures', entry.fixtureRefs),
      refSection('Fixture provenance', entry.fixtureProvenanceRefs),
      refSection('Fixture redaction', entry.fixtureRedactionRefs),
      refSection('Adapters', entry.adapterRefs),
      refSection('Providers', entry.providerRefs),
      refSection('Models', entry.modelRefs),
      refSection('Runtime versions', entry.versionRefs),
      refSection('Tool policy', entry.toolPolicyRefs),
      refSection('Budget policy', entry.budgetPolicyRefs),
      refSection('Result verdicts', entry.resultVerdictRefs),
      refSection('First divergence', entry.firstDivergenceRefs),
      refSection('Artifacts', entry.artifactRefs),
      refSection('Cost summaries', entry.costSummaryRefs),
      refSection('Latency summaries', entry.latencySummaryRefs),
      refSection('Safety verdicts', entry.safetyVerdictRefs),
      refSection('Public reports', entry.publicReportRefs),
      refSection('Private report refs', entry.privateReportRefs),
      refSection('Regression gates', entry.regressionGateRefs),
      refSection('Thresholds', entry.thresholdRefs),
      refSection('Fixture promotions', entry.fixturePromotionRefs),
      refSection('Review refs', entry.reviewRefs),
      refSection('Failures', entry.failureRefs),
      refSection('Eval blockers', entry.blockerRefs),
    ]),
  ])
}

const evaluationRegressionEvidencePanel = (
  work: AutopilotWorkProjection,
): Html => {
  const h = html<Message>()
  const view = projectForgeEvaluationRegressionEvidence(
    buildForgeEvaluationRegressionEvidenceInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Evaluation and regression',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Suite, fixture, adapter, provider, model, policy, verdict, divergence, report, gate, threshold, and safety refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          evaluationRegressionStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Entries', String(view.counts.entries)),
        compactionMetric('Passed', String(view.counts.passed)),
        compactionMetric('Failed', String(view.counts.failed)),
        compactionMetric('Regressed', String(view.counts.regressed)),
        compactionMetric('Public reports', String(view.counts.publicReports)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Evaluation blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No evaluation/regression snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(evaluationRegressionEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe evaluation/regression ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const securityReviewEvidenceStatusTone = (
  status: ForgeSecurityReviewEvidenceStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'approved'
    ? 'positive'
    : status === 'blocked' || status === 'denied'
      ? 'negative'
      : status === 'needs_review'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const securityReviewEvidenceEntryTone = (
  status: ForgeSecurityReviewEvidenceItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'approved'
    ? 'positive'
    : status === 'blocked' || status === 'denied' || status === 'expired'
      ? 'negative'
      : status === 'needs_review'
        ? 'warning'
        : 'info'

const securityReviewEvidenceEntryPanel = (
  entry: ForgeSecurityReviewEvidenceItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.domainRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.domain.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status.replaceAll('_', ' '), securityReviewEvidenceEntryTone(entry.status)),
          badge(entry.risk, entry.risk === 'high' || entry.risk === 'critical' ? 'warning' : 'info'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Domain ref', [entry.domainRef]),
      refSection('Threat models', entry.threatModelRefs),
      refSection('Owner policies', entry.ownerPolicyRefs),
      refSection('Approval gates', entry.approvalGateRefs),
      refSection('Denial receipts', entry.denialReceiptRefs),
      refSection('Exceptions', entry.exceptionRefs),
      refSection('Exception expiry', entry.exceptionExpiryRefs),
      refSection('Redaction scans', entry.redactionScanRefs),
      refSection('Regression fixtures', entry.regressionFixtureRefs),
      refSection('Provider credential policy', entry.providerCredentialPolicyRefs),
      refSection('Release integrity', entry.releaseIntegrityRefs),
      refSection('Public projection scans', entry.publicProjectionScanRefs),
      refSection('Diagnostic bundles', entry.diagnosticBundleRefs),
      refSection('Security blockers', entry.blockerRefs),
    ]),
  ])
}

const securityReviewEvidencePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeSecurityReviewEvidence(
    buildForgeSecurityReviewEvidenceInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Security review',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Threat model, policy, approval gate, denial, exception, redaction, regression, credential, release, projection, and diagnostic refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          securityReviewEvidenceStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Domains', String(view.counts.domains)),
        compactionMetric('Approved', String(view.counts.approved)),
        compactionMetric('Denied', String(view.counts.denied)),
        compactionMetric('High risk', String(view.counts.highRisk)),
        compactionMetric('Exceptions', String(view.counts.exceptions)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Security blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No security review snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(securityReviewEvidenceEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe security review ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const dataRetentionDeletionStatusTone = (
  status: ForgeDataRetentionDeletionStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'pending_deletion' || status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const dataRetentionDeletionEntryTone = (
  status: ForgeDataRetentionDeletionItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'retained' || status === 'active'
    ? 'positive'
    : status === 'blocked' || status === 'expired'
      ? 'negative'
      : status === 'delete_requested' || status === 'legal_hold'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const dataRetentionDeletionEntryPanel = (
  entry: ForgeDataRetentionDeletionItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.dataClassRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.dataClass.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.status.replaceAll('_', ' '),
            dataRetentionDeletionEntryTone(entry.status),
          ),
          badge(entry.dataClass.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Data class ref', [entry.dataClassRef]),
      refSection('Retention policies', entry.retentionPolicyRefs),
      refSection('Deletion requests', entry.deletionRequestRefs),
      refSection('Deletion receipts', entry.deletionReceiptRefs),
      refSection('Tombstones', entry.tombstoneRefs),
      refSection('Export manifests', entry.exportManifestRefs),
      refSection('Retention sweeps', entry.retentionSweepRefs),
      refSection('Projection freshness', entry.projectionFreshnessRefs),
      refSection('Projection invalidation', entry.projectionInvalidationRefs),
      refSection('Legal holds', entry.legalHoldRefs),
      refSection('Retention caveats', entry.caveatRefs),
      refSection('Retention blockers', entry.blockerRefs),
    ]),
  ])
}

const dataRetentionDeletionPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeDataRetentionDeletionEvidence(
    buildForgeDataRetentionDeletionInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Data retention and deletion',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Data class, retention policy, deletion, tombstone, export, sweep, projection invalidation, and legal/payment caveat refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          dataRetentionDeletionStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Classes', String(view.counts.classes)),
        compactionMetric('Pending deletion', String(view.counts.deleteRequested)),
        compactionMetric('Deleted/tombstoned', String(view.counts.deletedOrTombstoned)),
        compactionMetric('Exportable', String(view.counts.exportable)),
        compactionMetric('Public projection', String(view.counts.publicProjectionClasses)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Retention blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No retention/deletion snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(dataRetentionDeletionEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe retention/deletion ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const onboardingCapabilityStatusTone = (
  status: ForgeOnboardingCapabilityStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'in_progress' || status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const onboardingCapabilityEntryTone = (
  status: ForgeOnboardingCapabilityItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready' || status === 'completed'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'in_progress' || status === 'planned'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const onboardingCapabilityEntryPanel = (
  entry: ForgeOnboardingCapabilityItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.stepRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.stepKind.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.status.replaceAll('_', ' '),
            onboardingCapabilityEntryTone(entry.status),
          ),
          badge(entry.mode.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Step ref', [entry.stepRef]),
      refSection('User/device refs', entry.userDeviceRefs),
      refSection('Workspace refs', entry.workspaceRefs),
      refSection('Repository profiles', entry.repositoryProfileRefs),
      refSection('Capability probes', entry.capabilityProbeRefs),
      refSection('Provider readiness', entry.providerReadinessRefs),
      refSection('Credential policies', entry.credentialPolicyRefs),
      refSection('Permission decisions', entry.permissionDecisionRefs),
      refSection('Data scopes', entry.dataScopeRefs),
      refSection('Instruction refs', entry.instructionRefs),
      refSection('Invariant refs', entry.invariantRefs),
      refSection('First-run smokes', entry.firstRunSmokeRefs),
      refSection('Completion receipts', entry.completionReceiptRefs),
      refSection('Skip receipts', entry.skipReceiptRefs),
      refSection('Integration refs', entry.integrationRefs),
      refSection('Onboarding blockers', entry.blockerRefs),
    ]),
  ])
}

const onboardingCapabilityPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeOnboardingCapabilityEvidence(
    buildForgeOnboardingCapabilityInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Onboarding capability',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Setup step, mode, capability probe, provider readiness, permission, data scope, instruction, smoke, completion, and skip refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          onboardingCapabilityStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Steps', String(view.counts.steps)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Skipped', String(view.counts.skipped)),
        compactionMetric('Provider modes', String(view.counts.providerConnected)),
        compactionMetric('Smokes', String(view.counts.smokes)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Onboarding blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No onboarding capability snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(onboardingCapabilityEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe onboarding ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const outputStylePersonaStatusTone = (
  status: ForgeOutputStylePersonaStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'planned' || status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const outputStylePersonaEntryTone = (
  status: ForgeOutputStylePersonaItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'conflicted' || status === 'planned'
        ? 'warning'
        : 'info'

const outputStylePersonaEntryPanel = (
  entry: ForgeOutputStylePersonaItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.stylePolicyRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.domainMode.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.status.replaceAll('_', ' '),
            outputStylePersonaEntryTone(entry.status),
          ),
          badge(entry.verbosity, 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Style policy ref', [entry.stylePolicyRef]),
      refSection('User preferences', entry.userPreferenceRefs),
      refSection('Product defaults', entry.productDefaultRefs),
      refSection('Project constraints', entry.projectConstraintRefs),
      refSection('Managed policies', entry.managedPolicyRefs),
      refSection('Formatting refs', entry.formattingRefs),
      refSection('Final answer expectations', entry.finalAnswerExpectationRefs),
      refSection('Persona constraints', entry.personaConstraintRefs),
      refSection('Audience refs', entry.audienceRefs),
      refSection('Accessibility refs', entry.accessibilityRefs),
      refSection('Safety policies', entry.safetyPolicyRefs),
      refSection('Citation requirements', entry.citationRequirementRefs),
      refSection('Evidence requirements', entry.evidenceRequirementRefs),
      refSection('Disallowed claims', entry.disallowedClaimRefs),
      refSection('Claim receipts', entry.claimReceiptRefs),
      refSection('Conflict resolutions', entry.conflictResolutionRefs),
      refSection('Overrides', entry.overrideRefs),
      refSection('Style audits', entry.styleAuditRefs),
      refSection('Tool authority boundary', entry.toolAuthorityBoundaryRefs),
      refSection('Style blockers', entry.blockerRefs),
    ]),
  ])
}

const outputStylePersonaPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeOutputStylePersonaEvidence(
    buildForgeOutputStylePersonaInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Output style and persona',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Style policy, verbosity, formatting, persona, accessibility, evidence, conflict, override, and audit refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          outputStylePersonaStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Policies', String(view.counts.policies)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Conflicts', String(view.counts.conflicts)),
        compactionMetric('Overrides', String(view.counts.overrides)),
        compactionMetric('Accessibility', String(view.counts.accessibilityPolicies)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Style blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No output style/persona snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(outputStylePersonaEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe output style ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const promptSuggestionsStatusTone = (
  status: ForgePromptSuggestionsStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'disabled' || status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const promptSuggestionEntryTone = (
  status: ForgePromptSuggestionItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked' || status === 'expired'
      ? 'negative'
      : status === 'disabled' || status === 'stale'
        ? 'warning'
        : 'info'

const promptSuggestionEntryPanel = (
  entry: ForgePromptSuggestionItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.suggestionRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.kind.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.status.replaceAll('_', ' '),
            promptSuggestionEntryTone(entry.status),
          ),
          badge(entry.privacy.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Suggestion ref', [entry.suggestionRef]),
      refSection('Display refs', entry.displayRefs),
      refSection('Insert text refs', entry.insertTextRefs),
      refSection('Action ref', entry.actionRef === null ? [] : [entry.actionRef]),
      refSection('Action separation', entry.actionSeparationRefs),
      refSection('Confidence refs', entry.confidenceRefs),
      refSection('Provenance refs', entry.provenanceRefs),
      refSection('Ranking refs', entry.rankingRefs),
      refSection('Semantic selectors', entry.semanticSelectorRefs),
      refSection('Scope refs', entry.scopeRefs),
      refSection('Privacy refs', entry.privacyRefs),
      refSection('Permissions', entry.permissionRefs),
      refSection('Destructive actions', entry.destructiveActionRefs),
      refSection('External actions', entry.externalActionRefs),
      refSection('Expiration refs', entry.expirationRefs),
      refSection('Disablement refs', entry.disablementRefs),
      refSection('Validation refs', entry.validationRefs),
      refSection('Audit refs', entry.auditRefs),
      refSection('Suggestion blockers', entry.blockerRefs),
    ]),
  ])
}

const promptSuggestionsPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgePromptSuggestionsEvidence(
    buildForgePromptSuggestionsInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Prompt suggestions',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Autocomplete suggestion, insert, action, ranking, semantic selector, scope, privacy, permission, expiration, disablement, and audit refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          promptSuggestionsStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Suggestions', String(view.counts.suggestions)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Actions', String(view.counts.actions)),
        compactionMetric('Scoped', String(view.counts.scoped)),
        compactionMetric('Semantic', String(view.counts.semantic)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Suggestion blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No prompt suggestions snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(promptSuggestionEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe prompt suggestion ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const tipsEducationStatusTone = (
  status: ForgeTipsEducationStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked' || status === 'unsupported'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const tipsEducationEntryTone = (
  status: ForgeTipsEducationItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked' || status === 'unsupported' || status === 'expired'
      ? 'negative'
      : status === 'dismissed'
        ? 'warning'
        : 'info'

const tipsEducationEntryPanel = (entry: ForgeTipsEducationItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.tipRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.topic.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(
            entry.status.replaceAll('_', ' '),
            tipsEducationEntryTone(entry.status),
          ),
          badge(entry.topic.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Tip ref', [entry.tipRef]),
      refSection('Topics/help', entry.helpTopicRefs),
      refSection('Triggers', entry.triggerRefs),
      refSection('Audience', entry.audienceRefs),
      refSection('Scope refs', entry.scopeRefs),
      refSection('Capability refs', entry.capabilityRefs),
      refSection('Live-state refs', entry.liveStateRefs),
      refSection('Required warnings', entry.requiredWarningRefs),
      refSection('Dismissal receipts', entry.dismissalReceiptRefs),
      refSection('Caveats', entry.caveatRefs),
      refSection('Docs refs', entry.docsRefs),
      refSection('Non-interactive mode', entry.nonInteractiveModeRefs),
      refSection('Non-interactive docs', entry.nonInteractiveDocsRefs),
      refSection('Expiration refs', entry.expirationRefs),
      refSection('Version refs', entry.versionRefs),
      refSection('Unsupported claims', entry.unsupportedClaimRefs),
      refSection('Education blockers', entry.blockerRefs),
    ]),
  ])
}

const tipsEducationPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeTipsEducationEvidence(buildForgeTipsEducationInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Tips and education',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Tip, topic, trigger, capability, live-state, warning, dismissal, caveat, docs, and non-interactive refs.',
          ]),
        ]),
        badge(view.status.replaceAll('_', ' '), tipsEducationStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Tips', String(view.counts.tips)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Dismissed', String(view.counts.dismissed)),
        compactionMetric('Warnings', String(view.counts.requiredWarnings)),
        compactionMetric('Caveats', String(view.counts.caveats)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Education blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No tips/education snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(tipsEducationEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe tips/education ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const themeVisualStatusTone = (
  status: ForgeThemeVisualStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const themeVisualEntryTone = (
  status: ForgeThemeVisualItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'info'

const themeVisualEntryPanel = (entry: ForgeThemeVisualItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.themeRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.surface} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status, themeVisualEntryTone(entry.status)),
          badge(entry.surface, 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Theme ref', [entry.themeRef]),
      refSection('Token refs', entry.tokenRefs),
      refSection('Typography refs', entry.typographyRefs),
      refSection('Density refs', entry.densityRefs),
      refSection('Status visuals', entry.statusVisualRefs),
      refSection('Status labels', entry.statusLabelRefs),
      refSection('Status icons', entry.statusIconRefs),
      refSection('Runtime receipts', entry.runtimeReceiptRefs),
      refSection('Contrast checks', entry.contrastCheckRefs),
      refSection('Monochrome refs', entry.monochromeRefs),
      refSection('High contrast', entry.highContrastRefs),
      refSection('Reduced motion', entry.reducedMotionRefs),
      refSection('Focus rings', entry.focusRingRefs),
      refSection('Diff colors', entry.diffColorRefs),
      refSection('Progress colors', entry.progressColorRefs),
      refSection('Attention colors', entry.attentionColorRefs),
      refSection('Managed policies', entry.managedPolicyRefs),
      refSection('Cross-surface refs', entry.crossSurfaceRefs),
      refSection('Visual snapshots', entry.snapshotRefs),
      refSection('Warning preservation', entry.warningPreservationRefs),
      refSection('Theme blockers', entry.blockerRefs),
    ]),
  ])
}

const themeVisualPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeThemeVisualEvidence(buildForgeThemeVisualInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Theme and visual design',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Theme, token, status visual, contrast, motion, managed policy, snapshot, and runtime receipt refs.',
          ]),
        ]),
        badge(view.status, themeVisualStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Surfaces', String(view.counts.surfaces)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('High contrast', String(view.counts.highContrast)),
        compactionMetric('Reduced motion', String(view.counts.reducedMotion)),
        compactionMetric('Managed', String(view.counts.managed)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Theme blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No theme/visual snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(themeVisualEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe theme/visual ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const accessibilityNonInteractiveStatusTone = (
  status: ForgeAccessibilityNonInteractiveStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const accessibilityNonInteractiveEntryTone = (
  status: ForgeAccessibilityNonInteractiveItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'info'

const accessibilityNonInteractiveEntryPanel = (
  entry: ForgeAccessibilityNonInteractiveItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.modeRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.mode.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status, accessibilityNonInteractiveEntryTone(entry.status)),
          badge(entry.mode.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Mode ref', [entry.modeRef]),
      refSection('Terminal capabilities', entry.terminalCapabilityRefs),
      refSection('Structured output', entry.structuredOutputRefs),
      refSection('Schema refs', entry.schemaRefs),
      refSection('Exit-code refs', entry.exitCodeRefs),
      refSection('Status labels', entry.statusLabelRefs),
      refSection('No-color refs', entry.noColorRefs),
      refSection('High contrast', entry.highContrastRefs),
      refSection('Reduced motion', entry.reducedMotionRefs),
      refSection('Screen-reader status', entry.screenReaderStatusRefs),
      refSection('Keyboard navigation', entry.keyboardNavigationRefs),
      refSection('Prompt availability', entry.promptAvailabilityRefs),
      refSection('Approval resolvers', entry.approvalResolverRefs),
      refSection('Typed prompt blockers', entry.typedPromptBlockerRefs),
      refSection('Notifications', entry.notificationAvailabilityRefs),
      refSection('Remote bridge availability', entry.remoteBridgeAvailabilityRefs),
      refSection('CI policy refs', entry.ciPolicyRefs),
      refSection('Spend caveats', entry.spendCaveatRefs),
      refSection('Push caveats', entry.pushCaveatRefs),
      refSection('Deploy caveats', entry.deployCaveatRefs),
      refSection('Provider mutation caveats', entry.providerMutationCaveatRefs),
      refSection('Mode blockers', entry.blockerRefs),
    ]),
  ])
}

const accessibilityNonInteractivePanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeAccessibilityNonInteractiveEvidence(
    buildForgeAccessibilityNonInteractiveInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Accessibility and non-interactive mode',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Interaction mode, structured output, no-color, screen-reader, prompt blocker, CI policy, and exit-code refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          accessibilityNonInteractiveStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Modes', String(view.counts.total)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Non-interactive', String(view.counts.nonInteractive)),
        compactionMetric('CI', String(view.counts.ci)),
        compactionMetric('Screen reader', String(view.counts.screenReader)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Accessibility blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No accessibility/non-interactive snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(accessibilityNonInteractiveEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe accessibility/non-interactive ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const localizationBoundaryStatusTone = (
  status: ForgeLocalizationBoundaryStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const localizationBoundaryEntryTone = (
  status: ForgeLocalizationBoundaryItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'info'

const localizationBoundaryEntryPanel = (
  entry: ForgeLocalizationBoundaryItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.localizationRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.scope.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status, localizationBoundaryEntryTone(entry.status)),
          badge(entry.scope.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Localization ref', [entry.localizationRef]),
      refSection('Locale refs', entry.localeRefs),
      refSection('Locale preferences', entry.localePreferenceRefs),
      refSection('Message catalogs', entry.catalogRefs),
      refSection('Catalog validation', entry.catalogValidationRefs),
      refSection('Formatters', entry.formatterRefs),
      refSection('Visible fallbacks', entry.fallbackRefs),
      refSection('Missing translations', entry.missingTranslationRefs),
      refSection('Stable-id boundaries', entry.stableIdBoundaryRefs),
      refSection('JSON/schema stability', entry.jsonSchemaStabilityRefs),
      refSection('Public receipt stability', entry.publicReceiptStabilityRefs),
      refSection('Command id stability', entry.commandIdStabilityRefs),
      refSection('Tool id stability', entry.toolIdStabilityRefs),
      refSection('Permission id stability', entry.permissionIdStabilityRefs),
      refSection('Permission actions', entry.permissionActionRefs),
      refSection('Permission policies', entry.permissionPolicyRefs),
      refSection('Payment language review', entry.paymentLanguageReviewRefs),
      refSection('Localization blockers', entry.blockerRefs),
    ]),
  ])
}

const localizationBoundaryPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeLocalizationBoundaryEvidence(
    buildForgeLocalizationBoundaryInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Localization boundary',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Locale, catalog, formatter, fallback, stable-id, permission, payment, receipt, and schema refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          localizationBoundaryStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Entries', String(view.counts.total)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Catalogs', String(view.counts.catalogs)),
        compactionMetric('Fallbacks', String(view.counts.fallbacks)),
        compactionMetric('Stable IDs', String(view.counts.stableBoundaries)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Localization blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No localization boundary snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(localizationBoundaryEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe localization ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const enterpriseManagedPolicyStatusTone = (
  status: ForgeEnterpriseManagedPolicyStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const enterpriseManagedPolicyEntryTone = (
  status: ForgeEnterpriseManagedPolicyItem['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'info'

const enterpriseManagedPolicyEntryPanel = (
  entry: ForgeEnterpriseManagedPolicyItem,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.policyRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.decision} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.status, enterpriseManagedPolicyEntryTone(entry.status)),
          badge(entry.decision, 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Policy ref', [entry.policyRef]),
      refSection('Effective policy', entry.effectivePolicyRefs),
      refSection('Scope refs', entry.scopeRefs),
      refSection('Version refs', entry.versionRefs),
      refSection('Owner/admin refs', entry.ownerAdminRefs),
      refSection('Rule kind refs', entry.ruleKindRefs),
      refSection('Enforcement mode', entry.enforcementModeRefs),
      refSection('Runtime capability boundary', entry.runtimeCapabilityBoundaryRefs),
      refSection('Public summaries', entry.publicSummaryRefs),
      refSection('Audit refs', entry.auditRefs),
      refSection('Change refs', entry.changeRefs),
      refSection('Allow refs', entry.allowRefs),
      refSection('Ask refs', entry.askRefs),
      refSection('Restrict refs', entry.restrictRefs),
      refSection('Denial refs', entry.denialRefs),
      refSection('User-safe reasons', entry.userSafeReasonRefs),
      refSection('Conflicts', entry.conflictRefs),
      refSection('Conflict resolution', entry.conflictResolutionRefs),
      refSection('Conflict priority', entry.conflictPriorityRefs),
      refSection('Emergency override receipts', entry.emergencyOverrideReceiptRefs),
      refSection('Expiration refs', entry.expirationRefs),
      refSection('Organization policy', entry.organizationPolicyRefs),
      refSection('Team policy', entry.teamPolicyRefs),
      refSection('Repository policy', entry.repositoryPolicyRefs),
      refSection('User policy', entry.userPolicyRefs),
      refSection('Device policy', entry.devicePolicyRefs),
      refSection('Project policy', entry.projectPolicyRefs),
      refSection('Session policy', entry.sessionPolicyRefs),
      refSection('Provider policy', entry.providerPolicyRefs),
      refSection('Budget policy', entry.budgetPolicyRefs),
      refSection('Retention policy', entry.retentionPolicyRefs),
      refSection('Telemetry policy', entry.telemetryPolicyRefs),
      refSection('Update policy', entry.updatePolicyRefs),
      refSection('Plugin policy', entry.pluginPolicyRefs),
      refSection('MCP policy', entry.mcpPolicyRefs),
      refSection('Hook policy', entry.hookPolicyRefs),
      refSection('Remote bridge policy', entry.remoteBridgePolicyRefs),
      refSection('Policy caveats', entry.caveatRefs),
      refSection('Policy blockers', entry.blockerRefs),
    ]),
  ])
}

const enterpriseManagedPolicyPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeEnterpriseManagedPolicyEvidence(
    buildForgeEnterpriseManagedPolicyInput(work),
  )

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Enterprise managed policy',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Effective policy, enforcement, denial, conflict, override, audit, capability-boundary, and domain policy refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          enterpriseManagedPolicyStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Policies', String(view.counts.total)),
        compactionMetric('Ready', String(view.counts.ready)),
        compactionMetric('Denied', String(view.counts.denied)),
        compactionMetric('Ask/restrict', String(view.counts.askRestrict)),
        compactionMetric('Overrides', String(view.counts.emergencyOverrides)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Managed policy blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No enterprise managed policy snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(enterpriseManagedPolicyEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe managed policy ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const terminalUiShellStatusTone = (
  status: ForgeTerminalUiShellStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const terminalSurfaceStateTone = (
  state: ForgeTerminalSurfaceItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'available'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'degraded'
        ? 'warning'
        : state === 'unknown'
          ? 'info'
          : 'accent'

const terminalSurfacePanel = (surface: ForgeTerminalSurfaceItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            surface.surfaceRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${surface.mode.replaceAll('_', ' ')} - ${surface.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(surface.state, terminalSurfaceStateTone(surface.state)),
          badge(surface.mode.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Surface ref', [surface.surfaceRef]),
      refSection('Shell refs', surface.shellRefs),
      refSection('Pane refs', surface.paneRefs),
      refSection('Stream refs', surface.streamRefs),
      refSection('Transcript summaries', surface.transcriptSummaryRefs),
      refSection('Command descriptors', surface.commandDescriptorRefs),
      refSection('Input descriptors', surface.inputDescriptorRefs),
      refSection('Non-interactive refs', surface.nonInteractiveRefs),
      refSection('Accessibility refs', surface.accessibilityRefs),
      refSection('Parity refs', surface.parityRefs),
      refSection('Policy refs', surface.policyRefs),
      refSection('Surface blockers', surface.blockerRefs),
    ]),
  ])
}

const terminalUiShellPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeTerminalUiShell(buildForgeTerminalUiShellInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Terminal UI shell',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Terminal, headless, pane, stream, input, and parity evidence refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          terminalUiShellStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Surfaces', String(view.counts.total)),
        compactionMetric('Available', String(view.counts.available)),
        compactionMetric('Interactive', String(view.counts.interactive)),
        compactionMetric('Degraded', String(view.counts.degraded)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Terminal blockers', view.blockerRefs),
      ]),
      view.surfaces.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No terminal surface snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.surfaces.map(terminalSurfacePanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe terminal ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const inputKeybindingStatusTone = (
  status: ForgeInputKeybindingStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const inputKeybindingStateTone = (
  state: ForgeInputKeybindingItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'available'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'degraded'
        ? 'warning'
        : state === 'unknown'
          ? 'info'
          : 'accent'

const inputKeybindingEntryPanel = (entry: ForgeInputKeybindingItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            entry.inputModeRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${entry.mode.replaceAll('_', ' ')} - ${entry.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(entry.state, inputKeybindingStateTone(entry.state)),
          badge(entry.mode.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Input mode ref', [entry.inputModeRef]),
      refSection('Binding maps', entry.bindingMapRefs),
      refSection('Keymaps', entry.keymapRefs),
      refSection('Command descriptors', entry.commandDescriptorRefs),
      refSection('Non-interactive fallbacks', entry.nonInteractiveFallbackRefs),
      refSection('Platform refs', entry.platformRefs),
      refSection('Accessibility refs', entry.accessibilityRefs),
      refSection('Conflict refs', entry.conflictRefs),
      refSection('Policy refs', entry.policyRefs),
      refSection('Input blockers', entry.blockerRefs),
    ]),
  ])
}

const inputKeybindingPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeInputKeybinding(buildForgeInputKeybindingInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Input and keybinding',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Input modes, keymaps, command descriptors, fallbacks, and policy refs.',
          ]),
        ]),
        badge(
          view.status.replaceAll('_', ' '),
          inputKeybindingStatusTone(view.status),
        ),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Modes', String(view.counts.total)),
        compactionMetric('Available', String(view.counts.available)),
        compactionMetric('Interactive', String(view.counts.interactive)),
        compactionMetric('Conflicts', String(view.counts.conflicts)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Input blockers', view.blockerRefs),
      ]),
      view.entries.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No input/keybinding snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.entries.map(inputKeybindingEntryPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe input/keybinding ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const commandSystemStatusTone = (
  status: ForgeCommandSystemStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : status === 'unknown'
          ? 'info'
          : 'accent'

const commandStateTone = (
  state: ForgeCommandItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'available'
    ? 'positive'
    : state === 'blocked'
      ? 'negative'
      : state === 'conflicted' || state === 'unavailable'
        ? 'warning'
        : state === 'unknown'
          ? 'info'
          : 'accent'

const commandPanel = (command: ForgeCommandItem): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.div([Ui.className<Message>('text-sm font-medium text-white/75')], [
            command.commandRef,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `${command.kind.replaceAll('_', ' ')} - ${command.freshness}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
          badge(command.state, commandStateTone(command.state)),
          badge(command.kind.replaceAll('_', ' '), 'accent'),
        ]),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('Command ref', [command.commandRef]),
      refSection('Command descriptors', command.commandDescriptorRefs),
      refSection('Parser refs', command.parserRefs),
      refSection('Planner refs', command.plannerRefs),
      refSection('Selector refs', command.selectorRefs),
      refSection('Input modes', command.inputModeRefs),
      refSection('Capability refs', command.capabilityRefs),
      refSection('Fallback refs', command.fallbackRefs),
      refSection('Conflict refs', command.conflictRefs),
      refSection('Policy refs', command.policyRefs),
      refSection('Command blockers', command.blockerRefs),
    ]),
  ])
}

const commandSystemPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const view = projectForgeCommandSystem(buildForgeCommandSystemInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div(
      [Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')],
      [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Command system',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Command descriptors, parser, planner, selector, fallback, and policy refs.',
          ]),
        ]),
        badge(view.status.replaceAll('_', ' '), commandSystemStatusTone(view.status)),
      ],
    ),
    h.div([Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4')], [
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-5')], [
        compactionMetric('Commands', String(view.counts.total)),
        compactionMetric('Available', String(view.counts.available)),
        compactionMetric('Unavailable', String(view.counts.unavailable)),
        compactionMetric('Conflicted', String(view.counts.conflicted)),
        compactionMetric('Blocked', String(view.counts.blocked)),
      ]),
      h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
        refSection('Snapshot ref', view.snapshotRef === null ? [] : [view.snapshotRef]),
        refSection('Version ref', view.versionRef === null ? [] : [view.versionRef]),
        refSection('Command blockers', view.blockerRefs),
      ]),
      view.commands.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No command catalog snapshot available.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...view.commands.map(commandPanel),
          ]),
      view.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${view.omittedUnsafeRefCount} unsafe command ref(s) were omitted before rendering.`,
          ]),
    ]),
  ])
}

const contextSnapshotTone = (
  status: ForgeContextSnapshotStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'accent'

const contextFreshnessTone = (
  freshness: ForgeContextFreshness,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  freshness === 'fresh'
    ? 'positive'
    : freshness === 'stale'
      ? 'warning'
      : 'accent'

const contextDirtyTone = (
  dirtyState: ForgeContextDirtyState,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  dirtyState === 'clean'
    ? 'positive'
    : dirtyState === 'dirty'
      ? 'warning'
      : 'accent'

const contextMetric = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-1 border border-[#222] p-3')], [
    h.div([Ui.className<Message>('text-[0.6875rem] uppercase text-white/35')], [
      label,
    ]),
    h.div(
      [Ui.className<Message>('min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/75')],
      [value],
        ),
  ])
}

const contextRefGroup = (
  group:
    | (Readonly<{
        blockerRefs?: ReadonlyArray<string>
        refs?: ReadonlyArray<string>
      }>)
    | undefined,
): ForgeContextRefGroupInput | undefined =>
  group === undefined
    ? undefined
    : {
        ...(group.blockerRefs === undefined ? {} : { blockerRefs: group.blockerRefs }),
        ...(group.refs === undefined ? {} : { refs: group.refs }),
      }

const contextSnapshotInput = (
  work: AutopilotWorkProjection,
): ForgeContextSnapshotInput => {
  const source = work.contextSnapshot
  const devDoctor = contextRefGroup(source?.devDoctor)
  const adapters =
    source?.adapters === undefined
      ? undefined
      : {
          ...(source.adapters.blockerRefs === undefined
            ? {}
            : { blockerRefs: source.adapters.blockerRefs }),
          ...(source.adapters.capabilityRefs === undefined
            ? {}
            : { capabilityRefs: source.adapters.capabilityRefs }),
          ...(source.adapters.refs === undefined ? {} : { refs: source.adapters.refs }),
        }
  const currentJob =
    source?.currentJob === undefined
      ? undefined
      : {
          ...(source.currentJob.blockerRefs === undefined
            ? {}
            : { blockerRefs: source.currentJob.blockerRefs }),
          ...(source.currentJob.capabilityRefs === undefined
            ? {}
            : { capabilityRefs: source.currentJob.capabilityRefs }),
          ...(source.currentJob.jobRefs === undefined
            ? {}
            : { jobRefs: source.currentJob.jobRefs }),
          ...(source.currentJob.verificationRefs === undefined
            ? {}
            : { verificationRefs: source.currentJob.verificationRefs }),
        }
  const instructions =
    source?.instructions === undefined
      ? undefined
      : {
          ...(source.instructions.blockerRefs === undefined
            ? {}
            : { blockerRefs: source.instructions.blockerRefs }),
          ...(source.instructions.configRefs === undefined
            ? {}
            : { configRefs: source.instructions.configRefs }),
          ...(source.instructions.refs === undefined
            ? {}
            : { refs: source.instructions.refs }),
        }
  const repo =
    source?.repo === undefined
      ? undefined
      : {
          ...(source.repo.blockerRefs === undefined
            ? {}
            : { blockerRefs: source.repo.blockerRefs }),
          ...(source.repo.changedCount === undefined
            ? {}
            : { changedCount: source.repo.changedCount }),
          ...(source.repo.dirtyState === undefined
            ? {}
            : { dirtyState: source.repo.dirtyState }),
          ...(source.repo.dirtyStateRefs === undefined
            ? {}
            : { dirtyStateRefs: source.repo.dirtyStateRefs }),
          ...(source.repo.identityRefs === undefined
            ? {}
            : { identityRefs: source.repo.identityRefs }),
        }

  return {
    generatedAt: work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(adapters === undefined ? {} : { adapters }),
    ...(currentJob === undefined ? {} : { currentJob }),
    ...(devDoctor === undefined ? {} : { devDoctor }),
    ...(source?.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source?.freshness === undefined ? {} : { freshness: source.freshness }),
    ...(source?.observedAt === undefined ? {} : { observedAt: source.observedAt }),
    ...(instructions === undefined ? {} : { instructions }),
    ...(repo === undefined ? {} : { repo }),
  }
}

const repositoryMemoryProfileInput = (
  work: AutopilotWorkProjection,
): ForgeRepositoryMemoryProfileInput | undefined => {
  const source = work.contextSnapshot
  const profile = source?.repositoryMemoryProfile

  return profile === undefined
    ? undefined
    : {
        generatedAt: profile.generatedAt,
        profileRef: profile.profileRef,
        workOrderRef: work.workOrderRef,
        ...(profile.blockerRefs === undefined
          ? {}
          : { blockerRefs: profile.blockerRefs }),
        ...(profile.blockedClaimRefs === undefined
          ? {}
          : { blockedClaimRefs: profile.blockedClaimRefs }),
        ...(profile.changedProfileKinds === undefined
          ? {}
          : { changedProfileKinds: profile.changedProfileKinds }),
        ...(profile.commandProfileRefs === undefined
          ? {}
          : { commandProfileRefs: profile.commandProfileRefs }),
        ...(profile.corpusManifestRef === undefined
          ? {}
          : { corpusManifestRef: profile.corpusManifestRef }),
        ...(profile.currentInstructionRefs !== undefined
          ? { currentInstructionRefs: profile.currentInstructionRefs }
          : source?.instructions?.refs === undefined
            ? {}
            : { currentInstructionRefs: source.instructions.refs }),
        ...(profile.datasetRefs === undefined ? {} : { datasetRefs: profile.datasetRefs }),
        ...(profile.devDoctorRefs !== undefined
          ? { devDoctorRefs: profile.devDoctorRefs }
          : source?.devDoctor?.refs === undefined
            ? {}
            : { devDoctorRefs: source.devDoctor.refs }),
        ...(profile.dirtyState !== undefined
          ? { dirtyState: profile.dirtyState }
          : source?.repo?.dirtyState === undefined
            ? {}
            : { dirtyState: source.repo.dirtyState }),
        ...(profile.freshness === undefined ? {} : { freshness: profile.freshness }),
        ...(profile.holdoutEvaluationRef === undefined
          ? {}
          : { holdoutEvaluationRef: profile.holdoutEvaluationRef }),
        ...(profile.instructionRefs === undefined
          ? {}
          : { instructionRefs: profile.instructionRefs }),
        ...(profile.invariantRefs === undefined
          ? {}
          : { invariantRefs: profile.invariantRefs }),
        ...(profile.privateValidationTrendRef === undefined
          ? {}
          : { privateValidationTrendRef: profile.privateValidationTrendRef }),
        ...(profile.publicRetainedScoreRef === undefined
          ? {}
          : { publicRetainedScoreRef: profile.publicRetainedScoreRef }),
        ...(profile.refreshedAt === undefined
          ? {}
          : { refreshedAt: profile.refreshedAt }),
        ...(profile.refreshEvents === undefined
          ? {}
          : { refreshEvents: profile.refreshEvents }),
        ...(profile.refreshReceiptRefs === undefined
          ? {}
          : { refreshReceiptRefs: profile.refreshReceiptRefs }),
        ...(profile.repoIdentityRefs !== undefined
          ? { repoIdentityRefs: profile.repoIdentityRefs }
          : source?.repo?.identityRefs === undefined
            ? {}
            : { repoIdentityRefs: source.repo.identityRefs }),
        ...(profile.studyPacketFreshness === undefined
          ? {}
          : { studyPacketFreshness: profile.studyPacketFreshness }),
        ...(profile.studyPacketRef === undefined
          ? {}
          : { studyPacketRef: profile.studyPacketRef }),
        ...(profile.testProfileRefs === undefined
          ? {}
          : { testProfileRefs: profile.testProfileRefs }),
      }
}

const contextEvidenceCount = (
  context: ReturnType<typeof projectForgeContextSnapshot>,
): number =>
  context.repo.identityRefs.length +
  context.repo.dirtyStateRefs.length +
  context.instructions.instructionRefs.length +
  context.instructions.configRefs.length +
  context.adapters.readinessRefs.length +
  context.adapters.capabilityRefs.length +
  context.devDoctor.doctorRefs.length +
  context.currentJob.jobRefs.length +
  context.currentJob.verificationRefs.length +
  context.currentJob.capabilityRefs.length

const repositoryMemoryStatusTone = (
  status: ForgeRepositoryMemoryProfileStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'accent'

const repositoryMemoryProfilePanel = (
  profile: ForgeRepositoryMemoryProfile,
): Html => {
  const h = html<Message>()
  const refreshedLabel =
    profile.refreshedAt === null
      ? 'No refresh time'
      : `Refreshed ${formatIsoDateTime(profile.refreshedAt)}`
  const changedKinds =
    profile.changedProfileKinds.length === 0
      ? 'none'
      : profile.changedProfileKinds.join(', ')
  const studyLaneText = profile.laneLabel.replace('_', ' ')
  const studyLaneLabel = `${studyLaneText.charAt(0).toUpperCase()}${studyLaneText.slice(1)}`
  const productPromiseLabel = profile.productPromiseState.replace('_', ' ')

  return h.div([Ui.className<Message>('grid gap-4 border border-[#222] p-4')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/75')], [
          'Repository memory profile',
        ]),
        h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
          `${refreshedLabel} - changed profile kinds: ${changedKinds}`,
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(profile.status, repositoryMemoryStatusTone(profile.status)),
        badge(profile.freshness, contextFreshnessTone(profile.freshness)),
        ...profile.changedProfileKinds.map(kind => badge(kind, 'accent')),
      ]),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-3')], [
      contextMetric('Profile status', profile.status),
      contextMetric('Profile freshness', profile.freshness),
      contextMetric('Changed kinds', changedKinds),
      contextMetric('Study lane', studyLaneLabel),
      contextMetric('Study freshness', profile.studyPacketFreshness),
      contextMetric('Authority', profile.authorityBoundary),
      contextMetric('Mutation authority', String(profile.mutationAuthority)),
      contextMetric('Product promise', productPromiseLabel),
    ]),
    h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
      h.div([Ui.className<Message>('flex flex-wrap items-center justify-between gap-2')], [
        h.h4([Ui.className<Message>('m-0 text-xs font-medium text-white/70')], [
          'Study packet memory',
        ]),
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(studyLaneLabel, 'accent'),
          badge('evidence only', 'info'),
          badge('no mutation authority', 'warning'),
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection(
          'Study packet ref',
          profile.studyPacketRef === null ? [] : [profile.studyPacketRef],
        ),
        refSection(
          'Corpus manifest ref',
          profile.corpusManifestRef === null ? [] : [profile.corpusManifestRef],
        ),
        refSection('Dataset refs', profile.datasetRefs),
        refSection(
          'Public retained score ref',
          profile.publicRetainedScoreRef === null
            ? []
            : [profile.publicRetainedScoreRef],
        ),
        refSection(
          'Private validation trend ref',
          profile.privateValidationTrendRef === null
            ? []
            : [profile.privateValidationTrendRef],
        ),
        refSection(
          'Holdout evaluation ref',
          profile.holdoutEvaluationRef === null
            ? []
            : [profile.holdoutEvaluationRef],
        ),
        refSection('Blocked claim refs', profile.blockedClaimRefs),
      ]),
    ]),
    h.div(
      [Ui.className<Message>('grid gap-4 md:grid-cols-2')],
      [
        refSection('Profile ref', [profile.profileRef]),
        refSection('Repo profile identity', profile.repoIdentityRefs),
        refSection('Instruction profile refs', profile.instructionRefs),
        refSection('Current instruction refs', profile.currentInstructionRefs),
        refSection('Command profile refs', profile.commandProfileRefs),
        refSection('Test profile refs', profile.testProfileRefs),
        refSection('Invariant profile refs', profile.invariantRefs),
        refSection('Dev doctor profile refs', profile.devDoctorRefs),
        refSection('Refresh receipt refs', profile.refreshReceiptRefs),
        refSection('Repository memory blockers', profile.blockerRefs),
      ],
    ),
    profile.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${profile.omittedUnsafeRefCount} unsafe repository-memory ref(s) were omitted before rendering.`,
        ]),
  ])
}

const contextSnapshotPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const context = projectForgeContextSnapshot(contextSnapshotInput(work))
  const repositoryMemoryInput = repositoryMemoryProfileInput(work)
  const repositoryMemory =
    repositoryMemoryInput === undefined
      ? undefined
      : projectForgeRepositoryMemoryProfile(repositoryMemoryInput)
  const observedLabel =
    context.observedAt === null
      ? 'No observation time'
      : `Observed ${formatIsoDateTime(context.observedAt)}`

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Context snapshot',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Refs-only context readiness for repo identity, instructions, adapters, dev doctor, and current job state.',
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(context.status, contextSnapshotTone(context.status)),
        badge(context.freshness, contextFreshnessTone(context.freshness)),
      ]),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-3')], [
      contextMetric('Observed', observedLabel),
      contextMetric('Dirty state', context.repo.dirtyState),
      contextMetric(
        'Changed files',
        context.repo.changedCount === null ? 'unknown' : String(context.repo.changedCount),
      ),
    ]),
    h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
      badge(context.repo.dirtyState, contextDirtyTone(context.repo.dirtyState)),
      badge(`${contextEvidenceCount(context)} context ref(s)`, 'accent'),
    ]),
    contextEvidenceCount(context) === 0
      ? h.div([Ui.className<Message>('border border-[#222] p-4')], [
          h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
            'No context evidence available yet.',
          ]),
        ])
      : h.span([Ui.className<Message>('hidden')], []),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Repo identity', context.repo.identityRefs),
        refSection('Dirty state refs', context.repo.dirtyStateRefs),
        refSection('Instruction refs', context.instructions.instructionRefs),
        refSection('Config refs', context.instructions.configRefs),
        refSection('Adapter readiness', context.adapters.readinessRefs),
        refSection('Adapter capabilities', context.adapters.capabilityRefs),
        refSection('Dev doctor', context.devDoctor.doctorRefs),
        refSection('Current job', context.currentJob.jobRefs),
        refSection('Verification refs', context.currentJob.verificationRefs),
        refSection('Current capabilities', context.currentJob.capabilityRefs),
        refSection('Context blockers', context.blockerRefs),
      ],
    ),
    repositoryMemory === undefined
      ? h.span([Ui.className<Message>('hidden')], [])
      : repositoryMemoryProfilePanel(repositoryMemory),
    context.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${context.omittedUnsafeRefCount} unsafe context ref(s) were omitted before rendering.`,
        ]),
  ])
}

const sessionNavigationTone = (
  status: ForgeSessionNavigationStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'complete'
    ? 'positive'
    : status === 'attention'
      ? 'warning'
      : status === 'active'
        ? 'info'
        : 'accent'

const sessionItemTone = (
  state: ForgeSessionNavigationItem['state'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  state === 'completed'
    ? 'positive'
    : state === 'failed' || state === 'cancelled'
      ? 'warning'
      : state === 'running'
        ? 'info'
        : 'accent'

const sessionActionLabel = (action: ForgeSessionNavigationAction): string =>
  action.charAt(0).toUpperCase() + action.slice(1)

const sessionActionBlockerRefs = (
  item: ForgeSessionNavigationItem,
): ReadonlyArray<string> =>
  Array.from(
    new Set(
      Object.values(item.actions).flatMap(actionState => actionState.blockerRefs),
    ),
  )

const SESSION_CONTROL_ACTION_PATH = '/api/autopilot/session-control'

const hiddenInput = (name: string, value: string): Html => {
  const h = html<Message>()

  return h.input([h.Type('hidden'), h.Name(name), h.Value(value)])
}

const sessionControlAction = (
  workOrderRef: string,
  item: ForgeSessionNavigationItem,
  action: ForgeSessionNavigationAction,
): Html => {
  const h = html<Message>()
  const actionState = item.actions[action]

  if (actionState.availability === 'available') {
    return h.form(
      [
        Ui.className<Message>('contents'),
        h.Method('post'),
        h.Action(SESSION_CONTROL_ACTION_PATH),
        h.DataAttribute('forge-session-action-form', action),
        h.DataAttribute('forge-session-action-availability', actionState.availability),
      ],
      [
        hiddenInput('workOrderRef', workOrderRef),
        hiddenInput('sessionRef', item.sessionRef),
        hiddenInput('action', action),
        hiddenInput('requestRef', actionState.requestRef),
        ...actionState.authorityRefs.map(ref =>
          hiddenInput('controlAuthorityRef', ref),
        ),
        ...actionState.policyRefs.map(ref => hiddenInput('controlPolicyRef', ref)),
        Ui.button<Message>({
          attrs: [
            h.Type('submit'),
            h.DataAttribute('forge-session-action', action),
            h.DataAttribute(
              'forge-session-action-availability',
              actionState.availability,
            ),
            h.DataAttribute('forge-session-control-request-ref', actionState.requestRef),
          ],
          label: sessionActionLabel(action),
          size: 'sm',
          variant: 'secondary',
        }),
      ],
    )
  }

  return Ui.button<Message>({
    attrs: [
      h.Type('button'),
      h.Disabled(true),
      h.DataAttribute('forge-session-action', action),
      h.DataAttribute('forge-session-action-availability', actionState.availability),
      h.DataAttribute(
        'forge-session-action-blocker-refs',
        actionState.blockerRefs.join(' '),
      ),
    ],
    label: sessionActionLabel(action),
    size: 'sm',
    variant: 'secondary',
  })
}

const sessionItemPanel = (
  workOrderRef: string,
  item: ForgeSessionNavigationItem,
): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-4 border border-[#222] p-4'),
      h.DataAttribute('forge-session-navigation-item', item.sessionRef),
      h.DataAttribute('forge-session-navigation-source', item.source),
      h.DataAttribute('forge-session-navigation-state', item.state),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('min-w-0 grid gap-1')], [
          h.h3(
            [
              Ui.className<Message>(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/80',
              ),
            ],
            [item.title],
          ),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            item.observedAt === null
              ? `Source ${item.source} - no observation time`
              : `Source ${item.source} - observed ${formatIsoDateTime(item.observedAt)}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(item.state, sessionItemTone(item.state)),
          badge(item.source, 'accent'),
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Session ref', [item.sessionRef]),
        refSection('Artifacts', item.artifactRefs),
        refSection('Events', item.eventRefs),
        refSection('Checkpoints', item.checkpointRefs),
        refSection('Bridge refs', item.bridgeRefs),
        refSection('Control authority', item.controlAuthorityRefs),
        refSection('Control policy', item.controlPolicyRefs),
        refSection('Control blockers', sessionActionBlockerRefs(item)),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap gap-2')], [
        sessionControlAction(workOrderRef, item, 'resume'),
        sessionControlAction(workOrderRef, item, 'fork'),
        sessionControlAction(workOrderRef, item, 'rewind'),
        sessionControlAction(workOrderRef, item, 'cancel'),
      ]),
    ],
  )
}

const sessionControlReceiptTone = (
  outcome: ForgeSessionControlReceiptItem['outcome'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  outcome === 'applied'
    ? 'positive'
    : outcome === 'blocked'
      ? 'negative'
      : outcome === 'stale'
        ? 'warning'
        : 'info'

const sessionControlReceiptPanel = (
  receipt: ForgeSessionControlReceiptItem,
): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-3 border border-[#222] p-4'),
      h.DataAttribute('forge-session-control-receipt', receipt.receiptRef),
      h.DataAttribute('forge-session-control-outcome', receipt.outcome),
      h.DataAttribute('forge-session-control-action', receipt.action),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/80')], [
            `${sessionActionLabel(receipt.action)} ${receipt.outcome}`,
          ]),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `Generated ${formatIsoDateTime(receipt.generatedAt)}`,
          ]),
        ]),
        badge(receipt.outcome, sessionControlReceiptTone(receipt.outcome)),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Receipt ref', [receipt.receiptRef]),
        refSection('Request ref', [receipt.requestRef]),
        refSection('Session ref', [receipt.sessionRef]),
        refSection('Actor ref', [receipt.actorRef]),
        refSection('Provenance refs', receipt.provenanceRefs),
        refSection('Receipt blockers', receipt.blockerRefs),
      ]),
    ],
  )
}

const sessionNavigationPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const source = work.sessionNavigation
  const sessionNavigation = projectForgeSessionNavigation({
    generatedAt: work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source?.bridgeSessions === undefined
      ? {}
      : { bridgeSessions: source.bridgeSessions }),
    ...(source?.claudeSessions === undefined
      ? {}
      : { claudeSessions: source.claudeSessions }),
    ...(source?.codexSessions === undefined
      ? {}
      : { codexSessions: source.codexSessions }),
    ...(source?.controlReceipts === undefined
      ? {}
      : { controlReceipts: source.controlReceipts }),
    ...(source?.localPylonSessions === undefined
      ? {}
      : { localPylonSessions: source.localPylonSessions }),
  })

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Session navigation',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Session summaries for Pylon, Codex, Claude, and bridge runs with authority-gated resume, fork, rewind, and cancel controls.',
        ]),
      ]),
      badge(
        sessionNavigation.status.replaceAll('_', ' '),
        sessionNavigationTone(sessionNavigation.status),
      ),
    ]),
    sessionNavigation.items.length === 0
      ? h.div([Ui.className<Message>('border border-[#222] p-4')], [
          h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
            'No session summaries available yet.',
          ]),
        ])
      : h.div([Ui.className<Message>('grid gap-3')], [
          ...sessionNavigation.items.map(item =>
            sessionItemPanel(sessionNavigation.workOrderRef, item),
          ),
        ]),
    sessionNavigation.controlReceipts.length === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.div([Ui.className<Message>('grid gap-3')], [
          h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/75')], [
            'Session control receipts',
          ]),
          ...sessionNavigation.controlReceipts.map(sessionControlReceiptPanel),
        ]),
    refSection('Session blockers', sessionNavigation.blockerRefs),
    sessionNavigation.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${sessionNavigation.omittedUnsafeRefCount} unsafe session ref(s) were omitted before rendering.`,
      ]),
  ])
}

const supportDiagnosticsTone = (
  status: ForgeSupportDiagnosticsStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'attention'
      ? 'warning'
      : status === 'failing'
        ? 'negative'
        : 'accent'

const doctorSeverityTone = (
  severity: ForgeDoctorSeverity,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  severity === 'ok'
    ? 'positive'
    : severity === 'warning'
      ? 'warning'
      : severity === 'error'
        ? 'negative'
        : 'info'

const supportExportReadinessTone = (
  readiness: ForgeSupportExportReadiness,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  readiness === 'ready'
    ? 'positive'
    : readiness === 'consent_required'
      ? 'warning'
      : 'negative'

const doctorCheckPanel = (check: ForgeDoctorCheckItem): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-4 border border-[#222] p-4'),
      h.DataAttribute('forge-support-doctor-check', check.checkRef),
      h.DataAttribute('forge-support-doctor-category', check.category),
      h.DataAttribute('forge-support-doctor-severity', check.severity),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.h3(
          [
            Ui.className<Message>(
              'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/80',
            ),
          ],
          [check.checkRef],
        ),
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(check.category, 'accent'),
          badge(check.severity, doctorSeverityTone(check.severity)),
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Evidence', check.evidenceRefs),
        refSection('Fixes', check.fixRefs),
      ]),
    ],
  )
}

const supportDiagnosticsPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const source = work.supportDiagnostics
  const diagnostics = projectForgeSupportDiagnostics({
    generatedAt: work.generatedAt,
    workOrderRef: work.workOrderRef,
    ...(source?.diagnosticLogRefs === undefined
      ? {}
      : { diagnosticLogRefs: source.diagnosticLogRefs }),
    ...(source?.doctorChecks === undefined
      ? {}
      : { doctorChecks: source.doctorChecks }),
    ...(source?.helpCommandRefs === undefined
      ? {}
      : { helpCommandRefs: source.helpCommandRefs }),
    ...(source?.preflightRefs === undefined
      ? {}
      : { preflightRefs: source.preflightRefs }),
    ...(source?.supportBundleSections === undefined
      ? {}
      : { supportBundleSections: source.supportBundleSections }),
  })

  return h.section(
    [
      Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5'),
      h.DataAttribute('forge-support-diagnostics-status', diagnostics.status),
      h.DataAttribute(
        'forge-support-diagnostics-export-readiness',
        diagnostics.exportReadiness,
      ),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('grid gap-1')], [
          h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
            'Help, doctor, and debug',
          ]),
          h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
            'Refs-only help, environment doctor, preflight, and support-bundle evidence. Read-only: cannot run checks, export bundles, grant consent, mutate settings, or read credentials.',
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(
            diagnostics.status.replaceAll('_', ' '),
            supportDiagnosticsTone(diagnostics.status),
          ),
          badge(
            `export ${diagnostics.exportReadiness.replaceAll('_', ' ')}`,
            supportExportReadinessTone(diagnostics.exportReadiness),
          ),
        ]),
      ]),
      diagnostics.doctorChecks.length === 0
        ? h.div([Ui.className<Message>('border border-[#222] p-4')], [
            h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
              'No doctor checks reported yet.',
            ]),
          ])
        : h.div([Ui.className<Message>('grid gap-3')], [
            ...diagnostics.doctorChecks.map(doctorCheckPanel),
          ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Help commands', diagnostics.helpCommandRefs),
        refSection('Preflight', diagnostics.preflightRefs),
        refSection('Diagnostic logs', diagnostics.diagnosticLogRefs),
        refSection(
          'Support bundle sections',
          diagnostics.supportBundleSections.map(
            section => `${section.sectionRef} (${section.consent})`,
          ),
        ),
      ]),
      refSection('Support diagnostics blockers', diagnostics.blockerRefs),
      diagnostics.omittedUnsafeRefCount === 0
        ? h.span([Ui.className<Message>('hidden')], [])
        : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
            `${diagnostics.omittedUnsafeRefCount} unsafe support ref(s) were omitted before rendering.`,
          ]),
    ],
  )
}

const retrievalStatusTone = (
  status: ForgeRetrievalPlanStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready'
    ? 'positive'
    : status === 'blocked'
      ? 'negative'
      : status === 'stale'
        ? 'warning'
        : 'accent'

const retrievalFreshnessTone = (
  freshness: ForgeRetrievalFreshness,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  freshness === 'fresh'
    ? 'positive'
    : freshness === 'stale'
      ? 'warning'
      : 'accent'

const retrievalSkipTone = (
  reason: ForgeRetrievalSkippedCandidate['reason'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  reason === 'filtered_private' || reason === 'missing_source'
    ? 'warning'
    : reason === 'unsupported_mode'
      ? 'accent'
      : 'info'

const retrievalPlanInput = (
  work: AutopilotWorkProjection,
): ForgeRetrievalPlanInput => {
  const source = work.retrievalPlan
  const liveAdapter = source?.liveAdapter

  if (liveAdapter !== undefined) {
    return buildForgeLiveRetrievalPlanInput({
      generatedAt: liveAdapter.generatedAt ?? source?.generatedAt ?? work.generatedAt,
      planRef:
        liveAdapter.planRef ??
        source?.planRef ??
        `forge-live-retrieval-plan:${work.workOrderRef}`,
      requestRef: liveAdapter.requestRef ?? source?.requestRef ?? work.clientRequestRef,
      ...(liveAdapter.blockerRefs === undefined
        ? source?.blockerRefs === undefined
          ? {}
          : { blockerRefs: source.blockerRefs }
        : { blockerRefs: liveAdapter.blockerRefs }),
      ...(liveAdapter.freshness === undefined
        ? source?.freshness === undefined
          ? {}
          : { freshness: source.freshness }
        : { freshness: liveAdapter.freshness }),
      ...(liveAdapter.minimumScore === undefined
        ? {}
        : { minimumScore: liveAdapter.minimumScore }),
      ...(liveAdapter.mode === undefined
        ? source?.mode === undefined
          ? {}
          : { mode: source.mode }
        : { mode: liveAdapter.mode }),
      ...(liveAdapter.providerEvidenceRefs === undefined
        ? {}
        : { providerEvidenceRefs: liveAdapter.providerEvidenceRefs }),
      ...(liveAdapter.queryRefs === undefined
        ? source?.queryRefs === undefined
          ? {}
          : { queryRefs: source.queryRefs }
        : { queryRefs: liveAdapter.queryRefs }),
      ...(liveAdapter.sourceRefs === undefined
        ? source?.sourceRefs === undefined
          ? {}
          : { sourceRefs: source.sourceRefs }
        : { sourceRefs: liveAdapter.sourceRefs }),
      ...(liveAdapter.sources === undefined ? {} : { sources: liveAdapter.sources }),
      ...(liveAdapter.workspaceBoundaryRefs === undefined
        ? {}
        : { workspaceBoundaryRefs: liveAdapter.workspaceBoundaryRefs }),
    })
  }

  return {
    generatedAt: source?.generatedAt ?? work.generatedAt,
    mode: source?.mode ?? 'exact',
    planRef: source?.planRef ?? `forge-retrieval-plan:${work.workOrderRef}`,
    requestRef: source?.requestRef ?? work.clientRequestRef,
    ...(source?.blockerRefs === undefined ? {} : { blockerRefs: source.blockerRefs }),
    ...(source?.candidates === undefined ? {} : { candidates: source.candidates }),
    ...(source?.freshness === undefined ? {} : { freshness: source.freshness }),
    ...(source?.queryRefs === undefined ? {} : { queryRefs: source.queryRefs }),
    ...(source?.skippedCandidates === undefined
      ? {}
      : { skippedCandidates: source.skippedCandidates }),
    ...(source?.sourceRefs === undefined ? {} : { sourceRefs: source.sourceRefs }),
  }
}

const retrievalMetricValue = (value: number | string | null): string =>
  value === null ? 'missing' : String(value)

const retrievalCandidatePanel = (candidate: ForgeRetrievalCandidate): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-4 border border-[#222] p-4'),
      h.DataAttribute('forge-retrieval-candidate', candidate.candidateRef),
      h.DataAttribute('forge-retrieval-candidate-mode', candidate.mode),
      h.DataAttribute('forge-retrieval-candidate-freshness', candidate.freshness),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('min-w-0 grid gap-1')], [
          h.h3(
            [
              Ui.className<Message>(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/80',
              ),
            ],
            [candidate.candidateRef],
          ),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `Rank ${retrievalMetricValue(candidate.rank)} - score ${retrievalMetricValue(candidate.score)}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(candidate.mode, 'accent'),
          badge(candidate.freshness, retrievalFreshnessTone(candidate.freshness)),
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Candidate ref', [candidate.candidateRef]),
        refSection(
          'Source ref',
          candidate.sourceRef === null ? [] : [candidate.sourceRef],
        ),
        refSection('Provenance', candidate.provenanceRefs),
        refSection('Candidate blockers', candidate.blockerRefs),
      ]),
    ],
  )
}

const skippedRetrievalCandidatePanel = (
  candidate: ForgeRetrievalSkippedCandidate,
): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-4 border border-[#222] p-4'),
      h.DataAttribute('forge-retrieval-skipped-candidate', candidate.candidateRef),
      h.DataAttribute('forge-retrieval-skipped-reason', candidate.reason),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('min-w-0 grid gap-1')], [
          h.h3(
            [
              Ui.className<Message>(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/80',
              ),
            ],
            [candidate.candidateRef],
          ),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `Skipped: ${candidate.reason.replaceAll('_', ' ')}`,
          ]),
        ]),
        badge(candidate.reason.replaceAll('_', ' '), retrievalSkipTone(candidate.reason)),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Skipped candidate', [candidate.candidateRef]),
        refSection(
          'Source ref',
          candidate.sourceRef === null ? [] : [candidate.sourceRef],
        ),
        refSection('Skip blockers', candidate.blockerRefs),
      ]),
    ],
  )
}

const retrievalSearchPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const retrieval = projectForgeRetrievalPlan(retrievalPlanInput(work))

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Retrieval search',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Refs-only retrieval plan evidence. Ranking is projected upstream; the cockpit only renders selected and skipped candidates.',
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(retrieval.status, retrievalStatusTone(retrieval.status)),
        badge(retrieval.mode, 'accent'),
        badge(retrieval.freshness, retrievalFreshnessTone(retrieval.freshness)),
      ]),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
      contextMetric('Selected', String(retrieval.resultSet.totalSelected)),
      contextMetric('Skipped', String(retrieval.resultSet.totalSkipped)),
      contextMetric('Sources', String(retrieval.resultSet.sourceRefs.length)),
      contextMetric('Plan ref', retrieval.planRef),
    ]),
    refSection('Query refs', retrieval.queryRefs),
    retrieval.candidates.length === 0
      ? h.div([Ui.className<Message>('border border-[#222] p-4')], [
          h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
            'No retrieval candidates selected yet.',
          ]),
        ])
      : h.div([Ui.className<Message>('grid gap-3')], [
          ...retrieval.candidates.map(retrievalCandidatePanel),
        ]),
    retrieval.skippedCandidates.length === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.div([Ui.className<Message>('grid gap-3')], [
          h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/75')], [
            'Skipped candidates',
          ]),
          ...retrieval.skippedCandidates.map(skippedRetrievalCandidatePanel),
        ]),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Source refs', retrieval.sourceRefs),
        refSection('Result selected refs', retrieval.resultSet.selectedCandidateRefs),
        refSection('Result skipped refs', retrieval.resultSet.skippedCandidateRefs),
        refSection('Retrieval blockers', retrieval.blockerRefs),
      ],
    ),
    retrieval.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${retrieval.omittedUnsafeRefCount} unsafe retrieval ref(s) were omitted before rendering.`,
        ]),
  ])
}

const extensibilityExecutionInput = (
  work: AutopilotWorkProjection,
): ForgeExtensibilityExecutionReceiptsInput | undefined => {
  const source = work.extensibility
  const effectiveConfig = source?.effectiveConfig

  return source === undefined
    ? undefined
    : {
        config: {
          configRef:
            effectiveConfig?.configRef ?? `extensibility-config:${work.workOrderRef}`,
          generatedAt: effectiveConfig?.generatedAt ?? work.generatedAt,
          workOrderRef: effectiveConfig?.workOrderRef ?? work.workOrderRef,
          ...(effectiveConfig?.blockerRefs === undefined
            ? {}
            : { blockerRefs: effectiveConfig.blockerRefs }),
          ...(effectiveConfig?.entries === undefined
            ? {}
            : { entries: effectiveConfig.entries }),
          ...(effectiveConfig?.freshness === undefined
            ? {}
            : { freshness: effectiveConfig.freshness }),
        },
        generatedAt: work.generatedAt,
        workOrderRef: work.workOrderRef,
        ...(source.executionRequests === undefined
          ? {}
          : { requests: source.executionRequests }),
      }
}

const extensibilityOutcomeTone = (
  outcome: ForgeExtensibilityExecutionReceipt['outcome'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  outcome === 'callable'
    ? 'positive'
    : outcome === 'blocked' || outcome === 'failed'
      ? 'negative'
      : outcome === 'needs_auth' || outcome === 'needs_trust'
        ? 'warning'
        : 'accent'

const extensibilityStatusTone = (
  status: ForgeExtensibilityExecutionReceiptsView['status'],
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready' ? 'positive' : status === 'blocked' ? 'negative' : 'accent'

const extensibilityReceiptPanel = (
  receipt: ForgeExtensibilityExecutionReceipt,
): Html => {
  const h = html<Message>()

  return h.article(
    [
      Ui.className<Message>('grid gap-4 border border-[#222] p-4'),
      h.DataAttribute('forge-extensibility-execution-receipt', receipt.receiptRef),
      h.DataAttribute('forge-extensibility-execution-outcome', receipt.outcome),
      h.DataAttribute('forge-extensibility-execution-kind', receipt.requestKind),
    ],
    [
      h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
        h.div([Ui.className<Message>('min-w-0 grid gap-1')], [
          h.h3(
            [
              Ui.className<Message>(
                'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/80',
              ),
            ],
            [receipt.requestKind.replaceAll('_', ' ')],
          ),
          h.p([Ui.className<Message>('m-0 text-xs text-white/40')], [
            `Generated ${formatIsoDateTime(receipt.generatedAt)} - skill body loaded: ${String(receipt.authority.skillBodyLoaded)}`,
          ]),
        ]),
        h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
          badge(receipt.outcome.replaceAll('_', ' '), extensibilityOutcomeTone(receipt.outcome)),
          badge(receipt.domain, 'accent'),
        ]),
      ]),
      h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
        refSection('Request ref', [receipt.requestRef]),
        refSection('Receipt ref', [receipt.receiptRef]),
        refSection('Target ref', [receipt.targetRef]),
        refSection('Config refs', receipt.configRefs),
        refSection('Catalog refs', receipt.catalogRefs),
        refSection('Policy refs', receipt.policyRefs),
        refSection('Source refs', receipt.sourceRefs),
        refSection('Auth refs', receipt.authRefs),
        refSection('Provider account refs', receipt.providerAccountRefs),
        refSection('Workspace trust refs', receipt.workspaceTrustRefs),
        refSection('Failure refs', receipt.failureRefs),
        refSection('Request blockers', receipt.blockerRefs),
      ]),
    ],
  )
}

const extensibilityExecutionPanel = (work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const input = extensibilityExecutionInput(work)

  if (input === undefined) {
    return h.span([Ui.className<Message>('hidden')], [])
  }

  const view = projectForgeExtensibilityExecutionReceipts(input)

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Extensibility requests',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Refs-only request receipts for MCP, skills, hooks, plugins, and settings capabilities.',
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(view.status, extensibilityStatusTone(view.status)),
        badge(view.config.status.replaceAll('_', ' '), 'accent'),
      ]),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
      contextMetric('Requests', String(view.receipts.length)),
      contextMetric('Config status', view.config.status),
      contextMetric('Skill bodies loaded', 'false'),
      contextMetric('Unsafe refs omitted', String(view.omittedUnsafeRefCount)),
    ]),
    view.receipts.length === 0
      ? h.div([Ui.className<Message>('border border-[#222] p-4')], [
          h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
            'No extensibility execution requests available yet.',
          ]),
        ])
      : h.div([Ui.className<Message>('grid gap-3')], [
          ...view.receipts.map(extensibilityReceiptPanel),
        ]),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Effective config ref', [view.config.configRef]),
        refSection('Extensibility blockers', view.blockerRefs),
      ],
    ),
    view.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${view.omittedUnsafeRefCount} unsafe extensibility ref(s) were omitted before rendering.`,
        ]),
  ])
}

const briefingPanel = (briefing: AutopilotMissionBriefing): Html => {
  const closeoutRefs = briefing.drilldown.flatMap(group =>
    group.kind === 'closeout' ? group.refs : [],
  )
  const assignmentRefs = briefing.drilldown.flatMap(group =>
    group.kind === 'assignment' ? group.refs : [],
  )
  const buildRefs = briefing.drilldown.flatMap(group =>
    group.kind === 'build' ? group.refs : [],
  )
  const testRefs = briefing.drilldown.flatMap(group =>
    group.kind === 'test' ? group.refs : [],
  )

  return html<Message>().section(
    [Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')],
    [
      html<Message>().div([Ui.className<Message>('grid gap-1')], [
        html<Message>().h2(
          [Ui.className<Message>('m-0 text-base font-medium text-white/80')],
          ['Evidence bundle'],
        ),
        html<Message>().p([Ui.className<Message>('m-0 text-sm text-white/45')], [
          `Generated ${formatIsoDateTime(briefing.generatedAt)}`,
        ]),
      ]),
      html<Message>().div(
        [Ui.className<Message>('grid gap-4 lg:grid-cols-2')],
        [
          refSection('Assignments', assignmentRefs),
          refSection('Closeouts', closeoutRefs),
          refSection('Build refs', buildRefs),
          refSection('Test refs', testRefs),
          refSection('Artifacts', briefing.whatChanged.artifactRefs),
          refSection('Results', briefing.whatChanged.resultRefs),
          refSection('Blockers', briefing.whatIsBlocked.blockerRefs),
          refSection('Next action', briefing.decisionsWaiting.callerActionRefs),
        ],
      ),
    ],
  )
}

const briefingStatePanel = (model: Model): Html =>
  M.value(model.autopilotWorkBriefing).pipe(
    M.tags({
      AutopilotWorkBriefingIdle: () =>
        loadingView('Evidence bundle has not loaded.'),
      AutopilotWorkBriefingLoading: () =>
        loadingView('Loading evidence bundle...'),
      AutopilotWorkBriefingFailed: ({ error }) => errorView(error),
      AutopilotWorkBriefingLoaded: ({ response }) =>
        briefingPanel(response.briefing),
    }),
    M.exhaustive,
  )

const loadedBriefing = (model: Model): AutopilotMissionBriefing | null =>
  model.autopilotWorkBriefing._tag === 'AutopilotWorkBriefingLoaded'
    ? model.autopilotWorkBriefing.response.briefing
    : null

const reviewStatusLabel = (status: ForgeDiffReviewStatus): string =>
  status.replaceAll('_', ' ')

const reviewStatusTone = (
  status: ForgeDiffReviewStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'review_ready'
    ? 'positive'
    : status === 'pending_delivery'
      ? 'info'
      : 'warning'

const diffArtifactDrilldownTone = (
  status: ForgeDiffArtifactDrilldownStatus,
): 'accent' | 'positive' | 'warning' | 'negative' | 'info' =>
  status === 'ready' ? 'positive' : status === 'stale' ? 'warning' : 'negative'

const reviewValue = (value: number | string | null): string =>
  value === null ? 'missing' : String(value)

const lineDeltaLabel = (review: ForgeDiffReviewView): string => {
  const added = review.addedLineCount === null ? '?' : `+${review.addedLineCount}`
  const removed =
    review.removedLineCount === null ? '?' : `-${review.removedLineCount}`

  return `${added} / ${removed}`
}

const reviewMetric = (label: string, value: string): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-1 border border-[#222] p-3')], [
    h.div([Ui.className<Message>('text-[0.6875rem] uppercase text-white/35')], [
      label,
    ]),
    h.div(
      [Ui.className<Message>('min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-sm font-medium text-white/75')],
      [value],
    ),
  ])
}

const diffArtifactDrilldownGroupPanel = (
  group: ForgeDiffArtifactDrilldownFileGroup,
): Html => {
  const h = html<Message>()

  return h.div([Ui.className<Message>('grid gap-3 border border-[#222] p-3')], [
    h.div(
      [Ui.className<Message>('min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-xs font-medium text-white/70')],
      [group.groupRef],
    ),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-2')], [
      refSection('File refs', group.fileRefs),
      refSection('Artifact refs', group.artifactRefs),
      refSection('Hunk summaries', group.hunkSummaryRefs),
      refSection('Summaries', group.summaryRefs),
    ]),
  ])
}

const diffReviewPanel = (
  model: Model,
  work: AutopilotWorkProjection,
): Html => {
  const h = html<Message>()
  const briefing = loadedBriefing(model)
  const review = projectForgeDiffReview(work, briefing)
  const drilldown = projectForgeDiffArtifactDrilldown(work, briefing)
  const drilldownId = `diff-artifact-drilldown-${work.workOrderRef}`

  return h.section([Ui.className<Message>('grid gap-4 border-t border-[#222] pt-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('grid gap-1')], [
        h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
          'Review changes',
        ]),
        h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
          'Refs-only delivery review. Raw patches and private local material stay out of the cockpit projection.',
        ]),
      ]),
      badge(reviewStatusLabel(review.status), reviewStatusTone(review.status)),
    ]),
    h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
      reviewMetric('Files', reviewValue(review.fileCount)),
      reviewMetric('Lines', lineDeltaLabel(review)),
      reviewMetric('Patch digest', reviewValue(review.patchDigestRef)),
      reviewMetric('Verification', review.verificationState),
    ]),
    review.artifactRefs.length === 0
      ? h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          'No safe diff artifact ref is available for drilldown yet.',
        ])
      : h.a(
          [
            h.Href(`#${drilldownId}`),
            Ui.className<Message>(
              'w-fit border border-[#333] px-3 py-2 text-sm font-medium text-white/70 hover:border-white/45 hover:text-white',
            ),
          ],
          ['Open diff artifact drilldown'],
        ),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Change captures', review.changeCaptureRefs),
        refSection('Delivery readiness', review.deliveryReadinessRefs),
        refSection('Verification refs', review.verificationRefs),
        refSection('Writeback authority', review.authorityReceiptRefs),
        refSection('Artifacts', review.artifactRefs),
        refSection('Results', review.resultRefs),
        refSection('Caveats', review.reviewCaveatRefs),
        refSection('Blockers', review.blockerRefs),
      ],
    ),
    review.omittedUnsafeRefCount === 0
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
          `${review.omittedUnsafeRefCount} unsafe review ref(s) were omitted before rendering.`,
        ]),
    h.div(
      [
        h.Id(drilldownId),
        Ui.className<Message>('grid gap-4 border border-[#222] bg-[#050505] p-4'),
      ],
      [
        h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
          h.div([Ui.className<Message>('grid gap-1')], [
            h.h3([Ui.className<Message>('m-0 text-sm font-medium text-white/80')], [
              'Diff artifact drilldown',
            ]),
            h.p([Ui.className<Message>('m-0 max-w-3xl text-sm text-white/45')], [
              'Bounded artifact evidence only. This panel does not fetch raw patches or grant accept, writeback, deploy, or settlement authority.',
            ]),
          ]),
          badge(
            drilldown.status.replaceAll('_', ' '),
            diffArtifactDrilldownTone(drilldown.status),
          ),
        ]),
        h.div([Ui.className<Message>('grid gap-3 md:grid-cols-4')], [
          reviewMetric('Patch digest', reviewValue(drilldown.patchDigestRef)),
          reviewMetric('Artifacts', String(drilldown.artifactRefs.length)),
          reviewMetric('File groups', String(drilldown.fileGroups.length)),
          reviewMetric('Hunk summaries', String(drilldown.hunkSummaryRefs.length)),
        ]),
        h.div([Ui.className<Message>('grid gap-4 md:grid-cols-2')], [
          refSection('Drilldown artifacts', drilldown.artifactRefs),
          refSection('Change captures', drilldown.changeCaptureRefs),
          refSection('Delivery readiness', drilldown.deliveryReadinessRefs),
          refSection('Verification refs', drilldown.verificationRefs),
          refSection('Caveats', drilldown.caveatRefs),
          refSection('Drilldown blockers', drilldown.blockerRefs),
        ]),
        drilldown.fileGroups.length === 0
          ? h.div([Ui.className<Message>('border border-[#222] p-3')], [
              h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
                'No file-group refs available for this artifact yet.',
              ]),
            ])
          : h.div([Ui.className<Message>('grid gap-3')], [
              ...drilldown.fileGroups.map(diffArtifactDrilldownGroupPanel),
            ]),
        drilldown.omittedUnsafeRefCount === 0
          ? h.span([Ui.className<Message>('hidden')], [])
          : h.p([Ui.className<Message>('m-0 text-sm text-[#ffb400]')], [
              `${drilldown.omittedUnsafeRefCount} unsafe diff artifact ref(s) were omitted before rendering.`,
            ]),
      ],
    ),
  ])
}

// The accepted-outcome receipt: an approval gate over a delivered Run. Reframes
// the review action onto the `AiElements` confirmation primitive while keeping
// the existing review messages and disable rules.
const receiptAction = (
  work: AutopilotWorkProjection,
  action: AutopilotWorkReviewAction,
  label: string,
  variant: 'primary' | 'secondary' | 'danger',
): Html => {
  const h = html<Message>()
  const disabled = work.state !== 'delivered' || work.reviewDecision !== null

  return Ui.AiElements.confirmationAction<Message>({
    label,
    variant,
    attrs: [
      h.Type('button'),
      ...(disabled
        ? [h.Disabled(true)]
        : [
            h.OnClick(
              SubmittedAutopilotWorkReview({
                action,
                workOrderRef: work.workOrderRef,
              }),
            ),
          ]),
    ],
  })
}

const receiptState = (
  work: AutopilotWorkProjection,
): Ui.AiElements.ConfirmationState => {
  if (work.reviewDecision === null) {
    return 'requested'
  }

  return work.reviewDecision.action === 'accept' ? 'approved' : 'rejected'
}

const receiptDetail = (work: AutopilotWorkProjection): string =>
  work.reviewDecision === null
    ? work.state === 'delivered'
      ? 'Run delivered. Accept the outcome to record a receipt, or send it back.'
      : 'The receipt opens once the Run is delivered.'
    : `Recorded: ${work.reviewDecision.action.replaceAll('_', ' ')}`

const receiptPanel = (model: Model, work: AutopilotWorkProjection): Html => {
  const h = html<Message>()
  const state = receiptState(work)

  return h.section([Ui.className<Message>('grid gap-3 border-t border-[#222] pt-5')], [
    h.h2([Ui.className<Message>('m-0 text-base font-medium text-white/80')], [
      'Accepted-outcome receipt',
    ]),
    Ui.AiElements.confirmation<Message>({
      props: {
        title: `Outcome for ${work.workOrderRef}`,
        state,
        detail: receiptDetail(work),
      },
      actions: [
        receiptAction(work, 'accept', 'Accept', 'primary'),
        receiptAction(work, 'request_changes', 'Request changes', 'secondary'),
        receiptAction(work, 'reject', 'Reject', 'danger'),
      ],
    }),
    M.value(model.autopilotWorkReview).pipe(
      M.tags({
        AutopilotWorkReviewIdle: () => h.span([Ui.className<Message>('hidden')], []),
        AutopilotWorkReviewSubmitting: ({ action }) =>
          loadingView(`Submitting ${action.replaceAll('_', ' ')}...`),
        AutopilotWorkReviewSucceeded: () =>
          h.p([Ui.className<Message>('m-0 text-sm text-[#7ccf8a]')], [
            'Receipt recorded.',
          ]),
        AutopilotWorkReviewFailed: ({ error }) => errorView(error),
      }),
      M.exhaustive,
    ),
  ])
}

const workSummaryPanel = (model: Model, work: AutopilotWorkProjection): Html => {
  const h = html<Message>()

  return h.section([Ui.className<Message>('grid gap-5')], [
    h.div([Ui.className<Message>('flex flex-wrap items-start justify-between gap-3')], [
      h.div([Ui.className<Message>('min-w-0 grid gap-2')], [
        h.a(
          [
            h.Href(autopilotWorkRouter()),
            Ui.className<Message>(Ui.textLinkClass),
          ],
          ['Forge cockpit'],
        ),
        h.h1(
          [
            Ui.className<Message>(
              'm-0 min-w-0 overflow-hidden text-ellipsis whitespace-nowrap text-2xl font-semibold text-white',
            ),
          ],
          [work.workOrderRef],
        ),
        h.p([Ui.className<Message>('m-0 text-sm text-white/45')], [
          `Runs on ${placementSummary(work)} - generated ${formatIsoDateTime(work.generatedAt)}`,
        ]),
      ]),
      h.div([Ui.className<Message>('flex flex-wrap items-center gap-2')], [
        badge(stateLabel(work.state), stateTone(work.state)),
        Ui.button<Message>({
          attrs: [
            h.Type('button'),
            h.OnClick(
              RequestedLoadAutopilotWorkDetail({
                workOrderRef: work.workOrderRef,
              }),
            ),
          ],
          label: 'Refresh',
          size: 'sm',
          variant: 'secondary',
        }),
      ]),
    ]),
    h.div(
      [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
      [
        refSection('Tasks', work.taskRefs),
        refSection('Access requests', work.accessRequestRefs),
        refSection('Next action refs', work.nextAction.callerActionRefs),
        refSection('Reason refs', work.nextAction.reasonRefs),
      ],
    ),
    work.executionCloseout === null
      ? h.span([Ui.className<Message>('hidden')], [])
      : h.div(
          [Ui.className<Message>('grid gap-4 border border-[#222] p-4 md:grid-cols-2')],
          [
            refSection('Assignment refs', work.executionCloseout.assignmentRefs),
            refSection('Closeout refs', work.executionCloseout.closeoutRefs),
            refSection('Proof refs', work.executionCloseout.proofRefs),
            refSection('Result refs', work.executionCloseout.resultRefs),
            refSection('Artifact refs', work.executionCloseout.artifactRefs ?? []),
            refSection('Build refs', work.executionCloseout.buildRefs ?? []),
            refSection('Test refs', work.executionCloseout.testRefs ?? []),
            refSection('Blocker refs', work.executionCloseout.blockerRefs ?? []),
          ],
    ),
    progressPanel(model, work),
    notificationAttentionPanel(work),
    errorRecoveryPanel(model, work),
    compactionSummaryPanel(work),
    usageBudgetPanel(work),
    modelProviderPanel(work),
    instructionLayeringPanel(work),
    sessionMemoryPanel(work),
    contextSnapshotPanel(work),
    diagnosticsPanel(work),
    helpDoctorDebugPanel(work),
    mcpServerExportPanel(work),
    settingsConfigurationPanel(work),
    credentialStoragePanel(work),
    gitWorkflowPanel(work),
    editorIntegrationPanel(work),
    browserDesktopIntegrationPanel(work),
    multimodalInputPanel(work),
    remoteSessionBridgePanel(work),
    companionSurfacePanel(work),
    teamSharedMemoryPanel(work),
    multiAgentCoordinationPanel(work),
    externalWorkIntakePanel(work),
    artifactReceiptIndexPanel(work),
    schedulingCronPanel(work),
    structuredEventLogPanel(work),
    telemetryPrivacyPanel(work),
    performanceDiagnosticsPanel(work),
    updateReleasePanel(work),
    migrationEvidencePanel(work),
    testingSmokeEvidencePanel(work),
    evaluationRegressionEvidencePanel(work),
    securityReviewEvidencePanel(work),
    dataRetentionDeletionPanel(work),
    onboardingCapabilityPanel(work),
    outputStylePersonaPanel(work),
    promptSuggestionsPanel(work),
    tipsEducationPanel(work),
    themeVisualPanel(work),
    accessibilityNonInteractivePanel(work),
    localizationBoundaryPanel(work),
    enterpriseManagedPolicyPanel(work),
    terminalUiShellPanel(work),
    inputKeybindingPanel(work),
    commandSystemPanel(work),
    sessionNavigationPanel(work),
    supportDiagnosticsPanel(work),
    retrievalSearchPanel(work),
    extensibilityExecutionPanel(work),
    diffReviewPanel(model, work),
    receiptPanel(model, work),
    eventsPanel(model),
    briefingStatePanel(model),
  ])
}

export const detailView = (model: Model): Html =>
  M.value(model.autopilotWorkDetail).pipe(
    M.tags({
      AutopilotWorkDetailIdle: () => loadingView('Run has not loaded.'),
      AutopilotWorkDetailLoading: () => loadingView('Loading Run...'),
      AutopilotWorkDetailFailed: ({ error }) => errorView(error),
      AutopilotWorkDetailLoaded: ({ response }) =>
        workSummaryPanel(model, response.work),
    }),
    M.exhaustive,
  )

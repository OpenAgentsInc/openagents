import { Effect, Schema as S } from 'effect'

import {
  type SiteBuilderPreviewKind,
  type SiteBuilderPreviewRecord,
  type SiteBuilderRuntime,
  SiteBuilderSessionStorageError,
  SiteBuilderSessionValidationError,
  appendSiteBuilderEvent,
  recordSiteBuilderPreview,
  systemSiteBuilderRuntime,
} from './sites-builder-sessions'

export const SiteBuilderPreviewTier = S.Literals([
  'r2_static',
  'wfp_staging',
  'container_metered',
])
export type SiteBuilderPreviewTier = typeof SiteBuilderPreviewTier.Type

export const SiteBuilderPreviewCandidateKind = S.Literals([
  'static_assets',
  'worker_module',
  'needs_build',
  'dev_server',
  'unknown',
])
export type SiteBuilderPreviewCandidateKind =
  typeof SiteBuilderPreviewCandidateKind.Type

export type SiteBuilderPreviewRuntimeNeeds = Readonly<{
  buildExecution?: boolean | undefined
  dependencyHeavy?: boolean | undefined
  dependencyInstall?: boolean | undefined
  devServer?: boolean | undefined
  runtimeRepair?: boolean | undefined
  ssrRuntime?: boolean | undefined
}>

export type SiteBuilderPreviewCandidate = Readonly<{
  artifactRef?: string | undefined
  candidateKind: SiteBuilderPreviewCandidateKind
  healthRef?: string | undefined
  previewUrl?: string | undefined
  runtimeNeeds?: SiteBuilderPreviewRuntimeNeeds | undefined
  versionRef?: string | undefined
  workerModulePath?: string | undefined
}>

export type SiteBuilderPreviewTierSelection = Readonly<{
  containerWorkGated: boolean
  customerSafeSummary: string
  previewKind: SiteBuilderPreviewKind
  reason: string
  tier: SiteBuilderPreviewTier
}>

export type RecordSiteBuilderPreviewCandidateInput = Readonly<{
  candidate: SiteBuilderPreviewCandidate
  id?: string | undefined
  idempotencyKey: string
  sessionId: string
}>

export type SiteBuilderPreviewRunnerResult = Readonly<{
  preview: SiteBuilderPreviewRecord
  selection: SiteBuilderPreviewTierSelection
}>

export const selectSiteBuilderPreviewTier = (
  candidate: SiteBuilderPreviewCandidate,
): SiteBuilderPreviewTierSelection => {
  const needs = candidate.runtimeNeeds ?? {}

  if (
    candidate.candidateKind === 'dev_server' ||
    candidate.candidateKind === 'needs_build' ||
    needs.buildExecution === true ||
    needs.dependencyHeavy === true ||
    needs.dependencyInstall === true ||
    needs.devServer === true ||
    needs.runtimeRepair === true ||
    needs.ssrRuntime === true
  ) {
    return {
      containerWorkGated: true,
      customerSafeSummary:
        'This candidate needs metered build or runtime execution before preview.',
      previewKind: 'container',
      reason: 'build_or_runtime_execution_required',
      tier: 'container_metered',
    }
  }

  if (
    candidate.candidateKind === 'worker_module' ||
    candidate.workerModulePath !== undefined
  ) {
    return {
      containerWorkGated: false,
      customerSafeSummary:
        'This candidate can use a staging Worker-compatible preview.',
      previewKind: 'workers_for_platforms',
      reason: 'worker_module_ready',
      tier: 'wfp_staging',
    }
  }

  return {
    containerWorkGated: false,
    customerSafeSummary: 'This candidate can use a static OpenAgents preview.',
    previewKind: 'static_r2',
    reason: 'static_candidate',
    tier: 'r2_static',
  }
}

const previewStatusForSelection = (
  selection: SiteBuilderPreviewTierSelection,
  candidate: SiteBuilderPreviewCandidate,
) => {
  if (candidate.previewUrl !== undefined && !selection.containerWorkGated) {
    return 'ready' as const
  }

  return selection.containerWorkGated
    ? ('requested' as const)
    : ('building' as const)
}

const previewMetadata = (
  selection: SiteBuilderPreviewTierSelection,
  candidate: SiteBuilderPreviewCandidate,
) => ({
  candidateKind: candidate.candidateKind,
  containerWorkGated: selection.containerWorkGated,
  customerSafeSummary: selection.customerSafeSummary,
  selectedReason: selection.reason,
  tier: selection.tier,
  ...(candidate.workerModulePath === undefined
    ? {}
    : { workerModulePath: candidate.workerModulePath }),
})

export const recordSiteBuilderPreviewCandidate = (
  db: D1Database,
  input: RecordSiteBuilderPreviewCandidateInput,
  runtime: SiteBuilderRuntime = systemSiteBuilderRuntime,
): Effect.Effect<
  SiteBuilderPreviewRunnerResult,
  SiteBuilderSessionStorageError | SiteBuilderSessionValidationError
> =>
  Effect.gen(function* () {
    const selection = selectSiteBuilderPreviewTier(input.candidate)
    const preview = yield* recordSiteBuilderPreview(
      db,
      {
        artifactRef: input.candidate.artifactRef,
        healthRef: input.candidate.healthRef,
        id: input.id,
        idempotencyKey: input.idempotencyKey,
        metadata: previewMetadata(selection, input.candidate),
        previewKind: selection.previewKind,
        previewUrl: input.candidate.previewUrl,
        sessionId: input.sessionId,
        status: previewStatusForSelection(selection, input.candidate),
        versionRef: input.candidate.versionRef,
      },
      runtime,
    )

    yield* appendSiteBuilderEvent(
      db,
      {
        eventKind: 'preview_created',
        idempotencyKey: `${input.idempotencyKey}:event`,
        payload: {
          containerWorkGated: selection.containerWorkGated,
          previewId: preview.id,
          selectedReason: selection.reason,
          status: preview.status,
          tier: selection.tier,
        },
        phaseKind: 'preview',
        sessionId: input.sessionId,
        status:
          preview.status === 'ready'
            ? 'succeeded'
            : selection.containerWorkGated
              ? 'blocked'
              : 'running',
        summary: selection.customerSafeSummary,
        title: 'Preview tier selected',
        visibility: 'customer',
      },
      runtime,
    )

    return { preview, selection }
  })

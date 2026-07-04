import { type WorkerBindings } from '@openagentsinc/sync-worker'
import { Effect } from 'effect'

import type { OnboardingStreamMetadata } from './autopilot-onboarding-program'
import type { OpenAgentsWorkerConfigEnv } from './config'
import {
  buildKhalaTokensServedDelta,
  publishKhalaTokensServedDelta,
} from './inference/khala-tokens-served-sync'
import {
  type InferenceAdapterRouteMetadata,
  type InferenceUsage,
} from './inference/provider-adapter'
import {
  type ServedTokensRecorderInput,
  buildServedTokensIngestBody,
} from './inference/served-tokens-recorder'
import type { KhalaSyncHyperdriveBinding } from './khala-sync-push-routes'
import { recordTokensServedProjectionBestEffort } from './khala-sync-public-tokens-served'
import { openAgentsDatabase } from './runtime'
import { currentIsoTimestamp } from './runtime-primitives'

const PUBLIC_KHALA_CHAT_ACCOUNT_REF = 'public:khala-chat'
const PUBLIC_KHALA_CHAT_DEMAND_SOURCE = 'khala-cli-public-chat'

type PublicKhalaChatServedTokenEnv = OpenAgentsWorkerConfigEnv &
  Pick<WorkerBindings, 'OPENAGENTS_DB' | 'SYNC_ROOM'> &
  Readonly<{ KHALA_SYNC_DB?: KhalaSyncHyperdriveBinding }>

const publicKhalaChatServedTokenMetrics = (
  adapterId: string,
  fallbackReason: string | null | undefined,
  adapterRouteMetadata: InferenceAdapterRouteMetadata | undefined,
): ServedTokensRecorderInput['requestMetrics'] => ({
  requestClass: 'interactive_stream',
  ...(fallbackReason === undefined || fallbackReason === null
    ? {}
    : { fallbackReason }),
  supplyLane: adapterId,
  ...(adapterRouteMetadata?.selectedReplicaId === undefined
    ? {}
    : { selectedReplicaId: adapterRouteMetadata.selectedReplicaId }),
  ...(adapterRouteMetadata?.selectedReplicaRef === undefined
    ? {}
    : { selectedReplicaRef: adapterRouteMetadata.selectedReplicaRef }),
  ...(adapterRouteMetadata?.replicaFallbackReason === undefined
    ? {}
    : { replicaFallbackReason: adapterRouteMetadata.replicaFallbackReason }),
  ...(adapterRouteMetadata?.replicaBusyReason === undefined
    ? {}
    : { replicaBusyReason: adapterRouteMetadata.replicaBusyReason }),
  ...(adapterRouteMetadata?.replicaHealthScore === undefined
    ? {}
    : { replicaHealthScore: adapterRouteMetadata.replicaHealthScore }),
  ...(adapterRouteMetadata?.replicaRegion === undefined
    ? {}
    : { replicaRegion: adapterRouteMetadata.replicaRegion }),
  ...(adapterRouteMetadata?.replicaCapacityClass === undefined
    ? {}
    : { replicaCapacityClass: adapterRouteMetadata.replicaCapacityClass }),
  ...(adapterRouteMetadata?.replicaCostProfileRef === undefined
    ? {}
    : { replicaCostProfileRef: adapterRouteMetadata.replicaCostProfileRef }),
  ...(adapterRouteMetadata?.replicaInflightCount === undefined
    ? {}
    : { replicaInflightCount: adapterRouteMetadata.replicaInflightCount }),
  ...(adapterRouteMetadata?.replicaMaxInflight === undefined
    ? {}
    : { replicaMaxInflight: adapterRouteMetadata.replicaMaxInflight }),
  ...(adapterRouteMetadata?.replicaQueueDepth === undefined
    ? {}
    : { replicaQueueDepth: adapterRouteMetadata.replicaQueueDepth }),
  ...(adapterRouteMetadata?.replicaWarmState === undefined
    ? {}
    : { replicaWarmState: adapterRouteMetadata.replicaWarmState }),
  ...(adapterRouteMetadata?.glmSaturationPolicy === undefined
    ? {}
    : { glmSaturationPolicy: adapterRouteMetadata.glmSaturationPolicy }),
  ...(adapterRouteMetadata?.queueWaitMs === undefined
    ? {}
    : { queueWaitMs: adapterRouteMetadata.queueWaitMs }),
})

export const recordPublicKhalaChatServedTokens = ({
  env: rawEnv,
  metadata,
  traceRef,
}: {
  env: unknown
  metadata: OnboardingStreamMetadata
  traceRef: string
}): Effect.Effect<void, unknown> => {
  const usage = metadata.usage as InferenceUsage | undefined
  const servedAdapterId = metadata.servedAdapterId
  const servedModel = metadata.servedModel
  const requestedModel = metadata.requestedModel
  if (
    usage === undefined ||
    servedAdapterId === undefined ||
    servedModel === undefined ||
    requestedModel === undefined
  ) {
    return Effect.sync(() => undefined)
  }
  const env = rawEnv as PublicKhalaChatServedTokenEnv | undefined
  if (env === undefined) {
    return Effect.sync(() => undefined)
  }
  const inputTokens = Math.max(0, Math.trunc(usage.promptTokens))
  const outputTokens = Math.max(0, Math.trunc(usage.completionTokens))
  if (inputTokens + outputTokens <= 0) {
    return Effect.sync(() => undefined)
  }

  const observedAt = currentIsoTimestamp()
  const body = buildServedTokensIngestBody({
    accountRef: PUBLIC_KHALA_CHAT_ACCOUNT_REF,
    adapterId: servedAdapterId,
    observedAt,
    requestAttribution: {
      demandClient: 'khala-cli',
      demandKind: 'external',
      demandSource: PUBLIC_KHALA_CHAT_DEMAND_SOURCE,
    },
    requestId: traceRef,
    requestMetrics: publicKhalaChatServedTokenMetrics(
      servedAdapterId,
      metadata.fallbackReason,
      metadata.adapterRouteMetadata as InferenceAdapterRouteMetadata | undefined,
    ),
    requestedModel,
    servedModel,
    usage,
  })
  return Effect.promise(async () => {
    const result = await openAgentsDatabase(env)
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
        currentIsoTimestamp(),
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
        JSON.stringify(body.safeMetadata ?? {}),
      )
      .run()

    if (Number(result.meta.changes ?? 0) > 0) {
      // KS-6.3 (#8304): bump the scope.public.tokens-served projection for
      // this fresh row, exact-once by the row's idempotency key. Fail-soft
      // by contract; never affects the served completion.
      await recordTokensServedProjectionBestEffort(
        { binding: env.KHALA_SYNC_DB },
        {
          idempotencyKey: body.idempotencyKey,
          observedAt,
          tokensServedDelta: inputTokens + outputTokens,
        },
      ).catch(() => undefined)

      await publishKhalaTokensServedDelta(
        env,
        buildKhalaTokensServedDelta({
          eventRef: body.eventId,
          observedAt,
          tokensServedDelta: inputTokens + outputTokens,
        }),
      ).catch(() => undefined)
    }
  })
}

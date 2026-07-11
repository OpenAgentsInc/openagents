import { Effect } from 'effect'

import {
  ClientGroupId,
  ClientId,
  decodeRuntimeInteraction,
  KHALA_SYNC_PROTOCOL_VERSION,
  MutationEnvelope,
  MutationId,
  MutatorName,
  PushRequest,
  SyncSchemaVersion,
} from '@openagentsinc/khala-sync'
import {
  executePush,
  readRuntimeInteractionByRef,
  RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME,
  type MutatorRegistry,
} from '@openagentsinc/khala-sync-server'

import { methodNotAllowed, noStoreJsonResponse } from './http/responses'
import {
  defaultMakeKhalaSyncSqlClient,
  type KhalaSyncHyperdriveBinding,
  type KhalaSyncPushSqlClient,
  type MakeKhalaSyncPushSqlClient,
} from './khala-sync-push-routes'

export const KHALA_SYNC_RUNTIME_INTERACTION_PATH =
  '/api/internal/khala-sync/runtime-interaction'
export const KHALA_SYNC_RUNTIME_INTERACTION_ROUTE_REF =
  'route.internal.khala_sync.runtime_interaction.v1'

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$/

export type KhalaSyncRuntimeInteractionDependencies = Readonly<{
  requireOperator: () => Promise<boolean>
  binding: KhalaSyncHyperdriveBinding | undefined
  registry: MutatorRegistry
  makeSqlClient?: MakeKhalaSyncPushSqlClient
  readInteraction?: typeof readRuntimeInteractionByRef
  executeMutation?: typeof executePush
}>

const connectionString = (
  binding: KhalaSyncHyperdriveBinding | undefined,
): string | null => typeof binding?.connectionString === 'string' && binding.connectionString !== ''
  ? binding.connectionString
  : null

/**
 * Trusted Pylon interaction seam. POST requests one canonical pending
 * interaction through the real mutator; GET reads only one exact owner/ref
 * post-image. The admin credential never enters the durable entity.
 */
export const handleKhalaSyncRuntimeInteraction = (
  request: Request,
  deps: KhalaSyncRuntimeInteractionDependencies,
): Effect.Effect<Response> => Effect.promise(async () => {
  if (request.method !== 'GET' && request.method !== 'POST') {
    return methodNotAllowed(['GET', 'POST'])
  }
  if (!(await deps.requireOperator())) {
    return noStoreJsonResponse({ error: 'unauthorized' }, { status: 401 })
  }
  const configured = connectionString(deps.binding)
  if (configured === null) {
    return noStoreJsonResponse({ ok: false, reason: 'Khala Sync storage is not configured.', routeRef: KHALA_SYNC_RUNTIME_INTERACTION_ROUTE_REF })
  }

  let client: KhalaSyncPushSqlClient | undefined
  try {
    client = await (deps.makeSqlClient ?? defaultMakeKhalaSyncSqlClient)(configured)
    if (request.method === 'GET') {
      const url = new URL(request.url)
      const ownerUserId = url.searchParams.get('ownerUserId')
      const interactionRef = url.searchParams.get('interactionRef')
      if (ownerUserId === null || interactionRef === null || !SAFE_REF.test(ownerUserId) || !SAFE_REF.test(interactionRef)) {
        return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
      }
      const interaction = await (deps.readInteraction ?? readRuntimeInteractionByRef)(client.sql, { interactionRef, ownerUserId })
      return noStoreJsonResponse({ interaction: interaction?.interaction ?? null, ok: true, routeRef: KHALA_SYNC_RUNTIME_INTERACTION_ROUTE_REF })
    }

    const contentLength = Number(request.headers.get('content-length') ?? '0')
    if (Number.isFinite(contentLength) && contentLength > 64 * 1024) {
      return noStoreJsonResponse({ error: 'payload_too_large' }, { status: 413 })
    }
    const body = await request.json() as { ownerUserId?: unknown; interaction?: unknown }
    if (typeof body.ownerUserId !== 'string' || !SAFE_REF.test(body.ownerUserId)) {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }
    let interaction
    try {
      interaction = decodeRuntimeInteraction(body.interaction)
    } catch {
      return noStoreJsonResponse({ error: 'invalid_request' }, { status: 400 })
    }
    const push = new PushRequest({
      clientGroupId: ClientGroupId.make(`server.pylon.interaction.${interaction.interactionRef}`),
      clientId: ClientId.make('openagents.worker.pylon_runtime_interaction'),
      mutations: [new MutationEnvelope({
        argsJson: JSON.stringify(interaction),
        mutationId: MutationId.make(1),
        name: MutatorName.make(RUNTIME_REQUEST_INTERACTION_MUTATOR_NAME),
      })],
      protocolVersion: KHALA_SYNC_PROTOCOL_VERSION,
      schemaVersion: SyncSchemaVersion.make(1),
    })
    const result = (await (deps.executeMutation ?? executePush)({
      registry: deps.registry,
      request: push,
      sql: client.sql,
      userId: body.ownerUserId,
    })).results[0]
    return result?.status === 'applied'
      ? noStoreJsonResponse({ ok: true, routeRef: KHALA_SYNC_RUNTIME_INTERACTION_ROUTE_REF })
      : noStoreJsonResponse({ error: result?.errorCode ?? 'mutation_rejected', ok: false, routeRef: KHALA_SYNC_RUNTIME_INTERACTION_ROUTE_REF }, { status: 409 })
  } catch {
    return noStoreJsonResponse({ error: 'khala_sync_runtime_interaction_failed', ok: false, routeRef: KHALA_SYNC_RUNTIME_INTERACTION_ROUTE_REF }, { status: 503 })
  } finally {
    try { await client?.end() } catch { /* best-effort close */ }
  }
})

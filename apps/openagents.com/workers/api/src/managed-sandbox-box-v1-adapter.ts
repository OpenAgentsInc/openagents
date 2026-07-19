import {
  ManagedSandboxStoreError,
  PostgresManagedSandboxStore,
} from '@openagentsinc/khala-sync-server'
import {
  BOX_V1_TRANSLATOR_REF,
  ManagedSandboxResourceSchema,
  ManagedSandboxRuntimeEventInputSchema,
} from '@openagentsinc/managed-sandbox-contract'
import { Effect, Schema as S } from 'effect'

import type { OpenAgentsWorkerEnv } from './bindings'
import { parseJsonUnknown } from './json-boundary'
import { defaultMakeKhalaSyncSqlClient } from './khala-sync-push-routes'
import {
  BoxV1FacadeError,
  type BoxV1NativeStore,
  type BoxV1Policy,
  type BoxV1Principal,
  type BoxV1Runtime,
  unavailableBoxV1Runtime,
  upstreamUnavailable,
} from './managed-sandbox-box-v1-routes'

export const isManagedSandboxBoxV1Enabled = (
  value: string | undefined,
): boolean =>
  value === '1' ||
  value?.toLowerCase() === 'true' ||
  value?.toLowerCase() === 'on'

const storeError = (error: unknown): BoxV1FacadeError => {
  if (error instanceof ManagedSandboxStoreError) {
    switch (error.code) {
      case 'not_found':
        return new BoxV1FacadeError({
          code: 'resource_not_found',
          status: 404,
          message: error.message,
          retryable: false,
        })
      case 'permission_denied':
        return new BoxV1FacadeError({
          code: 'permission_denied',
          status: 403,
          message: error.message,
          retryable: false,
        })
      case 'invalid':
      case 'unsafe_value':
        return new BoxV1FacadeError({
          code: 'validation_failed',
          status: 400,
          message: error.message,
          retryable: false,
        })
      case 'idempotency_conflict':
      case 'command_conflict':
      case 'stale_version':
      case 'stale_generation':
      case 'invalid_transition':
      case 'event_conflict':
      case 'cursor_conflict':
        return new BoxV1FacadeError({
          code: 'conflict',
          status: 409,
          message: error.message,
          retryable: false,
        })
      case 'corrupt_store':
        return new BoxV1FacadeError({
          code: 'upstream_unavailable',
          status: 503,
          message: 'managed-sandbox lifecycle authority is unavailable',
          retryable: true,
        })
    }
  }
  return new BoxV1FacadeError({
    code: 'upstream_unavailable',
    status: 503,
    message: 'managed-sandbox lifecycle authority is unavailable',
    retryable: true,
  })
}

const connectionString = (
  env: OpenAgentsWorkerEnv,
): Effect.Effect<string, BoxV1FacadeError> => {
  const value = env.KHALA_SYNC_DB?.connectionString
  return typeof value === 'string' && value.length > 0
    ? Effect.succeed(value)
    : Effect.fail(
        new BoxV1FacadeError({
          code: 'upstream_unavailable',
          status: 503,
          message: 'managed-sandbox lifecycle storage is not configured',
          retryable: true,
        }),
      )
}

const withPostgresStore = <A>(
  env: OpenAgentsWorkerEnv,
  use: (store: PostgresManagedSandboxStore) => Promise<A>,
): Effect.Effect<A, BoxV1FacadeError> =>
  Effect.gen(function* () {
    const configured = yield* connectionString(env)
    const client = yield* Effect.tryPromise({
      try: () => defaultMakeKhalaSyncSqlClient(configured),
      catch: storeError,
    })
    return yield* Effect.tryPromise({
      try: () => use(new PostgresManagedSandboxStore(client.sql)),
      catch: storeError,
    }).pipe(
      Effect.ensuring(
        Effect.tryPromise({
          try: () => client.end(),
          catch: () => undefined,
        }).pipe(Effect.ignore),
      ),
    )
  })

export const managedSandboxBoxV1StoreForEnv = (
  env: OpenAgentsWorkerEnv,
): BoxV1NativeStore => ({
  reservation: input =>
    withPostgresStore(env, store => store.reservation(input)),
  reserve: input => withPostgresStore(env, store => store.reserve(input)),
  settle: input => withPostgresStore(env, store => store.settle(input)),
  inspect: input => withPostgresStore(env, store => store.inspect(input)),
  list: input => withPostgresStore(env, store => store.list(input)),
  readEvents: input => withPostgresStore(env, store => store.readEvents(input)),
  turns: input => withPostgresStore(env, store => store.turns(input)),
  inspectTurn: input =>
    withPostgresStore(env, store => store.inspectTurn(input)),
  readTurnEvents: input =>
    withPostgresStore(env, store => store.readTurnEvents(input)),
  recordRuntimeEvents: input =>
    withPostgresStore(env, store => store.recordRuntimeEvents(input)),
  readProjection: input =>
    withPostgresStore(env, store => store.readProjection(input)),
  advanceProjection: input =>
    withPostgresStore(env, store => store.advanceProjection(input)),
})

export const managedSandboxBoxV1PolicyForEnv = (
  env: OpenAgentsWorkerEnv,
): Effect.Effect<BoxV1Policy, BoxV1FacadeError> => {
  const imageDigest = env.OA_MANAGED_SANDBOX_IMAGE_DIGEST
  if (imageDigest === undefined) {
    return Effect.fail(
      new BoxV1FacadeError({
        code: 'capacity_unavailable',
        status: 503,
        message: 'the exact managed-sandbox image/profile is not admitted',
        retryable: true,
        details: { translatorRef: BOX_V1_TRANSLATOR_REF },
      }),
    )
  }
  return Effect.try({
    try: () => {
      const resource = S.decodeUnknownSync(ManagedSandboxResourceSchema)({
        schema: 'openagents.managed_sandbox.v1',
        sandboxRef: 'sandbox.policy.validation',
        ownerRef: 'owner.policy.validation',
        tenantRef: 'tenant.policy.validation',
        programRef: 'program.managed_agent_sandboxes',
        workUnitRef: 'work.policy.validation',
        attachmentRef: 'attachment.policy.validation',
        attachmentGeneration: 1,
        resourceGeneration: 1,
        version: 0,
        lastEventSequence: 0,
        target: {
          targetRef: 'target.gcp.managed-sandbox.us-central1',
          targetClass: 'openagents_managed',
          provider: 'google_cloud',
          adapterRef: 'adapter.oa-codex-control.gce.v1',
          region: 'us-central1',
          isolation: 'gce_vm',
          dataPosture: 'openagents_managed_region',
        },
        imageDigest,
        profileRef: 'profile.sbx.gce.e2-small.v1',
        lease: {
          leaseRef: 'lease.policy.validation',
          state: 'active',
          issuedAt: '2026-07-19T00:00:00.000Z',
          expiresAt: '2026-07-19T01:00:00.000Z',
          ttlSeconds: 3_600,
          renewable: true,
        },
        budget: {
          currency: 'USD',
          maxCostMicros: 10_000,
          maxCpuMillis: 3_600_000,
          maxNetworkBytes: 100_000_000,
          maxArtifactBytes: 10_000_000,
          maxLifetimeSeconds: 3_600,
        },
        capabilities: [],
        facts: {
          lifecycle: 'provisioning',
          leaseState: 'active',
          guestState: 'starting',
          filesystemState: 'unallocated',
          ingressState: 'closed',
          runtimeState: 'none',
          acceptingWork: false,
          cleanupComplete: false,
        },
        createdAt: '2026-07-19T00:00:00.000Z',
        updatedAt: '2026-07-19T00:00:00.000Z',
      })
      return {
        target: resource.target,
        imageDigest: resource.imageDigest,
        profileRef: resource.profileRef,
        defaultTtlSeconds: 3_600,
        maxTtlSeconds: 86_400,
        maxActiveBoxes: 2,
        maxCostMicros: 10_000,
        maxCpuMillis: 86_400_000,
        maxNetworkBytes: 100_000_000,
        maxArtifactBytes: 10_000_000,
      }
    },
    catch: () =>
      new BoxV1FacadeError({
        code: 'capacity_unavailable',
        status: 503,
        message: 'the exact managed-sandbox image/profile is not admitted',
        retryable: true,
        details: { translatorRef: BOX_V1_TRANSLATOR_REF },
      }),
  })
}

const digestRef = (value: string): Effect.Effect<string, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: async () => {
      const digest = await crypto.subtle.digest(
        'SHA-256',
        new TextEncoder().encode(value),
      )
      return [...new Uint8Array(digest)]
        .map(byte => byte.toString(16).padStart(2, '0'))
        .join('')
    },
    catch: () =>
      new BoxV1FacadeError({
        code: 'upstream_unavailable',
        status: 503,
        message: 'owner scope could not be derived',
        retryable: true,
      }),
  })

export const makeBoxV1Principal = (input: {
  actorUserId: string
  linkedOwnerUserId?: string | null
  login?: string | null
  email?: string | null
}): Effect.Effect<BoxV1Principal, BoxV1FacadeError> => {
  const ownerRef = input.linkedOwnerUserId ?? input.actorUserId
  return digestRef(ownerRef).pipe(
    Effect.map(ownerDigest => ({
      actorRef: `agent:${input.actorUserId}`,
      ownerRef,
      tenantRef: `tenant.owner.${ownerDigest.slice(0, 32)}`,
      login: input.login ?? input.actorUserId,
      email: input.email ?? null,
    })),
  )
}

const managedSandboxTurnResponseSchema = S.Struct({
  schemaVersion: S.Literal('openagents.managed_sandbox_turn_runtime.v1'),
  turnRef: S.String,
  resourceGeneration: S.Number,
  events: S.Array(ManagedSandboxRuntimeEventInputSchema),
})

const managedSandboxRuntimeRequest = (
  env: OpenAgentsWorkerEnv,
  body: Readonly<Record<string, unknown>>,
) =>
  Effect.gen(function* () {
    const baseUrl = env.OA_CLOUD_CONTROL_URL?.trim()
    const bearerToken = env.OA_CLOUD_CONTROL_TOKEN?.trim()
    if (!baseUrl || !bearerToken) {
      return yield* upstreamUnavailable('agent_turn')
    }
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `${baseUrl.replace(/\/$/, '')}/v1/managed-sandbox/runtime/turns`,
          {
            method: 'POST',
            headers: {
              authorization: `Bearer ${bearerToken}`,
              'content-type': 'application/json',
            },
            body: JSON.stringify({
              schemaVersion: 'openagents.managed_sandbox_turn_runtime.v1',
              ...body,
            }),
          },
        ),
      catch: () =>
        new BoxV1FacadeError({
          code: 'upstream_unavailable',
          status: 503,
          message: 'managed-sandbox turn control is unavailable',
          retryable: true,
        }),
    })
    const text = yield* Effect.tryPromise({
      try: () => response.text(),
      catch: () =>
        new BoxV1FacadeError({
          code: 'upstream_unavailable',
          status: 503,
          message: 'managed-sandbox turn response is unavailable',
          retryable: true,
        }),
    })
    if (!response.ok) {
      return yield* new BoxV1FacadeError({
        code: response.status === 409 ? 'conflict' : 'upstream_unavailable',
        status: response.status === 409 ? 409 : 503,
        message: 'managed-sandbox turn control refused the operation',
        retryable: response.status >= 500,
      })
    }
    return yield* Effect.try({
      try: () =>
        S.decodeUnknownSync(managedSandboxTurnResponseSchema)(
          parseJsonUnknown(text),
        ),
      catch: () =>
        new BoxV1FacadeError({
          code: 'upstream_unavailable',
          status: 503,
          message: 'managed-sandbox turn response failed contract validation',
          retryable: true,
        }),
    })
  })

const runtimeScope = (
  input: Readonly<{
    principal: Parameters<BoxV1Runtime['sync']>[0]['principal']
    resource: Parameters<BoxV1Runtime['sync']>[0]['resource']
    turn: Parameters<BoxV1Runtime['sync']>[0]['turn']
  }>,
): Readonly<Record<string, unknown>> => ({
  actorRef: input.principal.actorRef,
  ownerRef: input.principal.ownerRef,
  tenantRef: input.principal.tenantRef,
  programRef: input.resource.programRef,
  workUnitRef: input.resource.workUnitRef,
  sandboxRef: input.resource.sandboxRef,
  turnRef: input.turn.turnRef,
  expectedResourceGeneration: input.resource.resourceGeneration,
  promptDigest: input.turn.promptDigest,
  runtime: input.turn.runtime,
})

export const managedSandboxBoxV1RuntimeForEnv = (
  env: OpenAgentsWorkerEnv,
): Effect.Effect<BoxV1Runtime, BoxV1FacadeError> =>
  Effect.succeed({
    ...unavailableBoxV1Runtime,
    dispatch: input =>
      managedSandboxRuntimeRequest(env, {
        action: 'dispatch',
        ...runtimeScope(input),
        prompt: input.prompt,
      }).pipe(Effect.map(response => response.events)),
    sync: input =>
      managedSandboxRuntimeRequest(env, {
        action: 'sync',
        ...runtimeScope(input),
        afterTurnSequence: input.afterTurnSequence,
      }).pipe(Effect.map(response => response.events)),
    interrupt: input =>
      managedSandboxRuntimeRequest(env, {
        action: 'interrupt',
        ...runtimeScope(input),
        afterTurnSequence: input.turn.lastEventSequence,
        reasonRef: input.reasonRef,
        idempotencyRef: input.idempotencyRef,
      }).pipe(Effect.map(response => response.events)),
  })

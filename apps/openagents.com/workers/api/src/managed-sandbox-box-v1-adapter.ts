import {
  ManagedSandboxStoreError,
  PostgresManagedSandboxStore,
} from '@openagentsinc/khala-sync-server'
import {
  BOX_V1_TRANSLATOR_REF,
  type ManagedSandboxCommand,
  type ManagedSandboxGuestIoAction,
  type ManagedSandboxGuestIoResponse,
  ManagedSandboxGuestIoResponseSchema,
  type ManagedSandboxResource,
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
import {
  managedSandboxProviderModel,
  mintManagedSandboxProviderCapability,
} from './managed-sandbox-provider-broker'

export const isManagedSandboxBoxV1Enabled = (
  value: string | undefined,
): boolean =>
  value === '1' ||
  value?.toLowerCase() === 'true' ||
  value?.toLowerCase() === 'on'

export const isManagedSandboxBrokerEnabled = isManagedSandboxBoxV1Enabled

const Sha256RefPattern = /^sha256:[0-9a-f]{64}$/

export const isManagedSandboxRuntimeConfigured = (
  env: OpenAgentsWorkerEnv,
): boolean =>
  env.KHALA_SYNC_DB !== undefined &&
  typeof env.OA_MANAGED_SANDBOX_CONTROL_URL === 'string' &&
  env.OA_MANAGED_SANDBOX_CONTROL_URL.trim().startsWith('https://') &&
  typeof env.OA_MANAGED_SANDBOX_CONTROL_TOKEN === 'string' &&
  env.OA_MANAGED_SANDBOX_CONTROL_TOKEN.trim() !== '' &&
  typeof env.OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY === 'string' &&
  env.OA_MANAGED_SANDBOX_BROKER_SIGNING_KEY.trim().length >= 32 &&
  typeof env.OA_MANAGED_SANDBOX_IMAGE_DIGEST === 'string' &&
  Sha256RefPattern.test(env.OA_MANAGED_SANDBOX_IMAGE_DIGEST.trim()) &&
  typeof env.OA_MANAGED_SANDBOX_PROFILE_DIGEST === 'string' &&
  Sha256RefPattern.test(env.OA_MANAGED_SANDBOX_PROFILE_DIGEST.trim())

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
          expiresAt: '2026-07-19T00:15:00.000Z',
          ttlSeconds: 900,
          renewable: true,
        },
        budget: {
          currency: 'USD',
          maxCostMicros: 10_000,
          maxCpuMillis: 900_000,
          maxNetworkBytes: 100_000_000,
          maxArtifactBytes: 10_000_000,
          maxLifetimeSeconds: 900,
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
        defaultTtlSeconds: 900,
        maxTtlSeconds: 1_800,
        maxActiveBoxes: 2,
        maxCostMicros: 10_000,
        maxCpuMillis: 1_800_000,
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

const NonNegativeInteger = S.Number.check(
  S.isInt(),
  S.isGreaterThanOrEqualTo(0),
)
const PositiveInteger = S.Number.check(S.isInt(), S.isGreaterThan(0))

const managedSandboxLifecycleResponseSchema = S.Struct({
  schemaVersion: S.Literal('openagents.managed_sandbox_runtime.v1'),
  receiptRef: S.String,
  operationRef: S.String,
  action: S.Literals(['create', 'stop', 'resume', 'delete']),
  sandboxRef: S.String,
  generation: PositiveInteger,
  phase: S.Literals([
    'ready',
    'stopped',
    'failed',
    'recovery_required',
    'deleted',
  ]),
  targetRef: S.String,
  profileRef: S.String,
  profileDigest: S.String,
  imageRef: S.String,
  imageDigest: S.String,
  isolationClass: S.String,
  networkPolicyRef: S.String,
  controlIdentityRef: S.String,
  guestIdentityRef: S.String,
  providerKind: S.Literal('live_gce'),
  readinessObserved: S.Boolean,
  cleanupObserved: S.Boolean,
  measuredRunningMs: NonNegativeInteger,
  measuredCostMicrousd: NonNegativeInteger,
  sandboxBudgetMicrousd: PositiveInteger,
  programBudgetMicrousd: PositiveInteger,
  emittedAtMs: PositiveInteger,
  errorCode: S.NullOr(S.String),
})

const lifecycleAction = (
  command: Extract<
    ManagedSandboxCommand,
    { _tag: 'Create' | 'Stop' | 'Resume' | 'Delete' }
  >,
): 'create' | 'stop' | 'resume' | 'delete' =>
  command._tag.toLowerCase() as 'create' | 'stop' | 'resume' | 'delete'

const managedSandboxLifecycleRequest = (
  env: OpenAgentsWorkerEnv,
  input: {
    principal: BoxV1Principal
    resource: ManagedSandboxResource
    command: Extract<
      ManagedSandboxCommand,
      { _tag: 'Create' | 'Stop' | 'Resume' | 'Delete' }
    >
  },
) =>
  Effect.gen(function* () {
    const baseUrl = env.OA_MANAGED_SANDBOX_CONTROL_URL?.trim()
    const bearerToken = env.OA_MANAGED_SANDBOX_CONTROL_TOKEN?.trim()
    const profileDigest = env.OA_MANAGED_SANDBOX_PROFILE_DIGEST?.trim()
    if (!baseUrl || !bearerToken || !profileDigest) {
      return yield* upstreamUnavailable('lifecycle')
    }
    const action = lifecycleAction(input.command)
    const capabilityRefs = yield* Effect.forEach(
      input.resource.capabilities,
      capability =>
        digestRef(
          `${input.resource.sandboxRef}\n${input.resource.resourceGeneration}\n${capability.capabilityRef}`,
        ).pipe(
          Effect.map(value => `capability-ref://run/${value.slice(0, 32)}`),
        ),
    )
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(
          `${baseUrl.replace(/\/$/, '')}/v1/managed-sandbox/runtime/operations`,
          {
            method: 'POST',
            headers: {
              'content-type': 'application/json',
              'x-openagents-managed-sandbox-token': bearerToken,
            },
            body: JSON.stringify({
              operationRef: input.command.commandRef,
              idempotencyRef: input.command.idempotencyRef,
              actorRef: input.principal.actorRef,
              ownerRef: input.resource.ownerRef,
              tenantRef: input.resource.tenantRef,
              programRef: input.resource.programRef,
              workUnitRef: input.resource.workUnitRef,
              sandboxRef: input.resource.sandboxRef,
              expectedGeneration:
                action === 'create' ? 0 : input.resource.resourceGeneration,
              action,
              ...(action === 'create'
                ? {
                    profile: {
                      profileRef: input.resource.profileRef,
                      profileDigest,
                      targetRef:
                        'target://openagents/google-cloud/managed-sandbox',
                      provisionerRef:
                        'provisioner-ref://openagents/oa-codex-control/gce-v1',
                      region: input.resource.target.region,
                      machineClass: 'e2-small',
                      isolationClass: input.resource.target.isolation,
                      imageRef: `gce-image-ref://sha256/${input.resource.imageDigest.replace('sha256:', '')}`,
                      imageDigest: input.resource.imageDigest,
                      networkPolicyRef:
                        'network-policy-ref://openagents/managed-sandbox/broker-only-v1',
                      controlIdentityRef:
                        'identity-ref://openagents/managed-sandbox/control',
                      guestIdentityRef:
                        'identity-ref://openagents/managed-sandbox/guest-none',
                      ttlMs: input.resource.lease.ttlSeconds * 1_000,
                      capacity: {
                        minCapacity: 0,
                        maxCapacity: 2,
                        prewarmCapacity: 0,
                        concurrentCapacityCap: 2,
                      },
                      budget: {
                        sandboxBudgetMicrousd:
                          input.resource.budget.maxCostMicros,
                        programBudgetMicrousd: 40_000,
                        maxHourlyCostMicrousd: 20_000,
                      },
                      capabilityRefs,
                    },
                  }
                : {}),
            }),
          },
        ),
      catch: () => upstreamUnavailable('lifecycle'),
    })
    const text = yield* Effect.tryPromise({
      try: () => response.text(),
      catch: () => upstreamUnavailable('lifecycle_response'),
    })
    if (!response.ok) {
      return yield* new BoxV1FacadeError({
        code:
          response.status === 403
            ? 'permission_denied'
            : response.status === 409
              ? 'conflict'
              : response.status === 429
                ? 'rate_limited'
                : 'upstream_unavailable',
        status: [403, 409, 429].includes(response.status)
          ? response.status
          : 503,
        message: 'managed-sandbox lifecycle control refused the operation',
        retryable: response.status === 429 || response.status >= 500,
      })
    }
    const receipt = yield* Effect.try({
      try: () =>
        S.decodeUnknownSync(managedSandboxLifecycleResponseSchema)(
          parseJsonUnknown(text),
          {
            onExcessProperty: 'preserve',
          },
        ),
      catch: () => upstreamUnavailable('lifecycle_contract'),
    })
    const expectedGeneration =
      action === 'create'
        ? 1
        : action === 'resume'
          ? input.resource.resourceGeneration + 1
          : input.resource.resourceGeneration
    const expectedPhase = {
      create: 'ready',
      stop: 'stopped',
      resume: 'ready',
      delete: 'deleted',
    }[action]
    const expectedImageRef = `gce-image-ref://sha256/${input.resource.imageDigest.replace('sha256:', '')}`
    if (
      receipt.operationRef !== input.command.commandRef ||
      receipt.action !== action ||
      receipt.sandboxRef !== input.resource.sandboxRef ||
      receipt.generation !== expectedGeneration ||
      (receipt.phase !== expectedPhase &&
        receipt.phase !== 'failed' &&
        receipt.phase !== 'recovery_required') ||
      receipt.targetRef !==
        'target://openagents/google-cloud/managed-sandbox' ||
      receipt.profileRef !== input.resource.profileRef ||
      receipt.profileDigest !== profileDigest ||
      receipt.imageRef !== expectedImageRef ||
      receipt.imageDigest !== input.resource.imageDigest ||
      receipt.isolationClass !== input.resource.target.isolation ||
      receipt.networkPolicyRef !==
        'network-policy-ref://openagents/managed-sandbox/broker-only-v1' ||
      receipt.controlIdentityRef !==
        'identity-ref://openagents/managed-sandbox/control' ||
      receipt.guestIdentityRef !==
        'identity-ref://openagents/managed-sandbox/guest-none' ||
      receipt.readinessObserved !== (receipt.phase === 'ready') ||
      (receipt.phase === 'deleted' && !receipt.cleanupObserved) ||
      (receipt.cleanupObserved &&
        receipt.phase !== 'deleted' &&
        receipt.phase !== 'failed') ||
      receipt.measuredCostMicrousd > receipt.sandboxBudgetMicrousd ||
      receipt.sandboxBudgetMicrousd !== input.resource.budget.maxCostMicros ||
      receipt.programBudgetMicrousd !== 40_000 ||
      !Number.isFinite(new Date(receipt.emittedAtMs).getTime())
    ) {
      return yield* new BoxV1FacadeError({
        code: 'conflict',
        status: 409,
        message:
          'managed-sandbox lifecycle receipt did not bind the exact request',
        retryable: false,
      })
    }
    return {
      operationRef: receipt.operationRef,
      receiptRef: receipt.receiptRef,
      action: receipt.action,
      phase: receipt.phase,
      generation: receipt.generation,
      readinessObserved: receipt.readinessObserved,
      cleanupObserved: receipt.cleanupObserved,
      measuredRunningMs: receipt.measuredRunningMs,
      measuredCostMicros: receipt.measuredCostMicrousd,
      errorCode: receipt.errorCode,
      observedAt: new Date(receipt.emittedAtMs).toISOString(),
    }
  })

const guestIoBytes = (
  encoding: 'utf8' | 'base64',
  content: string,
): Effect.Effect<Uint8Array, BoxV1FacadeError> =>
  Effect.try({
    try: () => {
      if (encoding === 'utf8') return new TextEncoder().encode(content)
      const decoded = atob(content)
      return Uint8Array.from(decoded, character => character.charCodeAt(0))
    },
    catch: () =>
      new BoxV1FacadeError({
        code: 'validation_failed',
        status: 400,
        message: 'base64 guest I/O content is invalid',
        retryable: false,
      }),
  })

const guestIoDigest = (
  bytes: Uint8Array,
): Effect.Effect<string, BoxV1FacadeError> =>
  Effect.tryPromise({
    try: async () => {
      const digest = await crypto.subtle.digest(
        'SHA-256',
        Uint8Array.from(bytes).buffer,
      )
      return `sha256:${[...new Uint8Array(digest)]
        .map(byte => byte.toString(16).padStart(2, '0'))
        .join('')}`
    },
    catch: () => upstreamUnavailable('guest_io_digest'),
  })

const managedSandboxGuestIoRequest = (
  env: OpenAgentsWorkerEnv,
  body: Readonly<Record<string, unknown>>,
) =>
  Effect.gen(function* () {
    const baseUrl = env.OA_MANAGED_SANDBOX_CONTROL_URL?.trim()
    const bearerToken = env.OA_MANAGED_SANDBOX_CONTROL_TOKEN?.trim()
    if (!baseUrl || !bearerToken) return yield* upstreamUnavailable('guest_io')
    const response = yield* Effect.tryPromise({
      try: () =>
        fetch(`${baseUrl.replace(/\/$/, '')}/v1/managed-sandbox/runtime/io`, {
          method: 'POST',
          headers: {
            'content-type': 'application/json',
            'x-openagents-managed-sandbox-token': bearerToken,
          },
          body: JSON.stringify({
            schemaVersion: 'openagents.managed_sandbox_guest_io.v1',
            ...body,
          }),
        }),
      catch: () => upstreamUnavailable('guest_io'),
    })
    const text = yield* Effect.tryPromise({
      try: () => response.text(),
      catch: () => upstreamUnavailable('guest_io_response'),
    })
    if (!response.ok) {
      const status = response.status
      return yield* new BoxV1FacadeError({
        code:
          status === 400
            ? 'validation_failed'
            : status === 403
              ? 'permission_denied'
              : status === 409
                ? 'conflict'
                : status === 429
                  ? 'rate_limited'
                  : 'upstream_unavailable',
        status: [400, 403, 409, 429].includes(status) ? status : 503,
        message: 'managed-sandbox guest I/O control refused the operation',
        retryable: status === 429 || status >= 500,
      })
    }
    return yield* Effect.try({
      try: () =>
        S.decodeUnknownSync(ManagedSandboxGuestIoResponseSchema)(
          parseJsonUnknown(text),
        ),
      catch: () => upstreamUnavailable('guest_io_contract'),
    })
  })

type GuestIoResponseFor<Action extends ManagedSandboxGuestIoAction> = Extract<
  ManagedSandboxGuestIoResponse,
  { action: Action }
>

const exactGuestIoResponse = <Action extends ManagedSandboxGuestIoAction>(
  response: ManagedSandboxGuestIoResponse,
  action: Action,
  input: {
    operationRef: string
    capabilityRef: string
    resource: { sandboxRef: string; resourceGeneration: number }
  },
): Effect.Effect<GuestIoResponseFor<Action>, BoxV1FacadeError> => {
  if (
    response.action !== action ||
    response.operationRef !== input.operationRef ||
    response.sandboxRef !== input.resource.sandboxRef ||
    response.resourceGeneration !== input.resource.resourceGeneration ||
    response.receipt.operationRef !== input.operationRef ||
    response.receipt.sandboxRef !== input.resource.sandboxRef ||
    response.receipt.resourceGeneration !== input.resource.resourceGeneration ||
    response.receipt.capabilityRef !== input.capabilityRef ||
    response.receipt.action !== action ||
    response.receipt.outcome !== 'succeeded'
  ) {
    return Effect.fail(
      new BoxV1FacadeError({
        code: 'conflict',
        status: 409,
        message: 'guest I/O response does not bind the exact request scope',
        retryable: false,
      }),
    )
  }
  return Effect.succeed(response as GuestIoResponseFor<Action>)
}

const managedSandboxRuntimeRequest = (
  env: OpenAgentsWorkerEnv,
  body: Readonly<Record<string, unknown>>,
) =>
  Effect.gen(function* () {
    const baseUrl = env.OA_MANAGED_SANDBOX_CONTROL_URL?.trim()
    const bearerToken = env.OA_MANAGED_SANDBOX_CONTROL_TOKEN?.trim()
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
              'content-type': 'application/json',
              'x-openagents-managed-sandbox-token': bearerToken,
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

const guestIoScope = (input: {
  principal: Parameters<BoxV1Runtime['readFile']>[0]['principal']
  resource: Parameters<BoxV1Runtime['readFile']>[0]['resource']
  operationRef: string
  idempotencyRef: string
  capabilityRef: string
  capabilityState: 'active'
  capabilityExpiresAt: string
  requestedAt: string
  limits: Parameters<BoxV1Runtime['readFile']>[0]['limits']
}): Readonly<Record<string, unknown>> => ({
  operationRef: input.operationRef,
  idempotencyRef: input.idempotencyRef,
  actorRef: input.principal.actorRef,
  ownerRef: input.principal.ownerRef,
  tenantRef: input.principal.tenantRef,
  programRef: input.resource.programRef,
  workUnitRef: input.resource.workUnitRef,
  sandboxRef: input.resource.sandboxRef,
  resourceGeneration: input.resource.resourceGeneration,
  capabilityRef: input.capabilityRef,
  capabilityState: input.capabilityState,
  capabilityExpiresAt: input.capabilityExpiresAt,
  requestedAt: input.requestedAt,
  limits: input.limits,
})

export const managedSandboxBoxV1RuntimeForEnv = (
  env: OpenAgentsWorkerEnv,
): Effect.Effect<BoxV1Runtime, BoxV1FacadeError> =>
  Effect.succeed({
    ...unavailableBoxV1Runtime,
    lifecycle: input => managedSandboxLifecycleRequest(env, input),
    dispatch: input =>
      Effect.gen(function* () {
        const capability = input.resource.capabilities.find(
          candidate =>
            candidate.kind === 'agent_turn' && candidate.state === 'active',
        )
        if (capability === undefined) {
          return yield* new BoxV1FacadeError({
            code: 'permission_denied',
            status: 403,
            message: 'sandbox generation has no active agent-turn capability',
            retryable: false,
          })
        }
        const providerCapabilityToken =
          yield* mintManagedSandboxProviderCapability(env, {
            actorRef: input.principal.actorRef,
            ownerRef: input.principal.ownerRef,
            tenantRef: input.principal.tenantRef,
            sandboxRef: input.resource.sandboxRef,
            turnRef: input.turn.turnRef,
            resourceGeneration: input.resource.resourceGeneration,
            capabilityRef: capability.capabilityRef,
            capabilityExpiresAt: capability.expiresAt,
            provider: input.turn.runtime.provider,
            requestedModelRef: input.turn.runtime.modelRef,
          })
        const response = yield* managedSandboxRuntimeRequest(env, {
          action: 'dispatch',
          ...runtimeScope(input),
          prompt: input.prompt,
          providerCapabilityToken,
          providerModel: managedSandboxProviderModel(
            env,
            input.turn.runtime.provider,
            input.turn.runtime.modelRef,
          ),
        })
        return response.events
      }),
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
    readFile: input =>
      managedSandboxGuestIoRequest(env, {
        action: 'read_file',
        ...guestIoScope(input),
        path: input.path,
        encoding: input.encoding,
      }).pipe(
        Effect.flatMap(response =>
          exactGuestIoResponse(response, 'read_file', input),
        ),
        Effect.map(response => ({
          content: response.content,
          size: response.byteLength,
          receipt: response.receipt,
        })),
      ),
    writeFile: input =>
      Effect.gen(function* () {
        const bytes = yield* guestIoBytes(input.encoding, input.content)
        const contentDigest = yield* guestIoDigest(bytes)
        const response = yield* managedSandboxGuestIoRequest(env, {
          action: 'write_file',
          ...guestIoScope(input),
          path: input.path,
          encoding: input.encoding,
          content: input.content,
          contentDigest,
        })
        const exact = yield* exactGuestIoResponse(response, 'write_file', input)
        if (
          exact.contentDigest !== contentDigest ||
          exact.byteLength !== bytes.byteLength
        ) {
          return yield* new BoxV1FacadeError({
            code: 'conflict',
            status: 409,
            message: 'guest file write receipt does not match request bytes',
            retryable: false,
          })
        }
        return {
          size: exact.byteLength,
          receipt: exact.receipt,
        }
      }),
    command: input =>
      Effect.gen(function* () {
        const commandDigest = yield* guestIoDigest(
          new TextEncoder().encode(input.command),
        )
        const response = yield* managedSandboxGuestIoRequest(env, {
          action: 'execute_command',
          ...guestIoScope(input),
          command: input.command,
          commandDigest,
          cwd: input.cwd,
          timeoutMillis: input.timeoutSeconds * 1_000,
        })
        const exact = yield* exactGuestIoResponse(
          response,
          'execute_command',
          input,
        )
        return {
          success: exact.success,
          exitCode: exact.exitCode,
          signal: exact.signal,
          stdout: exact.stdout,
          stderr: exact.stderr,
          stdoutTruncated: exact.stdoutTruncated,
          stderrTruncated: exact.stderrTruncated,
          timedOut: exact.timedOut,
          startedAt: exact.receipt.startedAt,
          finishedAt: exact.receipt.finishedAt,
          receipt: exact.receipt,
        }
      }),
    artifact: input =>
      Effect.gen(function* () {
        const response = yield* managedSandboxGuestIoRequest(env, {
          action: 'read_artifact',
          ...guestIoScope(input),
          path: input.path,
          retentionUntil: input.retentionUntil,
        })
        const exact = yield* exactGuestIoResponse(
          response,
          'read_artifact',
          input,
        )
        const bytes = yield* guestIoBytes('base64', exact.contentBase64)
        const contentDigest = yield* guestIoDigest(bytes)
        if (
          exact.artifact.contentDigest !== contentDigest ||
          exact.artifact.artifactRef !==
            `artifact.sha256.${contentDigest.slice('sha256:'.length)}` ||
          exact.artifact.byteLength !== bytes.byteLength ||
          exact.artifact.sourceGeneration !==
            input.resource.resourceGeneration ||
          exact.artifact.retentionUntil !== input.retentionUntil
        ) {
          return yield* new BoxV1FacadeError({
            code: 'conflict',
            status: 409,
            message: 'guest artifact receipt does not match returned bytes',
            retryable: false,
          })
        }
        return {
          bytes,
          contentType: exact.artifact.contentType,
          receipt: exact.receipt,
          artifact: exact.artifact,
        }
      }),
  })

import type {
  ManagedSandboxCommandReservation,
  ManagedSandboxEventPage,
  ManagedSandboxProjectionState,
  ManagedSandboxTurnOrder,
} from '@openagentsinc/khala-sync-server'
import {
  BOX_V1_TRANSLATOR_REF,
  type ManagedSandboxCommand,
  type ManagedSandboxEvent,
  ManagedSandboxEventSchema,
  type ManagedSandboxResource,
  ManagedSandboxResourceSchema,
} from '@openagentsinc/managed-sandbox-contract'
import { Effect, Schema as S } from 'effect'

import {
  BoxV1FacadeError,
  type BoxV1NativeStore,
  type BoxV1Policy,
  type BoxV1Principal,
  type BoxV1Runtime,
} from './managed-sandbox-box-v1-routes'

const notFound = () =>
  new BoxV1FacadeError({
    code: 'resource_not_found',
    status: 404,
    message: 'managed sandbox does not exist',
    retryable: false,
  })

const permissionDenied = () =>
  new BoxV1FacadeError({
    code: 'permission_denied',
    status: 403,
    message: 'managed sandbox belongs to another owner or tenant',
    retryable: false,
  })

const conflict = (message: string) =>
  new BoxV1FacadeError({
    code: 'conflict',
    status: 409,
    message,
    retryable: false,
  })

const decodeResource = S.decodeUnknownSync(ManagedSandboxResourceSchema)
const decodeEvent = S.decodeUnknownSync(ManagedSandboxEventSchema)

type StoredReservation = Readonly<{
  fingerprint: string
  reservation: ManagedSandboxCommandReservation
}>

export class BoxV1MemoryAuthority implements BoxV1NativeStore {
  readonly resources = new Map<string, ManagedSandboxResource>()
  readonly commands = new Map<string, StoredReservation>()
  readonly eventsBySandbox = new Map<string, Array<ManagedSandboxEvent>>()
  readonly turnsBySandbox = new Map<string, Array<ManagedSandboxTurnOrder>>()
  readonly projections = new Map<string, ManagedSandboxProjectionState>()

  reservation = (input: {
    ownerRef: string
    tenantRef: string
    commandRef: string
  }): Effect.Effect<
    ManagedSandboxCommandReservation | undefined,
    BoxV1FacadeError
  > => {
    const stored = this.commands.get(input.commandRef)?.reservation
    if (stored === undefined) return Effect.sync(() => undefined)
    return stored.command.ownerRef === input.ownerRef &&
      stored.command.tenantRef === input.tenantRef
      ? Effect.succeed({ ...stored, disposition: 'replayed' })
      : Effect.fail(permissionDenied())
  }

  private scoped(input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
  }): Effect.Effect<ManagedSandboxResource, BoxV1FacadeError> {
    const resource = this.resources.get(input.sandboxRef)
    if (resource === undefined) return Effect.fail(notFound())
    return resource.ownerRef === input.ownerRef &&
      resource.tenantRef === input.tenantRef
      ? Effect.succeed(resource)
      : Effect.fail(permissionDenied())
  }

  private append(
    resource: ManagedSandboxResource,
    inputs: ReadonlyArray<
      Readonly<{
        _tag: ManagedSandboxEvent['_tag']
        turnRef?: string
        checkpointDigest?: string
      }>
    >,
    observedAt: string,
  ): { resource: ManagedSandboxResource; events: Array<ManagedSandboxEvent> } {
    const existing = this.eventsBySandbox.get(resource.sandboxRef) ?? []
    const appended = inputs.map((input, offset) =>
      decodeEvent({
        _tag: input._tag,
        schema: 'openagents.managed_sandbox_event.v1',
        eventRef: `event.box.${resource.sandboxRef}.${resource.lastEventSequence + offset + 1}`,
        sandboxRef: resource.sandboxRef,
        resourceGeneration: resource.resourceGeneration,
        sequence: resource.lastEventSequence + offset + 1,
        observedAt,
        ...(input.turnRef === undefined ? {} : { turnRef: input.turnRef }),
        ...(input.checkpointDigest === undefined
          ? {}
          : { checkpointDigest: input.checkpointDigest }),
      }),
    )
    this.eventsBySandbox.set(resource.sandboxRef, [...existing, ...appended])
    return {
      resource: decodeResource({
        ...resource,
        lastEventSequence: resource.lastEventSequence + appended.length,
        version: resource.version + 1,
        updatedAt: observedAt,
      }),
      events: appended,
    }
  }

  reserve = (input: {
    command: ManagedSandboxCommand
    initialResource?: ManagedSandboxResource
  }): Effect.Effect<ManagedSandboxCommandReservation, BoxV1FacadeError> => {
    const self = this
    return Effect.gen(function* () {
      const fingerprint = JSON.stringify(input)
      const replay = self.commands.get(input.command.commandRef)
      if (replay !== undefined) {
        if (replay.fingerprint !== fingerprint) {
          return yield* conflict('idempotency key is bound to different bytes')
        }
        return { ...replay.reservation, disposition: 'replayed' as const }
      }

      const command = input.command
      let resource: ManagedSandboxResource
      let turnSequence: number | undefined
      if (command._tag === 'Create') {
        if (input.initialResource === undefined) {
          return yield* conflict('create requires an initial resource')
        }
        if (self.resources.has(input.initialResource.sandboxRef)) {
          return yield* conflict('sandbox ref already exists')
        }
        const appended = self.append(
          input.initialResource,
          [{ _tag: 'ProvisionRequested' }, { _tag: 'GuestReady' }],
          command.requestedAt,
        )
        resource = decodeResource({
          ...appended.resource,
          facts: {
            lifecycle: 'ready',
            leaseState: 'active',
            guestState: 'present',
            filesystemState: 'attached',
            ingressState: 'broker_only',
            runtimeState: 'none',
            acceptingWork: true,
            cleanupComplete: false,
          },
        })
      } else {
        resource = yield* self.scoped({
          ownerRef: command.ownerRef,
          tenantRef: command.tenantRef,
          sandboxRef: command.sandboxRef,
        })
        if (
          'expectedVersion' in command &&
          command.expectedVersion !== resource.version
        ) {
          return yield* conflict('managed sandbox version is stale')
        }
        switch (command._tag) {
          case 'Inspect':
            break
          case 'Update':
            resource = decodeResource({
              ...resource,
              version: resource.version + 1,
              ...(command.lease === undefined
                ? {}
                : {
                    lease: command.lease,
                    facts: {
                      ...resource.facts,
                      leaseState: command.lease.state,
                    },
                  }),
              ...(command.budget === undefined
                ? {}
                : { budget: command.budget }),
              updatedAt: command.requestedAt,
            })
            break
          case 'Stop': {
            const appended = self.append(
              resource,
              [
                { _tag: 'StopRequested' },
                {
                  _tag: 'FilesystemCheckpointed',
                  checkpointDigest: `sha256:${'c'.repeat(64)}`,
                },
                { _tag: 'GuestStopped' },
              ],
              command.requestedAt,
            )
            resource = decodeResource({
              ...appended.resource,
              facts: {
                ...resource.facts,
                lifecycle: 'stopped',
                guestState: 'absent',
                filesystemState: 'durable',
                ingressState: 'revoked',
                runtimeState: 'settled',
                acceptingWork: false,
              },
            })
            break
          }
          case 'Resume': {
            const nextGeneration = resource.resourceGeneration + 1
            resource = decodeResource({
              ...resource,
              resourceGeneration: nextGeneration,
            })
            const appended = self.append(
              resource,
              [{ _tag: 'ResumeRequested' }, { _tag: 'GuestReady' }],
              command.requestedAt,
            )
            resource = decodeResource({
              ...appended.resource,
              facts: {
                ...resource.facts,
                lifecycle: 'ready',
                guestState: 'present',
                filesystemState: 'attached',
                ingressState: 'broker_only',
                runtimeState: 'none',
                acceptingWork: true,
              },
            })
            break
          }
          case 'Delete': {
            const appended = self.append(
              resource,
              [{ _tag: 'DeleteRequested' }, { _tag: 'CleanupObserved' }],
              command.requestedAt,
            )
            resource = decodeResource({
              ...appended.resource,
              facts: {
                ...resource.facts,
                lifecycle: 'deleted',
                leaseState: 'released',
                guestState: 'absent',
                filesystemState: 'deleted',
                ingressState: 'revoked',
                runtimeState: 'settled',
                acceptingWork: false,
                cleanupComplete: true,
              },
            })
            break
          }
          case 'Dispatch': {
            if (!resource.facts.acceptingWork) {
              return yield* conflict('sandbox is not accepting work')
            }
            const turns = self.turnsBySandbox.get(resource.sandboxRef) ?? []
            turnSequence = turns.length + 1
            self.turnsBySandbox.set(resource.sandboxRef, [
              ...turns,
              { turnSequence, turnRef: command.turnRef, status: 'running' },
            ])
            const appended = self.append(
              resource,
              [{ _tag: 'RuntimeStarted', turnRef: command.turnRef }],
              command.requestedAt,
            )
            resource = decodeResource({
              ...appended.resource,
              facts: {
                ...resource.facts,
                lifecycle: 'running',
                runtimeState: 'running',
              },
            })
            break
          }
          case 'Interrupt': {
            const turns = self.turnsBySandbox.get(resource.sandboxRef) ?? []
            self.turnsBySandbox.set(
              resource.sandboxRef,
              turns.map(turn =>
                turn.turnRef === command.turnRef
                  ? { ...turn, status: 'interrupted' }
                  : turn,
              ),
            )
            const appended = self.append(
              resource,
              [{ _tag: 'RuntimeSettled', turnRef: command.turnRef }],
              command.requestedAt,
            )
            resource = decodeResource({
              ...appended.resource,
              facts: {
                ...resource.facts,
                lifecycle: 'idle',
                runtimeState: 'settled',
              },
            })
            break
          }
        }
      }

      self.resources.set(resource.sandboxRef, resource)
      const reservation: ManagedSandboxCommandReservation = {
        disposition:
          command._tag === 'Update' || command._tag === 'Inspect'
            ? 'settled'
            : 'reserved',
        status:
          command._tag === 'Update' || command._tag === 'Inspect'
            ? 'settled'
            : 'pending',
        command,
        resource,
        ...(turnSequence === undefined ? {} : { turnSequence }),
      }
      self.commands.set(command.commandRef, { fingerprint, reservation })
      return reservation
    })
  }

  inspect = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
  }): Effect.Effect<ManagedSandboxResource, BoxV1FacadeError> =>
    this.scoped(input)

  list = (input: {
    ownerRef: string
    tenantRef: string
    limit?: number
  }): Effect.Effect<ReadonlyArray<ManagedSandboxResource>, BoxV1FacadeError> =>
    Effect.succeed(
      [...this.resources.values()]
        .filter(
          resource =>
            resource.ownerRef === input.ownerRef &&
            resource.tenantRef === input.tenantRef,
        )
        .sort(
          (left, right) =>
            right.updatedAt.localeCompare(left.updatedAt) ||
            left.sandboxRef.localeCompare(right.sandboxRef),
        )
        .slice(0, input.limit ?? 100),
    )

  readEvents = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    afterSequence: number
    limit: number
  }): Effect.Effect<ManagedSandboxEventPage, BoxV1FacadeError> =>
    this.scoped(input).pipe(
      Effect.map(resource => {
        const events = (this.eventsBySandbox.get(input.sandboxRef) ?? [])
          .filter(event => event.sequence > input.afterSequence)
          .slice(0, input.limit)
        return {
          sandboxRef: input.sandboxRef,
          afterSequence: input.afterSequence,
          nextSequence: events.at(-1)?.sequence ?? input.afterSequence,
          terminalSequence: resource.lastEventSequence,
          events,
        }
      }),
    )

  turns = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
  }): Effect.Effect<ReadonlyArray<ManagedSandboxTurnOrder>, BoxV1FacadeError> =>
    this.scoped(input).pipe(
      Effect.map(() => this.turnsBySandbox.get(input.sandboxRef) ?? []),
    )

  readProjection = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    translatorRef: string
  }): Effect.Effect<
    ManagedSandboxProjectionState | undefined,
    BoxV1FacadeError
  > =>
    this.scoped(input).pipe(
      Effect.map(() =>
        this.projections.get(`${input.sandboxRef}|${input.translatorRef}`),
      ),
    )

  advanceProjection = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    expectedProjectionVersion: number
    cursor: ManagedSandboxProjectionState['cursor']
    observedAt: string
  }): Effect.Effect<ManagedSandboxProjectionState, BoxV1FacadeError> =>
    this.scoped(input).pipe(
      Effect.flatMap(resource => {
        const key = `${input.sandboxRef}|${input.cursor.translatorRef}`
        const current = this.projections.get(key)
        if (
          (current?.projectionVersion ?? 0) !== input.expectedProjectionVersion
        ) {
          return Effect.fail(conflict('projection cursor version is stale'))
        }
        if (input.cursor.nativeEventSequence > resource.lastEventSequence) {
          return Effect.fail(
            conflict('projection cursor exceeds native authority'),
          )
        }
        const next = {
          projectionVersion: input.expectedProjectionVersion + 1,
          cursor: input.cursor,
        }
        this.projections.set(key, next)
        return Effect.succeed(next)
      }),
    )
}

export const boxV1TestPolicy: BoxV1Policy = {
  target: {
    targetRef: 'target.gcp.sbx.staging',
    targetClass: 'openagents_managed',
    provider: 'google_cloud',
    adapterRef: 'adapter.gce.staging.v1',
    region: 'us-central1',
    isolation: 'gce_vm',
    dataPosture: 'openagents_managed_region',
  },
  imageDigest: `sha256:${'a'.repeat(64)}`,
  profileRef: 'profile.sbx.gce.staging.v1',
  defaultTtlSeconds: 3_600,
  maxTtlSeconds: 86_400,
  maxActiveBoxes: 4,
  maxCostMicros: 10_000,
  maxCpuMillis: 86_400_000,
  maxNetworkBytes: 100_000_000,
  maxArtifactBytes: 10_000_000,
}

export const boxV1TestPrincipal: BoxV1Principal = {
  actorRef: 'agent:test-box-sdk',
  ownerRef: 'owner.box-sdk',
  tenantRef: 'tenant.box-sdk',
  login: 'openagents-box-sdk',
  email: 'box-sdk@openagents.test',
}

export const makeBoxV1MemoryRuntime = (): BoxV1Runtime => {
  const files = new Map<string, string>()
  files.set('workspace/README.md', 'OpenAgents staging sandbox')
  return {
    admit: () => Effect.sync(() => undefined),
    readFile: input => {
      const content = files.get(input.path)
      return content === undefined
        ? Effect.fail(notFound())
        : Effect.succeed({
            content,
            size: new TextEncoder().encode(content).length,
          })
    },
    writeFile: input =>
      Effect.sync(() => {
        files.set(input.path, input.content)
        return { size: new TextEncoder().encode(input.content).length }
      }),
    command: input =>
      Effect.succeed({
        success: true,
        exitCode: 0,
        signal: null,
        stdout: input.command === 'pwd' ? `${input.cwd}\n` : 'ok\n',
        stderr: '',
        stdoutTruncated: false,
        stderrTruncated: false,
        timedOut: false,
        startedAt: '2026-07-19T18:30:00.000Z',
        finishedAt: '2026-07-19T18:30:01.000Z',
      }),
    artifact: input => {
      const content = files.get(input.path)
      return content === undefined
        ? Effect.fail(notFound())
        : Effect.succeed({
            bytes: new TextEncoder().encode(content),
            contentType: 'text/plain; charset=utf-8',
          })
    },
  }
}

export const BOX_V1_TEST_TRANSLATOR_REF = BOX_V1_TRANSLATOR_REF

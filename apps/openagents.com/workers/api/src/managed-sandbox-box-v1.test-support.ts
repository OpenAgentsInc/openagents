import type {
  ManagedSandboxCommandReservation,
  ManagedSandboxEventPage,
  ManagedSandboxProjectionState,
  ManagedSandboxRuntimeEventPage,
  ManagedSandboxTurnOrder,
  RecordManagedSandboxRuntimeEventsResult,
} from '@openagentsinc/khala-sync-server'
import {
  BOX_V1_TRANSLATOR_REF,
  type ManagedSandboxCommand,
  type ManagedSandboxEvent,
  ManagedSandboxEventSchema,
  type ManagedSandboxGuestIoReceipt,
  type ManagedSandboxReceipt,
  ManagedSandboxReceiptSchema,
  type ManagedSandboxResource,
  ManagedSandboxResourceSchema,
  type ManagedSandboxRuntimeEventInput,
  type ManagedSandboxTurn,
  type ManagedSandboxTurnReceipt,
  ManagedSandboxTurnSchema,
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
const decodeReceipt = S.decodeUnknownSync(ManagedSandboxReceiptSchema)
const decodeTurn = S.decodeUnknownSync(ManagedSandboxTurnSchema)

type StoredReservation = Readonly<{
  fingerprint: string
  reservation: ManagedSandboxCommandReservation
}>

export class BoxV1MemoryAuthority implements BoxV1NativeStore {
  readonly resources = new Map<string, ManagedSandboxResource>()
  readonly commands = new Map<string, StoredReservation>()
  readonly eventsBySandbox = new Map<string, Array<ManagedSandboxEvent>>()
  readonly turnsBySandbox = new Map<string, Array<ManagedSandboxTurnOrder>>()
  readonly turnDetails = new Map<string, ManagedSandboxTurn>()
  readonly turnReceipts = new Map<string, ManagedSandboxTurnReceipt>()
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
      }> &
        Readonly<Record<string, unknown>>
    >,
    observedAt: string,
  ): { resource: ManagedSandboxResource; events: Array<ManagedSandboxEvent> } {
    const existing = this.eventsBySandbox.get(resource.sandboxRef) ?? []
    const appended = inputs.map((input, offset) =>
      decodeEvent({
        ...input,
        _tag: input._tag,
        schema: 'openagents.managed_sandbox_event.v1',
        eventRef: `event.box.${resource.sandboxRef}.${resource.lastEventSequence + offset + 1}`,
        sandboxRef: resource.sandboxRef,
        resourceGeneration: resource.resourceGeneration,
        sequence: resource.lastEventSequence + offset + 1,
        observedAt,
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

  private applyRuntimeEvents(
    resource: ManagedSandboxResource,
    current: ManagedSandboxTurn,
    events: ReadonlyArray<ManagedSandboxEvent>,
  ): { resource: ManagedSandboxResource; turn: ManagedSandboxTurn } {
    let turn = current
    let lifecycle = resource.facts.lifecycle
    let runtimeState = resource.facts.runtimeState
    let acceptingWork = resource.facts.acceptingWork
    for (const event of events) {
      if (
        !('turnRef' in event) ||
        !('turnEventSequence' in event) ||
        event.turnRef !== turn.turnRef ||
        event.resourceGeneration !== turn.resourceGeneration ||
        event.turnEventSequence !== turn.lastEventSequence + 1
      ) {
        throw conflict('runtime event does not bind the exact dense turn')
      }
      switch (event._tag) {
        case 'RuntimeStarted':
          turn = decodeTurn({
            ...turn,
            status: 'running',
            startedAt: event.observedAt,
            lastEventSequence: event.turnEventSequence,
          })
          lifecycle = 'running'
          runtimeState = 'running'
          break
        case 'RuntimeTextDelta':
        case 'RuntimeToolStarted':
        case 'RuntimeToolCompleted':
          turn = decodeTurn({
            ...turn,
            lastEventSequence: event.turnEventSequence,
          })
          break
        case 'RuntimeUsageRecorded':
          turn = decodeTurn({
            ...turn,
            usage: event.usage,
            lastEventSequence: event.turnEventSequence,
          })
          break
        case 'RuntimeInterruptRequested':
          turn = decodeTurn({
            ...turn,
            status: 'interrupting',
            lastEventSequence: event.turnEventSequence,
          })
          runtimeState = 'interrupting'
          break
        case 'RuntimeSettled':
          turn = decodeTurn({
            ...turn,
            status: 'settled',
            settledAt: event.observedAt,
            terminalReason: event.finishReason,
            ...(event.usage === undefined ? {} : { usage: event.usage }),
            lastEventSequence: event.turnEventSequence,
          })
          lifecycle = 'idle'
          runtimeState = 'settled'
          break
        case 'RuntimeFailed':
          turn = decodeTurn({
            ...turn,
            status: 'failed',
            settledAt: event.observedAt,
            terminalReason: 'provider_failure',
            lastEventSequence: event.turnEventSequence,
          })
          lifecycle = 'failed'
          runtimeState = 'failed'
          acceptingWork = false
          break
        case 'RuntimeInterrupted':
          turn = decodeTurn({
            ...turn,
            status: 'interrupted',
            settledAt: event.observedAt,
            terminalReason: 'explicit_stop',
            lastEventSequence: event.turnEventSequence,
          })
          lifecycle = 'idle'
          runtimeState = 'settled'
          break
        default:
          break
      }
    }
    return {
      turn,
      resource: decodeResource({
        ...resource,
        version: resource.version + 1,
        lastEventSequence:
          events.at(-1)?.sequence ?? resource.lastEventSequence,
        facts: {
          ...resource.facts,
          lifecycle,
          runtimeState,
          acceptingWork,
        },
        updatedAt: events.at(-1)?.observedAt ?? resource.updatedAt,
      }),
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
              ...(command.capabilities === undefined
                ? {}
                : { capabilities: command.capabilities }),
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
              { turnSequence, turnRef: command.turnRef, status: 'pending' },
            ])
            self.turnDetails.set(
              command.turnRef,
              decodeTurn({
                schema: 'openagents.managed_sandbox_turn.v1',
                turnRef: command.turnRef,
                sandboxRef: resource.sandboxRef,
                ownerRef: resource.ownerRef,
                tenantRef: resource.tenantRef,
                workUnitRef: resource.workUnitRef,
                attachmentRef: resource.attachmentRef,
                attachmentGeneration: resource.attachmentGeneration,
                resourceGeneration: resource.resourceGeneration,
                turnSequence,
                lastEventSequence: 0,
                commandRef: command.commandRef,
                capabilityRef: command.capabilityRef,
                promptDigest: command.promptDigest,
                runtime: command.runtime,
                status: 'pending',
                createdAt: command.requestedAt,
              }),
            )
            resource = decodeResource({
              ...resource,
              version: resource.version + 1,
              updatedAt: command.requestedAt,
            })
            break
          }
          case 'Interrupt': {
            resource = decodeResource({
              ...resource,
              version: resource.version + 1,
              updatedAt: command.requestedAt,
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

  settle = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    commandRef: string
    expectedResourceGeneration: number
    events: ReadonlyArray<ManagedSandboxEvent>
    outcome: 'succeeded' | 'failed' | 'refused'
    artifactRefs?: ReadonlyArray<string>
    errorCode?: string
    observedAt: string
  }): Effect.Effect<ManagedSandboxReceipt, BoxV1FacadeError> => {
    const self = this
    return Effect.gen(function* () {
      const stored = self.commands.get(input.commandRef)
      if (stored === undefined) return yield* notFound()
      let resource = yield* self.scoped(input)
      if (resource.resourceGeneration !== input.expectedResourceGeneration) {
        return yield* conflict('managed sandbox generation is stale')
      }
      const turnRef =
        stored.reservation.command._tag === 'Dispatch' ||
        stored.reservation.command._tag === 'Interrupt'
          ? stored.reservation.command.turnRef
          : undefined
      if (turnRef === undefined) {
        return yield* conflict('test settlement supports runtime commands only')
      }
      const currentTurn = self.turnDetails.get(turnRef)
      if (currentTurn === undefined) return yield* notFound()
      const applied = self.applyRuntimeEvents(
        resource,
        currentTurn,
        input.events,
      )
      resource = applied.resource
      self.resources.set(resource.sandboxRef, resource)
      self.eventsBySandbox.set(resource.sandboxRef, [
        ...(self.eventsBySandbox.get(resource.sandboxRef) ?? []),
        ...input.events,
      ])
      self.turnDetails.set(turnRef, applied.turn)
      self.turnsBySandbox.set(
        resource.sandboxRef,
        (self.turnsBySandbox.get(resource.sandboxRef) ?? []).map(turn =>
          turn.turnRef === turnRef
            ? { ...turn, status: applied.turn.status }
            : turn,
        ),
      )
      const receipt = decodeReceipt({
        schema: 'openagents.managed_sandbox_receipt.v1',
        receiptRef: `receipt.box.${input.commandRef}`,
        commandRef: input.commandRef,
        sandboxRef: resource.sandboxRef,
        ownerRef: resource.ownerRef,
        tenantRef: resource.tenantRef,
        resourceGeneration: resource.resourceGeneration,
        version: resource.version,
        outcome: input.outcome,
        lifecycle: resource.facts.lifecycle,
        eventRefs: input.events.map(event => event.eventRef),
        artifactRefs: input.artifactRefs ?? [],
        ...(input.errorCode === undefined
          ? {}
          : { errorCode: input.errorCode }),
        observedAt: input.observedAt,
      })
      self.commands.set(input.commandRef, {
        ...stored,
        reservation: {
          ...stored.reservation,
          status: 'settled',
          resource,
          receipt,
        },
      })
      return receipt
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

  inspectTurn = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    turnRef: string
  }): Effect.Effect<
    { turn: ManagedSandboxTurn; receipt?: ManagedSandboxTurnReceipt },
    BoxV1FacadeError
  > =>
    this.scoped(input).pipe(
      Effect.flatMap(() => {
        const turn = this.turnDetails.get(input.turnRef)
        if (turn === undefined || turn.sandboxRef !== input.sandboxRef) {
          return Effect.fail(notFound())
        }
        const receipt = this.turnReceipts.get(input.turnRef)
        return Effect.succeed(
          receipt === undefined ? { turn } : { turn, receipt },
        )
      }),
    )

  readTurnEvents = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    turnRef: string
    afterTurnSequence: number
    limit: number
  }): Effect.Effect<ManagedSandboxRuntimeEventPage, BoxV1FacadeError> =>
    this.inspectTurn(input).pipe(
      Effect.map(({ turn }) => {
        const events = (this.eventsBySandbox.get(input.sandboxRef) ?? [])
          .filter(
            event =>
              'turnRef' in event &&
              event.turnRef === input.turnRef &&
              'turnEventSequence' in event &&
              event.turnEventSequence > input.afterTurnSequence,
          )
          .slice(0, input.limit)
        const last = events.at(-1)
        return {
          turn,
          events,
          afterTurnSequence: input.afterTurnSequence,
          nextTurnSequence:
            last !== undefined && 'turnEventSequence' in last
              ? last.turnEventSequence
              : input.afterTurnSequence,
          terminalTurnSequence: turn.lastEventSequence,
        }
      }),
    )

  recordRuntimeEvents = (input: {
    ownerRef: string
    tenantRef: string
    sandboxRef: string
    turnRef: string
    expectedResourceGeneration: number
    events: ReadonlyArray<ManagedSandboxRuntimeEventInput>
    evidenceRefs?: ReadonlyArray<string>
  }): Effect.Effect<
    RecordManagedSandboxRuntimeEventsResult,
    BoxV1FacadeError
  > => {
    const self = this
    return Effect.gen(function* () {
      let resource = yield* self.scoped(input)
      let turn = self.turnDetails.get(input.turnRef)
      if (turn === undefined) return yield* notFound()
      if (
        resource.resourceGeneration !== input.expectedResourceGeneration ||
        turn.resourceGeneration !== input.expectedResourceGeneration
      ) {
        return yield* conflict('managed sandbox generation is stale')
      }
      const appended: Array<ManagedSandboxEvent> = []
      for (const providerEvent of input.events) {
        if (providerEvent.turnEventSequence <= turn.lastEventSequence) {
          const existing = (
            self.eventsBySandbox.get(input.sandboxRef) ?? []
          ).find(
            event =>
              'turnRef' in event &&
              event.turnRef === input.turnRef &&
              'turnEventSequence' in event &&
              event.turnEventSequence === providerEvent.turnEventSequence,
          )
          if (existing === undefined) {
            return yield* conflict('runtime event replay is missing')
          }
          continue
        }
        const event = decodeEvent({
          ...providerEvent,
          schema: 'openagents.managed_sandbox_event.v1',
          eventRef: `event.box.runtime.${providerEvent.turnEventSequence}.${input.turnRef}`,
          sandboxRef: input.sandboxRef,
          sequence: resource.lastEventSequence + appended.length + 1,
        })
        const applied = self.applyRuntimeEvents(resource, turn, [event])
        resource = decodeResource({
          ...applied.resource,
          version: resource.version,
        })
        turn = applied.turn
        appended.push(event)
      }
      if (appended.length > 0) {
        resource = decodeResource({
          ...resource,
          version: resource.version + 1,
        })
        self.resources.set(input.sandboxRef, resource)
        self.eventsBySandbox.set(input.sandboxRef, [
          ...(self.eventsBySandbox.get(input.sandboxRef) ?? []),
          ...appended,
        ])
        self.turnDetails.set(input.turnRef, turn)
        self.turnsBySandbox.set(
          input.sandboxRef,
          (self.turnsBySandbox.get(input.sandboxRef) ?? []).map(order =>
            order.turnRef === input.turnRef
              ? { ...order, status: turn.status }
              : order,
          ),
        )
      }
      const receipt = self.turnReceipts.get(input.turnRef)
      return receipt === undefined
        ? { turn, events: appended }
        : { turn, receipt, events: appended }
    })
  }

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
  const guestIoReceipt = (
    input: {
      operationRef: string
      resource: ManagedSandboxResource
      capabilityRef: string
      path?: string
      cwd?: string
    },
    action: ManagedSandboxGuestIoReceipt['action'],
    bytesRead: number,
    bytesWritten: number,
  ): ManagedSandboxGuestIoReceipt => ({
    schemaVersion: 'openagents.managed_sandbox_guest_io_receipt.v1',
    receiptRef: `receipt.${input.operationRef}`,
    operationRef: input.operationRef,
    sandboxRef: input.resource.sandboxRef,
    resourceGeneration: input.resource.resourceGeneration,
    capabilityRef: input.capabilityRef,
    action,
    outcome: 'succeeded',
    pathDigest: `sha256:${'c'.repeat(64)}`,
    startedAt: '2026-07-19T18:30:00.000Z',
    finishedAt: '2026-07-19T18:30:01.000Z',
    bytesRead,
    bytesWritten,
    cpuMillis: 1,
    networkBytes: 0,
    ...(action === 'execute_command'
      ? { processRef: `process.${input.operationRef}` }
      : {}),
    processTerminated: true,
    descendantsRemaining: 0,
    scratchCleaned: true,
    ingressClosed: true,
    egressDenied: true,
    pathPolicy: 'resolved_beneath_workspace_root',
    symlinkTraversal: false,
    secretScan: 'clean',
    evidenceRefs: [`evidence.${input.operationRef}`],
  })
  return {
    dispatch: input =>
      Effect.succeed([
        {
          _tag: 'RuntimeStarted' as const,
          turnRef: input.turn.turnRef,
          resourceGeneration: input.turn.resourceGeneration,
          turnEventSequence: input.turn.lastEventSequence + 1,
          observedAt: '2026-07-19T18:30:00.000Z',
        },
      ]),
    sync: () => Effect.succeed([]),
    interrupt: input =>
      Effect.succeed([
        {
          _tag: 'RuntimeInterruptRequested' as const,
          turnRef: input.turn.turnRef,
          resourceGeneration: input.turn.resourceGeneration,
          turnEventSequence: input.turn.lastEventSequence + 1,
          reasonRef: input.reasonRef,
          observedAt: '2026-07-19T18:30:00.000Z',
        },
        {
          _tag: 'RuntimeInterrupted' as const,
          turnRef: input.turn.turnRef,
          resourceGeneration: input.turn.resourceGeneration,
          turnEventSequence: input.turn.lastEventSequence + 2,
          reasonRef: input.reasonRef,
          observedAt: '2026-07-19T18:30:01.000Z',
        },
      ]),
    readFile: input => {
      const content = files.get(input.path)
      return content === undefined
        ? Effect.fail(notFound())
        : Effect.succeed({
            content,
            size: new TextEncoder().encode(content).length,
            receipt: guestIoReceipt(
              input,
              'read_file',
              new TextEncoder().encode(content).length,
              0,
            ),
          })
    },
    writeFile: input =>
      Effect.sync(() => {
        files.set(input.path, input.content)
        const size = new TextEncoder().encode(input.content).length
        return {
          size,
          receipt: guestIoReceipt(input, 'write_file', 0, size),
        }
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
        receipt: guestIoReceipt(
          input,
          'execute_command',
          0,
          input.command === 'pwd' ? 10 : 3,
        ),
      }),
    artifact: input => {
      const content = files.get(input.path)
      return content === undefined
        ? Effect.fail(notFound())
        : Effect.succeed({
            bytes: new TextEncoder().encode(content),
            contentType: 'text/plain; charset=utf-8',
            receipt: guestIoReceipt(
              input,
              'read_artifact',
              new TextEncoder().encode(content).length,
              0,
            ),
            artifact: {
              schemaVersion: 'openagents.managed_sandbox_artifact_receipt.v1',
              artifactRef: `artifact.sha256.${'d'.repeat(64)}`,
              contentDigest: `sha256:${'d'.repeat(64)}`,
              byteLength: new TextEncoder().encode(content).length,
              sourceGeneration: input.resource.resourceGeneration,
              sourcePathDigest: `sha256:${'c'.repeat(64)}`,
              retentionUntil: input.retentionUntil,
              contentType: 'text/plain; charset=utf-8',
              evidenceRefs: [`evidence.${input.operationRef}`],
            },
          })
    },
  }
}

export const BOX_V1_TEST_TRANSLATOR_REF = BOX_V1_TRANSLATOR_REF

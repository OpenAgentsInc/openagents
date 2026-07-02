import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type AgentRegistrationStore,
  type ProgrammaticAgentSession,
} from './agent-registration'
import {
  publicRefTriggersAgentSecretScanner,
  publicScannerSafeRef,
} from './public-ref-scanner-safety'
import {
  type PylonApiAssignmentRecord,
  type PylonApiEventRecord,
  type PylonApiProviderJobLifecycleRecord,
  type PylonApiQuarantineRecord,
  type PylonApiRegistrationRecord,
  type PylonApiStore,
  PylonApiStoreError,
  type PylonSparkPayoutTargetRecord,
  type PylonSparkPayoutTargetStore,
  buildPylonApiAssignmentRecord,
  buildPylonApiEventRecord,
  buildPylonApiRegistrationRecord,
  makeD1PylonApiStore,
  providerJobLifecycleRecordFromAssignment,
  pylonApiPayloadHasPrivateMaterial,
  pylonClientVersionMeetsMinimum,
  resolveSparkPayoutDestination,
  resolveSparkPayoutTargetReadiness,
} from './pylon-api'
import {
  controlledPylonAssignmentDispatchGate,
  makePylonApiRoutes,
} from './pylon-api-routes'

type PylonRouteJson = Readonly<{
  assignment?: Readonly<{
    acceptedWorkRefs?: ReadonlyArray<string>
    assignmentRef?: string
    closeoutRefs?: ReadonlyArray<string>
    codingAssignment?: Record<string, unknown> | null
    leaseState?: string
    paymentMode?: string
    settlement?: Readonly<{
      paymentReceiptRefs?: ReadonlyArray<string>
      payoutClaimAllowed?: boolean
      realBitcoinMoved?: boolean
      settlementRefs?: ReadonlyArray<string>
      settlementState?: string
      transitionReceiptRefs?: ReadonlyArray<string>
      treasuryReceiptRefs?: ReadonlyArray<string>
    }>
    state?: string
  }>
  assignments?: ReadonlyArray<
    Readonly<{
      acceptedWorkRefs?: ReadonlyArray<string>
      assignmentRef?: string
      closeoutRefs?: ReadonlyArray<string>
      codingAssignment?: Record<string, unknown> | null
      leaseState?: string
      paymentMode?: string
      state?: string
    }>
  >
  dispatchGate?: Readonly<{
    blockerRefs?: ReadonlyArray<string>
    dispatchAllowed?: boolean
    forumAutoPublishAllowed?: boolean
    noSpendDispatch?: boolean
    paymentMode?: string | null
    settlementMutationAllowed?: boolean
    walletSpendAllowed?: boolean
  }>
  error?: string
  event?: Readonly<{
    eventKind?: string
    pylonRef?: string
    status?: string
  }>
  reason?: string
  events?: ReadonlyArray<unknown>
  idempotent?: boolean
  pylon?: Readonly<{
    capabilityRefs?: ReadonlyArray<string>
    codingCapacity?: ReadonlyArray<unknown>
    clientProtocolVersion?: string | null
    clientVersion?: string | null
    createdAtDisplay?: string
    latestCapacityRefs?: ReadonlyArray<string>
    latestHeartbeatDisplay?: string | null
    latestHeartbeatStatus?: string | null
    latestHealthRefs?: ReadonlyArray<string>
    latestLoadRefs?: ReadonlyArray<string>
    latestResourceMode?: string | null
    providerMarketRelayRefs?: ReadonlyArray<string>
    providerNip90LaneRefs?: ReadonlyArray<string>
    providerNostrNpub?: string | null
    providerNostrPubkey?: string | null
    pylonRef?: string
    quarantine?: Readonly<{
      active?: boolean
      quarantineRef?: string
      reasonRefs?: ReadonlyArray<string>
      state?: string
    }> | null
    sparkPayoutTargetReady?: boolean
    sparkPayoutTargetRef?: string | null
    walletReady?: boolean
  }>
  pylons?: ReadonlyArray<
    Readonly<{
      capabilityRefs?: ReadonlyArray<string>
      codingCapacity?: ReadonlyArray<unknown>
      providerMarketRelayRefs?: ReadonlyArray<string>
      providerNip90LaneRefs?: ReadonlyArray<string>
      providerNostrNpub?: string | null
      providerNostrPubkey?: string | null
      pylonRef?: string
      sparkPayoutTargetReady?: boolean
      sparkPayoutTargetRef?: string | null
    }>
  >
  tassadarCapabilityAdmission?: Readonly<{
    refusalRefs?: ReadonlyArray<string>
    selfTestReceiptRefs?: ReadonlyArray<string>
    state?: string
  }>
}>

const activeLeaseAssignmentStates = new Set<PylonApiAssignmentRecord['state']>([
  'accepted',
  'blocked',
  'offered',
  'proof_submitted',
  'running',
])

class MemoryPylonApiStore implements PylonApiStore {
  assignments = new Map<string, PylonApiAssignmentRecord>()
  assignmentsByIdempotency = new Map<string, PylonApiAssignmentRecord>()
  events = new Map<string, PylonApiEventRecord>()
  eventsByIdempotency = new Map<string, PylonApiEventRecord>()
  providerJobLifecycle = new Map<string, PylonApiProviderJobLifecycleRecord>()
  quarantines = new Map<string, PylonApiQuarantineRecord>()
  registrations = new Map<string, PylonApiRegistrationRecord>()

  createAssignment = async (record: PylonApiAssignmentRecord) => {
    const existing = this.assignmentsByIdempotency.get(
      record.idempotencyKeyHash,
    )

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.assignments.set(record.assignmentRef, record)
    this.assignmentsByIdempotency.set(record.idempotencyKeyHash, record)
    this.providerJobLifecycle.set(
      record.assignmentRef,
      providerJobLifecycleRecordFromAssignment(record),
    )

    return { idempotent: false, record }
  }

  createEvent = async (record: PylonApiEventRecord) => {
    const existing = this.eventsByIdempotency.get(record.idempotencyKeyHash)

    if (existing !== undefined) {
      return { idempotent: true, record: existing }
    }

    this.events.set(record.eventRef, record)
    this.eventsByIdempotency.set(record.idempotencyKeyHash, record)

    return { idempotent: false, record }
  }

  listEventsForPylon = async (pylonRef: string, limit: number) =>
    Array.from(this.events.values())
      .filter(event => event.pylonRef === pylonRef)
      .slice(0, limit)

  listEventsForAssignment = async (assignmentRef: string, limit: number) =>
    Array.from(this.events.values())
      .filter(event => event.assignmentRef === assignmentRef)
      .slice(0, limit)

  listAssignmentsForPylon = async (pylonRef: string, limit: number) =>
    Array.from(this.assignments.values())
      .filter(assignment => assignment.pylonRef === pylonRef)
      .filter(assignment => activeLeaseAssignmentStates.has(assignment.state))
      .slice(0, limit)

  sweepStaleAssignmentLeases = async (
    pylonRef: string,
    nowIso: string,
    staleBeforeIso: string,
  ) => {
    const refs: string[] = []
    for (const assignment of this.assignments.values()) {
      if (
        assignment.pylonRef === pylonRef &&
        ['accepted', 'blocked', 'offered', 'proof_submitted', 'running'].includes(
          assignment.state,
        ) &&
        assignment.leaseExpiresAt > nowIso &&
        assignment.updatedAt < staleBeforeIso
      ) {
        const next = {
          ...assignment,
          leaseExpiresAt: nowIso,
          state: 'stale' as const,
          updatedAt: nowIso,
        }
        refs.push(assignment.assignmentRef)
        this.assignments.set(assignment.assignmentRef, next)
        this.assignmentsByIdempotency.set(next.idempotencyKeyHash, next)
        this.providerJobLifecycle.set(
          next.assignmentRef,
          providerJobLifecycleRecordFromAssignment(next),
        )
      }
    }
    return refs
  }

  listRegistrations = async (limit: number) =>
    Array.from(this.registrations.values()).slice(0, limit)

  listRegistrationsForOwnerAgentUserIds = async (
    ownerAgentUserIds: ReadonlyArray<string>,
    limit: number,
  ) =>
    Array.from(this.registrations.values())
      .filter(registration =>
        ownerAgentUserIds.includes(registration.ownerAgentUserId),
      )
      .slice(0, limit)

  listProviderJobLifecycleForPylons = async (
    pylonRefs: ReadonlyArray<string>,
    limit: number,
  ) =>
    Array.from(this.providerJobLifecycle.values())
      .filter(record => pylonRefs.includes(record.pylonRef))
      .slice(0, limit)

  readEventByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.eventsByIdempotency.get(idempotencyKeyHash)

  readAssignment = async (assignmentRef: string) =>
    this.assignments.get(assignmentRef)

  readAssignmentByIdempotencyKeyHash = async (idempotencyKeyHash: string) =>
    this.assignmentsByIdempotency.get(idempotencyKeyHash)

  readRegistration = async (pylonRef: string) =>
    this.registrations.get(pylonRef)

  readActiveQuarantineForPylon = async (pylonRef: string, nowIso: string) =>
    Array.from(this.quarantines.values())
      .filter(record => record.pylonRef === pylonRef)
      .filter(record => record.state === 'active')
      .filter(record => record.releasedAt === null)
      .filter(
        record =>
          record.expiresAt === null || Date.parse(record.expiresAt) > Date.parse(nowIso),
      )
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0]

  updateAssignment = async (record: PylonApiAssignmentRecord) => {
    this.assignments.set(record.assignmentRef, record)
    this.assignmentsByIdempotency.set(record.idempotencyKeyHash, record)
    this.providerJobLifecycle.set(
      record.assignmentRef,
      providerJobLifecycleRecordFromAssignment(record),
    )

    return record
  }

  updateAssignmentIfState = async (
    record: PylonApiAssignmentRecord,
    expectedState: PylonApiAssignmentRecord['state'],
  ) => {
    const current = this.assignments.get(record.assignmentRef)

    if (current === undefined || current.state !== expectedState) {
      return undefined
    }

    return this.updateAssignment(record)
  }

  upsertProviderJobLifecycle = async (
    record: PylonApiProviderJobLifecycleRecord,
  ) => {
    this.providerJobLifecycle.set(record.assignmentRef, record)

    return record
  }

  upsertQuarantine = async (record: PylonApiQuarantineRecord) => {
    const existing = this.quarantines.get(record.quarantineRef)
    const next =
      existing === undefined ? record : { ...record, createdAt: existing.createdAt, id: existing.id }
    this.quarantines.set(record.quarantineRef, next)

    return next
  }

  upsertRegistration = async (
    record: PylonApiRegistrationRecord,
    options?: Readonly<{ allowOwnerTransferFrom?: string | undefined }>,
  ) => {
    const existing = this.registrations.get(record.pylonRef)

    if (
      existing !== undefined &&
      existing.ownerAgentUserId !== record.ownerAgentUserId &&
      options?.allowOwnerTransferFrom !== existing.ownerAgentUserId
    ) {
      throw new PylonApiStoreError({
        kind: 'conflict',
        reason: 'Pylon ref is already owned by another registered agent.',
      })
    }

    const next =
      existing === undefined
        ? record
        : {
            ...record,
            createdAt: existing.createdAt,
            id: existing.id,
          }

    this.registrations.set(record.pylonRef, next)

    return next
  }
}

// #5252: in-memory private Spark payout-target store mirroring the D1 upsert
// semantics (idempotent by pylonRef). The raw address lives only here.
class MemorySparkPayoutTargetStore implements PylonSparkPayoutTargetStore {
  records = new Map<string, PylonSparkPayoutTargetRecord>()

  upsert = async (record: PylonSparkPayoutTargetRecord) => {
    const existing = this.records.get(record.pylonRef)
    const next =
      existing === undefined
        ? record
        : { ...record, createdAt: existing.createdAt }

    this.records.set(record.pylonRef, next)

    return next
  }

  read = async (pylonRef: string) => this.records.get(pylonRef)

  // #5252 owner-scoped fallback: most-recently-updated target for the owner,
  // across ANY of that owner's pylonRefs. Bound to the owning agent only.
  readByOwner = async (ownerAgentUserId: string) =>
    [...this.records.values()]
      .filter(record => record.ownerAgentUserId === ownerAgentUserId)
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))[0]
}

type D1InsertShape = Readonly<{
  bindCount: number
  columnCount: number
  valueCount: number
}>

const d1Result = <T>(results: ReadonlyArray<T> = []): D1Result<T> => ({
  meta: {} as D1Meta & Record<string, unknown>,
  results: [...results],
  success: true,
})

const countCommaSeparatedSqlValues = (value: string): number =>
  value
    .split(',')
    .map(part => part.trim())
    .filter(part => part !== '').length

class RegistrationInsertStatement implements D1PreparedStatement {
  private bindings: ReadonlyArray<unknown> = []

  constructor(
    private readonly db: RegistrationInsertD1,
    private readonly query: string,
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    this.bindings = values

    return this
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM pylon_api_registrations')) {
      return Promise.resolve(null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (this.query.includes('INSERT INTO pylon_api_registrations')) {
      const columnMatch = /\(([^)]+)\)\s*VALUES/s.exec(this.query)
      const valueMatch = /VALUES\s*\(([^)]+)\)/s.exec(this.query)

      this.db.registrationInsertShape = {
        bindCount: this.bindings.length,
        columnCount: countCommaSeparatedSqlValues(columnMatch?.[1] ?? ''),
        valueCount: countCommaSeparatedSqlValues(valueMatch?.[1] ?? ''),
      }

      return Promise.resolve(d1Result<T>())
    }

    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = unknown>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<T[] | [string[], ...T[]]> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

class RegistrationInsertD1 implements D1Database {
  registrationInsertShape: D1InsertShape | null = null

  batch<T = unknown>(): Promise<Array<D1Result<T>>> {
    return Promise.reject(new Error('D1 batch should not be used'))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.reject(new Error('D1 dump should not be used'))
  }

  exec(): Promise<D1ExecResult> {
    return Promise.reject(new Error('D1 exec should not be used'))
  }

  prepare(query: string): D1PreparedStatement {
    return new RegistrationInsertStatement(this, query)
  }

  withSession(): D1DatabaseSession {
    throw new Error('D1 session should not be used')
  }
}

class AssignmentBatchStatement implements D1PreparedStatement {
  readonly bindings: ReadonlyArray<unknown> = []

  constructor(readonly query: string) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    return Object.assign(new AssignmentBatchStatement(this.query), {
      bindings: values,
    })
  }

  first<T = unknown>(): Promise<T | null> {
    if (this.query.includes('FROM pylon_api_assignments')) {
      return Promise.resolve(null)
    }

    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(
      new Error('D1 run should not be used for assignment lifecycle writes'),
    )
  }

  all<T = unknown>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<T[] | [string[], ...T[]]> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

class AssignmentBatchD1 implements D1Database {
  batchQueries: string[] = []
  failBatch = false

  batch<T = unknown>(
    statements: D1PreparedStatement[],
  ): Promise<Array<D1Result<T>>> {
    this.batchQueries = statements.map(
      statement => (statement as AssignmentBatchStatement).query,
    )

    if (this.failBatch) {
      return Promise.reject(new Error('simulated mid-batch failure'))
    }

    return Promise.resolve(statements.map(() => d1Result<T>()))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.reject(new Error('D1 dump should not be used'))
  }

  exec(): Promise<D1ExecResult> {
    return Promise.reject(new Error('D1 exec should not be used'))
  }

  prepare(query: string): D1PreparedStatement {
    return new AssignmentBatchStatement(query)
  }

  withSession(): D1DatabaseSession {
    throw new Error('D1 session should not be used')
  }
}

type EventRow = Readonly<{
  assignment_ref: string | null
  created_at: string
  event_body_json: string
  event_kind: string
  event_ref: string
  id: string
  idempotency_key_hash: string
  owner_agent_user_id: string
  public_projection_json: string
  pylon_ref: string
  status: string
}>

class EventIdempotentStatement implements D1PreparedStatement {
  constructor(
    private readonly db: EventIdempotentD1,
    private readonly query: string,
    private readonly bindings: ReadonlyArray<unknown> = [],
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    return new EventIdempotentStatement(this.db, this.query, values)
  }

  first<T = unknown>(): Promise<T | null> {
    if (!this.query.includes('FROM pylon_api_events')) {
      return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
    }

    this.db.readCount += 1
    if (this.db.readCount === 1) {
      return Promise.resolve(null)
    }

    const idempotencyKeyHash = String(this.bindings[0] ?? '')
    return Promise.resolve(
      (this.db.events.get(idempotencyKeyHash) ?? null) as T | null,
    )
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    if (!this.query.includes('INSERT INTO pylon_api_events')) {
      return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
    }

    this.db.insertQueries.push(this.query)
    if (!this.query.includes('ON CONFLICT(idempotency_key_hash) DO UPDATE')) {
      return Promise.reject(new Error('event insert is not idempotent'))
    }

    const idempotencyKeyHash = String(this.bindings[4] ?? '')
    if (!this.db.events.has(idempotencyKeyHash)) {
      this.db.events.set(idempotencyKeyHash, {
        assignment_ref:
          typeof this.bindings[6] === 'string' ? this.bindings[6] : null,
        created_at: String(this.bindings[10] ?? ''),
        event_body_json: String(this.bindings[8] ?? '{}'),
        event_kind: String(this.bindings[5] ?? ''),
        event_ref: String(this.bindings[1] ?? ''),
        id: String(this.bindings[0] ?? ''),
        idempotency_key_hash: idempotencyKeyHash,
        owner_agent_user_id: String(this.bindings[3] ?? ''),
        public_projection_json: String(this.bindings[9] ?? '{}'),
        pylon_ref: String(this.bindings[2] ?? ''),
        status: String(this.bindings[7] ?? ''),
      })
    }

    return Promise.resolve(d1Result<T>())
  }

  all<T = unknown>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected D1 all: ${this.query}`))
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<T[] | [string[], ...T[]]> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

class EventIdempotentD1 implements D1Database {
  readonly events = new Map<string, EventRow>()
  readonly insertQueries: string[] = []
  readCount = 0

  constructor(existing: EventRow) {
    this.events.set(existing.idempotency_key_hash, existing)
  }

  batch<T = unknown>(): Promise<Array<D1Result<T>>> {
    return Promise.reject(new Error('D1 batch should not be used'))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.reject(new Error('D1 dump should not be used'))
  }

  exec(): Promise<D1ExecResult> {
    return Promise.reject(new Error('D1 exec should not be used'))
  }

  prepare(query: string): D1PreparedStatement {
    return new EventIdempotentStatement(this, query)
  }

  withSession(): D1DatabaseSession {
    throw new Error('D1 session should not be used')
  }
}

class ChunkedSelectStatement implements D1PreparedStatement {
  constructor(
    private readonly db: ChunkedSelectD1,
    private readonly query: string,
    private readonly bindings: ReadonlyArray<unknown> = [],
  ) {}

  bind(...values: ReadonlyArray<unknown>): D1PreparedStatement {
    return new ChunkedSelectStatement(this.db, this.query, values)
  }

  first<T = unknown>(): Promise<T | null> {
    return Promise.reject(new Error(`Unexpected D1 first: ${this.query}`))
  }

  run<T = Record<string, unknown>>(): Promise<D1Result<T>> {
    return Promise.reject(new Error(`Unexpected D1 run: ${this.query}`))
  }

  all<T = unknown>(): Promise<D1Result<T>> {
    this.db.bindCounts.push(this.bindings.length)
    this.db.queries.push(this.query)

    if (this.bindings.length > 81) {
      return Promise.reject(
        new Error(`D1 select bind count exceeded: ${this.bindings.length}`),
      )
    }

    return Promise.resolve(d1Result<T>())
  }

  raw<T = unknown[]>(options: {
    columnNames: true
  }): Promise<[string[], ...T[]]>
  raw<T = unknown[]>(options?: { columnNames?: false }): Promise<T[]>
  raw<T = unknown[]>(options?: {
    columnNames?: boolean
  }): Promise<T[] | [string[], ...T[]]> {
    return Promise.reject(new Error(`Unexpected D1 raw: ${this.query}`))
  }
}

class ChunkedSelectD1 implements D1Database {
  readonly bindCounts: number[] = []
  readonly queries: string[] = []

  batch<T = unknown>(): Promise<Array<D1Result<T>>> {
    return Promise.reject(new Error('D1 batch should not be used'))
  }

  dump(): Promise<ArrayBuffer> {
    return Promise.reject(new Error('D1 dump should not be used'))
  }

  exec(): Promise<D1ExecResult> {
    return Promise.reject(new Error('D1 exec should not be used'))
  }

  prepare(query: string): D1PreparedStatement {
    return new ChunkedSelectStatement(this, query)
  }

  withSession(): D1DatabaseSession {
    throw new Error('D1 session should not be used')
  }
}

const sessionFor = (userId: string): ProgrammaticAgentSession => ({
  credential: {
    id: `credential-${userId}`,
    lastUsedAt: '2026-06-07T00:00:00.000Z',
    openauthUserId: null,
    profileMetadataJson: '{}',
    tokenPrefix: 'oa_agent_test',
  },
  user: {
    avatarUrl: null,
    createdAt: '2026-06-07T00:00:00.000Z',
    displayName: `Agent ${userId}`,
    id: userId,
    kind: 'agent',
    primaryEmail: null,
    status: 'active',
    updatedAt: '2026-06-07T00:00:00.000Z',
  },
})

const agentStoreFor = (
  userId: string,
  options: Readonly<{
    linkedAgentUserIds?: ReadonlyArray<string>
    openauthUserId?: string | null
  }> = {},
): AgentRegistrationStore => ({
  createAgentRegistration: () => Promise.resolve(),
  findAgentByTokenHash: () =>
    Promise.resolve({
      credentialId: `credential-${userId}`,
      openauthUserId: options.openauthUserId ?? null,
      profileMetadataJson: '{}',
      tokenPrefix: 'oa_agent_test',
      user: sessionFor(userId).user,
    }),
  linkOpenAuthAgent: () => Promise.resolve(),
  listLinkedAgentsForOpenAuthUser: () =>
    Promise.resolve(
      (options.linkedAgentUserIds ?? [userId]).map(linkedUserId => ({
        agentUserId: linkedUserId,
        credentialId: `credential-${linkedUserId}`,
        displayName: `Agent ${linkedUserId}`,
        linkKind: 'credential_anchor',
        openauthUserId: options.openauthUserId ?? 'openauth-user-one',
        tokenPrefix: 'oa_agent_test',
      })),
    ),
  touchAgentCredential: () => Promise.resolve(),
  updateAgentDisplayName: () => Promise.resolve(0),
})

const route = async (
  store: MemoryPylonApiStore,
  path: string,
  options: Readonly<{
    adminToken?: boolean
    body?: unknown
    idempotencyKey?: string
    method?: string
    nowIso?: string
    linkedAgentUserIds?: ReadonlyArray<string>
    openauthUserId?: string | null
    tokenUserId?: string
  }> = {},
) => {
  let counter = 0
  const init: RequestInit = {
    headers: {
      ...(options.body === undefined
        ? {}
        : { 'content-type': 'application/json' }),
      ...(options.idempotencyKey === undefined
        ? {}
        : { 'Idempotency-Key': options.idempotencyKey }),
      ...(options.adminToken === true
        ? { authorization: 'Bearer admin' }
        : options.tokenUserId === undefined
          ? {}
          : { authorization: `Bearer oa_agent_${options.tokenUserId}` }),
    },
    method: options.method ?? 'GET',
  }

  if (options.body !== undefined) {
    init.body = JSON.stringify(options.body)
  }

  const request = new Request(`https://openagents.com${path}`, init)
  const agentStoreOptions = {
    ...(options.linkedAgentUserIds === undefined
      ? {}
      : { linkedAgentUserIds: options.linkedAgentUserIds }),
    ...(options.openauthUserId === undefined
      ? {}
      : { openauthUserId: options.openauthUserId }),
  }
  const routes = makePylonApiRoutes({
    agentStore: () =>
      agentStoreFor(options.tokenUserId ?? 'agent-one', agentStoreOptions),
    makeId: () => `test-${++counter}`,
    makeStore: () => store,
    nowIso: () => options.nowIso ?? '2026-06-07T00:10:00.000Z',
    requireAdminApiToken: request =>
      Promise.resolve(
        options.adminToken === true &&
          request.headers.get('authorization') === 'Bearer admin',
      ),
  })
  const response = routes.routePylonApiRequest(
    request,
    {
      OPENAGENTS_DB: {} as D1Database,
    },
    {} as ExecutionContext,
  )

  if (response === undefined) {
    throw new Error(`No route matched ${path}`)
  }

  return Effect.runPromise(response)
}

const responseJson = async <A = Record<string, unknown>>(response: Response) =>
  response.json() as Promise<A>

const registerPylon = async (
  store: MemoryPylonApiStore,
  input: Readonly<{
    capabilityRefs?: ReadonlyArray<string>
    idempotencyKey?: string
    providerMarketRelayRefs?: ReadonlyArray<string>
    providerNip90LaneRefs?: ReadonlyArray<string>
    providerNostrNpub?: string
    providerNostrPubkey?: string
    pylonRef?: string
    tokenUserId?: string
  }> = {},
) =>
  route(store, '/api/pylons/register', {
    body: {
      capabilityRefs: input.capabilityRefs ?? ['capability.public.inference'],
      clientProtocolVersion: '0.2.5',
      clientVersion: 'openagents.pylon@0.2.5',
      displayName: 'Edge Pylon',
      ...(input.providerMarketRelayRefs === undefined
        ? {}
        : { providerMarketRelayRefs: input.providerMarketRelayRefs }),
      ...(input.providerNip90LaneRefs === undefined
        ? {}
        : { providerNip90LaneRefs: input.providerNip90LaneRefs }),
      ...(input.providerNostrNpub === undefined
        ? {}
        : { providerNostrNpub: input.providerNostrNpub }),
      ...(input.providerNostrPubkey === undefined
        ? {}
        : { providerNostrPubkey: input.providerNostrPubkey }),
      pylonRef: input.pylonRef ?? 'pylon.test.one',
      resourceMode: 'background_20',
      walletRef: 'wallet.public.edge',
    },
    idempotencyKey: input.idempotencyKey ?? 'register-pylon-test-one',
    method: 'POST',
    tokenUserId: input.tokenUserId ?? 'agent-one',
  })

const markOnline = async (
  store: MemoryPylonApiStore,
  input: Readonly<{
    capacityRefs?: ReadonlyArray<string>
    loadRefs?: ReadonlyArray<string>
    nowIso?: string
    pylonRef?: string
    status?: string
    tokenUserId?: string
  }> = {},
) => {
  const pylonRef = input.pylonRef ?? 'pylon.test.one'
  const nowIso = input.nowIso ?? '2026-06-07T00:10:00.000Z'

  return route(store, `/api/pylons/${pylonRef}/heartbeat`, {
    body: {
      capacityRefs: input.capacityRefs ?? ['capacity.public.gpu_available'],
      clientProtocolVersion: '0.2.6',
      clientVersion: 'pylon-v0.2.6',
      healthRefs: ['health.public.ok'],
      loadRefs: input.loadRefs ?? ['load.public.low'],
      resourceMode: 'balanced',
      status: input.status ?? 'online',
    },
    idempotencyKey: `heartbeat-online-${pylonRef}-${nowIso.replace(/\D/g, '')}`,
    method: 'POST',
    nowIso,
    tokenUserId: input.tokenUserId ?? 'agent-one',
  })
}

const markWalletReady = async (
  store: MemoryPylonApiStore,
  pylonRef = 'pylon.test.one',
  tokenUserId = 'agent-one',
) =>
  route(store, `/api/pylons/${pylonRef}/wallet-readiness`, {
    body: {
      readinessRefs: ['readiness.public.mdk_agent_wallet_receive_ready'],
      walletReady: true,
      walletRef: 'wallet.public.edge',
    },
    idempotencyKey: `wallet-ready-${pylonRef}`,
    method: 'POST',
    tokenUserId,
  })

const createAssignment = async (
  store: MemoryPylonApiStore,
  input: Readonly<{
    assignmentRef?: string
    campaignPaused?: boolean
    campaignPolicyRefs?: ReadonlyArray<string>
    forumAutoPublishAllowed?: boolean
    idempotencyKey?: string
    jobKind?: string
    leaseSeconds?: number
    noDuplicateAssignmentRefs?: ReadonlyArray<string>
    nowIso?: string
    paymentMode?: string
    pylonRef?: string
    requiredCapabilityRefs?: ReadonlyArray<string>
    spendCapRefs?: ReadonlyArray<string>
    tokenUserId?: string
  }> = {},
) =>
  route(store, '/api/operator/pylons/assignments', {
    body: {
      acceptanceCriteriaRefs: ['acceptance.public.echo_result'],
      assignmentRef: input.assignmentRef ?? 'assignment.public.issue502.echo',
      campaignPaused: input.campaignPaused ?? false,
      campaignPolicyRefs: input.campaignPolicyRefs ?? [
        'policy.public.probe_gepa.no_spend_dispatch',
      ],
      campaignRef: 'campaign.public.probe_gepa.stage0.no_spend',
      closeoutPathRefs: ['closeout.public.operator_review_required'],
      forumAutoPublishAllowed: input.forumAutoPublishAllowed ?? false,
      idempotencyRefs: ['idempotency.public.pylon_assignment.request_key'],
      jobKind: input.jobKind ?? 'healthcheck_echo',
      leaseSeconds: input.leaseSeconds ?? 600,
      noDuplicateAssignmentRefs: input.noDuplicateAssignmentRefs ?? [
        'dedupe.public.pylon_assignment.active_lease',
      ],
      noForumAutoPublishRefs: ['policy.public.no_forum_auto_publish'],
      operatorPauseRefs: ['pause.public.artanis.pylon_dispatch'],
      paymentMode: input.paymentMode ?? 'unpaid_smoke',
      pylonRef: input.pylonRef ?? 'pylon.test.one',
      requiredCapabilityRefs: input.requiredCapabilityRefs ?? [
        'capability.public.inference',
      ],
      resultExpectationRefs: ['result.public.echo_summary'],
      rollbackRefs: ['rollback.public.artanis.cancel_pylon_dispatch'],
      selectionPolicyRefs: ['selection.public.pylon.capability_match'],
      spendCapRefs: input.spendCapRefs ?? [],
      taskRefs: ['task.public.echo_hello_world'],
    },
    idempotencyKey: input.idempotencyKey ?? 'assignment-create-echo',
    method: 'POST',
    ...(input.nowIso === undefined ? {} : { nowIso: input.nowIso }),
    ...(input.tokenUserId === undefined
      ? { adminToken: true }
      : { tokenUserId: input.tokenUserId }),
  })

const quarantinePylon = async (
  store: MemoryPylonApiStore,
  input: Readonly<{
    expiresAt?: string
    pylonRef?: string
    state?: 'active' | 'released'
  }> = {},
) =>
  route(store, `/api/operator/pylons/${input.pylonRef ?? 'pylon.test.one'}/quarantine`, {
    adminToken: true,
    body: {
      actionRefs: ['action.public.pylon_quarantine.disconnect_executor'],
      ...(input.expiresAt === undefined ? {} : { expiresAt: input.expiresAt }),
      reasonRefs: ['reason.public.pylon_quarantine.anomaly_detected'],
      sourceRefs: ['source.public.issue.6424'],
      state: input.state ?? 'active',
    },
    method: 'POST',
  })

describe('Pylon API routes', () => {
  test('D1 store inserts new Pylon registrations with every migrated column represented', async () => {
    const db = new RegistrationInsertD1()
    const store = makeD1PylonApiStore(db)
    const record = buildPylonApiRegistrationRecord({
      credentialId: 'credential-agent-one',
      displayName: 'Agent One',
      makeId: () => 'new-pylon',
      nowIso: '2026-06-08T14:20:00.000Z',
      ownerAgentTokenPrefix: 'oa_agent_test',
      ownerAgentUserId: 'agent-one',
      request: {
        capabilityRefs: ['capability.public.probe_gepa_unpaid_smoke'],
        clientProtocolVersion: '0.2.5',
        clientVersion: 'openagents.pylon@0.2.5',
        displayName: 'D1 Insert Pylon',
        pylonRef: 'pylon.test.d1_insert',
        resourceMode: 'background_20',
        statusRefs: ['status.public.d1_insert'],
        walletRef: 'wallet.public.d1_insert',
      },
    })

    await store.upsertRegistration(record)

    expect(db.registrationInsertShape).toEqual({
      bindCount: 26,
      columnCount: 27,
      valueCount: 27,
    })
  })

  test('D1 assignment creation batches assignment and provider lifecycle writes atomically', async () => {
    const db = new AssignmentBatchD1()
    const store = makeD1PylonApiStore(db)
    const record = buildPylonApiAssignmentRecord({
      idempotencyKeyHash: 'hash.assignment.lifecycle',
      makeId: () => 'assignment-lifecycle',
      nowIso: '2026-06-10T09:30:00.000Z',
      ownerAgentUserId: 'agent-one',
      request: {
        acceptanceCriteriaRefs: ['acceptance.public.echo_result'],
        campaignPaused: false,
        campaignPolicyRefs: ['policy.public.probe_gepa.no_spend_dispatch'],
        closeoutPathRefs: ['closeout.public.operator_review_required'],
        forumAutoPublishAllowed: false,
        idempotencyRefs: ['idempotency.public.pylon_assignment.request_key'],
        jobKind: 'healthcheck_echo',
        noDuplicateAssignmentRefs: [
          'dedupe.public.pylon_assignment.active_lease',
        ],
        noForumAutoPublishRefs: ['policy.public.no_forum_auto_publish'],
        operatorPauseRefs: ['pause.public.artanis.pylon_dispatch'],
        pylonRef: 'pylon.test.one',
        requiredCapabilityRefs: ['capability.public.inference'],
        resultExpectationRefs: ['result.public.echo_summary'],
        rollbackRefs: ['rollback.public.artanis.cancel_pylon_dispatch'],
        selectionPolicyRefs: ['selection.public.pylon.capability_match'],
        spendCapRefs: [],
        taskRefs: ['task.public.echo_hello_world'],
      },
    })

    await store.createAssignment(record)

    expect(db.batchQueries).toHaveLength(2)
    expect(db.batchQueries[0]).toContain('INSERT INTO pylon_api_assignments')
    expect(db.batchQueries[1]).toContain(
      'INSERT INTO pylon_provider_job_lifecycle',
    )

    const failingDb = new AssignmentBatchD1()
    failingDb.failBatch = true

    await expect(
      makeD1PylonApiStore(failingDb).createAssignment(record),
    ).rejects.toThrow('simulated mid-batch failure')
    expect(failingDb.batchQueries).toHaveLength(2)
  })

  test('D1 event creation is idempotent when the idempotency pre-read races a duplicate heartbeat write (#6820)', async () => {
    const idempotencyKeyHash = 'hash.presence.duplicate'
    const existing = buildPylonApiEventRecord({
      body: {
        capacityRefs: ['capacity.public.gpu_available'],
        healthRefs: ['health.public.ok'],
        loadRefs: ['load.public.low'],
        status: 'online',
      },
      eventKind: 'heartbeat',
      idempotencyKeyHash,
      makeId: () => 'existing',
      nowIso: '2026-06-28T10:00:00.000Z',
      ownerAgentUserId: 'agent-one',
      pylonRef: 'pylon.test.one',
      status: 'online',
    })
    const db = new EventIdempotentD1({
      assignment_ref: existing.assignmentRef,
      created_at: existing.createdAt,
      event_body_json: JSON.stringify(existing.eventBody),
      event_kind: existing.eventKind,
      event_ref: existing.eventRef,
      id: existing.id,
      idempotency_key_hash: existing.idempotencyKeyHash,
      owner_agent_user_id: existing.ownerAgentUserId,
      public_projection_json: existing.publicProjectionJson,
      pylon_ref: existing.pylonRef,
      status: existing.status,
    })
    const duplicate = buildPylonApiEventRecord({
      body: {
        capacityRefs: ['capacity.public.gpu_available'],
        healthRefs: ['health.public.ok'],
        loadRefs: ['load.public.low'],
        status: 'online',
      },
      eventKind: 'heartbeat',
      idempotencyKeyHash,
      makeId: () => 'duplicate',
      nowIso: '2026-06-28T10:00:01.000Z',
      ownerAgentUserId: 'agent-one',
      pylonRef: 'pylon.test.one',
      status: 'online',
    })

    const result = await makeD1PylonApiStore(db).createEvent(duplicate)

    expect(result.idempotent).toBe(true)
    expect(result.record.eventRef).toBe(existing.eventRef)
    expect(db.insertQueries).toHaveLength(1)
    expect(db.insertQueries[0]).toContain(
      'ON CONFLICT(idempotency_key_hash) DO UPDATE',
    )
    expect(db.events.get(idempotencyKeyHash)?.event_ref).toBe(
      existing.eventRef,
    )
    expect(db.events.get(idempotencyKeyHash)?.created_at).toBe(
      existing.createdAt,
    )
  })

  test('D1 store chunks multi-value Pylon reads under the D1 bind ceiling', async () => {
    const db = new ChunkedSelectD1()
    const store = makeD1PylonApiStore(db)
    const refs = Array.from(
      { length: 105 },
      (_, index) => `pylon.test.chunk_${index}`,
    )
    const ownerIds = Array.from(
      { length: 105 },
      (_, index) => `agent_chunk_${index}`,
    )

    await store.listAssignmentsForPylons?.(refs, 1_000)
    await store.listProviderJobLifecycleForPylons(refs, 1_000)
    await store.listRegistrationsForOwnerAgentUserIds?.(ownerIds, 200)

    expect(db.bindCounts).toEqual([81, 26, 81, 26, 81, 26])
    const assignmentQueries = db.queries.filter(query =>
      query.includes('FROM pylon_api_assignments'),
    )
    expect(assignmentQueries).toHaveLength(2)
    expect(
      assignmentQueries.every(query =>
        query.includes(
          "state IN ('accepted', 'blocked', 'offered', 'proof_submitted', 'running')",
        ),
      ),
    ).toBe(true)
    expect(
      db.queries.filter(query =>
        query.includes('FROM pylon_provider_job_lifecycle'),
      ),
    ).toHaveLength(2)
    expect(
      db.queries.filter(query =>
        query.includes('FROM pylon_api_registrations'),
      ),
    ).toHaveLength(2)
  })

  test('registers a Pylon and exposes public-safe reads', async () => {
    const store = new MemoryPylonApiStore()
    const created = await registerPylon(store)
    const createdJson = await responseJson<PylonRouteJson>(created)
    const list = await route(store, '/api/pylons')
    const detail = await route(store, '/api/pylons/pylon.test.one')

    expect(created.status).toBe(201)
    expect(createdJson.pylon?.pylonRef).toBe('pylon.test.one')
    expect(createdJson.pylon?.clientVersion).toBe('openagents.pylon@0.2.5')
    expect(createdJson.pylon?.clientProtocolVersion).toBe('0.2.5')
    expect(createdJson.pylon?.createdAtDisplay).toBe('Just now')
    expect(JSON.stringify(createdJson)).not.toMatch(/2026-06-07T00:10/)
    expect((await responseJson<PylonRouteJson>(list)).pylons).toHaveLength(1)
    expect((await responseJson<PylonRouteJson>(detail)).events).toHaveLength(1)
  })

  test('projects operator quarantine on heartbeat and blocks new dispatch (#6424)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, {
      capabilityRefs: ['capability.public.inference'],
    })
    await markOnline(store)

    const quarantine = await quarantinePylon(store)
    const quarantineJson = await responseJson<{
      quarantine: { active: boolean; quarantineRef: string }
    }>(quarantine)
    expect(quarantine.status).toBe(201)
    expect(quarantineJson.quarantine).toMatchObject({
      active: true,
      quarantineRef: 'quarantine.public.pylon.pylon.test.one',
    })

    const heartbeat = await markOnline(store, {
      nowIso: '2026-06-07T00:11:00.000Z',
    })
    const heartbeatJson = await responseJson<PylonRouteJson>(heartbeat)
    expect(heartbeatJson.pylon?.quarantine).toMatchObject({
      active: true,
      state: 'active',
    })

    const assignment = await createAssignment(store, {
      idempotencyKey: 'assignment-blocked-by-quarantine',
      nowIso: '2026-06-07T00:11:30.000Z',
    })
    const assignmentJson = await responseJson<PylonRouteJson>(assignment)
    expect(assignment.status).toBe(409)
    expect(assignmentJson.error).toBe('controlled_dispatch_gate_blocked')
    expect(assignmentJson.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.executor_quarantined',
    )
  })

  test('refuses an unreceipted Tassadar executor capability claim at registration (W4.1)', async () => {
    const store = new MemoryPylonApiStore()
    const created = await registerPylon(store, {
      capabilityRefs: [
        'capability.public.inference',
        'capability.tassadar_poc.numeric_model_executor',
      ],
    })
    const createdJson = await responseJson<PylonRouteJson>(created)

    expect(created.status).toBe(201)
    expect(createdJson.tassadarCapabilityAdmission?.state).toBe('refused')
    expect(createdJson.tassadarCapabilityAdmission?.refusalRefs).toEqual([
      'refusal.public.pylon_capability.tassadar_executor_unreceipted',
    ])
    expect(createdJson.pylon?.capabilityRefs).toEqual([
      'capability.public.inference',
    ])

    const stored = store.registrations.get('pylon.test.one')
    expect(stored?.capabilityRefs).toEqual(['capability.public.inference'])
  })

  test('admits and advertises a Tassadar executor capability carried with its self-test receipt (W4.1)', async () => {
    const store = new MemoryPylonApiStore()
    const receiptRef = 'receipt.tassadar_executor.self_test.v1.f2995c4e3c959b42'
    const created = await registerPylon(store, {
      capabilityRefs: [
        'capability.tassadar_poc.numeric_model_executor',
        receiptRef,
      ],
    })
    const createdJson = await responseJson<PylonRouteJson>(created)

    expect(created.status).toBe(201)
    expect(createdJson.tassadarCapabilityAdmission?.state).toBe('admitted')
    expect(
      createdJson.tassadarCapabilityAdmission?.selfTestReceiptRefs,
    ).toEqual([receiptRef])
    expect(createdJson.pylon?.capabilityRefs).toContain(
      'capability.tassadar_poc.numeric_model_executor',
    )
    expect(createdJson.pylon?.capabilityRefs).toContain(receiptRef)
  })

  test('blocks executor dispatch against a legacy registration whose capability has no self-test receipt (W4.1)', async () => {
    const store = new MemoryPylonApiStore()
    const receiptRef = 'receipt.tassadar_executor.self_test.v1.f2995c4e3c959b42'
    await registerPylon(store, {
      capabilityRefs: [
        'capability.tassadar_poc.numeric_model_executor',
        receiptRef,
      ],
    })
    await markOnline(store)
    await markWalletReady(store)

    const receipted = await createAssignment(store, {
      assignmentRef: 'assignment.public.tassadar_receipted',
      idempotencyKey: 'assignment-tassadar-receipted',
      requiredCapabilityRefs: [
        'capability.tassadar_poc.numeric_model_executor',
      ],
    })
    expect(receipted.status).toBe(201)

    // Simulate a pre-W4.1 registration row: capability claim stored
    // without its self-test receipt ref.
    const stored = store.registrations.get('pylon.test.one')
    store.registrations.set('pylon.test.one', {
      ...stored!,
      capabilityRefs: ['capability.tassadar_poc.numeric_model_executor'],
    })

    const unreceipted = await createAssignment(store, {
      assignmentRef: 'assignment.public.tassadar_unreceipted',
      idempotencyKey: 'assignment-tassadar-unreceipted',
      requiredCapabilityRefs: [
        'capability.tassadar_poc.numeric_model_executor',
      ],
    })
    const unreceiptedJson = await responseJson<PylonRouteJson>(unreceipted)

    expect(unreceipted.status).toBe(409)
    expect(unreceiptedJson.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.tassadar_capability_unreceipted',
    )
  })

  test('renders scanner-shaped Pylon public refs as short dotted aliases', async () => {
    const store = new MemoryPylonApiStore()
    const scannerShapedCapabilityRef =
      'edge-pylon-capability-8b378373002501f3e896dcd3'
    const expectedCapabilityRef = publicScannerSafeRef(
      'capability.public.pylon',
      scannerShapedCapabilityRef,
    )
    const created = await registerPylon(store, {
      capabilityRefs: [scannerShapedCapabilityRef, 'capability.public.gpu'],
    })
    const createdJson = await responseJson<PylonRouteJson>(created)
    const listJson = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons'),
    )
    const detailJson = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons/pylon.test.one'),
    )

    expect(createdJson.pylon?.capabilityRefs).toEqual([
      'capability.public.gpu',
      expectedCapabilityRef,
    ])
    expect(listJson.pylons?.[0]?.capabilityRefs).toEqual([
      'capability.public.gpu',
      expectedCapabilityRef,
    ])
    expect(detailJson.pylon?.capabilityRefs).toEqual([
      'capability.public.gpu',
      expectedCapabilityRef,
    ])
    expect(JSON.stringify({ createdJson, listJson, detailJson })).not.toContain(
      scannerShapedCapabilityRef,
    )
  })

  test('publishes provider Nostr pubkey, relay refs, and NIP-90 lane refs for provider Pylons (#4864)', async () => {
    const store = new MemoryPylonApiStore()
    const providerNostrPubkey =
      'a1b2c3d4e5f6a7b8c9d0e1f2a3b4c5d6e7f8a9b0c1d2e3f4a5b6c7d8e9f0a1b2'
    const providerNostrNpub =
      'npub1qpzry9x8gf2tvdw0s3jn54khce6mua7lqpzry9x8gf2tvdw0s3jn54khce'
    const providerMarketRelayRefs = [
      'wss://relay.openagents.com',
      'wss://relay.damus.io',
    ]
    const providerNip90LaneRefs = [
      'lane.public.nip90.5050.text_generation',
      'lane.public.nip90.5934.labor_code_task',
    ]

    // The hex pubkey is exactly the shape the raw-id scanner aliases, so
    // this test pins the deliberate identity-field allowlist: the value
    // must survive projection verbatim or stranger buyers cannot map
    // relay bids (event.pubkey) to registered capacity.
    expect(publicRefTriggersAgentSecretScanner(providerNostrPubkey)).toBe(true)

    const created = await registerPylon(store, {
      providerMarketRelayRefs,
      providerNip90LaneRefs,
      providerNostrNpub,
      providerNostrPubkey,
    })
    const createdJson = await responseJson<PylonRouteJson>(created)
    const listJson = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons'),
    )
    const detailJson = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons/pylon.test.one'),
    )

    expect(created.status).toBe(201)
    expect(createdJson.pylon?.providerNostrPubkey).toBe(providerNostrPubkey)
    expect(createdJson.pylon?.providerNostrNpub).toBe(providerNostrNpub)
    expect(createdJson.pylon?.providerMarketRelayRefs).toEqual(
      providerMarketRelayRefs,
    )
    expect(createdJson.pylon?.providerNip90LaneRefs).toEqual(
      providerNip90LaneRefs,
    )
    expect(listJson.pylons?.[0]?.providerNostrPubkey).toBe(providerNostrPubkey)
    expect(listJson.pylons?.[0]?.providerNostrNpub).toBe(providerNostrNpub)
    expect(listJson.pylons?.[0]?.providerMarketRelayRefs).toEqual(
      providerMarketRelayRefs,
    )
    expect(listJson.pylons?.[0]?.providerNip90LaneRefs).toEqual(
      providerNip90LaneRefs,
    )
    expect(detailJson.pylon?.providerNostrPubkey).toBe(providerNostrPubkey)
  })

  test('leaves provider discovery fields absent for non-provider or pre-upgrade registrations (#4864)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    const listJson = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons'),
    )

    expect(listJson.pylons?.[0]?.providerNostrPubkey).toBeNull()
    expect(listJson.pylons?.[0]?.providerNostrNpub).toBeNull()
    expect(listJson.pylons?.[0]?.providerMarketRelayRefs).toEqual([])
    expect(listJson.pylons?.[0]?.providerNip90LaneRefs).toEqual([])
  })

  test('upgrades provider discovery fields from a heartbeat without re-registration (#4864)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    const providerNostrPubkey =
      'f0e1d2c3b4a5f6e7d8c9b0a1f2e3d4c5b6a7f8e9d0c1b2a3f4e5d6c7b8a9f0e1'
    const heartbeat = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: {
          capacityRefs: ['capacity.public.gpu_available'],
          healthRefs: ['health.public.ok'],
          loadRefs: ['load.public.low'],
          providerMarketRelayRefs: ['wss://relay.openagents.com'],
          providerNip90LaneRefs: ['lane.public.nip90.5050.text_generation'],
          providerNostrPubkey,
          status: 'online',
        },
        idempotencyKey: 'heartbeat-provider-discovery-upgrade',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const heartbeatJson = await responseJson<PylonRouteJson>(heartbeat)
    const listJson = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons'),
    )

    expect(heartbeat.status).toBe(201)
    expect(heartbeatJson.pylon?.providerNostrPubkey).toBe(providerNostrPubkey)
    expect(listJson.pylons?.[0]?.providerNostrPubkey).toBe(providerNostrPubkey)
    expect(listJson.pylons?.[0]?.providerMarketRelayRefs).toEqual([
      'wss://relay.openagents.com',
    ])
    expect(listJson.pylons?.[0]?.providerNip90LaneRefs).toEqual([
      'lane.public.nip90.5050.text_generation',
    ])
  })

  test('refreshes capability refs from a heartbeat without re-registration (#6354)', async () => {
    const store = new MemoryPylonApiStore()
    // Register BEFORE a Codex account is linked: the registration carries no
    // local Codex capability.
    await registerPylon(store, {
      capabilityRefs: ['capability.public.inference'],
    })
    const before = store.registrations.get('pylon.test.one')
    expect(before?.capabilityRefs).toEqual(['capability.public.inference'])

    // A later heartbeat advertises the newly-linked Codex capability and its
    // codex capacity. The dispatch gate must see the Pylon as Codex-capable.
    const heartbeat = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: {
          capabilityRefs: [
            'capability.public.inference',
            'capability.pylon.local_codex',
          ],
          capacityRefs: [
            'capacity.public.pylon_cli.available',
            'capacity.coding.codex.ready=2',
            'capacity.coding.codex.available=2',
          ],
          healthRefs: ['health.public.pylon_cli.ok'],
          loadRefs: [
            'load.public.pylon_cli.low',
            'load.coding.codex.busy=0',
            'load.coding.codex.queued=0',
          ],
          status: 'online',
        },
        idempotencyKey: 'heartbeat-capability-refresh-6354',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )

    expect(heartbeat.status).toBe(201)
    const stored = store.registrations.get('pylon.test.one')
    expect(stored?.capabilityRefs).toContain('capability.pylon.local_codex')
    expect(stored?.capabilityRefs).toContain('capability.public.inference')
  })

  test('a heartbeat cannot inject an unreceipted Tassadar executor capability (#6354/W4.1)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, {
      capabilityRefs: ['capability.public.inference'],
    })

    const heartbeat = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: {
          capabilityRefs: [
            'capability.public.inference',
            'capability.tassadar_poc.numeric_model_executor',
          ],
          capacityRefs: ['capacity.public.pylon_cli.available'],
          healthRefs: ['health.public.pylon_cli.ok'],
          loadRefs: ['load.public.pylon_cli.low'],
          status: 'online',
        },
        idempotencyKey: 'heartbeat-unreceipted-executor-6354',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )

    expect(heartbeat.status).toBe(201)
    const stored = store.registrations.get('pylon.test.one')
    expect(stored?.capabilityRefs).toContain('capability.public.inference')
    expect(stored?.capabilityRefs).not.toContain(
      'capability.tassadar_poc.numeric_model_executor',
    )
  })

  test('a heartbeat without capability refs leaves the stored set intact (#6354)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, {
      capabilityRefs: [
        'capability.public.inference',
        'capability.pylon.local_codex',
      ],
    })

    // An older client that omits `capabilityRefs` must not silently strip the
    // registration's capabilities.
    await markOnline(store)
    const stored = store.registrations.get('pylon.test.one')
    expect(stored?.capabilityRefs).toContain('capability.pylon.local_codex')
    expect(stored?.capabilityRefs).toContain('capability.public.inference')
  })

  test('rejects malformed provider discovery identity fields (#4864)', async () => {
    const store = new MemoryPylonApiStore()
    const rejected = await registerPylon(store, {
      providerNostrPubkey: 'not-a-pubkey',
    })

    expect(rejected.status).toBe(400)
  })

  test('parses Pylon client versions with a v0.2.5 minimum helper', () => {
    expect(pylonClientVersionMeetsMinimum('0.2.5', '0.2.5')).toBe(true)
    expect(pylonClientVersionMeetsMinimum('pylon-v0.2.6', '0.2.5')).toBe(true)
    expect(
      pylonClientVersionMeetsMinimum('openagents.pylon@0.2.5', '0.2.5'),
    ).toBe(true)
    expect(pylonClientVersionMeetsMinimum('pylon-v0.2.4', '0.2.5')).toBe(false)
    expect(pylonClientVersionMeetsMinimum('release train ready', '0.2.5')).toBe(
      false,
    )
  })

  test('heartbeat with walletReady=true flips registration walletReady without a separate wallet-readiness event (#5151)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)

    const heartbeat = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: {
          capacityRefs: ['capacity.public.gpu_available'],
          clientProtocolVersion: '0.2.6',
          clientVersion: 'pylon-v0.2.6',
          healthRefs: ['health.public.ok'],
          loadRefs: ['load.public.low'],
          resourceMode: 'balanced',
          status: 'online',
          walletReady: true,
        },
        idempotencyKey: 'key-heartbeat-walletready',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    expect(heartbeat.status).toBe(201)

    const detail = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons/pylon.test.one'),
    )
    // Before #5151 this stayed false until a separate wallet-readiness event.
    expect(detail.pylon?.walletReady).toBe(true)
  })

  test('heartbeat accepts counted coding capacity refs and projects service dimensions (#6276)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, {
      capabilityRefs: ['capability.pylon.local_codex'],
    })

    const heartbeat = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: {
          capacityRefs: [
            'capacity.coding.codex.ready=2',
            'capacity.coding.codex.available=1',
          ],
          clientProtocolVersion: '0.3.0',
          clientVersion: 'openagents.pylon@0.3.0',
          healthRefs: ['health.public.ok'],
          loadRefs: ['load.coding.codex.busy=1', 'load.coding.codex.queued=0'],
          resourceMode: 'balanced',
          status: 'online',
        },
        idempotencyKey: 'key-heartbeat-counted-coding-capacity',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    expect(heartbeat.status).toBe(201)

    const detail = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons/pylon.test.one'),
    )
    expect(detail.pylon?.latestCapacityRefs).toEqual([
      'capacity.coding.codex.available=1',
      'capacity.coding.codex.ready=2',
    ])
    expect(detail.pylon?.latestLoadRefs).toEqual([
      'load.coding.codex.busy=1',
      'load.coding.codex.queued=0',
    ])
    expect(detail.pylon?.codingCapacity).toContainEqual({
      accounts: [],
      available: 1,
      busy: 1,
      queued: 0,
      ready: 2,
      service: 'codex',
    })
  })

  test('records heartbeat, wallet readiness, assignment, artifact, payment, and settlement events', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    const heartbeat = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: {
          capacityRefs: ['capacity.public.gpu_available'],
          clientProtocolVersion: '0.2.6',
          clientVersion: 'pylon-v0.2.6',
          healthRefs: ['health.public.ok'],
          loadRefs: ['load.public.low'],
          resourceMode: 'balanced',
          status: 'online',
        },
        idempotencyKey: 'key-heartbeat',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const wallet = await route(
      store,
      '/api/pylons/pylon.test.one/wallet-readiness',
      {
        body: {
          readinessRefs: ['readiness.public.ok'],
          status: 'ready',
          walletReady: true,
          walletRef: 'wallet.public.edge',
        },
        idempotencyKey: 'key-wallet',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const assignment = await createAssignment(store, {
      assignmentRef: 'assignment.public.one',
      idempotencyKey: 'key-assignment',
    })
    const writes = [
      [
        '/api/pylons/pylon.test.one/payout-target-admission',
        {
          admissionRefs: ['admission.public.requested'],
          payoutTargetRef: 'payout_target.public.edge_hash',
          status: 'requested',
        },
      ],
      [
        '/api/pylons/pylon.test.one/assignments/assignment.public.one/accept',
        {
          acceptanceRefs: ['acceptance.public.assignment_one'],
          accepted: true,
        },
      ],
      [
        '/api/pylons/pylon.test.one/assignments/assignment.public.one/progress',
        {
          progressPercent: 50,
          progressRefs: ['progress.public.halfway'],
          status: 'running',
        },
      ],
      [
        '/api/pylons/pylon.test.one/assignments/assignment.public.one/artifacts',
        {
          artifactRefs: ['artifact.public.bundle_one'],
          proofRefs: ['proof.public.bundle_one'],
        },
      ],
      [
        '/api/pylons/pylon.test.one/assignments/assignment.public.one/payment-receipts',
        {
          paymentProofRefs: ['payment_proof.public.redacted_one'],
          receiptRefs: ['receipt.public.payment_one'],
        },
      ],
      [
        '/api/pylons/pylon.test.one/assignments/assignment.public.one/settlement-status',
        {
          settlementRefs: ['settlement.public.done'],
          status: 'settled',
          treasuryReceiptRefs: ['treasury_receipt.public.one'],
        },
      ],
    ] as const

    expect(heartbeat.status).toBe(201)
    expect(wallet.status).toBe(201)
    expect(assignment.status).toBe(201)

    for (const [path, body] of writes) {
      const response = await route(store, path, {
        body,
        idempotencyKey: `key-${path}`,
        method: 'POST',
        tokenUserId: 'agent-one',
      })

      expect(response.status).toBe(201)
    }

    const detail = await responseJson<PylonRouteJson>(
      await route(store, '/api/pylons/pylon.test.one'),
    )

    expect(detail.events).toHaveLength(9)
    expect(detail.pylon?.walletReady).toBe(true)
    expect(detail.pylon?.latestHeartbeatDisplay).toBe('Just now')
    expect(detail.pylon?.clientVersion).toBe('pylon-v0.2.6')
    expect(detail.pylon?.clientProtocolVersion).toBe('0.2.6')
    expect(detail.pylon?.latestHeartbeatStatus).toBe('online')
    expect(detail.pylon?.latestResourceMode).toBe('balanced')
    expect(detail.pylon?.latestHealthRefs).toEqual(['health.public.ok'])
    expect(detail.pylon?.latestLoadRefs).toEqual(['load.public.low'])
    expect(detail.pylon?.latestCapacityRefs).toEqual([
      'capacity.public.gpu_available',
    ])
  })

  test('rejects malformed Pylon client versions', async () => {
    const store = new MemoryPylonApiStore()
    const response = await route(store, '/api/pylons/register', {
      body: {
        capabilityRefs: ['capability.public.inference'],
        clientVersion: 'pylon release train ready',
        pylonRef: 'pylon.test.bad_version',
      },
      idempotencyKey: 'register-bad-client-version',
      method: 'POST',
      tokenUserId: 'agent-one',
    })
    const body = await responseJson<PylonRouteJson>(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('pylon_api_validation_error')
  })

  test('lets a same-OpenAuth rotated agent credential reclaim and heartbeat a Pylon', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, { tokenUserId: 'agent-old' })

    const reRegister = await route(store, '/api/pylons/register', {
      body: {
        capabilityRefs: ['capability.pylon.local_codex'],
        clientProtocolVersion: '0.3.0',
        clientVersion: 'openagents.pylon@1.0.5',
        pylonRef: 'pylon.test.one',
        resourceMode: 'background_20',
      },
      idempotencyKey: 'register-pylon-test-one-rotated',
      linkedAgentUserIds: ['agent-old', 'agent-new'],
      method: 'POST',
      openauthUserId: 'openauth-owner-one',
      tokenUserId: 'agent-new',
    })
    const heartbeat = await route(store, '/api/pylons/pylon.test.one/heartbeat', {
      body: {
        capacityRefs: ['capacity.coding.codex.available=1'],
        clientProtocolVersion: '0.3.0',
        clientVersion: 'openagents.pylon@1.0.5',
        healthRefs: ['health.public.ok'],
        loadRefs: ['load.coding.codex.busy=0'],
        resourceMode: 'background_20',
        status: 'online',
      },
      idempotencyKey: 'heartbeat-rotated-agent',
      linkedAgentUserIds: ['agent-old', 'agent-new'],
      method: 'POST',
      openauthUserId: 'openauth-owner-one',
      tokenUserId: 'agent-new',
    })
    const stored = await store.readRegistration('pylon.test.one')

    expect(reRegister.status).toBe(201)
    expect(heartbeat.status).toBe(201)
    expect(stored?.ownerAgentUserId).toBe('agent-new')
    expect(stored?.latestHeartbeatStatus).toBe('online')
    expect(stored?.latestCapacityRefs).toEqual([
      'capacity.coding.codex.available=1',
    ])
  })

  test('keeps unrelated agent credentials from reclaiming a registered Pylon', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, { tokenUserId: 'agent-old' })

    const reRegister = await route(store, '/api/pylons/register', {
      body: {
        capabilityRefs: ['capability.pylon.local_codex'],
        pylonRef: 'pylon.test.one',
      },
      idempotencyKey: 'register-pylon-test-one-foreign',
      linkedAgentUserIds: ['agent-new'],
      method: 'POST',
      openauthUserId: 'openauth-owner-two',
      tokenUserId: 'agent-new',
    })
    const heartbeat = await route(store, '/api/pylons/pylon.test.one/heartbeat', {
      body: { healthRefs: ['health.public.ok'] },
      idempotencyKey: 'heartbeat-foreign-agent',
      linkedAgentUserIds: ['agent-new'],
      method: 'POST',
      openauthUserId: 'openauth-owner-two',
      tokenUserId: 'agent-new',
    })
    const registerBody = await responseJson<PylonRouteJson>(reRegister)
    const heartbeatBody = await responseJson<PylonRouteJson>(heartbeat)

    expect(reRegister.status).toBe(409)
    expect(registerBody.error).toBe('pylon_api_conflict')
    expect(heartbeat.status).toBe(403)
    expect(heartbeatBody.error).toBe('pylon_api_forbidden')
  })

  test('collapses duplicate idempotency keys', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    const first = await route(store, '/api/pylons/pylon.test.one/heartbeat', {
      body: { healthRefs: ['health.public.ok'] },
      idempotencyKey: 'heartbeat-once',
      method: 'POST',
      tokenUserId: 'agent-one',
    })
    const replay = await route(store, '/api/pylons/pylon.test.one/heartbeat', {
      body: { healthRefs: ['health.public.ok'] },
      idempotencyKey: 'heartbeat-once',
      method: 'POST',
      tokenUserId: 'agent-one',
    })

    expect(first.status).toBe(201)
    expect(replay.status).toBe(200)
    expect((await responseJson<PylonRouteJson>(replay)).idempotent).toBe(true)
  })

  test('returns the documented token-only presence contract for a self-signed Nostr heartbeat (#5058)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)

    const request = new Request(
      'https://openagents.com/api/pylons/pylon.test.one/heartbeat',
      {
        body: JSON.stringify({
          healthRefs: ['health.public.ok'],
          status: 'online',
        }),
        headers: {
          authorization:
            'Nostr eyJpZCI6ImFiYyIsImtpbmQiOjI3MjM1LCJzaWciOiJkZWFkYmVlZiJ9',
          'content-type': 'application/json',
          'Idempotency-Key': 'heartbeat-nostr-self-signed',
        },
        method: 'POST',
      },
    )
    const routes = makePylonApiRoutes({
      agentStore: () => agentStoreFor('agent-one'),
      makeId: () => 'test-nostr',
      makeStore: () => store,
      nowIso: () => '2026-06-07T00:10:00.000Z',
      requireAdminApiToken: () => Promise.resolve(false),
    })
    const response = routes.routePylonApiRequest(
      request,
      {
        OPENAGENTS_DB: {} as D1Database,
      },
      {} as ExecutionContext,
    )

    if (response === undefined) {
      throw new Error('No route matched heartbeat')
    }

    const resolved = await Effect.runPromise(response)
    const body = await responseJson<PylonRouteJson>(resolved)

    expect(resolved.status).toBe(401)
    expect(resolved.headers.get('www-authenticate')).toBe('Bearer')
    expect(body.error).toBe('pylon_api_presence_requires_agent_token')
    expect(body.reason).toContain('Bearer')

    const detail = await route(store, '/api/pylons/pylon.test.one', {})
    expect(
      (await responseJson<PylonRouteJson>(detail)).pylon?.latestHeartbeatStatus,
    ).not.toBe('online')
  })

  test('returns a bare unauthorized for a heartbeat with no authorization', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)

    const response = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: { healthRefs: ['health.public.ok'], status: 'online' },
        idempotencyKey: 'heartbeat-no-auth',
        method: 'POST',
      },
    )
    const body = await responseJson<PylonRouteJson>(response)

    expect(response.status).toBe(401)
    expect(response.headers.get('www-authenticate')).toBeNull()
    expect(body.error).toBe('unauthorized')
  })

  test('records payout-target admission lifecycle statuses as public-safe events', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)

    for (const status of [
      'pending',
      'approved',
      'revoked',
      'blocked',
      'stale',
    ]) {
      const response = await route(
        store,
        '/api/pylons/pylon.test.one/payout-target-admission',
        {
          body: {
            admissionRefs: [`admission.public.${status}`],
            payoutTargetRef: 'payout_target.public.edge_hash',
            policyRefs: [`policy.public.${status}`],
            status,
          },
          idempotencyKey: `payout-target-${status}`,
          method: 'POST',
          tokenUserId: 'agent-one',
        },
      )

      expect(response.status).toBe(201)
    }

    const events = Array.from(store.eventsByIdempotency.values()).filter(
      event => event.eventKind === 'payout_target_admission',
    )

    expect(events.map(event => event.status)).toEqual([
      'pending',
      'approved',
      'revoked',
      'blocked',
      'stale',
    ])
    expect(
      JSON.stringify(events.map(event => event.publicProjectionJson)),
    ).not.toMatch(/lnbc|payment_hash|preimage|balance\.mdk_agent_wallet\.\d/i)
  })

  // #5252: raw Spark address registration as a payout target. The raw spark1…
  // is stored privately keyed to pylonRef; the public projection carries only
  // the redacted payout.spark.<digest> ref.
  const sparkDigestRef = async (rawSparkAddress: string) => {
    const buffer = await crypto.subtle.digest(
      'SHA-256',
      new TextEncoder().encode(rawSparkAddress.trim()),
    )
    const hex = Array.from(new Uint8Array(buffer))
      .map(byte => byte.toString(16).padStart(2, '0'))
      .join('')

    return `payout.spark.${hex.slice(0, 24)}`
  }

  const sparkRoute = async (
    store: MemoryPylonApiStore,
    sparkStore: MemorySparkPayoutTargetStore | undefined,
    path: string,
    options: Readonly<{
      body?: unknown
      idempotencyKey?: string
      method?: string
      nowIso?: string
      tokenUserId?: string
    }> = {},
  ) => {
    let counter = 0
    const init: RequestInit = {
      headers: {
        ...(options.body === undefined
          ? {}
          : { 'content-type': 'application/json' }),
        ...(options.idempotencyKey === undefined
          ? {}
          : { 'Idempotency-Key': options.idempotencyKey }),
        ...(options.tokenUserId === undefined
          ? {}
          : { authorization: `Bearer oa_agent_${options.tokenUserId}` }),
      },
      method: options.method ?? 'POST',
    }

    if (options.body !== undefined) {
      init.body = JSON.stringify(options.body)
    }

    const request = new Request(`https://openagents.com${path}`, init)
    const routes = makePylonApiRoutes({
      agentStore: () => agentStoreFor(options.tokenUserId ?? 'agent-one'),
      makeId: () => `test-${++counter}`,
      makeStore: () => store,
      ...(sparkStore === undefined
        ? {}
        : { makeSparkPayoutTargetStore: () => sparkStore }),
      nowIso: () => options.nowIso ?? '2026-06-07T00:10:00.000Z',
      requireAdminApiToken: () => Promise.resolve(false),
    })
    const response = routes.routePylonApiRequest(
      request,
      {
        OPENAGENTS_DB: {} as D1Database,
      },
      {} as ExecutionContext,
    )

    if (response === undefined) {
      throw new Error(`No route matched ${path}`)
    }

    return Effect.runPromise(response)
  }

  test('registers a raw Spark address privately and projects only the redacted digest (#5252)', async () => {
    const store = new MemoryPylonApiStore()
    const sparkStore = new MemorySparkPayoutTargetStore()
    await registerPylon(store)

    const rawSparkAddress =
      'spark1pqqqqq0000000000000000000000000000agentpayout'
    const payoutTargetRef = await sparkDigestRef(rawSparkAddress)

    const response = await sparkRoute(
      store,
      sparkStore,
      '/api/pylons/pylon.test.one/spark-payout-target',
      {
        body: {
          payoutTargetRef,
          policyRefs: ['policy.public.pylon.redacted_payout_target_only'],
          rawSparkAddress,
          status: 'registered',
        },
        idempotencyKey: 'spark-register-one',
        tokenUserId: 'agent-one',
      },
    )
    const body = await responseJson<PylonRouteJson>(response)

    expect(response.status).toBe(201)

    // The raw address lives ONLY in the private store, keyed to the pylonRef.
    const stored = await sparkStore.read('pylon.test.one')
    expect(stored?.rawSparkAddress).toBe(rawSparkAddress)
    expect(stored?.ownerAgentUserId).toBe('agent-one')
    expect(stored?.payoutTargetRef).toBe(payoutTargetRef)

    // The public event carries only the redacted digest, never the raw address.
    const events = Array.from(store.eventsByIdempotency.values())
    const eventJson = JSON.stringify(events)
    expect(eventJson).not.toContain('spark1')
    expect(eventJson).toContain(payoutTargetRef)

    // The HTTP response projection must not leak the raw spark1… anywhere.
    expect(JSON.stringify(body)).not.toContain('spark1')
  })

  test('idempotent re-register of the same Spark address is a no-op update (#5252)', async () => {
    const store = new MemoryPylonApiStore()
    const sparkStore = new MemorySparkPayoutTargetStore()
    await registerPylon(store)

    const rawSparkAddress =
      'spark1pqqqqq0000000000000000000000000000agentpayout'
    const payoutTargetRef = await sparkDigestRef(rawSparkAddress)
    const body = {
      payoutTargetRef,
      policyRefs: ['policy.public.pylon.redacted_payout_target_only'],
      rawSparkAddress,
      status: 'registered',
    }

    const first = await sparkRoute(
      store,
      sparkStore,
      '/api/pylons/pylon.test.one/spark-payout-target',
      { body, idempotencyKey: 'spark-register-idem', tokenUserId: 'agent-one' },
    )
    const second = await sparkRoute(
      store,
      sparkStore,
      '/api/pylons/pylon.test.one/spark-payout-target',
      { body, idempotencyKey: 'spark-register-idem', tokenUserId: 'agent-one' },
    )

    expect(first.status).toBe(201)
    expect(second.status).toBe(200)
    expect((await responseJson<PylonRouteJson>(second)).idempotent).toBe(true)
    expect(sparkStore.records.size).toBe(1)
  })

  test('rejects a Spark payout-target ref that does not match the raw address digest (#5252)', async () => {
    const store = new MemoryPylonApiStore()
    const sparkStore = new MemorySparkPayoutTargetStore()
    await registerPylon(store)

    const response = await sparkRoute(
      store,
      sparkStore,
      '/api/pylons/pylon.test.one/spark-payout-target',
      {
        body: {
          payoutTargetRef: 'payout.spark.deadbeefdeadbeefdeadbeef',
          rawSparkAddress:
            'spark1pqqqqq0000000000000000000000000000agentpayout',
          status: 'registered',
        },
        idempotencyKey: 'spark-register-mismatch',
        tokenUserId: 'agent-one',
      },
    )
    const body = await responseJson<PylonRouteJson>(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('pylon_api_validation_error')
    expect(await sparkStore.read('pylon.test.one')).toBeUndefined()
  })

  test('an agent cannot register a Spark target on a Pylon it does not own (#5252)', async () => {
    const store = new MemoryPylonApiStore()
    const sparkStore = new MemorySparkPayoutTargetStore()
    await registerPylon(store, { tokenUserId: 'agent-one' })

    const rawSparkAddress =
      'spark1pqqqqq0000000000000000000000000000agentpayout'
    const response = await sparkRoute(
      store,
      sparkStore,
      '/api/pylons/pylon.test.one/spark-payout-target',
      {
        body: {
          payoutTargetRef: await sparkDigestRef(rawSparkAddress),
          rawSparkAddress,
          status: 'registered',
        },
        idempotencyKey: 'spark-register-foreign',
        tokenUserId: 'agent-two',
      },
    )
    const body = await responseJson<PylonRouteJson>(response)

    expect(response.status).toBe(403)
    expect(body.error).toBe('pylon_api_forbidden')
    expect(await sparkStore.read('pylon.test.one')).toBeUndefined()
  })

  test('fails closed (501) when the private Spark store is not wired (#5252)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)

    const rawSparkAddress =
      'spark1pqqqqq0000000000000000000000000000agentpayout'
    const response = await sparkRoute(
      store,
      undefined,
      '/api/pylons/pylon.test.one/spark-payout-target',
      {
        body: {
          payoutTargetRef: await sparkDigestRef(rawSparkAddress),
          rawSparkAddress,
          status: 'registered',
        },
        idempotencyKey: 'spark-register-unwired',
        tokenUserId: 'agent-one',
      },
    )

    expect(response.status).toBe(501)
    expect((await responseJson<PylonRouteJson>(response)).error).toBe(
      'pylon_api_spark_payout_target_unavailable',
    )
  })

  test('settlement resolver returns the registered raw Spark address for a recipient that has one (#5252)', async () => {
    const store = new MemoryPylonApiStore()
    const sparkStore = new MemorySparkPayoutTargetStore()
    await registerPylon(store)

    const rawSparkAddress = 'spark1pqqqqq0000000000000000000000000000recipient'
    await sparkRoute(
      store,
      sparkStore,
      '/api/pylons/pylon.test.one/spark-payout-target',
      {
        body: {
          payoutTargetRef: await sparkDigestRef(rawSparkAddress),
          rawSparkAddress,
          status: 'registered',
        },
        idempotencyKey: 'spark-register-resolve',
        tokenUserId: 'agent-one',
      },
    )

    // register -> private store -> resolve: the resolver returns the raw address
    // that the settlement payout authority will send natively over Spark.
    const resolved = await resolveSparkPayoutDestination(
      sparkStore,
      'pylon.test.one',
    )
    expect(resolved).toBe(rawSparkAddress)
  })

  test('settlement resolver fails closed (undefined) when the recipient has no Spark target (#5252)', async () => {
    const sparkStore = new MemorySparkPayoutTargetStore()

    expect(
      await resolveSparkPayoutDestination(sparkStore, 'pylon.test.none'),
    ).toBeUndefined()
  })

  test('settlement resolver fails closed (undefined) when the store read throws (#5252)', async () => {
    const throwingStore: PylonSparkPayoutTargetStore = {
      read: () => Promise.reject(new Error('store unavailable')),
      readByOwner: () => Promise.reject(new Error('store unavailable')),
      upsert: record => Promise.resolve(record),
    }

    expect(
      await resolveSparkPayoutDestination(throwingStore, 'pylon.test.one'),
    ).toBeUndefined()
  })

  // #5252 owner-scoped canonical fallback. INVARIANTS for
  // resolveSparkPayoutDestination: a contributor whose registered Pylon shows
  // `sparkPayoutTargetReady: true` MUST resolve a destination even when the
  // training-window lease was claimed under a DIFFERENT pylonRef owned by the
  // SAME agent (e.g. a device-ref). The fallback is bound to the owning agent
  // and fails closed everywhere else (no resolver, no owner, no target, throw).
  test('settlement resolver resolves an owner-registered target by a DIFFERENT lease pylonRef of the same agent (#5252)', async () => {
    // Real shape from the bug: target registered under pylon.81f0... but the
    // lease that did the work was claimed under a device-ref.
    const registeredPylonRef = 'pylon.81f0facfe7971870f685'
    const leaseDeviceRef = 'pylon_45b58c56783cbedf2d113a0c'
    const rawSparkAddress =
      'spark1pqqqqq0000000000000000000000000000ownerfallback'

    const sparkStore = new MemorySparkPayoutTargetStore()
    await sparkStore.upsert({
      pylonRef: registeredPylonRef,
      ownerAgentUserId: 'agent_trigger',
      payoutTargetRef: 'payout.spark.deadbeef',
      rawSparkAddress,
      createdAt: '2026-06-16T00:00:00.000Z',
      updatedAt: '2026-06-16T00:00:00.000Z',
    })

    // The lease's device-ref has NO direct target; owner-resolver maps it to
    // the same owning agent that registered the target above.
    const resolveOwner = (pylonRef: string) =>
      Promise.resolve(pylonRef === leaseDeviceRef ? 'agent_trigger' : undefined)

    const resolved = await resolveSparkPayoutDestination(
      sparkStore,
      leaseDeviceRef,
      resolveOwner,
    )
    expect(resolved).toBe(rawSparkAddress)
  })

  test('settlement resolver still fails closed (undefined) with NO owner-resolver and no direct target (#5252)', async () => {
    const sparkStore = new MemorySparkPayoutTargetStore()

    expect(
      await resolveSparkPayoutDestination(
        sparkStore,
        'pylon_45b58c56783cbedf2d113a0c',
      ),
    ).toBeUndefined()
  })

  test('settlement resolver fails closed when the resolved owner has NO target anywhere (#5252)', async () => {
    const sparkStore = new MemorySparkPayoutTargetStore()
    const resolveOwner = () =>
      Promise.resolve<string | undefined>('agent_trigger')

    expect(
      await resolveSparkPayoutDestination(
        sparkStore,
        'pylon_45b58c56783cbedf2d113a0c',
        resolveOwner,
      ),
    ).toBeUndefined()
  })

  test('settlement resolver fails closed when the owner-resolver returns undefined (unknown pylonRef) (#5252)', async () => {
    const sparkStore = new MemorySparkPayoutTargetStore()
    await sparkStore.upsert({
      pylonRef: 'pylon.81f0facfe7971870f685',
      ownerAgentUserId: 'agent_trigger',
      payoutTargetRef: 'payout.spark.deadbeef',
      rawSparkAddress: 'spark1pqqqqq0000000000000000000000000000unknownowner',
      createdAt: '2026-06-16T00:00:00.000Z',
      updatedAt: '2026-06-16T00:00:00.000Z',
    })
    const resolveOwner = () => Promise.resolve<string | undefined>(undefined)

    expect(
      await resolveSparkPayoutDestination(
        sparkStore,
        'pylon_unknown_device_ref',
        resolveOwner,
      ),
    ).toBeUndefined()
  })

  test('settlement resolver: a direct exact-ref match still wins without using the owner fallback (#5252)', async () => {
    const rawSparkAddress = 'spark1pqqqqq0000000000000000000000000000directwins'
    const sparkStore = new MemorySparkPayoutTargetStore()
    await sparkStore.upsert({
      pylonRef: 'pylon.test.one',
      ownerAgentUserId: 'agent_trigger',
      payoutTargetRef: 'payout.spark.cafebabe',
      rawSparkAddress,
      createdAt: '2026-06-16T00:00:00.000Z',
      updatedAt: '2026-06-16T00:00:00.000Z',
    })

    // Owner-resolver would throw if consulted; the direct match must short-circuit.
    const resolveOwner = () =>
      Promise.reject<string | undefined>(
        new Error('owner-resolver must not be consulted on a direct hit'),
      )

    expect(
      await resolveSparkPayoutDestination(
        sparkStore,
        'pylon.test.one',
        resolveOwner,
      ),
    ).toBe(rawSparkAddress)
  })

  test('settlement resolver never crosses agent ownership: a different owner with no target stays undefined (#5252)', async () => {
    // agent_a owns a target; the lease resolves to agent_b who has none.
    const sparkStore = new MemorySparkPayoutTargetStore()
    await sparkStore.upsert({
      pylonRef: 'pylon.agent_a',
      ownerAgentUserId: 'agent_a',
      payoutTargetRef: 'payout.spark.aaaa',
      rawSparkAddress: 'spark1pqqqqq0000000000000000000000000000agentaonly',
      createdAt: '2026-06-16T00:00:00.000Z',
      updatedAt: '2026-06-16T00:00:00.000Z',
    })
    const resolveOwner = () => Promise.resolve<string | undefined>('agent_b')

    expect(
      await resolveSparkPayoutDestination(
        sparkStore,
        'pylon_device_of_agent_b',
        resolveOwner,
      ),
    ).toBeUndefined()
  })

  test('the Pylon payload scanner rejects a raw spark1 address but allows the digest ref (#5252)', () => {
    // Defense in depth: a raw spark1… can never sit in a public Pylon payload,
    // while the redacted payout.spark.<digest> ref is allowed.
    expect(
      pylonApiPayloadHasPrivateMaterial({
        rawSparkAddress: 'spark1pqqqqq0000000000000000000000000000leak',
      }),
    ).toBe(true)
    expect(
      pylonApiPayloadHasPrivateMaterial({
        payoutTargetRef: 'payout.spark.deadbeefdeadbeefdeadbeef',
      }),
    ).toBe(false)
  })

  test('the Pylon payload scanner allows public token-counter refs without allowing raw token fields', () => {
    expect(
      pylonApiPayloadHasPrivateMaterial({
        objectiveSummary:
          'Implement issue #6330 for khala-tokens-served history.',
        sourceRefs: [
          'route:/api/public/khala-tokens-served/history',
          'table.public.token_usage_events',
          'src/public-khala-tokens-served-routes.test.ts',
        ],
      }),
    ).toBe(false)

    expect(
      pylonApiPayloadHasPrivateMaterial({
        token: 'private-token-value',
      }),
    ).toBe(true)
    expect(
      pylonApiPayloadHasPrivateMaterial({
        tokenSecret: 'private-token-value',
      }),
    ).toBe(true)
  })

  test('the Pylon payload scanner does not mistake hydralisk file paths for sk secrets', () => {
    expect(
      pylonApiPayloadHasPrivateMaterial({
        verificationCommand: {
          args: [
            'bun',
            'run',
            '--cwd',
            'apps/openagents.com/workers/api',
            'test',
            '--',
            'src/inference/hydralisk-adapter.test.ts',
          ],
          commandRef: 'command.public.pylon_khala.verify.hydralisk_adapter',
        },
      }),
    ).toBe(false)

    expect(
      pylonApiPayloadHasPrivateMaterial({
        verificationCommand: {
          args: ['OPENAI_API_KEY=sk-testsecret000000000'],
          commandRef: 'command.public.pylon_khala.verify.secret_rejected',
        },
      }),
    ).toBe(true)
  })

  test('agent/pylon projection exposes sparkPayoutTargetReady:false with a null ref when no target is registered (#5306)', async () => {
    const store = new MemoryPylonApiStore()
    const sparkStore = new MemorySparkPayoutTargetStore()
    await registerPylon(store)

    const detail = await responseJson<PylonRouteJson>(
      await sparkRoute(store, sparkStore, '/api/pylons/pylon.test.one', {
        method: 'GET',
      }),
    )

    expect(detail.pylon?.sparkPayoutTargetReady).toBe(false)
    expect(detail.pylon?.sparkPayoutTargetRef).toBeNull()

    const list = await responseJson<PylonRouteJson>(
      await sparkRoute(store, sparkStore, '/api/pylons', { method: 'GET' }),
    )
    expect(list.pylons?.[0]?.sparkPayoutTargetReady).toBe(false)
    expect(list.pylons?.[0]?.sparkPayoutTargetRef).toBeNull()
  })

  test('agent/pylon projection flips sparkPayoutTargetReady:true with the redacted digest ref once a node registers a target — self-heals with no manual step (#5306)', async () => {
    const store = new MemoryPylonApiStore()
    const sparkStore = new MemorySparkPayoutTargetStore()
    await registerPylon(store)

    // Before the node registers, readiness is a visible, self-healing gap.
    const before = await responseJson<PylonRouteJson>(
      await sparkRoute(store, sparkStore, '/api/pylons/pylon.test.one', {
        method: 'GET',
      }),
    )
    expect(before.pylon?.sparkPayoutTargetReady).toBe(false)

    // The node (#5305) auto-registers its OWN raw Spark address.
    const rawSparkAddress = 'spark1pqqqqq0000000000000000000000000000readyflip'
    const payoutTargetRef = await sparkDigestRef(rawSparkAddress)
    await sparkRoute(
      store,
      sparkStore,
      '/api/pylons/pylon.test.one/spark-payout-target',
      {
        body: {
          payoutTargetRef,
          policyRefs: ['policy.public.pylon.redacted_payout_target_only'],
          rawSparkAddress,
          status: 'registered',
        },
        idempotencyKey: 'spark-register-readyflip',
        tokenUserId: 'agent-one',
      },
    )

    // Backstop: the next read recomputes readiness from the live store and the
    // flag flips to ready, carrying only the redacted digest ref.
    const after = await responseJson<PylonRouteJson>(
      await sparkRoute(store, sparkStore, '/api/pylons/pylon.test.one', {
        method: 'GET',
      }),
    )
    expect(after.pylon?.sparkPayoutTargetReady).toBe(true)
    expect(after.pylon?.sparkPayoutTargetRef).toBe(payoutTargetRef)

    // The list projection reflects the same readiness, and never the raw address.
    const list = await responseJson<PylonRouteJson>(
      await sparkRoute(store, sparkStore, '/api/pylons', { method: 'GET' }),
    )
    expect(list.pylons?.[0]?.sparkPayoutTargetReady).toBe(true)
    expect(list.pylons?.[0]?.sparkPayoutTargetRef).toBe(payoutTargetRef)
    expect(JSON.stringify(list)).not.toContain('spark1')
  })

  test('the spark-payout-target register response projects sparkPayoutTargetReady:true without leaking the raw address (#5306)', async () => {
    const store = new MemoryPylonApiStore()
    const sparkStore = new MemorySparkPayoutTargetStore()
    await registerPylon(store)

    const rawSparkAddress =
      'spark1pqqqqq0000000000000000000000000000registerresp'
    const payoutTargetRef = await sparkDigestRef(rawSparkAddress)
    const body = await responseJson<PylonRouteJson>(
      await sparkRoute(
        store,
        sparkStore,
        '/api/pylons/pylon.test.one/spark-payout-target',
        {
          body: {
            payoutTargetRef,
            policyRefs: ['policy.public.pylon.redacted_payout_target_only'],
            rawSparkAddress,
            status: 'registered',
          },
          idempotencyKey: 'spark-register-resp-ready',
          tokenUserId: 'agent-one',
        },
      ),
    )

    expect(body.pylon?.sparkPayoutTargetReady).toBe(true)
    expect(body.pylon?.sparkPayoutTargetRef).toBe(payoutTargetRef)
    expect(JSON.stringify(body)).not.toContain('spark1')
  })

  test('readiness fails closed to false when the spark store dependency is not wired (#5306)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)

    // No spark store wired: readiness must be a visible gap, not an error.
    const detail = await responseJson<PylonRouteJson>(
      await sparkRoute(store, undefined, '/api/pylons/pylon.test.one', {
        method: 'GET',
      }),
    )
    expect(detail.pylon?.sparkPayoutTargetReady).toBe(false)
    expect(detail.pylon?.sparkPayoutTargetRef).toBeNull()
  })

  test('readiness resolver fails closed (not ready) when the store read throws (#5306)', async () => {
    const throwingStore: PylonSparkPayoutTargetStore = {
      read: () => Promise.reject(new Error('store unavailable')),
      readByOwner: () => Promise.reject(new Error('store unavailable')),
      upsert: record => Promise.resolve(record),
    }

    expect(
      await resolveSparkPayoutTargetReadiness(throwingStore, 'pylon.test.one'),
    ).toEqual({ ready: false, ref: null })
  })

  test('readiness resolver fails closed (not ready) when a stored row has a malformed digest ref or empty raw address (#5306)', async () => {
    const sparkStore = new MemorySparkPayoutTargetStore()

    // Malformed (non payout.spark.* shaped) ref must never project as ready.
    await sparkStore.upsert({
      pylonRef: 'pylon.malformed',
      ownerAgentUserId: 'agent-one',
      payoutTargetRef: 'not-a-spark-digest',
      rawSparkAddress: 'spark1pqqqqq0000000000000000000000000000malformed',
      createdAt: '2026-06-07T00:00:00.000Z',
      updatedAt: '2026-06-07T00:00:00.000Z',
    })
    expect(
      await resolveSparkPayoutTargetReadiness(sparkStore, 'pylon.malformed'),
    ).toEqual({ ready: false, ref: null })

    // A row with an empty raw address (no node-registered target) is not ready.
    await sparkStore.upsert({
      pylonRef: 'pylon.emptyraw',
      ownerAgentUserId: 'agent-one',
      payoutTargetRef: 'payout.spark.deadbeefdeadbeefdeadbeef',
      rawSparkAddress: '   ',
      createdAt: '2026-06-07T00:00:00.000Z',
      updatedAt: '2026-06-07T00:00:00.000Z',
    })
    expect(
      await resolveSparkPayoutTargetReadiness(sparkStore, 'pylon.emptyraw'),
    ).toEqual({ ready: false, ref: null })
  })

  test('readiness resolver returns ready with the redacted digest ref for a properly registered target (#5306)', async () => {
    const sparkStore = new MemorySparkPayoutTargetStore()
    const rawSparkAddress = 'spark1pqqqqq0000000000000000000000000000resolverok'
    const payoutTargetRef = await sparkDigestRef(rawSparkAddress)
    await sparkStore.upsert({
      pylonRef: 'pylon.ready',
      ownerAgentUserId: 'agent-one',
      payoutTargetRef,
      rawSparkAddress,
      createdAt: '2026-06-07T00:00:00.000Z',
      updatedAt: '2026-06-07T00:00:00.000Z',
    })

    const readiness = await resolveSparkPayoutTargetReadiness(
      sparkStore,
      'pylon.ready',
    )
    expect(readiness.ready).toBe(true)
    expect(readiness.ref).toBe(payoutTargetRef)
    // The redacted ref is a digest, never the raw spark1… address.
    expect(readiness.ref).not.toContain('spark1')
  })

  test('blocks controlled assignment dispatch to missing or offline Pylons', async () => {
    const store = new MemoryPylonApiStore()
    const missing = await createAssignment(store, {
      assignmentRef: 'assignment.public.missing_pylon',
      idempotencyKey: 'assignment-missing-pylon',
      pylonRef: 'pylon.test.missing',
    })
    await registerPylon(store)
    await markWalletReady(store)
    const offline = await createAssignment(store, {
      assignmentRef: 'assignment.public.offline_pylon',
      idempotencyKey: 'assignment-offline-pylon',
    })
    const missingBody = await responseJson<PylonRouteJson>(missing)
    const offlineBody = await responseJson<PylonRouteJson>(offline)

    expect(missing.status).toBe(409)
    expect(missingBody.error).toBe('controlled_dispatch_gate_blocked')
    expect(missingBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.pylon_missing',
    )
    expect(offline.status).toBe(409)
    expect(offlineBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.pylon_offline',
    )
  })

  test('blocks controlled assignment dispatch for stale, paused, and wrong-capability Pylons', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    await markOnline(store, { nowIso: '2026-06-07T00:00:00.000Z' })
    await markWalletReady(store)
    const stale = await createAssignment(store, {
      assignmentRef: 'assignment.public.stale_pylon',
      idempotencyKey: 'assignment-stale-pylon',
    })
    await markOnline(store)
    const paused = await createAssignment(store, {
      assignmentRef: 'assignment.public.paused_campaign',
      campaignPaused: true,
      idempotencyKey: 'assignment-paused-campaign',
    })
    const wrongCapability = await createAssignment(store, {
      assignmentRef: 'assignment.public.wrong_capability',
      idempotencyKey: 'assignment-wrong-capability',
      requiredCapabilityRefs: ['capability.public.training'],
    })
    const staleBody = await responseJson<PylonRouteJson>(stale)
    const pausedBody = await responseJson<PylonRouteJson>(paused)
    const wrongCapabilityBody =
      await responseJson<PylonRouteJson>(wrongCapability)

    expect(stale.status).toBe(409)
    expect(staleBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.pylon_stale',
    )
    expect(paused.status).toBe(409)
    expect(pausedBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.campaign_paused',
    )
    expect(wrongCapability.status).toBe(409)
    expect(wrongCapabilityBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.wrong_capability',
    )
  })

  test('blocks duplicate dispatches and paid modes without spend-cap refs', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    await markOnline(store)
    await markWalletReady(store)
    const first = await createAssignment(store, {
      assignmentRef: 'assignment.public.active_one',
      idempotencyKey: 'assignment-active-one',
    })
    const duplicate = await createAssignment(store, {
      assignmentRef: 'assignment.public.active_two',
      idempotencyKey: 'assignment-active-two',
    })
    await registerPylon(store, {
      idempotencyKey: 'register-pylon-test-two',
      pylonRef: 'pylon.test.two',
      tokenUserId: 'agent-two',
    })
    await markOnline(store, {
      pylonRef: 'pylon.test.two',
      tokenUserId: 'agent-two',
    })
    await markWalletReady(store, 'pylon.test.two', 'agent-two')
    const paidWithoutSpendCap = await createAssignment(store, {
      assignmentRef: 'assignment.public.paid_without_cap',
      idempotencyKey: 'assignment-paid-without-cap',
      paymentMode: 'payable_pending_settlement',
      pylonRef: 'pylon.test.two',
    })
    const duplicateBody = await responseJson<PylonRouteJson>(duplicate)
    const paidWithoutSpendCapBody =
      await responseJson<PylonRouteJson>(paidWithoutSpendCap)

    expect(first.status).toBe(201)
    expect(duplicate.status).toBe(409)
    expect(duplicateBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
    expect(paidWithoutSpendCap.status).toBe(409)
    expect(paidWithoutSpendCapBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.paid_mode_missing_spend_cap',
    )
  })

  test('allows non-admin agents to dispatch no-spend Codex assignments only to their own Pylon (#6382)', async () => {
    const store = new MemoryPylonApiStore()
    const pylonRef = 'pylon.owner.codex'
    const requiredCapabilityRefs = ['capability.public.codex_agent_task']
    await registerPylon(store, {
      capabilityRefs: requiredCapabilityRefs,
      idempotencyKey: 'register-owner-codex-pylon',
      pylonRef,
      tokenUserId: 'agent-owner',
    })
    await markOnline(store, {
      pylonRef,
      tokenUserId: 'agent-owner',
    })
    await markWalletReady(store, pylonRef, 'agent-owner')

    const ownerDispatch = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_owner',
      idempotencyKey: 'assignment-owner-codex',
      jobKind: 'codex_agent_task',
      pylonRef,
      requiredCapabilityRefs,
      tokenUserId: 'agent-owner',
    })
    const ownerReplay = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_owner',
      idempotencyKey: 'assignment-owner-codex',
      jobKind: 'codex_agent_task',
      pylonRef,
      requiredCapabilityRefs,
      tokenUserId: 'agent-owner',
    })
    const crossTenantDispatch = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_cross_tenant',
      idempotencyKey: 'assignment-cross-tenant-codex',
      jobKind: 'codex_agent_task',
      pylonRef,
      requiredCapabilityRefs,
      tokenUserId: 'agent-other',
    })
    const crossTenantList = await route(
      store,
      `/api/pylons/${pylonRef}/assignments`,
      {
        tokenUserId: 'agent-other',
      },
    )
    const ownerBody = await responseJson<PylonRouteJson>(ownerDispatch)
    const replayBody = await responseJson<PylonRouteJson>(ownerReplay)
    const crossTenantDispatchBody =
      await responseJson<PylonRouteJson>(crossTenantDispatch)
    const crossTenantListBody =
      await responseJson<PylonRouteJson>(crossTenantList)

    expect(ownerDispatch.status).toBe(201)
    expect(ownerBody.assignment?.state).toBe('offered')
    expect(ownerBody.dispatchGate?.dispatchAllowed).toBe(true)
    expect(ownerBody.dispatchGate?.noSpendDispatch).toBe(true)
    expect(ownerBody.dispatchGate?.walletSpendAllowed).toBe(false)
    expect(ownerBody.dispatchGate?.settlementMutationAllowed).toBe(false)
    expect(ownerBody.dispatchGate?.forumAutoPublishAllowed).toBe(false)
    expect(ownerReplay.status).toBe(200)
    expect(replayBody.idempotent).toBe(true)
    expect(crossTenantDispatch.status).toBe(403)
    expect(crossTenantDispatchBody.error).toBe('pylon_api_forbidden')
    expect(crossTenantList.status).toBe(403)
    expect(crossTenantListBody.error).toBe('pylon_api_forbidden')
  })

  test('does not let expired active leases consume future dispatch capacity', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    await markOnline(store)
    await markWalletReady(store)
    const expired = await createAssignment(store, {
      assignmentRef: 'assignment.public.expired_active_one',
      idempotencyKey: 'assignment-expired-active-one',
      leaseSeconds: 60,
      nowIso: '2026-06-07T00:10:00.000Z',
    })
    const fresh = await createAssignment(store, {
      assignmentRef: 'assignment.public.after_expired_active_two',
      idempotencyKey: 'assignment-after-expired-active-two',
      nowIso: '2026-06-07T00:11:01.000Z',
    })
    const freshBody = await responseJson<PylonRouteJson>(fresh)

    expect(expired.status).toBe(201)
    expect(fresh.status).toBe(201)
    expect(freshBody.dispatchGate?.dispatchAllowed).toBe(true)
    expect(freshBody.dispatchGate?.blockerRefs).not.toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
  })

  test('sweeps silent active leases before dispatch capacity accounting (#6410)', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    await markOnline(store)
    await markWalletReady(store)
    const staleRunning = await createAssignment(store, {
      assignmentRef: 'assignment.public.silent_running_one',
      idempotencyKey: 'assignment-silent-running-one',
      nowIso: '2026-06-07T00:10:00.000Z',
    })
    expect(staleRunning.status).toBe(201)
    const staleRecord = store.assignments.get(
      'assignment.public.silent_running_one',
    )
    expect(staleRecord).toBeDefined()
    if (staleRecord === undefined) {
      throw new Error('expected seeded assignment')
    }
    const runningRecord = {
      ...staleRecord,
      leaseExpiresAt: '2026-06-07T01:10:00.000Z',
      state: 'running' as const,
      updatedAt: '2026-06-07T00:09:59.000Z',
    }
    store.assignments.set(runningRecord.assignmentRef, runningRecord)
    store.assignmentsByIdempotency.set(
      runningRecord.idempotencyKeyHash,
      runningRecord,
    )

    const next = await createAssignment(store, {
      assignmentRef: 'assignment.public.after_silent_running_two',
      idempotencyKey: 'assignment-after-silent-running-two',
      nowIso: '2026-06-07T00:15:00.000Z',
    })
    const nextBody = await responseJson<PylonRouteJson>(next)

    expect(next.status).toBe(201)
    expect(nextBody.dispatchGate?.dispatchAllowed).toBe(true)
    expect(nextBody.dispatchGate?.blockerRefs).not.toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
    expect(
      store.assignments.get('assignment.public.silent_running_one')?.state,
    ).toBe('stale')
  })

  test('allows unpaid smoke dispatch without wallet readiness', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    await markOnline(store)

    const create = await createAssignment(store, {
      assignmentRef: 'assignment.public.unpaid_without_wallet',
      idempotencyKey: 'assignment-unpaid-without-wallet',
    })
    const body = await responseJson<PylonRouteJson>(create)

    expect(create.status).toBe(201)
    expect(body.dispatchGate?.dispatchAllowed).toBe(true)
    expect(body.dispatchGate?.noSpendDispatch).toBe(true)
    expect(body.dispatchGate?.walletSpendAllowed).toBe(false)
    expect(body.dispatchGate?.blockerRefs).not.toContain(
      'blocker.public.pylon_dispatch.wallet_not_ready',
    )
  })

  test('allows parallel coding dispatches up to advertised ready Codex slots', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, {
      capabilityRefs: ['capability.pylon.local_codex'],
    })
    await markOnline(store, {
      capacityRefs: [
        'capacity.coding.codex.ready=2',
        'capacity.coding.codex.available=2',
      ],
      loadRefs: ['load.coding.codex.busy=0', 'load.coding.codex.queued=0'],
    })
    await markWalletReady(store)
    const first = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_parallel_one',
      idempotencyKey: 'assignment-codex-parallel-one',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    })
    const second = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_parallel_two',
      idempotencyKey: 'assignment-codex-parallel-two',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    })
    const third = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_parallel_three',
      idempotencyKey: 'assignment-codex-parallel-three',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    })
    const thirdBody = await responseJson<PylonRouteJson>(third)

    expect(first.status).toBe(201)
    expect(second.status).toBe(201)
    expect(third.status).toBe(409)
    expect(thirdBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
  })

  test('#6388: a saturated Codex lane does not block an available Claude lease on the same Pylon', () => {
    const nowIso = '2026-06-27T12:00:00.000Z'
    const leaseExpiresAt = '2026-06-27T12:30:00.000Z'
    const registration = {
      capabilityRefs: [
        'capability.pylon.local_codex',
        'capability.pylon.local_claude_agent',
      ],
      clientVersion: '0.3.0',
      latestCapacityRefs: [
        'capacity.coding.codex.ready=6',
        'capacity.coding.codex.available=0',
        'capacity.coding.claude.ready=3',
        'capacity.coding.claude.available=3',
      ],
      latestHeartbeatAt: nowIso,
      latestHeartbeatStatus: 'online',
      latestLoadRefs: [
        'load.coding.codex.busy=6',
        'load.coding.claude.busy=0',
      ],
      status: 'active',
      walletReady: true,
    } as unknown as PylonApiRegistrationRecord
    // Six active Codex leases fully saturate the Codex lane.
    const activeCodexAssignments = Array.from({ length: 6 }, (_, index) =>
      ({
        assignmentRef: `assignment.public.codex_busy_${index}`,
        codingAssignment: { codex: { agentKind: 'codex_sdk' } },
        jobKind: 'codex_agent_task',
        leaseExpiresAt,
        state: 'running',
      }) as unknown as PylonApiAssignmentRecord,
    )
    const claudeBody = {
      campaignPaused: false,
      campaignPolicyRefs: ['policy.public.khala_coding.own_capacity_only'],
      campaignRef: 'campaign.public.khala_coding.own_capacity',
      closeoutPathRefs: ['closeout.public.khala_coding.durable_stream'],
      codingAssignment: { claudeAgent: { agentKind: 'claude_agent_sdk' } },
      forumAutoPublishAllowed: false,
      idempotencyRefs: ['idempotency.public.khala_coding.request'],
      jobKind: 'claude_agent_task' as const,
      noDuplicateAssignmentRefs: ['dedupe.public.pylon_assignment.active_lease'],
      noForumAutoPublishRefs: ['policy.public.no_forum_auto_publish'],
      operatorPauseRefs: ['pause.public.khala_coding.kill_switch_default_off'],
      paymentMode: 'unpaid_smoke',
      pylonRef: 'pylon.test.one',
      requiredCapabilityRefs: ['capability.pylon.local_claude_agent'],
      resultExpectationRefs: ['result.public.khala_coding.worker_closeout'],
      rollbackRefs: ['rollback.public.khala_coding.assignment_cancel'],
      selectionPolicyRefs: ['selection.public.khala_coding.claude_first'],
      spendCapRefs: [],
      taskRefs: ['task.public.khala_coding.claude'],
    } as unknown as Parameters<
      typeof controlledPylonAssignmentDispatchGate
    >[0]['body']

    const claudeGate = controlledPylonAssignmentDispatchGate({
      activeAssignments: activeCodexAssignments,
      assignmentRef: 'assignment.public.claude_one',
      body: claudeBody,
      nowIso,
      registration,
    })
    expect(claudeGate.blockerRefs).not.toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
    expect(claudeGate.dispatchAllowed).toBe(true)

    // A Codex request against the same saturated lane is still correctly blocked.
    const codexGate = controlledPylonAssignmentDispatchGate({
      activeAssignments: activeCodexAssignments,
      assignmentRef: 'assignment.public.codex_one',
      body: {
        ...claudeBody,
        codingAssignment: { codex: { agentKind: 'codex_sdk' } },
        jobKind: 'codex_agent_task',
        requiredCapabilityRefs: ['capability.pylon.local_codex'],
      },
      nowIso,
      registration,
    })
    expect(codexGate.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
  })

  test('#6386: stale over-admitted `offered` leases do not deadlock a per-account Codex lane that advertises free capacity', () => {
    const nowIso = '2026-06-27T17:00:00.000Z'
    const accountKey = '651c03fed68925d7acb2c02f'
    const accountRefHash = `account.pylon.codex.${accountKey}`
    // Heartbeat advertises real free per-account capacity (busy counts only
    // RUNNING work, so the orphaned `offered` backlog is invisible to it).
    const registration = {
      capabilityRefs: ['capability.pylon.local_codex'],
      clientVersion: '0.3.0',
      latestCapacityRefs: [
        `capacity.coding.codex.account.${accountKey}.ready=8`,
        `capacity.coding.codex.account.${accountKey}.available=8`,
      ],
      latestHeartbeatAt: nowIso,
      latestHeartbeatStatus: 'online',
      latestLoadRefs: [`load.coding.codex.account.${accountKey}.busy=0`],
      status: 'active',
      walletReady: true,
    } as unknown as PylonApiRegistrationRecord
    // 11 `offered` leases for this account, created/updated ~40 minutes ago and
    // never claimed (over-admitted by a concurrent burst), each still holding a
    // 1-hour lease. This is well above the per-account ceiling of 8.
    const staleOffered = Array.from(
      { length: 11 },
      (_, index) =>
        ({
          assignmentRef: `assignment.public.stale_offered_${index}`,
          codingAssignment: {
            codex: { agentKind: 'codex_sdk', accountRefHash },
          },
          createdAt: '2026-06-27T16:20:00.000Z',
          updatedAt: '2026-06-27T16:20:00.000Z',
          jobKind: 'codex_agent_task',
          leaseExpiresAt: '2026-06-27T17:20:00.000Z',
          state: 'offered',
        }) as unknown as PylonApiAssignmentRecord,
    )
    const codexBody = {
      campaignPaused: false,
      campaignPolicyRefs: ['policy.public.khala_coding.own_capacity_only'],
      campaignRef: 'campaign.public.khala_coding.own_capacity',
      closeoutPathRefs: ['closeout.public.khala_coding.durable_stream'],
      codingAssignment: { codex: { agentKind: 'codex_sdk', accountRefHash } },
      forumAutoPublishAllowed: false,
      idempotencyRefs: ['idempotency.public.khala_coding.request'],
      jobKind: 'codex_agent_task' as const,
      noDuplicateAssignmentRefs: ['dedupe.public.pylon_assignment.active_lease'],
      noForumAutoPublishRefs: ['policy.public.no_forum_auto_publish'],
      operatorPauseRefs: ['pause.public.khala_coding.kill_switch_default_off'],
      paymentMode: 'unpaid_smoke',
      pylonRef: 'pylon.test.one',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
      resultExpectationRefs: ['result.public.khala_coding.worker_closeout'],
      rollbackRefs: ['rollback.public.khala_coding.assignment_cancel'],
      selectionPolicyRefs: ['selection.public.khala_coding.codex_first'],
      spendCapRefs: [],
      taskRefs: ['task.public.khala_coding.codex'],
    } as unknown as Parameters<
      typeof controlledPylonAssignmentDispatchGate
    >[0]['body']

    const staleGate = controlledPylonAssignmentDispatchGate({
      activeAssignments: staleOffered,
      assignmentRef: 'assignment.public.codex_new',
      body: codexBody,
      nowIso,
      registration,
    })
    expect(staleGate.blockerRefs).not.toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
    expect(staleGate.dispatchAllowed).toBe(true)

    // Fresh `offered` leases (claimed-imminent) still throttle: 8 fresh offered
    // == the per-account ceiling, so the 9th is correctly refused.
    const freshOffered = Array.from(
      { length: 8 },
      (_, index) =>
        ({
          assignmentRef: `assignment.public.fresh_offered_${index}`,
          codingAssignment: {
            codex: { agentKind: 'codex_sdk', accountRefHash },
          },
          createdAt: nowIso,
          updatedAt: nowIso,
          jobKind: 'codex_agent_task',
          leaseExpiresAt: '2026-06-27T18:00:00.000Z',
          state: 'offered',
        }) as unknown as PylonApiAssignmentRecord,
    )
    const freshGate = controlledPylonAssignmentDispatchGate({
      activeAssignments: freshOffered,
      assignmentRef: 'assignment.public.codex_new_two',
      body: codexBody,
      nowIso,
      registration,
    })
    expect(freshGate.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
  })

  test('does not treat remaining available Codex slots as the total parallel ceiling', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, {
      capabilityRefs: ['capability.pylon.local_codex'],
    })
    await markOnline(store, {
      capacityRefs: [
        'capacity.coding.codex.ready=4',
        'capacity.coding.codex.available=2',
      ],
      loadRefs: ['load.coding.codex.busy=2', 'load.coding.codex.queued=0'],
    })
    await markWalletReady(store)
    const first = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_busy_parallel_one',
      idempotencyKey: 'assignment-codex-busy-parallel-one',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    })
    const second = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_busy_parallel_two',
      idempotencyKey: 'assignment-codex-busy-parallel-two',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    })
    const third = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_busy_parallel_three',
      idempotencyKey: 'assignment-codex-busy-parallel-three',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    })
    const fourth = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_busy_parallel_four',
      idempotencyKey: 'assignment-codex-busy-parallel-four',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    })
    const fifth = await createAssignment(store, {
      assignmentRef: 'assignment.public.codex_busy_parallel_five',
      idempotencyKey: 'assignment-codex-busy-parallel-five',
      requiredCapabilityRefs: ['capability.pylon.local_codex'],
    })
    const fifthBody = await responseJson<PylonRouteJson>(fifth)

    expect(first.status).toBe(201)
    expect(second.status).toBe(201)
    expect(third.status).toBe(201)
    expect(fourth.status).toBe(201)
    expect(fifth.status).toBe(409)
    expect(fifthBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.duplicate_active_assignment',
    )
  })

  test('blocks missing dispatcher guard refs and automatic Forum publishing', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    await markOnline(store)
    await markWalletReady(store)
    const missingPolicy = await createAssignment(store, {
      assignmentRef: 'assignment.public.missing_policy',
      campaignPolicyRefs: [],
      idempotencyKey: 'assignment-missing-policy',
    })
    const forumPublish = await createAssignment(store, {
      assignmentRef: 'assignment.public.forum_publish',
      forumAutoPublishAllowed: true,
      idempotencyKey: 'assignment-forum-publish',
    })
    const missingPolicyBody = await responseJson<PylonRouteJson>(missingPolicy)
    const forumPublishBody = await responseJson<PylonRouteJson>(forumPublish)

    expect(missingPolicy.status).toBe(409)
    expect(missingPolicyBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.campaign_policy_missing',
    )
    expect(forumPublish.status).toBe(409)
    expect(forumPublishBody.dispatchGate?.blockerRefs).toContain(
      'blocker.public.pylon_dispatch.forum_auto_publish_requested',
    )
  })

  test('leases, accepts, runs, proves, and closes accepted Pylon work', async () => {
    const store = new MemoryPylonApiStore()
    const assignmentRef = 'assignment.public.issue502.echo'
    const readAssignmentLeaseExpiresAt = async () => {
      const assignment = await store.readAssignment(assignmentRef)
      expect(assignment).toBeDefined()

      return assignment?.leaseExpiresAt ?? ''
    }

    await registerPylon(store)
    await markOnline(store)
    await markWalletReady(store)

    const create = await createAssignment(store)
    const replay = await createAssignment(store)
    const list = await route(store, '/api/pylons/pylon.test.one/assignments', {
      tokenUserId: 'agent-one',
    })
    const createdLeaseExpiresAt = await readAssignmentLeaseExpiresAt()
    const accept = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/accept',
      {
        body: {
          acceptanceRefs: ['acceptance.public.echo_ready'],
          accepted: true,
        },
        idempotencyKey: 'accept-echo',
        method: 'POST',
        nowIso: '2026-06-07T00:10:30.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const acceptedLeaseExpiresAt = await readAssignmentLeaseExpiresAt()
    const acceptReplay = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/accept',
      {
        body: {
          acceptanceRefs: ['acceptance.public.echo_ready'],
          accepted: true,
        },
        idempotencyKey: 'accept-echo',
        method: 'POST',
        nowIso: '2026-06-07T00:10:30.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const duplicateAccept = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/accept',
      {
        body: {
          acceptanceRefs: ['acceptance.public.echo_ready_duplicate'],
          accepted: true,
        },
        idempotencyKey: 'accept-echo-duplicate-claim',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const progress = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/progress',
      {
        body: {
          progressPercent: 50,
          progressRefs: ['progress.public.echo_halfway'],
          status: 'running',
        },
        idempotencyKey: 'progress-echo',
        method: 'POST',
        nowIso: '2026-06-07T00:21:00.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const progressLeaseExpiresAt = await readAssignmentLeaseExpiresAt()
    const artifacts = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/artifacts',
      {
        body: {
          artifactRefs: ['artifact.public.echo_manifest'],
          proofRefs: ['proof.public.echo_result'],
        },
        idempotencyKey: 'artifact-echo',
        method: 'POST',
        nowIso: '2026-06-07T00:35:00.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const artifactsLeaseExpiresAt = await readAssignmentLeaseExpiresAt()
    const workerCloseout = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/closeout',
      {
        body: {
          artifactRefs: ['artifact.public.echo_manifest'],
          buildRefs: ['build.public.echo_not_required'],
          closeoutRefs: ['closeout.public.worker_echo_summary'],
          proofRefs: ['proof.public.echo_result'],
          resultRefs: ['result.public.echo_summary'],
          status: 'closeout_submitted',
          testRefs: ['test.public.echo_not_required'],
        },
        idempotencyKey: 'worker-closeout-echo',
        method: 'POST',
        nowIso: '2026-06-07T00:36:00.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const workerCloseoutLeaseExpiresAt = await readAssignmentLeaseExpiresAt()
    const workerCloseoutBody =
      await responseJson<PylonRouteJson>(workerCloseout)
    const listAfterWorkerCloseout = await route(
      store,
      '/api/pylons/pylon.test.one/assignments',
      {
        tokenUserId: 'agent-one',
      },
    )
    const closeout = await route(
      store,
      '/api/operator/pylons/assignments/assignment.public.issue502.echo/closeout',
      {
        adminToken: true,
        body: {
          accepted: true,
          acceptedWorkRefs: ['accepted_work.public.echo_result'],
          closeoutRefs: ['closeout.public.operator_reviewed_echo'],
        },
        method: 'POST',
      },
    )
    const paymentReceipt = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/payment-receipts',
      {
        body: {
          paymentProofRefs: ['payment_proof.public.accepted_work'],
          receiptRefs: ['receipt.public.accepted_work'],
        },
        idempotencyKey: 'payment-after-closeout',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const progressAfterCloseout = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/progress',
      {
        body: {
          progressRefs: ['progress.public.should_not_mutate'],
        },
        idempotencyKey: 'progress-after-closeout',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const createBody = await responseJson<PylonRouteJson>(create)
    const replayBody = await responseJson<PylonRouteJson>(replay)
    const listBody = await responseJson<PylonRouteJson>(list)
    const acceptBody = await responseJson<PylonRouteJson>(accept)
    const acceptReplayBody = await responseJson<PylonRouteJson>(acceptReplay)
    const duplicateAcceptBody =
      await responseJson<PylonRouteJson>(duplicateAccept)
    const progressBody = await responseJson<PylonRouteJson>(progress)
    const artifactsBody = await responseJson<PylonRouteJson>(artifacts)
    const listAfterWorkerCloseoutBody = await responseJson<PylonRouteJson>(
      listAfterWorkerCloseout,
    )
    const closeoutBody = await responseJson<PylonRouteJson>(closeout)
    const paymentReceiptBody =
      await responseJson<PylonRouteJson>(paymentReceipt)
    const progressAfterCloseoutBody = await responseJson<PylonRouteJson>(
      progressAfterCloseout,
    )

    expect(create.status).toBe(201)
    expect(createBody.assignment?.state).toBe('offered')
    expect(createBody.dispatchGate?.dispatchAllowed).toBe(true)
    expect(createBody.dispatchGate?.noSpendDispatch).toBe(true)
    expect(createBody.dispatchGate?.walletSpendAllowed).toBe(false)
    expect(createBody.dispatchGate?.settlementMutationAllowed).toBe(false)
    expect(createBody.dispatchGate?.forumAutoPublishAllowed).toBe(false)
    expect(replay.status).toBe(200)
    expect(replayBody.idempotent).toBe(true)
    expect(listBody.assignments?.[0]?.assignmentRef).toBe(
      'assignment.public.issue502.echo',
    )
    expect(accept.status).toBe(201)
    expect(acceptBody.assignment?.state).toBe('accepted')
    expect(acceptReplay.status).toBe(200)
    expect(acceptReplayBody.idempotent).toBe(true)
    expect(duplicateAccept.status).toBe(409)
    expect(duplicateAcceptBody.error).toBe('pylon_api_conflict')
    expect(progress.status).toBe(201)
    expect(progressBody.assignment?.state).toBe('running')
    expect(Date.parse(acceptedLeaseExpiresAt)).toBeGreaterThan(
      Date.parse(createdLeaseExpiresAt),
    )
    expect(Date.parse(progressLeaseExpiresAt)).toBeGreaterThan(
      Date.parse(acceptedLeaseExpiresAt),
    )
    expect(artifacts.status).toBe(201)
    expect(artifactsBody.assignment?.state).toBe('proof_submitted')
    expect(Date.parse(artifactsLeaseExpiresAt)).toBeGreaterThan(
      Date.parse(progressLeaseExpiresAt),
    )
    expect(workerCloseout.status).toBe(201)
    expect(workerCloseoutLeaseExpiresAt).toBe('2026-06-07T00:36:00.000Z')
    expect(workerCloseoutBody.assignment).toMatchObject({
      acceptedWorkRefs: [],
      closeoutRefs: ['closeout.public.worker_echo_summary'],
      leaseState: 'terminal',
      state: 'closeout_submitted',
    })
    expect(listAfterWorkerCloseout.status).toBe(200)
    expect(listAfterWorkerCloseoutBody.assignments).toEqual([])
    expect(closeout.status).toBe(200)
    expect(closeoutBody.assignment?.state).toBe('accepted_work')
    expect(paymentReceipt.status).toBe(201)
    expect(paymentReceiptBody.assignment?.state).toBe('accepted_work')
    expect(progressAfterCloseout.status).toBe(409)
    expect(progressAfterCloseoutBody.error).toBe('pylon_api_conflict')
  })

  test('#7921: runtime progress renews long-running assignment leases through worker closeout', async () => {
    const store = new MemoryPylonApiStore()
    const assignmentRef = 'assignment.public.issue7921.long_running'
    const readAssignmentLeaseExpiresAt = async () => {
      const assignment = await store.readAssignment(assignmentRef)
      expect(assignment).toBeDefined()

      return assignment?.leaseExpiresAt ?? ''
    }

    await registerPylon(store)
    await markOnline(store, { nowIso: '2026-06-07T00:00:00.000Z' })
    await markWalletReady(store)

    const create = await createAssignment(store, {
      assignmentRef,
      idempotencyKey: 'assignment-create-issue7921-long-running',
      leaseSeconds: 900,
      nowIso: '2026-06-07T00:00:00.000Z',
    })
    const accept = await route(
      store,
      `/api/pylons/pylon.test.one/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.issue7921.long_running'],
          accepted: true,
        },
        idempotencyKey: 'accept-issue7921-long-running',
        method: 'POST',
        nowIso: '2026-06-07T00:00:10.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const acceptedLeaseExpiresAt = await readAssignmentLeaseExpiresAt()
    const progress = await route(
      store,
      `/api/pylons/pylon.test.one/assignments/${assignmentRef}/progress`,
      {
        body: {
          elapsedMs: 860_000,
          phase: 'running',
          progressRefs: ['progress.public.issue7921.runtime_heartbeat'],
          status: 'running',
        },
        idempotencyKey: 'progress-issue7921-runtime-heartbeat',
        method: 'POST',
        nowIso: '2026-06-07T00:14:30.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const progressLeaseExpiresAt = await readAssignmentLeaseExpiresAt()
    const workerCloseout = await route(
      store,
      `/api/pylons/pylon.test.one/assignments/${assignmentRef}/closeout`,
      {
        body: {
          artifactRefs: ['artifact.public.issue7921.long_running'],
          buildRefs: ['build.public.issue7921.verified'],
          closeoutRefs: ['closeout.public.issue7921.long_running'],
          proofRefs: ['proof.public.issue7921.verified'],
          resultRefs: ['result.public.issue7921.done'],
          status: 'closeout_submitted',
          testRefs: ['test.public.issue7921.verify_green'],
        },
        idempotencyKey: 'worker-closeout-issue7921-long-running',
        method: 'POST',
        nowIso: '2026-06-07T00:28:30.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const workerCloseoutBody =
      await responseJson<PylonRouteJson>(workerCloseout)

    expect(create.status).toBe(201)
    expect(accept.status).toBe(201)
    expect(progress.status).toBe(201)
    expect(Date.parse(progressLeaseExpiresAt)).toBeGreaterThan(
      Date.parse(acceptedLeaseExpiresAt),
    )
    expect(workerCloseout.status).toBe(201)
    expect(workerCloseoutBody.assignment).toMatchObject({
      closeoutRefs: ['closeout.public.issue7921.long_running'],
      leaseState: 'terminal',
      state: 'closeout_submitted',
    })
  })

  test('records a settled paid GEPA-style assignment receipt after accepted closeout', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store, {
      capabilityRefs: ['capability.public.probe_gepa_metric_call'],
    })
    await markOnline(store, {
      capacityRefs: ['capacity.public.probe_gepa_metric_call.available=1'],
    })
    await markWalletReady(store)

    const assignmentRef =
      'assignment.public.pylon_gepa_metric_call.stage0.paid_settlement'
    const create = await createAssignment(store, {
      assignmentRef,
      campaignPolicyRefs: ['policy.public.probe_gepa.paid_settlement'],
      idempotencyKey: 'assignment-gepa-paid-settlement',
      paymentMode: 'settled_bitcoin',
      requiredCapabilityRefs: ['capability.public.probe_gepa_metric_call'],
      spendCapRefs: ['spend_cap.public.probe_gepa.one_sat'],
    })
    const createBody = await responseJson<PylonRouteJson>(create)

    expect(create.status).toBe(201)
    expect(createBody.assignment?.paymentMode).toBe('settled_bitcoin')
    expect(createBody.dispatchGate?.paymentMode).toBe('settled_bitcoin')

    await route(
      store,
      `/api/pylons/pylon.test.one/assignments/${assignmentRef}/accept`,
      {
        body: {
          acceptanceRefs: ['acceptance.public.probe_gepa.worker_claimed'],
          accepted: true,
        },
        idempotencyKey: 'accept-gepa-paid-settlement',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    await route(
      store,
      `/api/pylons/pylon.test.one/assignments/${assignmentRef}/artifacts`,
      {
        body: {
          artifactRefs: ['artifact.public.probe_gepa.metric_call.bundle'],
          proofRefs: ['proof.public.probe_gepa.metric_call.verifier_passed'],
        },
        idempotencyKey: 'artifacts-gepa-paid-settlement',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    await route(
      store,
      `/api/pylons/pylon.test.one/assignments/${assignmentRef}/closeout`,
      {
        body: {
          artifactRefs: ['artifact.public.probe_gepa.metric_call.bundle'],
          closeoutRefs: ['closeout.public.probe_gepa.metric_call.accepted'],
          proofRefs: ['proof.public.probe_gepa.metric_call.verifier_passed'],
          resultRefs: ['result.public.probe_gepa.metric_call.completed'],
          status: 'closeout_submitted',
          testRefs: ['test.public.probe_gepa.metric_call.passed'],
        },
        idempotencyKey: 'worker-closeout-gepa-paid-settlement',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    await route(
      store,
      `/api/operator/pylons/assignments/${assignmentRef}/closeout`,
      {
        adminToken: true,
        body: {
          accepted: true,
          acceptedWorkRefs: ['accepted_work.public.probe_gepa.metric_call'],
          closeoutRefs: ['closeout.public.operator.probe_gepa.accepted'],
        },
        method: 'POST',
      },
    )
    const payment = await route(
      store,
      `/api/pylons/pylon.test.one/assignments/${assignmentRef}/payment-receipts`,
      {
        body: {
          paymentProofRefs: ['payment_proof.public.probe_gepa.redacted'],
          receiptRefs: ['receipt.public.probe_gepa.paid_assignment'],
          settlementRefs: ['settlement.public.probe_gepa.spark.pending'],
        },
        idempotencyKey: 'payment-gepa-paid-settlement',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const settlement = await route(
      store,
      `/api/pylons/pylon.test.one/assignments/${assignmentRef}/settlement-status`,
      {
        body: {
          settlementRefs: ['settlement.public.probe_gepa.spark.settled'],
          status: 'settled',
          treasuryReceiptRefs: [
            'treasury_receipt.public.spark.probe_gepa.paid_assignment',
          ],
        },
        idempotencyKey: 'settlement-gepa-paid-settlement',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const paymentBody = await responseJson<PylonRouteJson>(payment)
    const settlementBody = await responseJson<PylonRouteJson>(settlement)

    expect(payment.status).toBe(201)
    expect(paymentBody.assignment?.settlement).toMatchObject({
      paymentReceiptRefs: ['receipt.public.probe_gepa.paid_assignment'],
      payoutClaimAllowed: false,
      realBitcoinMoved: false,
      settlementState: 'paid_receipt_recorded',
    })
    expect(settlement.status).toBe(201)
    expect(settlementBody.assignment?.settlement).toMatchObject({
      paymentReceiptRefs: ['receipt.public.probe_gepa.paid_assignment'],
      payoutClaimAllowed: true,
      realBitcoinMoved: true,
      settlementRefs: [
        'settlement.public.probe_gepa.spark.pending',
        'settlement.public.probe_gepa.spark.settled',
      ],
      settlementState: 'settled',
      treasuryReceiptRefs: [
        'treasury_receipt.public.spark.probe_gepa.paid_assignment',
      ],
    })
    expect(
      settlementBody.assignment?.settlement?.transitionReceiptRefs?.[0],
    ).toContain(
      'receipt.public.pylon_assignment.paid_settlement_transition.assignment.public.pylon_gepa_metric_call.stage0.paid_settlement',
    )
  })

  test('does not record acceptance events after a lost assignment-claim race', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    await markOnline(store)
    await markWalletReady(store)
    await createAssignment(store)

    let attemptedAtomicClaim = false
    store.updateAssignmentIfState = async () => {
      attemptedAtomicClaim = true
      return undefined
    }

    const accept = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.echo/accept',
      {
        body: {
          acceptanceRefs: ['acceptance.public.echo_ready'],
          accepted: true,
        },
        idempotencyKey: 'accept-echo-lost-claim-race',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const body = await responseJson<PylonRouteJson>(accept)
    const assignment = await store.readAssignment(
      'assignment.public.issue502.echo',
    )

    expect(attemptedAtomicClaim).toBe(true)
    expect(accept.status).toBe(409)
    expect(body.error).toBe('pylon_api_conflict')
    expect(assignment?.state).toBe('offered')
    expect(
      Array.from(store.events.values()).filter(
        event => event.eventKind === 'assignment_acceptance',
      ),
    ).toHaveLength(0)
  })

  test('blocks stale leases, wrong Pylon writes, invalid proof material, and supports rejected closeout', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    await markOnline(store)
    await markWalletReady(store)
    await createAssignment(store, {
      assignmentRef: 'assignment.public.issue502.stale',
      idempotencyKey: 'assignment-create-stale',
      leaseSeconds: 60,
    })
    const staleAccept = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.stale/accept',
      {
        body: { accepted: true },
        idempotencyKey: 'accept-stale',
        method: 'POST',
        nowIso: '2026-06-07T00:11:01.000Z',
        tokenUserId: 'agent-one',
      },
    )
    await route(store, '/api/pylons/register', {
      body: {
        capabilityRefs: ['capability.public.inference'],
        pylonRef: 'pylon.test.two',
      },
      idempotencyKey: 'register-pylon-test-two',
      method: 'POST',
      tokenUserId: 'agent-two',
    })
    const wrongPylon = await route(
      store,
      '/api/pylons/pylon.test.two/assignments/assignment.public.issue502.stale/accept',
      {
        body: { accepted: true },
        idempotencyKey: 'accept-wrong-pylon',
        method: 'POST',
        tokenUserId: 'agent-two',
      },
    )
    await createAssignment(store, {
      assignmentRef: 'assignment.public.issue502.reject',
      idempotencyKey: 'assignment-create-reject',
      nowIso: '2026-06-07T00:11:02.000Z',
    })
    await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.reject/accept',
      {
        body: { accepted: true },
        idempotencyKey: 'accept-reject-path',
        method: 'POST',
        nowIso: '2026-06-07T00:11:03.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const invalidProof = await route(
      store,
      '/api/pylons/pylon.test.one/assignments/assignment.public.issue502.reject/artifacts',
      {
        body: {
          artifactRefs: ['raw_artifact.private_runner_output'],
          proofRefs: ['proof.public.invalid'],
        },
        idempotencyKey: 'artifact-invalid',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const rejected = await route(
      store,
      '/api/operator/pylons/assignments/assignment.public.issue502.reject/closeout',
      {
        adminToken: true,
        body: {
          accepted: false,
          closeoutRefs: ['closeout.public.invalid_proof_reviewed'],
          rejectionRefs: ['rejection.public.invalid_proof'],
        },
        method: 'POST',
      },
    )
    const listAfterRejected = await route(
      store,
      '/api/pylons/pylon.test.one/assignments',
      {
        nowIso: '2026-06-07T00:11:04.000Z',
        tokenUserId: 'agent-one',
      },
    )
    const staleBody = await responseJson<PylonRouteJson>(staleAccept)
    const wrongPylonBody = await responseJson<PylonRouteJson>(wrongPylon)
    const invalidProofBody = await responseJson<PylonRouteJson>(invalidProof)
    const rejectedBody = await responseJson<PylonRouteJson>(rejected)
    const listAfterRejectedBody =
      await responseJson<PylonRouteJson>(listAfterRejected)

    expect(staleAccept.status).toBe(409)
    expect(staleBody.error).toBe('pylon_api_conflict')
    expect(wrongPylon.status).toBe(404)
    expect(wrongPylonBody.error).toBe('pylon_api_not_found')
    expect(invalidProof.status).toBe(400)
    expect(invalidProofBody.error).toBe('pylon_api_validation_error')
    expect(rejected.status).toBe(200)
    expect(rejectedBody.assignment?.state).toBe('rejected')
    expect(rejectedBody.assignment?.leaseState).toBe('terminal')
    expect(listAfterRejected.status).toBe(200)
    expect(listAfterRejectedBody.assignments).toEqual([])
  })

  test('scopes event idempotency keys by owner, Pylon, event kind, and assignment', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    const firstHeartbeat = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: { healthRefs: ['health.public.ok'] },
        idempotencyKey: 'pylon-event-key',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const replayHeartbeat = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: { healthRefs: ['health.public.changed'] },
        idempotencyKey: 'pylon-event-key',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    await registerPylon(store, {
      idempotencyKey: 'register-pylon-test-two',
      pylonRef: 'pylon.test.two',
      tokenUserId: 'agent-one',
    })
    const sameKeyDifferentPylon = await route(
      store,
      '/api/pylons/pylon.test.two/heartbeat',
      {
        body: { healthRefs: ['health.public.ok'] },
        idempotencyKey: 'pylon-event-key',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const sameKeyDifferentEventKind = await route(
      store,
      '/api/pylons/pylon.test.one/wallet-readiness',
      {
        body: { walletReady: true },
        idempotencyKey: 'pylon-event-key',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const wrongOwner = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: { healthRefs: ['health.public.ok'] },
        idempotencyKey: 'pylon-event-key',
        method: 'POST',
        tokenUserId: 'agent-two',
      },
    )
    const registrationWithSameClientKey = await route(
      store,
      '/api/pylons/register',
      {
        body: { pylonRef: 'pylon.test.from-event-key' },
        idempotencyKey: 'pylon-event-key',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const registerReplayWithDifferentPylon = await route(
      store,
      '/api/pylons/register',
      {
        body: { pylonRef: 'pylon.test.different' },
        idempotencyKey: 'register-pylon-test-one',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const replayHeartbeatBody = await responseJson<PylonRouteJson>(
      replayHeartbeat,
    )
    const registerReplayBody = await responseJson<PylonRouteJson>(
      registerReplayWithDifferentPylon,
    )

    expect(firstHeartbeat.status).toBe(201)
    expect(replayHeartbeat.status).toBe(200)
    expect(replayHeartbeatBody.idempotent).toBe(true)
    expect(replayHeartbeatBody.event).toMatchObject({
      eventKind: 'heartbeat',
      pylonRef: 'pylon.test.one',
      status: 'online',
    })
    expect(sameKeyDifferentPylon.status).toBe(201)
    expect(sameKeyDifferentEventKind.status).toBe(201)
    expect(wrongOwner.status).toBe(403)
    expect(registrationWithSameClientKey.status).toBe(201)
    expect(registerReplayWithDifferentPylon.status).toBe(200)
    expect(registerReplayBody.idempotent).toBe(true)
    expect(registerReplayBody.pylon?.pylonRef).toBe('pylon.test.one')
    expect(store.registrations.has('pylon.test.different')).toBe(false)
    expect(store.registrations.has('pylon.test.from-event-key')).toBe(true)
  })

  test('requires registered-agent auth and registration ownership for writes', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    const unauthenticated = await route(store, '/api/pylons/register', {
      body: { pylonRef: 'pylon.no.auth' },
      idempotencyKey: 'no-auth',
      method: 'POST',
    })
    const wrongOwner = await route(
      store,
      '/api/pylons/pylon.test.one/heartbeat',
      {
        body: { healthRefs: ['health.public.ok'] },
        idempotencyKey: 'wrong-owner',
        method: 'POST',
        tokenUserId: 'agent-two',
      },
    )

    expect(unauthenticated.status).toBe(401)
    expect(wrongOwner.status).toBe(403)
  })

  test('rejects raw payment and wallet material in write payloads', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    const response = await route(
      store,
      '/api/pylons/pylon.test.one/wallet-readiness',
      {
        body: {
          readinessRefs: ['readiness.public.ok'],
          walletReady: true,
          walletRef: 'lnbc10n1rawinvoice',
        },
        idempotencyKey: 'unsafe-wallet',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const body = await responseJson<PylonRouteJson>(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('pylon_api_validation_error')
  })

  test('rejects exact wallet balances in readiness payloads', async () => {
    const store = new MemoryPylonApiStore()
    await registerPylon(store)
    const response = await route(
      store,
      '/api/pylons/pylon.test.one/wallet-readiness',
      {
        body: {
          balanceRefs: ['balance.mdk_agent_wallet.10000'],
          readinessRefs: ['readiness.public.ok'],
          walletReady: true,
          walletRef: 'wallet.public.edge',
        },
        idempotencyKey: 'unsafe-balance',
        method: 'POST',
        tokenUserId: 'agent-one',
      },
    )
    const body = await responseJson<PylonRouteJson>(response)

    expect(response.status).toBe(400)
    expect(body.error).toBe('pylon_api_validation_error')
  })
})

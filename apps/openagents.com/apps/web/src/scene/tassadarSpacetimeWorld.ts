import type {
  AgentAvatar,
  AvatarPosition,
  ProofRef,
  PylonStation,
  RunEntity,
  SettlementRef,
  TrainingRun,
  WorldEdge,
  WorldEvent,
} from './spacetimeWorldBindings/types'
import {
  type TassadarRunPublicSummary,
  spacetimeWorldSummaryFromRows,
} from './tassadarRunSnapshot'

export const TASSADAR_SPACETIME_WORLD_URL_DATA_KEY = 'spacetime-world-url'
export const TASSADAR_SPACETIME_DATABASE_DATA_KEY = 'spacetime-database'
export const TASSADAR_SPACETIME_WORLD_URL_ATTRIBUTE = `data-${TASSADAR_SPACETIME_WORLD_URL_DATA_KEY}`
export const TASSADAR_SPACETIME_DATABASE_ATTRIBUTE = `data-${TASSADAR_SPACETIME_DATABASE_DATA_KEY}`

const SUBSCRIBED_TABLES = [
  'training_run',
  'run_entity',
  'world_edge',
  'proof_ref',
  'settlement_ref',
  'world_event',
] as const

const REGION_REF_PREFIX = 'region.'
const REGION_REF_SUFFIX = '.main'

type SpacetimeBindings = typeof import('./spacetimeWorldBindings')
type DbConnection = InstanceType<SpacetimeBindings['DbConnection']>

export type TassadarSpacetimeWorldConfig = Readonly<{
  database: string
  worldUrl: string
}>

export type TassadarSpacetimeWorldRows = Readonly<{
  agentAvatars: ReadonlyArray<AgentAvatar>
  avatarPositions: ReadonlyArray<AvatarPosition>
  proofRefs: ReadonlyArray<ProofRef>
  pylonStations: ReadonlyArray<PylonStation>
  runEntities: ReadonlyArray<RunEntity>
  settlementRefs: ReadonlyArray<SettlementRef>
  trainingRuns: ReadonlyArray<TrainingRun>
  worldEdges: ReadonlyArray<WorldEdge>
  worldEvents: ReadonlyArray<WorldEvent>
}>

export type TassadarSpacetimeWorldSubscription = Readonly<{
  disconnect: () => void
}>

const text = (value: string | null): string => value?.trim() ?? ''

const sqlString = (value: string): string => `'${value.replaceAll("'", "''")}'`

const regionRefForRun = (runRef: string): string =>
  `${REGION_REF_PREFIX}${runRef}${REGION_REF_SUFFIX}`

export const spacetimeConfigFromElement = (
  element: HTMLElement,
): TassadarSpacetimeWorldConfig | null => {
  const worldUrl = text(element.getAttribute(TASSADAR_SPACETIME_WORLD_URL_ATTRIBUTE))
  const database = text(element.getAttribute(TASSADAR_SPACETIME_DATABASE_ATTRIBUTE))
  if (worldUrl === '' || database === '') return null
  try {
    return { database, worldUrl: new URL(worldUrl).toString() }
  } catch {
    return null
  }
}

const subscriptionQueries = (runRef: string): ReadonlyArray<string> => {
  const run = sqlString(runRef)
  const region = sqlString(regionRefForRun(runRef))
  return [
    ...SUBSCRIBED_TABLES.map(table => `SELECT * FROM ${table} WHERE run_ref = ${run}`),
    `SELECT * FROM pylon_station WHERE run_ref = ${run}`,
    'SELECT * FROM agent_avatar',
    `SELECT * FROM avatar_position WHERE region_ref = ${region}`,
  ]
}

const rowsFromConnection = (conn: DbConnection): TassadarSpacetimeWorldRows => ({
  agentAvatars: [
    ...conn.db.agent_avatar.iter(),
  ] as unknown as ReadonlyArray<AgentAvatar>,
  avatarPositions: [
    ...conn.db.avatar_position.iter(),
  ] as unknown as ReadonlyArray<AvatarPosition>,
  proofRefs: [...conn.db.proof_ref.iter()] as unknown as ReadonlyArray<ProofRef>,
  pylonStations: [
    ...conn.db.pylon_station.iter(),
  ] as unknown as ReadonlyArray<PylonStation>,
  runEntities: [
    ...conn.db.run_entity.iter(),
  ] as unknown as ReadonlyArray<RunEntity>,
  settlementRefs: [
    ...conn.db.settlement_ref.iter(),
  ] as unknown as ReadonlyArray<SettlementRef>,
  trainingRuns: [
    ...conn.db.training_run.iter(),
  ] as unknown as ReadonlyArray<TrainingRun>,
  worldEdges: [
    ...conn.db.world_edge.iter(),
  ] as unknown as ReadonlyArray<WorldEdge>,
  worldEvents: [
    ...conn.db.world_event.iter(),
  ] as unknown as ReadonlyArray<WorldEvent>,
})

const observeTable = <Row>(
  table: {
    onDelete: (cb: (ctx: unknown, row: Row) => void) => void
    onInsert: (cb: (ctx: unknown, row: Row) => void) => void
    onUpdate: (cb: (ctx: unknown, oldRow: Row, row: Row) => void) => void
  },
  callback: () => void,
): void => {
  table.onInsert(callback)
  table.onUpdate(callback)
  table.onDelete(callback)
}

export const startTassadarSpacetimeWorldSubscription = async (
  input: Readonly<{
    baseSummary: TassadarRunPublicSummary
    config: TassadarSpacetimeWorldConfig
    onError: (error: unknown) => void
    onSummary: (summary: TassadarRunPublicSummary) => void
  }>,
): Promise<TassadarSpacetimeWorldSubscription> => {
  const { DbConnection } = await import('./spacetimeWorldBindings')
  const runRef = input.baseSummary.runRef ?? 'run.tassadar.executor.20260615'
  let closed = false
  let pending = false
  let conn: DbConnection | null = null

  const publish = (): void => {
    if (closed || conn === null || pending) return
    pending = true
    queueMicrotask(() => {
      pending = false
      if (closed || conn === null) return
      input.onSummary(
        spacetimeWorldSummaryFromRows(input.baseSummary, rowsFromConnection(conn)),
      )
    })
  }

  conn = DbConnection.builder()
    .withUri(input.config.worldUrl)
    .withDatabaseName(input.config.database)
    .withCompression('none')
    .onConnect((connection: DbConnection) => {
      observeTable(connection.db.training_run, publish)
      observeTable(connection.db.run_entity, publish)
      observeTable(connection.db.world_edge, publish)
      observeTable(connection.db.proof_ref, publish)
      observeTable(connection.db.settlement_ref, publish)
      observeTable(connection.db.world_event, publish)
      observeTable(connection.db.pylon_station, publish)
      observeTable(connection.db.agent_avatar, publish)
      observeTable(connection.db.avatar_position, publish)
      connection
        .subscriptionBuilder()
        .onApplied(publish)
        .onError(input.onError)
        .subscribe([...subscriptionQueries(runRef)])
    })
    .onConnectError((_ctx: unknown, error: Error) => {
      input.onError(error)
    })
    .onDisconnect((_ctx: unknown, error: Error | undefined) => {
      if (error !== undefined) input.onError(error)
    })
    .build()

  return {
    disconnect: () => {
      closed = true
      conn?.disconnect()
      conn = null
    },
  }
}

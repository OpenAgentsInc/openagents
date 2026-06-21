import type {
  AgentAvatar,
  AvatarPosition,
  ChatBubble,
  LocalChatMessage,
  ProofRef,
  PylonAttention,
  PylonStation,
  RunEntity,
  SettlementRef,
  TrainingRun,
  WorldEdge,
  WorldEvent,
  WorldRegion,
} from './spacetimeWorldBindings/types'
import {
  type TassadarRunPublicSummary,
  spacetimeWorldSummaryFromRows,
} from './tassadarRunSnapshot'

export const TASSADAR_SPACETIME_WORLD_URL_DATA_KEY = 'spacetime-world-url'
export const TASSADAR_SPACETIME_DATABASE_DATA_KEY = 'spacetime-database'
export const TASSADAR_SPACETIME_WORLD_URL_ATTRIBUTE = `data-${TASSADAR_SPACETIME_WORLD_URL_DATA_KEY}`
export const TASSADAR_SPACETIME_DATABASE_ATTRIBUTE = `data-${TASSADAR_SPACETIME_DATABASE_DATA_KEY}`
export const TASSADAR_AVATAR_POSITION_THROTTLE_MS = 250
export const TASSADAR_ATTENTION_THROTTLE_MS = 1_000
export const TASSADAR_STALE_AVATAR_POSITION_MS = 20_000
export const TASSADAR_STARTER_REGION_CONTRACT = {
  bounds: {
    maxX: 160,
    maxY: 40,
    maxZ: 160,
    minX: -160,
    minY: 0,
    minZ: -160,
  },
  localOrigin: { x: 0, y: 0, z: 0 },
  roadDirection: { x: 0, y: 0, z: 1 },
  starterPylonSiteOffset: { x: 24, y: 0, z: 0 },
  streetNextRegionRef: 'region.run.tassadar.executor.20260615.street.next',
  streetPrevRegionRef: 'region.run.tassadar.executor.20260615.street.prev',
} as const
export const TASSADAR_REGION_BOUNDS = TASSADAR_STARTER_REGION_CONTRACT.bounds

const SUBSCRIBED_TABLES = [
  'training_run',
  'run_entity',
  'world_edge',
  'proof_ref',
  'settlement_ref',
  'world_event',
  'world_region',
] as const

const REGION_REF_PREFIX = 'region.'
const REGION_REF_SUFFIX = '.main'
const PUBLIC_ACTIVITY_TIMELINE_WORLD_RUN_REF = 'run.public_activity_timeline'

type SpacetimeBindings = typeof import('./spacetimeWorldBindings')
type DbConnection = InstanceType<SpacetimeBindings['DbConnection']>

export type TassadarSpacetimeWorldConfig = Readonly<{
  database: string
  worldUrl: string
}>

export type TassadarSpacetimeWorldRows = Readonly<{
  agentAvatars: ReadonlyArray<AgentAvatar>
  avatarPositions: ReadonlyArray<AvatarPosition>
  chatBubbles: ReadonlyArray<ChatBubble>
  localChatMessages: ReadonlyArray<LocalChatMessage>
  proofRefs: ReadonlyArray<ProofRef>
  pylonAttention: ReadonlyArray<PylonAttention>
  pylonStations: ReadonlyArray<PylonStation>
  runEntities: ReadonlyArray<RunEntity>
  settlementRefs: ReadonlyArray<SettlementRef>
  trainingRuns: ReadonlyArray<TrainingRun>
  worldEdges: ReadonlyArray<WorldEdge>
  worldEvents: ReadonlyArray<WorldEvent>
  worldRegions: ReadonlyArray<WorldRegion>
}>

export type TassadarSpacetimeWorldSubscription = Readonly<{
  clearPylonFocus: (pylonRef: string) => void
  disconnect: () => void
  focusPylon: (input: TassadarPylonAttentionUpdate) => void
  regionRef: string
  sendLocalMessage: (input: TassadarLocalChatInput) => void
  sendPylonMessage: (input: TassadarPylonChatInput) => void
  updateLocalAvatar: (position: TassadarLocalAvatarPosition) => void
}>

export type TassadarLocalAvatarPosition = Readonly<{
  movementMode: 'ghost' | 'idle' | 'inspecting' | 'running' | 'walking'
  pitch: number
  positionX: number
  positionY: number
  positionZ: number
  yaw: number
}>

export type TassadarPylonAttentionUpdate = Readonly<{
  attentionKind: 'approaching' | 'inspecting' | 'looking' | 'nearby' | 'talking'
  distanceMeters: number
  pylonRef: string
  sourceEntityRef?: string
}>

export type TassadarLocalChatInput = Readonly<{
  body: string
  radiusMeters: number
  targetRef?: string
}>

export type TassadarPylonChatInput = Readonly<{
  body: string
  pylonRef: string
}>

const text = (value: string | null): string => value?.trim() ?? ''

const sqlString = (value: string): string => `'${value.replaceAll("'", "''")}'`

export const tassadarRegionRefForRun = (runRef: string): string =>
  `${REGION_REF_PREFIX}${runRef}${REGION_REF_SUFFIX}`

export const clampTassadarWorldCoordinate = (
  value: number,
  min: number,
  max: number,
): number =>
  Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : min

export const clampTassadarLocalAvatarPosition = (
  position: TassadarLocalAvatarPosition,
): TassadarLocalAvatarPosition => ({
  ...position,
  positionX: clampTassadarWorldCoordinate(
    position.positionX,
    TASSADAR_REGION_BOUNDS.minX,
    TASSADAR_REGION_BOUNDS.maxX,
  ),
  positionY: clampTassadarWorldCoordinate(
    position.positionY,
    TASSADAR_REGION_BOUNDS.minY,
    TASSADAR_REGION_BOUNDS.maxY,
  ),
  positionZ: clampTassadarWorldCoordinate(
    position.positionZ,
    TASSADAR_REGION_BOUNDS.minZ,
    TASSADAR_REGION_BOUNDS.maxZ,
  ),
})

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

export const tassadarSpacetimeWorldSubscriptionQueries = (
  runRef: string,
): ReadonlyArray<string> => {
  const run = sqlString(runRef)
  const publicActivityRun = sqlString(PUBLIC_ACTIVITY_TIMELINE_WORLD_RUN_REF)
  const region = sqlString(tassadarRegionRefForRun(runRef))
  return [
    ...SUBSCRIBED_TABLES.map(table => `SELECT * FROM ${table} WHERE run_ref = ${run}`),
    `SELECT * FROM world_event WHERE run_ref = ${publicActivityRun}`,
    `SELECT * FROM pylon_station WHERE run_ref = ${run}`,
    'SELECT * FROM agent_avatar',
    `SELECT * FROM avatar_position WHERE region_ref = ${region}`,
    'SELECT * FROM pylon_attention',
    `SELECT * FROM local_chat_message WHERE region_ref = ${region}`,
    'SELECT * FROM chat_bubble',
  ]
}

const rowsFromConnection = (conn: DbConnection): TassadarSpacetimeWorldRows => ({
  agentAvatars: [
    ...conn.db.agent_avatar.iter(),
  ] as unknown as ReadonlyArray<AgentAvatar>,
  avatarPositions: [
    ...conn.db.avatar_position.iter(),
  ] as unknown as ReadonlyArray<AvatarPosition>,
  chatBubbles: [
    ...conn.db.chat_bubble.iter(),
  ] as unknown as ReadonlyArray<ChatBubble>,
  localChatMessages: [
    ...conn.db.local_chat_message.iter(),
  ] as unknown as ReadonlyArray<LocalChatMessage>,
  proofRefs: [...conn.db.proof_ref.iter()] as unknown as ReadonlyArray<ProofRef>,
  pylonAttention: [
    ...conn.db.pylon_attention.iter(),
  ] as unknown as ReadonlyArray<PylonAttention>,
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
  worldRegions: [
    ...conn.db.world_region.iter(),
  ] as unknown as ReadonlyArray<WorldRegion>,
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
    displayName: string
    onError: (error: unknown) => void
    onSummary: (summary: TassadarRunPublicSummary) => void
  }>,
): Promise<TassadarSpacetimeWorldSubscription> => {
  const { DbConnection } = await import('./spacetimeWorldBindings')
  const runRef = input.baseSummary.runRef ?? 'run.tassadar.executor.20260615'
  const regionRef = tassadarRegionRefForRun(runRef)
  let closed = false
  let connected = false
  let pending = false
  let conn: DbConnection | null = null

  const callReducer = (
    reducerName: string,
    params: Record<string, unknown>,
  ): void => {
    if (closed || !connected || conn === null) return
    const reducer = (conn.reducers as Record<string, unknown>)[reducerName]
    if (typeof reducer !== 'function') return
    void (reducer as (params: Record<string, unknown>) => Promise<void>)(params)
      .catch(input.onError)
  }

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
      conn = connection
      observeTable(connection.db.training_run, publish)
      observeTable(connection.db.run_entity, publish)
      observeTable(connection.db.world_edge, publish)
      observeTable(connection.db.proof_ref, publish)
      observeTable(connection.db.settlement_ref, publish)
      observeTable(connection.db.world_event, publish)
      observeTable(connection.db.world_region, publish)
      observeTable(connection.db.pylon_station, publish)
      observeTable(connection.db.agent_avatar, publish)
      observeTable(connection.db.avatar_position, publish)
      observeTable(connection.db.pylon_attention, publish)
      observeTable(connection.db.local_chat_message, publish)
      observeTable(connection.db.chat_bubble, publish)
      connected = true
      callReducer('joinRegion', {
        displayName: input.displayName,
        regionRef,
      })
      connection
        .subscriptionBuilder()
        .onApplied(publish)
        .onError(input.onError)
        .subscribe([...tassadarSpacetimeWorldSubscriptionQueries(runRef)])
    })
    .onConnectError((_ctx: unknown, error: Error) => {
      input.onError(error)
    })
    .onDisconnect((_ctx: unknown, error: Error | undefined) => {
      if (error !== undefined) input.onError(error)
    })
    .build()

  return {
    clearPylonFocus: pylonRef => {
      callReducer('clearPylonFocus', { pylonRef })
    },
    disconnect: () => {
      closed = true
      if (connected && conn !== null) {
        const reducer = (conn.reducers as Record<string, unknown>).leaveRegion
        if (typeof reducer === 'function') {
          void (reducer as (params: { regionRef: string }) => Promise<void>)({
            regionRef,
          }).catch(input.onError)
        }
      }
      conn?.disconnect()
      conn = null
    },
    focusPylon: input => {
      callReducer('focusPylon', {
        attentionKind: input.attentionKind,
        distanceMeters: input.distanceMeters,
        pylonRef: input.pylonRef,
        sourceEntityRef: input.sourceEntityRef ?? null,
      })
    },
    regionRef,
    sendLocalMessage: message => {
      callReducer('sendLocalMessage', {
        body: message.body,
        radiusMeters: message.radiusMeters,
        regionRef,
        targetRef: message.targetRef ?? null,
      })
    },
    sendPylonMessage: message => {
      callReducer('sendPylonMessage', {
        body: message.body,
        pylonRef: message.pylonRef,
      })
    },
    updateLocalAvatar: position => {
      callReducer('setAvatarPosition', {
        ...clampTassadarLocalAvatarPosition(position),
        regionRef,
      })
    },
  }
}

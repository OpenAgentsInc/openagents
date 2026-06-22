import {
  createBrowserWorldTransport,
  createWorldClient,
  makeWorldClientActorRef,
  runWorldClientEffect,
  type ClientWorld,
  type WorldClient,
  worldClientNowIso,
} from '@openagentsinc/world-client'
import {
  WORLD_CONTRACT_SCHEMA_VERSION,
  type WorldCommandEnvelope,
  type WorldCommandName,
  type WorldIsoTimestamp,
  type WorldRef,
  type WorldRegionRef,
  type WorldSequence,
  worldAvatarRefForCharacter,
} from '@openagentsinc/world-contract'

import {
  type TassadarRunPublicSummary,
  type TassadarSpacetimeWorldRows,
  spacetimeWorldSummaryFromRows,
} from './tassadarRunSnapshot'

export const TASSADAR_SPACETIME_WORLD_URL_DATA_KEY = 'cloudflare-world-url'
export const TASSADAR_SPACETIME_DATABASE_DATA_KEY = 'cloudflare-world-database'
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

const PUBLIC_ACTIVITY_TIMELINE_WORLD_RUN_REF = 'run.public_activity_timeline'

export type TassadarSpacetimeWorldConfig = Readonly<{
  database: string
  worldUrl: string
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

const runRefForSummary = (summary: TassadarRunPublicSummary): string =>
  summary.runRef ?? 'run.tassadar.executor.20260615'

export const tassadarRegionRefForRun = (runRef: string): string =>
  `region.${runRef}.main`

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
): ReadonlyArray<string> => [
  `cloudflare-world:scope=run:${runRef}`,
  `cloudflare-world:scope=run:${PUBLIC_ACTIVITY_TIMELINE_WORLD_RUN_REF}`,
  `cloudflare-world:region=${tassadarRegionRefForRun(runRef)}`,
]

const movementModeToAnimation = (
  movementMode: TassadarLocalAvatarPosition['movementMode'],
): 'idle' | 'run' | 'walk' =>
  movementMode === 'running'
    ? 'run'
    : movementMode === 'walking'
      ? 'walk'
      : 'idle'

const readModelRows = (
  readModel: ClientWorld,
  runRef: string,
): TassadarSpacetimeWorldRows => ({
  agentAvatars: Object.values(readModel.avatars).map(row => ({
    actorKind: row.avatarKind,
    actorRef: row.accountRef ?? row.avatarRef,
    avatarRef: row.avatarRef,
    displayName: row.label,
    homePylonRef: undefined,
    publicProfileUrl: undefined,
  } as never)),
  avatarPositions: Object.values(readModel.positions).map(row => ({
    avatarRef: row.avatarRef,
    movementMode: row.animation === 'run'
      ? 'running'
      : row.animation === 'walk'
        ? 'walking'
        : 'idle',
    pitch: 0,
    positionX: row.position.x,
    positionY: row.position.y,
    positionZ: row.position.z,
    regionRef: row.regionRef,
    yaw: row.rotationY,
  } as never)),
  localChatMessages: Object.values(readModel.chatMessages).map(row => ({
    body: row.text,
    bodyFormat: 'plain_text',
    channelKind: row.channel,
    messageRef: row.messageRef,
    moderationState: row.moderationState,
    radiusMeters: 12,
    regionRef: row.regionRef,
    speakerAvatarRef: row.avatarRef,
    targetRef: undefined,
  } as never)),
  pylonAttention: Object.values(readModel.intents).flatMap(row =>
    row.intent === 'focus_pylon' && row.targetRef !== undefined
      ? [{
          attentionKind: 'looking',
          attentionRef: row.intentRef,
          avatarRef: row.avatarRef,
          distanceMeters: 0,
          pylonRef: row.targetRef,
          sourceEntityRef: undefined,
        } as never]
      : [],
  ),
  pylonStations: Object.values(readModel.pylons).map(row => ({
    interactionRadiusMeters: 12,
    label: row.label,
    positionX: row.position.x,
    positionY: row.position.y,
    positionZ: row.position.z,
    pylonRef: row.pylonRef,
    regionRef: row.regionRef,
    runRef,
    sourceUrl: `https://openagents.com/pylons/${encodeURIComponent(row.pylonRef)}`,
  } as never)),
  runEntities: Object.values(readModel.entities).map(row => ({
    entityKind: row.entityKind,
    entityRef: row.entityRef,
    label: row.label,
    runRef: row.runRef,
    sourceRef: row.entityRef,
    status: 'verified',
  } as never)),
  proofRefs: Object.values(readModel.proofRefs).map(row => ({
    entityRef: row.proofRef,
    proofKind: 'public_ref',
    proofRef: row.proofRef,
    runRef: row.runRef,
    title: row.label,
    url: row.url,
  } as never)),
  settlementRefs: Object.values(readModel.settlementRefs).map(row => ({
    amountSats: row.amountSats ?? 0,
    entityRef: row.settlementRef,
    movementMode: 'simulation',
    realBitcoinMoved: false,
    receiptRef: row.settlementRef,
    runRef: row.runRef,
    url: '',
  } as never)),
  trainingRuns: Object.values(readModel.runs).map(row => ({
    label: row.label,
    maxStalenessSeconds: 30,
    runRef: row.runRef,
    runState: row.state,
    sourceGeneratedAt: row.updatedAt,
    state: row.state,
    stalenessKind: 'cloudflare_world_projection',
  } as never)),
  worldEdges: Object.values(readModel.edges).map(row => ({
    edgeKind: row.relation,
    fromEntityRef: row.fromRef,
    sourceRef: row.edgeRef,
    toEntityRef: row.toRef,
    runRef,
  } as never)),
  worldEvents: Object.values(readModel.events).map(row => ({
    entityRef: row.eventRef,
    eventKind: row.eventKind,
    eventRef: row.eventRef,
    runRef: row.runRef ?? runRef,
    sourceGeneratedAt: row.createdAt,
    sourceRef: row.sourceRefs[0] ?? row.eventRef,
    summary: row.text,
  } as never)),
  worldRegions: Object.values(readModel.regions).map(row => ({
    avatarPositionMinIntervalMs: 100,
    label: row.label,
    localOriginX: row.origin.x,
    localOriginY: row.origin.y,
    localOriginZ: row.origin.z,
    maxX: row.bounds.max.x,
    maxY: row.bounds.max.y,
    maxZ: row.bounds.max.z,
    minX: row.bounds.min.x,
    minY: row.bounds.min.y,
    minZ: row.bounds.min.z,
    proximityRadiusMeters: row.proximityRadius,
    regionRef: row.regionRef,
    roadDirectionX: TASSADAR_STARTER_REGION_CONTRACT.roadDirection.x,
    roadDirectionY: TASSADAR_STARTER_REGION_CONTRACT.roadDirection.y,
    roadDirectionZ: TASSADAR_STARTER_REGION_CONTRACT.roadDirection.z,
    runRef,
    staleAvatarPositionMs: row.staleAvatarTtlMs,
    starterPylonSiteOffsetX: TASSADAR_STARTER_REGION_CONTRACT.starterPylonSiteOffset.x,
    starterPylonSiteOffsetY: TASSADAR_STARTER_REGION_CONTRACT.starterPylonSiteOffset.y,
    starterPylonSiteOffsetZ: TASSADAR_STARTER_REGION_CONTRACT.starterPylonSiteOffset.z,
    streetNextRegionRef: TASSADAR_STARTER_REGION_CONTRACT.streetNextRegionRef,
    streetPrevRegionRef: TASSADAR_STARTER_REGION_CONTRACT.streetPrevRegionRef,
  } as never)),
})

const makeCommand = (input: {
  actorRef: string
  command: WorldCommandName
  commandRef: string
  issuedAt: string
  payload: unknown
  regionRef: string
  seq: number
}): WorldCommandEnvelope => ({
  schemaVersion: WORLD_CONTRACT_SCHEMA_VERSION,
  actorClass: 'browser',
  actorRef: input.actorRef as WorldRef,
  command: input.command,
  commandRef: input.commandRef as WorldRef,
  issuedAt: input.issuedAt as WorldIsoTimestamp,
  payload: input.payload,
  regionRef: input.regionRef as WorldRegionRef,
  seq: input.seq as WorldSequence,
})

export const startTassadarSpacetimeWorldSubscription = async (
  input: Readonly<{
    baseSummary: TassadarRunPublicSummary
    config: TassadarSpacetimeWorldConfig
    displayName: string
    onError: (error: unknown) => void
    onSummary: (summary: TassadarRunPublicSummary) => void
  }>,
): Promise<TassadarSpacetimeWorldSubscription> => {
  const runRef = runRefForSummary(input.baseSummary)
  const regionRef = tassadarRegionRefForRun(runRef)
  const actorRef = makeWorldClientActorRef('web')
  const characterId = 'web'
  const client = createWorldClient({
    initialRegionRef: regionRef,
    transport: createBrowserWorldTransport({
      worldUrl: input.config.worldUrl,
      actorRef,
      actorClass: 'browser',
      onDelta: () => publish(client),
      onDiagnostic: () => publish(client),
    }),
  })
  let closed = false
  let seq = 0
  const nextSeq = (): number => {
    seq += 1
    return seq
  }
  const callCommand = (command: WorldCommandName, payload: unknown): void => {
    if (closed) return
    const issuedAt = worldClientNowIso()
    const commandSeq = nextSeq()
    void runWorldClientEffect(client.callCommand(makeCommand({
      actorRef,
      command,
      commandRef: `command.web.${command}.${commandSeq}.${issuedAt}`,
      issuedAt,
      payload,
      regionRef,
      seq: commandSeq,
    }))).catch(input.onError)
  }
  const publish = (worldClient: WorldClient): void => {
    if (closed) return
    void runWorldClientEffect(worldClient.readModel())
      .then(readModel => {
        if (!closed) {
          input.onSummary(
            spacetimeWorldSummaryFromRows(input.baseSummary, readModelRows(readModel, runRef)),
          )
        }
      })
      .catch(input.onError)
  }

  await runWorldClientEffect(client.connect({
    characterId,
    regionRef,
    runRef,
    scope: 'region',
  }))
  callCommand('join_region', {
    characterId,
    label: input.displayName,
    avatarRef: worldAvatarRefForCharacter(actorRef, characterId),
  })
  publish(client)

  return {
    clearPylonFocus: pylonRef => {
      callCommand('clear_pylon_focus', { pylonRef })
    },
    disconnect: () => {
      closed = true
      callCommand('leave_region', { characterId })
      void runWorldClientEffect(client.disconnect()).catch(input.onError)
    },
    focusPylon: focus => {
      callCommand('focus_pylon', {
        attentionKind: focus.attentionKind,
        distanceMeters: focus.distanceMeters,
        pylonRef: focus.pylonRef,
        sourceEntityRef: focus.sourceEntityRef,
      })
    },
    regionRef,
    sendLocalMessage: message => {
      callCommand('send_local_message', {
        text: message.body,
        radiusMeters: message.radiusMeters,
        targetRef: message.targetRef,
      })
    },
    sendPylonMessage: message => {
      callCommand('send_pylon_message', {
        text: message.body,
        pylonRef: message.pylonRef,
      })
    },
    updateLocalAvatar: position => {
      const clamped = clampTassadarLocalAvatarPosition(position)
      callCommand('set_avatar_position', {
        position: {
          x: clamped.positionX,
          y: clamped.positionY,
          z: clamped.positionZ,
        },
        rotationY: clamped.yaw,
        animation: movementModeToAnimation(clamped.movementMode),
      })
    },
  }
}

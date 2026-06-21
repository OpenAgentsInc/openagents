export const OPENAGENTS_WORLD_DATABASE = "openagents-world"
export const OPENAGENTS_WORLD_URL = "https://spacetime.openagents.com"
export const DEFAULT_TASSADAR_WORLD_RUN_REF = "run.tassadar.executor.20260615"
export const PUBLIC_ACTIVITY_TIMELINE_WORLD_RUN_REF = "run.public_activity_timeline"
export const CHAT_WORLD_DESKTOP_AVATAR_REF = "avatar.desktop.local"

export type ChatWorldStationRow = Readonly<{
  pylonRef: string
  runRef: string
  regionRef: string
  x: number
  y: number
  z: number
  label: string
}>

export type ChatWorldAvatarRow = Readonly<{
  avatarRef: string
  displayName: string
  avatarKind: string
  actorRef: string
  colorHex: string
}>

export type ChatWorldAvatarPresenceFeed = "high" | "low"

export type ChatWorldAvatarPositionRow = Readonly<{
  avatarRef: string
  regionRef: string
  x: number
  y: number
  z: number
  yaw: number
  movementMode: string
  lastSeenEpochMs: number
  presenceFeed?: ChatWorldAvatarPresenceFeed
}>

export type ChatWorldLocalChatMessageRow = Readonly<{
  messageRef: string
  avatarRef: string
  regionRef: string
  text: string
  radiusMeters: number
  expiresAtEpochMs: number
}>

export type ChatWorldPylonAttentionRow = Readonly<{
  attentionRef: string
  avatarRef: string
  pylonRef: string
  attentionKind: string
  expiresAtEpochMs: number
}>

export type ChatWorldMultiplayerRows = Readonly<{
  stations: ReadonlyArray<ChatWorldStationRow>
  avatars: ReadonlyArray<ChatWorldAvatarRow>
  positions: ReadonlyArray<ChatWorldAvatarPositionRow>
  messages: ReadonlyArray<ChatWorldLocalChatMessageRow>
  attention: ReadonlyArray<ChatWorldPylonAttentionRow>
}>

export type ChatWorldMultiplayerAgent = Readonly<{
  avatarRef: string
  label: string
  avatarKind: string
  actorRef: string
  color: string
  x: number
  y: number
  z: number
  yaw: number
  movementMode: string
  lastSeenEpochMs: number
  presenceFeed: ChatWorldAvatarPresenceFeed
  chatMessages: ReadonlyArray<string>
  attentionRefs: ReadonlyArray<string>
}>

export type ChatWorldMultiplayerStation = Readonly<{
  pylonRef: string
  label: string
  x: number
  y: number
  z: number
}>

export type ChatWorldMultiplayerProjection = Readonly<{
  connected: boolean
  database: string
  worldUrl: string
  regionRef: string
  projectedAtMs: number
  agents: ReadonlyArray<ChatWorldMultiplayerAgent>
  stations: ReadonlyArray<ChatWorldMultiplayerStation>
  proximityChatCount: number
}>

export type ChatWorldPresenceFeedMode = "single-region" | "split-near-far"

export type ChatWorldPresenceFeedScope = Readonly<{
  centerX?: number
  centerZ?: number
  mode?: ChatWorldPresenceFeedMode
  nearRadiusMeters?: number
}>

export type ChatWorldPresenceFeedEstimate = Readonly<{
  avatarCount: number
  farUpdateIntervalMs: number
  highUpdateIntervalMs: number
  recommendedMode: ChatWorldPresenceFeedMode
  singleFeedRowsPerSecond: number
  splitFarRowsPerSecond: number
  splitNearRowsPerSecond: number
}>

export const CHAT_WORLD_PRESENCE_FEED_CONTRACT = {
  farUpdateIntervalMs: 1_000,
  highUpdateIntervalMs: 100,
  nearRadiusMeters: 64,
  singleFeedMaxAvatars: 96,
  singleFeedMaxRowsPerSecond: 960,
} as const

const sqlString = (value: string): string => `'${value.replace(/'/g, "''")}'`

const sqlNumber = (value: number): string =>
  Number(value.toFixed(3)).toString()

export const chatWorldRegionRefForRun = (runRef: string): string =>
  `region.${runRef}.main`

export const estimateChatWorldPresenceFeedLoad = (input: {
  readonly avatarCount: number
  readonly highUpdateIntervalMs?: number
  readonly nearAvatarCount?: number
}): ChatWorldPresenceFeedEstimate => {
  const avatarCount = Math.max(0, Math.floor(input.avatarCount))
  const highUpdateIntervalMs = Math.max(
    1,
    input.highUpdateIntervalMs ?? CHAT_WORLD_PRESENCE_FEED_CONTRACT.highUpdateIntervalMs,
  )
  const farUpdateIntervalMs = CHAT_WORLD_PRESENCE_FEED_CONTRACT.farUpdateIntervalMs
  const nearAvatarCount = Math.max(
    0,
    Math.min(avatarCount, Math.floor(input.nearAvatarCount ?? avatarCount)),
  )
  const farAvatarCount = Math.max(0, avatarCount - nearAvatarCount)
  const singleFeedRowsPerSecond = Number(
    ((avatarCount * 1_000) / highUpdateIntervalMs).toFixed(3),
  )
  const splitNearRowsPerSecond = Number(
    ((nearAvatarCount * 1_000) / highUpdateIntervalMs).toFixed(3),
  )
  const splitFarRowsPerSecond = Number(
    ((farAvatarCount * 1_000) / farUpdateIntervalMs).toFixed(3),
  )
  const recommendedMode =
    avatarCount > CHAT_WORLD_PRESENCE_FEED_CONTRACT.singleFeedMaxAvatars ||
    singleFeedRowsPerSecond > CHAT_WORLD_PRESENCE_FEED_CONTRACT.singleFeedMaxRowsPerSecond
      ? "split-near-far"
      : "single-region"

  return {
    avatarCount,
    farUpdateIntervalMs,
    highUpdateIntervalMs,
    recommendedMode,
    singleFeedRowsPerSecond,
    splitFarRowsPerSecond,
    splitNearRowsPerSecond,
  }
}

const hasSplitPresenceScope = (
  scope: ChatWorldPresenceFeedScope | undefined,
): scope is Required<Pick<ChatWorldPresenceFeedScope, "centerX" | "centerZ">> &
  ChatWorldPresenceFeedScope => (
    scope?.mode === "split-near-far" &&
    Number.isFinite(scope.centerX) &&
    Number.isFinite(scope.centerZ)
  )

export const chatWorldMultiplayerSubscriptionQueries = (
  runRef: string,
  presenceScope?: ChatWorldPresenceFeedScope,
): ReadonlyArray<string> => {
  const run = sqlString(runRef)
  const publicActivityRun = sqlString(PUBLIC_ACTIVITY_TIMELINE_WORLD_RUN_REF)
  const region = sqlString(chatWorldRegionRefForRun(runRef))
  const splitPresence = hasSplitPresenceScope(presenceScope)
  const nearRadiusMeters = Math.max(
    1,
    presenceScope?.nearRadiusMeters ?? CHAT_WORLD_PRESENCE_FEED_CONTRACT.nearRadiusMeters,
  )
  const minX = splitPresence ? sqlNumber(presenceScope.centerX - nearRadiusMeters) : "0"
  const maxX = splitPresence ? sqlNumber(presenceScope.centerX + nearRadiusMeters) : "0"
  const minZ = splitPresence ? sqlNumber(presenceScope.centerZ - nearRadiusMeters) : "0"
  const maxZ = splitPresence ? sqlNumber(presenceScope.centerZ + nearRadiusMeters) : "0"
  const highPositionTable = splitPresence ? "avatar_position_near" : "avatar_position"
  const highPositionWhere = splitPresence
    ? `${highPositionTable}.region_ref = ${region} AND ${highPositionTable}.position_x >= ${minX} AND ${highPositionTable}.position_x <= ${maxX} AND ${highPositionTable}.position_z >= ${minZ} AND ${highPositionTable}.position_z <= ${maxZ}`
    : `${highPositionTable}.region_ref = ${region}`
  const farPositionWhere =
    `avatar_position_far.region_ref = ${region} AND avatar_position_far.position_x < ${minX} ` +
    `OR avatar_position_far.region_ref = ${region} AND avatar_position_far.position_x > ${maxX} ` +
    `OR avatar_position_far.region_ref = ${region} AND avatar_position_far.position_z < ${minZ} ` +
    `OR avatar_position_far.region_ref = ${region} AND avatar_position_far.position_z > ${maxZ}`
  const avatarProfileQueries = splitPresence
    ? [
        `SELECT agent_avatar.* FROM avatar_position_near JOIN agent_avatar ON avatar_position_near.avatar_ref = agent_avatar.avatar_ref WHERE ${highPositionWhere}`,
        `SELECT agent_avatar.* FROM avatar_position_far JOIN agent_avatar ON avatar_position_far.avatar_ref = agent_avatar.avatar_ref WHERE ${farPositionWhere}`,
      ]
    : [
        `SELECT agent_avatar.* FROM avatar_position JOIN agent_avatar ON avatar_position.avatar_ref = agent_avatar.avatar_ref WHERE ${highPositionWhere}`,
      ]
  const positionQueries = splitPresence
    ? [
        `SELECT * FROM avatar_position_near WHERE ${highPositionWhere}`,
        `SELECT * FROM avatar_position_far WHERE ${farPositionWhere}`,
      ]
    : [
        `SELECT * FROM avatar_position WHERE ${highPositionWhere}`,
      ]
  return [
    `SELECT * FROM world_event WHERE run_ref = ${run}`,
    `SELECT * FROM world_event WHERE run_ref = ${publicActivityRun}`,
    `SELECT * FROM world_region WHERE region_ref = ${region}`,
    `SELECT * FROM pylon_station WHERE region_ref = ${region}`,
    ...avatarProfileQueries,
    ...positionQueries,
    `SELECT pylon_attention.* FROM pylon_station JOIN pylon_attention ON pylon_station.pylon_ref = pylon_attention.pylon_ref WHERE pylon_station.region_ref = ${region}`,
    `SELECT * FROM local_chat_message WHERE region_ref = ${region}`,
    `SELECT chat_bubble.* FROM local_chat_message JOIN chat_bubble ON local_chat_message.message_ref = chat_bubble.message_ref WHERE local_chat_message.region_ref = ${region}`,
    `SELECT * FROM local_emote WHERE region_ref = ${region}`,
    `SELECT agent_intent.* FROM avatar_position JOIN agent_intent ON avatar_position.avatar_ref = agent_intent.avatar_ref WHERE avatar_position.region_ref = ${region}`,
  ]
}

const finite = (value: number): boolean =>
  Number.isFinite(value) && !Number.isNaN(value)

const positionPresenceFeed = (
  position: ChatWorldAvatarPositionRow,
): ChatWorldAvatarPresenceFeed =>
  position.presenceFeed === "low" ? "low" : "high"

const positionFeedRank = (feed: ChatWorldAvatarPresenceFeed): number =>
  feed === "high" ? 1 : 0

const choosePositionRow = (
  current: ChatWorldAvatarPositionRow | undefined,
  candidate: ChatWorldAvatarPositionRow,
): ChatWorldAvatarPositionRow => {
  if (current === undefined) return candidate
  const currentFeed = positionPresenceFeed(current)
  const candidateFeed = positionPresenceFeed(candidate)
  return positionFeedRank(candidateFeed) > positionFeedRank(currentFeed) ||
    (
      candidateFeed === currentFeed &&
      candidate.lastSeenEpochMs > current.lastSeenEpochMs
    )
    ? candidate
    : current
}

export const projectChatWorldMultiplayer = (input: {
  readonly flagEnabled: boolean
  readonly runRef: string
  readonly rows: ChatWorldMultiplayerRows | null
  readonly nowMs: number
  readonly worldUrl?: string
  readonly database?: string
}): ChatWorldMultiplayerProjection => {
  const regionRef = chatWorldRegionRefForRun(input.runRef)
  const rows = input.rows
  if (input.flagEnabled !== true || rows === null) {
    return {
      connected: false,
      database: input.database ?? OPENAGENTS_WORLD_DATABASE,
      worldUrl: input.worldUrl ?? OPENAGENTS_WORLD_URL,
      regionRef,
      projectedAtMs: input.nowMs,
      agents: [],
      stations: [],
      proximityChatCount: 0,
    }
  }

  const avatarByRef = new Map(rows.avatars.map(avatar => [avatar.avatarRef, avatar]))
  const stationRows = rows.stations
    .filter(station => station.runRef === input.runRef)
    .filter(station => station.regionRef === regionRef)
    .filter(station => finite(station.x) && finite(station.y) && finite(station.z))
  const stationRefsInRegion = new Set(stationRows.map(station => station.pylonRef))
  const positionRows = rows.positions
    .filter(position => position.regionRef === regionRef)
    .filter(position => finite(position.x) && finite(position.y) && finite(position.z))
    .reduce((byAvatar, position) => {
      byAvatar.set(position.avatarRef, choosePositionRow(
        byAvatar.get(position.avatarRef),
        position,
      ))
      return byAvatar
    }, new Map<string, ChatWorldAvatarPositionRow>())
    .values()
  const activePositionRows = [...positionRows]
  const avatarRefsInRegion = new Set(activePositionRows.map(position => position.avatarRef))
  const messagesByAvatar = new Map<string, Array<string>>()
  for (const message of rows.messages) {
    if (
      message.regionRef !== regionRef ||
      message.expiresAtEpochMs <= input.nowMs ||
      message.text.trim().length === 0
    ) {
      continue
    }
    const current = messagesByAvatar.get(message.avatarRef) ?? []
    current.push(message.text)
    messagesByAvatar.set(message.avatarRef, current)
  }

  const attentionByAvatar = new Map<string, Array<string>>()
  for (const attention of rows.attention) {
    if (
      attention.expiresAtEpochMs <= input.nowMs ||
      !stationRefsInRegion.has(attention.pylonRef) ||
      !avatarRefsInRegion.has(attention.avatarRef)
    ) {
      continue
    }
    const current = attentionByAvatar.get(attention.avatarRef) ?? []
    current.push(attention.attentionRef)
    attentionByAvatar.set(attention.avatarRef, current)
  }

  const agents = activePositionRows.flatMap(position => {
    const avatar = avatarByRef.get(position.avatarRef)
    if (avatar === undefined) {
      return []
    }
    return [{
      avatarRef: position.avatarRef,
      label: avatar.displayName,
      avatarKind: avatar.avatarKind,
      actorRef: avatar.actorRef,
      color: avatar.colorHex,
      x: position.x,
      y: position.y,
      z: position.z,
      yaw: finite(position.yaw) ? position.yaw : 0,
      movementMode: position.movementMode,
      lastSeenEpochMs: position.lastSeenEpochMs,
      presenceFeed: positionPresenceFeed(position),
      chatMessages: messagesByAvatar.get(position.avatarRef) ?? [],
      attentionRefs: attentionByAvatar.get(position.avatarRef) ?? [],
    }]
  })

  const stations = stationRows.map(station => ({
    pylonRef: station.pylonRef,
    label: station.label,
    x: station.x,
    y: station.y,
    z: station.z,
  }))

  return {
    connected: true,
    database: input.database ?? OPENAGENTS_WORLD_DATABASE,
    worldUrl: input.worldUrl ?? OPENAGENTS_WORLD_URL,
    regionRef,
    projectedAtMs: input.nowMs,
    agents,
    stations,
    proximityChatCount: agents.reduce(
      (count, agent) => count + agent.chatMessages.length,
      0,
    ),
  }
}

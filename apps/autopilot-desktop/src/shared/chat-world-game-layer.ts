export type ChatWorldGameLayerFlags = Readonly<{
  hotbar: boolean
  reputation: boolean
  manaHud: boolean
  handTracking: boolean
}>

export type HotbarAgent = Readonly<{
  agentRef: string
  label: string
  group: number
  paneRef?: string
}>

export type HotbarSlot = Readonly<{
  slot: number
  agents: ReadonlyArray<HotbarAgent>
  focusCommand: string
  enabled: boolean
}>

export type ReputationGlyph = Readonly<{
  actorRef: string
  tier: "new" | "trusted" | "guild" | "legend"
  glyph: "dot" | "chevron" | "diamond" | "crown"
  enabled: boolean
}>

export type ManaBudgetHud = Readonly<{
  available: number
  total: number
  ratio: number
  label: string
  enabled: boolean
}>

export type HandPose = Readonly<{
  thumbTip: readonly [number, number]
  indexTip: readonly [number, number]
  confidence: number
}>

export type HandPinchState = Readonly<{
  pinching: boolean
  distance: number
  enabled: boolean
}>

export type ChatWorldGameLayer = Readonly<{
  hotbar: ReadonlyArray<HotbarSlot>
  reputation: ReadonlyArray<ReputationGlyph>
  mana: ManaBudgetHud
  hand: HandPinchState
}>

const clamp01 = (value: number): number =>
  Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0

export const projectHotbarAgentGroups = (input: {
  readonly flags: ChatWorldGameLayerFlags
  readonly agents: ReadonlyArray<HotbarAgent>
}): ReadonlyArray<HotbarSlot> =>
  Array.from({ length: 9 }, (_unused, index) => {
    const slot = index + 1
    return {
      slot,
      agents: input.agents.filter(agent => agent.group === slot),
      focusCommand: `Ctrl+${slot}`,
      enabled: input.flags.hotbar,
    }
  })

export const reputationGlyphForScore = (input: {
  readonly flags: ChatWorldGameLayerFlags
  readonly actorRef: string
  readonly score: number
}): ReputationGlyph => {
  const score = Number.isFinite(input.score) ? input.score : 0
  const tier =
    score >= 1_000 ? "legend" : score >= 250 ? "guild" : score >= 50 ? "trusted" : "new"
  const glyph =
    tier === "legend" ? "crown" : tier === "guild" ? "diamond" : tier === "trusted" ? "chevron" : "dot"
  return { actorRef: input.actorRef, tier, glyph, enabled: input.flags.reputation }
}

export const projectManaBudgetHud = (input: {
  readonly flags: ChatWorldGameLayerFlags
  readonly available: number
  readonly total: number
}): ManaBudgetHud => {
  const total = Number.isFinite(input.total) && input.total > 0 ? input.total : 0
  const available =
    total === 0 ? 0 : Math.min(total, Math.max(0, input.available))
  const ratio = total === 0 ? 0 : clamp01(available / total)
  return {
    available,
    total,
    ratio,
    label: `${available}/${total} compute`,
    enabled: input.flags.manaHud,
  }
}

export const projectHandPinch = (input: {
  readonly flags: ChatWorldGameLayerFlags
  readonly pose: HandPose | null
  readonly pinchDistance?: number
}): HandPinchState => {
  if (input.flags.handTracking !== true || input.pose === null || input.pose.confidence < 0.65) {
    return { pinching: false, distance: Infinity, enabled: input.flags.handTracking }
  }
  const dx = input.pose.thumbTip[0] - input.pose.indexTip[0]
  const dy = input.pose.thumbTip[1] - input.pose.indexTip[1]
  const distance = Math.sqrt(dx * dx + dy * dy)
  return {
    pinching: distance <= (input.pinchDistance ?? 0.045),
    distance,
    enabled: true,
  }
}

export const projectChatWorldGameLayer = (input: {
  readonly flags: ChatWorldGameLayerFlags
  readonly agents: ReadonlyArray<HotbarAgent>
  readonly reputationScores: ReadonlyArray<Readonly<{ actorRef: string; score: number }>>
  readonly manaAvailable: number
  readonly manaTotal: number
  readonly handPose: HandPose | null
}): ChatWorldGameLayer => ({
  hotbar: projectHotbarAgentGroups({ flags: input.flags, agents: input.agents }),
  reputation: input.reputationScores.map(item =>
    reputationGlyphForScore({
      flags: input.flags,
      actorRef: item.actorRef,
      score: item.score,
    }),
  ),
  mana: projectManaBudgetHud({
    flags: input.flags,
    available: input.manaAvailable,
    total: input.manaTotal,
  }),
  hand: projectHandPinch({ flags: input.flags, pose: input.handPose }),
})

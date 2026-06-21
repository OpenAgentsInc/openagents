import type {
  TrainingRunBeamDefinition,
  TrainingRunBurstDefinition,
  TrainingRunEntityDefinition,
  TrainingRunNodeDefinition,
  TrainingRunRemoteAvatarDefinition,
  TrainingRunVector,
  TrainingRunVisualizationOptions,
  TrainingRunWorldItemDefinition,
} from "@openagentsinc/three-effect/core"

export type VerseVisualizationAppend = Readonly<{
  beams?: readonly TrainingRunBeamDefinition[]
  bursts?: readonly TrainingRunBurstDefinition[]
  entities?: readonly TrainingRunEntityDefinition[]
  nodes?: readonly TrainingRunNodeDefinition[]
  remoteAvatars?: readonly TrainingRunRemoteAvatarDefinition[]
  worldItems?: readonly TrainingRunWorldItemDefinition[]
}>

export const uniqueVerseStrings = (
  values: ReadonlyArray<string | null | undefined>,
): string[] => {
  const out: string[] = []
  for (const value of values) {
    const trimmed = typeof value === "string" ? value.trim() : ""
    if (trimmed.length === 0 || out.includes(trimmed)) continue
    out.push(trimmed)
  }
  return out
}

export const compactVerseLines = (
  values: ReadonlyArray<string | null | undefined>,
): readonly string[] => uniqueVerseStrings(values)

export const finitePositiveVerseNumber = (
  value: number | null | undefined,
): number =>
  typeof value === "number" && Number.isFinite(value) && value > 0 ? value : 0

export const roundVerseNumber = (value: number, decimals = 3): number =>
  Number.isFinite(value) ? Number(value.toFixed(decimals)) : 0

export const roundedVerseVector = (
  vector: readonly [number, number, number],
  decimals = 3,
): TrainingRunVector => [
  roundVerseNumber(vector[0], decimals),
  roundVerseNumber(vector[1], decimals),
  roundVerseNumber(vector[2], decimals),
]

export const appendVerseVisualization = (
  base: TrainingRunVisualizationOptions,
  append: VerseVisualizationAppend,
): TrainingRunVisualizationOptions => ({
  ...base,
  ...(append.nodes === undefined
    ? {}
    : { nodes: [...(base.nodes ?? []), ...append.nodes] }),
  ...(append.entities === undefined
    ? {}
    : { entities: [...(base.entities ?? []), ...append.entities] }),
  ...(append.worldItems === undefined
    ? {}
    : { worldItems: [...(base.worldItems ?? []), ...append.worldItems] }),
  ...(append.remoteAvatars === undefined
    ? {}
    : {
        remoteAvatars: [
          ...(base.remoteAvatars ?? []),
          ...append.remoteAvatars,
        ],
      }),
  ...(append.beams === undefined
    ? {}
    : { beams: [...(base.beams ?? []), ...append.beams] }),
  ...(append.bursts === undefined
    ? {}
    : { bursts: [...(base.bursts ?? []), ...append.bursts] }),
})

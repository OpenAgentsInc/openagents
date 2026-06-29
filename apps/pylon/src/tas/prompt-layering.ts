export const INSTRUCTION_SCOPE_ORDER = [
  "system",
  "project",
  "session",
  "task",
] as const

export type InstructionScope = (typeof INSTRUCTION_SCOPE_ORDER)[number]

export type InstructionLayer = {
  readonly scope: InstructionScope
  readonly ref: string
  readonly text: string
}

export type InstructionLayerRef = {
  readonly scope: InstructionScope
  readonly ref: string
}

export type InstructionLayeringProvenance = {
  readonly layers: readonly InstructionLayerRef[]
}

export type InstructionLayeringSnapshot = {
  readonly ordered: readonly InstructionLayerRef[]
  readonly provenance: InstructionLayeringProvenance
}

export function assembleInstructions(
  layers: readonly InstructionLayer[],
): InstructionLayeringSnapshot {
  const byRef = new Map<string, InstructionLayerRef>()

  for (const layer of layers) {
    const current = byRef.get(layer.ref)

    if (
      current === undefined ||
      scopeRank(layer.scope) >= scopeRank(current.scope)
    ) {
      byRef.set(layer.ref, { scope: layer.scope, ref: layer.ref })
    }
  }

  const ordered = [...byRef.values()].sort(
    (left, right) => scopeRank(left.scope) - scopeRank(right.scope),
  )

  return {
    ordered,
    provenance: {
      layers: ordered.map((layer) => ({ ...layer })),
    },
  }
}

function scopeRank(scope: InstructionScope): number {
  return INSTRUCTION_SCOPE_ORDER.indexOf(scope)
}

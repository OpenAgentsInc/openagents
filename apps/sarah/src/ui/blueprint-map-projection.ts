import type {
  GraphEdgeModel,
  GraphNodeModel,
  GraphStatus,
} from "@effect-native/core"

import type { SarahBlueprintDelta, SarahBlueprintFactLabel } from "../services/avatar-event-bus.ts"
import type {
  CustomerBlueprintDraft,
  SuggestedModule,
} from "../services/customer-blueprint.ts"
import type { SarahProspectFact } from "../services/prospect-memory.ts"

export type BlueprintMapFact = Readonly<{
  label: SarahBlueprintFactLabel
  text: string
  sourceTurnId: string
}>

export type BlueprintMapProjectionInput = Readonly<{
  draft: CustomerBlueprintDraft | null
  facts: ReadonlyArray<BlueprintMapFact>
  contactEmail: string | null
  accountLinked: boolean
  live: boolean
}>

export type BlueprintMapProjection = Readonly<{
  nodes: ReadonlyArray<GraphNodeModel>
  edges: ReadonlyArray<GraphEdgeModel>
}>

const factSlotLabels = ["company", "role", "stack", "contact"] as const
type FactSlotLabel = (typeof factSlotLabels)[number]

const slotTitle: Record<FactSlotLabel, string> = {
  company: "Company",
  role: "Role",
  stack: "Stack",
  contact: "Contact",
}

const factLabels = new Set<SarahBlueprintFactLabel>([
  "company",
  "role",
  "need",
  "stack",
  "contact",
  "other",
])

const shortLabel = (value: string, max = 44): string => {
  const clean = value.replace(/\s+/g, " ").trim()
  if (clean.length <= max) return clean
  return `${clean.slice(0, Math.max(0, max - 1)).trim()}…`
}

const factText = (fact: string): string => {
  const withoutLabel = fact.replace(/^[a-z_]+:\s*/i, "").trim()
  const quoted = withoutLabel.match(/^"([\s\S]*)"$/)
  return quoted ? quoted[1]!.trim() : withoutLabel
}

export function blueprintMapFactFromProfileFact(
  fact: Pick<SarahProspectFact, "fact" | "sourceTurnId">,
): BlueprintMapFact {
  const raw = fact.fact.split(":", 1)[0]?.trim() ?? ""
  const label = factLabels.has(raw as SarahBlueprintFactLabel)
    ? (raw as SarahBlueprintFactLabel)
    : "other"
  return {
    label,
    text: factText(fact.fact),
    sourceTurnId: fact.sourceTurnId,
  }
}

export function blueprintMapFactFromDelta(
  delta: SarahBlueprintDelta,
): BlueprintMapFact | null {
  if (delta.kind !== "fact_added") return null
  return {
    label: delta.label,
    text: delta.text,
    sourceTurnId: delta.sourceTurnId,
  }
}

const dedupeFacts = (
  facts: ReadonlyArray<BlueprintMapFact>,
): ReadonlyArray<BlueprintMapFact> => {
  const seen = new Set<string>()
  const result: BlueprintMapFact[] = []
  for (const fact of facts) {
    const key = `${fact.label}\u0000${fact.sourceTurnId}\u0000${fact.text}`
    if (seen.has(key)) continue
    seen.add(key)
    result.push(fact)
  }
  return result
}

const needFactsFromDraft = (draft: CustomerBlueprintDraft | null): BlueprintMapFact[] =>
  (draft?.needs ?? []).map((need) => ({
    label: "need",
    text: factText(need.need),
    sourceTurnId: need.sourceTurnId,
  }))

const profileFactsFromDraft = (draft: CustomerBlueprintDraft | null): BlueprintMapFact[] =>
  (draft?.business.facts ?? []).map(blueprintMapFactFromProfileFact)

const semanticModules = (
  modules: ReadonlyArray<SuggestedModule>,
): ReadonlyArray<SuggestedModule> =>
  modules.filter(
    (module) =>
      module.matchBasis === "semantic" &&
      module.matchedNeedTurnIds.length > 0,
  )

const graphStatusForFact = (known: boolean): GraphStatus =>
  known ? "success" : "pending"

export function blueprintMapProjection(
  input: BlueprintMapProjectionInput,
): BlueprintMapProjection {
  const facts = dedupeFacts([
    ...profileFactsFromDraft(input.draft),
    ...needFactsFromDraft(input.draft),
    ...input.facts,
  ])
  const contactEmail = input.contactEmail ?? input.draft?.contacts.email ?? null
  const accountLinked =
    input.accountLinked ||
    (input.draft?.contacts.contactId ?? "").startsWith("oa_user:")
  const nodes: GraphNodeModel[] = [
    {
      id: "prospect",
      label: shortLabel(contactEmail ?? "You", 32),
      kind: "arbiter",
      status: input.live ? "active" : "idle",
      x: -300,
      y: 0,
    },
  ]
  const edges: GraphEdgeModel[] = []

  factSlotLabels.forEach((slot, index) => {
    const fact =
      facts.find((entry) => entry.label === slot) ??
      (slot === "contact" && contactEmail
        ? { label: "contact", text: contactEmail, sourceTurnId: "contact" }
        : null)
    const nodeId = `fact:${slot}`
    nodes.push({
      id: nodeId,
      label: fact
        ? `${slotTitle[slot]}: ${shortLabel(fact.text, 30)}`
        : `${slotTitle[slot]}?`,
      kind: "generic",
      status: graphStatusForFact(fact !== null),
      x: -110,
      y: -135 + index * 90,
    })
    edges.push({
      id: `edge:prospect:${nodeId}`,
      from: "prospect",
      to: nodeId,
      kind: "dependency",
      status: fact ? "success" : "pending",
    })
  })

  const needFacts = facts.filter((fact) => fact.label === "need")
  needFacts.slice(0, 4).forEach((need, index) => {
    const nodeId = `need:${need.sourceTurnId || index}`
    nodes.push({
      id: nodeId,
      label: `Need: ${shortLabel(need.text, 34)}`,
      kind: "task",
      status: input.live ? "active" : "success",
      x: 115,
      y: -105 + index * 85,
    })
    edges.push({
      id: `edge:prospect:${nodeId}`,
      from: "prospect",
      to: nodeId,
      kind: "flow",
      status: input.live ? "active" : "success",
    })
  })

  const modules = semanticModules(input.draft?.suggestedModules ?? [])
  modules.slice(0, 4).forEach((module, index) => {
    const nodeId = `offering:${module.ref}`
    nodes.push({
      id: nodeId,
      label: shortLabel(module.name, 34),
      kind: "worker",
      status: "success",
      x: 350,
      y: -105 + index * 85,
    })
    for (const turnId of module.matchedNeedTurnIds) {
      const needNodeId = `need:${turnId}`
      if (!nodes.some((node) => node.id === needNodeId)) continue
      edges.push({
        id: `edge:${needNodeId}:${nodeId}`,
        from: needNodeId,
        to: nodeId,
        kind: "pairing",
        status: "success",
      })
    }
  })

  const candidateCount =
    (input.draft?.suggestedModules ?? []).filter(
      (module) => module.matchBasis === "candidate_default",
    ).length
  if (modules.length === 0 && candidateCount > 0) {
    nodes.push({
      id: "offering:candidates",
      label: `${candidateCount} candidate modules`,
      kind: "worker",
      status: "idle",
      x: 350,
      y: 130,
    })
    edges.push({
      id: "edge:prospect:offering:candidates",
      from: "prospect",
      to: "offering:candidates",
      kind: "dependency",
      status: "idle",
    })
  }

  nodes.push({
    id: "account",
    label: accountLinked ? "Account linked" : "Account pending",
    kind: "validator",
    status: accountLinked ? "success" : "pending",
    x: -300,
    y: 155,
  })
  edges.push({
    id: "edge:prospect:account",
    from: "prospect",
    to: "account",
    kind: "dependency",
    status: accountLinked ? "success" : "pending",
  })

  return { nodes, edges }
}

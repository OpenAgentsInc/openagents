/**
 * Obligation dependency-graph analysis and projection (GAP_ANALYSIS §6,
 * §11 item 5; ASSURANCE_SPEC §5 dependency_refs / activation_gate, §12.2
 * `cyclic_obligation_dependency`).
 *
 * Everything here is a pure function over the parsed document model: no
 * filesystem, no clock, no randomness, no model calls (Law 2). The validator
 * consumes `analyzeObligationDependencies` for the structural codes; the CLI
 * `graph` command and the MCP `get_obligation_graph` tool consume
 * `projectObligationGraph` through the shared handler (one implementation,
 * two transports).
 *
 * Honesty posture (Law 7): the projection reports declared structure only.
 * No receipts exist, so nothing here claims a dependency was satisfied, and
 * `design_order` is a dependency-respecting order for proof design — never a
 * manifest-level execution ordering, which is the AS-2 compiler's projection
 * (ASSURANCE_SPEC §9). Spec-to-spec graphs are deliberately out of scope
 * until more than one AssuranceSpec exists (GAP_ANALYSIS §6).
 */
import type { AssuranceObligation, AssuranceSpecDocument } from "./schema.ts"

// ---------------------------------------------------------------------------
// Dependency analysis (shared by the validator and the projection)
// ---------------------------------------------------------------------------

export type ObligationDependencyIssueCode =
  | "self_obligation_dependency"
  | "dangling_dependency_ref"
  | "cyclic_obligation_dependency"

export type ObligationDependencyIssue = Readonly<{
  code: ObligationDependencyIssueCode
  message: string
  obligation_id: string
  dependency_ref?: string
  cycle?: ReadonlyArray<string>
}>

export type ObligationDependencyEdge = Readonly<{
  /** The dependent obligation (it waits). */
  from: string
  /** The obligation it depends on. */
  to: string
}>

export type ObligationDependencyAnalysis = Readonly<{
  /** Resolved dependency edges only: self and dangling refs are excluded. */
  edges: ReadonlyArray<ObligationDependencyEdge>
  /** Resolved dependencies per obligation, declared order, deduplicated. */
  waits_on: ReadonlyMap<string, ReadonlyArray<string>>
  /** Every dependency cycle, members in document order, reported once. */
  cycles: ReadonlyArray<ReadonlyArray<string>>
  issues: ReadonlyArray<ObligationDependencyIssue>
}>

/**
 * Deterministic cycle enumeration: Tarjan strongly connected components over
 * the resolved dependency edges, in document order. Self-dependencies are
 * excluded from the edge set (they carry their own issue code), so only
 * components with two or more members are cycles.
 */
const findDependencyCycles = (
  order: ReadonlyArray<string>,
  waitsOn: ReadonlyMap<string, ReadonlyArray<string>>,
): ReadonlyArray<ReadonlyArray<string>> => {
  const index = new Map<string, number>()
  const lowlink = new Map<string, number>()
  const onStack = new Set<string>()
  const stack: string[] = []
  const cycles: Array<ReadonlyArray<string>> = []
  const documentIndex = new Map(order.map((id, position) => [id, position] as const))
  let counter = 0

  const connect = (node: string): void => {
    index.set(node, counter)
    lowlink.set(node, counter)
    counter += 1
    stack.push(node)
    onStack.add(node)
    for (const next of waitsOn.get(node) ?? []) {
      if (!index.has(next)) {
        connect(next)
        lowlink.set(node, Math.min(lowlink.get(node)!, lowlink.get(next)!))
      } else if (onStack.has(next)) {
        lowlink.set(node, Math.min(lowlink.get(node)!, index.get(next)!))
      }
    }
    if (lowlink.get(node) === index.get(node)) {
      const component: string[] = []
      let member: string
      do {
        member = stack.pop()!
        onStack.delete(member)
        component.push(member)
      } while (member !== node)
      if (component.length > 1) {
        cycles.push(component.sort((left, right) => documentIndex.get(left)! - documentIndex.get(right)!))
      }
    }
  }

  for (const node of order) {
    if (!index.has(node)) connect(node)
  }
  return cycles.sort((left, right) => documentIndex.get(left[0]!)! - documentIndex.get(right[0]!)!)
}

export const analyzeObligationDependencies = (
  obligations: ReadonlyArray<AssuranceObligation>,
): ObligationDependencyAnalysis => {
  const order = obligations.map((obligation) => obligation.id)
  const known = new Set(order)
  const issues: ObligationDependencyIssue[] = []
  const edges: ObligationDependencyEdge[] = []
  const waitsOn = new Map<string, ReadonlyArray<string>>()

  for (const obligation of obligations) {
    const resolved: string[] = []
    const seen = new Set<string>()
    for (const dependencyRef of obligation.dependency_refs ?? []) {
      if (seen.has(dependencyRef)) continue
      seen.add(dependencyRef)
      if (dependencyRef === obligation.id) {
        issues.push({
          code: "self_obligation_dependency",
          message: `Obligation ${obligation.id} declares a dependency on itself.`,
          obligation_id: obligation.id,
          dependency_ref: dependencyRef,
        })
        continue
      }
      if (!known.has(dependencyRef)) {
        issues.push({
          code: "dangling_dependency_ref",
          message: `Obligation ${obligation.id} depends on unknown obligation ${dependencyRef}.`,
          obligation_id: obligation.id,
          dependency_ref: dependencyRef,
        })
        continue
      }
      resolved.push(dependencyRef)
      edges.push({ from: obligation.id, to: dependencyRef })
    }
    waitsOn.set(obligation.id, resolved)
  }

  const cycles = findDependencyCycles(order, waitsOn)
  for (const cycle of cycles) {
    issues.push({
      code: "cyclic_obligation_dependency",
      message: `Obligation dependency cycle among: ${cycle.join(", ")}.`,
      obligation_id: cycle[0]!,
      cycle,
    })
  }

  return { edges, waits_on: waitsOn, cycles, issues }
}

// ---------------------------------------------------------------------------
// Designable-now vs blocked vs gated projection
// ---------------------------------------------------------------------------

export type ObligationGraphStatus = "designable_now" | "blocked" | "gated"

export type ObligationGraphNode = Readonly<{
  obligation_id: string
  status: ObligationGraphStatus
  waits_on: ReadonlyArray<string>
  activation_gate: string | null
}>

export type ObligationGraph = Readonly<{
  designable_now: ReadonlyArray<string>
  blocked: ReadonlyArray<Readonly<{ obligation_id: string; waits_on: ReadonlyArray<string> }>>
  gated: ReadonlyArray<Readonly<{ obligation_id: string; activation_gate: string }>>
  /**
   * Dependency-respecting order for proof design, document order among
   * independents. Obligations inside or downstream of a dependency cycle are
   * omitted (a structurally valid spec has none). Never an execution
   * manifest ordering (ASSURANCE_SPEC §9).
   */
  design_order: ReadonlyArray<string>
  nodes: ReadonlyArray<ObligationGraphNode>
  edges: ReadonlyArray<ObligationDependencyEdge>
  warnings: ReadonlyArray<ObligationDependencyIssue>
  message: string
}>

export const projectObligationGraph = (document: AssuranceSpecDocument): ObligationGraph => {
  const analysis = analyzeObligationDependencies(document.obligations)
  const order = document.obligations.map((obligation) => obligation.id)
  const documentIndex = new Map(order.map((id, position) => [id, position] as const))
  const gateById = new Map(
    document.obligations.map((obligation) => [obligation.id, obligation.activation_gate ?? null] as const),
  )

  const nodes: ObligationGraphNode[] = order.map((obligationId) => {
    const waits = analysis.waits_on.get(obligationId) ?? []
    const activationGate = gateById.get(obligationId) ?? null
    const status: ObligationGraphStatus = waits.length > 0
      ? "blocked"
      : activationGate !== null
        ? "gated"
        : "designable_now"
    return { obligation_id: obligationId, status, waits_on: waits, activation_gate: activationGate }
  })

  // Kahn's algorithm with a document-order frontier: deterministic, and
  // stable under unrelated edits because ties break by document position.
  const remaining = new Map<string, number>()
  const dependents = new Map<string, string[]>()
  for (const obligationId of order) {
    remaining.set(obligationId, (analysis.waits_on.get(obligationId) ?? []).length)
    dependents.set(obligationId, [])
  }
  for (const edge of analysis.edges) dependents.get(edge.to)!.push(edge.from)
  const frontier = order.filter((obligationId) => remaining.get(obligationId) === 0)
  const designOrder: string[] = []
  while (frontier.length > 0) {
    frontier.sort((left, right) => documentIndex.get(left)! - documentIndex.get(right)!)
    const next = frontier.shift()!
    designOrder.push(next)
    for (const dependent of dependents.get(next) ?? []) {
      const left = remaining.get(dependent)! - 1
      remaining.set(dependent, left)
      if (left === 0) frontier.push(dependent)
    }
  }

  return {
    designable_now: nodes
      .filter((node) => node.status === "designable_now")
      .map((node) => node.obligation_id),
    blocked: nodes
      .filter((node) => node.status === "blocked")
      .map((node) => ({ obligation_id: node.obligation_id, waits_on: node.waits_on })),
    gated: nodes
      .filter((node) => node.status === "gated")
      .map((node) => ({ obligation_id: node.obligation_id, activation_gate: node.activation_gate! })),
    design_order: designOrder,
    nodes,
    edges: analysis.edges,
    warnings: analysis.issues,
    message:
      "Declared dependency_refs and activation gates only. No receipts exist: nothing here claims a dependency was satisfied, design_order is a proof-design order (never an execution manifest ordering), and no blended score is computed (Law 7).",
  }
}

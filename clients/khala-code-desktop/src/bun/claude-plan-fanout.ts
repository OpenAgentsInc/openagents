import { Schema as S } from "effect"

import type {
  PlanDagWorkSource,
  PlanDagWorkUnit,
} from "../../../../apps/pylon/src/orchestration/work-planner.js"

export const CLAUDE_PLAN_FANOUT_DAG_SCHEMA = "openagents.khala_code.claude_plan_fanout_dag.v1" as const
export const CLAUDE_PLAN_FANOUT_REVIEW_SCHEMA = "openagents.khala_code.claude_plan_fanout_review.v1" as const

export const ClaudePlanFanoutDagNodeSchema = S.Struct({
  nodeRef: S.String,
  title: S.String,
  objective: S.String,
  dependsOn: S.optional(S.Array(S.String)),
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
  issue: S.optional(S.Number),
  labels: S.optional(S.Array(S.String)),
  evidenceRefs: S.optional(S.Array(S.String)),
})
export type ClaudePlanFanoutDagNode = typeof ClaudePlanFanoutDagNodeSchema.Type

export const ClaudePlanFanoutDagSchema = S.Struct({
  schema: S.Literal(CLAUDE_PLAN_FANOUT_DAG_SCHEMA),
  planRef: S.String,
  source: S.Literal("claude_plan_mode"),
  generatedAt: S.String,
  objective: S.String,
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
  evidenceRefs: S.optional(S.Array(S.String)),
  nodes: S.Array(ClaudePlanFanoutDagNodeSchema),
})
export type ClaudePlanFanoutDag = typeof ClaudePlanFanoutDagSchema.Type

export const ClaudePlanFanoutReviewVerdictSchema = S.Literals(["accept", "request_changes", "replan"])
export type ClaudePlanFanoutReviewVerdict = typeof ClaudePlanFanoutReviewVerdictSchema.Type

export const ClaudePlanFanoutReviewSchema = S.Struct({
  schema: S.Literal(CLAUDE_PLAN_FANOUT_REVIEW_SCHEMA),
  reviewRef: S.String,
  planRef: S.String,
  generatedAt: S.String,
  verdict: ClaudePlanFanoutReviewVerdictSchema,
  summary: S.String,
  targetNodeRefs: S.optional(S.Array(S.String)),
  changeRequests: S.optional(S.Array(S.String)),
  evidenceRefs: S.optional(S.Array(S.String)),
})
export type ClaudePlanFanoutReview = typeof ClaudePlanFanoutReviewSchema.Type

export type ClaudePlanFanoutReviewAdvisorySignal = {
  readonly advisory: true
  readonly controlFlowAuthority: "khala.fleet.delegate"
  readonly deterministicGateRequired: true
  readonly planRef: string
  readonly reviewRef: string
  readonly targetNodeRefs: readonly string[]
  readonly verdict: ClaudePlanFanoutReviewVerdict
}

export class ClaudePlanFanoutContractError extends Error {
  readonly _tag = "ClaudePlanFanoutContractError"

  constructor(message: string) {
    super(message)
    this.name = "ClaudePlanFanoutContractError"
  }
}

const MAX_PLAN_NODES = 25
const MAX_TEXT_LENGTH = 4_000
const MAX_TITLE_LENGTH = 160
const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,180}$/u
const UNSAFE_TEXT_PATTERNS: readonly RegExp[] = [
  /(^|[\s"'`])\/Users\//iu,
  /(^|[\s"'`])\/private\//iu,
  /(^|[\s"'`])~\//iu,
  /\.secrets\//iu,
  /\b(?:OPENAI|ANTHROPIC|STRIPE|MDK|NEXUS|PROBE)_[A-Z0-9_]*(?:KEY|TOKEN|SECRET)\b/iu,
  /\bBearer\s+[A-Za-z0-9._~+/=-]{12,}/u,
  /[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/iu,
]

const nonEmpty = (field: string, value: string): string => {
  const trimmed = value.trim()
  if (trimmed.length === 0) throw new ClaudePlanFanoutContractError(`${field} is required`)
  return trimmed
}

const assertText = (field: string, value: string, maxLength = MAX_TEXT_LENGTH): void => {
  const trimmed = nonEmpty(field, value)
  if (trimmed.length > maxLength) {
    throw new ClaudePlanFanoutContractError(`${field} is too long`)
  }
  const unsafe = UNSAFE_TEXT_PATTERNS.find((pattern) => pattern.test(trimmed))
  if (unsafe !== undefined) {
    throw new ClaudePlanFanoutContractError(`${field} is not public-safe`)
  }
}

const assertPublicRef = (field: string, value: string): void => {
  const trimmed = nonEmpty(field, value)
  if (!PUBLIC_REF_PATTERN.test(trimmed)) {
    throw new ClaudePlanFanoutContractError(`${field} must be a public-safe ref`)
  }
  assertText(field, trimmed, 181)
}

const assertIsoDate = (field: string, value: string): void => {
  assertText(field, value, 80)
  if (Number.isNaN(Date.parse(value))) {
    throw new ClaudePlanFanoutContractError(`${field} must be ISO-compatible`)
  }
}

const assertOptionalText = (field: string, value: string | undefined, maxLength = MAX_TEXT_LENGTH): void => {
  if (value !== undefined) assertText(field, value, maxLength)
}

const assertOptionalRefs = (field: string, refs: readonly string[] | undefined): void => {
  for (const ref of refs ?? []) assertPublicRef(field, ref)
}

export function validateClaudePlanFanoutDag(dag: ClaudePlanFanoutDag): ClaudePlanFanoutDag {
  assertPublicRef("planRef", dag.planRef)
  assertIsoDate("generatedAt", dag.generatedAt)
  assertText("objective", dag.objective)
  assertOptionalText("repo", dag.repo, 120)
  assertOptionalText("branch", dag.branch, 120)
  assertOptionalText("baseCommit", dag.baseCommit, 80)
  assertOptionalText("verify", dag.verify)
  assertOptionalRefs("evidenceRefs", dag.evidenceRefs)
  if (dag.nodes.length < 1) throw new ClaudePlanFanoutContractError("nodes must not be empty")
  if (dag.nodes.length > MAX_PLAN_NODES) throw new ClaudePlanFanoutContractError("nodes exceeds bounded fan-out")

  const nodesByRef = new Map<string, ClaudePlanFanoutDagNode>()
  for (const node of dag.nodes) {
    assertPublicRef("nodeRef", node.nodeRef)
    if (nodesByRef.has(node.nodeRef)) {
      throw new ClaudePlanFanoutContractError(`duplicate nodeRef: ${node.nodeRef}`)
    }
    nodesByRef.set(node.nodeRef, node)
    assertText(`node ${node.nodeRef} title`, node.title, MAX_TITLE_LENGTH)
    assertText(`node ${node.nodeRef} objective`, node.objective)
    assertOptionalText(`node ${node.nodeRef} repo`, node.repo, 120)
    assertOptionalText(`node ${node.nodeRef} branch`, node.branch, 120)
    assertOptionalText(`node ${node.nodeRef} baseCommit`, node.baseCommit, 80)
    assertOptionalText(`node ${node.nodeRef} verify`, node.verify)
    assertOptionalRefs(`node ${node.nodeRef} labels`, node.labels)
    assertOptionalRefs(`node ${node.nodeRef} evidenceRefs`, node.evidenceRefs)
    if (node.issue !== undefined && (!Number.isInteger(node.issue) || node.issue < 1)) {
      throw new ClaudePlanFanoutContractError(`node ${node.nodeRef} issue must be a positive integer`)
    }
  }

  for (const node of dag.nodes) {
    for (const depRef of node.dependsOn ?? []) {
      assertPublicRef(`node ${node.nodeRef} dependsOn`, depRef)
      if (!nodesByRef.has(depRef)) {
        throw new ClaudePlanFanoutContractError(`node ${node.nodeRef} depends on unknown node ${depRef}`)
      }
    }
  }

  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (nodeRef: string, path: readonly string[]): void => {
    if (visited.has(nodeRef)) return
    if (visiting.has(nodeRef)) {
      throw new ClaudePlanFanoutContractError(`plan DAG contains a cycle: ${[...path, nodeRef].join(" -> ")}`)
    }
    visiting.add(nodeRef)
    const node = nodesByRef.get(nodeRef)
    if (node !== undefined) {
      for (const depRef of node.dependsOn ?? []) visit(depRef, [...path, nodeRef])
    }
    visiting.delete(nodeRef)
    visited.add(nodeRef)
  }
  for (const node of dag.nodes) visit(node.nodeRef, [])

  return dag
}

export function decodeClaudePlanFanoutDag(input: unknown): ClaudePlanFanoutDag {
  return validateClaudePlanFanoutDag(S.decodeUnknownSync(ClaudePlanFanoutDagSchema)(input))
}

const fencedJsonPattern = /```(?:json)?\s*([\s\S]*?)```/iu

export function parseClaudePlanFanoutDagFromText(text: string): ClaudePlanFanoutDag {
  const trimmed = text.trim()
  const fenced = fencedJsonPattern.exec(trimmed)?.[1]?.trim()
  const candidates = [
    fenced,
    trimmed,
    trimmed.slice(trimmed.indexOf("{"), trimmed.lastIndexOf("}") + 1),
  ].filter((value): value is string => value !== undefined && value.trim().length > 0)

  let lastError: unknown
  for (const candidate of candidates) {
    try {
      const parsed: unknown = JSON.parse(candidate)
      return decodeClaudePlanFanoutDag(parsed)
    } catch (error) {
      lastError = error
    }
  }
  throw new ClaudePlanFanoutContractError(
    `Claude plan output did not contain a valid ${CLAUDE_PLAN_FANOUT_DAG_SCHEMA} object: ${
      lastError instanceof Error ? lastError.message : String(lastError)
    }`,
  )
}

const nodeToWorkUnit = (node: ClaudePlanFanoutDagNode): PlanDagWorkUnit => ({
  ref: node.nodeRef,
  title: node.title,
  objective: node.objective,
  ...(node.dependsOn === undefined ? {} : { dependsOn: [...node.dependsOn] }),
  ...(node.repo === undefined ? {} : { repo: node.repo }),
  ...(node.branch === undefined ? {} : { branch: node.branch }),
  ...(node.baseCommit === undefined ? {} : { baseCommit: node.baseCommit }),
  ...(node.verify === undefined ? {} : { verify: node.verify }),
  ...(node.issue === undefined ? {} : { issue: node.issue }),
  ...(node.labels === undefined ? {} : { labels: [...node.labels] }),
})

export function claudePlanFanoutDagToWorkSource(dag: ClaudePlanFanoutDag): PlanDagWorkSource {
  const validated = validateClaudePlanFanoutDag(dag)
  return {
    kind: "plan_dag",
    planRef: validated.planRef,
    nodes: validated.nodes.map(nodeToWorkUnit),
    ...(validated.repo === undefined ? {} : { repo: validated.repo }),
    ...(validated.branch === undefined ? {} : { branch: validated.branch }),
    ...(validated.baseCommit === undefined ? {} : { baseCommit: validated.baseCommit }),
    ...(validated.verify === undefined ? {} : { verify: validated.verify }),
  }
}

export function validateClaudePlanFanoutReview(
  review: ClaudePlanFanoutReview,
  input: { readonly knownNodeRefs?: readonly string[] } = {},
): ClaudePlanFanoutReview {
  assertPublicRef("reviewRef", review.reviewRef)
  assertPublicRef("planRef", review.planRef)
  assertIsoDate("generatedAt", review.generatedAt)
  assertText("summary", review.summary)
  assertOptionalRefs("targetNodeRefs", review.targetNodeRefs)
  assertOptionalRefs("evidenceRefs", review.evidenceRefs)
  for (const request of review.changeRequests ?? []) assertText("changeRequests", request)
  if (input.knownNodeRefs !== undefined) {
    const known = new Set(input.knownNodeRefs)
    const unknown = (review.targetNodeRefs ?? []).find(ref => !known.has(ref))
    if (unknown !== undefined) {
      throw new ClaudePlanFanoutContractError(`review targets unknown node ${unknown}`)
    }
  }
  return review
}

export function decodeClaudePlanFanoutReview(
  input: unknown,
  options: { readonly knownNodeRefs?: readonly string[] } = {},
): ClaudePlanFanoutReview {
  return validateClaudePlanFanoutReview(S.decodeUnknownSync(ClaudePlanFanoutReviewSchema)(input), options)
}

export function claudePlanFanoutReviewAdvisorySignal(
  review: ClaudePlanFanoutReview,
): ClaudePlanFanoutReviewAdvisorySignal {
  const validated = validateClaudePlanFanoutReview(review)
  return {
    advisory: true,
    controlFlowAuthority: "khala.fleet.delegate",
    deterministicGateRequired: true,
    planRef: validated.planRef,
    reviewRef: validated.reviewRef,
    targetNodeRefs: [...(validated.targetNodeRefs ?? [])],
    verdict: validated.verdict,
  }
}

export function claudePlanFanoutPlanModeInstructions(): string {
  return [
    "You are in Claude plan mode for Khala Code plan-then-fan-out.",
    "Do not edit files or dispatch workers directly.",
    `Emit one JSON object matching ${CLAUDE_PLAN_FANOUT_DAG_SCHEMA}.`,
    "Use source='claude_plan_mode'. Nodes must form an acyclic task DAG.",
    "Each node objective must be public-safe and bounded for a Codex worker.",
    "Use dependsOn nodeRef values only; deterministic FleetRun supervision owns control flow.",
  ].join("\n")
}

import { Schema as S } from "effect"
import { FleetWorkUnitPlacementPolicy } from "@openagentsinc/khala-fleet-intents"

import { buildPylonKhalaGitCheckoutWorkspace } from "../khala-requester.js"
import { assertPublicProjectionSafe } from "../state.js"
import { assertPublicSafe } from "../work-requester.js"

export const FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA =
  "openagents.pylon.fleet_run_work_source.v1" as const

const WorkItemStateSchema = S.Literals(["open", "closed", "merged", "OPEN", "CLOSED", "MERGED"])

export const FleetRunIssueListItemSchema = S.Struct({
  number: S.Number,
  kind: S.optional(S.Literals(["issue", "pr"])),
  labels: S.optional(S.Array(S.String)),
  state: S.optional(WorkItemStateSchema),
  title: S.optional(S.String),
  body: S.optional(S.String),
  url: S.optional(S.String),
})

export const FleetRunFixtureUnitSchema = S.Struct({
  ref: S.String,
  title: S.optional(S.String),
  labels: S.optional(S.Array(S.String)),
  state: S.optional(S.Literals(["open", "closed", "merged"])),
})

export const FleetRunPlanDagNodeSchema = S.Struct({
  ref: S.String,
  title: S.String,
  objective: S.String,
  dependsOn: S.optional(S.Array(S.String)),
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
  issue: S.optional(S.Number),
  labels: S.optional(S.Array(S.String)),
  url: S.optional(S.String),
  placement: S.optional(FleetWorkUnitPlacementPolicy),
})

export const FleetRunFixtureWorkSourceDescriptorSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA),
  kind: S.Literal("fixture"),
  count: S.optional(S.Number),
  units: S.optional(S.Array(FleetRunFixtureUnitSchema)),
})

export const FleetRunIssueListWorkSourceDescriptorSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA),
  kind: S.Literal("issue_list"),
  repo: S.String,
  branch: S.String,
  baseCommit: S.String,
  verify: S.String,
  issues: S.Array(S.Union([S.Number, FleetRunIssueListItemSchema])),
  pullRequests: S.optional(S.Array(FleetRunIssueListItemSchema)),
})

export const FleetRunGithubBacklogWorkSourceDescriptorSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA),
  kind: S.Literal("github_backlog"),
  repo: S.String,
  branch: S.String,
  baseCommit: S.String,
  verify: S.String,
  limit: S.optional(S.Number),
})

export const FleetRunPlanDagWorkSourceDescriptorSchema = S.Struct({
  schema: S.Literal(FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA),
  kind: S.Literal("plan_dag"),
  planRef: S.String,
  repo: S.optional(S.String),
  branch: S.optional(S.String),
  baseCommit: S.optional(S.String),
  verify: S.optional(S.String),
  nodes: S.Array(FleetRunPlanDagNodeSchema),
})

export const FleetRunWorkSourceDescriptorSchema = S.Union([
  FleetRunFixtureWorkSourceDescriptorSchema,
  FleetRunIssueListWorkSourceDescriptorSchema,
  FleetRunGithubBacklogWorkSourceDescriptorSchema,
  FleetRunPlanDagWorkSourceDescriptorSchema,
])

export type FleetRunIssueListItem = typeof FleetRunIssueListItemSchema.Type
export type FleetRunFixtureUnit = typeof FleetRunFixtureUnitSchema.Type
export type FleetRunPlanDagNode = typeof FleetRunPlanDagNodeSchema.Type
export type FleetRunFixtureWorkSourceDescriptor = typeof FleetRunFixtureWorkSourceDescriptorSchema.Type
export type FleetRunIssueListWorkSourceDescriptor = typeof FleetRunIssueListWorkSourceDescriptorSchema.Type
export type FleetRunGithubBacklogWorkSourceDescriptor = typeof FleetRunGithubBacklogWorkSourceDescriptorSchema.Type
export type FleetRunPlanDagWorkSourceDescriptor = typeof FleetRunPlanDagWorkSourceDescriptorSchema.Type
export type FleetRunWorkSourceDescriptor = typeof FleetRunWorkSourceDescriptorSchema.Type

const PUBLIC_REF_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._:/#-]{0,180}$/u
const MAX_DESCRIPTOR_BYTES = 128 * 1024
const MAX_FIXTURE_UNITS = 1_000
const MAX_ISSUES = 1_000
const MAX_LABELS = 50
const MAX_LABEL_LENGTH = 80
const MAX_NODES = 500
const MAX_DEPENDENCIES = 100
const MAX_TITLE_LENGTH = 240
const MAX_BODY_LENGTH = 8_000
const MAX_URL_LENGTH = 2_048
const MAX_REPOSITORY_LENGTH = 240
const MAX_BRANCH_LENGTH = 120
const MAX_VERIFY_LENGTH = 2_500
const VALIDATION_COMMIT = "0123456789abcdef0123456789abcdef01234567"

const assertPublicRef = (field: string, value: string): string => {
  const ref = value.trim()
  if (!PUBLIC_REF_PATTERN.test(ref)) throw new Error(`${field} must be a public-safe ref`)
  return ref
}

const assertPositiveInteger = (field: string, value: number, max = Number.MAX_SAFE_INTEGER): number => {
  if (!Number.isInteger(value) || value < 1 || value > max) {
    throw new Error(`${field} must be a positive integer no greater than ${max}`)
  }
  return value
}

const boundedString = (
  field: string,
  value: string,
  input: { readonly min?: number; readonly max: number },
): string => {
  const normalized = value.trim()
  const min = input.min ?? 0
  if (normalized.length < min || normalized.length > input.max) {
    throw new Error(`${field} must be ${min}-${input.max} characters`)
  }
  return normalized
}

const boundedLabels = (field: string, labels: readonly string[] | undefined): readonly string[] | undefined => {
  if (labels === undefined) return undefined
  if (labels.length > MAX_LABELS) throw new Error(`${field} must contain at most ${MAX_LABELS} labels`)
  return labels.map((label) => boundedString(`${field} label`, label, { min: 1, max: MAX_LABEL_LENGTH }))
}

const boundedUrl = (field: string, value: string | undefined): string | undefined => {
  if (value === undefined) return undefined
  const url = boundedString(field, value, { min: 1, max: MAX_URL_LENGTH })
  let parsed: URL
  try {
    parsed = new URL(url)
  } catch {
    throw new Error(`${field} must be a public GitHub URL`)
  }
  if (parsed.protocol !== "https:" || parsed.hostname !== "github.com") {
    throw new Error(`${field} must be a public GitHub URL`)
  }
  return url
}

const normalizePinnedCheckout = (input: {
  readonly repo: string | undefined
  readonly branch: string | undefined
  readonly baseCommit: string | undefined
  readonly verify: string | undefined
}) => {
  const repo = input.repo === undefined
    ? undefined
    : boundedString("repository", input.repo, { min: 1, max: MAX_REPOSITORY_LENGTH })
  const branch = input.branch === undefined
    ? undefined
    : boundedString("branch", input.branch, { min: 1, max: MAX_BRANCH_LENGTH })
  const baseCommit = input.baseCommit === undefined
    ? undefined
    : boundedString("base commit", input.baseCommit, { min: 40, max: 40 })
  const verify = input.verify === undefined
    ? undefined
    : boundedString("verification command", input.verify, { min: 1, max: MAX_VERIFY_LENGTH })
  const workspace = buildPylonKhalaGitCheckoutWorkspace({
    ...(repo === undefined ? {} : { repository: repo }),
    ...(branch === undefined ? {} : { branch }),
    ...(baseCommit === undefined ? {} : { commit: baseCommit }),
    ...(verify === undefined ? {} : { verificationCommand: verify }),
  })
  return {
    repo: workspace.repository.fullName,
    branch: workspace.repository.branch,
    baseCommit: workspace.repository.commitSha,
    verify: workspace.verificationCommand.args.join(" "),
  }
}

const normalizeOptionalPin = (
  kind: "repo" | "branch" | "baseCommit" | "verify",
  value: string | undefined,
): string | undefined => {
  if (value === undefined) return undefined
  const workspace = normalizePinnedCheckout({
    repo: kind === "repo" ? value : "OpenAgentsInc/openagents",
    branch: kind === "branch" ? value : "main",
    baseCommit: kind === "baseCommit" ? value : VALIDATION_COMMIT,
    verify: kind === "verify" ? value : "pnpm test",
  })
  return workspace[kind]
}

const normalizeIssueItem = (item: number | FleetRunIssueListItem): number | FleetRunIssueListItem => {
  if (typeof item === "number") return assertPositiveInteger("issue_list issue number", item)
  assertPositiveInteger("issue_list issue number", item.number, 2_147_483_647)
  const labels = boundedLabels("issue_list", item.labels)
  const url = boundedUrl("issue_list url", item.url)
  return {
    number: item.number,
    ...(item.kind === undefined ? {} : { kind: item.kind }),
    ...(labels === undefined ? {} : { labels }),
    ...(item.state === undefined ? {} : { state: item.state }),
    ...(item.title === undefined
      ? {}
      : { title: boundedString("issue_list title", item.title, { min: 1, max: MAX_TITLE_LENGTH }) }),
    ...(item.body === undefined
      ? {}
      : { body: boundedString("issue_list body", item.body, { max: MAX_BODY_LENGTH }) }),
    ...(url === undefined ? {} : { url }),
  }
}

const assertDag = (source: FleetRunPlanDagWorkSourceDescriptor): void => {
  assertPublicRef("plan_dag planRef", source.planRef)
  if (source.nodes.length === 0) throw new Error("plan_dag requires at least one node")
  if (source.nodes.length > MAX_NODES) throw new Error(`plan_dag supports at most ${MAX_NODES} nodes`)
  const nodes = new Map<string, FleetRunPlanDagNode>()
  for (const node of source.nodes) {
    const ref = assertPublicRef("plan_dag node ref", node.ref)
    if (nodes.has(ref)) throw new Error(`plan_dag duplicate node ref: ${ref}`)
    boundedString(`plan_dag node ${ref} title`, node.title, { min: 1, max: MAX_TITLE_LENGTH })
    boundedString(`plan_dag node ${ref} objective`, node.objective, { min: 3, max: MAX_BODY_LENGTH })
    boundedLabels(`plan_dag node ${ref}`, node.labels)
    boundedUrl(`plan_dag node ${ref} url`, node.url)
    if (node.placement !== undefined) {
      S.decodeUnknownSync(FleetWorkUnitPlacementPolicy)(node.placement, {
        onExcessProperty: "error",
      })
    }
    if ((node.dependsOn?.length ?? 0) > MAX_DEPENDENCIES) {
      throw new Error(`plan_dag node ${ref} supports at most ${MAX_DEPENDENCIES} dependencies`)
    }
    if (node.issue !== undefined) assertPositiveInteger(`plan_dag node ${ref} issue`, node.issue, 2_147_483_647)
    nodes.set(ref, node)
  }
  for (const node of source.nodes) {
    for (const dependency of node.dependsOn ?? []) {
      const dependencyRef = assertPublicRef(`plan_dag node ${node.ref} dependency`, dependency)
      if (!nodes.has(dependencyRef)) {
        throw new Error(`plan_dag node ${node.ref} depends on unknown node: ${dependencyRef}`)
      }
    }
  }
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const visit = (ref: string, path: readonly string[]): void => {
    if (visited.has(ref)) return
    if (visiting.has(ref)) throw new Error(`plan_dag contains a cycle: ${[...path, ref].join(" -> ")}`)
    visiting.add(ref)
    for (const dependency of nodes.get(ref)?.dependsOn ?? []) visit(dependency, [...path, ref])
    visiting.delete(ref)
    visited.add(ref)
  }
  for (const ref of nodes.keys()) visit(ref, [])
}

/** Strict, normalized decode for the descriptor persisted in FleetRun JSON. */
export function decodeFleetRunWorkSourceDescriptor(input: unknown): FleetRunWorkSourceDescriptor {
  const encoded = JSON.stringify(input)
  if (encoded === undefined || new TextEncoder().encode(encoded).byteLength > MAX_DESCRIPTOR_BYTES) {
    throw new Error(`fleet run work-source descriptor must be at most ${MAX_DESCRIPTOR_BYTES} bytes`)
  }
  assertPublicSafe(input, "fleet run work-source descriptor")
  assertPublicProjectionSafe(input, "fleetRun.workSourceDescriptor")
  const descriptor = S.decodeUnknownSync(FleetRunWorkSourceDescriptorSchema)(input, {
    onExcessProperty: "error",
  })

  if (descriptor.kind === "fixture") {
    if (descriptor.count !== undefined) assertPositiveInteger("fixture count", descriptor.count, MAX_FIXTURE_UNITS)
    if (descriptor.units !== undefined && descriptor.units.length > MAX_FIXTURE_UNITS) {
      throw new Error(`fixture supports at most ${MAX_FIXTURE_UNITS} units`)
    }
    if (descriptor.count !== undefined && descriptor.units !== undefined) {
      throw new Error("fixture descriptor must use count or units, not both")
    }
    const refs = new Set<string>()
    const units = descriptor.units?.map((unit) => {
      const ref = assertPublicRef("fixture unit ref", unit.ref)
      if (refs.has(ref)) throw new Error(`fixture duplicate unit ref: ${ref}`)
      refs.add(ref)
      const labels = boundedLabels("fixture unit", unit.labels)
      return {
        ref,
        ...(unit.title === undefined
          ? {}
          : { title: boundedString("fixture unit title", unit.title, { min: 1, max: MAX_TITLE_LENGTH }) }),
        ...(labels === undefined ? {} : { labels }),
        ...(unit.state === undefined ? {} : { state: unit.state }),
      }
    })
    if (units !== undefined && units.length === 0) throw new Error("fixture units must not be empty")
    return {
      schema: FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA,
      kind: "fixture",
      ...(descriptor.count === undefined ? {} : { count: descriptor.count }),
      ...(units === undefined ? {} : { units }),
    }
  }

  if (descriptor.kind === "issue_list") {
    if (descriptor.issues.length === 0) throw new Error("issue_list requires at least one issue")
    if (descriptor.issues.length > MAX_ISSUES) throw new Error(`issue_list supports at most ${MAX_ISSUES} issues`)
    if ((descriptor.pullRequests?.length ?? 0) > MAX_ISSUES) {
      throw new Error(`issue_list supports at most ${MAX_ISSUES} pull requests`)
    }
    const pins = normalizePinnedCheckout(descriptor)
    return {
      schema: FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA,
      kind: "issue_list",
      ...pins,
      issues: descriptor.issues.map(normalizeIssueItem),
      ...(descriptor.pullRequests === undefined
        ? {}
        : { pullRequests: descriptor.pullRequests.map((item) => normalizeIssueItem(item) as FleetRunIssueListItem) }),
    }
  }

  if (descriptor.kind === "github_backlog") {
    const pins = normalizePinnedCheckout(descriptor)
    return {
      schema: FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA,
      kind: "github_backlog",
      ...pins,
      ...(descriptor.limit === undefined
        ? {}
        : { limit: assertPositiveInteger("github_backlog limit", descriptor.limit, MAX_ISSUES) }),
    }
  }

  assertDag(descriptor)
  const normalizedRoot = {
    repo: normalizeOptionalPin("repo", descriptor.repo),
    branch: normalizeOptionalPin("branch", descriptor.branch) ?? "main",
    baseCommit: normalizeOptionalPin("baseCommit", descriptor.baseCommit),
    verify: normalizeOptionalPin("verify", descriptor.verify),
  }
  const nodes = descriptor.nodes.map((node) => {
    const pins = normalizePinnedCheckout({
      repo: node.repo ?? normalizedRoot.repo,
      branch: node.branch ?? normalizedRoot.branch,
      baseCommit: node.baseCommit ?? normalizedRoot.baseCommit,
      verify: node.verify ?? normalizedRoot.verify,
    })
    const labels = boundedLabels(`plan_dag node ${node.ref}`, node.labels)
    const url = boundedUrl(`plan_dag node ${node.ref} url`, node.url)
    return {
      ref: node.ref.trim(),
      title: boundedString("plan_dag node title", node.title, { min: 1, max: MAX_TITLE_LENGTH }),
      objective: boundedString("plan_dag node objective", node.objective, { min: 3, max: MAX_BODY_LENGTH }),
      ...pins,
      ...(node.dependsOn === undefined ? {} : { dependsOn: node.dependsOn.map((ref) => ref.trim()) }),
      ...(node.issue === undefined ? {} : { issue: node.issue }),
      ...(labels === undefined ? {} : { labels }),
      ...(url === undefined ? {} : { url }),
      ...(node.placement === undefined ? {} : { placement: node.placement }),
    }
  })
  return {
    schema: FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA,
    kind: "plan_dag",
    planRef: descriptor.planRef.trim(),
    nodes,
    ...(normalizedRoot.repo === undefined ? {} : { repo: normalizedRoot.repo }),
    branch: normalizedRoot.branch,
    ...(normalizedRoot.baseCommit === undefined ? {} : { baseCommit: normalizedRoot.baseCommit }),
    ...(normalizedRoot.verify === undefined ? {} : { verify: normalizedRoot.verify }),
  }
}

export const fleetRunWorkSourceDescriptorFrom = (
  source: unknown,
): FleetRunWorkSourceDescriptor => {
  if (typeof source !== "object" || source === null || Array.isArray(source)) {
    throw new Error("fleet run work-source descriptor must be an object")
  }
  return decodeFleetRunWorkSourceDescriptor({
    schema: FLEET_RUN_WORK_SOURCE_DESCRIPTOR_SCHEMA,
    ...(source as Record<string, unknown>),
  })
}

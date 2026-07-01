import { Schema as S } from "effect"

import type { PylonOrchestrationStore } from "./store.js"

export const WORK_PLANNER_SCHEMA = "openagents.khala_code.work_planner.v1" as const

export const WorkPlannerSourceKindSchema = S.Literals(["github_backlog", "issue_list", "fixture"])
export type WorkPlannerSourceKind = typeof WorkPlannerSourceKindSchema.Type

export const WorkPlannerUnitKindSchema = S.Literals(["github_issue", "github_pr", "fixture"])
export type WorkPlannerUnitKind = typeof WorkPlannerUnitKindSchema.Type

export const WorkPlannerSkipReasonSchema = S.Literals([
  "already_claimed",
  "pr_exists",
  "merged",
  "closed",
  "needs_owner",
  "label_excluded",
])
export type WorkPlannerSkipReason = typeof WorkPlannerSkipReasonSchema.Type

export type WorkPlannerCandidateState = "open" | "closed" | "merged"

export type WorkPlannerCandidate = {
  readonly workUnitRef: string
  readonly kind: WorkPlannerUnitKind
  readonly title: string
  readonly source: WorkPlannerSourceKind
  readonly repo?: string
  readonly number?: number
  readonly url?: string
  readonly labels?: readonly string[]
  readonly state?: WorkPlannerCandidateState
  readonly body?: string
}

export type WorkPlannerClaimableUnit = WorkPlannerCandidate & {
  readonly status: "claimable"
}

export type WorkPlannerSkippedUnit = WorkPlannerCandidate & {
  readonly status: "skipped"
  readonly skipReason: WorkPlannerSkipReason
  readonly detail?: string
}

export type WorkPlannerUnit = WorkPlannerClaimableUnit | WorkPlannerSkippedUnit

export type WorkPlannerOutput = {
  readonly schema: typeof WORK_PLANNER_SCHEMA
  readonly source: WorkPlannerSourceKind
  readonly generatedAt: string
  readonly units: readonly WorkPlannerUnit[]
  readonly claimable: readonly WorkPlannerClaimableUnit[]
  readonly skipped: readonly WorkPlannerSkippedUnit[]
}

export type WorkPlannerClaimRegistry = Pick<PylonOrchestrationStore, "getLiveWorkClaim">

export type WorkPlannerOptions = {
  readonly now?: Date
  readonly claimRegistry?: WorkPlannerClaimRegistry
  readonly excludedLabels?: readonly string[]
  readonly needsOwnerLabels?: readonly string[]
  readonly pullRequests?: readonly WorkPlannerCandidate[]
}

export type IssueListWorkSource = {
  readonly kind: "issue_list"
  readonly repo: string
  readonly issues: readonly (number | IssueListItem)[]
  readonly pullRequests?: readonly IssueListItem[]
}

export type IssueListItem = {
  readonly number: number
  readonly title?: string
  readonly state?: WorkPlannerCandidateState | "OPEN" | "CLOSED" | "MERGED"
  readonly labels?: readonly string[]
  readonly body?: string
  readonly url?: string
  readonly kind?: "issue" | "pr"
}

export type FixtureWorkSource = {
  readonly kind: "fixture"
  readonly count?: number
  readonly units?: readonly FixtureWorkUnit[]
}

export type FixtureWorkUnit = {
  readonly ref: string
  readonly title?: string
  readonly labels?: readonly string[]
  readonly state?: WorkPlannerCandidateState
}

export type GithubBacklogWorkSource = {
  readonly kind: "github_backlog"
  readonly repo: string
  // gh defaults to 30 items, which silently truncates a real backlog; the
  // planner must see the whole candidate set (no silent drops).
  readonly limit?: number
}

export type GithubBacklogGhRunner = (args: readonly string[]) => Promise<string>

type GithubLabel = { readonly name?: unknown }
type GhIssueRecord = {
  readonly number?: unknown
  readonly title?: unknown
  readonly state?: unknown
  readonly labels?: unknown
  readonly body?: unknown
  readonly url?: unknown
}
type GhPullRequestRecord = GhIssueRecord & {
  readonly mergedAt?: unknown
}

const DEFAULT_NEEDS_OWNER_LABELS = ["needs-owner", "needs owner", "owner-needed", "owner needed"]

const normalizeState = (value: unknown): WorkPlannerCandidateState => {
  const normalized = typeof value === "string" ? value.trim().toLowerCase() : ""
  if (normalized === "merged") return "merged"
  if (normalized === "closed") return "closed"
  return "open"
}

const normalizeLabel = (label: string): string => label.trim().toLowerCase()

const normalizeLabels = (labels: unknown): string[] => {
  if (!Array.isArray(labels)) return []
  return labels
    .map((label) => {
      if (typeof label === "string") return label
      if (typeof label === "object" && label !== null && typeof (label as GithubLabel).name === "string") {
        return (label as { name: string }).name
      }
      return ""
    })
    .map((label) => label.trim())
    .filter((label) => label.length > 0)
}

const issueWorkUnitRef = (repo: string, issueNumber: number): string => `github:${repo}:issue:${issueNumber}`
const prWorkUnitRef = (repo: string, prNumber: number): string => `github:${repo}:pr:${prNumber}`

const issueRefPatterns = (issueNumber: number): RegExp[] => [
  new RegExp(`(?:close[sd]?|fix(?:e[sd])?|resolve[sd]?)\\s+#${issueNumber}(?!\\d)`, "i"),
  new RegExp(`#${issueNumber}(?!\\d)`, "i"),
]

const prReferencesIssue = (pr: WorkPlannerCandidate, issueNumber: number): boolean => {
  const haystack = `${pr.title}\n${pr.body ?? ""}`
  return issueRefPatterns(issueNumber).some((pattern) => pattern.test(haystack))
}

const skipForPrSibling = (
  candidate: WorkPlannerCandidate,
  pullRequests: readonly WorkPlannerCandidate[],
): WorkPlannerSkippedUnit | null => {
  if (candidate.kind !== "github_issue" || candidate.number === undefined) return null
  const matching = pullRequests.find((pr) => prReferencesIssue(pr, candidate.number!))
  if (matching === undefined) return null
  if (matching.state === "merged") {
    return skipped(candidate, "merged", `matching PR ${matching.number ?? matching.workUnitRef} is merged`)
  }
  if (matching.state === "closed") {
    return skipped(candidate, "closed", `matching PR ${matching.number ?? matching.workUnitRef} is closed`)
  }
  return skipped(candidate, "pr_exists", `matching PR ${matching.number ?? matching.workUnitRef} is open`)
}

const skipped = (
  candidate: WorkPlannerCandidate,
  skipReason: WorkPlannerSkipReason,
  detail?: string,
): WorkPlannerSkippedUnit => ({
  ...candidate,
  status: "skipped",
  skipReason,
  ...(detail === undefined ? {} : { detail }),
})

export const normalizeIssueListCandidate = (repo: string, item: number | IssueListItem): WorkPlannerCandidate => {
  const issue = typeof item === "number" ? { number: item } : item
  const kind: WorkPlannerUnitKind = issue.kind === "pr" ? "github_pr" : "github_issue"
  return {
    workUnitRef: kind === "github_pr" ? prWorkUnitRef(repo, issue.number) : issueWorkUnitRef(repo, issue.number),
    kind,
    source: "issue_list",
    repo,
    number: issue.number,
    title: issue.title ?? `${kind === "github_pr" ? "PR" : "Issue"} #${issue.number}`,
    state: normalizeState(issue.state),
    labels: [...(issue.labels ?? [])],
    body: issue.body,
    url: issue.url,
  }
}

export const fixtureCandidates = (source: FixtureWorkSource): WorkPlannerCandidate[] => {
  const units: readonly FixtureWorkUnit[] = source.units ?? Array.from({ length: source.count ?? 10 }, (_, index) => ({
    ref: `fixture.unit.${index + 1}`,
    title: `Fixture unit ${index + 1}`,
  }))
  return units.map((unit) => ({
    workUnitRef: `fixture:${unit.ref}`,
    kind: "fixture",
    source: "fixture",
    title: unit.title ?? unit.ref,
    labels: [...(unit.labels ?? [])],
    state: unit.state ?? "open",
  }))
}

export const issueListCandidates = (source: IssueListWorkSource): WorkPlannerCandidate[] => [
  ...source.issues.map((issue) => normalizeIssueListCandidate(source.repo, issue)),
  ...(source.pullRequests ?? []).map((pr) => normalizeIssueListCandidate(source.repo, { ...pr, kind: "pr" })),
]

const ghIssueRecordToCandidate = (repo: string, record: GhIssueRecord): WorkPlannerCandidate | null => {
  if (typeof record.number !== "number") return null
  return {
    workUnitRef: issueWorkUnitRef(repo, record.number),
    kind: "github_issue",
    source: "github_backlog",
    repo,
    number: record.number,
    title: typeof record.title === "string" && record.title.trim() ? record.title : `Issue #${record.number}`,
    state: normalizeState(record.state),
    labels: normalizeLabels(record.labels),
    body: typeof record.body === "string" ? record.body : undefined,
    url: typeof record.url === "string" ? record.url : undefined,
  }
}

const ghPullRequestRecordToCandidate = (repo: string, record: GhPullRequestRecord): WorkPlannerCandidate | null => {
  if (typeof record.number !== "number") return null
  const state = typeof record.mergedAt === "string" && record.mergedAt.trim() ? "merged" : normalizeState(record.state)
  return {
    workUnitRef: prWorkUnitRef(repo, record.number),
    kind: "github_pr",
    source: "github_backlog",
    repo,
    number: record.number,
    title: typeof record.title === "string" && record.title.trim() ? record.title : `PR #${record.number}`,
    state,
    labels: normalizeLabels(record.labels),
    body: typeof record.body === "string" ? record.body : undefined,
    url: typeof record.url === "string" ? record.url : undefined,
  }
}

const parseGhJsonArray = (raw: string, command: string): unknown[] => {
  const parsed: unknown = JSON.parse(raw)
  if (!Array.isArray(parsed)) throw new Error(`gh ${command} returned non-array JSON`)
  return parsed
}

export const githubBacklogCandidates = async (
  source: GithubBacklogWorkSource,
  gh: GithubBacklogGhRunner,
): Promise<WorkPlannerCandidate[]> => {
  const limit = String(source.limit ?? 1000)
  const issueArgs = [
    "issue",
    "list",
    "--repo",
    source.repo,
    "--state",
    "all",
    "--limit",
    limit,
    "--json",
    "number,title,state,labels,body,url",
  ]
  const prArgs = [
    "pr",
    "list",
    "--repo",
    source.repo,
    "--state",
    "all",
    "--limit",
    limit,
    "--json",
    "number,title,state,labels,body,url,mergedAt",
  ]
  const [issuesRaw, prsRaw] = await Promise.all([gh(issueArgs), gh(prArgs)])
  const issues = parseGhJsonArray(issuesRaw, "issue list")
    .map((record) => ghIssueRecordToCandidate(source.repo, record as GhIssueRecord))
    .filter((candidate): candidate is WorkPlannerCandidate => candidate !== null)
  const prs = parseGhJsonArray(prsRaw, "pr list")
    .map((record) => ghPullRequestRecordToCandidate(source.repo, record as GhPullRequestRecord))
    .filter((candidate): candidate is WorkPlannerCandidate => candidate !== null)
  return [...issues, ...prs]
}

export const planWorkCandidates = (
  source: WorkPlannerSourceKind,
  candidates: readonly WorkPlannerCandidate[],
  options: WorkPlannerOptions = {},
): WorkPlannerOutput => {
  const now = options.now ?? new Date()
  const excludedLabels = new Set((options.excludedLabels ?? []).map(normalizeLabel))
  const needsOwnerLabels = new Set((options.needsOwnerLabels ?? DEFAULT_NEEDS_OWNER_LABELS).map(normalizeLabel))
  const pullRequests = options.pullRequests ?? candidates.filter((candidate) => candidate.kind === "github_pr")

  const units = candidates.map((candidate): WorkPlannerUnit => {
    const labels = new Set((candidate.labels ?? []).map(normalizeLabel))
    const excludedLabel = [...labels].find((label) => excludedLabels.has(label))
    if (excludedLabel !== undefined) return skipped(candidate, "label_excluded", excludedLabel)
    const needsOwnerLabel = [...labels].find((label) => needsOwnerLabels.has(label))
    if (needsOwnerLabel !== undefined) return skipped(candidate, "needs_owner", needsOwnerLabel)
    if (candidate.state === "merged") return skipped(candidate, "merged")
    if (candidate.state === "closed") return skipped(candidate, "closed")
    const prSkip = skipForPrSibling(candidate, pullRequests)
    if (prSkip !== null) return prSkip
    const claim = options.claimRegistry?.getLiveWorkClaim(candidate.workUnitRef, now)
    if (claim !== undefined && claim !== null) return skipped(candidate, "already_claimed", claim.claimRef)
    return { ...candidate, status: "claimable" }
  })

  const claimable = units.filter((unit): unit is WorkPlannerClaimableUnit => unit.status === "claimable")
  const skippedUnits = units.filter((unit): unit is WorkPlannerSkippedUnit => unit.status === "skipped")
  return {
    schema: WORK_PLANNER_SCHEMA,
    source,
    generatedAt: now.toISOString(),
    units,
    claimable,
    skipped: skippedUnits,
  }
}

export const planIssueListWork = (
  source: IssueListWorkSource,
  options: WorkPlannerOptions = {},
): WorkPlannerOutput => {
  const candidates = issueListCandidates(source)
  return planWorkCandidates(source.kind, candidates, options)
}

export const planFixtureWork = (
  source: FixtureWorkSource,
  options: Omit<WorkPlannerOptions, "claimRegistry"> = {},
): WorkPlannerOutput => {
  const { claimRegistry: _claimRegistry, ...safeOptions } = options as WorkPlannerOptions
  return planWorkCandidates(source.kind, fixtureCandidates(source), safeOptions)
}

export const planGithubBacklogWork = async (
  source: GithubBacklogWorkSource,
  gh: GithubBacklogGhRunner,
  options: WorkPlannerOptions = {},
): Promise<WorkPlannerOutput> => {
  const candidates = await githubBacklogCandidates(source, gh)
  return planWorkCandidates(source.kind, candidates, options)
}

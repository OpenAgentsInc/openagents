import { createHash } from "node:crypto"

export type WorkIntakeSource = "github" | "forum" | "intent" | "market"

export type UnifiedWorkIntake = {
  readonly intakeId: string
  readonly source: WorkIntakeSource
  readonly title: string
  readonly body: string
  readonly originRef: string
  readonly receivedAt: string
}

export type GitHubIssueWorkIntake = {
  readonly source: "github"
  readonly owner: string
  readonly repo: string
  readonly issueNumber: number
  readonly title: string
  readonly body: string
  readonly receivedAt: string
}

export type ForumPostWorkIntake = {
  readonly source: "forum"
  readonly forumRef: string
  readonly postRef: string
  readonly title: string
  readonly body: string
  readonly receivedAt: string
}

export type DirectIntentWorkIntake = {
  readonly source: "intent"
  readonly intentId: string
  readonly submittedByClientRef: string
  readonly title: string
  readonly body: string
  readonly receivedAt: string
}

export type MarketWorkIntake = {
  readonly source: "market"
  readonly jobRef: string
  readonly title: string
  readonly body: string
  readonly receivedAt: string
}

export type RawWorkIntake =
  | GitHubIssueWorkIntake
  | ForumPostWorkIntake
  | DirectIntentWorkIntake
  | MarketWorkIntake

function stableRef(prefix: string, parts: readonly string[]) {
  return `${prefix}.${createHash("sha256").update(parts.join("\u001f")).digest("hex").slice(0, 24)}`
}

function intakeIdFor(input: Omit<UnifiedWorkIntake, "intakeId">) {
  return stableRef("work_intake", [
    input.source,
    input.title,
    input.body,
    input.originRef,
    input.receivedAt,
  ])
}

function unified(input: Omit<UnifiedWorkIntake, "intakeId">): UnifiedWorkIntake {
  return {
    intakeId: intakeIdFor(input),
    ...input,
  }
}

function normalizeGitHubIssue(raw: GitHubIssueWorkIntake): UnifiedWorkIntake {
  return unified({
    source: "github",
    title: raw.title,
    body: raw.body,
    originRef: `github.issue.${raw.owner}.${raw.repo}.${raw.issueNumber}`,
    receivedAt: raw.receivedAt,
  })
}

function normalizeForumPost(raw: ForumPostWorkIntake): UnifiedWorkIntake {
  return unified({
    source: "forum",
    title: raw.title,
    body: raw.body,
    originRef: `${raw.forumRef}.post.${raw.postRef}`,
    receivedAt: raw.receivedAt,
  })
}

function normalizeDirectIntent(raw: DirectIntentWorkIntake): UnifiedWorkIntake {
  return unified({
    source: "intent",
    title: raw.title,
    body: raw.body,
    originRef: `${raw.submittedByClientRef}.intent.${raw.intentId}`,
    receivedAt: raw.receivedAt,
  })
}

function normalizeMarketJob(raw: MarketWorkIntake): UnifiedWorkIntake {
  return unified({
    source: "market",
    title: raw.title,
    body: raw.body,
    originRef: raw.jobRef,
    receivedAt: raw.receivedAt,
  })
}

export function normalizeIntake(raw: RawWorkIntake): UnifiedWorkIntake {
  switch (raw.source) {
    case "github":
      return normalizeGitHubIssue(raw)
    case "forum":
      return normalizeForumPost(raw)
    case "intent":
      return normalizeDirectIntent(raw)
    case "market":
      return normalizeMarketJob(raw)
  }
}

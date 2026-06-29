#!/usr/bin/env bun

import { execFileSync } from "node:child_process"

export const OPENAGENTS_REPOSITORY = "OpenAgentsInc/openagents"

export const PRIORITY_LABELS = [
  "prio:0-pr-burndown",
  "prio:1-continual-learning",
  "prio:2-issue-triage",
  "prio:3-product-promises",
  "prio:4-backstop-burn",
] as const

export type PriorityLabel = (typeof PRIORITY_LABELS)[number]

export type GitHubIssue = Readonly<{
  body?: string | null | undefined
  labels: ReadonlyArray<Readonly<{ name: string }>>
  number: number
  title: string
  url?: string | undefined
}>

export type DuplicateCandidate = Readonly<{
  number: number
  score: number
  title: string
  url?: string | undefined
}>

export type IssueTriageDecision = Readonly<{
  duplicateCandidates: ReadonlyArray<DuplicateCandidate>
  issueNumber: number
  label: PriorityLabel
  plan: ReadonlyArray<string>
  relevantFiles: ReadonlyArray<string>
  rationale: string
}>

const STOP_WORDS = new Set([
  "a",
  "add",
  "an",
  "and",
  "for",
  "in",
  "is",
  "of",
  "on",
  "or",
  "the",
  "to",
  "with",
])

const AREA_RULES: ReadonlyArray<
  Readonly<{
    keywords: ReadonlyArray<string>
    label: PriorityLabel
    rationale: string
  }>
> = [
  {
    keywords: [
      "bug",
      "broken",
      "crash",
      "deploy",
      "failing",
      "failure",
      "fix",
      "regression",
      "security",
      "test",
    ],
    label: "prio:0-pr-burndown",
    rationale: "Looks like a concrete bug, regression, failing verification, or deploy blocker.",
  },
  {
    keywords: [
      "atif",
      "dspy",
      "eval",
      "evaluation",
      "gepa",
      "learning",
      "studybench",
      "trace",
      "training",
    ],
    label: "prio:1-continual-learning",
    rationale: "Touches learning, eval, trace, or optimization systems.",
  },
  {
    keywords: [
      "backlog",
      "dedup",
      "duplicate",
      "groom",
      "issue",
      "label",
      "ops",
      "scope",
      "triage",
    ],
    label: "prio:2-issue-triage",
    rationale: "Primarily backlog grooming, scoping, deduplication, or issue operations.",
  },
  {
    keywords: [
      "claim",
      "copy",
      "promise",
      "promises",
      "registry",
      "report",
    ],
    label: "prio:3-product-promises",
    rationale: "Touches product promises, public claims, report intake, or promise registry work.",
  },
  {
    keywords: [
      "benchmark",
      "benchmarks",
      "backstop",
      "gym",
      "ladder",
      "mirrorcode",
      "terminalbench",
    ],
    label: "prio:4-backstop-burn",
    rationale: "Matches benchmark, gym, MirrorCode, or backstop-burn work.",
  },
]

const uniqueSorted = (values: ReadonlyArray<string>): ReadonlyArray<string> =>
  Array.from(new Set(values)).sort((a, b) => a.localeCompare(b))

const issueText = (issue: Pick<GitHubIssue, "body" | "title">): string =>
  `${issue.title}\n${issue.body ?? ""}`

export const issueHasPriorityLabel = (issue: GitHubIssue): boolean =>
  issue.labels.some(label => PRIORITY_LABELS.includes(label.name as PriorityLabel))

export const issueHasAnyLabel = (issue: GitHubIssue): boolean => issue.labels.length > 0

export const issueNeedsPriorityTriage = (issue: GitHubIssue): boolean => !issueHasPriorityLabel(issue)

export const tokenizeIssueText = (value: string): ReadonlyArray<string> =>
  uniqueSorted(
    value
      .toLowerCase()
      .replace(/https?:\/\/\S+/g, " ")
      .split(/[^a-z0-9._/-]+/g)
      .map(token => token.trim())
      .filter(token => token.length >= 3 && !STOP_WORDS.has(token)),
  )

export const classifyIssuePriority = (
  issue: Pick<GitHubIssue, "body" | "title">,
): Readonly<{ label: PriorityLabel; rationale: string }> => {
  const tokens = new Set(tokenizeIssueText(issueText(issue)))
  const scores = AREA_RULES.map(rule => ({
    ...rule,
    score: rule.keywords.reduce((sum, keyword) => sum + (tokens.has(keyword) ? 1 : 0), 0),
  }))
  const best = scores.sort((a, b) => b.score - a.score)[0]

  if (best !== undefined && best.score > 0) {
    return {
      label: best.label,
      rationale: best.rationale,
    }
  }

  return {
    label: "prio:2-issue-triage",
    rationale: "No stronger lane signal was found; defaulting to backlog triage for human review.",
  }
}

export const inferRelevantFiles = (
  issue: Pick<GitHubIssue, "body" | "title">,
  repositoryFiles: ReadonlyArray<string>,
): ReadonlyArray<string> => {
  const text = issueText(issue)
  const explicitPaths = Array.from(
    text.matchAll(
      /(?:^|[\s`"'(])((?:apps|clients|docs|packages|scripts)\/[A-Za-z0-9._/-]+)(?=$|[\s`"',).])/g,
    ),
    match => match[1],
  )

  const tokens = tokenizeIssueText(text).filter(token => token.length >= 4)
  const tokenMatches = repositoryFiles.filter(file => {
    const haystack = file.toLowerCase()
    return tokens.some(token => haystack.includes(token))
  })

  return uniqueSorted([...explicitPaths, ...tokenMatches]).slice(0, 12)
}

const similarityScore = (left: ReadonlyArray<string>, right: ReadonlyArray<string>): number => {
  const a = new Set(left)
  const b = new Set(right)
  const intersection = Array.from(a).filter(token => b.has(token)).length
  const union = new Set([...a, ...b]).size
  return union === 0 ? 0 : intersection / union
}

export const findDuplicateCandidates = (
  issue: GitHubIssue,
  openIssues: ReadonlyArray<GitHubIssue>,
  minScore = 0.42,
): ReadonlyArray<DuplicateCandidate> => {
  const sourceTokens = tokenizeIssueText(issueText(issue))

  return openIssues
    .filter(candidate => candidate.number !== issue.number)
    .map(candidate => ({
      number: candidate.number,
      score: similarityScore(sourceTokens, tokenizeIssueText(issueText(candidate))),
      title: candidate.title,
      url: candidate.url,
    }))
    .filter(candidate => candidate.score >= minScore)
    .sort((a, b) => b.score - a.score || a.number - b.number)
    .slice(0, 5)
}

export const buildTechnicalPlan = (
  issue: GitHubIssue,
  label: PriorityLabel,
  relevantFiles: ReadonlyArray<string>,
  duplicates: ReadonlyArray<DuplicateCandidate>,
): ReadonlyArray<string> => {
  const files =
    relevantFiles.length > 0
      ? `Inspect the likely touched files first: ${relevantFiles.slice(0, 6).join(", ")}.`
      : "Search the codebase by the issue title/body terms before editing."
  const duplicateStep =
    duplicates.length > 0
      ? `Compare against possible duplicate issue(s): ${duplicates.map(item => `#${item.number}`).join(", ")}.`
      : "No high-similarity open duplicate was found by the local title/body scan."

  return [
    files,
    duplicateStep,
    `Route the work through ${label}; adjust only if a maintainer has stronger product context.`,
    `Before closeout, run the smallest relevant verification and report exact command output for #${issue.number}.`,
  ]
}

export const triageIssue = (
  issue: GitHubIssue,
  input: Readonly<{
    openIssues: ReadonlyArray<GitHubIssue>
    repositoryFiles: ReadonlyArray<string>
  }>,
): IssueTriageDecision => {
  const priority = classifyIssuePriority(issue)
  const relevantFiles = inferRelevantFiles(issue, input.repositoryFiles)
  const duplicateCandidates = findDuplicateCandidates(issue, input.openIssues)

  return {
    duplicateCandidates,
    issueNumber: issue.number,
    label: priority.label,
    plan: buildTechnicalPlan(issue, priority.label, relevantFiles, duplicateCandidates),
    relevantFiles,
    rationale: priority.rationale,
  }
}

export const renderTriageComment = (decision: IssueTriageDecision): string => {
  const duplicates =
    decision.duplicateCandidates.length === 0
      ? "None found by the local similarity scan."
      : decision.duplicateCandidates
          .map(candidate => `- #${candidate.number} (${candidate.score.toFixed(2)}): ${candidate.title}`)
          .join("\n")
  const files =
    decision.relevantFiles.length === 0
      ? "No direct file hits; start with repo search from the issue terms."
      : decision.relevantFiles.map(file => `- \`${file}\``).join("\n")

  return [
    "Automated triage pass:",
    "",
    `Label: \`${decision.label}\``,
    `Rationale: ${decision.rationale}`,
    "",
    "Potential duplicates:",
    duplicates,
    "",
    "Likely relevant files:",
    files,
    "",
    "Execution plan:",
    ...decision.plan.map(step => `- ${step}`),
  ].join("\n")
}

const runGhJson = (args: ReadonlyArray<string>): unknown => {
  const output = execFileSync("gh", args, { encoding: "utf8" })
  return JSON.parse(output)
}

export const listRepositoryFiles = (cwd: string): ReadonlyArray<string> => {
  const output = execFileSync("git", ["ls-files"], { cwd, encoding: "utf8" })
  return uniqueSorted(output.split("\n").map(line => line.trim()).filter(Boolean))
}

export const buildCandidateIssueSearch = (includeLabeled: boolean): string =>
  includeLabeled
    ? "is:issue is:open sort:created-desc"
    : `is:issue is:open ${PRIORITY_LABELS.map(label => `-label:"${label}"`).join(" ")} sort:created-desc`

const listCandidateIssues = (
  repo: string,
  limit: number,
  includeLabeled: boolean,
): ReadonlyArray<GitHubIssue> => {
  const search = buildCandidateIssueSearch(includeLabeled)
  return runGhJson([
    "issue",
    "list",
    "--repo",
    repo,
    "--state",
    "open",
    "--limit",
    String(limit),
    "--search",
    search,
    "--json",
    "number,title,body,labels,url",
  ]) as ReadonlyArray<GitHubIssue>
}

const listOpenIssues = (repo: string, limit: number): ReadonlyArray<GitHubIssue> =>
  runGhJson([
    "issue",
    "list",
    "--repo",
    repo,
    "--state",
    "open",
    "--limit",
    String(limit),
    "--json",
    "number,title,body,labels,url",
  ]) as ReadonlyArray<GitHubIssue>

const parseArgs = (
  args: ReadonlyArray<string>,
): Readonly<{
  apply: boolean
  comment: boolean
  includeLabeled: boolean
  limit: number
  repo: string
}> => {
  const limitIndex = args.indexOf("--limit")
  const repoIndex = args.indexOf("--repo")
  const limit =
    limitIndex >= 0 && args[limitIndex + 1] !== undefined
      ? Number.parseInt(args[limitIndex + 1]!, 10)
      : 20

  return {
    apply: args.includes("--apply"),
    comment: args.includes("--comment"),
    includeLabeled: args.includes("--include-labeled"),
    limit: Number.isFinite(limit) && limit > 0 ? limit : 20,
    repo: repoIndex >= 0 && args[repoIndex + 1] !== undefined ? args[repoIndex + 1]! : OPENAGENTS_REPOSITORY,
  }
}

const main = () => {
  const options = parseArgs(Bun.argv.slice(2))
  const repositoryFiles = listRepositoryFiles(process.cwd())
  const candidates = listCandidateIssues(options.repo, options.limit, options.includeLabeled).filter(
    issue => options.includeLabeled || issueNeedsPriorityTriage(issue),
  )
  const openIssues = listOpenIssues(options.repo, Math.max(options.limit, 100))
  const decisions = candidates.map(issue =>
    triageIssue(issue, {
      openIssues,
      repositoryFiles,
    }),
  )

  for (const decision of decisions) {
    console.log(renderTriageComment(decision))
    console.log("")

    if (options.apply) {
      execFileSync("gh", [
        "issue",
        "edit",
        String(decision.issueNumber),
        "--repo",
        options.repo,
        "--add-label",
        decision.label,
      ], { stdio: "inherit" })

      if (options.comment) {
        execFileSync("gh", [
          "issue",
          "comment",
          String(decision.issueNumber),
          "--repo",
          options.repo,
          "--body",
          renderTriageComment(decision),
        ], { stdio: "inherit" })
      }
    }
  }

  if (!options.apply) {
    console.error("[github-issue-triage] dry run only; pass --apply to write labels.")
  }
}

if (import.meta.main) {
  main()
}

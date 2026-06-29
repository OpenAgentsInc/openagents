import { describe, expect, test } from "bun:test"
import { spawnSync } from "node:child_process"
import {
  buildTechnicalPlan,
  buildCandidateIssueSearch,
  classifyIssuePriority,
  findDuplicateCandidates,
  inferRelevantFiles,
  issueHasAnyLabel,
  issueHasPriorityLabel,
  renderTriageComment,
  tokenizeIssueText,
  triageIssue,
  type GitHubIssue,
} from "./github-issue-triage"

const issue = (overrides: Partial<GitHubIssue>): GitHubIssue => ({
  body: "",
  labels: [],
  number: 1,
  title: "fixture",
  ...overrides,
})

describe("github issue triage classification", () => {
  test("classifies concrete failures as pr burndown", () => {
    expect(
      classifyIssuePriority(
        issue({
          body: "The deploy verification is failing after the worker routing regression.",
          title: "fix: worker deploy regression",
        }),
      ),
    ).toMatchObject({ label: "prio:0-pr-burndown" })
  })

  test("classifies promise and benchmark lanes", () => {
    expect(
      classifyIssuePriority(
        issue({
          body: "Sync docs/promises/registry.md after the public product claim changed.",
          title: "docs: product promise registry update",
        }),
      ),
    ).toMatchObject({ label: "prio:3-product-promises" })

    expect(
      classifyIssuePriority(
        issue({
          body: "Run the MirrorCode benchmark ladder and record the backstop evidence.",
          title: "task: gym benchmark backstop",
        }),
      ),
    ).toMatchObject({ label: "prio:4-backstop-burn" })
  })

  test("defaults unclear issues to the triage lane", () => {
    expect(classifyIssuePriority(issue({ title: "misc follow-up" }))).toMatchObject({
      label: "prio:2-issue-triage",
    })
  })
})

describe("github issue triage duplicate and file inference", () => {
  test("tokenizes stable issue terms without noise", () => {
    expect(tokenizeIssueText("Fix the failing deploy check for apps/pylon/src/index.ts")).toEqual([
      "apps/pylon/src/index.ts",
      "check",
      "deploy",
      "failing",
      "fix",
    ])
  })

  test("finds high-similarity duplicate candidates", () => {
    const source = issue({
      number: 10,
      title: "fix pylon codex account status readiness",
      body: "accounts list should report readiness state",
    })
    const duplicate = issue({
      number: 11,
      title: "fix codex account readiness status in pylon",
      body: "pylon accounts list should report readiness state",
    })
    const unrelated = issue({
      number: 12,
      title: "docs product promise copy sync",
      body: "update promise registry",
    })

    expect(findDuplicateCandidates(source, [source, duplicate, unrelated], 0.35)).toEqual([
      expect.objectContaining({ number: 11 }),
    ])
  })

  test("infers explicit and token-matched repository files", () => {
    expect(
      inferRelevantFiles(
        issue({
          body: "Touch apps/pylon/src/accounts.ts and the khala CLI account surface.",
          title: "pylon account status",
        }),
        [
          "apps/pylon/src/accounts.ts",
          "clients/khala-cli/src/accounts.ts",
          "docs/promises/registry.md",
        ],
      ),
    ).toEqual(["apps/pylon/src/accounts.ts", "clients/khala-cli/src/accounts.ts"])
  })
})

describe("github issue triage output", () => {
  test("detects existing prio labels", () => {
    expect(issueHasPriorityLabel(issue({ labels: [{ name: "prio:2-issue-triage" }] }))).toBe(true)
    expect(issueHasPriorityLabel(issue({ labels: [{ name: "bug" }] }))).toBe(false)
  })

  test("detects any existing labels for the default unlabeled queue", () => {
    expect(issueHasAnyLabel(issue({ labels: [] }))).toBe(false)
    expect(issueHasAnyLabel(issue({ labels: [{ name: "standing-task" }] }))).toBe(true)
  })

  test("builds the default candidate query from newly opened unlabeled issues", () => {
    expect(buildCandidateIssueSearch(false)).toBe("is:issue is:open no:label sort:created-desc")
    expect(buildCandidateIssueSearch(true)).toBe("is:issue is:open sort:created-desc")
  })

  test("builds a scoped execution plan and rendered comment", () => {
    const target = issue({
      number: 42,
      title: "task(ops): standing backlog triage + scoping + dedup loop",
      body: "Scan newly opened issues, search the codebase, check duplicates, and apply prio labels.",
    })
    const decision = triageIssue(target, {
      openIssues: [target],
      repositoryFiles: ["scripts/github-issue-triage.ts", "docs/promises/registry.md"],
    })

    expect(decision.label).toBe("prio:2-issue-triage")
    expect(decision.relevantFiles).toContain("scripts/github-issue-triage.ts")
    expect(buildTechnicalPlan(target, decision.label, decision.relevantFiles, [])).toHaveLength(4)
    expect(renderTriageComment(decision)).toContain("Automated triage pass")
    expect(renderTriageComment(decision)).toContain("prio:2-issue-triage")
  })

  test("prints the searched queue when no candidates are found", () => {
    const result = spawnSync(
      process.execPath,
      ["run", "scripts/github-issue-triage.ts", "--repo", "OpenAgentsInc/openagents", "--limit", "1"],
      {
        cwd: import.meta.dir + "/..",
        encoding: "utf8",
        env: {
          ...process.env,
          PATH: `${import.meta.dir}/fixtures/github-issue-triage/bin:${process.env.PATH ?? ""}`,
        },
      },
    )

    expect(result.status).toBe(0)
    expect(result.stdout).toContain(
      "[github-issue-triage] no candidate issues found for OpenAgentsInc/openagents using search: is:issue is:open no:label sort:created-desc",
    )
  })
})

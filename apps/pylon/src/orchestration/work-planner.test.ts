import { describe, expect, test } from "bun:test"
import { Database } from "bun:sqlite"

import { createPylonOrchestrationStore } from "./store.js"
import {
  githubBacklogCandidates,
  planFixtureWork,
  planGithubBacklogWork,
  planIssueListWork,
  planWorkCandidates,
  type GithubBacklogGhRunner,
  type WorkPlannerSkipReason,
} from "./work-planner.js"

const now = new Date("2026-07-01T12:00:00.000Z")
const repo = "OpenAgentsInc/openagents"

describe("typed work planner", () => {
  test("issue_list emits claimable units and typed skips with no silent drops", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.tryClaimWorkUnit({
      claimRef: "claim.issue.101",
      workUnitRef: "github:OpenAgentsInc/openagents:issue:101",
      runRef: "run.t4-1",
      workerAccountRef: "codex-1",
      ttl: 60_000,
      now,
    })

    const result = planIssueListWork(
      {
        kind: "issue_list",
        repo,
        issues: [
          { number: 100, title: "fresh issue" },
          { number: 101, title: "claimed issue" },
          { number: 102, title: "owner gated", labels: ["needs-owner"] },
          { number: 103, title: "excluded label", labels: ["wontfix"] },
          { number: 104, title: "closed issue", state: "closed" },
          { number: 105, title: "has PR already" },
        ],
        pullRequests: [
          { number: 900, title: "Implement #105", state: "open", body: "Closes #105" },
        ],
      },
      { claimRegistry: store, excludedLabels: ["wontfix"], now },
    )

    expect(result).toMatchObject({
      schema: "openagents.khala_code.work_planner.v1",
      source: "issue_list",
      generatedAt: "2026-07-01T12:00:00.000Z",
    })
    expect(result.units).toHaveLength(7)
    expect(result.claimable.map((unit) => unit.workUnitRef)).toEqual([
      "github:OpenAgentsInc/openagents:issue:100",
      "github:OpenAgentsInc/openagents:pr:900",
    ])
    expect(skipReasonsByNumber(result.skipped)).toEqual({
      101: "already_claimed",
      102: "needs_owner",
      103: "label_excluded",
      104: "closed",
      105: "pr_exists",
    })
    expect(result.skipped.every((unit) => unit.skipReason !== undefined)).toBe(true)
  })

  test("fixture adapter produces fixture-only units and never reads the real claim registry", () => {
    const registry = {
      getLiveWorkClaim() {
        throw new Error("fixture planning must not inspect real claims")
      },
    }

    const result = planFixtureWork(
      {
        kind: "fixture",
        units: [
          { ref: "one", title: "First fixture" },
          { ref: "two", title: "Second fixture", labels: ["needs-owner"] },
          { ref: "three", title: "Third fixture", labels: ["skip-me"] },
        ],
      },
      {
        excludedLabels: ["skip-me"],
        needsOwnerLabels: ["needs-owner"],
        // @ts-expect-error fixture source intentionally rejects real claim registries.
        claimRegistry: registry,
        now,
      },
    )

    expect(result.units.map((unit) => unit.workUnitRef)).toEqual([
      "fixture:one",
      "fixture:two",
      "fixture:three",
    ])
    expect(result.claimable.map((unit) => unit.kind)).toEqual(["fixture"])
    expect(skipReasonsByRef(result.skipped)).toEqual({
      "fixture:two": "needs_owner",
      "fixture:three": "label_excluded",
    })
  })

  test("github_backlog lists issues and PRs through the injected gh runner", async () => {
    const called: string[][] = []
    const gh: GithubBacklogGhRunner = async (args) => {
      called.push([...args])
      if (args[0] === "issue") {
        return JSON.stringify([
          { number: 200, title: "Fresh backlog item", state: "OPEN", labels: [{ name: "bug" }], url: "https://github.test/200" },
          { number: 201, title: "Needs owner", state: "OPEN", labels: [{ name: "needs-owner" }] },
          { number: 202, title: "Already has PR", state: "OPEN", labels: [] },
          { number: 203, title: "Closed upstream", state: "CLOSED", labels: [] },
        ])
      }
      return JSON.stringify([
        { number: 901, title: "Fixes #202", state: "OPEN", labels: [], body: "Fixes #202", mergedAt: null },
        { number: 902, title: "Old merged PR", state: "MERGED", labels: [], body: "", mergedAt: "2026-07-01T11:00:00Z" },
        { number: 903, title: "Closed PR", state: "CLOSED", labels: [], body: "" },
      ])
    }

    const result = await planGithubBacklogWork({ kind: "github_backlog", repo }, gh, { now })

    expect(called).toEqual([
      ["issue", "list", "--repo", repo, "--state", "all", "--limit", "1000", "--json", "number,title,state,labels,body,url"],
      ["pr", "list", "--repo", repo, "--state", "all", "--limit", "1000", "--json", "number,title,state,labels,body,url,mergedAt"],
    ])
    expect(result.units).toHaveLength(7)
    expect(result.claimable.map((unit) => unit.workUnitRef)).toEqual([
      "github:OpenAgentsInc/openagents:issue:200",
      "github:OpenAgentsInc/openagents:pr:901",
    ])
    expect(skipReasonsByNumber(result.skipped)).toEqual({
      201: "needs_owner",
      202: "pr_exists",
      203: "closed",
      902: "merged",
      903: "closed",
    })
  })

  test("github_backlog candidate adapter rejects non-array gh JSON", async () => {
    await expect(
      githubBacklogCandidates({ kind: "github_backlog", repo }, async () => JSON.stringify({ items: [] })),
    ).rejects.toThrow(/non-array JSON/)
  })

  test("skip priority is deterministic and includes merged PR siblings", () => {
    const store = createPylonOrchestrationStore(new Database(":memory:"))
    store.tryClaimWorkUnit({
      claimRef: "claim.priority",
      workUnitRef: "github:OpenAgentsInc/openagents:issue:300",
      runRef: "run.t4-1",
      workerAccountRef: "codex-1",
      ttl: 60_000,
      now,
    })

    const result = planWorkCandidates(
      "issue_list",
      [
        {
          workUnitRef: "github:OpenAgentsInc/openagents:issue:300",
          kind: "github_issue",
          source: "issue_list",
          repo,
          number: 300,
          title: "Merged PR wins before claim",
          state: "open",
        },
        {
          workUnitRef: "github:OpenAgentsInc/openagents:pr:930",
          kind: "github_pr",
          source: "issue_list",
          repo,
          number: 930,
          title: "Closes #300",
          body: "Closes #300",
          state: "merged",
        },
      ],
      { claimRegistry: store, now },
    )

    expect(skipReasonsByNumber(result.skipped)).toEqual({
      300: "merged",
      930: "merged",
    })
  })
})

function skipReasonsByNumber(units: readonly { number?: number; skipReason: WorkPlannerSkipReason }[]) {
  return Object.fromEntries(units.map((unit) => [unit.number, unit.skipReason]))
}

function skipReasonsByRef(units: readonly { workUnitRef: string; skipReason: WorkPlannerSkipReason }[]) {
  return Object.fromEntries(units.map((unit) => [unit.workUnitRef, unit.skipReason]))
}

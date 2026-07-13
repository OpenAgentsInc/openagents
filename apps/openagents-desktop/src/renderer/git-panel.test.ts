/**
 * Git/GitHub panel unit tests (EP250 E2–E5, #8712): pure `state -> View`
 * (status header, commit box, disabled-reason popovers) plus the typed intent
 * loop driven headlessly through the real registry with a fake typed bridge
 * (stage toggle, commit SHA receipt, push receipt, gh-unavailable reason,
 * issue create url receipt).
 */
import { describe, expect, test } from "bun:test"
import { resolveIntentRef, type View } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"

import {
  emptyGitPanelState,
  gitPanelIntents,
  gitPanelView,
  makeGitPanelHandlers,
  type GitGithubBridge,
  type GitPanelState,
} from "./git-panel.ts"
import { gitGithubError, type GitGithubResult } from "../git-github-contract.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

type AnyNode = Readonly<Record<string, unknown>>

const collectNodes = (root: unknown): Array<AnyNode> => {
  const found: Array<AnyNode> = []
  const walk = (value: unknown): void => {
    if (Array.isArray(value)) { for (const item of value) walk(item); return }
    if (typeof value !== "object" || value === null) return
    const node = value as AnyNode
    if (typeof node._tag === "string") found.push(node)
    for (const [prop, child] of Object.entries(node)) {
      if (prop === "_tag" || prop === "a11y") continue
      walk(child)
    }
  }
  walk(root)
  return found
}
const nodeByKey = (view: View, key: string): AnyNode | undefined =>
  collectNodes(view).find((node) => node.key === key)

const readyStatus = (over: Partial<GitPanelState["status"] & object> = {}): GitGithubResult => ({
  ok: true,
  op: "status",
  branch: "main",
  upstream: "origin/main",
  detached: false,
  ahead: 0,
  behind: 0,
  staged: [{ path: "a.txt", status: "modified" }],
  unstaged: [{ path: "b.txt", status: "modified" }],
  untracked: [{ path: "c.txt", status: "untracked" }],
  truncated: false,
  repositoryRef: "workspace.repository.test",
  statusRef: "workspace.git-status.test",
  headRef: "a".repeat(40),
  ...over,
})

const readyState = (over: Partial<GitPanelState> = {}): GitPanelState => ({
  ...emptyGitPanelState(),
  phase: "ready",
  status: readyStatus() as GitPanelState["status"],
  ...over,
})

const readyDiff = (): Extract<GitGithubResult, { op: "diff" }> => ({
  ok: true,
  op: "diff",
  repositoryRef: "workspace.repository.test",
  statusRef: "workspace.git-status.test",
  path: "b.txt",
  source: "unstaged",
  causalItemRef: "timeline.item.file-change.1",
  content: "@@ -1 +1 @@\n-old\n+new\n",
  hunks: [{ header: "@@ -1 +1 @@", oldStart: 1, oldLines: 1, newStart: 1, newLines: 1, content: "@@ -1 +1 @@\n-old\n+new\n" }],
  truncated: false,
})

// ---------------------------------------------------------------------------
// View
// ---------------------------------------------------------------------------

describe("git panel view", () => {
  test("status header shows branch, upstream, and a Refresh control", () => {
    const view = gitPanelView(readyState())
    expect((nodeByKey(view, "git-status-branch") as { content?: string }).content).toBe("main")
    expect((nodeByKey(view, "git-status-upstream") as { content?: string }).content).toBe("origin/main")
    expect(nodeByKey(view, "git-refresh")).toBeDefined()
  })

  test("commit box, push button, branch switcher, and issues/PRs section all render", () => {
    const view = gitPanelView(readyState())
    expect(nodeByKey(view, "git-commit-message")).toBeDefined()
    expect(nodeByKey(view, "git-commit")).toBeDefined()
    expect(nodeByKey(view, "git-push")).toBeDefined()
    expect(nodeByKey(view, "git-branches")).toBeDefined()
    expect(nodeByKey(view, "git-issues-prs")).toBeDefined()
  })

  test("changed files render staged and unstaged rows with stage toggles", () => {
    const view = gitPanelView(readyState())
    expect(nodeByKey(view, "git-stage-toggle-s-a.txt")).toBeDefined() // staged Unstage
    expect(nodeByKey(view, "git-stage-toggle-u-b.txt")).toBeDefined() // unstaged Stage
    expect(nodeByKey(view, "git-stage-toggle-u-c.txt")).toBeDefined() // untracked Stage
    expect(nodeByKey(view, "git-review-u-b.txt")).toBeDefined()
    expect(nodeByKey(view, "git-discard-b.txt")).toBeDefined()
    expect(nodeByKey(view, "git-review-u-c.txt")).toBeUndefined()
  })

  test("reviewed typed hunks render with composer attachment and close controls", () => {
    const view = gitPanelView(readyState({ diff: readyDiff() as Extract<GitPanelState["diff"], object> }))
    expect(nodeByKey(view, "git-review-diff-view")?._tag).toBe("DiffView")
    expect(nodeByKey(view, "git-review-attach")).toBeDefined()
    expect(nodeByKey(view, "git-review-close")).toBeDefined()
  })

  test("commit is disabled with a hover reason when nothing is staged", () => {
    const state = readyState({ status: readyStatus({ staged: [] }) as GitPanelState["status"], commitMessage: "msg" })
    const view = gitPanelView(state)
    const tooltip = nodeByKey(view, "git-commit-reason") as { _tag?: string; content?: string }
    expect(tooltip?._tag).toBe("Tooltip")
    expect(tooltip?.content).toBe("Stage changes to commit")
  })

  test("commit is disabled with a hover reason when the message is empty", () => {
    const view = gitPanelView(readyState({ commitMessage: "" }))
    const tooltip = nodeByKey(view, "git-commit-reason") as { _tag?: string; content?: string }
    expect(tooltip?.content).toBe("Enter a commit message")
  })

  test("push is disabled with a hover reason when there is no upstream", () => {
    const state = readyState({ status: readyStatus({ upstream: null }) as GitPanelState["status"] })
    const view = gitPanelView(state)
    const tooltip = nodeByKey(view, "git-push-reason") as { _tag?: string; content?: string }
    expect(tooltip?._tag).toBe("Tooltip")
    expect(tooltip?.content).toBe("This branch has no upstream yet")
  })

  test("gh Create affordances are disabled with the gate reason when gh is unavailable", () => {
    const view = gitPanelView(readyState({ ghAvailable: false, ghReason: "The GitHub CLI (gh) is not installed." }))
    const tooltip = nodeByKey(view, "git-create-issue-reason") as { _tag?: string; content?: string }
    expect(tooltip?._tag).toBe("Tooltip")
    expect(tooltip?.content).toBe("The GitHub CLI (gh) is not installed.")
    // The reason is also stated once, in-flow, beneath the section heading.
    expect((nodeByKey(view, "git-gh-reason") as { content?: string }).content).toBe("The GitHub CLI (gh) is not installed.")
  })

  test("a commit receipt renders its SHA headline once landed", () => {
    const view = gitPanelView(readyState({ receipt: { kind: "commit", headline: "Committed 0000000", detail: "feat: x" } }))
    expect((nodeByKey(view, "git-receipt-headline") as { content?: string }).content).toBe("Committed 0000000")
  })

  test("the unavailable phase explains itself and hides commit/push", () => {
    const view = gitPanelView({ ...emptyGitPanelState(), phase: "unavailable", reason: "Not a Git repository." })
    expect((nodeByKey(view, "git-unavailable") as { content?: string }).content).toBe("Not a Git repository.")
    expect(nodeByKey(view, "git-commit")).toBeUndefined()
  })
})

// ---------------------------------------------------------------------------
// Typed intent loop
// ---------------------------------------------------------------------------

/** A fake typed bridge: routes each op to a scripted decodable result. */
const makeFakeBridge = (
  handlers: Partial<Record<string, (request: Record<string, unknown>) => GitGithubResult>>,
): { bridge: GitGithubBridge; calls: Array<Record<string, unknown>> } => {
  const calls: Array<Record<string, unknown>> = []
  const bridge: GitGithubBridge = {
    run: async (value: unknown) => {
      const request = value as Record<string, unknown>
      calls.push(request)
      const handler = handlers[String(request["op"])]
      return handler ? handler(request) : gitGithubError("status", "operation_failed", "unhandled op")
    },
  }
  return { bridge, calls }
}

const harness = (
  bridge: GitGithubBridge,
  initial: GitPanelState = emptyGitPanelState(),
  onAttach?: (diff: ReturnType<typeof readyDiff>) => Effect.Effect<void, unknown>,
) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make({ git: initial })
    const registry = yield* makeIntentRegistry(gitPanelIntents, makeGitPanelHandlers(state, bridge, onAttach))
    return { state, registry }
  })

const pressIntent = (view: View, key: string) => {
  const node = nodeByKey(view, key) as { onPress: Parameters<typeof resolveIntentRef>[0] }
  return resolveIntentRef(node.onPress, null)
}

describe("git panel intent loop", () => {
  test("refresh loads status + branches through the bridge", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge } = makeFakeBridge({
        status: () => readyStatus(),
        branchList: () => ({ ok: true, op: "branchList", current: "main", branches: [{ name: "main", current: true, upstream: "origin/main" }], truncated: false }),
      })
      const { state, registry } = yield* harness(bridge)
      const view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-refresh"))
      const git = (yield* SubscriptionRef.get(state)).git
      expect(git.phase).toBe("ready")
      expect(git.status?.branch).toBe("main")
      expect(git.branches.map((b) => b.name)).toEqual(["main"])
      expect(git.currentBranch).toBe("main")
    }))
  })

  test("stage toggle stages an unstaged path (and unstages a staged one)", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge, calls } = makeFakeBridge({
        stage: (req) => ({ ok: true, op: "stage", paths: req["paths"] as string[] }),
        unstage: (req) => ({ ok: true, op: "unstage", paths: req["paths"] as string[] }),
        status: () => readyStatus(),
        branchList: () => ({ ok: true, op: "branchList", current: "main", branches: [], truncated: false }),
      })
      const { state, registry } = yield* harness(bridge, readyState())
      const view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      // b.txt is unstaged → toggling it calls stage.
      yield* registry.dispatch(pressIntent(view, "git-stage-toggle-u-b.txt"))
      // a.txt is staged → toggling it calls unstage.
      const view2 = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view2, "git-stage-toggle-s-a.txt"))
      const ops = calls.map((call) => call["op"])
      expect(ops).toContain("stage")
      expect(ops).toContain("unstage")
    }))
  })

  test("commit success clears the input and records a SHA receipt", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge } = makeFakeBridge({
        commit: () => ({ ok: true, op: "commit", sha: "a".repeat(40), shortSha: "aaaaaaa", summary: "feat: x" }),
        status: () => readyStatus(),
        branchList: () => ({ ok: true, op: "branchList", current: "main", branches: [], truncated: false }),
      })
      const { state, registry } = yield* harness(bridge, readyState({ commitMessage: "feat: x" }))
      const view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-commit"))
      const git = (yield* SubscriptionRef.get(state)).git
      expect(git.commitMessage).toBe("")
      expect(git.receipt?.kind).toBe("commit")
      expect(git.receipt?.headline).toBe("Committed aaaaaaa")
    }))
  })

  test("a typed commit failure surfaces the message, not a fake success", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge } = makeFakeBridge({
        commit: () => gitGithubError("commit", "blocked_by_hook", "A pre-commit hook blocked this commit."),
      })
      const { state, registry } = yield* harness(bridge, readyState({ commitMessage: "feat: x" }))
      const view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-commit"))
      const git = (yield* SubscriptionRef.get(state)).git
      expect(git.receipt).toBeNull()
      expect(git.actionError).toBe("A pre-commit hook blocked this commit.")
    }))
  })

  test("push success records the pushed ref receipt", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge } = makeFakeBridge({
        push: () => ({ ok: true, op: "push", ref: "main", remote: "origin", sha: "b".repeat(40) }),
        status: () => readyStatus(),
        branchList: () => ({ ok: true, op: "branchList", current: "main", branches: [], truncated: false }),
      })
      const { state, registry } = yield* harness(bridge, readyState())
      const view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-push"))
      const git = (yield* SubscriptionRef.get(state)).git
      expect(git.receipt?.kind).toBe("push")
      expect(git.receipt?.headline).toBe("Pushed main")
    }))
  })

  test("a gh-unavailable issue load marks gh unavailable with a reason", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge } = makeFakeBridge({
        issueList: () => gitGithubError("issueList", "gh_unavailable", "The GitHub CLI (gh) is not installed."),
      })
      const { state, registry } = yield* harness(bridge, readyState())
      const view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-load-issues"))
      const git = (yield* SubscriptionRef.get(state)).git
      expect(git.ghAvailable).toBe(false)
      expect(git.ghReason).toBe("The GitHub CLI (gh) is not installed.")
    }))
  })

  test("create issue returns a number+url receipt", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge } = makeFakeBridge({
        issueCreate: () => ({ ok: true, op: "issueCreate", number: 8712, url: "https://github.com/o/r/issues/8712" }),
      })
      const { state, registry } = yield* harness(bridge, readyState({ create: "issue", createTitle: "A bug" }))
      const view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-create-submit"))
      const git = (yield* SubscriptionRef.get(state)).git
      expect(git.receipt?.kind).toBe("issue")
      expect(git.receipt?.headline).toBe("Created issue #8712")
      expect(git.receipt?.detail).toBe("https://github.com/o/r/issues/8712")
      expect(git.create).toBe("none")
    }))
  })

  test("branch checkout refreshes on success", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const checkedOut: string[] = []
      const { bridge } = makeFakeBridge({
        checkout: (req) => { checkedOut.push(req["name"] as string); return { ok: true, op: "checkout", name: req["name"] as string } },
        status: () => readyStatus({ branch: "feature/x" }),
        branchList: () => ({ ok: true, op: "branchList", current: "feature/x", branches: [], truncated: false }),
      })
      const branches = [{ name: "feature/x", current: false, upstream: null }, { name: "main", current: true, upstream: "origin/main" }]
      const { state, registry } = yield* harness(bridge, readyState({ branches, currentBranch: "main" }))
      const view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-branch-feature/x"))
      expect(checkedOut).toEqual(["feature/x"])
      expect((yield* SubscriptionRef.get(state)).git.currentBranch).toBe("feature/x")
    }))
  })

  test("review and composer attachment use the exact repository/status snapshot", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const attached: Array<ReturnType<typeof readyDiff>> = []
      const { bridge, calls } = makeFakeBridge({ diff: () => readyDiff() })
      const { state, registry } = yield* harness(
        bridge,
        readyState(),
        diff => Effect.sync(() => { attached.push(diff as ReturnType<typeof readyDiff>) }),
      )
      let view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-review-u-b.txt"))
      expect(calls[0]).toEqual({
        op: "diff",
        repositoryRef: "workspace.repository.test",
        statusRef: "workspace.git-status.test",
        path: "b.txt",
        source: "unstaged",
        causalItemRef: null,
      })
      view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-review-attach"))
      expect(attached).toHaveLength(1)
      expect(attached[0]?.path).toBe("b.txt")
    }))
  })

  test("review preserves the exact causal timeline item in the request and view", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge, calls } = makeFakeBridge({ diff: () => readyDiff() })
      const { state, registry } = yield* harness(
        bridge,
        readyState({ causalItemRef: "timeline.item.file-change.1" }),
      )
      let view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-review-u-b.txt"))
      expect(calls[0]).toMatchObject({
        op: "diff",
        causalItemRef: "timeline.item.file-change.1",
      })
      view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      expect((nodeByKey(view, "git-review-causal-item") as { content?: string }).content)
        .toBe("Timeline timeline.item.file-change.1")
    }))
  })

  test("discard requires inline confirmation and sends the exact fenced request", async () => {
    await Effect.runPromise(Effect.gen(function* () {
      const { bridge, calls } = makeFakeBridge({
        discard: () => ({ ok: true, op: "discard", repositoryRef: "workspace.repository.test", path: "b.txt", statusRef: "workspace.git-status.next" }),
        status: () => readyStatus({ unstaged: [] }),
        branchList: () => ({ ok: true, op: "branchList", current: "main", branches: [], truncated: false }),
      })
      const { state, registry } = yield* harness(bridge, readyState())
      let view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      yield* registry.dispatch(pressIntent(view, "git-discard-b.txt"))
      view = gitPanelView((yield* SubscriptionRef.get(state)).git)
      expect(nodeByKey(view, "git-discard-confirmation")).toBeDefined()
      yield* registry.dispatch(pressIntent(view, "git-discard-confirm"))
      expect(calls[0]).toEqual({
        op: "discard",
        repositoryRef: "workspace.repository.test",
        statusRef: "workspace.git-status.test",
        path: "b.txt",
      })
      expect((yield* SubscriptionRef.get(state)).git.discardConfirmPath).toBeNull()
    }))
  })
})

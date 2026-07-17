import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import { decodeCodingComposerDraftSnapshot, emptyComposerState } from "@openagentsinc/khala-sync-client"

import type { MobileCodingComposerSession } from "../src/coding/mobile-coding-composer"
import {
  decodeMobileGitMutationResult,
  decodeMobileGitStatus,
  type MobileGitMutationRequest,
  type MobileGitStatus,
} from "../src/coding/mobile-repository-git"
import { buildHomeProgram, renderContentView } from "../src/screens/home-core"

const scope = { sessionRef: "session.git", repositoryRef: "repository.git", worktreeRef: "worktree.git" }
const status = (input: Partial<MobileGitStatus> = {}): MobileGitStatus => ({
  ...scope,
  statusRef: "status.git.1",
  headRef: "commit.git.1",
  branch: "feature/mobile",
  detached: false,
  upstream: "origin/feature/mobile",
  ahead: 0,
  behind: 0,
  defaultBranch: false,
  files: [{ pathRef: "src/app.ts", status: "modified", staged: false }],
  branches: [
    { branchRef: "branch.feature", name: "feature/mobile", current: true, upstream: "origin/feature/mobile" },
    { branchRef: "branch.main", name: "main", current: false, upstream: "origin/main" },
  ],
  truncated: false,
  ...input,
})

const composer = (): MobileCodingComposerSession => {
  const state = emptyComposerState()
  return {
    repositoryLabel: "openagents",
    worktreeLabel: "git",
    targetLabel: "Codex",
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1",
      draftRef: "draft.git",
      ownerRef: "owner.git",
      sessionRef: scope.sessionRef,
      threadRef: "thread.git",
      revision: 1,
      doc: state.doc,
      selection: state.selection,
      view: state.view,
      context: [{ kind: "repository", repositoryRef: scope.repositoryRef, revisionRef: "revision.repository" }, {
        kind: "worktree", repositoryRef: scope.repositoryRef, worktreeRef: scope.worktreeRef, revisionRef: "revision.worktree",
      }],
      target: { laneRef: "lane.codex", providerRef: "provider.codex", modelRef: "model.codex", accountRef: "account.codex", executionTargetRef: "codex:git", readiness: "ready" },
      submission: { status: "editing" },
      updatedAt: "2026-07-17T23:20:00.000Z",
    }),
  }
}

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})
const lastState = (program: ReturnType<typeof buildHomeProgram>) => Effect.map(Stream.runHead(program.stateChanges), option => {
  if (option._tag !== "Some") throw new Error("expected state")
  return option.value
})

const receipt = (request: MobileGitMutationRequest, next: MobileGitStatus) => ({
  ...scope,
  ok: true,
  op: request.op,
  requestStatusRef: request.statusRef,
  receiptRef: `receipt.git.${request.op}`,
  recordedAt: "2026-07-17T23:20:01.000Z",
  branch: next.branch,
  commitRef: request.op === "checkout" ? null : next.headRef,
  remote: request.op === "push" ? "origin" : null,
  summary: request.op === "checkout" ? `Checked out ${next.branch}` : request.op === "commit" ? "Ship mobile Git" : `Pushed ${next.branch}`,
  status: next,
})

describe("T3M-E1 exact-worktree Git", () => {
  test("decodes bounded status and typed mutation outcomes only at the exact status fence", () => {
    const current = status()
    expect(decodeMobileGitStatus(current, scope)?.branches).toHaveLength(2)
    expect(decodeMobileGitStatus({ ...current, worktreeRef: "foreign" }, scope)).toBeNull()
    expect(decodeMobileGitStatus({ ...current, branches: [...current.branches, { ...current.branches[0] }] }, scope)).toBeNull()
    const request: MobileGitMutationRequest = { ...scope, op: "push", statusRef: current.statusRef, expectedHeadRef: current.headRef,
      branchName: "feature/mobile", idempotencyRef: "git.mobile.push.1", confirmationRef: "confirmation.mobile.push.1" }
    expect(decodeMobileGitMutationResult({ ...scope, ok: false, op: "push", requestStatusRef: current.statusRef,
      code: "non_fast_forward", message: "Remote moved." }, request)).toMatchObject({ code: "non_fast_forward" })
    expect(decodeMobileGitMutationResult({ ...receipt(request, status({ statusRef: "status.git.2", ahead: 0 })), requestStatusRef: "stale" }, request)).toBeNull()
  })

  test("commits, pushes, and switches branch through confirmations and exact receipts without moving transcript state", async () => {
    const active = composer()
    const requests: Array<MobileGitMutationRequest | typeof scope> = []
    const program = buildHomeProgram({ coding: {
      directory: { authority: "confirmed", phase: "live", cacheState: "current", offlineCache: { accounting: "live_confirmed", ownerScopeRef: "scope.owner", cachedRepositoryCount: 1, cachedSessionCount: 1, lastConfirmedCursor: 1 }, repositories: [], sessions: [] },
      activeComposer: () => active,
      clearSelection: async () => undefined,
      selectSession: async () => null,
      updateComposerText: async session => session,
      pickComposerAttachments: async () => ({ status: "cancelled" }),
      repositoryGit: {
        gitStatus: async request => { requests.push(request); return status() },
        gitMutate: async request => {
          requests.push(request)
          if (request.op === "commit") return receipt(request, status({
            statusRef: "status.git.2", headRef: "commit.git.2", ahead: 1, files: [],
          }))
          if (request.op === "push") return receipt(request, status({
            statusRef: "status.git.3", headRef: "commit.git.2", ahead: 0, files: [],
          }))
          return receipt(request, status({
            statusRef: "status.git.4", headRef: "commit.main.2", branch: "main", upstream: "origin/main", defaultBranch: true,
            branches: [
              { branchRef: "branch.feature", name: "feature/mobile", current: false, upstream: "origin/feature/mobile" },
              { branchRef: "branch.main", name: "main", current: true, upstream: "origin/main" },
            ],
            files: [],
          }))
        },
      },
    } })
    const transcript = program.initialState.khala
    program.coding.openGit()
    await Effect.runPromise(settle)
    program.coding.changeGitCommitMessage("Ship mobile Git")
    program.coding.requestGitCommit()
    program.coding.acceptGitConfirmation()
    await Effect.runPromise(settle)
    program.coding.requestGitPush()
    program.coding.acceptGitConfirmation()
    await Effect.runPromise(settle)
    program.coding.selectGitBranch("branch.main", "main")
    program.coding.acceptGitConfirmation()
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(state.repositoryGit.receipts.map(item => item.op)).toEqual(["commit", "push", "checkout"])
    expect(state.repositoryGit.status?.branch).toBe("main")
    expect(state.khala).toEqual(transcript)
    expect(JSON.stringify(renderContentView(state))).toContain("Branch selected")
    expect(requests).toHaveLength(4)
    expect(requests.slice(1).every(request => "confirmationRef" in request && request.confirmationRef.startsWith("confirmation.mobile."))).toBe(true)
  })

  test("renders a typed non-fast-forward failure without inventing a push receipt", async () => {
    const active = composer()
    const current = status({ ahead: 1, files: [] })
    const program = buildHomeProgram({ coding: {
      directory: { authority: "confirmed", phase: "live", cacheState: "current", offlineCache: { accounting: "live_confirmed", ownerScopeRef: "scope.owner", cachedRepositoryCount: 1, cachedSessionCount: 1, lastConfirmedCursor: 1 }, repositories: [], sessions: [] },
      activeComposer: () => active,
      clearSelection: async () => undefined,
      selectSession: async () => null,
      updateComposerText: async session => session,
      pickComposerAttachments: async () => ({ status: "cancelled" }),
      repositoryGit: {
        gitStatus: async () => current,
        gitMutate: async request => ({ ...scope, ok: false, op: request.op, requestStatusRef: request.statusRef,
          code: "non_fast_forward", message: "Remote moved." }),
      },
    } })
    program.coding.openGit()
    await Effect.runPromise(settle)
    program.coding.requestGitPush()
    await Effect.runPromise(settle)
    program.coding.acceptGitConfirmation()
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    expect(state.repositoryGit.failureCode).toBe("non_fast_forward")
    expect(state.repositoryGit.receipts).toEqual([])
    expect(JSON.stringify(renderContentView(state))).toContain("remote moved")
  })
})

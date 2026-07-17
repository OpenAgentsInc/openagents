import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import { decodeCodingComposerDraftSnapshot, emptyComposerState } from "@openagentsinc/khala-sync-client"

import {
  decodeMobileRepositoryPreview,
  decodeMobileRepositoryTreePage,
  loadMobileRepositoryPreview,
  loadMobileRepositoryTree,
} from "../src/coding/mobile-repository-files"
import type { MobileCodingComposerSession } from "../src/coding/mobile-coding-composer"
import { buildHomeProgram, renderContentView } from "../src/screens/home-core"

const scope = {
  sessionRef: "session.mobile.files",
  repositoryRef: "repository.mobile.files",
  worktreeRef: "worktree.mobile.files",
}
const treeRequest = { ...scope, directoryRef: "", cursor: null, limit: 100 }
const treePage = {
  ...scope,
  directoryRef: "",
  revisionRef: "revision.tree.root",
  nextCursor: null,
  entries: [{
    name: "README.md",
    pathRef: "README.md",
    kind: "file",
    expandable: false,
    sizeBytes: 7,
    revisionRef: "revision.readme",
  }, {
    name: "src",
    pathRef: "src",
    kind: "directory",
    expandable: true,
    sizeBytes: null,
    revisionRef: "revision.src",
  }],
}

const composer = (): MobileCodingComposerSession => {
  const state = emptyComposerState()
  return {
    repositoryLabel: "openagents",
    worktreeLabel: "main",
    targetLabel: "Codex",
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1",
      draftRef: "draft.mobile.files",
      ownerRef: "owner.mobile.files",
      sessionRef: scope.sessionRef,
      threadRef: "thread.mobile.files",
      revision: 1,
      doc: state.doc,
      selection: state.selection,
      view: state.view,
      context: [{ kind: "repository", repositoryRef: scope.repositoryRef, revisionRef: "revision.repository" }, {
        kind: "worktree",
        repositoryRef: scope.repositoryRef,
        worktreeRef: scope.worktreeRef,
        revisionRef: "revision.worktree",
      }],
      target: {
        laneRef: "lane.codex",
        providerRef: "provider.openai.codex",
        modelRef: "model.gpt-5.6-sol",
        accountRef: "account.mobile.files",
        executionTargetRef: "codex:mobile-files",
        readiness: "ready",
      },
      submission: { status: "editing" },
      updatedAt: "2026-07-17T22:45:00.000Z",
    }),
  }
}

const settle = Effect.gen(function* () {
  yield* Effect.promise<void>(() => new Promise(resolve => setTimeout(resolve, 0)))
  yield* Effect.yieldNow
})

const lastState = (program: ReturnType<typeof buildHomeProgram>) =>
  Effect.map(Stream.runHead(program.stateChanges), option => {
    if (option._tag !== "Some") throw new Error("expected state")
    return option.value
  })

describe("contract openagents_mobile.repository_files.v1", () => {
  test("decodes exact scoped, direct-child, bounded tree pages", () => {
    expect(decodeMobileRepositoryTreePage(treePage, treeRequest)?.entries).toHaveLength(2)
    expect(decodeMobileRepositoryTreePage({ ...treePage, worktreeRef: "worktree.foreign" }, treeRequest)).toBeNull()
    expect(decodeMobileRepositoryTreePage({
      ...treePage,
      entries: [{ ...treePage.entries[0], pathRef: "../README.md" }],
    }, treeRequest)).toBeNull()
    expect(decodeMobileRepositoryTreePage({
      ...treePage,
      entries: [{ ...treePage.entries[0], pathRef: "src/nested.ts" }],
    }, treeRequest)).toBeNull()
    expect(decodeMobileRepositoryTreePage({
      ...treePage,
      entries: [treePage.entries[0], treePage.entries[0]],
    }, treeRequest)).toBeNull()
  })

  test("decodes exact UTF-8 source and Markdown while refusing stale/oversized content", () => {
    const request = { ...scope, pathRef: "README.md", expectedRevisionRef: "revision.readme" }
    const content = "# Hello"
    const response = {
      ...scope,
      pathRef: request.pathRef,
      revisionRef: request.expectedRevisionRef,
      sizeBytes: new TextEncoder().encode(content).byteLength,
      kind: "markdown",
      content,
    }
    expect(decodeMobileRepositoryPreview(response, request)).toMatchObject({ kind: "markdown", content })
    expect(decodeMobileRepositoryPreview({ ...response, revisionRef: "revision.stale" }, request)).toBeNull()
    expect(decodeMobileRepositoryPreview({ ...response, sizeBytes: 1 }, request)).toBeNull()
    expect(decodeMobileRepositoryPreview({ ...response, content: "bad\0text" }, request)).toBeNull()
  })

  test("accepts only bounded HTTPS images with supported media and digest identity", () => {
    const request = { ...scope, pathRef: "assets/icon.png", expectedRevisionRef: "revision.icon" }
    const response = {
      ...scope,
      pathRef: request.pathRef,
      revisionRef: request.expectedRevisionRef,
      sizeBytes: 2048,
      kind: "image",
      mediaType: "image/png",
      contentUrl: "https://openagents.com/api/mobile/files/content/receipt.fixture",
      sha256: "a".repeat(64),
    }
    expect(decodeMobileRepositoryPreview(response, request)).toMatchObject({ kind: "image" })
    expect(decodeMobileRepositoryPreview({ ...response, contentUrl: "http://openagents.com/icon.png" }, request)).toBeNull()
    expect(decodeMobileRepositoryPreview({ ...response, mediaType: "image/svg+xml" }, request)).toBeNull()
    expect(decodeMobileRepositoryPreview({ ...response, sha256: "short" }, request)).toBeNull()
  })

  test("ports surface invalid and unavailable results without manufacturing entries", async () => {
    expect(await loadMobileRepositoryTree({
      tree: async () => treePage,
      read: async () => null,
    }, { ...scope, directoryRef: "", cursor: null })).toMatchObject({ state: "ready" })
    expect(await loadMobileRepositoryTree({
      tree: async () => ({ ...treePage, repositoryRef: "foreign" }),
      read: async () => null,
    }, { ...scope, directoryRef: "", cursor: null })).toMatchObject({ state: "failed" })
    expect(await loadMobileRepositoryPreview({
      tree: async () => treePage,
      read: async () => { throw new Error("offline") },
    }, { ...scope, pathRef: "README.md", expectedRevisionRef: "revision.readme" }))
      .toMatchObject({ state: "failed", message: "That file preview is unavailable right now." })
  })

  test("opens exact worktree files, previews content, and returns without disturbing transcript state", async () => {
    const active = composer()
    const calls: unknown[] = []
    const program = buildHomeProgram({ coding: {
      directory: {
        authority: "confirmed",
        phase: "live",
        cacheState: "current",
        offlineCache: { accounting: "live_confirmed", ownerScopeRef: "scope.owner", cachedRepositoryCount: 1, cachedSessionCount: 1, lastConfirmedCursor: 1 },
        repositories: [],
        sessions: [],
      },
      activeComposer: () => active,
      clearSelection: async () => undefined,
      selectSession: async () => null,
      updateComposerText: async session => session,
      pickComposerAttachments: async () => ({ status: "cancelled" }),
      repositoryFiles: {
        tree: async request => { calls.push(request); return treePage },
        read: async request => {
          calls.push(request)
          const content = "# Hello"
          return {
            ...scope,
            pathRef: "README.md",
            revisionRef: "revision.readme",
            sizeBytes: new TextEncoder().encode(content).byteLength,
            kind: "markdown",
            content,
          }
        },
      },
    } })
    const transcriptBefore = program.initialState.khala
    program.coding.openFiles()
    await Effect.runPromise(settle)
    let state = await Effect.runPromise(lastState(program))
    expect(state.workbenchRoute).toBe("files")
    expect(state.repositoryBrowser.state).toBe("ready")
    expect(calls[0]).toEqual({ ...scope, directoryRef: "", cursor: null, limit: 100 })
    expect(JSON.stringify(renderContentView(state))).toContain("README.md")

    program.coding.selectFile("README.md", "revision.readme")
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.repositoryBrowser.preview).toMatchObject({ state: "ready", preview: { kind: "markdown" } })
    expect(JSON.stringify(renderContentView(state))).toContain("Hello")

    program.coding.closeFiles()
    await Effect.runPromise(settle)
    state = await Effect.runPromise(lastState(program))
    expect(state.workbenchRoute).toBe("conversation")
    expect(state.khala).toEqual(transcriptBefore)
  })
})

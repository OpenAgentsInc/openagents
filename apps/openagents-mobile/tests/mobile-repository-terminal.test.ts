import { describe, expect, test } from "vite-plus/test"
import { Effect, Stream } from "@effect-native/core/effect"
import { decodeCodingComposerDraftSnapshot, emptyComposerState } from "@openagentsinc/khala-sync-client"

import type { MobileCodingComposerSession } from "../src/coding/mobile-coding-composer"
import {
  applyMobileTerminalReplay,
  decodeMobileTerminalCommandReceipt,
  decodeMobileTerminalReplay,
  decodeMobileTerminalSnapshot,
  type MobileTerminalSession,
} from "../src/coding/mobile-repository-terminal"
import { mobileTerminalHostDriver } from "../src/effect-native/mobile-terminal-host-driver"
import { buildHomeProgram, renderContentView } from "../src/screens/home-core"

const scope = { sessionRef: "session.terminal", repositoryRef: "repository.terminal", worktreeRef: "worktree.terminal" }
const terminal = (input: Partial<MobileTerminalSession> = {}): MobileTerminalSession => ({
  terminalRef: "terminal.mobile.1",
  sessionVersionRef: "terminal.version.1",
  label: "Shell 1",
  shellLabel: "zsh",
  status: "running",
  exitCode: null,
  cols: 80,
  rows: 24,
  lastSeq: 1,
  gap: false,
  recovered: true,
  tail: "$ ",
  ...input,
})
const snapshot = (sessions: ReadonlyArray<MobileTerminalSession>, ref = "terminal.snapshot.1") => ({
  ...scope, snapshotRef: ref, sessions, truncated: false,
})

const composer = (): MobileCodingComposerSession => {
  const state = emptyComposerState()
  return {
    repositoryLabel: "openagents", worktreeLabel: "terminal", targetLabel: "Codex",
    draft: decodeCodingComposerDraftSnapshot({
      schema: "openagents.coding_composer_draft.v1", draftRef: "draft.terminal", ownerRef: "owner.terminal",
      sessionRef: scope.sessionRef, threadRef: "thread.terminal", revision: 1, doc: state.doc, selection: state.selection,
      view: state.view, context: [{ kind: "repository", repositoryRef: scope.repositoryRef, revisionRef: "revision.repository" },
        { kind: "worktree", repositoryRef: scope.repositoryRef, worktreeRef: scope.worktreeRef, revisionRef: "revision.worktree" }],
      target: { laneRef: "lane.codex", providerRef: "provider.codex", modelRef: "model.codex", accountRef: "account.codex", executionTargetRef: "codex:terminal", readiness: "ready" },
      submission: { status: "editing" }, updatedAt: "2026-07-17T23:30:00.000Z",
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

describe("T3M-E2 mobile terminal", () => {
  test("decodes exact contiguous replay and accounts for gaps without inventing output", () => {
    const session = terminal()
    expect(decodeMobileTerminalSnapshot(snapshot([session]), scope)?.sessions[0]?.recovered).toBe(true)
    expect(decodeMobileTerminalSnapshot({ ...snapshot([session]), worktreeRef: "foreign" }, scope)).toBeNull()
    const request = { ...scope, terminalRef: session.terminalRef, sessionVersionRef: session.sessionVersionRef, afterSeq: 1, limit: 500 }
    const page = decodeMobileTerminalReplay({ ...request, toSeq: 2, gap: true, truncated: false,
      events: [{ seq: 2, kind: "output", data: "pwd\r\n/work\n", exitCode: null }] }, request)
    if (page === null) throw new Error("expected replay")
    expect(applyMobileTerminalReplay(session, page).tail).toContain("Earlier terminal output unavailable")
    expect(decodeMobileTerminalReplay({ ...page, events: [{ ...page.events[0], seq: 3 }] }, request)).toBeNull()
    const command = { ...scope, terminalRef: session.terminalRef, sessionVersionRef: session.sessionVersionRef,
      op: "input" as const, data: "pwd\r", idempotencyRef: "terminal.mobile.input.1" }
    expect(decodeMobileTerminalCommandReceipt({ ...scope, terminalRef: session.terminalRef, requestVersionRef: session.sessionVersionRef,
      op: "input", receiptRef: "receipt.terminal.input.1", sessionVersionRef: "terminal.version.2", status: "running",
      recordedAt: "2026-07-17T23:30:01.000Z" }, command)?.receiptRef).toBe("receipt.terminal.input.1")
  })

  test("native terminal driver emits bounded input and negotiated geometry", () => {
    const events: unknown[] = []
    const createElement = (type: unknown, props: Record<string, unknown> | null = null, ...children: unknown[]) => ({
      type, props: { ...(props ?? {}), ...(children.length === 0 ? {} : { children }) },
    })
    const native = { View: "View", Text: "Text", TextInput: "TextInput", Pressable: "Pressable", ScrollView: "ScrollView" }
    const instance = mobileTerminalHostDriver.mount({ output: "$ ", autoFit: true }, {
      dependencies: { React: { createElement }, ReactNative: native as never },
      report: () => Effect.void,
      emit: event => events.push(event),
    })
    const root = instance.render({ output: "$ ", autoFit: true }) as { props: Record<string, unknown> }
    ;(root.props.onLayout as (event: unknown) => void)({ nativeEvent: { layout: { width: 720, height: 320 } } })
    const find = (node: unknown, testID: string): { props: Record<string, unknown> } | null => {
      if (typeof node !== "object" || node === null) return null
      const row = node as { props?: Record<string, unknown> }
      if (row.props?.testID === testID) return row as { props: Record<string, unknown> }
      const children = row.props?.children
      for (const child of Array.isArray(children) ? children : [children]) {
        const found = find(child, testID)
        if (found !== null) return found
      }
      return null
    }
    const input = find(root, "oa-mobile-terminal-input")
    if (input === null) throw new Error("expected terminal input")
    ;(input.props.onSubmitEditing as (event: unknown) => void)({ nativeEvent: { text: "pwd" } })
    expect(events).toEqual([
      { type: "resize", cols: 100, rows: 20 },
      { type: "data", data: "pwd\r" },
    ])
  })

  test("recovers, creates, replays, and resizes exact worktree sessions without moving transcript state", async () => {
    const active = composer()
    const shell1 = terminal()
    const shell2 = terminal({ terminalRef: "terminal.mobile.2", sessionVersionRef: "terminal.version.10", label: "Shell 2", recovered: false, lastSeq: 0, tail: "" })
    const requests: unknown[] = []
    const program = buildHomeProgram({ coding: {
      directory: { authority: "confirmed", phase: "live", cacheState: "current", offlineCache: { accounting: "live_confirmed", ownerScopeRef: "scope.owner", cachedRepositoryCount: 1, cachedSessionCount: 1, lastConfirmedCursor: 1 }, repositories: [], sessions: [] },
      activeComposer: () => active, clearSelection: async () => undefined, selectSession: async () => null,
      updateComposerText: async session => session, pickComposerAttachments: async () => ({ status: "cancelled" }),
      repositoryTerminal: {
        terminalSnapshot: async request => { requests.push(request); return snapshot([shell1]) },
        terminalCreate: async request => { requests.push(request); return snapshot([shell1, shell2], "terminal.snapshot.2") },
        terminalReplay: async request => {
          requests.push(request)
          return request.sessionVersionRef === "terminal.version.2"
            ? { ...scope, terminalRef: shell1.terminalRef, sessionVersionRef: "terminal.version.2", afterSeq: 1, toSeq: 2, gap: false, truncated: false,
                events: [{ seq: 2, kind: "output", data: "pwd\r\n/work\n", exitCode: null }] }
            : { ...scope, terminalRef: request.terminalRef, sessionVersionRef: request.sessionVersionRef, afterSeq: request.afterSeq,
                toSeq: request.afterSeq, gap: false, truncated: false, events: [] }
        },
        terminalCommand: async request => {
          requests.push(request)
          return { ...scope, terminalRef: request.terminalRef, requestVersionRef: request.sessionVersionRef, op: request.op,
            receiptRef: `receipt.terminal.${request.op}`, sessionVersionRef: request.op === "input" ? "terminal.version.2" : "terminal.version.3",
            status: "running", recordedAt: "2026-07-17T23:30:01.000Z" }
        },
      },
    } })
    const transcript = program.initialState.khala
    program.coding.openTerminal()
    await Effect.runPromise(settle)
    program.coding.createTerminal()
    await Effect.runPromise(settle)
    program.coding.selectTerminal(shell1.terminalRef)
    await Effect.runPromise(settle)
    program.coding.sendTerminalData("pwd\r")
    await Effect.runPromise(settle)
    program.coding.resizeTerminal(100, 30)
    await Effect.runPromise(settle)
    const state = await Effect.runPromise(lastState(program))
    const recovered = state.repositoryTerminal.sessions.find(session => session.terminalRef === shell1.terminalRef)
    expect(state.repositoryTerminal.sessions).toHaveLength(2)
    expect(recovered?.tail).toContain("/work")
    expect([recovered?.cols, recovered?.rows]).toEqual([100, 30])
    expect(state.repositoryTerminal.lastReceipt?.op).toBe("resize")
    expect(state.khala).toEqual(transcript)
    expect(JSON.stringify(renderContentView(state))).toContain('"kind":"terminal"')
    expect(requests).toHaveLength(6)
  })
})

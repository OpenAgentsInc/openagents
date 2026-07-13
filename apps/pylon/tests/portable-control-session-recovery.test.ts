import { Database } from "bun:sqlite"
import { afterEach, describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"

import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap.js"
import { PYLON_DEV_CHECK_SCHEMA } from "../src/dev-loop.js"
import {
  createControlSessionActions,
  type ControlSessionExecutor,
} from "../src/node/control-sessions.js"
import { PylonPortableSessionOperationLedger } from "../src/portable-session-operation-ledger.js"

const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true })))
})

const passed = () => ({
  schema: PYLON_DEV_CHECK_SCHEMA,
  observedAt: "2026-07-13T00:00:00.000Z",
  action: "check" as const,
  state: "passed" as const,
  changeSummary: {
    repo: { state: "clean" as const, rootRef: "repo.test", branch: "main", commit: "a".repeat(40) },
    dirty: { state: "clean" as const, changedCount: 0, stagedCount: 0, unstagedCount: 0, untrackedCount: 0 },
    changedFileRefs: [], areaRefs: [], blockerRefs: [],
  },
  checkPlan: { state: "ready" as const, commandRefs: [], blockerRefs: [] },
  commandResults: [],
  latestRecordRef: null,
  branchUntouched: true,
  commitUntouched: true,
  pushPerformed: false,
  blockerRefs: [],
})

describe("durable portable control-session restart recovery", () => {
  test("reconstructs the exact root/child binding as non-accepting and fences the stale process epoch", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-portable-control-restart-"))
    roots.push(root)
    const workspace = join(root, "workspace")
    await mkdir(workspace, { recursive: true })
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), {
      PYLON_HOME: join(root, "pylon"),
    })
    const path = join(root, "portable.sqlite")
    const firstDatabase = new Database(path, { create: true })
    const firstLedger = new PylonPortableSessionOperationLedger(firstDatabase)
    const executor: ControlSessionExecutor = async input => {
      await new Promise<never>((_resolve, reject) => {
        if (input.abortSignal.aborted) return reject(new Error("settled"))
        input.abortSignal.addEventListener("abort", () => reject(new Error("settled")), { once: true })
      })
      return {
        commandCount: 0,
        devCheck: passed(),
        editedFileCount: 0,
        eventCount: 0,
        externalSessionRef: null,
        responseDigestRef: null,
        totalTokens: 0,
      }
    }
    const first = createControlSessionActions({
      env: {},
      executor,
      portableLedger: firstLedger,
      portableRuntimeInstanceRef: "runtime.pylon.port03.before_restart",
      summary,
    })
    const rootRun = await first.spawn({
      type: "session.spawn",
      adapter: "codex",
      worktreePath: workspace,
      objective: "root work",
      verify: ["true"],
    })
    const childRun = await first.reply({
      type: "session.reply",
      sessionRef: rootRun.sessionRef,
      objective: "child work",
    })
    const sessionRef = "session.port03.restart.binding"
    const attachmentRef = "attachment.port03.restart.binding.1"
    await Effect.runPromise(firstLedger.registerSession({
      sessionRef,
      attachmentRef,
      generation: 1,
      acceptingWork: true,
    }))
    first.portable.bind({
      sessionRef,
      attachmentRef,
      generation: 1,
      agents: [
        { agentRef: "agent.port03.restart.root", controlSessionRef: rootRun.sessionRef },
        { agentRef: "agent.port03.restart.child", controlSessionRef: childRun.sessionRef },
      ],
    })

    const before = await Effect.runPromise(firstLedger.readControlBinding(sessionRef))
    expect(before).toMatchObject({
      runtimeInstanceRef: "runtime.pylon.port03.before_restart",
      state: "accepting",
      agents: [
        { agentRef: "agent.port03.restart.child", parentAgentRef: "agent.port03.restart.root", processLifecycle: "active" },
        { agentRef: "agent.port03.restart.root", processLifecycle: "active" },
      ],
    })
    expect(JSON.stringify(before)).not.toContain(root)

    // A second SQLite handle models the fresh Pylon process. Recovery changes
    // the shared durable epoch before any new process can accept graph work.
    const restartedDatabase = new Database(path)
    const restartedLedger = new PylonPortableSessionOperationLedger(restartedDatabase)
    const restarted = createControlSessionActions({
      env: {},
      executor,
      portableLedger: restartedLedger,
      portableRuntimeInstanceRef: "runtime.pylon.port03.after_restart",
      summary,
    })
    const recoveryInput = {
      recoveryRef: "recovery.port03.restart.binding.1",
      sessionRef,
      attachmentRef,
      generation: 1,
    }
    const recovery = await restarted.portable.recover(recoveryInput)
    expect(recovery).toEqual({
      schema: "openagents.pylon.portable_operation_ledger.v1",
      recoveryRef: recoveryInput.recoveryRef,
      sessionRef,
      attachmentRef,
      generation: 1,
      runtimeInstanceRef: "runtime.pylon.port03.after_restart",
      outcome: "recovered_quiesced",
      acceptingWork: false,
      agentRefs: ["agent.port03.restart.child", "agent.port03.restart.root"],
      controlSessionRefs: [childRun.sessionRef, rootRun.sessionRef],
      workspaceRefs: [before.agents[0]!.workspaceRef],
    })
    expect(await restarted.portable.recover(recoveryInput)).toEqual(recovery)
    expect(await Effect.runPromise(restartedLedger.readControlBinding(sessionRef))).toMatchObject({
      runtimeInstanceRef: "runtime.pylon.port03.after_restart",
      state: "quiesced",
      revision: 1,
      agents: [
        { processLifecycle: "absent_after_restart", workspaceLifecycle: "retained" },
        { processLifecycle: "absent_after_restart", workspaceLifecycle: "retained" },
      ],
    })

    // The deliberately still-live pre-restart manager has an accepting
    // in-memory map, but its old runtime epoch loses at the SQLite fence.
    await expect(first.reply({
      type: "session.reply",
      sessionRef: rootRun.sessionRef,
      objective: "stale work after restart",
    })).rejects.toThrow("not accepting work")
    await expect(restarted.portable.recover({
      ...recoveryInput,
      attachmentRef: "attachment.port03.restart.binding.conflict",
    })).rejects.toMatchObject({ reason: "conflicting_replay" })

    await first.cancel(rootRun.sessionRef)
    await first.cancel(childRun.sessionRef)
    restartedDatabase.close()
    firstDatabase.close()
  })
})

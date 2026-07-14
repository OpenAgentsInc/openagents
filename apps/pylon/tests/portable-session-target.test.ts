import { Runtime } from "@openagentsinc/runtime-platform"
import { setTimeout as sleep } from "node:timers/promises"
import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import { afterEach, describe, expect, test } from "vite-plus/test"
import { access, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
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
import { createPylonOwnerLocalExecutionTarget } from "../src/portable-session-target.js"

const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true })))
})

const git = async (cwd: string, ...args: string[]) => {
  const proc = Runtime.spawn(["git", ...args], { cwd, stdout: "pipe", stderr: "pipe" })
  if (await proc.exited !== 0) throw new Error(await new Response(proc.stderr).text())
}

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

describe("owner-local Pylon portable execution target", () => {
  test("quiesces and joins a real root/child control graph, persists an exact checkpoint, then cleans once", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-portable-target-"))
    roots.push(root)
    const home = join(root, "pylon")
    const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
    let invocation = 0
    const executor: ControlSessionExecutor = async input => {
      invocation += 1
      await writeFile(join(input.cwd, `agent-${invocation}.txt`), `agent ${invocation}\n`, "utf8")
      await new Promise<never>((_resolve, reject) => {
        if (input.abortSignal.aborted) return reject(new Error("portable quiesce"))
        input.abortSignal.addEventListener("abort", () => reject(new Error("portable quiesce")), { once: true })
      })
      return {
        commandCount: 0, devCheck: passed(), editedFileCount: 0, eventCount: 0,
        externalSessionRef: null, responseDigestRef: null, totalTokens: 0,
      }
    }
    const actions = createControlSessionActions({
      env: {},
      executor,
      summary,
      workspaceCheckoutRunner: async workingDirectory => {
        await mkdir(workingDirectory, { recursive: true })
        await git(workingDirectory, "init", "-b", "main")
        await git(workingDirectory, "config", "user.email", "test@openagents.com")
        await git(workingDirectory, "config", "user.name", "OpenAgents Test")
        await writeFile(join(workingDirectory, "README.md"), "base\n", "utf8")
        await git(workingDirectory, "add", "README.md")
        await git(workingDirectory, "commit", "-m", "base")
      },
    })
    const rootRun = await actions.spawn({
      type: "session.spawn",
      adapter: "codex",
      repoRef: {
        branch: "main",
        commitSha: "a".repeat(40),
        fullName: "OpenAgentsInc/openagents",
        provider: "github",
        visibility: "public",
      },
      objective: "Run root",
      verify: ["true"],
    })
    const childRun = await actions.reply({
      type: "session.reply",
      sessionRef: rootRun.sessionRef,
      objective: "Run child",
    })
    while (invocation < 2) await sleep(5)

    const databasePath = join(root, "portable.sqlite")
    const database = new NodeTestDatabase(databasePath, { create: true })
    const ledger = new PylonPortableSessionOperationLedger(database)
    const sessionRef = "session.portable.owner.1"
    const attachmentRef = "attachment.portable.owner.1"
    const graph = {
      rootAgentRef: "agent.portable.root",
      nodes: [
        {
          agentRef: "agent.portable.root", threadRef: "thread.portable.root",
          transcriptRef: "transcript.portable.root", activityCursor: 4,
          lifecycle: "quiesced" as const, attachmentGeneration: 1,
        },
        {
          agentRef: "agent.portable.child", parentAgentRef: "agent.portable.root",
          threadRef: "thread.portable.child", transcriptRef: "transcript.portable.child",
          activityCursor: 2, lifecycle: "quiesced" as const, attachmentGeneration: 1,
        },
      ],
    }
    const agentRefs = graph.nodes.map(node => node.agentRef)
    const binding = {
      sessionRef,
      attachmentRef,
      generation: 1,
      agents: [
        { agentRef: agentRefs[0]!, controlSessionRef: rootRun.sessionRef },
        { agentRef: agentRefs[1]!, controlSessionRef: childRun.sessionRef },
      ],
    }
    const target = await createPylonOwnerLocalExecutionTarget({
      targetRef: "target.pylon.owner.local",
      ledger,
      lifecycle: actions.portable,
      binding,
    })
    const quiesced = await target.quiesceGraph({
      operationRef: "operation.portable.owner.quiesce",
      sessionRef,
      attachmentRef,
      generation: 1,
      graph,
      threadCursors: [],
    })
    expect(quiesced.quiescedAgentRefs.sort()).toEqual([...agentRefs].sort())
    expect((await actions.list()).filter(row =>
      row.sessionRef === rootRun.sessionRef || row.sessionRef === childRun.sessionRef
    ).every(row => row.state === "cancelled")).toBe(true)
    await expect(actions.reply({
      type: "session.reply", sessionRef: rootRun.sessionRef, objective: "stale work",
    })).rejects.toThrow("not accepting work")

    const checkpointInput = {
      operationRef: "operation.portable.owner.checkpoint",
      checkpointRef: "checkpoint.portable.owner.1",
      sessionRef,
      attachmentRef,
      generation: 1,
      eventLogCursor: 9,
      executionBinding: {
        schema: "openagents.portable_session_execution_binding.v1" as const,
        sessionRef,
        ownerRef: "owner.portable.1",
        runRef: "run.portable.1",
        repositoryRef: "repository.openagents.main",
        pinnedBaseRef: `commit.${"a".repeat(40)}`,
      },
      graph,
      threadCursors: [
        { threadRef: "thread.portable.root", transcriptRef: "transcript.portable.root", activityCursor: 4, eventCursor: 7 },
        { threadRef: "thread.portable.child", transcriptRef: "transcript.portable.child", activityCursor: 2, eventCursor: 5 },
      ],
    }
    const bundle = await target.createCheckpoint(checkpointInput)
    expect(bundle.checkpoint).toMatchObject({
      checkpointRef: checkpointInput.checkpointRef,
      repositoryRef: checkpointInput.executionBinding.repositoryRef,
      eventLogCursor: 9,
      secretMaterial: "excluded",
      processState: "excluded",
    })
    expect(bundle.checkpoint.digest).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(bundle.checkpoint.repositoryPostImageDigest).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(bundle.checkpoint.diffDigest).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(bundle.checkpoint.graphDigest).toMatch(/^sha256:[a-f0-9]{64}$/)
    expect(JSON.stringify(bundle)).not.toContain(root)

    const reopenedDb = new NodeTestDatabase(databasePath)
    const reopenedLedger = new PylonPortableSessionOperationLedger(reopenedDb)
    expect(await Effect.runPromise(reopenedLedger.readCheckpointBundle(checkpointInput.operationRef)))
      .toEqual(bundle)
    const restartedTarget = await createPylonOwnerLocalExecutionTarget({
      targetRef: "target.pylon.owner.local",
      ledger: reopenedLedger,
      lifecycle: actions.portable,
      binding,
    })
    expect(await restartedTarget.createCheckpoint(checkpointInput)).toEqual(bundle)
    reopenedDb.close()

    const workspace = (await actions.list()).find(row => row.sessionRef === rootRun.sessionRef)!
    const cleanup = await target.cleanupSource({
      operationRef: "operation.portable.owner.source.cleanup",
      sessionRef,
      attachmentRef,
      generation: 1,
      agentRefs,
    })
    expect(cleanup).toMatchObject({ processes: "released", scratch: "released", ports: "released" })
    expect(cleanup.cleanedAgentRefs.sort()).toEqual([...agentRefs].sort())
    expect((await target.cleanupSource({
      operationRef: "operation.portable.owner.source.cleanup",
      sessionRef,
      attachmentRef,
      generation: 1,
      agentRefs,
    })).evidenceRefs).toEqual(cleanup.evidenceRefs)
    await expect(access(join(summary.paths.cache, "control-session-worktrees", workspace.workspaceRef)))
      .rejects.toThrow()
    database.close()
  })
})

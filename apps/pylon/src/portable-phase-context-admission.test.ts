import { mkdtemp, rm, stat } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { openLegacySqliteDatabase } from "@openagentsinc/sqlite-runtime"
import { Effect, Schema } from "effect"
import { describe, expect, test } from "vite-plus/test"

import { makePylonNodeRuntime } from "./node/runtime.js"
import { startControlServer } from "./node/control-server.js"
import type { PylonOwnerLocalExecutionTarget } from "./portable-session-target.js"
import {
  PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA,
  PylonPortablePhasePrivatePayloadSchema,
  PylonPortablePhaseContextAdmissionError,
  PylonPortablePhaseContextAdmissionStore,
  makeDurablePylonPortablePhaseTargetResolver,
  openPylonPortablePhaseContextAdmissionStore,
  type PylonPortablePhaseContextAdmissionInput,
  type PylonPortablePhasePrivatePayload,
} from "./portable-phase-context-admission.js"

const refs = {
  operationRef: "operation.ide13.admission.quiesce",
  commandRef: "command.ide13.admission",
  commandExecutionClaimRef: "claim.ide13.admission.command",
  ownerRef: "owner.ide13.admission",
  sessionRef: "session.ide13.admission",
  attachmentRef: "attachment.ide13.admission",
  targetRef: "target.ide13.admission",
  pylonRef: "pylon.ide13.admission",
}

const quiesceAdmission = (): PylonPortablePhaseContextAdmissionInput => ({
  schema: PYLON_PORTABLE_PHASE_CONTEXT_ADMISSION_SCHEMA,
  request: {
    schema: "openagents.portable_phase_operation.v1",
    ...refs,
    attachmentGeneration: 1,
    kind: "quiesce",
    checkpointRef: null,
    checkpointObjectRef: null,
    checkpointDigest: null,
    evidenceRefs: [],
    expiresAt: "2096-07-20T12:10:00.000Z",
  },
  payload: {
    kind: "quiesce",
    input: {
      operationRef: refs.operationRef,
      sessionRef: refs.sessionRef,
      attachmentRef: refs.attachmentRef,
      generation: 1,
      graph: {
        rootAgentRef: "agent.ide13.admission",
        nodes: [{
          agentRef: "agent.ide13.admission",
          threadRef: "thread.ide13.admission",
          transcriptRef: "transcript.ide13.admission",
          activityCursor: 0,
          lifecycle: "running",
          attachmentGeneration: 1,
        }],
      },
      threadCursors: [],
    },
  },
})

const target = (): PylonOwnerLocalExecutionTarget => ({
  targetRef: refs.targetRef,
  targetClass: "owner_local",
  quiesceGraph: async () => ({ quiescedAgentRefs: [], evidenceRefs: [] }),
  createCheckpoint: async () => { throw new Error("not used") },
  cleanupSource: async () => { throw new Error("not used") },
  stageCheckpoint: async () => { throw new Error("not used") },
  activate: async () => { throw new Error("not used") },
  abortStaged: async () => { throw new Error("not used") },
})

const payloads = (): ReadonlyArray<PylonPortablePhasePrivatePayload> => {
  const source = {
    operationRef: refs.operationRef,
    sessionRef: refs.sessionRef,
    attachmentRef: refs.attachmentRef,
    generation: 1,
  }
  const destination = {
    operationRef: refs.operationRef,
    sessionRef: refs.sessionRef,
    destinationAttachmentRef: refs.attachmentRef,
    destinationGeneration: 1,
  }
  const executionBinding = {
    schema: "openagents.portable_session_execution_binding.v1" as const,
    sessionRef: refs.sessionRef,
    ownerRef: refs.ownerRef,
    runRef: "run.ide13.admission",
    repositoryRef: "repository.ide13.admission",
    pinnedBaseRef: "revision.ide13.admission",
  }
  const graph = {
    rootAgentRef: "agent.ide13.admission",
    nodes: [{
      agentRef: "agent.ide13.admission",
      threadRef: "thread.ide13.admission",
      transcriptRef: "transcript.ide13.admission",
      activityCursor: 0,
      lifecycle: "running" as const,
      attachmentGeneration: 1,
    }],
  }
  return [
    { kind: "quiesce", input: { ...source, graph, threadCursors: [] } },
    {
      kind: "checkpoint-create",
      checkpointObjectRef: "object.ide13.admission",
      input: {
        ...source,
        checkpointRef: "checkpoint.ide13.admission",
        eventLogCursor: 0,
        executionBinding,
        graph,
        threadCursors: [],
      },
    },
    { kind: "source-cleanup", input: { ...source, agentRefs: ["agent.ide13.admission"] } },
    {
      kind: "checkpoint-stage",
      input: {
        operationRef: refs.operationRef,
        bundle: {
          checkpoint: {
            schema: "openagents.portable_checkpoint.v1",
            checkpointRef: "checkpoint.ide13.admission",
            sessionRef: refs.sessionRef,
            sourceAttachmentRef: refs.attachmentRef,
            sourceGeneration: 1,
            digest: `sha256:${"1".repeat(64)}`,
            repositoryRef: executionBinding.repositoryRef,
            repositoryRevisionRef: executionBinding.pinnedBaseRef,
            repositoryPostImageDigest: `sha256:${"2".repeat(64)}`,
            diffDigest: `sha256:${"3".repeat(64)}`,
            eventLogCursor: 0,
            catalogGenerationRef: "catalog.ide13.admission",
            graphDigest: `sha256:${"4".repeat(64)}`,
            approvalRefs: [],
            artifactRefs: [],
            receiptRefs: [],
            secretMaterial: "excluded",
            processState: "excluded",
          },
          executionBinding,
          graph,
          threadCursors: [],
        },
        destinationAttachmentRef: refs.attachmentRef,
        destinationGeneration: 1,
        capabilityLeaseRefs: [],
      },
    },
    {
      kind: "destination-activate",
      input: {
        ...destination,
        checkpointRef: "checkpoint.ide13.admission",
        executionBinding,
        capabilityLeaseRefs: [],
      },
    },
    { kind: "staged-abort", input: destination },
  ]
}

describe("Pylon private portable phase context admission", () => {
  test("defines exact private payloads for all six phases", () => {
    const decoded = payloads().map(payload =>
      Schema.decodeUnknownSync(PylonPortablePhasePrivatePayloadSchema)(payload, {
        onExcessProperty: "error",
      }),
    )
    expect(decoded.map(payload => payload.kind)).toEqual([
      "quiesce",
      "checkpoint-create",
      "source-cleanup",
      "checkpoint-stage",
      "destination-activate",
      "staged-abort",
    ])
  })

  test("persists before resolution, replays exact bytes, and defaults recovery to not proven", async () => {
    const root = await mkdtemp(join(tmpdir(), "pylon-phase-admission-"))
    try {
      const opened = await openPylonPortablePhaseContextAdmissionStore({
        databasePath: join(root, "private", "contexts.sqlite"),
        now: () => new Date("2026-07-20T12:00:00.000Z"),
      })
      const admission = quiesceAdmission()
      const first = opened.store.admit(admission)
      const replay = opened.store.admit(admission)
      expect(replay).toEqual(first)
      expect(first.recoverySemantics).toBe("not_proven")
      expect((await stat(join(root, "private"))).mode & 0o077).toBe(0)
      expect((await stat(join(root, "private", "contexts.sqlite"))).mode & 0o077).toBe(0)

      const executionTarget = target()
      const resolver = makeDurablePylonPortablePhaseTargetResolver({
        store: opened.store,
        target: targetRef => targetRef === executionTarget.targetRef ? executionTarget : undefined,
      })
      const resolved = await resolver.resolve(admission.request)
      expect(resolved?.call.kind).toBe("quiesce")
      expect(resolved?.operationRefSemantics).toBe("not_proven")
      expect(await resolver.resolve({ ...admission.request, attachmentGeneration: 2 })).toBeUndefined()
      opened.close()

      const reopened = await openPylonPortablePhaseContextAdmissionStore({
        databasePath: join(root, "private", "contexts.sqlite"),
        now: () => new Date("2026-07-20T12:00:01.000Z"),
      })
      expect(reopened.store.resolve(admission.request.operationRef)?.record).toEqual(first)
      reopened.store.acknowledgeTerminal(admission.request.operationRef)
      expect(reopened.store.resolve(admission.request.operationRef)).toBeUndefined()
      expect(reopened.store.purge()).toBe(1)
      reopened.close()
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })

  test("rejects a conflicting replay and corrupt durable bytes", () => {
    const database = openLegacySqliteDatabase(":memory:")
    try {
      const store = new PylonPortablePhaseContextAdmissionStore(database)
      const admission = quiesceAdmission()
      store.admit(admission)
      expect(() => store.admit({
        ...admission,
        payload: {
          ...admission.payload,
          input: { ...admission.payload.input, threadCursors: [{
            threadRef: "thread.ide13.admission",
            transcriptRef: "transcript.ide13.admission",
            activityCursor: 1,
            eventCursor: 1,
          }] },
        },
      })).toThrow(new PylonPortablePhaseContextAdmissionError("conflicting_replay"))
      database.query("UPDATE pylon_portable_phase_context_admissions SET context_digest = ?").run(
        `sha256:${"f".repeat(64)}`,
      )
      expect(() => store.resolve(admission.request.operationRef)).toThrow(
        new PylonPortablePhaseContextAdmissionError("corrupt_admission"),
      )
    } finally {
      database.close()
    }
  })

  test("admits only through the bearer-authenticated loopback control action", async () => {
    const admitted: PylonPortablePhaseContextAdmissionInput[] = []
    await Effect.runPromise(Effect.scoped(Effect.gen(function* () {
      const runtime = yield* makePylonNodeRuntime
      const server = yield* startControlServer(runtime, {
        token: "test-token-0123456789abcdef",
        actions: {
          walletSend: async () => null,
          walletReceive: async () => null,
          walletAdmitPayoutTarget: async () => null,
          portablePhaseContextAdmit: async input => {
            admitted.push(input)
            return { state: "admitted" }
          },
        },
        port: 0,
      })
      const unauthorized = yield* Effect.promise(() => fetch(`${server.url}/command`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ type: "portable_phase.context.admit", admission: quiesceAdmission() }),
      }))
      expect(unauthorized.status).toBe(401)
      const response = yield* Effect.promise(() => fetch(`${server.url}/command`, {
        method: "POST",
        headers: {
          authorization: "Bearer test-token-0123456789abcdef",
          "content-type": "application/json",
        },
        body: JSON.stringify({ type: "portable_phase.context.admit", admission: quiesceAdmission() }),
      }))
      expect(response.status).toBe(200)
      expect(admitted).toHaveLength(1)
    })))
  })
})

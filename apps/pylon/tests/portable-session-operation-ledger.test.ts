import { NodeTestDatabase } from "@openagentsinc/sqlite-runtime/test"
import { afterEach, describe, expect, test } from "vite-plus/test"
import { mkdtemp, rm } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { Effect } from "effect"

import {
  PylonPortableOperationLedgerError,
  PylonPortableSessionOperationLedger,
} from "../src/portable-session-operation-ledger.js"

const roots: string[] = []

afterEach(async () => {
  await Promise.all(roots.splice(0).map(root => rm(root, { recursive: true, force: true })))
})

const fileLedger = async (): Promise<Readonly<{
  path: string
  database: NodeTestDatabase
  ledger: PylonPortableSessionOperationLedger
}>> => {
  const root = await mkdtemp(join(tmpdir(), "pylon-portable-ledger-"))
  roots.push(root)
  const path = join(root, "portable.sqlite")
  const database = new NodeTestDatabase(path, { create: true })
  return { path, database, ledger: new PylonPortableSessionOperationLedger(database) }
}

const register = (ledger: PylonPortableSessionOperationLedger, suffix: string) =>
  Effect.runPromise(ledger.registerSession({
    sessionRef: `session.port03.pylon.${suffix}`,
    attachmentRef: `attachment.port03.pylon.${suffix}.1`,
    generation: 1,
    acceptingWork: true,
  }))

describe("owner-local Pylon portable operation ledger", () => {
  test("persists quiescence, exact operation replay, and one-generation activation across reopen", async () => {
    const first = await fileLedger()
    const initial = await register(first.ledger, "move")
    expect(initial).toMatchObject({ generation: 1, acceptingWork: true, revision: 0 })

    const quiesceInput = {
      operationRef: "operation.port03.pylon.move.quiesce",
      sessionRef: initial.sessionRef,
      attachmentRef: initial.attachmentRef,
      generation: 1,
      evidenceRefs: ["evidence.port03.pylon.root.quiesced", "evidence.port03.pylon.child.quiesced"],
    }
    const quiesced = await Effect.runPromise(first.ledger.quiesceGeneration(quiesceInput))
    expect(quiesced).toMatchObject({
      status: "completed",
      fence: { acceptingWork: false, generation: 1, revision: 1 },
      record: { state: "completed", kind: "quiesce" },
    })
    first.database.close()

    const secondDatabase = new NodeTestDatabase(first.path)
    const second = new PylonPortableSessionOperationLedger(secondDatabase)
    expect(await Effect.runPromise(second.quiesceGeneration(quiesceInput))).toMatchObject({
      status: "replayed",
      fence: { acceptingWork: false, generation: 1, revision: 1 },
    })

    const activated = await Effect.runPromise(second.activateGeneration({
      operationRef: "operation.port03.pylon.move.activate",
      sessionRef: initial.sessionRef,
      sourceAttachmentRef: initial.attachmentRef,
      sourceGeneration: 1,
      destinationAttachmentRef: "attachment.port03.pylon.move.2",
      destinationGeneration: 2,
      authorityEvidenceRef: "evidence.port03.authority.generation.2.committed",
    }))
    expect(activated).toMatchObject({
      status: "completed",
      fence: {
        attachmentRef: "attachment.port03.pylon.move.2",
        generation: 2,
        acceptingWork: true,
        revision: 2,
      },
    })
    secondDatabase.close()

    const thirdDatabase = new NodeTestDatabase(first.path)
    const third = new PylonPortableSessionOperationLedger(thirdDatabase)
    expect(await Effect.runPromise(third.readSession(initial.sessionRef))).toMatchObject({
      attachmentRef: "attachment.port03.pylon.move.2",
      generation: 2,
      acceptingWork: true,
      revision: 2,
    })
    expect(await Effect.runPromise(third.activateGeneration({
      operationRef: "operation.port03.pylon.move.activate",
      sessionRef: initial.sessionRef,
      sourceAttachmentRef: initial.attachmentRef,
      sourceGeneration: 1,
      destinationAttachmentRef: "attachment.port03.pylon.move.2",
      destinationGeneration: 2,
      authorityEvidenceRef: "evidence.port03.authority.generation.2.committed",
    }))).toMatchObject({ status: "replayed", fence: { generation: 2, acceptingWork: true } })
    await expect(Effect.runPromise(third.activateGeneration({
      operationRef: "operation.port03.pylon.move.activate",
      sessionRef: initial.sessionRef,
      sourceAttachmentRef: initial.attachmentRef,
      sourceGeneration: 1,
      destinationAttachmentRef: "attachment.port03.pylon.move.conflict",
      destinationGeneration: 2,
      authorityEvidenceRef: "evidence.port03.authority.generation.2.committed",
    }))).rejects.toMatchObject({ reason: "conflicting_replay" })
    thirdDatabase.close()
  })

  test("refuses stale generations and conflicting operation bytes before mutation", async () => {
    const { database, ledger } = await fileLedger()
    const initial = await register(ledger, "stale")
    const first = await Effect.runPromise(ledger.admitOperation({
      operationRef: "operation.port03.pylon.stale.checkpoint",
      sessionRef: initial.sessionRef,
      attachmentRef: initial.attachmentRef,
      generation: 1,
      kind: "checkpoint",
    }))
    expect(first.status).toBe("admitted")
    expect((await Effect.runPromise(ledger.admitOperation({
      operationRef: "operation.port03.pylon.stale.checkpoint",
      sessionRef: initial.sessionRef,
      attachmentRef: initial.attachmentRef,
      generation: 1,
      kind: "checkpoint",
    }))).status).toBe("replayed")

    await expect(Effect.runPromise(ledger.admitOperation({
      operationRef: "operation.port03.pylon.stale.checkpoint",
      sessionRef: initial.sessionRef,
      attachmentRef: initial.attachmentRef,
      generation: 1,
      kind: "cleanup",
    }))).rejects.toMatchObject({ reason: "conflicting_replay" })
    await expect(Effect.runPromise(ledger.admitOperation({
      operationRef: "operation.port03.pylon.stale.future",
      sessionRef: initial.sessionRef,
      attachmentRef: initial.attachmentRef,
      generation: 2,
      kind: "checkpoint",
    }))).rejects.toMatchObject({ reason: "stale_generation" })
    expect(await Effect.runPromise(ledger.readSession(initial.sessionRef))).toMatchObject({
      generation: 1,
      acceptingWork: true,
      revision: 0,
    })
    database.close()
  })

  test("retains an admitted operation over restart and accepts one refs-only terminal outcome", async () => {
    const first = await fileLedger()
    const initial = await register(first.ledger, "checkpoint")
    const operation = {
      operationRef: "operation.port03.pylon.checkpoint.create",
      sessionRef: initial.sessionRef,
      attachmentRef: initial.attachmentRef,
      generation: 1,
      kind: "checkpoint" as const,
    }
    await Effect.runPromise(first.ledger.admitOperation(operation))
    first.database.close()

    const reopenedDatabase = new NodeTestDatabase(first.path)
    const reopened = new PylonPortableSessionOperationLedger(reopenedDatabase)
    expect(await Effect.runPromise(reopened.admitOperation(operation))).toMatchObject({
      status: "replayed",
      record: { state: "admitted" },
    })
    const outcome = {
      evidenceRefs: ["evidence.port03.pylon.checkpoint.created"],
      checkpointRef: "checkpoint.port03.pylon.checkpoint.1",
      repositoryPostImageDigest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
      diffDigest: "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
      graphDigest: "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
    }
    expect(await Effect.runPromise(reopened.completeOperation({ operationRef: operation.operationRef, outcome }))).toMatchObject({
      status: "completed",
      record: { state: "completed", outcome },
    })
    expect((await Effect.runPromise(reopened.completeOperation({ operationRef: operation.operationRef, outcome }))).status).toBe("replayed")

    await expect(Effect.runPromise(reopened.completeOperation({
      operationRef: operation.operationRef,
      outcome: { ...outcome, evidenceRefs: ["evidence.port03.pylon.checkpoint.changed"] },
    }))).rejects.toBeInstanceOf(PylonPortableOperationLedgerError)
    reopenedDatabase.close()
  })

  test("rejects path- and secret-shaped outcome material without completing the operation", async () => {
    const { database, ledger } = await fileLedger()
    const initial = await register(ledger, "private")
    const operationRef = "operation.port03.pylon.private.cleanup"
    await Effect.runPromise(ledger.admitOperation({
      operationRef,
      sessionRef: initial.sessionRef,
      attachmentRef: initial.attachmentRef,
      generation: 1,
      kind: "cleanup",
    }))
    await expect(Effect.runPromise(ledger.completeOperation({
      operationRef,
      outcome: {
        evidenceRefs: ["evidence.port03.pylon.cleanup"],
        cleanupReceiptRef: "/Users/operator/private/cleanup.json",
      },
    }))).rejects.toMatchObject({ reason: "unsafe_result" })
    expect(await Effect.runPromise(ledger.admitOperation({
      operationRef,
      sessionRef: initial.sessionRef,
      attachmentRef: initial.attachmentRef,
      generation: 1,
      kind: "cleanup",
    }))).toMatchObject({ status: "replayed", record: { state: "admitted" } })
    database.close()
  })
})

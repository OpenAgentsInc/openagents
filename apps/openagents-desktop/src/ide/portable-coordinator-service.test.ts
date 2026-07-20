import { Deferred, Effect, Fiber, Layer } from "effect"
import { describe, expect, test } from "vite-plus/test"

import {
  IdePortableCheckpointManifestSchema,
  IdePortableCoordinatorCommandSchema,
  IdePortableCoordinatorSnapshotSchema,
} from "@openagentsinc/portable-session-contract"

import {
  IdePortableCoordinator,
  IdePortableCoordinatorError,
  makeIdePortableCoordinatorLayer,
  type IdePortableCoordinatorAdapter,
} from "./portable-coordinator-service.ts"

const digest = `sha256:${"a".repeat(64)}`
const project = {
  projectRef: "project.alpha",
  projectRootRef: "root.alpha",
  worktreeRef: "worktree.alpha",
  selectedFileRef: null,
  documentSnapshotRef: null,
  proposalRef: null,
  diagnosticResultRef: null,
  testResultRef: null,
  artifactRef: null,
  evidenceRef: null,
} as const

const seed = IdePortableCoordinatorSnapshotSchema.make({
  sessionRef: "session.alpha",
  project,
  phase: "attached",
  activePlacementRef: "placement.local",
  activeAttachmentRef: "attachment.local.1",
  activeGeneration: 1,
  pendingCommandRef: null,
  pendingDestinationPlacementRef: null,
  checkpointManifestRef: null,
  eventSequence: 1,
  stopped: false,
})

const manifest = IdePortableCheckpointManifestSchema.make({
  manifestRef: "manifest.alpha.1",
  checkpointRef: "checkpoint.alpha.1",
  sessionRef: seed.sessionRef,
  sourceAttachmentRef: seed.activeAttachmentRef,
  sourceGeneration: seed.activeGeneration,
  digest,
  byteSize: 1_024,
  fileCount: 2,
  repositoryPostImageDigest: digest,
  graphDigest: digest,
  project,
  includedCapabilityRefs: ["capability.files"],
  omittedCapabilityRefs: ["capability.pty"],
  historyRefs: [],
  proposalRefs: [],
  taskRefs: [],
  testRefs: [],
  deliveryEvidenceRefs: [],
  secretMaterial: "excluded",
  processState: "excluded",
  nativeState: "excluded",
  vimState: "destination_setting",
  themeState: "destination_setting",
  policy: {
    maximumBytes: 2_048,
    maximumFiles: 10,
    encryption: "owner_key",
    encryptionKeyRef: "key.alpha",
    custody: "owner_managed",
    retentionSeconds: 3_600,
    expiresAt: "2030-01-01T00:00:00.000Z",
  },
  integrityReceiptRef: "integrity.alpha.1",
})

const move = IdePortableCoordinatorCommandSchema.cases.Move.make({
  commandRef: "command.move.1",
  idempotencyKey: "idempotency.move.1",
  actorRef: "actor.owner",
  policyRef: "policy.portable",
  sessionRef: seed.sessionRef,
  project,
  expectedAttachmentRef: seed.activeAttachmentRef,
  expectedGeneration: seed.activeGeneration,
  deadlineAt: "2030-01-01T00:00:00.000Z",
  approvalRef: "approval.move.1",
  destinationPlacementRef: "placement.remote",
})

const adapter = (
  calls: string[],
  quiesce: IdePortableCoordinatorAdapter["quiesceAndCheckpoint"] = () => Effect.succeed(manifest),
  validate: IdePortableCoordinatorAdapter["validateCheckpoint"] = () => Effect.void,
): IdePortableCoordinatorAdapter => ({
  quiesceAndCheckpoint: (snapshot, command) => Effect.sync(() => calls.push("quiesce")).pipe(
    Effect.andThen(quiesce(snapshot, command)),
  ),
  validateCheckpoint: (value, placement, stage) => Effect.sync(() => {
    calls.push(`validate:${stage}`)
  }).pipe(Effect.andThen(validate(value, placement, stage))),
  stageDestination: () => Effect.sync(() => {
    calls.push("stage")
    return { attachmentRef: "attachment.remote.2" }
  }),
  revokeSource: () => Effect.sync(() => {
    calls.push("revoke")
  }),
  attachDestination: () => Effect.sync(() => {
    calls.push("attach")
  }),
  restartFreshHelpers: () => Effect.sync(() => {
    calls.push("restart_helpers")
  }),
  rollbackDestination: () => Effect.sync(() => {
    calls.push("rollback")
  }),
  resumeSource: () => Effect.sync(() => {
    calls.push("resume_source")
  }),
  stop: () => Effect.sync(() => {
    calls.push("stop")
  }),
})

const run = <A>(effect: Effect.Effect<A, IdePortableCoordinatorError, IdePortableCoordinator>, layer: Layer.Layer<IdePortableCoordinator>) =>
  Effect.runPromise(effect.pipe(Effect.provide(layer)))

describe("IDE portable coordinator", () => {
  test("moves in the exclusive order, advances generation, fences stale writers, and replays idempotently", async () => {
    const calls: string[] = []
    const layer = makeIdePortableCoordinatorLayer(seed, adapter(calls), {
      now: () => "2029-01-01T00:00:00.000Z",
      nextReceiptRef: () => "receipt.move.1",
    })
    const result = await run(Effect.gen(function* () {
      const service = yield* IdePortableCoordinator
      const first = yield* service.execute(move)
      const replay = yield* service.execute(move)
      const stale = yield* service.authorizeMutation({
        sessionRef: seed.sessionRef,
        attachmentRef: seed.activeAttachmentRef,
        generation: 1,
      }).pipe(Effect.exit)
      yield* service.authorizeMutation({
        sessionRef: seed.sessionRef,
        attachmentRef: "attachment.remote.2",
        generation: 2,
      })
      return { first, replay, stale }
    }), layer)
    expect(calls).toEqual([
      "quiesce", "validate:source", "stage", "validate:destination",
      "revoke", "attach", "restart_helpers",
    ])
    expect(result.first.snapshot.activeGeneration).toBe(2)
    expect(result.first.snapshot.activeAttachmentRef).toBe("attachment.remote.2")
    expect(result.first.receipt?.sourceGeneration).toBe(1)
    expect(result.first.receipt?.destinationGeneration).toBe(2)
    expect(result.replay.receipt?.receiptRef).toBe(result.first.receipt?.receiptRef)
    expect(calls).toHaveLength(7)
    expect(result.stale._tag).toBe("Failure")
  })

  test("cancels before source revocation, tears down the staged destination, and resumes the source", async () => {
    const calls: string[] = []
    const result = await Effect.runPromise(Effect.gen(function* () {
      const entered = yield* Deferred.make<void>()
      const release = yield* Deferred.make<void>()
      const gate: IdePortableCoordinatorAdapter["validateCheckpoint"] = (_manifest, _placement, stage) =>
        stage === "destination"
          ? Deferred.succeed(entered, undefined).pipe(Effect.andThen(Deferred.await(release)))
          : Effect.void
      const layer = makeIdePortableCoordinatorLayer(seed, adapter(calls, () => Effect.succeed(manifest), gate), {
        now: () => "2029-01-01T00:00:00.000Z",
      })
      return yield* Effect.gen(function* () {
        const service = yield* IdePortableCoordinator
        const fiber = yield* Effect.forkChild(service.execute(move))
        yield* Deferred.await(entered)
        const cancel = IdePortableCoordinatorCommandSchema.cases.Cancel.make({
          commandRef: "command.cancel.1",
          idempotencyKey: "idempotency.cancel.1",
          actorRef: "actor.owner",
          policyRef: "policy.portable",
          sessionRef: seed.sessionRef,
          project,
          expectedAttachmentRef: seed.activeAttachmentRef,
          expectedGeneration: 1,
          deadlineAt: "2030-01-01T00:00:00.000Z",
          approvalRef: "approval.cancel.1",
          targetCommandRef: move.commandRef,
        })
        yield* service.execute(cancel)
        yield* Deferred.succeed(release, undefined)
        const exit = yield* Fiber.await(fiber)
        const snapshot = yield* service.snapshot()
        return { exit, snapshot }
      }).pipe(Effect.provide(layer))
    }))
    expect(result.exit._tag).toBe("Failure")
    expect(result.snapshot.phase).toBe("attached")
    expect(result.snapshot.activeGeneration).toBe(1)
    expect(calls).toContain("rollback")
    expect(calls).toContain("resume_source")
    expect(calls).not.toContain("revoke")
  })
})

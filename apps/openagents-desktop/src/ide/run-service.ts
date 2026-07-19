import { createHash, randomUUID } from "node:crypto"

import { Context, Effect, Layer, PubSub, Ref, Schema, Stream, SubscriptionRef } from "effect"

import {
  IdeArtifactRefSchema,
  IdeOutputChannelRefSchema,
  IdeOutputChunkSchema,
  IdeOutputSequenceSchema,
  IdeRunCommandResultSchema,
  IdeRunEventSchema,
  IdeRunReceiptRefSchema,
  IdeRunReceiptSchema,
  IdeRunSnapshotSchema,
  IdeTaskRunRefSchema,
  IdeTaskDiscoveryGenerationSchema,
  IdeTerminalReconnectGenerationSchema,
  IdeTerminalSplitRefSchema,
  IdeTestRunRefSchema,
  IdeTestDiscoveryGenerationSchema,
  type IdeArtifact,
  type IdeEnvironmentManifest,
  type IdeOutputChannel,
  type IdeOutputChunk,
  type IdeOutputLocation,
  type IdeOutputProducer,
  type IdeRunActor,
  type IdeRunBinding,
  type IdeRunEvent,
  type IdeRunReceipt,
  type IdeRunSnapshot,
  type IdeTaskDefinition,
  type IdeTaskRun,
  type IdeTerminalProfile,
  type IdeTerminalSession,
  type IdeTestController,
  type IdeTestItemResult,
  type IdeTestRun,
} from "./run-contract.ts"
import { IdeTimestampSchema } from "./project-contract.ts"

const MAX_RECEIPTS = 2_000
const MAX_RUNS = 1_000
const DEFAULT_OUTPUT_LIMIT = 262_144

export class IdeRunInvalidInput extends Schema.TaggedErrorClass<IdeRunInvalidInput>()(
  "IdeRun.InvalidInput",
  { operation: Schema.String, detail: Schema.String },
) {}

export class IdeRunStale extends Schema.TaggedErrorClass<IdeRunStale>()(
  "IdeRun.Stale",
  { operation: Schema.String, detail: Schema.String },
) {}

export class IdeRunNotFound extends Schema.TaggedErrorClass<IdeRunNotFound>()(
  "IdeRun.NotFound",
  { operation: Schema.String, detail: Schema.String },
) {}

export class IdeRunStopped extends Schema.TaggedErrorClass<IdeRunStopped>()(
  "IdeRun.Stopped",
  { operation: Schema.String, detail: Schema.String },
) {}

export type IdeRunServiceError = IdeRunInvalidInput | IdeRunStale | IdeRunNotFound | IdeRunStopped

type StartTerminalInput = Readonly<{
  session: IdeTerminalSession
  actor: IdeRunActor
}>

type AppendOutputInput = Readonly<{
  channelRef: IdeOutputChannel["channelRef"]
  producer: IdeOutputProducer
  stream: IdeOutputChunk["stream"]
  text: string
  byteLength: number
  redacted: boolean
  truncated: boolean
  gapBefore: boolean
  invalidEncoding: boolean
  locations: ReadonlyArray<IdeOutputLocation>
}>

type StartTaskInput = Readonly<{
  definitionRef: IdeTaskDefinition["definitionRef"]
  actor: IdeRunActor
}>

type SettleTaskInput = Readonly<{
  runRef: IdeTaskRun["runRef"]
  exitCode: number | null
  cancelled: boolean
  timedOut: boolean
  semanticChecksPassed: boolean
  problems: ReadonlyArray<IdeOutputLocation>
  artifacts: ReadonlyArray<IdeArtifact>
}>

type StartTestInput = Readonly<{
  controllerRef: IdeTestController["controllerRef"]
  itemRefs: ReadonlyArray<IdeTestRun["requestedItemRefs"][number]>
  profile: "run" | "coverage"
  actor: IdeRunActor
  retryOf: IdeTestRun["retryOf"]
}>

type SettleTestInput = Readonly<{
  runRef: IdeTestRun["runRef"]
  exitCode: number | null
  cancelled: boolean
  assertionsObserved: boolean
  results: ReadonlyArray<IdeTestItemResult>
  artifacts: ReadonlyArray<IdeArtifact>
  coveragePercent: number | null
}>

export interface IdeRunServiceShape {
  readonly snapshot: Effect.Effect<IdeRunSnapshot>
  readonly events: Stream.Stream<IdeRunEvent>
  readonly replaceDiscovery: (
    taskDefinitions: ReadonlyArray<IdeTaskDefinition>,
    testControllers: ReadonlyArray<IdeTestController>,
  ) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly terminalStarted: (input: StartTerminalInput) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly terminalRenamed: (
    sessionRef: IdeTerminalSession["sessionRef"],
    title: string,
  ) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly terminalSplit: (
    sessionRef: IdeTerminalSession["sessionRef"],
  ) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly terminalResized: (
    sessionRef: IdeTerminalSession["sessionRef"],
    cols: number,
    rows: number,
  ) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly terminalExited: (
    sessionRef: IdeTerminalSession["sessionRef"],
    exitCode: number | null,
    signal: string | null,
  ) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly terminalReconnected: (
    sessionRef: IdeTerminalSession["sessionRef"],
  ) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly terminalClosed: (
    sessionRef: IdeTerminalSession["sessionRef"],
    reason: "user" | "project_stale" | "app_quit" | "spawn_failed",
    actor: IdeRunActor,
  ) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly appendOutput: (input: AppendOutputInput) => Effect.Effect<IdeOutputChunk, IdeRunServiceError>
  readonly startTask: (input: StartTaskInput) => Effect.Effect<IdeTaskRun, IdeRunServiceError>
  readonly taskReady: (runRef: IdeTaskRun["runRef"]) => Effect.Effect<IdeTaskRun, IdeRunServiceError>
  readonly settleTask: (input: SettleTaskInput) => Effect.Effect<IdeTaskRun, IdeRunServiceError>
  readonly cancelTask: (runRef: IdeTaskRun["runRef"], actor: IdeRunActor) => Effect.Effect<IdeTaskRun, IdeRunServiceError>
  readonly startTests: (input: StartTestInput) => Effect.Effect<IdeTestRun, IdeRunServiceError>
  readonly settleTests: (input: SettleTestInput) => Effect.Effect<IdeTestRun, IdeRunServiceError>
  readonly cancelTests: (runRef: IdeTestRun["runRef"], actor: IdeRunActor) => Effect.Effect<IdeTestRun, IdeRunServiceError>
  readonly clearOutput: (channelRef: IdeOutputChannel["channelRef"]) => Effect.Effect<IdeRunSnapshot, IdeRunServiceError>
  readonly exportOutput: (
    channelRef: IdeOutputChannel["channelRef"],
    actor: IdeRunActor,
  ) => Effect.Effect<IdeArtifact, IdeRunServiceError>
  readonly stop: (reason: string) => Effect.Effect<IdeRunSnapshot>
}

export class IdeRunService extends Context.Service<IdeRunService, IdeRunServiceShape>()(
  "@openagents/desktop/IdeRunService",
) {}

const bindingMatches = (left: IdeRunBinding, right: IdeRunBinding): boolean =>
  left.projectRef === right.projectRef &&
  left.rootRef === right.rootRef &&
  left.worktreeRef === right.worktreeRef &&
  left.attachmentGeneration === right.attachmentGeneration &&
  left.placementGeneration === right.placementGeneration &&
  left.placementRef === right.placementRef &&
  left.cwdRef === right.cwdRef

const trim = <A>(values: ReadonlyArray<A>, maximum: number): ReadonlyArray<A> =>
  values.length <= maximum ? values : values.slice(values.length - maximum)

const emptyChannel = (
  channelRef: IdeOutputChannel["channelRef"],
  label: string,
  producer: IdeOutputProducer,
  retentionByteLimit = DEFAULT_OUTPUT_LIMIT,
): IdeOutputChannel => ({
  channelRef,
  label,
  producer,
  firstSequence: null,
  lastSequence: null,
  chunks: [],
  retainedBytes: 0,
  retentionByteLimit,
  droppedBytes: 0,
  gap: false,
  redactionCount: 0,
  disposed: false,
})

const outputText = (channel: IdeOutputChannel): string =>
  channel.chunks.map((chunk) => chunk.text).join("")

const findDependencyCycle = (
  definitions: ReadonlyArray<IdeTaskDefinition>,
): ReadonlyArray<string> | null => {
  const known = new Map<string, IdeTaskDefinition>(definitions.map((definition) => [definition.definitionRef, definition]))
  const visiting = new Set<string>()
  const visited = new Set<string>()
  const path: string[] = []
  const visit = (ref: string): ReadonlyArray<string> | null => {
    if (visiting.has(ref)) return [...path, ref]
    if (visited.has(ref)) return null
    const definition = known.get(ref)
    if (definition === undefined) return [ref]
    visiting.add(ref)
    path.push(ref)
    for (const dependency of definition.dependencies) {
      const cycle = visit(dependency)
      if (cycle !== null) return cycle
    }
    path.pop()
    visiting.delete(ref)
    visited.add(ref)
    return null
  }
  for (const definition of definitions) {
    const cycle = visit(definition.definitionRef)
    if (cycle !== null) return cycle
  }
  return null
}

const nowTimestamp = (now: () => string): IdeRunReceipt["recordedAt"] =>
  IdeTimestampSchema.make(now())

const retainUtf8Tail = (
  text: string,
  maximumBytes: number,
): Readonly<{ text: string; droppedBytes: number }> => {
  const originalBytes = Buffer.byteLength(text)
  if (originalBytes <= maximumBytes) return { text, droppedBytes: 0 }
  let retained = text
  while (retained.length > 0 && Buffer.byteLength(retained) > maximumBytes) {
    const first = retained.codePointAt(0)
    retained = retained.slice(first !== undefined && first > 0xffff ? 2 : 1)
  }
  return { text: retained, droppedBytes: originalBytes - Buffer.byteLength(retained) }
}

export const makeIdeRunServiceLayer = (
  seed: IdeRunSnapshot,
  options: Readonly<{ now?: () => string; outputByteLimit?: number }> = {},
): Layer.Layer<IdeRunService, IdeRunInvalidInput> =>
  Layer.effect(
    IdeRunService,
    Effect.gen(function* () {
      const decoded = yield* Schema.decodeUnknownEffect(IdeRunSnapshotSchema)(seed).pipe(
        Effect.mapError((cause) => new IdeRunInvalidInput({
          operation: "IdeRun.acquire",
          detail: String(cause).slice(0, 2_000),
        })),
      )
      const state = yield* SubscriptionRef.make(decoded)
      const stopped = yield* Ref.make<string | null>(decoded.stopped ? "seed stopped" : null)
      const events = yield* PubSub.bounded<IdeRunEvent>({ capacity: 1_024 })
      const now = options.now ?? (() => new Date().toISOString())
      const outputByteLimit = options.outputByteLimit ?? DEFAULT_OUTPUT_LIMIT

      const ensureActive = Effect.fn("IdeRun.ensureActive")(function* (operation: string) {
        const reason = yield* Ref.get(stopped)
        if (reason !== null) return yield* Effect.fail(new IdeRunStopped({ operation, detail: reason }))
      })

      const publishSnapshot = Effect.fn("IdeRun.publishSnapshot")(function* (snapshot: IdeRunSnapshot) {
        yield* PubSub.publish(events, IdeRunEventSchema.cases.Snapshot.make({ snapshot }))
        return snapshot
      })

      const update = Effect.fn("IdeRun.update")(function* (
        operation: string,
        change: (snapshot: IdeRunSnapshot) => IdeRunSnapshot,
      ) {
        yield* ensureActive(operation)
        const next = yield* SubscriptionRef.modify(state, (snapshot) => {
          const changed = IdeRunSnapshotSchema.make(change(snapshot))
          return [changed, changed] as const
        })
        return yield* publishSnapshot(next)
      })

      const receipt = (
        snapshot: IdeRunSnapshot,
        input: Readonly<{
          actor: IdeRunActor
          operation: IdeRunReceipt["operation"]
          subjectRef: string
          environment: IdeEnvironmentManifest
          outputChannelRef: IdeOutputChannel["channelRef"]
          outcome: string
        }>,
      ): IdeRunSnapshot => {
        const row = IdeRunReceiptSchema.make({
          receiptRef: IdeRunReceiptRefSchema.make(`ide.run-receipt.${input.operation}.${randomUUID()}`),
          actor: input.actor,
          operation: input.operation,
          subjectRef: input.subjectRef,
          binding: snapshot.binding,
          environmentManifestRef: input.environment.manifestRef,
          outputChannelRef: input.outputChannelRef,
          outcome: input.outcome,
          publicSafe: true,
          secretsIncluded: false,
          recordedAt: nowTimestamp(now),
        })
        return { ...snapshot, receipts: trim([...snapshot.receipts, row], MAX_RECEIPTS) }
      }

      const service = IdeRunService.of({
        snapshot: SubscriptionRef.get(state),
        events: Stream.fromPubSub(events),

        replaceDiscovery: Effect.fn("IdeRun.replaceDiscovery")(function* (taskDefinitions, testControllers) {
          if (new Set(taskDefinitions.map((definition) => definition.definitionRef)).size !== taskDefinitions.length) {
            return yield* Effect.fail(new IdeRunInvalidInput({
              operation: "IdeRun.replaceDiscovery",
              detail: "Task discovery contains duplicate definition references.",
            }))
          }
          if (new Set(testControllers.map((controller) => controller.controllerRef)).size !== testControllers.length) {
            return yield* Effect.fail(new IdeRunInvalidInput({
              operation: "IdeRun.replaceDiscovery",
              detail: "Test discovery contains duplicate controller references.",
            }))
          }
          const cycle = findDependencyCycle(taskDefinitions)
          if (cycle !== null) {
            return yield* Effect.fail(new IdeRunInvalidInput({
              operation: "IdeRun.replaceDiscovery",
              detail: `Task dependency cycle or missing dependency: ${cycle.join(" -> ")}`,
            }))
          }
          const snapshot = yield* SubscriptionRef.get(state)
          for (const definition of taskDefinitions) {
            if (!bindingMatches(definition.binding, snapshot.binding)) {
              return yield* Effect.fail(new IdeRunStale({
                operation: "IdeRun.replaceDiscovery",
                detail: `Task ${definition.definitionRef} is bound to a stale project generation.`,
              }))
            }
          }
          for (const controller of testControllers) {
            if (!bindingMatches(controller.binding, snapshot.binding)) {
              return yield* Effect.fail(new IdeRunStale({
                operation: "IdeRun.replaceDiscovery",
                detail: `Test controller ${controller.controllerRef} is bound to a stale project generation.`,
              }))
            }
            const itemRefs = controller.items.map((item) => item.itemRef)
            if (new Set(itemRefs).size !== itemRefs.length || controller.items.some((item) =>
              item.controllerRef !== controller.controllerRef || item.discoveryGeneration !== controller.discoveryGeneration)) {
              return yield* Effect.fail(new IdeRunInvalidInput({
                operation: "IdeRun.replaceDiscovery",
                detail: `Test controller ${controller.controllerRef} has duplicate or cross-generation items.`,
              }))
            }
          }
          return yield* update("IdeRun.replaceDiscovery", (current) => ({
            ...current,
            taskDefinitions,
            testControllers,
            taskDiscoveryGeneration: IdeTaskDiscoveryGenerationSchema.make(Math.max(
              current.taskDiscoveryGeneration,
              ...taskDefinitions.map((definition) => definition.discoveryGeneration),
            )),
            testDiscoveryGeneration: IdeTestDiscoveryGenerationSchema.make(Math.max(
              current.testDiscoveryGeneration,
              ...testControllers.map((controller) => controller.discoveryGeneration),
            )),
          }))
        }),

        terminalStarted: Effect.fn("IdeRun.terminalStarted")(function* ({ session, actor }) {
          const snapshot = yield* SubscriptionRef.get(state)
          if (!bindingMatches(session.binding, snapshot.binding)) {
            return yield* Effect.fail(new IdeRunStale({
              operation: "IdeRun.terminalStarted",
              detail: "The terminal binding does not match the active project generation.",
            }))
          }
          if (snapshot.terminals.some((candidate) => candidate.sessionRef === session.sessionRef)) {
            return yield* Effect.fail(new IdeRunInvalidInput({
              operation: "IdeRun.terminalStarted",
              detail: `Terminal ${session.sessionRef} already exists.`,
            }))
          }
          const channel = emptyChannel(session.outputChannelRef, `${session.title} output`, {
            _tag: "Terminal",
            sessionRef: session.sessionRef,
          }, outputByteLimit)
          return yield* update("IdeRun.terminalStarted", (current) => receipt({
            ...current,
            terminals: [...current.terminals, session],
            outputChannels: [...current.outputChannels, channel],
          }, {
            actor,
            operation: "terminal_create",
            subjectRef: session.sessionRef,
            environment: session.environment,
            outputChannelRef: session.outputChannelRef,
            outcome: "running",
          }))
        }),

        terminalRenamed: Effect.fn("IdeRun.terminalRenamed")(function* (sessionRef, title) {
          if (title.trim() === "") return yield* Effect.fail(new IdeRunInvalidInput({ operation: "IdeRun.terminalRenamed", detail: "Terminal title is empty." }))
          const snapshot = yield* SubscriptionRef.get(state)
          if (!snapshot.terminals.some((terminal) => terminal.sessionRef === sessionRef)) {
            return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.terminalRenamed", detail: `Terminal ${sessionRef} was not found.` }))
          }
          return yield* update("IdeRun.terminalRenamed", (current) => ({
            ...current,
            terminals: current.terminals.map((terminal) => terminal.sessionRef === sessionRef ? { ...terminal, title: title.trim() } : terminal),
          }))
        }),

        terminalSplit: Effect.fn("IdeRun.terminalSplit")(function* (sessionRef) {
          const snapshot = yield* SubscriptionRef.get(state)
          if (!snapshot.terminals.some((terminal) => terminal.sessionRef === sessionRef)) {
            return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.terminalSplit", detail: `Terminal ${sessionRef} was not found.` }))
          }
          const splitRef = IdeTerminalSplitRefSchema.make(`ide.terminal-split.${randomUUID()}`)
          return yield* update("IdeRun.terminalSplit", (current) => ({
            ...current,
            terminals: current.terminals.map((terminal) => terminal.sessionRef === sessionRef ? { ...terminal, splitRef } : terminal),
          }))
        }),

        terminalResized: Effect.fn("IdeRun.terminalResized")(function* (sessionRef, cols, rows) {
          const snapshot = yield* SubscriptionRef.get(state)
          if (!snapshot.terminals.some((terminal) => terminal.sessionRef === sessionRef)) {
            return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.terminalResized", detail: `Terminal ${sessionRef} was not found.` }))
          }
          if (!Number.isInteger(cols) || !Number.isInteger(rows) || cols < 1 || cols > 1_000 || rows < 1 || rows > 1_000) {
            return yield* Effect.fail(new IdeRunInvalidInput({ operation: "IdeRun.terminalResized", detail: "Terminal dimensions are outside the admitted range." }))
          }
          return yield* update("IdeRun.terminalResized", (current) => ({
            ...current,
            terminals: current.terminals.map((terminal) => terminal.sessionRef === sessionRef ? { ...terminal, cols, rows } : terminal),
          }))
        }),

        terminalExited: Effect.fn("IdeRun.terminalExited")(function* (sessionRef, exitCode, signal) {
          const exitedAt = nowTimestamp(now)
          return yield* update("IdeRun.terminalExited", (current) => ({
            ...current,
            terminals: current.terminals.map((terminal) => terminal.sessionRef === sessionRef ? {
              ...terminal,
              lifecycle: {
                _tag: "Exited" as const,
                startedAt: terminal.lifecycle._tag === "Running" || terminal.lifecycle._tag === "Starting"
                  ? terminal.lifecycle.startedAt
                  : exitedAt,
                exitedAt,
                exitCode,
                signal,
              },
            } : terminal),
          }))
        }),

        terminalReconnected: Effect.fn("IdeRun.terminalReconnected")(function* (sessionRef) {
          const snapshot = yield* SubscriptionRef.get(state)
          const terminal = snapshot.terminals.find((candidate) => candidate.sessionRef === sessionRef)
          if (terminal === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.terminalReconnected", detail: `Terminal ${sessionRef} was not found.` }))
          return yield* update("IdeRun.terminalReconnected", (current) => ({
            ...current,
            terminals: current.terminals.map((candidate) => candidate.sessionRef === sessionRef ? {
              ...candidate,
              reconnectGeneration: IdeTerminalReconnectGenerationSchema.make(candidate.reconnectGeneration + 1),
              lifecycle: { _tag: "Running" as const, startedAt: nowTimestamp(now), pidPresent: true },
            } : candidate),
          }))
        }),

        terminalClosed: Effect.fn("IdeRun.terminalClosed")(function* (sessionRef, reason, actor) {
          const snapshot = yield* SubscriptionRef.get(state)
          const terminal = snapshot.terminals.find((candidate) => candidate.sessionRef === sessionRef)
          if (terminal === undefined) return snapshot
          const closedAt = nowTimestamp(now)
          return yield* update("IdeRun.terminalClosed", (current) => receipt({
            ...current,
            terminals: current.terminals.map((candidate) => candidate.sessionRef === sessionRef ? {
              ...candidate,
              lifecycle: { _tag: "Closed" as const, closedAt, reason },
            } : candidate),
            outputChannels: current.outputChannels.map((channel) => channel.channelRef === terminal.outputChannelRef ? { ...channel, disposed: true } : channel),
          }, {
            actor,
            operation: "terminal_close",
            subjectRef: sessionRef,
            environment: terminal.environment,
            outputChannelRef: terminal.outputChannelRef,
            outcome: reason,
          }))
        }),

        appendOutput: Effect.fn("IdeRun.appendOutput")(function* (input) {
          yield* ensureActive("IdeRun.appendOutput")
          const snapshot = yield* SubscriptionRef.get(state)
          const channel = snapshot.outputChannels.find((candidate) => candidate.channelRef === input.channelRef)
          if (channel === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.appendOutput", detail: `Output channel ${input.channelRef} was not found.` }))
          if (channel.disposed) return yield* Effect.fail(new IdeRunStopped({ operation: "IdeRun.appendOutput", detail: `Output channel ${input.channelRef} is disposed.` }))
          const sequence = IdeOutputSequenceSchema.make((channel.lastSequence ?? 0) + 1)
          const chunk = IdeOutputChunkSchema.make({
            ...input,
            sequence,
            observedAt: nowTimestamp(now),
          })
          let chunks = [...channel.chunks, chunk]
          let retainedBytes = channel.retainedBytes + input.byteLength
          let droppedBytes = channel.droppedBytes
          while (retainedBytes > channel.retentionByteLimit && chunks.length > 1) {
            const removed = chunks.shift()
            if (removed === undefined) break
            retainedBytes -= removed.byteLength
            droppedBytes += removed.byteLength
          }
          if (retainedBytes > channel.retentionByteLimit && chunks.length === 1) {
            const only = chunks[0]
            if (only !== undefined) {
              const retained = retainUtf8Tail(only.text, channel.retentionByteLimit)
              const retainedChunk = IdeOutputChunkSchema.make({
                ...only,
                text: retained.text,
                byteLength: Buffer.byteLength(retained.text),
                truncated: true,
                gapBefore: true,
              })
              chunks = [retainedChunk]
              retainedBytes = retainedChunk.byteLength
              droppedBytes += retained.droppedBytes
            }
          }
          const next = yield* SubscriptionRef.modify(state, (current) => {
            const changed = IdeRunSnapshotSchema.make({
              ...current,
              outputChannels: current.outputChannels.map((candidate) => candidate.channelRef === input.channelRef ? {
                ...candidate,
                firstSequence: chunks[0]?.sequence ?? sequence,
                lastSequence: sequence,
                chunks,
                retainedBytes,
                droppedBytes,
                gap: candidate.gap || input.gapBefore || droppedBytes > 0,
                redactionCount: candidate.redactionCount + (input.redacted ? 1 : 0),
              } : candidate),
            })
            return [changed, changed] as const
          })
          yield* PubSub.publish(events, IdeRunEventSchema.cases.Output.make({ chunk }))
          yield* PubSub.publish(events, IdeRunEventSchema.cases.Snapshot.make({ snapshot: next }))
          return chunk
        }),

        startTask: Effect.fn("IdeRun.startTask")(function* ({ definitionRef, actor }) {
          const snapshot = yield* SubscriptionRef.get(state)
          const definition = snapshot.taskDefinitions.find((candidate) => candidate.definitionRef === definitionRef)
          if (definition === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.startTask", detail: `Task ${definitionRef} was not found.` }))
          if (!definition.executable.admitted) return yield* Effect.fail(new IdeRunInvalidInput({ operation: "IdeRun.startTask", detail: definition.executable.refusalReason ?? "Task executable is not admitted." }))
          const active = snapshot.taskRuns.find((run) => run.definitionRef === definitionRef && (run.outcome._tag === "Running" || run.outcome._tag === "Ready"))
          if (active !== undefined) return yield* Effect.fail(new IdeRunInvalidInput({ operation: "IdeRun.startTask", detail: `Task ${definitionRef} is already running.` }))
          const runRef = IdeTaskRunRefSchema.make(`ide.task-run.${randomUUID()}`)
          const outputChannelRef = IdeOutputChannelRefSchema.make(`ide.output-channel.task.${randomUUID()}`)
          const run = {
            runRef,
            definitionRef,
            actor,
            binding: snapshot.binding,
            outputChannelRef,
            outcome: { _tag: "Running" as const, startedAt: nowTimestamp(now), attempt: 1 },
            problems: [],
            artifacts: [],
            evidenceRefs: [],
          } satisfies IdeTaskRun
          const channel = emptyChannel(outputChannelRef, `${definition.label} output`, { _tag: "Task", runRef }, outputByteLimit)
          yield* update("IdeRun.startTask", (current) => receipt({
            ...current,
            taskRuns: trim([...current.taskRuns, run], MAX_RUNS),
            outputChannels: [...current.outputChannels, channel],
          }, {
            actor,
            operation: "task_run",
            subjectRef: runRef,
            environment: definition.environment,
            outputChannelRef,
            outcome: "running",
          }))
          return run
        }),

        taskReady: Effect.fn("IdeRun.taskReady")(function* (runRef) {
          const snapshot = yield* SubscriptionRef.get(state)
          const run = snapshot.taskRuns.find((candidate) => candidate.runRef === runRef)
          if (run === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.taskReady", detail: `Task run ${runRef} was not found.` }))
          if (run.outcome._tag !== "Running") return run
          const ready = {
            ...run,
            outcome: {
              _tag: "Ready" as const,
              startedAt: run.outcome.startedAt,
              readyAt: nowTimestamp(now),
              attempt: run.outcome.attempt,
            },
          }
          yield* update("IdeRun.taskReady", (current) => ({
            ...current,
            taskRuns: current.taskRuns.map((candidate) => candidate.runRef === runRef ? ready : candidate),
          }))
          return ready
        }),

        settleTask: Effect.fn("IdeRun.settleTask")(function* (input) {
          const snapshot = yield* SubscriptionRef.get(state)
          const run = snapshot.taskRuns.find((candidate) => candidate.runRef === input.runRef)
          if (run === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.settleTask", detail: `Task run ${input.runRef} was not found.` }))
          if (run.outcome._tag !== "Running" && run.outcome._tag !== "Ready") return run
          const completedAt = nowTimestamp(now)
          const outcome: IdeTaskRun["outcome"] = input.cancelled
            ? { _tag: "Cancelled", completedAt, actor: run.actor }
            : input.timedOut
              ? { _tag: "TimedOut", completedAt, timeoutMs: snapshot.taskDefinitions.find((definition) => definition.definitionRef === run.definitionRef)?.timeoutMs ?? 0 }
              : input.exitCode === 0 && input.semanticChecksPassed
                ? { _tag: "Succeeded", completedAt, exitCode: 0, semanticChecksPassed: true }
                : { _tag: "Failed", completedAt, exitCode: input.exitCode, reason: input.exitCode === 0 ? "Process exited zero but semantic completion evidence was incomplete." : "Task process failed." }
          const settled = { ...run, outcome, problems: input.problems, artifacts: input.artifacts }
          const definition = snapshot.taskDefinitions.find((candidate) => candidate.definitionRef === run.definitionRef)
          yield* update("IdeRun.settleTask", (current) => {
            const next = {
              ...current,
              taskRuns: current.taskRuns.map((candidate) => candidate.runRef === input.runRef ? settled : candidate),
              outputChannels: current.outputChannels.map((channel) => channel.channelRef === run.outputChannelRef ? { ...channel, disposed: true } : channel),
            }
            return definition === undefined ? next : receipt(next, {
              actor: run.actor,
              operation: "task_run",
              subjectRef: run.runRef,
              environment: definition.environment,
              outputChannelRef: run.outputChannelRef,
              outcome: outcome._tag,
            })
          })
          return settled
        }),

        cancelTask: Effect.fn("IdeRun.cancelTask")(function* (runRef, actor) {
          const snapshot = yield* SubscriptionRef.get(state)
          const run = snapshot.taskRuns.find((candidate) => candidate.runRef === runRef)
          if (run === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.cancelTask", detail: `Task run ${runRef} was not found.` }))
          if (run.outcome._tag !== "Running" && run.outcome._tag !== "Ready") return run
          const settled = { ...run, outcome: { _tag: "Cancelled" as const, completedAt: nowTimestamp(now), actor } }
          const definition = snapshot.taskDefinitions.find((candidate) => candidate.definitionRef === run.definitionRef)
          yield* update("IdeRun.cancelTask", (current) => {
            const next = {
              ...current,
              taskRuns: current.taskRuns.map((candidate) => candidate.runRef === runRef ? settled : candidate),
              outputChannels: current.outputChannels.map((channel) => channel.channelRef === run.outputChannelRef ? { ...channel, disposed: true } : channel),
            }
            return definition === undefined ? next : receipt(next, {
              actor,
              operation: "task_cancel",
              subjectRef: runRef,
              environment: definition.environment,
              outputChannelRef: run.outputChannelRef,
              outcome: "Cancelled",
            })
          })
          return settled
        }),

        startTests: Effect.fn("IdeRun.startTests")(function* (input) {
          const snapshot = yield* SubscriptionRef.get(state)
          const controller = snapshot.testControllers.find((candidate) => candidate.controllerRef === input.controllerRef)
          if (controller === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.startTests", detail: `Test controller ${input.controllerRef} was not found.` }))
          if (!controller.executable.admitted) return yield* Effect.fail(new IdeRunInvalidInput({ operation: "IdeRun.startTests", detail: controller.executable.refusalReason ?? "Test executable is not admitted." }))
          if (!controller.profiles.includes(input.profile)) return yield* Effect.fail(new IdeRunInvalidInput({ operation: "IdeRun.startTests", detail: `Test profile ${input.profile} is not admitted by this controller.` }))
          if (input.retryOf !== null) {
            const prior = snapshot.testRuns.find((run) => run.runRef === input.retryOf)
            if (prior === undefined || prior.controllerRef !== controller.controllerRef || prior.outcome._tag === "Running") {
              return yield* Effect.fail(new IdeRunInvalidInput({ operation: "IdeRun.startTests", detail: "The retry reference is absent, active, or belongs to another controller." }))
            }
          }
          if (!controller.discoveryComplete) return yield* Effect.fail(new IdeRunInvalidInput({ operation: "IdeRun.startTests", detail: controller.discoveryError ?? "Test discovery is incomplete." }))
          const requested = input.itemRefs.length === 0
            ? controller.items.filter((item) => item.runnable).map((item) => item.itemRef)
            : input.itemRefs
          if (requested.some((ref) => !controller.items.some((item) => item.itemRef === ref && item.runnable))) {
            return yield* Effect.fail(new IdeRunStale({ operation: "IdeRun.startTests", detail: "A requested test item is absent or stale." }))
          }
          const runRef = IdeTestRunRefSchema.make(`ide.test-run.${randomUUID()}`)
          const outputChannelRef = IdeOutputChannelRefSchema.make(`ide.output-channel.test.${randomUUID()}`)
          const run = {
            runRef,
            controllerRef: input.controllerRef,
            requestedItemRefs: requested,
            discoveryGeneration: controller.discoveryGeneration,
            actor: input.actor,
            binding: snapshot.binding,
            outputChannelRef,
            outcome: { _tag: "Running" as const, startedAt: nowTimestamp(now), profile: input.profile },
            results: requested.map((itemRef) => ({ itemRef, status: "queued" as const, durationMs: null, message: null, location: null })),
            artifacts: [],
            coveragePercent: null,
            retryOf: input.retryOf,
            evidenceRefs: [],
          } satisfies IdeTestRun
          const channel = emptyChannel(outputChannelRef, `${controller.label} output`, { _tag: "Test", runRef }, outputByteLimit)
          yield* update("IdeRun.startTests", (current) => receipt({
            ...current,
            testRuns: trim([...current.testRuns, run], MAX_RUNS),
            outputChannels: [...current.outputChannels, channel],
          }, {
            actor: input.actor,
            operation: "test_run",
            subjectRef: runRef,
            environment: controller.environment,
            outputChannelRef,
            outcome: "running",
          }))
          return run
        }),

        settleTests: Effect.fn("IdeRun.settleTests")(function* (input) {
          const snapshot = yield* SubscriptionRef.get(state)
          const run = snapshot.testRuns.find((candidate) => candidate.runRef === input.runRef)
          if (run === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.settleTests", detail: `Test run ${input.runRef} was not found.` }))
          if (run.outcome._tag !== "Running") return run
          const completedAt = nowTimestamp(now)
          const outcome: IdeTestRun["outcome"] = input.cancelled
            ? { _tag: "Cancelled", completedAt, actor: run.actor }
            : input.exitCode === 0 && input.assertionsObserved && input.results.every((result) => result.status === "passed" || result.status === "skipped")
              ? { _tag: "Succeeded", completedAt, exitCode: 0, assertionsObserved: true }
              : { _tag: "Failed", completedAt, exitCode: input.exitCode, reason: input.exitCode === 0 ? "Process exited zero but assertion/discovery evidence was incomplete." : "Test process failed." }
          const settled = { ...run, outcome, results: input.results, artifacts: input.artifacts, coveragePercent: input.coveragePercent }
          const controller = snapshot.testControllers.find((candidate) => candidate.controllerRef === run.controllerRef)
          yield* update("IdeRun.settleTests", (current) => {
            const next = {
              ...current,
              testRuns: current.testRuns.map((candidate) => candidate.runRef === input.runRef ? settled : candidate),
              outputChannels: current.outputChannels.map((channel) => channel.channelRef === run.outputChannelRef ? { ...channel, disposed: true } : channel),
            }
            return controller === undefined ? next : receipt(next, {
              actor: run.actor,
              operation: "test_run",
              subjectRef: run.runRef,
              environment: controller.environment,
              outputChannelRef: run.outputChannelRef,
              outcome: outcome._tag,
            })
          })
          return settled
        }),

        cancelTests: Effect.fn("IdeRun.cancelTests")(function* (runRef, actor) {
          const snapshot = yield* SubscriptionRef.get(state)
          const run = snapshot.testRuns.find((candidate) => candidate.runRef === runRef)
          if (run === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.cancelTests", detail: `Test run ${runRef} was not found.` }))
          if (run.outcome._tag !== "Running") return run
          const settled = { ...run, outcome: { _tag: "Cancelled" as const, completedAt: nowTimestamp(now), actor } }
          const controller = snapshot.testControllers.find((candidate) => candidate.controllerRef === run.controllerRef)
          yield* update("IdeRun.cancelTests", (current) => {
            const next = {
              ...current,
              testRuns: current.testRuns.map((candidate) => candidate.runRef === runRef ? settled : candidate),
              outputChannels: current.outputChannels.map((channel) => channel.channelRef === run.outputChannelRef ? { ...channel, disposed: true } : channel),
            }
            return controller === undefined ? next : receipt(next, {
              actor,
              operation: "test_cancel",
              subjectRef: runRef,
              environment: controller.environment,
              outputChannelRef: run.outputChannelRef,
              outcome: "Cancelled",
            })
          })
          return settled
        }),

        clearOutput: Effect.fn("IdeRun.clearOutput")(function* (channelRef) {
          const snapshot = yield* SubscriptionRef.get(state)
          const channel = snapshot.outputChannels.find((candidate) => candidate.channelRef === channelRef)
          if (channel === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.clearOutput", detail: `Output channel ${channelRef} was not found.` }))
          return yield* update("IdeRun.clearOutput", (current) => ({
            ...current,
            outputChannels: current.outputChannels.map((candidate) => candidate.channelRef === channelRef ? {
              ...candidate,
              firstSequence: null,
              lastSequence: null,
              chunks: [],
              retainedBytes: 0,
              droppedBytes: 0,
              gap: false,
              redactionCount: 0,
            } : candidate),
          }))
        }),

        exportOutput: Effect.fn("IdeRun.exportOutput")(function* (channelRef, actor) {
          const snapshot = yield* SubscriptionRef.get(state)
          const channel = snapshot.outputChannels.find((candidate) => candidate.channelRef === channelRef)
          if (channel === undefined) return yield* Effect.fail(new IdeRunNotFound({ operation: "IdeRun.exportOutput", detail: `Output channel ${channelRef} was not found.` }))
          const text = outputText(channel)
          const artifact = {
            artifactRef: IdeArtifactRefSchema.make(`ide.artifact.output.${randomUUID()}`),
            label: `${channel.label} export`,
            pathRef: `exports/${channel.channelRef}.txt`,
            mediaType: "text/plain; charset=utf-8",
            byteLength: Buffer.byteLength(text),
            digest: `sha256:${createHash("sha256").update(text).digest("hex")}`,
            available: true,
          } satisfies IdeArtifact
          const manifestRef = snapshot.receipts.findLast((row) => row.outputChannelRef === channelRef)?.environmentManifestRef
          const environment = snapshot.taskDefinitions.find((definition) => definition.environment.manifestRef === manifestRef)?.environment
            ?? snapshot.testControllers.find((controller) => controller.environment.manifestRef === manifestRef)?.environment
            ?? snapshot.terminals.find((terminal) => terminal.outputChannelRef === channelRef)?.environment
          if (environment !== undefined) {
            yield* update("IdeRun.exportOutput", (current) => receipt(current, {
              actor,
              operation: "output_export",
              subjectRef: artifact.artifactRef,
              environment,
              outputChannelRef: channelRef,
              outcome: "exported",
            }))
          }
          return artifact
        }),

        stop: Effect.fn("IdeRun.stop")(function* (reason) {
          const already = yield* Ref.get(stopped)
          if (already !== null) return yield* SubscriptionRef.get(state)
          yield* Ref.set(stopped, reason)
          const closedAt = nowTimestamp(now)
          const next = yield* SubscriptionRef.modify(state, (current) => {
            const changed = IdeRunSnapshotSchema.make({
              ...current,
              terminals: current.terminals.map((terminal) => terminal.lifecycle._tag === "Closed" ? terminal : {
                ...terminal,
                lifecycle: { _tag: "Closed" as const, closedAt, reason: "app_quit" as const },
              }),
              taskRuns: current.taskRuns.map((run) => run.outcome._tag === "Running" || run.outcome._tag === "Ready" ? {
                ...run,
                outcome: { _tag: "Cancelled" as const, completedAt: closedAt, actor: run.actor },
              } : run),
              testRuns: current.testRuns.map((run) => run.outcome._tag === "Running" ? {
                ...run,
                outcome: { _tag: "Cancelled" as const, completedAt: closedAt, actor: run.actor },
              } : run),
              outputChannels: current.outputChannels.map((channel) => ({ ...channel, disposed: true })),
              stopped: true,
            })
            return [changed, changed] as const
          })
          yield* PubSub.publish(events, IdeRunEventSchema.cases.Snapshot.make({ snapshot: next }))
          return next
        }),
      })

      yield* Effect.addFinalizer(() => Effect.gen(function* () {
        yield* service.stop("layer scope closed")
        yield* PubSub.shutdown(events)
      }))

      return service
    }),
  )

export const emptyIdeRunSnapshot = (
  binding: IdeRunBinding,
  profiles: ReadonlyArray<IdeTerminalProfile> = [],
): IdeRunSnapshot => IdeRunSnapshotSchema.make({
  schemaVersion: "openagents.desktop.ide-run.v1",
  binding,
  taskDiscoveryGeneration: IdeTaskDiscoveryGenerationSchema.make(1),
  testDiscoveryGeneration: IdeTestDiscoveryGenerationSchema.make(1),
  profiles,
  terminals: [],
  taskDefinitions: [],
  taskRuns: [],
  testControllers: [],
  testRuns: [],
  outputChannels: [],
  receipts: [],
  stopped: false,
})

export const refusedIdeRunCommand = (
  reason: Extract<ReturnType<typeof IdeRunCommandResultSchema.cases.Refused.make>, { readonly _tag: "Refused" }>["reason"],
  message: string,
  snapshot: IdeRunSnapshot,
) => IdeRunCommandResultSchema.cases.Refused.make({ reason, message, snapshot })

import { Effect, Stream } from "effect"
import {
  decodePylonLifecycleWireEventJson,
  type PylonLifecycleWireEvent,
} from "@openagentsinc/agent-runtime-schema"

import type {
  KhalaCodeDesktopFleetAssignment,
  KhalaCodeDesktopFleetStatus,
  KhalaCodeDesktopFleetWorkerControlRequest,
} from "../shared/rpc"

export type KhalaFleetWorkerNeutralState =
  | "idle"
  | "queued"
  | "working"
  | "waiting"
  | "blocked"
  | "done"
  | "failed"
  | "offline"

export type KhalaFleetWorkerLifecycleFrame = Readonly<{
  assignmentRef: string | null
  elapsedMs: number | null
  event: string
  line: string
  observedAt: string
  tokenCountKind: "exact" | "estimated" | null
  tokensSoFar: number | null
}>

export type KhalaFleetWorkerCard = Readonly<{
  assignmentRef: string | null
  assignmentRefHash: string | null
  blockerRefs: readonly string[]
  claimedWorkUnit: string
  closeoutStatus: string | null
  elapsedMs: number | null
  issueRef: string | null
  issueRefHash: string | null
  lifecycle: KhalaFleetWorkerLifecycleFrame | null
  neutralState: KhalaFleetWorkerNeutralState
  tokenLabel: string
  workerRefHash: string
}>

export type KhalaFleetWorkerCardActionHandlers = Readonly<{
  onWorkerControl: (
    request: KhalaCodeDesktopFleetWorkerControlRequest,
  ) => void
}>

const hashRef = (value: string): string => {
  let hash = 0x811c9dc5
  for (let index = 0; index < value.length; index += 1) {
    hash ^= value.charCodeAt(index)
    hash = Math.imul(hash, 0x01000193)
  }
  return (hash >>> 0).toString(16).padStart(8, "0")
}

export const khalaFleetWorkerRefHash = (
  value: string | null | undefined,
  fallback: string,
): string => `ref.${hashRef(value ?? fallback)}`

const eventName = (event: PylonLifecycleWireEvent): string =>
  event.schema === "openagents.pylon.assignment_run_lifecycle_event.v0.1"
    ? event.event
    : event.assignmentEvent ?? event.state

const eventAssignmentRef = (event: PylonLifecycleWireEvent): string | null =>
  event.assignmentRef ?? null

const eventElapsedMs = (event: PylonLifecycleWireEvent): number | null =>
  event.schema === "openagents.pylon.assignment_run_lifecycle_event.v0.1"
    ? event.elapsedMs ?? null
    : null

const eventTokensSoFar = (event: PylonLifecycleWireEvent): number | null =>
  event.schema === "openagents.pylon.assignment_run_lifecycle_event.v0.1"
    ? event.tokensSoFar ?? null
    : null

const eventTokenCountKind = (
  event: PylonLifecycleWireEvent,
): "exact" | "estimated" | null =>
  event.schema === "openagents.pylon.assignment_run_lifecycle_event.v0.1"
    ? event.tokenCountKind ?? null
    : null

const lifecycleLine = (event: PylonLifecycleWireEvent): string => {
  const parts = [eventName(event)]
  if (event.schema === "openagents.pylon.assignment_run_lifecycle_event.v0.1") {
    if (event.phase !== undefined) parts.push(`phase=${event.phase}`)
    if (event.status !== undefined) parts.push(`status=${event.status}`)
    if (event.tokensSoFar !== undefined) parts.push(`tokens=${event.tokensSoFar}`)
  } else if (event.status !== undefined) {
    parts.push(`status=${event.status}`)
  }
  return parts.join(" ")
}

export const khalaFleetWorkerLifecycleFrameFromWireEvent = (
  event: PylonLifecycleWireEvent,
): KhalaFleetWorkerLifecycleFrame => ({
  assignmentRef: eventAssignmentRef(event),
  elapsedMs: eventElapsedMs(event),
  event: eventName(event),
  line: lifecycleLine(event),
  observedAt: event.observedAt,
  tokenCountKind: eventTokenCountKind(event),
  tokensSoFar: eventTokensSoFar(event),
})

export const parseKhalaFleetWorkerLifecycleNdjsonLine = (
  line: string,
): KhalaFleetWorkerLifecycleFrame | null => {
  const trimmed = line.trim()
  if (!trimmed.startsWith("{") || !trimmed.endsWith("}")) return null
  try {
    return khalaFleetWorkerLifecycleFrameFromWireEvent(
      decodePylonLifecycleWireEventJson(trimmed),
    )
  } catch {
    return null
  }
}

export const khalaFleetWorkerLifecycleFramesFromNdjson = (
  ndjson: string,
): Promise<readonly KhalaFleetWorkerLifecycleFrame[]> => {
  const frames: KhalaFleetWorkerLifecycleFrame[] = []
  return Effect.runPromise(Effect.gen(function* () {
    yield* Stream.fromIterable([new TextEncoder().encode(ndjson)]).pipe(
      Stream.decodeText(),
      Stream.splitLines,
      Stream.runForEach(line =>
        Effect.sync(() => {
          const frame = parseKhalaFleetWorkerLifecycleNdjsonLine(line)
          if (frame !== null) frames.push(frame)
        }),
      ),
    )
    return frames
  }))
}

const bytesFromLifecycleChunks = async function* (
  source: AsyncIterable<string | Uint8Array>,
): AsyncIterable<Uint8Array> {
  const encoder = new TextEncoder()
  for await (const chunk of source) {
    yield typeof chunk === "string" ? encoder.encode(chunk) : chunk
  }
}

export const consumeKhalaFleetWorkerLifecycleNdjson = (
  source: AsyncIterable<string | Uint8Array>,
  onFrame: (frame: KhalaFleetWorkerLifecycleFrame) => void | Promise<void>,
): Promise<void> =>
  Effect.runPromise(
    Stream.fromAsyncIterable(bytesFromLifecycleChunks(source), error => new Error(String(error))).pipe(
      Stream.decodeText(),
      Stream.splitLines,
      Stream.runForEach(line =>
        Effect.promise(async () => {
          const frame = parseKhalaFleetWorkerLifecycleNdjsonLine(line)
          if (frame !== null) await onFrame(frame)
        }),
      ),
      Effect.catch(() => Effect.void),
    ),
  )

export type KhalaFleetWorkerCardUpdate = Readonly<{
  frame: KhalaFleetWorkerLifecycleFrame
  frames: readonly KhalaFleetWorkerLifecycleFrame[]
}>

export type KhalaFleetWorkerCardThrottler = Readonly<{
  flush: () => void
  push: (frame: KhalaFleetWorkerLifecycleFrame) => void
}>

export const createKhalaFleetWorkerCardThrottler = (input: {
  readonly intervalMs: number
  readonly onUpdate: (update: KhalaFleetWorkerCardUpdate) => void
  readonly setTimeout?: (callback: () => void, ms: number) => number
  readonly clearTimeout?: (handle: number) => void
}): KhalaFleetWorkerCardThrottler => {
  const setTimer = input.setTimeout ?? ((callback, ms) => window.setTimeout(callback, ms))
  const clearTimer = input.clearTimeout ?? (handle => window.clearTimeout(handle))
  const framesByAssignment = new Map<string, KhalaFleetWorkerLifecycleFrame>()
  const pendingByAssignment = new Map<string, KhalaFleetWorkerLifecycleFrame>()
  let timer: number | null = null
  const assignmentKey = (frame: KhalaFleetWorkerLifecycleFrame): string =>
    frame.assignmentRef ?? `__unassigned__:${frame.observedAt}:${frame.event}`

  const flush = (): void => {
    if (timer !== null) {
      clearTimer(timer)
      timer = null
    }
    if (pendingByAssignment.size === 0) return
    const pending = [...pendingByAssignment.values()]
    pendingByAssignment.clear()
    for (const frame of pending) {
      framesByAssignment.set(assignmentKey(frame), frame)
    }
    const frames = [...framesByAssignment.values()]
    for (const frame of pending) {
      input.onUpdate({ frame, frames })
    }
  }

  return {
    flush,
    push: frame => {
      pendingByAssignment.set(assignmentKey(frame), frame)
      if (timer !== null) return
      timer = setTimer(flush, Math.max(0, input.intervalMs))
    },
  }
}

const neutralStateForAssignment = (
  assignment: KhalaCodeDesktopFleetAssignment,
): KhalaFleetWorkerNeutralState => {
  const blockers = assignment.blockerRefs ?? assignment.workerSession?.blockerRefs ?? []
  if (blockers.length > 0) return "blocked"
  const closeout = assignment.closeoutStatus ?? assignment.workerSession?.closeoutStatus
  if (closeout === "accepted" || closeout === "completed") return "done"
  if (closeout === "failed" || closeout === "rejected") return "failed"
  if (assignment.workerSession?.approvalState === "approval_required") return "waiting"
  return "working"
}

const tokenLabel = (
  assignment: KhalaCodeDesktopFleetAssignment,
  frame: KhalaFleetWorkerLifecycleFrame | null,
): string => {
  if (assignment.tokenRate.status === "exact") {
    const tokens = assignment.tokenRate.tokens === null ? "unknown" : String(assignment.tokenRate.tokens)
    return `${tokens} exact`
  }
  if (assignment.tokenRate.status === "pending") return "pending exact rows"
  if (assignment.tokenRate.status === "not_measured") {
    if (frame?.tokensSoFar !== null && frame?.tokensSoFar !== undefined) {
      const kind = frame.tokenCountKind ?? "pending"
      return `${frame.tokensSoFar} ${kind}`
    }
    return "not measured"
  }
  const tokens = assignment.tokenRate.tokens === null ? "unknown" : String(assignment.tokenRate.tokens)
  return `${tokens} ${assignment.tokenRate.status}`
}

export const buildKhalaFleetWorkerCards = (
  status: KhalaCodeDesktopFleetStatus,
  frames: readonly KhalaFleetWorkerLifecycleFrame[] = [],
): readonly KhalaFleetWorkerCard[] => {
  const latestByAssignment = new Map<string, KhalaFleetWorkerLifecycleFrame>()
  for (const frame of frames) {
    if (frame.assignmentRef !== null) latestByAssignment.set(frame.assignmentRef, frame)
  }
  return status.activeAssignments.map((assignment, index) => {
    const assignmentRefHash = assignment.assignmentRef === null
      ? null
      : khalaFleetWorkerRefHash(assignment.assignmentRef, `assignment-${index}`)
    const issueRefHash = assignment.issueRef === null
      ? null
      : khalaFleetWorkerRefHash(assignment.issueRef, `issue-${index}`)
    const workerRefHash = khalaFleetWorkerRefHash(
      assignment.assignmentRef ?? assignment.issueRef,
      `worker-${index}`,
    )
    const lifecycle = assignment.assignmentRef === null
      ? null
      : latestByAssignment.get(assignment.assignmentRef) ?? null
    return {
      assignmentRef: assignment.assignmentRef,
      assignmentRefHash,
      blockerRefs: assignment.blockerRefs ?? assignment.workerSession?.blockerRefs ?? [],
      claimedWorkUnit: issueRefHash ?? assignmentRefHash ?? workerRefHash,
      closeoutStatus: assignment.closeoutStatus ?? assignment.workerSession?.closeoutStatus ?? null,
      elapsedMs: lifecycle?.elapsedMs ?? assignment.elapsedMs,
      issueRef: assignment.issueRef,
      issueRefHash,
      lifecycle,
      neutralState: neutralStateForAssignment(assignment),
      tokenLabel: tokenLabel(assignment, lifecycle),
      workerRefHash,
    }
  })
}

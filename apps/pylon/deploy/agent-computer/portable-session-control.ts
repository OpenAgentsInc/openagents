#!/usr/bin/env node
import { Runtime } from "@openagentsinc/runtime-platform"

/**
 * Fixed in-guest PORT-03 controller for retained Agent Computers.
 *
 * The host invokes exactly this binary with one public-safe JSON operation.
 * Repository/checkpoint bytes must already have been materialized into the
 * derived guest-local session root; this controller verifies them before stage
 * and never accepts an arbitrary command from the caller.
 */

import { createHash } from "node:crypto"
import { spawn } from "node:child_process"
import { chmod, lstat, mkdir, readFile, readdir, readlink, rename, rm, symlink, writeFile } from "node:fs/promises"
import { dirname, join, posix } from "node:path"

const SAFE_REF = /^[A-Za-z0-9][A-Za-z0-9._:-]{2,255}$/u
const FORBIDDEN_PRIVATE_MATERIAL =
  /(?:Bearer|Basic)\s+[A-Za-z0-9._~+/-]+=*|(?:\/Users\/|\/home\/|[A-Za-z]:\\Users\\)|"(?:token|apiKey|password|secret|credential|mnemonic|hostname|processId|socket|authHome)"\s*:/iu

type Action = "stage" | "activate" | "abort" | "quiesce" | "checkpoint" | "reclaim" | "wipeCapability"
type Operation = Readonly<{
  operationRef: string
  action: Action
  ownerRef: string
  targetRef: string
  sessionRef: string
  attachmentRef: string
  generation: number
  resourceRef?: string
  payload: Record<string, unknown>
}>

type ControllerState = Readonly<{
  schema: "openagents.portable_agent_computer_guest_state.v1"
  ownerRef: string
  targetRef: string
  sessionRef: string
  attachmentRef: string
  generation: number
  state: "staged" | "active" | "quiesced" | "reclaimed"
  bundle: Record<string, unknown>
  operations: Record<string, Readonly<{ fingerprint: string; response: unknown }>>
}>

type PortableGuestHelperKind = "pty" | "lsp" | "dap" | "watcher" | "native"
type PortableGuestHelperReadiness = Readonly<{
  kind: PortableGuestHelperKind
  readiness: "ready" | "unsupported"
  instanceRef: string | null
  versionRef: string | null
  omissionRef: string | null
  evidenceRefs: ReadonlyArray<string>
}>

export type PortableSessionGuestRuntime = Readonly<{
  prepare: (input: Readonly<{ sessionRoot: string; agentRefs: ReadonlyArray<string> }>) => Promise<void>
  verifyStage: (input: Readonly<{ sessionRoot: string; bundle: Record<string, unknown> }>) => Promise<void>
  verifyCapabilities: (input: Readonly<{ sessionRoot: string; leaseRefs: ReadonlyArray<string> }>) => Promise<void>
  activate: (input: Readonly<{
    sessionRoot: string
    agentRefs: ReadonlyArray<string>
    destinationRunnerSessionReservationRef: string
    generation: number
  }>) => Promise<ReadonlyArray<PortableGuestHelperReadiness>>
  verifyActivation: (input: Readonly<{
    sessionRoot: string
    destinationRunnerSessionReservationRef: string
    generation: number
  }>) => Promise<void>
  continueWork: (input: Readonly<{
    sessionRoot: string
    ownerRef: string
    repositoryRef: string
    providerLeaseRef: string
    turns: ReadonlyArray<Readonly<{ agentRef: string; turnRef: string; task: string; activityCursor: number; eventCursor: number }>>
  }>) => Promise<ReadonlyArray<Readonly<{ agentRef: string; turnRef: string; activityCursor: number; eventCursor: number }>>>
  quiesce: (input: Readonly<{ sessionRoot: string; agentRefs: ReadonlyArray<string> }>) => Promise<void>
  reclaim: (input: Readonly<{ sessionRoot: string; agentRefs: ReadonlyArray<string> }>) => Promise<void>
  repositorySnapshot: (sessionRoot: string) => Promise<Readonly<{
    repositoryRevisionRef: string
    repositoryPostImageDigest: string
    diffDigest: string
  }>>
}>

export class PortableSessionControlError extends Error {
  readonly _tag = "PortableSessionControlError"
  override readonly name = "PortableSessionControlError"
}

type Continuation = Readonly<{
  operationRef: string
  ownerRef: string
  targetRef: string
  sessionRef: string
  attachmentRef: string
  generation: number
  providerLeaseRef: string
  expectedThreadCursors: ReadonlyArray<Readonly<{ agentRef: string; threadRef: string; activityCursor: number; eventCursor: number }>>
  turns: ReadonlyArray<Readonly<{ agentRef: string; turnRef: string; task: string }>>
}>

const canonicalJson = (value: unknown): string => {
  if (value === null || typeof value !== "object") return JSON.stringify(value)
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`
  const record = value as Record<string, unknown>
  return `{${Object.keys(record).sort().map(key => `${JSON.stringify(key)}:${canonicalJson(record[key])}`).join(",")}}`
}

const digest = (value: unknown): `sha256:${string}` =>
  `sha256:${createHash("sha256").update(canonicalJson(value)).digest("hex")}`

const stableRef = (prefix: string, seed: string): string =>
  `${prefix}.${createHash("sha256").update(seed).digest("hex").slice(0, 24)}`

const asObject = (value: unknown, field: string): Record<string, unknown> => {
  if (value === null || typeof value !== "object" || Array.isArray(value)) {
    throw new PortableSessionControlError(`${field} must be an object`)
  }
  return value as Record<string, unknown>
}

const asString = (value: unknown, field: string): string => {
  if (typeof value !== "string" || !SAFE_REF.test(value)) {
    throw new PortableSessionControlError(`${field} must be a public-safe ref`)
  }
  return value
}

const validateHelperReadiness = (helpers: ReadonlyArray<PortableGuestHelperReadiness>): void => {
  const expected = ["pty", "lsp", "dap", "watcher", "native"] as const
  if (helpers.length !== expected.length || expected.some(kind =>
    helpers.filter(helper => helper.kind === kind).length !== 1)) {
    throw new PortableSessionControlError("activation helper inventory is incomplete")
  }
  for (const helper of helpers) {
    if (Object.keys(helper).sort().join("\0") !==
        ["evidenceRefs", "instanceRef", "kind", "omissionRef", "readiness", "versionRef"].join("\0") ||
        new Set(helper.evidenceRefs).size !== helper.evidenceRefs.length ||
        helper.evidenceRefs.some(ref => !SAFE_REF.test(ref))) {
      throw new PortableSessionControlError("activation helper receipt is invalid")
    }
    if (helper.readiness === "ready") {
      if (helper.instanceRef === null || helper.versionRef === null || helper.omissionRef !== null ||
          !SAFE_REF.test(helper.instanceRef) || !SAFE_REF.test(helper.versionRef) || helper.evidenceRefs.length === 0) {
        throw new PortableSessionControlError("ready helper has no live identity evidence")
      }
    } else if (helper.instanceRef !== null || helper.versionRef !== null || helper.omissionRef === null ||
               !SAFE_REF.test(helper.omissionRef) || helper.evidenceRefs.length !== 0) {
      throw new PortableSessionControlError("unsupported helper claims a live identity")
    }
  }
}

const publicSafe = <A>(value: A): A => {
  if (FORBIDDEN_PRIVATE_MATERIAL.test(canonicalJson(value))) {
    throw new PortableSessionControlError("operation contains forbidden private material")
  }
  return value
}

export const portableSessionRoot = (stateRoot: string, sessionRef: string): string =>
  join(stateRoot, createHash("sha256").update(sessionRef).digest("hex").slice(0, 24))

const statePath = (sessionRoot: string): string => join(sessionRoot, "portable-control.json")
const workspacePath = (sessionRoot: string): string => join(sessionRoot, "workspace")
const agentStatePath = (sessionRoot: string, agentRef: string): string =>
  join(sessionRoot, "agents", createHash("sha256").update(agentRef).digest("hex").slice(0, 24))

const readState = async (sessionRoot: string): Promise<ControllerState | undefined> => {
  try {
    return JSON.parse(await readFile(statePath(sessionRoot), "utf8")) as ControllerState
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return undefined
    throw error
  }
}

const writeState = async (sessionRoot: string, state: ControllerState): Promise<void> => {
  await mkdir(sessionRoot, { recursive: true })
  const path = statePath(sessionRoot)
  const temporary = `${path}.tmp-${process.pid}`
  await writeFile(temporary, canonicalJson(publicSafe(state)), { mode: 0o600 })
  await rename(temporary, path)
}

const graphAgentRefs = (graphValue: unknown): ReadonlyArray<string> => {
  const graph = asObject(graphValue, "graph")
  if (!Array.isArray(graph.nodes) || graph.nodes.length === 0) {
    throw new PortableSessionControlError("graph.nodes must be non-empty")
  }
  const refs = graph.nodes.map((node, index) => asString(asObject(node, `graph.nodes[${index}]`).agentRef, "agentRef"))
  if (new Set(refs).size !== refs.length) throw new PortableSessionControlError("graph agent refs must be unique")
  return refs
}

const validateOperation = (input: unknown): Operation => {
  const value = publicSafe(asObject(input, "operation"))
  const action = value.action
  if (!["stage", "activate", "abort", "quiesce", "checkpoint", "reclaim", "wipeCapability"].includes(String(action))) {
    throw new PortableSessionControlError("action is invalid")
  }
  const generation = Number(value.generation)
  if (!Number.isSafeInteger(generation) || generation <= 0) {
    throw new PortableSessionControlError("generation must be positive")
  }
  return {
    operationRef: asString(value.operationRef, "operationRef"),
    action: action as Action,
    ownerRef: asString(value.ownerRef, "ownerRef"),
    targetRef: asString(value.targetRef, "targetRef"),
    sessionRef: asString(value.sessionRef, "sessionRef"),
    attachmentRef: asString(value.attachmentRef, "attachmentRef"),
    generation,
    ...(value.resourceRef === undefined ? {} : { resourceRef: asString(value.resourceRef, "resourceRef") }),
    payload: asObject(value.payload, "payload"),
  }
}

const assertScope = (state: ControllerState, operation: Operation): void => {
  if (state.ownerRef !== operation.ownerRef || state.targetRef !== operation.targetRef ||
      state.sessionRef !== operation.sessionRef || state.attachmentRef !== operation.attachmentRef ||
      state.generation !== operation.generation) {
    throw new PortableSessionControlError("operation does not match retained attachment generation")
  }
}

const validateContinuation = (input: unknown): Continuation => {
  const value = asObject(input, "continuation")
  const generation = Number(value.generation)
  if (!Number.isSafeInteger(generation) || generation <= 0) throw new PortableSessionControlError("generation must be positive")
  if (!Array.isArray(value.turns) || value.turns.length === 0 || value.turns.length > 64) {
    throw new PortableSessionControlError("continuation turns must be bounded")
  }
  const turns = value.turns.map((entry, index) => {
    const turn = asObject(entry, `turns[${index}]`)
    if (typeof turn.task !== "string" || turn.task.trim().length === 0 || Buffer.byteLength(turn.task) > 16 * 1024) {
      throw new PortableSessionControlError("continuation task must be non-empty and bounded")
    }
    return {
      agentRef: asString(turn.agentRef, "agentRef"),
      turnRef: asString(turn.turnRef, "turnRef"),
      task: turn.task,
    }
  })
  if (!Array.isArray(value.expectedThreadCursors) || value.expectedThreadCursors.length !== turns.length) {
    throw new PortableSessionControlError("expected thread cursors must cover every turn")
  }
  const expectedThreadCursors = value.expectedThreadCursors.map((entry, index) => {
    const cursor = asObject(entry, `expectedThreadCursors[${index}]`)
    const activityCursor = Number(cursor.activityCursor)
    const eventCursor = Number(cursor.eventCursor)
    if (!Number.isSafeInteger(activityCursor) || activityCursor < 0 || !Number.isSafeInteger(eventCursor) || eventCursor < 0) {
      throw new PortableSessionControlError("expected thread cursor is invalid")
    }
    return { agentRef: asString(cursor.agentRef, "agentRef"), threadRef: asString(cursor.threadRef, "threadRef"), activityCursor, eventCursor }
  })
  return {
    operationRef: asString(value.operationRef, "operationRef"),
    ownerRef: asString(value.ownerRef, "ownerRef"),
    targetRef: asString(value.targetRef, "targetRef"),
    sessionRef: asString(value.sessionRef, "sessionRef"),
    attachmentRef: asString(value.attachmentRef, "attachmentRef"),
    generation,
    providerLeaseRef: asString(value.providerLeaseRef, "providerLeaseRef"),
    expectedThreadCursors,
    turns,
  }
}

export const continuePortableSession = async (input: Readonly<{
  continuation: unknown
  stateRoot: string
  runtime: PortableSessionGuestRuntime
}>): Promise<unknown> => {
  const continuation = validateContinuation(input.continuation)
  const sessionRoot = portableSessionRoot(input.stateRoot, continuation.sessionRef)
  const state = await readState(sessionRoot)
  if (state === undefined || state.state !== "active") throw new PortableSessionControlError("continuation requires active retained state")
  assertScope(state, { ...continuation, action: "activate", payload: {} })
  const fingerprint = digest(continuation)
  const replay = state.operations[continuation.operationRef]
  if (replay !== undefined) {
    if (replay.fingerprint !== fingerprint) throw new PortableSessionControlError("operation bytes conflict")
    return { ...(replay.response as Record<string, unknown>), replay: "replayed" }
  }
  const agentRefs = graphAgentRefs(asObject(state.bundle.graph, "retained graph"))
  if (continuation.turns.length !== agentRefs.length ||
      continuation.turns.some((turn, index) => turn.agentRef !== agentRefs[index]) ||
      new Set(continuation.turns.map(turn => turn.turnRef)).size !== continuation.turns.length) {
    throw new PortableSessionControlError("continuation requires one unique turn for each canonical graph agent")
  }
  const graphNodes = (asObject(state.bundle.graph, "graph").nodes as ReadonlyArray<Record<string, unknown>>)
  const retainedCursors = state.bundle.threadCursors as ReadonlyArray<Record<string, unknown>>
  if (!Array.isArray(retainedCursors) || continuation.expectedThreadCursors.some((cursor, index) => {
    const node = graphNodes[index]
    const retained = retainedCursors.find(row => row.threadRef === cursor.threadRef)
    return cursor.agentRef !== continuation.turns[index]?.agentRef || node?.threadRef !== cursor.threadRef ||
      Number(node.activityCursor) !== cursor.activityCursor || Number(retained?.activityCursor) !== cursor.activityCursor ||
      Number(retained?.eventCursor) !== cursor.eventCursor
  })) {
    throw new PortableSessionControlError("continuation expected cursors differ from retained graph")
  }
  await input.runtime.verifyCapabilities({ sessionRoot, leaseRefs: [continuation.providerLeaseRef] })
  const executionBinding = asObject(state.bundle.executionBinding, "executionBinding")
  const completed = await input.runtime.continueWork({
    sessionRoot,
    ownerRef: continuation.ownerRef,
    repositoryRef: asString(executionBinding.repositoryRef, "repositoryRef"),
    providerLeaseRef: continuation.providerLeaseRef,
    turns: continuation.turns.map((turn, index) => ({ ...turn, ...continuation.expectedThreadCursors[index]! })),
  })
  if (completed.length !== continuation.turns.length || completed.some((row, index) =>
    row.agentRef !== continuation.turns[index]?.agentRef || row.turnRef !== continuation.turns[index]?.turnRef)) {
    throw new PortableSessionControlError("guest runtime continuation receipt differs from planned turns")
  }
  const cursors = completed.map((row, index) => ({
    agentRef: row.agentRef,
    threadRef: asString(graphNodes[index]?.threadRef, "threadRef"),
    activityCursor: row.activityCursor,
    eventCursor: row.eventCursor,
  }))
  const response = publicSafe({
    acceptedWorkRefs: completed.map(({ agentRef, turnRef }) => ({ agentRef, turnRef })),
    threadCursors: cursors,
    evidenceRefs: [stableRef("evidence.agent-computer.continuation", continuation.operationRef)],
    replay: "executed" as const,
    material: "excluded" as const,
  })
  const nextGraph = {
    ...asObject(state.bundle.graph, "graph"),
    nodes: graphNodes.map((node, index) => ({ ...node, activityCursor: cursors[index]!.activityCursor })),
  }
  const nextBundle = { ...state.bundle, graph: nextGraph, threadCursors: cursors.map(({ agentRef: _, ...cursor }) => cursor) }
  await writeState(sessionRoot, {
    ...state,
    bundle: nextBundle,
    operations: { ...state.operations, [continuation.operationRef]: { fingerprint, response } },
  })
  return response
}

export const executePortableSessionControl = async (input: Readonly<{
  operation: unknown
  stateRoot: string
  runtime: PortableSessionGuestRuntime
}>): Promise<unknown> => {
  const operation = validateOperation(input.operation)
  const sessionRoot = portableSessionRoot(input.stateRoot, operation.sessionRef)
  let state = await readState(sessionRoot)
  const fingerprint = digest(operation)
  const replay = state?.operations[operation.operationRef]
  if (replay !== undefined) {
    if (replay.fingerprint !== fingerprint) throw new PortableSessionControlError("operation bytes conflict")
    if (operation.action === "activate") {
      await input.runtime.verifyActivation({
        sessionRoot,
        destinationRunnerSessionReservationRef: asString(
          operation.payload.destinationRunnerSessionReservationRef,
          "destinationRunnerSessionReservationRef",
        ),
        generation: operation.generation,
      })
    }
    return replay.response
  }

  let response: unknown
  if (operation.action === "stage") {
    if (state !== undefined) throw new PortableSessionControlError("a retained session already exists")
    const bundle = asObject(operation.payload.bundle, "payload.bundle")
    const checkpoint = asObject(bundle.checkpoint, "bundle.checkpoint")
    if (checkpoint.sessionRef !== operation.sessionRef || Number(checkpoint.sourceGeneration) + 1 !== operation.generation) {
      throw new PortableSessionControlError("checkpoint session or destination generation is invalid")
    }
    await input.runtime.verifyStage({ sessionRoot, bundle })
    response = publicSafe({
      checkpointDigest: checkpoint.digest,
      repositoryPostImageDigest: checkpoint.repositoryPostImageDigest,
      diffDigest: checkpoint.diffDigest,
      graphDigest: checkpoint.graphDigest,
      threadCursors: bundle.threadCursors,
      acceptingWork: false as const,
      evidenceRefs: [stableRef("evidence.agent-computer.stage", operation.operationRef)],
    })
    state = {
      schema: "openagents.portable_agent_computer_guest_state.v1",
      ownerRef: operation.ownerRef,
      targetRef: operation.targetRef,
      sessionRef: operation.sessionRef,
      attachmentRef: operation.attachmentRef,
      generation: operation.generation,
      state: "staged",
      bundle,
      operations: {},
    }
  } else {
    if (state === undefined) throw new PortableSessionControlError("retained session is missing")
    assertScope(state, operation)
    const graph = asObject(state.bundle.graph, "retained graph")
    const agentRefs = graphAgentRefs(graph)
    if (operation.action === "activate") {
      if (state.state !== "staged") throw new PortableSessionControlError("activation requires stage")
      const checkpointRef = asString(operation.payload.checkpointRef, "checkpointRef")
      const authorityEvidenceRef = asString(operation.payload.authorityEvidenceRef, "authorityEvidenceRef")
      const authenticationPolicyRef = asString(operation.payload.authenticationPolicyRef, "authenticationPolicyRef")
      const destinationRunnerSessionReservationRef = asString(
        operation.payload.destinationRunnerSessionReservationRef,
        "destinationRunnerSessionReservationRef",
      )
      const authenticationObservedAt = asString(operation.payload.helpersObservedAt, "helpersObservedAt")
      if (!Number.isFinite(new Date(authenticationObservedAt).valueOf())) {
        throw new PortableSessionControlError("helpersObservedAt must be an ISO timestamp")
      }
      if (!Array.isArray(operation.payload.capabilityLeaseRefs) || operation.payload.capabilityLeaseRefs.length === 0) {
        throw new PortableSessionControlError("activation requires installed capability lease refs")
      }
      const leaseRefs = operation.payload.capabilityLeaseRefs.map((value, index) => asString(value, `capabilityLeaseRefs[${index}]`))
      if (new Set(leaseRefs).size !== leaseRefs.length) {
        throw new PortableSessionControlError("activation capability lease refs must be unique")
      }
      await input.runtime.verifyCapabilities({ sessionRoot, leaseRefs })
      const helpers = await input.runtime.activate({
        sessionRoot,
        agentRefs,
        destinationRunnerSessionReservationRef,
        generation: operation.generation,
      })
      try {
        validateHelperReadiness(helpers)
      } catch (error) {
        await input.runtime.quiesce({ sessionRoot, agentRefs }).catch(() => undefined)
        throw error
      }
      const helpersObservedAt = new Date().toISOString()
      const activationEvidenceRef = stableRef("evidence.agent-computer.activate", operation.operationRef)
      response = {
        schema: "openagents.ide_portable_destination_activation.v1",
        receiptRef: stableRef("receipt.agent-computer.destination-activation", operation.operationRef),
        operationRef: operation.operationRef,
        sessionRef: operation.sessionRef,
        checkpointRef,
        destinationTargetRef: operation.targetRef,
        destinationAttachmentRef: operation.attachmentRef,
        destinationRunnerSessionReservationRef,
        destinationGeneration: operation.generation,
        authentication: {
          state: "reauthenticated",
          policyRef: authenticationPolicyRef,
          evidenceRef: authorityEvidenceRef,
          observedAt: authenticationObservedAt,
          expiresAt: null,
        },
        helpersObservedAt,
        helpers,
        activatedAgentRefs: agentRefs,
        acceptedWorkRefs: [],
        evidenceRefs: [
          ...new Set([
            authorityEvidenceRef,
            activationEvidenceRef,
            ...helpers.flatMap(helper => helper.evidenceRefs),
          ]),
        ],
      }
      state = { ...state, state: "active" }
    } else if (operation.action === "quiesce") {
      if (state.state !== "active") throw new PortableSessionControlError("quiescence requires active state")
      await input.runtime.quiesce({ sessionRoot, agentRefs })
      response = {
        quiescedAgentRefs: agentRefs,
        evidenceRefs: [stableRef("evidence.agent-computer.quiesce", operation.operationRef)],
      }
      state = { ...state, state: "quiesced" }
    } else if (operation.action === "checkpoint") {
      if (state.state !== "quiesced") throw new PortableSessionControlError("checkpoint requires quiescence")
      const checkpoint = { ...asObject(state.bundle.checkpoint, "retained checkpoint") }
      const snapshot = await input.runtime.repositorySnapshot(sessionRoot)
      const next = {
        ...checkpoint,
        checkpointRef: asString(operation.payload.checkpointRef, "checkpointRef"),
        sourceAttachmentRef: operation.attachmentRef,
        sourceGeneration: operation.generation,
        eventLogCursor: Number(operation.payload.eventLogCursor),
        ...snapshot,
      }
      delete next.digest
      response = {
        checkpoint: { ...next, digest: digest(next) },
        executionBinding: operation.payload.executionBinding,
        graph: operation.payload.graph,
        threadCursors: operation.payload.threadCursors,
      }
    } else if (operation.action === "wipeCapability") {
      const leaseRef = asString(operation.payload.leaseRef, "leaseRef")
      asString(operation.payload.installationRef, "installationRef")
      const leaf = createHash("sha256").update(leaseRef).digest("hex").slice(0, 24)
      await rm(join(sessionRoot, "capability-material", `${leaf}.material`), { force: true })
      await rm(join(sessionRoot, "capabilities", `${leaf}.installed.json`), { force: true })
      response = {
        wipeReceiptRef: stableRef("receipt.agent-computer.capability-wipe", operation.operationRef),
        material: "excluded",
      }
    } else if (operation.action === "abort" || operation.action === "reclaim") {
      if (operation.action === "abort" && state.state !== "staged") throw new PortableSessionControlError("abort requires stage")
      if (operation.action === "reclaim" && state.state !== "quiesced") throw new PortableSessionControlError("reclaim requires quiescence")
      await input.runtime.reclaim({ sessionRoot, agentRefs })
      response = operation.action === "abort"
        ? { evidenceRefs: [stableRef("evidence.agent-computer.abort", operation.operationRef)] }
        : {
            cleanedAgentRefs: agentRefs,
            processes: "released",
            scratch: "released",
            ports: "released",
            evidenceRefs: [stableRef("evidence.agent-computer.reclaim", operation.operationRef)],
          }
      state = { ...state, state: "reclaimed" }
    } else {
      throw new PortableSessionControlError("unsupported action")
    }
  }

  const nextState: ControllerState = {
    ...state,
    operations: {
      ...state.operations,
      [operation.operationRef]: { fingerprint, response: publicSafe(response) },
    },
  }
  await writeState(sessionRoot, nextState)
  return response
}

type CapabilityInstallMetadata = Readonly<{
  operationRef: string
  ownerRef: string
  targetRef: string
  resourceRef: string
  sessionRef: string
  attachmentRef: string
  generation: number
  leaseRef: string
  evidenceRef: string
  capability: string
}>

export const installPortableCapability = async (input: Readonly<{
  metadata: unknown
  material: Uint8Array
  stateRoot: string
}>): Promise<unknown> => {
  const metadata = publicSafe(asObject(input.metadata, "capability install metadata")) as unknown as CapabilityInstallMetadata
  for (const field of ["operationRef", "ownerRef", "targetRef", "resourceRef", "sessionRef", "attachmentRef", "leaseRef", "evidenceRef", "capability"] as const) {
    asString(metadata[field], field)
  }
  if (!Number.isSafeInteger(metadata.generation) || metadata.generation < 1) {
    throw new PortableSessionControlError("generation must be positive")
  }
  if (input.material.byteLength === 0 || input.material.byteLength > 128 * 1024) {
    throw new PortableSessionControlError("capability material length is invalid")
  }
  const sessionRoot = portableSessionRoot(input.stateRoot, metadata.sessionRef)
  const state = await readState(sessionRoot)
  if (state === undefined) throw new PortableSessionControlError("retained session is missing")
  if (state.ownerRef !== metadata.ownerRef || state.targetRef !== metadata.targetRef ||
      state.sessionRef !== metadata.sessionRef || state.attachmentRef !== metadata.attachmentRef ||
      state.generation !== metadata.generation) {
    throw new PortableSessionControlError("capability install scope differs from retained session")
  }
  const leaf = createHash("sha256").update(metadata.leaseRef).digest("hex").slice(0, 24)
  const markerPath = join(sessionRoot, "capabilities", `${leaf}.installed.json`)
  const installationRef = stableRef("installation.agent-computer.capability", `${metadata.resourceRef}|${metadata.leaseRef}`)
  if (await Runtime.file(markerPath).exists()) {
    const marker = publicSafe(JSON.parse(await readFile(markerPath, "utf8"))) as Record<string, unknown>
    if (marker.leaseRef !== metadata.leaseRef || marker.evidenceRef !== metadata.evidenceRef) {
      throw new PortableSessionControlError("capability marker conflicts with install retry")
    }
    return { installationRef, evidenceRef: metadata.evidenceRef, marker, material: "excluded" }
  }
  const materialDir = join(sessionRoot, "capability-material")
  const markerDir = join(sessionRoot, "capabilities")
  await mkdir(materialDir, { recursive: true, mode: 0o700 })
  await mkdir(markerDir, { recursive: true, mode: 0o700 })
  const materialPath = join(materialDir, `${leaf}.material`)
  const temporary = `${materialPath}.tmp-${process.pid}`
  await writeFile(temporary, input.material, { mode: 0o600 })
  await rename(temporary, materialPath)
  const marker = { leaseRef: metadata.leaseRef, evidenceRef: metadata.evidenceRef }
  const markerTemporary = `${markerPath}.tmp-${process.pid}`
  await writeFile(markerTemporary, canonicalJson(marker), { mode: 0o600 })
  await rename(markerTemporary, markerPath)
  return { installationRef, evidenceRef: metadata.evidenceRef, marker, material: "excluded" }
}

const SAFE_ARCHIVE_EXTRACT = String.raw`
import os,sys,tarfile
source,destination=sys.argv[1],sys.argv[2]
with tarfile.open(source,'r:') as archive:
  total=0
  for member in archive.getmembers():
    name=member.name.rstrip('/')
    parts=name.split('/')
    if name.startswith('/') or '\\' in name or any(part in ('','.','..') for part in parts):
      raise SystemExit('unsafe archive path')
    if not (name=='manifest.json' or name=='repository.bundle' or name=='post-image' or name.startswith('post-image/')):
      raise SystemExit('unexpected archive entry')
    if member.islnk() or member.isdev():
      raise SystemExit('archive hard links and devices are forbidden')
    if member.issym():
      target=member.linkname
      if not name.startswith('post-image/') or not target or target.startswith('/') or '\\' in target:
        raise SystemExit('unsafe archive symbolic link')
      target_parts=target.split('/')
      if any(part in ('','.','..') for part in target_parts):
        raise SystemExit('unsafe archive symbolic link')
      resolved=os.path.normpath(os.path.join(os.path.dirname(name),target))
      if not resolved.startswith('post-image/'):
        raise SystemExit('archive symbolic link escapes post-image')
    elif not (member.isdir() or member.isfile()):
      raise SystemExit('unsupported archive entry')
    total += member.size
    if total > 134217728:
      raise SystemExit('archive content too large')
  archive.extractall(destination)
`

const safeRelativePath = (value: unknown): string => {
  if (typeof value !== "string" || value.length === 0 || value.startsWith("/") || value.includes("\\")) {
    throw new PortableSessionControlError("checkpoint file path is invalid")
  }
  const parts = value.split("/")
  if (parts.some(part => part === "" || part === "." || part === ".." || part === ".git")) {
    throw new PortableSessionControlError("checkpoint file path escapes the repository")
  }
  return value
}

const safeRelativeLinkTarget = (path: string, value: unknown): string => {
  if (typeof value !== "string" || value.length === 0 || value.startsWith("/") || value.includes("\\")) {
    throw new PortableSessionControlError("checkpoint symbolic link target is invalid")
  }
  const parts = value.split("/")
  if (parts.some(part => part === "" || part === "." || part === "..")) {
    throw new PortableSessionControlError("checkpoint symbolic link target is invalid")
  }
  const resolved = posix.normalize(posix.join(posix.dirname(path), value))
  if (resolved === ".." || resolved.startsWith("../") || posix.isAbsolute(resolved)) {
    throw new PortableSessionControlError("checkpoint symbolic link escapes the repository")
  }
  return value
}

const removeWorkspacePostImage = async (root: string): Promise<void> => {
  for (const entry of await readdir(root, { withFileTypes: true })) {
    if (entry.name === ".git") continue
    await rm(join(root, entry.name), { recursive: true, force: true })
  }
}

const listPortableEntries = async (root: string, prefix = ""): Promise<ReadonlyArray<string>> => {
  const files: string[] = []
  for (const entry of await readdir(join(root, prefix), { withFileTypes: true })) {
    const relative = prefix === "" ? entry.name : `${prefix}/${entry.name}`
    if (entry.isDirectory()) files.push(...await listPortableEntries(root, relative))
    else if (entry.isFile() || entry.isSymbolicLink()) files.push(relative)
    else throw new PortableSessionControlError("checkpoint post-image contains an unsupported entry")
  }
  return files.sort()
}

export const materializePortableCheckpoint = async (input: Readonly<{
  metadata: unknown
  archive: Uint8Array
  stateRoot: string
  runtime: PortableSessionGuestRuntime
  zstdBin?: string
}>): Promise<unknown> => {
  const metadata = publicSafe(asObject(input.metadata, "checkpoint metadata"))
  const artifactRef = asString(metadata.artifactRef, "artifactRef")
  const artifactDigest = asString(metadata.artifactDigest, "artifactDigest")
  const operation = validateOperation(metadata.stageOperation)
  if (operation.action !== "stage") throw new PortableSessionControlError("checkpoint metadata requires stage")
  if (`sha256:${createHash("sha256").update(input.archive).digest("hex")}` !== artifactDigest) {
    throw new PortableSessionControlError("checkpoint archive digest differs")
  }
  const sessionRoot = portableSessionRoot(input.stateRoot, operation.sessionRef)
  const work = join(sessionRoot, `.materialize-${createHash("sha256").update(artifactRef).digest("hex").slice(0, 16)}`)
  const archivePath = join(work, "checkpoint.tar.zst")
  const tarPath = join(work, "checkpoint.tar")
  const extracted = join(work, "extracted")
  try {
    await rm(work, { recursive: true, force: true })
    await mkdir(extracted, { recursive: true, mode: 0o700 })
    await writeFile(archivePath, input.archive, { mode: 0o600 })
    await run([input.zstdBin ?? "/usr/bin/zstd", "-q", "-d", "-f", archivePath, "-o", tarPath])
    await run(["/usr/bin/python3", "-c", SAFE_ARCHIVE_EXTRACT, tarPath, extracted])
    const manifest = publicSafe(JSON.parse(await readFile(join(extracted, "manifest.json"), "utf8"))) as Record<string, unknown>
    if (manifest.schema !== "openagents.portable_checkpoint_artifact.v1" || manifest.artifactRef !== artifactRef ||
        manifest.checkpointRef !== asObject(asObject(operation.payload.bundle, "bundle").checkpoint, "checkpoint").checkpointRef ||
        canonicalJson(manifest.bundle) !== canonicalJson(operation.payload.bundle)) {
      throw new PortableSessionControlError("checkpoint manifest does not bind the staged bundle")
    }
    if (!Array.isArray(manifest.files)) throw new PortableSessionControlError("checkpoint manifest files are missing")
    const declared = manifest.files.map((entry, index) => {
      const file = asObject(entry, `files[${index}]`)
      const path = safeRelativePath(file.path)
      const mode = Number(file.mode)
      const size = Number(file.size)
      if (![0o644, 0o755, 0o120000].includes(mode) || !Number.isSafeInteger(size) || size < 0 ||
          typeof file.sha256 !== "string" || !/^sha256:[a-f0-9]{64}$/u.test(file.sha256)) {
        throw new PortableSessionControlError("checkpoint file metadata is invalid")
      }
      const linkTarget = mode === 0o120000
        ? safeRelativeLinkTarget(path, file.linkTarget)
        : undefined
      if (mode !== 0o120000 && file.linkTarget !== undefined) {
        throw new PortableSessionControlError("regular checkpoint file declares a symbolic link target")
      }
      return { path, mode, size, sha256: file.sha256, linkTarget }
    })
    const paths = declared.map(file => file.path)
    if (new Set(paths).size !== paths.length || [...paths].sort().join("\0") !== paths.join("\0")) {
      throw new PortableSessionControlError("checkpoint file inventory is not unique and sorted")
    }
    if ((await listPortableEntries(join(extracted, "post-image"))).join("\0") !== paths.join("\0")) {
      throw new PortableSessionControlError("checkpoint post-image inventory differs from manifest")
    }
    for (const file of declared) {
      const source = join(extracted, "post-image", file.path)
      const info = await lstat(source)
      const bytes = file.mode === 0o120000
        ? new TextEncoder().encode(await readlink(source))
        : await readFile(source)
      if ((file.mode === 0o120000) !== info.isSymbolicLink() ||
          (file.mode !== 0o120000 && !info.isFile()) ||
          (file.linkTarget !== undefined && new TextDecoder().decode(bytes) !== file.linkTarget)) {
        throw new PortableSessionControlError("checkpoint post-image entry type differs from manifest")
      }
      if (bytes.byteLength !== file.size || `sha256:${createHash("sha256").update(bytes).digest("hex")}` !== file.sha256) {
        throw new PortableSessionControlError("checkpoint post-image file differs from manifest")
      }
    }
    const workspace = workspacePath(sessionRoot)
    await rm(workspace, { recursive: true, force: true })
    await mkdir(workspace, { recursive: true, mode: 0o700 })
    const checkpoint = asObject(asObject(operation.payload.bundle, "bundle").checkpoint, "checkpoint")
    const revision = asString(checkpoint.repositoryRevisionRef, "repositoryRevisionRef")
    await run(["/usr/bin/git", "-C", workspace, "init", "--quiet"])
    await run(["/usr/bin/git", "-C", workspace, "fetch", "--quiet", "--no-tags", join(extracted, "repository.bundle"), revision])
    await run(["/usr/bin/git", "-C", workspace, "checkout", "--quiet", "--detach", revision])
    await removeWorkspacePostImage(workspace)
    for (const file of declared) {
      const destination = join(workspace, file.path)
      await mkdir(dirname(destination), { recursive: true, mode: 0o700 })
      if (file.mode === 0o120000) {
        await symlink(file.linkTarget!, destination)
      } else {
        await writeFile(destination, await readFile(join(extracted, "post-image", file.path)), { mode: file.mode })
        await chmod(destination, file.mode)
      }
    }
    await rm(join(workspace, ".git", "hooks"), { recursive: true, force: true })
    await rm(join(workspace, ".git", "logs"), { recursive: true, force: true })
    await input.runtime.prepare({
      sessionRoot,
      agentRefs: graphAgentRefs(asObject(operation.payload.bundle, "bundle").graph),
    })
    return executePortableSessionControl({ operation, stateRoot: input.stateRoot, runtime: input.runtime })
  } finally {
    await rm(work, { recursive: true, force: true })
  }
}

export const exportPortableCheckpoint = async (input: Readonly<{
  metadata: unknown
  stateRoot: string
  zstdBin?: string
  tarBin?: string
}>): Promise<unknown> => {
  const metadata = publicSafe(asObject(input.metadata, "checkpoint export metadata"))
  for (const field of ["operationRef", "ownerRef", "targetRef", "sessionRef", "attachmentRef", "checkpointRef"] as const) {
    asString(metadata[field], field)
  }
  const generation = Number(metadata.generation)
  if (!Number.isSafeInteger(generation) || generation < 1) throw new PortableSessionControlError("generation must be positive")
  const sessionRoot = portableSessionRoot(input.stateRoot, String(metadata.sessionRef))
  const state = await readState(sessionRoot)
  if (state === undefined || state.state !== "quiesced" || state.ownerRef !== metadata.ownerRef ||
      state.targetRef !== metadata.targetRef || state.attachmentRef !== metadata.attachmentRef ||
      state.generation !== generation) {
    throw new PortableSessionControlError("checkpoint export requires exact quiesced retained scope")
  }
  const checkpointRef = String(metadata.checkpointRef)
  const checkpointBundle = Object.values(state.operations).map(record => record.response)
    .find(response => {
      if (response === null || typeof response !== "object" || Array.isArray(response)) return false
      const checkpoint = (response as Record<string, unknown>).checkpoint
      return checkpoint !== null && typeof checkpoint === "object" && !Array.isArray(checkpoint) &&
        (checkpoint as Record<string, unknown>).checkpointRef === checkpointRef
    }) as Record<string, unknown> | undefined
  if (checkpointBundle === undefined) throw new PortableSessionControlError("checkpoint export receipt is unavailable")
  const operationRef = String(metadata.operationRef)
  const exportRoot = join(sessionRoot, "exports", createHash("sha256").update(operationRef).digest("hex").slice(0, 24))
  const receiptPath = join(exportRoot, "receipt.json")
  if (await Runtime.file(receiptPath).exists()) return publicSafe(JSON.parse(await readFile(receiptPath, "utf8")))

  const work = join(exportRoot, "work")
  const postImage = join(work, "post-image")
  await rm(exportRoot, { recursive: true, force: true })
  await mkdir(postImage, { recursive: true, mode: 0o700 })
  const workspace = workspacePath(sessionRoot)
  const listed = new TextDecoder().decode(await git(workspace, ["ls-files", "-co", "--exclude-standard", "-z"]))
    .split("\0").filter(Boolean).sort()
  const files: Array<Record<string, unknown>> = []
  for (const relativePath of listed) {
    const safePath = safeRelativePath(relativePath)
    const source = join(workspace, safePath)
    const destination = join(postImage, safePath)
    const info = await lstat(source)
    await mkdir(dirname(destination), { recursive: true, mode: 0o700 })
    if (info.isSymbolicLink()) {
      const linkTarget = safeRelativeLinkTarget(safePath, await readlink(source))
      const bytes = new TextEncoder().encode(linkTarget)
      await symlink(linkTarget, destination)
      files.push({ path: safePath, mode: 0o120000, sha256: `sha256:${createHash("sha256").update(bytes).digest("hex")}`, size: bytes.byteLength, linkTarget })
    } else if (info.isFile()) {
      const bytes = await readFile(source)
      const mode = (info.mode & 0o111) === 0 ? 0o644 : 0o755
      await writeFile(destination, bytes, { mode })
      files.push({ path: safePath, mode, sha256: `sha256:${createHash("sha256").update(bytes).digest("hex")}`, size: bytes.byteLength })
    } else {
      throw new PortableSessionControlError("checkpoint workspace contains unsupported entry")
    }
  }
  const artifactRef = stableRef("artifact.portable-checkpoint", operationRef)
  const bundle = {
    checkpoint: checkpointBundle.checkpoint,
    executionBinding: checkpointBundle.executionBinding,
    graph: checkpointBundle.graph,
    threadCursors: checkpointBundle.threadCursors,
  }
  await writeFile(join(work, "manifest.json"), canonicalJson({
    schema: "openagents.portable_checkpoint_artifact.v1",
    artifactRef,
    checkpointRef,
    bundle,
    files,
  }), { mode: 0o600 })
  await run(["/usr/bin/git", "-C", workspace, "bundle", "create", join(work, "repository.bundle"), "HEAD"])
  const tarPath = join(exportRoot, "checkpoint.tar")
  const archivePath = join(exportRoot, "checkpoint.tar.zst")
  await run([input.tarBin ?? "/usr/bin/tar", "-C", work, "-cf", tarPath, "manifest.json", "repository.bundle", "post-image"])
  await run([input.zstdBin ?? "/usr/bin/zstd", "-q", "-f", tarPath, "-o", archivePath])
  await rm(tarPath, { force: true })
  await rm(work, { recursive: true, force: true })
  const archive = await readFile(archivePath)
  const receipt = publicSafe({
    artifactRef,
    artifactDigest: `sha256:${createHash("sha256").update(archive).digest("hex")}`,
    checkpointRef,
    material: "excluded" as const,
  })
  await writeFile(receiptPath, canonicalJson(receipt), { mode: 0o600 })
  return receipt
}

const run = async (command: ReadonlyArray<string>): Promise<string> => {
  const child = Runtime.spawn(command, { stdout: "pipe", stderr: "pipe", env: { PATH: "/usr/local/bin:/usr/bin:/bin" } })
  const [exitCode, stdout] = await Promise.all([child.exited, new Response(child.stdout).text()])
  if (exitCode !== 0) throw new PortableSessionControlError(`fixed guest runtime command failed (${command[0]})`)
  return stdout
}

const lifecycleState = async (stateDir: string): Promise<string> => {
  const output = JSON.parse(await run([
    "/usr/local/bin/oa-workroomd", "lifecycle", "status", "--state-dir", stateDir, "--json",
  ])) as { state?: unknown }
  if (typeof output.state !== "string") throw new PortableSessionControlError("workroom lifecycle status is invalid")
  return output.state
}

const transitionLifecycle = async (
  stateDir: string,
  desired: "running" | "paused",
): Promise<void> => {
  const current = await lifecycleState(stateDir)
  if (desired === "running" && ["running", "exposed"].includes(current)) return
  if (desired === "paused" && current === "paused") return
  const action = desired === "running"
    ? current === "created" ? "start" : current === "paused" ? "resume" : undefined
    : ["running", "exposed"].includes(current) ? "pause" : undefined
  if (action === undefined) throw new PortableSessionControlError("workroom lifecycle cannot reach requested state")
  await run([
    "/usr/local/bin/oa-workroomd", "lifecycle", action, "--state-dir", stateDir, "--json",
  ])
}

const repositoryEntryBytes = async (cwd: string, relativePath: string): Promise<Uint8Array> => {
  const path = join(cwd, relativePath)
  const info = await lstat(path)
  return info.isSymbolicLink() ? new TextEncoder().encode(await readlink(path)) : readFile(path)
}

const git = async (cwd: string, args: ReadonlyArray<string>): Promise<Uint8Array> => {
  const child = Runtime.spawn(["/usr/bin/git", "-C", cwd, ...args], {
    stdout: "pipe", stderr: "pipe", env: { PATH: "/usr/local/bin:/usr/bin:/bin" },
  })
  const [exitCode, bytes] = await Promise.all([child.exited, new Response(child.stdout).bytes()])
  if (exitCode !== 0) throw new PortableSessionControlError("repository verification failed")
  return bytes
}

export const repositorySnapshot = async (sessionRoot: string) => {
  const cwd = workspacePath(sessionRoot)
  const revision = new TextDecoder().decode(await git(cwd, ["rev-parse", "HEAD"])).trim()
  if (!/^[a-f0-9]{40,64}$/u.test(revision)) throw new PortableSessionControlError("repository revision is not pinned")
  const listed = new TextDecoder().decode(await git(cwd, ["ls-files", "-co", "--exclude-standard", "-z"]))
    .split("\0").filter(Boolean).sort()
  const postImage = createHash("sha256")
  for (const relativePath of listed) postImage.update(relativePath).update("\0").update(await repositoryEntryBytes(cwd, relativePath)).update("\0")
  const diffHash = createHash("sha256").update(await git(cwd, ["diff", "--binary", "HEAD", "--"]))
  const untracked = new TextDecoder().decode(await git(cwd, ["ls-files", "--others", "--exclude-standard", "-z"]))
    .split("\0").filter(Boolean).sort()
  for (const relativePath of untracked) diffHash.update(relativePath).update("\0").update(await repositoryEntryBytes(cwd, relativePath)).update("\0")
  return {
    repositoryRevisionRef: revision,
    repositoryPostImageDigest: `sha256:${postImage.digest("hex")}`,
    diffDigest: `sha256:${diffHash.digest("hex")}`,
  }
}

const PORTABLE_HELPER_STATE_SCHEMA = "openagents.portable_guest_helper_state.v2" as const
const TYPESCRIPT_LANGUAGE_SERVER_VERSION = "5.3.0"
const TYPESCRIPT_VERSION = "5.9.3"
const LSP_COMMAND = [
  "/usr/local/bin/node",
  "/opt/agent/typescript-lsp/node_modules/typescript-language-server/lib/cli.mjs",
  "--stdio",
] as const
const WATCHER_PROGRAM = String.raw`
const { watch, writeFileSync } = require("node:fs");
const [workspace, readyPath, instanceRef] = process.argv.slice(1);
if (!workspace || !readyPath || !instanceRef) process.exit(64);
let watcher;
try {
  watcher = watch(workspace, { recursive: true, persistent: true }, () => undefined);
  watcher.on("error", () => process.exit(70));
  writeFileSync(readyPath, instanceRef, { mode: 0o600 });
} catch {
  process.exit(70);
}
const stop = () => { watcher.close(); process.exit(0); };
process.on("SIGTERM", stop);
process.on("SIGINT", stop);
`

const LSP_HELPER_PROGRAM = String.raw`
const { spawn } = require("node:child_process");
const { writeFileSync } = require("node:fs");
const { pathToFileURL } = require("node:url");
const [workspace, readyPath, instanceRef, expectedVersion, commandJson, environmentJson] = process.argv.slice(1);
const command = JSON.parse(commandJson);
const environment = JSON.parse(environmentJson);
if (!workspace || !readyPath || !instanceRef || !expectedVersion || !Array.isArray(command) || command.length === 0) process.exit(64);
const child = spawn(command[0], command.slice(1), { stdio: ["pipe", "pipe", "pipe"], env: environment });
let buffer = Buffer.alloc(0);
let initialized = false;
let versionObserved = false;
let desiredExitCode = null;
let childExitObserved = false;
const childStopped = () => {
  if (childExitObserved) return;
  childExitObserved = true;
  process.exit(desiredExitCode === null ? 70 : desiredExitCode);
};
child.once("exit", childStopped);
child.once("close", childStopped);
const terminateChild = (exitCode, gracefulMessage) => {
  if (desiredExitCode !== null) return;
  desiredExitCode = exitCode;
  if (gracefulMessage) {
    try { send(gracefulMessage); } catch { /* termination still continues */ }
  }
  const timer = setTimeout(() => {
    if (child.pid && !child.killed) child.kill("SIGKILL");
  }, 500);
  timer.unref();
  if (!gracefulMessage && child.pid && !child.killed) child.kill("SIGKILL");
};
const send = message => {
  const body = Buffer.from(JSON.stringify(message));
  child.stdin.write(Buffer.concat([Buffer.from("Content-Length: " + body.length + "\r\n\r\n"), body]));
};
const ready = () => {
  if (initialized && versionObserved) writeFileSync(readyPath, instanceRef + "|" + expectedVersion, { mode: 0o600 });
};
const onMessage = message => {
  if (message.id === 1 && message.result) {
    initialized = true;
    send({ jsonrpc: "2.0", method: "initialized", params: {} });
    ready();
  }
  if (message.id === 2) send({ jsonrpc: "2.0", method: "exit", params: null });
  if (message.method === "$/typescriptVersion" && message.params && message.params.version === expectedVersion) {
    versionObserved = true;
    ready();
  }
};
child.stdout.on("data", chunk => {
  buffer = Buffer.concat([buffer, chunk]);
  while (true) {
    const headerEnd = buffer.indexOf("\r\n\r\n");
    if (headerEnd < 0) return;
    const match = /(?:^|\r\n)Content-Length:\s*(\d+)/i.exec(buffer.subarray(0, headerEnd).toString("ascii"));
    if (!match) { terminateChild(70); return; }
    const length = Number(match[1]);
    if (buffer.length < headerEnd + 4 + length) return;
    const body = buffer.subarray(headerEnd + 4, headerEnd + 4 + length);
    buffer = buffer.subarray(headerEnd + 4 + length);
    try { onMessage(JSON.parse(body.toString("utf8"))); } catch { terminateChild(70); return; }
  }
});
child.stderr.resume();
child.on("error", () => terminateChild(70));
child.stdin.on("error", () => terminateChild(70));
child.stdout.on("error", () => terminateChild(70));
send({ jsonrpc: "2.0", id: 1, method: "initialize", params: {
  processId: null,
  rootUri: pathToFileURL(workspace).href,
  capabilities: {},
  initializationOptions: { tsserver: { path: "/opt/agent/typescript-lsp/node_modules/typescript/lib" } },
  workspaceFolders: [{ uri: pathToFileURL(workspace).href, name: "workspace" }],
}});
const stop = () => {
  terminateChild(0, { jsonrpc: "2.0", id: 2, method: "shutdown", params: null });
};
process.on("SIGTERM", stop);
process.on("SIGINT", stop);
`

type ManagedHelperProcessState = Readonly<{
  pid: number
  instanceRef: string
  versionRef: string
  evidenceRefs: ReadonlyArray<string>
}>

type PortableGuestHelperState = Readonly<{
  schema: typeof PORTABLE_HELPER_STATE_SCHEMA
  destinationRunnerSessionReservationRef: string
  generation: number
  lsp: ManagedHelperProcessState
  watcher: ManagedHelperProcessState
}>

const helperStatePath = (sessionRoot: string): string => join(sessionRoot, "portable-helpers.json")
const helperReadyPath = (sessionRoot: string, instanceRef: string): string =>
  join(sessionRoot, `helper-${createHash("sha256").update(instanceRef).digest("hex").slice(0, 24)}.ready`)

const processIsLive = (pid: number): boolean => {
  if (!Number.isSafeInteger(pid) || pid <= 1) return false
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

const helperProcessIsLive = async (state: ManagedHelperProcessState): Promise<boolean> => {
  if (!processIsLive(state.pid)) return false
  if (!(await Runtime.file("/proc").exists())) return true
  const commandLine = await readFile(`/proc/${state.pid}/cmdline`, "utf8").catch(() => "")
  return commandLine.split("\0").includes(state.instanceRef)
}

const terminateManagedHelper = async (helper: ManagedHelperProcessState): Promise<void> => {
  if (!(await helperProcessIsLive(helper))) return
  try { process.kill(helper.pid, "SIGTERM") } catch { return }
  for (let attempt = 0; attempt < 100 && await helperProcessIsLive(helper); attempt += 1) {
    await new Promise(resolve => setTimeout(resolve, 10))
  }
  if (await helperProcessIsLive(helper)) {
    try { process.kill(helper.pid, "SIGKILL") } catch { /* already exited */ }
    for (let attempt = 0; attempt < 50 && await helperProcessIsLive(helper); attempt += 1) {
      await new Promise(resolve => setTimeout(resolve, 10))
    }
    if (await helperProcessIsLive(helper)) {
      throw new PortableSessionControlError("portable helper remained live after forced termination")
    }
  }
}

const readHelperState = async (sessionRoot: string): Promise<PortableGuestHelperState | undefined> => {
  try {
    return JSON.parse(await readFile(helperStatePath(sessionRoot), "utf8")) as PortableGuestHelperState
  } catch (error) {
    if (error instanceof Error && "code" in error && error.code === "ENOENT") return undefined
    throw error
  }
}

const assertHelperState = (
  state: PortableGuestHelperState,
  destinationRunnerSessionReservationRef: string,
  generation: number,
): void => {
  if (state.schema !== PORTABLE_HELPER_STATE_SCHEMA ||
      state.destinationRunnerSessionReservationRef !== destinationRunnerSessionReservationRef ||
      state.generation !== generation ||
      ([state.lsp, state.watcher] as const).some(helper =>
        !SAFE_REF.test(helper.instanceRef) || !SAFE_REF.test(helper.versionRef) ||
        helper.evidenceRefs.length === 0 || helper.evidenceRefs.some(ref => !SAFE_REF.test(ref)) ||
        !Number.isSafeInteger(helper.pid) || helper.pid <= 1)) {
    throw new PortableSessionControlError("portable helper state differs from the retained generation")
  }
}

const helperReadiness = (state: PortableGuestHelperState): ReadonlyArray<PortableGuestHelperReadiness> =>
  (["pty", "lsp", "dap", "watcher", "native"] as const).map(kind => kind === "lsp" || kind === "watcher"
    ? {
        kind,
        readiness: "ready",
        instanceRef: state[kind].instanceRef,
        versionRef: state[kind].versionRef,
        omissionRef: null,
        evidenceRefs: state[kind].evidenceRefs,
      }
    : {
        kind,
        readiness: "unsupported",
        instanceRef: null,
        versionRef: null,
        omissionRef: `omission.agent-computer.portable.${kind}.image_adapter_unavailable`,
        evidenceRefs: [],
      })

export const verifyPortableGuestHelpers = async (input: Readonly<{
  sessionRoot: string
  destinationRunnerSessionReservationRef: string
  generation: number
}>): Promise<ReadonlyArray<PortableGuestHelperReadiness>> => {
  const state = await readHelperState(input.sessionRoot)
  if (state === undefined) throw new PortableSessionControlError("portable helper state is missing")
  assertHelperState(state, input.destinationRunnerSessionReservationRef, input.generation)
  for (const [kind, helper] of Object.entries({ lsp: state.lsp, watcher: state.watcher })) {
    const expectedReady = `${helper.instanceRef}|${kind === "lsp" ? TYPESCRIPT_VERSION : "ready"}`
    if (!(await helperProcessIsLive(helper)) ||
        (await readFile(helperReadyPath(input.sessionRoot, helper.instanceRef), "utf8").catch(() => "")) !== expectedReady) {
      throw new PortableSessionControlError(`portable ${kind} helper is not live`)
    }
  }
  return helperReadiness(state)
}

export const stopPortableGuestHelpers = async (sessionRoot: string): Promise<void> => {
  const state = await readHelperState(sessionRoot)
  if (state === undefined) return
  for (const helper of [state.lsp, state.watcher]) {
    await terminateManagedHelper(helper)
    await rm(helperReadyPath(sessionRoot, helper.instanceRef), { force: true })
  }
  await rm(helperStatePath(sessionRoot), { force: true })
}

const startManagedLspHelper = async (input: Readonly<{
  sessionRoot: string
  instanceRef: string
  command: ReadonlyArray<string>
  nodeBin: string
}>): Promise<ManagedHelperProcessState> => {
  const readyPath = helperReadyPath(input.sessionRoot, input.instanceRef)
  let spawnFailure: Error | undefined
  const child = spawn(input.nodeBin, [
    "-e", LSP_HELPER_PROGRAM, workspacePath(input.sessionRoot), readyPath, input.instanceRef,
    TYPESCRIPT_VERSION, JSON.stringify(input.command), JSON.stringify({ PATH: "/usr/local/bin:/usr/bin:/bin" }),
  ], { detached: true, stdio: "ignore", env: { PATH: "/usr/local/bin:/usr/bin:/bin" } })
  child.once("error", error => { spawnFailure = error })
  child.unref()
  if (child.pid === undefined) throw new PortableSessionControlError("portable lsp helper did not start")
  const state: ManagedHelperProcessState = {
    pid: child.pid,
    instanceRef: input.instanceRef,
    versionRef: `version.typescript-language-server.${TYPESCRIPT_LANGUAGE_SERVER_VERSION.replaceAll(".", "_")}.typescript.${TYPESCRIPT_VERSION.replaceAll(".", "_")}.node24.linux-x64`,
    evidenceRefs: [stableRef("evidence.agent-computer.portable.lsp.protocol-ready", input.instanceRef)],
  }
  try {
    const expectedReady = `${input.instanceRef}|${TYPESCRIPT_VERSION}`
    for (let attempt = 0; attempt < 300; attempt += 1) {
      if ((await readFile(readyPath, "utf8").catch(() => "")) === expectedReady && processIsLive(child.pid)) return state
      if (spawnFailure !== undefined || !processIsLive(child.pid)) break
      await new Promise(resolve => setTimeout(resolve, 10))
    }
    throw new PortableSessionControlError("portable lsp helper did not report protocol ready")
  } catch (error) {
    await terminateManagedHelper(state)
    await rm(readyPath, { force: true })
    throw error
  }
}

export const startPortableGuestHelpers = async (input: Readonly<{
  sessionRoot: string
  destinationRunnerSessionReservationRef: string
  generation: number
  nodeBin?: string
  lspCommand?: ReadonlyArray<string>
}>): Promise<ReadonlyArray<PortableGuestHelperReadiness>> => {
  const prior = await readHelperState(input.sessionRoot)
  if (prior !== undefined) {
    assertHelperState(prior, input.destinationRunnerSessionReservationRef, input.generation)
    return verifyPortableGuestHelpers(input)
  }
  const watcherInstanceRef = stableRef(
    "instance.agent-computer.portable.watcher",
    `${input.destinationRunnerSessionReservationRef}|${input.generation}`,
  )
  const lspInstanceRef = stableRef("instance.agent-computer.portable.lsp", `${input.destinationRunnerSessionReservationRef}|${input.generation}`)
  const readyPath = helperReadyPath(input.sessionRoot, watcherInstanceRef)
  let spawnFailure: Error | undefined
  const nodeBin = input.nodeBin ?? "/usr/local/bin/node"
  const child = spawn(nodeBin, [
    "-e",
    WATCHER_PROGRAM,
    workspacePath(input.sessionRoot),
    readyPath,
    watcherInstanceRef,
  ], { detached: true, stdio: "ignore", env: { PATH: "/usr/local/bin:/usr/bin:/bin" } })
  child.once("error", error => { spawnFailure = error })
  child.unref()
  if (child.pid === undefined) throw new PortableSessionControlError("portable watcher helper did not start")
  const watcher: ManagedHelperProcessState = {
      pid: child.pid,
      instanceRef: watcherInstanceRef,
      versionRef: `version.node.fs-watch-recursive.${process.versions.node.replaceAll(".", "_")}`,
      evidenceRefs: [stableRef("evidence.agent-computer.portable.watcher.live", watcherInstanceRef)],
  }
  let lsp: ManagedHelperProcessState | undefined
  try {
    for (let attempt = 0; attempt < 100; attempt += 1) {
      if ((await readFile(readyPath, "utf8").catch(() => "")) === watcherInstanceRef && processIsLive(child.pid)) {
        await writeFile(readyPath, `${watcherInstanceRef}|ready`, { mode: 0o600 })
        lsp = await startManagedLspHelper({
          sessionRoot: input.sessionRoot,
          instanceRef: lspInstanceRef,
          command: input.lspCommand ?? LSP_COMMAND,
          nodeBin,
        })
        const state: PortableGuestHelperState = {
          schema: PORTABLE_HELPER_STATE_SCHEMA,
          destinationRunnerSessionReservationRef: input.destinationRunnerSessionReservationRef,
          generation: input.generation,
          lsp,
          watcher,
        }
        const path = helperStatePath(input.sessionRoot)
        const temporary = `${path}.tmp-${process.pid}`
        await writeFile(temporary, canonicalJson(state), { mode: 0o600 })
        await rename(temporary, path)
        return helperReadiness(state)
      }
      if (spawnFailure !== undefined || !processIsLive(child.pid)) break
      await new Promise(resolve => setTimeout(resolve, 10))
    }
    throw new PortableSessionControlError("portable watcher helper did not report ready")
  } catch (error) {
    await terminateManagedHelper(watcher)
    if (lsp !== undefined) await terminateManagedHelper(lsp)
    await rm(helperReadyPath(input.sessionRoot, lspInstanceRef), { force: true })
    await rm(readyPath, { force: true })
    throw error
  }
}

export const productionRuntime: PortableSessionGuestRuntime = {
  repositorySnapshot,
  prepare: async ({ sessionRoot, agentRefs }) => {
    for (const agentRef of agentRefs) {
      const stateDir = agentStatePath(sessionRoot, agentRef)
      await mkdir(stateDir, { recursive: true, mode: 0o700 })
      await run(["/usr/local/bin/oa-workroomd", "lifecycle", "create", "--state-dir", stateDir, "--json"])
    }
  },
  verifyStage: async ({ sessionRoot, bundle }) => {
    const expected = asObject(bundle.checkpoint, "checkpoint")
    const graph = asObject(bundle.graph, "graph")
    graphAgentRefs(graph)
    const normalizedGraph = {
      ...graph,
      nodes: [...(graph.nodes as ReadonlyArray<Record<string, unknown>>)]
        .map(node => ({ ...node }))
        .sort((left, right) => String(left.agentRef).localeCompare(String(right.agentRef))),
    }
    if (digest(normalizedGraph) !== expected.graphDigest) {
      throw new PortableSessionControlError("materialized graph does not match checkpoint")
    }
    const snapshot = await repositorySnapshot(sessionRoot)
    if (snapshot.repositoryRevisionRef !== expected.repositoryRevisionRef ||
        snapshot.repositoryPostImageDigest !== expected.repositoryPostImageDigest ||
        snapshot.diffDigest !== expected.diffDigest) {
      throw new PortableSessionControlError("materialized repository does not match checkpoint")
    }
    for (const agentRef of graphAgentRefs(bundle.graph)) {
      if (!(await Runtime.file(join(agentStatePath(sessionRoot, agentRef), "lifecycle-state.json")).exists())) {
        throw new PortableSessionControlError("materialized agent lifecycle state is missing")
      }
    }
  },
  verifyCapabilities: async ({ sessionRoot, leaseRefs }) => {
    for (const leaseRef of leaseRefs) {
      const marker = JSON.parse(await readFile(join(
        sessionRoot,
        "capabilities",
        `${createHash("sha256").update(leaseRef).digest("hex").slice(0, 24)}.installed.json`,
      ), "utf8")) as Record<string, unknown>
      if (marker.leaseRef !== leaseRef || typeof marker.evidenceRef !== "string" || !SAFE_REF.test(marker.evidenceRef)) {
        throw new PortableSessionControlError("capability installation marker is invalid")
      }
      publicSafe(marker)
    }
  },
  activate: async ({ sessionRoot, agentRefs, destinationRunnerSessionReservationRef, generation }) => {
    const activated: string[] = []
    try {
      for (const agentRef of agentRefs) {
        await transitionLifecycle(agentStatePath(sessionRoot, agentRef), "running")
        activated.push(agentRef)
      }
      return await startPortableGuestHelpers({
        sessionRoot,
        destinationRunnerSessionReservationRef,
        generation,
      })
    } catch (error) {
      await stopPortableGuestHelpers(sessionRoot).catch(() => undefined)
      for (const agentRef of activated.reverse()) {
        await transitionLifecycle(agentStatePath(sessionRoot, agentRef), "paused").catch(() => undefined)
      }
      throw error
    }
  },
  verifyActivation: async ({ sessionRoot, destinationRunnerSessionReservationRef, generation }) => {
    await verifyPortableGuestHelpers({
      sessionRoot,
      destinationRunnerSessionReservationRef,
      generation,
    })
  },
  continueWork: async ({ sessionRoot, ownerRef, repositoryRef, providerLeaseRef, turns }) => {
    const materialLeaf = createHash("sha256").update(providerLeaseRef).digest("hex").slice(0, 24)
    const authJsonPath = join(sessionRoot, "capability-material", `${materialLeaf}.material`)
    if (!(await Runtime.file(authJsonPath).exists())) throw new PortableSessionControlError("provider capability material is unavailable")
    const completed: Array<Readonly<{ agentRef: string; turnRef: string; activityCursor: number; eventCursor: number }>> = []
    for (const turn of turns) {
      const stateDir = agentStatePath(sessionRoot, turn.agentRef)
      const assignmentRef = stableRef("assignment.portable", `${turn.agentRef}|${turn.turnRef}`)
      const workroomRef = stableRef("workroom.portable", turn.agentRef)
      const grantRef = stableRef("grant.portable", `${providerLeaseRef}|${turn.turnRef}`)
      const now = Date.now()
      const assignmentPath = join(stateDir, `${assignmentRef}.json`)
      const grantPath = join(stateDir, `${grantRef}.json`)
      await writeFile(assignmentPath, canonicalJson({
        contract_version: "openagents.codex_workroom_assignment.v1",
        assignment_id: assignmentRef,
        workroom_id: workroomRef,
        target_node_id: turn.agentRef,
        user_ref: ownerRef,
        organization_ref: null,
        project_ref: null,
        provider_account_ref: providerLeaseRef,
        auth_grant_ref: grantRef,
        repo_ref: repositoryRef,
        prompt: turn.task,
        required_artifacts: ["continuation-receipt"],
        sandbox: "workspace_write",
        timeout_ms: 900_000,
        wallet_authority: false,
        created_at_ms: now,
        audit_context: turn.turnRef,
      }), { mode: 0o600 })
      await writeFile(grantPath, canonicalJson({
        contract_version: "openagents.codex_auth_grant.v1",
        workroom_id: workroomRef,
        user_ref: ownerRef,
        organization_ref: null,
        project_ref: null,
        provider_account_ref: providerLeaseRef,
        grant_ref: grantRef,
        provider_secret_ref: `provider-account://${providerLeaseRef}`,
        requested_mode: "exec",
        issued_at_ms: now,
        expires_at_ms: now + 60 * 60 * 1000,
        audit_context: turn.turnRef,
      }), { mode: 0o600 })
      const sessionStatePath = join(stateDir, "codex-session-state.json")
      const existingSession = await Runtime.file(sessionStatePath).exists()
      if (!existingSession) {
        const codexWorkspaceRoot = join(stateDir, "codex-workspaces")
        await mkdir(codexWorkspaceRoot, { recursive: true, mode: 0o700 })
        await symlink(workspacePath(sessionRoot), join(codexWorkspaceRoot, assignmentRef))
        await run([
          "/usr/local/bin/oa-workroomd", "codex", "session", "create",
          "--assignment-file", assignmentPath, "--ttl-ms", "7200000", "--state-dir", stateDir, "--json",
        ])
      }
      const output = JSON.parse(await run([
        "/usr/local/bin/oa-workroomd", "codex", "session",
        existingSession ? "continue-turn" : "start-turn",
        ...(existingSession ? ["--prompt", turn.task] : []),
        "--grant-file", grantPath,
        "--auth-json-file", authJsonPath,
        "--codex-bin", "/usr/local/bin/codex",
        "--state-dir", stateDir,
        "--json",
      ])) as Record<string, unknown>
      const session = asObject(output.session, "codex continuation session")
      const turnIndex = Number(session.turn_index)
      const events = Array.isArray(session.events) ? session.events.length : -1
      if (session.status !== "idle" || !Number.isSafeInteger(turnIndex) || turnIndex < 1 || events < 1) {
        throw new PortableSessionControlError("codex continuation did not complete one bounded turn")
      }
      completed.push({
        agentRef: turn.agentRef,
        turnRef: turn.turnRef,
        activityCursor: turn.activityCursor + 1,
        eventCursor: turn.eventCursor + 1,
      })
      await rm(assignmentPath, { force: true })
      await rm(grantPath, { force: true })
    }
    return completed
  },
  quiesce: async ({ sessionRoot, agentRefs }) => {
    await stopPortableGuestHelpers(sessionRoot)
    for (const agentRef of agentRefs) {
      await transitionLifecycle(agentStatePath(sessionRoot, agentRef), "paused")
    }
  },
  reclaim: async ({ sessionRoot }) => {
    // The authenticated host tears the entire Firecracker VM down only after
    // this fixed controller returns. Removing guest session state here is the
    // inner wipe; host VM teardown is the authoritative process/port fence.
    await stopPortableGuestHelpers(sessionRoot)
    await rm(workspacePath(sessionRoot), { recursive: true, force: true })
    await rm(join(sessionRoot, "agents"), { recursive: true, force: true })
  },
}

if (Runtime.isMain(import.meta.url)) {
  try {
    const encoded = Runtime.argv[2]
    if (encoded === undefined) throw new PortableSessionControlError("one fixed operation is required")
    const stateRoot = process.env.OPENAGENTS_PORTABLE_SESSION_ROOT ?? "/var/lib/openagents/portable-sessions"
    let response: unknown
    if (encoded === "capability-install") {
      const metadata = Runtime.argv[3]
      if (metadata === undefined) throw new PortableSessionControlError("capability metadata is required")
      const material = await Runtime.stdin.bytes()
      try {
        response = await installPortableCapability({ metadata: JSON.parse(metadata), material, stateRoot })
      } finally {
        material.fill(0)
      }
    } else if (encoded === "checkpoint-materialize") {
      const metadata = Runtime.argv[3]
      if (metadata === undefined) throw new PortableSessionControlError("checkpoint metadata is required")
      const archive = await Runtime.stdin.bytes()
      try {
        response = await materializePortableCheckpoint({ metadata: JSON.parse(metadata), archive, stateRoot, runtime: productionRuntime })
      } finally {
        archive.fill(0)
      }
    } else if (encoded === "continue") {
      const privateBody = await Runtime.stdin.bytes()
      try {
        response = await continuePortableSession({ continuation: JSON.parse(new TextDecoder().decode(privateBody)), stateRoot, runtime: productionRuntime })
      } finally {
        privateBody.fill(0)
      }
    } else if (encoded === "checkpoint-export") {
      const metadata = Runtime.argv[3]
      if (metadata === undefined) throw new PortableSessionControlError("checkpoint export metadata is required")
      response = await exportPortableCheckpoint({ metadata: JSON.parse(metadata), stateRoot })
    } else {
      response = await executePortableSessionControl({ operation: JSON.parse(encoded), stateRoot, runtime: productionRuntime })
    }
    process.stdout.write(`${canonicalJson(publicSafe(response))}\n`)
  } catch (error) {
    process.stderr.write(`${error instanceof Error ? error.message : "portable session control failed"}\n`)
    process.exitCode = 1
  }
}

import { createHash } from "node:crypto"
import { chmodSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from "node:fs"
import path from "node:path"

import { Schema } from "effect"

import {
  type CodexHandoffLedger,
  type CodexHandoffMode,
  type CodexHandoffOpenRequest,
  type CodexHandoffOpenResult,
  type CodexHandoffQuiesceResult,
  type CodexHandoffRepositoryState,
  type CodexHandoffRequest,
} from "./codex-handoff-contract.ts"
import type { ProductSpecRun } from "./product-spec-workroom-contract.ts"

const BINDING_LEDGER_SCHEMA = "openagents.desktop.codex_handoff_binding_ledger.v1" as const
const RefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const BindingSchema = Schema.Struct({
  bindingRef: RefSchema,
  runRef: RefSchema,
  workContextRef: RefSchema,
  sessionRef: Schema.NullOr(RefSchema),
  threadRef: Schema.NullOr(RefSchema),
  turnRef: Schema.NullOr(RefSchema),
  packetRef: RefSchema,
  leaseRef: RefSchema,
  specRef: RefSchema,
  specRevision: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  specDigest: Schema.String.check(Schema.isPattern(/^sha256:[a-f0-9]{64}$/)),
  criterionRefs: Schema.Array(RefSchema).check(Schema.isMinLength(1), Schema.isMaxLength(32)),
  generation: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  createdAt: Schema.String,
  updatedAt: Schema.String,
})
type Binding = typeof BindingSchema.Type
const BindingLedgerSchema = Schema.Struct({
  schema: Schema.Literal(BINDING_LEDGER_SCHEMA),
  bindings: Schema.Array(BindingSchema).check(Schema.isMaxLength(128)),
})

const digest = (value: string): string => createHash("sha256").update(value).digest("hex")
const bindingRef = (runRef: string, packetRef: string, leaseRef: string): string =>
  `handoff-binding.${digest(`${runRef}\0${packetRef}\0${leaseRef}`)}`

const privateWrite = (file: string, bindings: ReadonlyArray<Binding>): void => {
  mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(path.dirname(file), 0o700)
  const pending = `${file}.pending`
  try {
    rmSync(pending, { force: true })
    writeFileSync(pending, `${JSON.stringify({ schema: BINDING_LEDGER_SCHEMA, bindings })}\n`, {
      encoding: "utf8",
      mode: 0o600,
    })
    renameSync(pending, file)
    if (process.platform !== "win32") chmodSync(file, 0o600)
  } catch (error) {
    rmSync(pending, { force: true })
    throw error
  }
}

const readBindings = (file: string): Binding[] => {
  if (!existsSync(file)) return []
  return [...Schema.decodeUnknownSync(BindingLedgerSchema)(JSON.parse(readFileSync(file, "utf8"))).bindings]
}

export type CodexHandoffBindings = Readonly<{
  recordPacketAdmission: (run: ProductSpecRun, packetRef: string) => boolean
  bindNextTurn: (input: Readonly<{
    workContextRef: string
    sessionRef: string
    threadRef: string
    turnRef: string
  }>) => boolean
  resolve: (request: CodexHandoffOpenRequest) => Binding | null
}>

export const openCodexHandoffBindings = (
  file: string,
  now: () => string = () => new Date().toISOString(),
): CodexHandoffBindings => {
  const ledgerFile = path.resolve(file)
  let bindings = readBindings(ledgerFile)
  const persist = (): void => privateWrite(ledgerFile, bindings.slice(-128))
  return {
    recordPacketAdmission: (run, packetRef) => {
      const packet = run.plan.packets.find(candidate => candidate.packetRef === packetRef)
      if (run.plan.state !== "accepted" || packet?.state !== "active" || packet.activeLease == null) return false
      const ref = bindingRef(run.runRef, packet.packetRef, packet.activeLease.leaseRef)
      if (bindings.some(binding => binding.bindingRef === ref)) return true
      const priorGeneration = bindings
        .filter(binding => binding.runRef === run.runRef && binding.packetRef === packet.packetRef)
        .reduce((maximum, binding) => Math.max(maximum, binding.generation), 0)
      const timestamp = now()
      bindings.push(Schema.decodeUnknownSync(BindingSchema)({
        bindingRef: ref,
        runRef: run.runRef,
        workContextRef: run.workContextRef,
        sessionRef: null,
        threadRef: null,
        turnRef: null,
        packetRef: packet.packetRef,
        leaseRef: packet.activeLease.leaseRef,
        specRef: run.spec.specRef,
        specRevision: run.spec.revision,
        specDigest: run.spec.digest,
        criterionRefs: [...packet.criterionIds].sort(),
        generation: priorGeneration + 1,
        createdAt: timestamp,
        updatedAt: timestamp,
      }))
      persist()
      return true
    },
    bindNextTurn: input => {
      const exact = bindings.find(binding => binding.threadRef === input.threadRef && binding.turnRef === input.turnRef)
      if (exact !== undefined) {
        return exact.workContextRef === input.workContextRef && exact.sessionRef === input.sessionRef
      }
      const candidates = bindings.filter(binding =>
        binding.workContextRef === input.workContextRef && binding.threadRef === null && binding.turnRef === null)
      if (candidates.length !== 1) return false
      const selected = candidates[0]!
      const timestamp = now()
      bindings = bindings.map(binding => binding.bindingRef === selected.bindingRef
        ? { ...binding, sessionRef: input.sessionRef, threadRef: input.threadRef, turnRef: input.turnRef, updatedAt: timestamp }
        : binding)
      persist()
      return true
    },
    resolve: request => bindings.find(binding =>
      binding.threadRef === request.threadRef && binding.turnRef === request.turnRef) ?? null,
  }
}

export type CodexHandoffHost = Readonly<{
  open: (request: CodexHandoffOpenRequest) => Promise<CodexHandoffOpenResult>
}>

export const makeCodexHandoffHost = (input: Readonly<{
  bindings: CodexHandoffBindings
  ledger: CodexHandoffLedger
  pinnedRuntimeRef: string
  exactThreadProof?: (binding: Binding) => CodexHandoffRequest["exactThreadCandidate"]
  quiesce: (
    request: CodexHandoffOpenRequest,
    binding: Binding,
    operationRef: string,
  ) => Promise<CodexHandoffQuiesceResult>
  repositoryState: (binding: Binding) => Promise<CodexHandoffRepositoryState | null>
  launch: (binding: Binding, handoff: CodexHandoffMode) => Promise<"opened" | "unavailable" | "failed">
}>): CodexHandoffHost => ({
  open: async request => {
    const binding = input.bindings.resolve(request)
    if (binding === null || binding.sessionRef === null) {
      return {
        state: "refused",
        reason: "work_identity_unavailable",
        message: "Open in Codex requires one exact ProductSpec work packet bound to this turn.",
      }
    }
    const operationRef = `handoff.${digest(`${binding.bindingRef}\0${request.threadRef}\0${request.turnRef}`)}`
    const handoffRequest: CodexHandoffRequest = {
      operationRef,
      identity: {
        workContextRef: binding.workContextRef,
        sessionRef: binding.sessionRef,
        workPacketRef: binding.packetRef,
        specRef: binding.specRef,
        specRevision: binding.specRevision,
        specDigest: binding.specDigest.slice("sha256:".length),
        criterionRefs: binding.criterionRefs,
        openAgentsGeneration: binding.generation,
      },
      pinnedRuntimeRef: input.pinnedRuntimeRef,
      exactThreadCandidate: input.exactThreadProof?.(binding) ?? null,
      repositoryState: null,
    }
    const admitted = await input.ledger.admit(
      handoffRequest,
      () => input.quiesce(request, binding, operationRef),
      () => input.repositoryState(binding),
    )
    if (admitted.phase !== "admitted" || admitted.handoff === null) {
      const reason = admitted.refusal ?? "openagents_not_quiescent"
      return {
        state: "refused",
        reason,
        message: reason === "repository_state_unavailable"
          ? "Open in Codex could not capture the exact repository post-image after stopping OpenAgents."
          : "OpenAgents could not prove that this exact work packet is quiescent.",
      }
    }
    const launched = await input.launch(binding, admitted.handoff)
    if (launched !== "opened") {
      return {
        state: "refused",
        reason: launched === "unavailable" ? "codex_app_unavailable" : "launch_failed",
        message: launched === "unavailable"
          ? "The official Codex app is not installed on this Mac. The handoff remains durably admitted."
          : "The official Codex app did not open. The handoff remains durably admitted for exact retry.",
      }
    }
    return {
      state: "opened",
      operationRef,
      workPacketRef: binding.packetRef,
      mode: admitted.handoff.mode,
      transcriptGap: admitted.handoff.mode === "repository_state",
      message: admitted.handoff.mode === "exact_thread"
        ? "Opened the proven compatible Codex thread."
        : "Opened the exact repository post-image in Codex. Transcript continuity is not proven, so OpenAgents recorded a transcript gap.",
    }
  },
})

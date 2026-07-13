import { createHash } from "node:crypto"
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs"
import path from "node:path"

import { Schema } from "effect"

export const CODEX_HANDOFF_LEDGER_SCHEMA = "openagents.desktop.codex_handoff_ledger.v1" as const
export const CODEX_HANDOFF_RECORD_SCHEMA = "openagents.desktop.codex_handoff_record.v1" as const
export const CODEX_HANDOFF_LIMIT = 128

const RefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const DigestSchema = Schema.String.check(Schema.isPattern(/^[a-f0-9]{64}$/))
const PositiveIntSchema = Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0))

export const CodexHandoffIdentitySchema = Schema.Struct({
  workContextRef: RefSchema,
  sessionRef: RefSchema,
  workPacketRef: RefSchema,
  specRef: RefSchema,
  specRevision: PositiveIntSchema,
  specDigest: DigestSchema,
  criterionRefs: Schema.Array(RefSchema).check(Schema.isMinLength(1), Schema.isMaxLength(32)),
  openAgentsGeneration: PositiveIntSchema,
})
export type CodexHandoffIdentity = typeof CodexHandoffIdentitySchema.Type

export const CodexHandoffRequestSchema = Schema.Struct({
  operationRef: RefSchema,
  identity: CodexHandoffIdentitySchema,
  pinnedRuntimeRef: RefSchema,
  exactThreadCandidate: Schema.NullOr(Schema.Struct({
    providerThreadRef: RefSchema,
    compatibleRuntimeRef: RefSchema,
    compatibilityProofRef: RefSchema,
    transcriptContinuityProofRef: RefSchema,
  })),
  repositoryState: Schema.Struct({
    postImageRef: RefSchema,
    transcriptGapRef: RefSchema,
  }),
})
export type CodexHandoffRequest = typeof CodexHandoffRequestSchema.Type

export const CodexQuiescenceProofSchema = Schema.Struct({
  operationRef: RefSchema,
  workPacketRef: RefSchema,
  openAgentsGeneration: PositiveIntSchema,
  disposition: Schema.Literals(["blocked", "completed", "interrupted", "stopped"]),
  lastDurableEventRef: RefSchema,
  proofRef: RefSchema,
})
export type CodexQuiescenceProof = typeof CodexQuiescenceProofSchema.Type

const ExactThreadHandoffSchema = Schema.Struct({
  mode: Schema.Literal("exact_thread"),
  providerThreadRef: RefSchema,
  compatibleRuntimeRef: RefSchema,
  compatibilityProofRef: RefSchema,
  transcriptContinuityProofRef: RefSchema,
})
const RepositoryStateHandoffSchema = Schema.Struct({
  mode: Schema.Literal("repository_state"),
  postImageRef: RefSchema,
  transcriptGapRef: RefSchema,
  reason: Schema.Literal("exact_thread_continuity_unproven"),
})
export const CodexHandoffModeSchema = Schema.Union([
  ExactThreadHandoffSchema,
  RepositoryStateHandoffSchema,
])
export type CodexHandoffMode = typeof CodexHandoffModeSchema.Type

export const CodexHandoffRecordSchema = Schema.Struct({
  schema: Schema.Literal(CODEX_HANDOFF_RECORD_SCHEMA),
  operationRef: RefSchema,
  requestDigest: DigestSchema,
  identity: CodexHandoffIdentitySchema,
  phase: Schema.Literals(["quiescing", "admitted", "refused"]),
  quiescence: Schema.NullOr(CodexQuiescenceProofSchema),
  handoff: Schema.NullOr(CodexHandoffModeSchema),
  refusal: Schema.NullOr(Schema.Literals([
    "openagents_not_quiescent",
    "quiescence_identity_mismatch",
  ])),
  createdAt: Schema.String,
  updatedAt: Schema.String,
})
export type CodexHandoffRecord = typeof CodexHandoffRecordSchema.Type

const CodexHandoffLedgerSchema = Schema.Struct({
  schema: Schema.Literal(CODEX_HANDOFF_LEDGER_SCHEMA),
  records: Schema.Array(CodexHandoffRecordSchema).check(Schema.isMaxLength(CODEX_HANDOFF_LIMIT)),
})

export type CodexHandoffQuiesceResult =
  | Readonly<{ state: "quiescent"; proof: CodexQuiescenceProof }>
  | Readonly<{ state: "not_quiescent" }>

export type CodexHandoffLedger = Readonly<{
  list: () => ReadonlyArray<CodexHandoffRecord>
  get: (operationRef: string) => CodexHandoffRecord | null
  admit: (
    request: CodexHandoffRequest,
    quiesceOpenAgents: (input: Readonly<{
      operationRef: string
      identity: CodexHandoffIdentity
    }>) => Promise<CodexHandoffQuiesceResult>,
  ) => Promise<CodexHandoffRecord>
}>

export class CodexHandoffError extends Error {
  readonly _tag = "CodexHandoffError"
  override readonly name = "CodexHandoffError"

  constructor(
    readonly reason: "conflicting_operation" | "invalid_ledger" | "storage_unavailable",
    message: string,
  ) {
    super(message)
  }
}

const canonical = (value: unknown): string => {
  if (Array.isArray(value)) return `[${value.map(canonical).join(",")}]`
  if (value !== null && typeof value === "object") {
    return `{${Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, entry]) => `${JSON.stringify(key)}:${canonical(entry)}`)
      .join(",")}}`
  }
  return JSON.stringify(value)
}

const requestDigest = (request: CodexHandoffRequest): string =>
  createHash("sha256").update(canonical(request)).digest("hex")

const normalizeIdentity = (identity: CodexHandoffIdentity): CodexHandoffIdentity => ({
  ...identity,
  criterionRefs: [...new Set(identity.criterionRefs)].sort(),
})

const decodeRequest = (value: CodexHandoffRequest): CodexHandoffRequest => {
  const decoded = Schema.decodeUnknownSync(CodexHandoffRequestSchema)(value)
  if (new Set(decoded.identity.criterionRefs).size !== decoded.identity.criterionRefs.length) {
    throw new CodexHandoffError("conflicting_operation", "criterion refs must be unique")
  }
  return { ...decoded, identity: normalizeIdentity(decoded.identity) }
}

const ensurePrivateParent = (filePath: string): void => {
  const parent = path.dirname(filePath)
  mkdirSync(parent, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(parent, 0o700)
}

const writePrivateAtomic = (filePath: string, records: ReadonlyArray<CodexHandoffRecord>): void => {
  ensurePrivateParent(filePath)
  const pending = `${filePath}.pending`
  try {
    rmSync(pending, { force: true })
    writeFileSync(pending, `${JSON.stringify({ schema: CODEX_HANDOFF_LEDGER_SCHEMA, records })}\n`, {
      encoding: "utf8",
      mode: 0o600,
    })
    if (process.platform !== "win32") chmodSync(pending, 0o600)
    renameSync(pending, filePath)
    if (process.platform !== "win32") chmodSync(filePath, 0o600)
  } catch (error) {
    rmSync(pending, { force: true })
    throw new CodexHandoffError(
      "storage_unavailable",
      error instanceof Error ? error.message : "Codex handoff ledger unavailable",
    )
  }
}

const readLedger = (filePath: string): ReadonlyArray<CodexHandoffRecord> => {
  if (!existsSync(filePath)) return []
  try {
    return Schema.decodeUnknownSync(CodexHandoffLedgerSchema)(
      JSON.parse(readFileSync(filePath, "utf8")),
    ).records
  } catch {
    throw new CodexHandoffError("invalid_ledger", "Codex handoff ledger failed validation")
  }
}

const sameQuiescenceIdentity = (
  request: CodexHandoffRequest,
  proof: CodexQuiescenceProof,
): boolean => proof.operationRef === request.operationRef &&
  proof.workPacketRef === request.identity.workPacketRef &&
  proof.openAgentsGeneration === request.identity.openAgentsGeneration

const selectHandoff = (request: CodexHandoffRequest): CodexHandoffMode => {
  const exact = request.exactThreadCandidate
  if (exact !== null && exact.compatibleRuntimeRef === request.pinnedRuntimeRef) {
    return { mode: "exact_thread", ...exact }
  }
  return {
    mode: "repository_state",
    postImageRef: request.repositoryState.postImageRef,
    transcriptGapRef: request.repositoryState.transcriptGapRef,
    reason: "exact_thread_continuity_unproven",
  }
}

export const openCodexHandoffLedger = (
  file: string,
  now: () => Date = () => new Date(),
): CodexHandoffLedger => {
  const filePath = path.resolve(file)
  let records = [...readLedger(filePath)]
  let operations: Promise<void> = Promise.resolve()

  const persist = (): void => {
    records = records
      .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
      .slice(0, CODEX_HANDOFF_LIMIT)
    writePrivateAtomic(filePath, records)
  }
  const replace = (record: CodexHandoffRecord): CodexHandoffRecord => {
    records = records.filter(entry => entry.operationRef !== record.operationRef)
    records.push(Schema.decodeUnknownSync(CodexHandoffRecordSchema)(record))
    persist()
    return record
  }
  const serialize = <Value>(operation: () => Promise<Value>): Promise<Value> => {
    const result = operations.then(operation, operation)
    operations = result.then(() => undefined, () => undefined)
    return result
  }

  return {
    list: () => [...records],
    get: operationRef => records.find(record => record.operationRef === operationRef) ?? null,
    admit: (input, quiesceOpenAgents) => serialize(async () => {
      const request = decodeRequest(input)
      const digest = requestDigest(request)
      const existing = records.find(record => record.operationRef === request.operationRef)
      if (existing !== undefined && existing.requestDigest !== digest) {
        throw new CodexHandoffError(
          "conflicting_operation",
          "handoff operation identity was reused with different bytes",
        )
      }
      if (existing?.phase === "admitted" || existing?.phase === "refused") return existing

      const timestamp = now().toISOString()
      const quiescing = existing ?? replace({
        schema: CODEX_HANDOFF_RECORD_SCHEMA,
        operationRef: request.operationRef,
        requestDigest: digest,
        identity: request.identity,
        phase: "quiescing",
        quiescence: null,
        handoff: null,
        refusal: null,
        createdAt: timestamp,
        updatedAt: timestamp,
      })

      const result = await quiesceOpenAgents({
        operationRef: request.operationRef,
        identity: request.identity,
      })
      const updatedAt = now().toISOString()
      if (result.state === "not_quiescent") {
        return replace({
          ...quiescing,
          phase: "refused",
          refusal: "openagents_not_quiescent",
          updatedAt,
        })
      }
      const proof = Schema.decodeUnknownSync(CodexQuiescenceProofSchema)(result.proof)
      if (!sameQuiescenceIdentity(request, proof)) {
        return replace({
          ...quiescing,
          phase: "refused",
          refusal: "quiescence_identity_mismatch",
          updatedAt,
        })
      }
      return replace({
        ...quiescing,
        phase: "admitted",
        quiescence: proof,
        handoff: selectHandoff(request),
        refusal: null,
        updatedAt,
      })
    }),
  }
}

import { createHash } from "node:crypto"
import { mkdir, readFile, rename, rm, writeFile } from "node:fs/promises"
import { dirname, isAbsolute, relative, resolve } from "node:path"

import { Schema as S } from "effect"
import {
  FleetWorkerKind as FleetWorkerKindSchema,
  type FleetWorkerKind,
} from "@openagentsinc/khala-fleet-intents"

export const SARAH_CODING_FLEET_START_TOOL = "coding_fleet_start" as const
export const SARAH_CODING_FLEET_START_SCHEMA =
  "sarah.coding_fleet_start.request.v1" as const
export const SARAH_CODING_FLEET_RUN_INDEX_SCHEMA =
  "sarah.coding_fleet_runs.v1" as const

const TargetPreferenceSchema = S.Literals([
  "owner_local",
  "managed_cloud",
  "auto",
])

type TargetPreference = typeof TargetPreferenceSchema.Type

type CodingFleetRepositoryPin = {
  owner: string
  name: string
  branch: string
  commit: string
}

type CodingFleetVerifier =
  | { kind: "command"; command: string }
  | { kind: "ref"; ref: string }

type CodingFleetIssueListSource = {
  kind: "issue_list"
  issueRefs: string[]
}

type CodingFleetPlanUnit = {
  unitRef: string
  title: string
  dependsOn: string[]
}

type CodingFleetPlanDagSource = {
  kind: "plan_dag"
  planRef: string
  units: CodingFleetPlanUnit[]
}

type CodingFleetWorkSource =
  | CodingFleetIssueListSource
  | CodingFleetPlanDagSource

type CodingFleetWorkerPolicy = {
  workerKind: FleetWorkerKind
  targetPreference: TargetPreference
}

const CodingFleetRepositoryPinSchema = S.Struct({
  owner: S.String,
  name: S.String,
  branch: S.String,
  commit: S.String,
})

const CodingFleetVerifierSchema = S.Union([
  S.Struct({ kind: S.Literal("command"), command: S.String }),
  S.Struct({ kind: S.Literal("ref"), ref: S.String }),
])

const CodingFleetPlanUnitInputSchema = S.Struct({
  unitRef: S.String,
  title: S.String,
  dependsOn: S.optionalKey(S.Array(S.String)),
})

const CodingFleetWorkSourceInputSchema = S.Union([
  S.Struct({
    kind: S.Literal("issue_list"),
    issueRefs: S.Array(S.String),
  }),
  S.Struct({
    kind: S.Literal("plan_dag"),
    planRef: S.String,
    units: S.Array(CodingFleetPlanUnitInputSchema),
  }),
])

const CodingFleetWorkerPolicySchema = S.Struct({
  workerKind: FleetWorkerKindSchema,
  targetPreference: TargetPreferenceSchema,
})

const CodingFleetWorkerPolicyInputSchema = S.Struct({
  workerKind: S.String,
  targetPreference: S.String,
})

const SarahCodingFleetStartRequestSchema = S.Struct({
  schema: S.optionalKey(S.Literal(SARAH_CODING_FLEET_START_SCHEMA)),
  objective: S.String,
  repository: CodingFleetRepositoryPinSchema,
  verifier: CodingFleetVerifierSchema,
  workSource: CodingFleetWorkSourceInputSchema,
  workerPolicy: CodingFleetWorkerPolicyInputSchema,
  targetConcurrency: S.Number,
  idempotencyKey: S.String,
})

const CodingFleetIssueListSourceSchema = S.Struct({
  kind: S.Literal("issue_list"),
  issueRefs: S.Array(S.String),
})

const CodingFleetPlanUnitSchema = S.Struct({
  unitRef: S.String,
  title: S.String,
  dependsOn: S.Array(S.String),
})

const CodingFleetPlanDagSourceSchema = S.Struct({
  kind: S.Literal("plan_dag"),
  planRef: S.String,
  units: S.Array(CodingFleetPlanUnitSchema),
})

const CodingFleetWorkSourceSchema = S.Union([
  CodingFleetIssueListSourceSchema,
  CodingFleetPlanDagSourceSchema,
])

const SarahCodingFleetRunRecordSchema = S.Struct({
  schema: S.Literal(SARAH_CODING_FLEET_START_SCHEMA),
  runRef: S.String,
  scope: S.String,
  ownerRef: S.String,
  status: S.Literal("pending_executor"),
  objective: S.String,
  repository: CodingFleetRepositoryPinSchema,
  verifier: CodingFleetVerifierSchema,
  workSource: CodingFleetWorkSourceSchema,
  workerPolicy: CodingFleetWorkerPolicySchema,
  targetConcurrency: S.Number,
  idempotencyKey: S.String,
  requestFingerprint: S.String,
  createdAt: S.String,
  updatedAt: S.String,
})

const SarahCodingFleetRunIndexSchema = S.Struct({
  schema: S.Literal(SARAH_CODING_FLEET_RUN_INDEX_SCHEMA),
  idempotency: S.Record(S.String, S.String),
  runs: S.Record(S.String, SarahCodingFleetRunRecordSchema),
})

type SarahCodingFleetStartRequest =
  typeof SarahCodingFleetStartRequestSchema.Type

export type SarahCodingFleetRunRecord = {
  schema: typeof SARAH_CODING_FLEET_START_SCHEMA
  runRef: string
  scope: string
  ownerRef: string
  status: "pending_executor"
  objective: string
  repository: CodingFleetRepositoryPin
  verifier: CodingFleetVerifier
  workSource: CodingFleetWorkSource
  workerPolicy: CodingFleetWorkerPolicy
  targetConcurrency: number
  idempotencyKey: string
  requestFingerprint: string
  createdAt: string
  updatedAt: string
}

type SarahCodingFleetRunIndex = {
  schema: typeof SARAH_CODING_FLEET_RUN_INDEX_SCHEMA
  idempotency: Record<string, string>
  runs: Record<string, SarahCodingFleetRunRecord>
}

export type SarahCodingFleetStartResult =
  | {
      ok: true
      duplicate: boolean
      runRef: string
      scope: string
      status: "pending_executor"
      objective: string
      repository: CodingFleetRepositoryPin
      verifier: CodingFleetVerifier
      workSource: CodingFleetWorkSource
      workerPolicy: CodingFleetWorkerPolicy
      targetConcurrency: number
      idempotencyKey: string
      privateMaterialExcluded: true
    }
  | {
      ok: false
      error: {
        code:
          | "owner_auth_required"
          | "idempotency_conflict"
          | "invalid_request"
          | "store_unavailable"
          | "unsafe_private_material"
        message: string
        field?: string
      }
    }

type SarahCodingFleetStartFailure = Extract<
  SarahCodingFleetStartResult,
  { ok: false }
>

function emptyIndex(): SarahCodingFleetRunIndex {
  return {
    schema: SARAH_CODING_FLEET_RUN_INDEX_SCHEMA,
    idempotency: {},
    runs: {},
  }
}

type SarahCodingFleetRunStore = {
  readIndex: () => Promise<SarahCodingFleetRunIndex>
  writeIndex: (index: SarahCodingFleetRunIndex) => Promise<void>
}

let sarahCodingFleetRunStoreForTest: SarahCodingFleetRunStore | null = null

export function __setSarahCodingFleetRunStoreForTest(
  store: SarahCodingFleetRunStore | null,
) {
  sarahCodingFleetRunStoreForTest = store
  writeQueue = Promise.resolve()
}

class SarahCodingFleetStoreError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "SarahCodingFleetStoreError"
  }
}

function isNodeError(error: unknown): error is Error & { code?: unknown } {
  return error instanceof Error && "code" in error
}

function decodeRequest(
  value: unknown,
): SarahCodingFleetStartRequest | SarahCodingFleetStartFailure {
  try {
    return S.decodeUnknownSync(SarahCodingFleetStartRequestSchema)(value)
  } catch {
    return invalid(
      "request",
      "coding_fleet_start args failed schema validation.",
    )
  }
}

function decodeIndex(value: unknown): SarahCodingFleetRunIndex {
  try {
    return S.decodeUnknownSync(SarahCodingFleetRunIndexSchema)(
      value,
    ) as SarahCodingFleetRunIndex
  } catch (error) {
    throw new SarahCodingFleetStoreError(
      `coding fleet run store failed schema validation: ${errorMessage(error)}`,
    )
  }
}

function errorMessage(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error)
  return message.length > 240 ? `${message.slice(0, 237)}...` : message
}

function storeUnavailable(): SarahCodingFleetStartFailure {
  return {
    ok: false,
    error: {
      code: "store_unavailable",
      message:
        "coding_fleet_start does not have an enabled durable fleet run store.",
    },
  }
}

function idempotencyConflict(): SarahCodingFleetStartFailure {
  return {
    ok: false,
    error: {
      code: "idempotency_conflict",
      message:
        "idempotencyKey already belongs to a different Sarah coding fleet request.",
    },
  }
}

function resolveFixtureIndexPath(configured = "coding-fleet-runs.json") {
  const raw = configured.trim()
  if (!raw) {
    throw new SarahCodingFleetStoreError("fixture store path is required")
  }
  if (isAbsolute(raw) || raw.split(/[\\/]+/).includes("..")) {
    throw new SarahCodingFleetStoreError(
      "fixture store path must stay under .sarah",
    )
  }
  const base = resolve(
    /* turbopackIgnore: true */ process.cwd(),
    ".sarah",
  )
  const path = resolve(base, raw)
  const relativePath = relative(base, path)
  if (
    !relativePath ||
    relativePath.startsWith("..") ||
    isAbsolute(relativePath)
  ) {
    throw new SarahCodingFleetStoreError(
      "fixture store path must stay under .sarah",
    )
  }
  return path
}

export function createSarahCodingFleetFileStoreForTest(
  configuredPath = "coding-fleet-runs.json",
): SarahCodingFleetRunStore {
  const path = resolveFixtureIndexPath(configuredPath)
  return {
    readIndex: () => readIndex(path),
    writeIndex: (index) => writeIndex(path, index),
  }
}

async function readIndex(path: string): Promise<SarahCodingFleetRunIndex> {
  let raw: string
  try {
    raw = await readFile(path, "utf8")
  } catch (error) {
    if (isNodeError(error) && error.code === "ENOENT") {
      return emptyIndex()
    }
    throw new SarahCodingFleetStoreError(
      `coding fleet run store could not be read: ${errorMessage(error)}`,
    )
  }

  try {
    return validateLoadedIndex(decodeIndex(JSON.parse(raw)))
  } catch (error) {
    if (error instanceof SarahCodingFleetStoreError) throw error
    throw new SarahCodingFleetStoreError(
      `coding fleet run store is not valid JSON: ${errorMessage(error)}`,
    )
  }
}

let writeQueue = Promise.resolve()

async function writeIndex(path: string, index: SarahCodingFleetRunIndex) {
  const tempPath = `${path}.${process.pid}.${Date.now()}.tmp`
  await mkdir(dirname(path), { recursive: true })
  try {
    await writeFile(tempPath, `${JSON.stringify(index, null, 2)}\n`)
    await rename(tempPath, path)
  } catch (error) {
    await rm(tempPath, { force: true }).catch(() => {})
    throw new SarahCodingFleetStoreError(
      `coding fleet run store could not be written atomically: ${errorMessage(error)}`,
    )
  }
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

const PRIVATE_MATERIAL_PATTERNS = [
  /(?:^|\s)\/Users\//i,
  /(?:^|\s)\/private\//i,
  /(?:^|\s)~\//,
  /OPENAGENTS_AGENT_TOKEN/i,
  /(?:API|AUTH|SECRET|TOKEN|PASSWORD|PRIVATE)_?KEY/i,
  /BEGIN [A-Z ]*PRIVATE KEY/,
]

function publicText(
  value: unknown,
  field: string,
  options: { max: number; min?: number } = { max: 500 },
): string | SarahCodingFleetStartFailure {
  if (typeof value !== "string") {
    return invalid(field, `${field} must be a string.`)
  }
  const text = value.trim()
  const min = options.min ?? 1
  if (text.length < min) return invalid(field, `${field} is required.`)
  if (text.length > options.max) {
    return invalid(field, `${field} is too long for the Sarah fleet request.`)
  }
  if (PRIVATE_MATERIAL_PATTERNS.some((pattern) => pattern.test(text))) {
    return {
      ok: false,
      error: {
        code: "unsafe_private_material",
        field,
        message:
          "Sarah coding fleet requests may carry public-safe objectives and refs only, not local paths, credentials, or private material.",
      },
    }
  }
  return text
}

function invalid(field: string, message: string): SarahCodingFleetStartFailure {
  return { ok: false, error: { code: "invalid_request", field, message } }
}

function isFailure(value: unknown): value is SarahCodingFleetStartFailure {
  return isRecord(value) && value.ok === false && isRecord(value.error)
}

function parseRepository(
  value: unknown,
): CodingFleetRepositoryPin | SarahCodingFleetStartFailure {
  if (!isRecord(value)) return invalid("repository", "repository is required.")

  const owner = publicText(value.owner, "repository.owner", { max: 80 })
  if (isFailure(owner)) return owner
  const name = publicText(value.name, "repository.name", { max: 120 })
  if (isFailure(name)) return name
  const branch = publicText(value.branch, "repository.branch", { max: 160 })
  if (isFailure(branch)) return branch
  const commit = publicText(value.commit, "repository.commit", { max: 64, min: 7 })
  if (isFailure(commit)) return commit

  if (!/^[A-Za-z0-9_.-]+$/.test(owner)) {
    return invalid("repository.owner", "repository.owner must be a GitHub owner slug.")
  }
  if (!/^[A-Za-z0-9_.-]+$/.test(name)) {
    return invalid("repository.name", "repository.name must be a GitHub repo slug.")
  }
  if (
    !/^[A-Za-z0-9._/-]+$/.test(branch) ||
    branch.startsWith("/") ||
    branch.includes("..")
  ) {
    return invalid("repository.branch", "repository.branch must be a safe branch ref.")
  }
  if (!/^[A-Fa-f0-9]{7,64}$/.test(commit)) {
    return invalid("repository.commit", "repository.commit must be a pinned commit SHA.")
  }

  return { owner, name, branch, commit }
}

function parseVerifier(
  value: unknown,
): CodingFleetVerifier | SarahCodingFleetStartFailure {
  if (!isRecord(value)) return invalid("verifier", "verifier is required.")
  const kind = value.kind
  if (kind === "command") {
    const command = publicText(value.command, "verifier.command", {
      max: 240,
      min: 3,
    })
    if (isFailure(command)) return command
    return { kind, command }
  }
  if (kind === "ref") {
    const ref = publicText(value.ref, "verifier.ref", { max: 160, min: 3 })
    if (isFailure(ref)) return ref
    return { kind, ref }
  }
  return invalid("verifier.kind", "verifier.kind must be command or ref.")
}

function normalizeIssueRef(value: unknown): string | SarahCodingFleetStartFailure {
  const raw = publicText(value, "workSource.issueRefs", { max: 120 })
  if (isFailure(raw)) return raw
  const match = raw.match(/(?:^#?(\d+)$)|github\.com\/[^/]+\/[^/]+\/issues\/(\d+)/)
  const issue = match?.[1] ?? match?.[2]
  if (!issue) {
    return invalid(
      "workSource.issueRefs",
      "issue_list entries must be #123 or a GitHub issue URL.",
    )
  }
  return `#${issue}`
}

function parsePlanUnit(value: unknown): CodingFleetPlanUnit | SarahCodingFleetStartFailure {
  if (!isRecord(value)) return invalid("workSource.units", "plan units must be objects.")
  const unitRef = publicText(value.unitRef, "workSource.units.unitRef", { max: 80 })
  if (isFailure(unitRef)) return unitRef
  const title = publicText(value.title, "workSource.units.title", { max: 160 })
  if (isFailure(title)) return title
  const dependsOnRaw = Array.isArray(value.dependsOn) ? value.dependsOn : []
  const dependsOn: string[] = []
  for (const item of dependsOnRaw) {
    const dep = publicText(item, "workSource.units.dependsOn", { max: 80 })
    if (isFailure(dep)) return dep
    dependsOn.push(dep)
  }
  return { unitRef, title, dependsOn }
}

function validatePlanDag(
  units: CodingFleetPlanUnit[],
): SarahCodingFleetStartFailure | null {
  const unitRefs = new Set<string>()
  for (const unit of units) {
    if (unitRefs.has(unit.unitRef)) {
      return invalid(
        "workSource.units.unitRef",
        "plan_dag unitRef values must be unique.",
      )
    }
    unitRefs.add(unit.unitRef)
  }

  for (const unit of units) {
    for (const dependencyRef of unit.dependsOn) {
      if (!unitRefs.has(dependencyRef)) {
        return invalid(
          "workSource.units.dependsOn",
          "plan_dag dependencies must reference declared unitRef values.",
        )
      }
    }
  }

  const visiting = new Set<string>()
  const visited = new Set<string>()
  const byRef = new Map(units.map((unit) => [unit.unitRef, unit]))

  const visit = (unitRef: string): boolean => {
    if (visited.has(unitRef)) return true
    if (visiting.has(unitRef)) return false
    visiting.add(unitRef)
    const unit = byRef.get(unitRef)
    if (!unit) return false
    for (const dependencyRef of unit.dependsOn) {
      if (!visit(dependencyRef)) return false
    }
    visiting.delete(unitRef)
    visited.add(unitRef)
    return true
  }

  for (const unit of units) {
    if (!visit(unit.unitRef)) {
      return invalid(
        "workSource.units.dependsOn",
        "plan_dag dependencies must not contain cycles.",
      )
    }
  }

  return null
}

function parseWorkSource(
  value: unknown,
): CodingFleetWorkSource | SarahCodingFleetStartFailure {
  if (!isRecord(value)) return invalid("workSource", "workSource is required.")
  if (value.kind === "issue_list") {
    if (!Array.isArray(value.issueRefs) || value.issueRefs.length === 0) {
      return invalid("workSource.issueRefs", "issue_list needs at least one issue.")
    }
    if (value.issueRefs.length > 25) {
      return invalid("workSource.issueRefs", "issue_list is capped at 25 issues.")
    }
    const issueRefs: string[] = []
    for (const item of value.issueRefs) {
      const issueRef = normalizeIssueRef(item)
      if (isFailure(issueRef)) return issueRef
      issueRefs.push(issueRef)
    }
    return { kind: "issue_list", issueRefs }
  }
  if (value.kind === "plan_dag") {
    const planRef = publicText(value.planRef, "workSource.planRef", {
      max: 120,
      min: 3,
    })
    if (isFailure(planRef)) return planRef
    if (!Array.isArray(value.units) || value.units.length === 0) {
      return invalid("workSource.units", "plan_dag needs at least one unit.")
    }
    if (value.units.length > 25) {
      return invalid("workSource.units", "plan_dag is capped at 25 units.")
    }
    const units: CodingFleetPlanUnit[] = []
    for (const item of value.units) {
      const unit = parsePlanUnit(item)
      if (isFailure(unit)) return unit
      units.push(unit)
    }
    const dagError = validatePlanDag(units)
    if (dagError) return dagError
    return { kind: "plan_dag", planRef, units }
  }
  return invalid("workSource.kind", "workSource.kind must be issue_list or plan_dag.")
}

function parseWorkerPolicy(
  value: unknown,
): CodingFleetWorkerPolicy | SarahCodingFleetStartFailure {
  try {
    return S.decodeUnknownSync(CodingFleetWorkerPolicySchema)(
      value,
    ) as CodingFleetWorkerPolicy
  } catch {
    if (
      isRecord(value) &&
      typeof value.workerKind === "string" &&
      !["codex", "claude", "grok", "auto"].includes(value.workerKind)
    ) {
      return invalid(
        "workerPolicy.workerKind",
        "workerPolicy.workerKind must use the FleetWorkerKind vocabulary.",
      )
    }
    return invalid(
      "workerPolicy.targetPreference",
      "targetPreference must be owner_local, managed_cloud, or auto.",
    )
  }
}

function parseTargetConcurrency(value: unknown): number | SarahCodingFleetStartFailure {
  if (!Number.isInteger(value) || typeof value !== "number") {
    return invalid("targetConcurrency", "targetConcurrency must be an integer.")
  }
  if (value < 1 || value > 8) {
    return invalid("targetConcurrency", "targetConcurrency must be between 1 and 8.")
  }
  return value
}

function canonicalJson(value: unknown): string {
  if (Array.isArray(value)) {
    return `[${value.map((item) => canonicalJson(item)).join(",")}]`
  }
  if (isRecord(value)) {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalJson(value[key])}`)
      .join(",")}}`
  }
  return JSON.stringify(value)
}

function digestCanonical(value: unknown): string {
  return createHash("sha256").update(canonicalJson(value)).digest("hex")
}

function stableRunRef(ownerRef: string, idempotencyKey: string): string {
  const digest = digestCanonical({ idempotencyKey, ownerRef }).slice(0, 20)
  return `fleet_run.sarah.${digest}`
}

function sameJson(left: unknown, right: unknown) {
  return canonicalJson(left) === canonicalJson(right)
}

function requestFingerprint(input: {
  objective: string
  repository: CodingFleetRepositoryPin
  verifier: CodingFleetVerifier
  workSource: CodingFleetWorkSource
  workerPolicy: CodingFleetWorkerPolicy
  targetConcurrency: number
}) {
  return digestCanonical(input)
}

function assertLoadedRecordInvariant(
  runRef: string,
  record: SarahCodingFleetRunRecord,
  index: SarahCodingFleetRunIndex,
) {
  const fail = (message: string): never => {
    throw new SarahCodingFleetStoreError(message)
  }
  const loadedPublicText = (
    value: unknown,
    field: string,
    options: { max: number; min?: number },
    message: string,
  ): string => {
    const text = publicText(value, field, options)
    if (typeof text === "string") return text
    throw new SarahCodingFleetStoreError(message)
  }
  if (runRef !== record.runRef) fail("run key does not match runRef")
  const ownerRef = loadedPublicText(
    record.ownerRef,
    "ownerRef",
    { max: 160, min: 3 },
    "ownerRef is not a safe owner reference",
  )
  const idempotencyKey = loadedPublicText(
    record.idempotencyKey,
    "idempotencyKey",
    { max: 120, min: 8 },
    "idempotencyKey is not public-safe",
  )
  if (record.runRef !== stableRunRef(ownerRef, idempotencyKey)) {
    fail("runRef does not match owner-scoped idempotency key")
  }
  if (record.scope !== `scope.fleet_run.${record.runRef}`) {
    fail("scope does not match runRef")
  }
  const idempotencyRef = `${ownerRef}:${idempotencyKey}`
  if (index.idempotency[idempotencyRef] !== record.runRef) {
    fail("owner-scoped idempotency mapping does not match runRef")
  }

  const objective = publicText(record.objective, "objective", {
    max: 1000,
    min: 8,
  })
  if (isFailure(objective) || objective !== record.objective) {
    fail("objective is not canonical public-safe text")
  }
  const canonicalObjective = objective as string
  const repository = parseRepository(record.repository)
  if (isFailure(repository)) {
    fail("repository is not canonical public-safe data")
  }
  if (!sameJson(repository, record.repository)) {
    fail("repository is not canonical public-safe data")
  }
  const canonicalRepository = repository as CodingFleetRepositoryPin
  const verifier = parseVerifier(record.verifier)
  if (isFailure(verifier)) {
    fail("verifier is not canonical public-safe data")
  }
  if (!sameJson(verifier, record.verifier)) {
    fail("verifier is not canonical public-safe data")
  }
  const canonicalVerifier = verifier as CodingFleetVerifier
  const workSource = parseWorkSource(record.workSource)
  if (isFailure(workSource)) {
    fail("workSource is not canonical public-safe data")
  }
  if (!sameJson(workSource, record.workSource)) {
    fail("workSource is not canonical public-safe data")
  }
  const canonicalWorkSource = workSource as CodingFleetWorkSource
  const workerPolicy = parseWorkerPolicy(record.workerPolicy)
  if (isFailure(workerPolicy)) {
    fail("workerPolicy is not canonical public-safe data")
  }
  if (!sameJson(workerPolicy, record.workerPolicy)) {
    fail("workerPolicy is not canonical public-safe data")
  }
  const canonicalWorkerPolicy = workerPolicy as CodingFleetWorkerPolicy
  const targetConcurrency = parseTargetConcurrency(record.targetConcurrency)
  if (isFailure(targetConcurrency)) {
    fail("targetConcurrency is not valid")
  }
  if (targetConcurrency !== record.targetConcurrency) {
    fail("targetConcurrency is not valid")
  }
  const canonicalTargetConcurrency = targetConcurrency as number
  const fingerprint = requestFingerprint({
    objective: canonicalObjective,
    repository: canonicalRepository,
    verifier: canonicalVerifier,
    workSource: canonicalWorkSource,
    workerPolicy: canonicalWorkerPolicy,
    targetConcurrency: canonicalTargetConcurrency,
  })
  if (record.requestFingerprint !== fingerprint) {
    fail("request fingerprint does not match canonical request")
  }
}

function validateLoadedIndex(
  index: SarahCodingFleetRunIndex,
): SarahCodingFleetRunIndex {
  for (const [runRef, record] of Object.entries(index.runs)) {
    assertLoadedRecordInvariant(runRef, record, index)
  }
  for (const [idempotencyRef, runRef] of Object.entries(index.idempotency)) {
    const record = index.runs[runRef]
    if (!record) {
      throw new SarahCodingFleetStoreError(
        "idempotency mapping points at a missing run",
      )
    }
    if (idempotencyRef !== `${record.ownerRef}:${record.idempotencyKey}`) {
      throw new SarahCodingFleetStoreError(
        "idempotency mapping is not owner-scoped to its run",
      )
    }
  }
  return index
}

function publicResult(
  record: SarahCodingFleetRunRecord,
  duplicate: boolean,
): SarahCodingFleetStartResult {
  return {
    ok: true,
    duplicate,
    runRef: record.runRef,
    scope: record.scope,
    status: record.status,
    objective: record.objective,
    repository: record.repository,
    verifier: record.verifier,
    workSource: record.workSource,
    workerPolicy: record.workerPolicy,
    targetConcurrency: record.targetConcurrency,
    idempotencyKey: record.idempotencyKey,
    privateMaterialExcluded: true,
  }
}

export async function startSarahCodingFleetRun(
  args: unknown,
  context: {
    ownerRef?: string | undefined
    store?: SarahCodingFleetRunStore | undefined
  },
): Promise<SarahCodingFleetStartResult> {
  const rawOwnerRef = context.ownerRef?.trim()
  if (!rawOwnerRef) {
    return {
      ok: false,
      error: {
        code: "owner_auth_required",
        message: "coding_fleet_start requires an authenticated OpenAgents owner.",
      },
    }
  }
  const ownerRef = publicText(rawOwnerRef, "ownerRef", { max: 160, min: 3 })
  if (isFailure(ownerRef)) return ownerRef
  const request = decodeRequest(args)
  if (isFailure(request)) return request

  const objective = publicText(request.objective, "objective", {
    max: 1000,
    min: 8,
  })
  if (isFailure(objective)) return objective
  const repository = parseRepository(request.repository)
  if (isFailure(repository)) return repository
  const verifier = parseVerifier(request.verifier)
  if (isFailure(verifier)) return verifier
  const workSource = parseWorkSource(request.workSource)
  if (isFailure(workSource)) return workSource
  const workerPolicy = parseWorkerPolicy(request.workerPolicy)
  if (isFailure(workerPolicy)) return workerPolicy
  const targetConcurrency = parseTargetConcurrency(request.targetConcurrency)
  if (isFailure(targetConcurrency)) return targetConcurrency
  const idempotencyKey = publicText(request.idempotencyKey, "idempotencyKey", {
    max: 120,
    min: 8,
  })
  if (isFailure(idempotencyKey)) return idempotencyKey
  const store = context.store ?? sarahCodingFleetRunStoreForTest
  if (!store) return storeUnavailable()

  const idempotencyRef = `${ownerRef}:${idempotencyKey}`
  const runRef = stableRunRef(ownerRef, idempotencyKey)
  const fingerprint = requestFingerprint({
    objective,
    repository,
    verifier,
    workSource,
    workerPolicy,
    targetConcurrency,
  })
  const now = new Date().toISOString()

  let result: SarahCodingFleetStartResult | null = null
  const operation = writeQueue.catch(() => undefined).then(async () => {
    const index = await store.readIndex()
    const existingRef = index.idempotency[idempotencyRef]
    if (existingRef && index.runs[existingRef]) {
      if (index.runs[existingRef].requestFingerprint !== fingerprint) {
        result = idempotencyConflict()
        return
      }
      result = publicResult(index.runs[existingRef], true)
      return
    }
    const record: SarahCodingFleetRunRecord = {
      schema: SARAH_CODING_FLEET_START_SCHEMA,
      runRef,
      scope: `scope.fleet_run.${runRef}`,
      ownerRef,
      status: "pending_executor",
      objective,
      repository,
      verifier,
      workSource,
      workerPolicy,
      targetConcurrency,
      idempotencyKey,
      requestFingerprint: fingerprint,
      createdAt: now,
      updatedAt: now,
    }
    index.runs[runRef] = record
    index.idempotency[idempotencyRef] = runRef
    await store.writeIndex(index)
    result = publicResult(record, false)
  })
  writeQueue = operation.catch(() => undefined)
  try {
    await operation
  } catch {
    return storeUnavailable()
  }

  return result ?? invalid("store", "coding fleet run store did not return a result.")
}

export async function listSarahCodingFleetRunsForTest(): Promise<
  SarahCodingFleetRunRecord[]
> {
  if (!sarahCodingFleetRunStoreForTest) return []
  const index = await sarahCodingFleetRunStoreForTest.readIndex()
  return Object.values(index.runs)
}

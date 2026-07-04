import { readFileSync } from "node:fs"

const SCHEMA = "openagents.khala_sync.cross_device_chat_dogfood.v1"

const forbiddenKeys = new Set([
  "body",
  "content",
  "messagebody",
  "messagecontent",
  "prompt",
  "rawbody",
  "rawtext",
  "response",
  "text",
  "transcript",
])

const privateMaterialPattern =
  /(\/Users\/|\/home\/|auth\.json|bearer\s+[A-Za-z0-9._-]+|gh[opsu]_[A-Za-z0-9_]+|mdk[_-]?access[_-]?token|mnemonic|oa_agent_[A-Za-z0-9._-]+|password|private[_-]?key|secret|sk-[A-Za-z0-9_-]+)/i

type JsonRecord = Record<string, unknown>

export class KhalaSyncEvidenceValidationError extends Error {
  override readonly name = "KhalaSyncEvidenceValidationError"
}

const fail = (message: string): never => {
  throw new KhalaSyncEvidenceValidationError(message)
}

const asRecord = (value: unknown, label: string): JsonRecord =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? (value as JsonRecord)
    : fail(`${label} must be an object`)

const asString = (record: JsonRecord, key: string, label: string): string => {
  const value = record[key]
  return typeof value === "string" && value.length > 0
    ? value
    : fail(`${label}.${key} must be a non-empty string`)
}

const asArray = (record: JsonRecord, key: string, label: string): ReadonlyArray<unknown> => {
  const value = record[key]
  return Array.isArray(value) ? value : fail(`${label}.${key} must be an array`)
}

const asBoolean = (record: JsonRecord, key: string, label: string): boolean => {
  const value = record[key]
  return typeof value === "boolean" ? value : fail(`${label}.${key} must be boolean`)
}

const asNonNegativeInteger = (
  record: JsonRecord,
  key: string,
  label: string,
): number => {
  const value = record[key]
  return Number.isInteger(value) && Number(value) >= 0
    ? Number(value)
    : fail(`${label}.${key} must be a non-negative integer`)
}

const assertNoPrivateMaterial = (value: unknown, path = "$"): void => {
  if (typeof value === "string") {
    if (privateMaterialPattern.test(value)) {
      fail(`${path} contains private or secret-shaped material`)
    }
    return
  }
  if (Array.isArray(value)) {
    value.forEach((entry, index) => assertNoPrivateMaterial(entry, `${path}[${index}]`))
    return
  }
  if (typeof value === "object" && value !== null) {
    for (const [key, entry] of Object.entries(value as JsonRecord)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "")
      if (forbiddenKeys.has(normalized)) {
        fail(`${path}.${key} is not allowed in public-safe chat dogfood evidence`)
      }
      assertNoPrivateMaterial(entry, `${path}.${key}`)
    }
  }
}

const validateSafeguards = (record: JsonRecord): void => {
  const safeguards = asRecord(record.safeguards, "$.safeguards")
  if (asBoolean(safeguards, "containsChatContent", "$.safeguards") !== false) {
    fail("$.safeguards.containsChatContent must be false")
  }
  if (asBoolean(safeguards, "containsSecrets", "$.safeguards") !== false) {
    fail("$.safeguards.containsSecrets must be false")
  }
  if (asBoolean(safeguards, "contentFieldsRedacted", "$.safeguards") !== true) {
    fail("$.safeguards.contentFieldsRedacted must be true")
  }
  if (asBoolean(safeguards, "ownerSignedTransitionsOnly", "$.safeguards") !== true) {
    fail("$.safeguards.ownerSignedTransitionsOnly must be true")
  }
  if (asBoolean(safeguards, "promiseFlips", "$.safeguards") !== false) {
    fail("$.safeguards.promiseFlips must be false")
  }
}

const validateFlow = (value: unknown, index: number): void => {
  const label = `$.flows[${index}]`
  const flow = asRecord(value, label)
  asString(flow, "flowRef", label)
  asString(flow, "sourceSurface", label)
  asArray(flow, "observedSurfaces", label).forEach((surface, surfaceIndex) => {
    if (typeof surface !== "string" || surface.length === 0) {
      fail(`${label}.observedSurfaces[${surfaceIndex}] must be a non-empty string`)
    }
  })

  const counts = asRecord(flow.counts, `${label}.counts`)
  for (const key of [
    "threadsCreated",
    "messagesAppended",
    "threadsObserved",
    "messagesObserved",
  ]) {
    asNonNegativeInteger(counts, key, `${label}.counts`)
  }

  const latencyMs = asRecord(flow.latencyMs, `${label}.latencyMs`)
  for (const [key, value] of Object.entries(latencyMs)) {
    if (typeof value !== "number" || !Number.isFinite(value) || value < 0) {
      fail(`${label}.latencyMs.${key} must be a non-negative finite number`)
    }
  }

  for (const key of ["scopeRefs", "receiptRefs", "routeRefs"]) {
    asArray(flow, key, label).forEach((entry, entryIndex) => {
      if (typeof entry !== "string" || entry.length === 0) {
        fail(`${label}.${key}[${entryIndex}] must be a non-empty string`)
      }
    })
  }
}

export const validateKhalaSyncCrossDeviceEvidence = (input: unknown): JsonRecord => {
  assertNoPrivateMaterial(input)
  const record = asRecord(input, "$")
  if (asString(record, "schema", "$") !== SCHEMA) {
    fail(`$.schema must be ${SCHEMA}`)
  }
  const status = asString(record, "status", "$")
  if (status !== "pending_owner_signoff" && status !== "owner_signed") {
    fail("$.status must be pending_owner_signoff or owner_signed")
  }
  asString(record, "issueRef", "$")
  asString(record, "epicRef", "$")
  asString(record, "generatedAt", "$")
  validateSafeguards(record)

  const flows = asArray(record, "flows", "$")
  flows.forEach(validateFlow)

  asArray(record, "khalaCodeEvidenceRefs", "$").forEach((entry, index) => {
    if (typeof entry !== "string" || !entry.startsWith("khala_code.")) {
      fail(`$.khalaCodeEvidenceRefs[${index}] must be a khala_code.* ref`)
    }
  })

  if (status === "owner_signed") {
    if (flows.length === 0) fail("$.flows must not be empty for owner_signed evidence")
    const signoff = asRecord(record.ownerSignoff, "$.ownerSignoff")
    asString(signoff, "signerRef", "$.ownerSignoff")
    asString(signoff, "signedAt", "$.ownerSignoff")
    asString(signoff, "methodRef", "$.ownerSignoff")
  }

  return record
}

if (import.meta.main) {
  const path = Bun.argv[2]
  if (path === undefined) {
    console.error("usage: bun scripts/validate-khala-sync-cross-device-evidence.ts <bundle.json>")
    process.exit(2)
  }
  try {
    validateKhalaSyncCrossDeviceEvidence(JSON.parse(readFileSync(path, "utf8")) as unknown)
    console.log(`ok ${path}`)
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  }
}

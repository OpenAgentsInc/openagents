import { readFileSync } from "node:fs"

const SCHEMA = "openagents.khala_sync.runtime_ai_sdk_shaped_dogfood.v1"

const forbiddenKeys = new Set([
  "apikey",
  "body",
  "chatbody",
  "chatcontent",
  "chunk",
  "content",
  "localpath",
  "messagebody",
  "messagecontent",
  "path",
  "prompt",
  "providerchunk",
  "providerpayload",
  "raw",
  "rawbody",
  "rawchunk",
  "rawevent",
  "rawtext",
  "response",
  "secret",
  "text",
  "token",
  "tokens",
  "transcript",
])

const privateMaterialPattern =
  /(\/Users\/|\/home\/|\/private\/|~\/|\.codex\/|\.claude\/|\.secrets\/|auth\.json|bearer\s+[A-Za-z0-9._-]+|gh[opsu]_[A-Za-z0-9_]+|mdk[_-]?access[_-]?token|mnemonic|oa_agent_[A-Za-z0-9._-]+|openai[_-]?api[_-]?key|password|private[_-]?key|secret|sk-[A-Za-z0-9_-]+)/i

const latencyBuckets = new Set([
  "lt_1s",
  "lt_5s",
  "lt_30s",
  "lt_2m",
  "offline_window",
  "not_observed",
  "simulated",
])

type JsonRecord = Record<string, unknown>

export class KhalaSyncRuntimeDogfoodEvidenceValidationError extends Error {
  override readonly name =
    "KhalaSyncRuntimeDogfoodEvidenceValidationError"
}

const fail = (message: string): never => {
  throw new KhalaSyncRuntimeDogfoodEvidenceValidationError(message)
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

const optionalString = (
  record: JsonRecord,
  key: string,
  label: string,
): string | undefined => {
  const value = record[key]
  if (value === undefined) return undefined
  return typeof value === "string" && value.length > 0
    ? value
    : fail(`${label}.${key} must be a non-empty string when present`)
}

const asArray = (
  record: JsonRecord,
  key: string,
  label: string,
): ReadonlyArray<unknown> => {
  const value = record[key]
  return Array.isArray(value) ? value : fail(`${label}.${key} must be an array`)
}

const asBoolean = (record: JsonRecord, key: string, label: string): boolean => {
  const value = record[key]
  return typeof value === "boolean"
    ? value
    : fail(`${label}.${key} must be boolean`)
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
    value.forEach((entry, index) =>
      assertNoPrivateMaterial(entry, `${path}[${index}]`),
    )
    return
  }
  if (typeof value === "object" && value !== null) {
    for (const [key, entry] of Object.entries(value as JsonRecord)) {
      const normalized = key.toLowerCase().replace(/[^a-z0-9]/g, "")
      if (forbiddenKeys.has(normalized)) {
        fail(
          `${path}.${key} is not allowed in public-safe runtime dogfood evidence`,
        )
      }
      assertNoPrivateMaterial(entry, `${path}.${key}`)
    }
  }
}

const assertRefArray = (
  record: JsonRecord,
  key: string,
  label: string,
): void => {
  asArray(record, key, label).forEach((entry, index) => {
    if (typeof entry !== "string" || entry.length === 0) {
      fail(`${label}.${key}[${index}] must be a non-empty public-safe ref`)
    }
  })
}

const assertIssueRefs = (
  record: JsonRecord,
  key: string,
  label: string,
): void => {
  asArray(record, key, label).forEach((entry, index) => {
    if (
      typeof entry !== "string" ||
      !/^OpenAgentsInc\/openagents#\d+$/.test(entry)
    ) {
      fail(`${label}.${key}[${index}] must be an OpenAgents issue ref`)
    }
  })
}

const validateSafeguards = (record: JsonRecord): void => {
  const safeguards = asRecord(record.safeguards, "$.safeguards")
  for (const key of [
    "containsRawPrompts",
    "containsChatBodies",
    "containsProviderChunks",
    "containsLocalPaths",
    "containsTokens",
    "containsSecrets",
    "promiseFlips",
  ]) {
    if (asBoolean(safeguards, key, "$.safeguards") !== false) {
      fail(`$.safeguards.${key} must be false`)
    }
  }
  if (asBoolean(safeguards, "contentFieldsRedacted", "$.safeguards") !== true) {
    fail("$.safeguards.contentFieldsRedacted must be true")
  }
  if (asBoolean(safeguards, "publicSafeProjectionOnly", "$.safeguards") !== true) {
    fail("$.safeguards.publicSafeProjectionOnly must be true")
  }
  if (asBoolean(safeguards, "simulatorOnlyLabel", "$.safeguards") !== true) {
    fail("$.safeguards.simulatorOnlyLabel must be true")
  }
}

const validateFlowProofs = (flow: JsonRecord, label: string): void => {
  const proofs = asRecord(flow.proofs, `${label}.proofs`)
  for (const key of [
    "mobileIntentAppearedOnDesktopWithoutRestart",
    "desktopRuntimeEventAppearedOnMobileAfterResume",
    "restartResumeWithoutDuplicateEvents",
  ]) {
    if (asBoolean(proofs, key, `${label}.proofs`) !== true) {
      fail(`${label}.proofs.${key} must be true`)
    }
  }
}

const validateFlow = (
  value: unknown,
  index: number,
  status: string,
): void => {
  const label = `$.flows[${index}]`
  const flow = asRecord(value, label)
  asString(flow, "flowRef", label)
  asString(flow, "evidenceMode", label)
  asString(flow, "sourceSurface", label)
  asArray(flow, "observedSurfaces", label).forEach((surface, surfaceIndex) => {
    if (typeof surface !== "string" || surface.length === 0) {
      fail(`${label}.observedSurfaces[${surfaceIndex}] must be a non-empty string`)
    }
  })
  validateFlowProofs(flow, label)

  const counts = asRecord(flow.counts, `${label}.counts`)
  const requiredCounts = [
    "threadsCreated",
    "userMessagesAppended",
    "runtimeControlIntentsAccepted",
    "runtimeTurnsObservedDesktop",
    "runtimeEventsObservedMobile",
    "restartResumeCycles",
    "duplicateEventsAfterResume",
  ]
  for (const key of requiredCounts) {
    asNonNegativeInteger(counts, key, `${label}.counts`)
  }
  if (asNonNegativeInteger(counts, "duplicateEventsAfterResume", `${label}.counts`) !== 0) {
    fail(`${label}.counts.duplicateEventsAfterResume must be 0`)
  }
  if (status !== "pending_owner_device" && asNonNegativeInteger(counts, "runtimeEventsObservedMobile", `${label}.counts`) === 0) {
    fail(`${label}.counts.runtimeEventsObservedMobile must be positive`)
  }

  const latency = asRecord(flow.latencyBuckets, `${label}.latencyBuckets`)
  for (const [key, value] of Object.entries(latency)) {
    if (typeof value !== "string" || !latencyBuckets.has(value)) {
      fail(`${label}.latencyBuckets.${key} must be a known latency bucket`)
    }
  }

  for (const key of ["routeRefs", "scopeRefs", "receiptRefs", "buildRefs"]) {
    assertRefArray(flow, key, label)
  }
  assertIssueRefs(flow, "issueRefs", label)
}

const validateGap = (value: unknown, index: number): void => {
  const label = `$.gaps[${index}]`
  const gap = asRecord(value, label)
  asString(gap, "gapRef", label)
  const status = asString(gap, "status", label)
  if (status !== "open" && status !== "closed") {
    fail(`${label}.status must be open or closed`)
  }
  optionalString(gap, "summaryRef", label)
  assertIssueRefs(gap, "issueRefs", label)
}

export const validateKhalaSyncRuntimeDogfoodEvidence = (
  input: unknown,
): JsonRecord => {
  assertNoPrivateMaterial(input)
  const record = asRecord(input, "$")
  if (asString(record, "schema", "$") !== SCHEMA) {
    fail(`$.schema must be ${SCHEMA}`)
  }
  const status = asString(record, "status", "$")
  if (
    status !== "simulator_only" &&
    status !== "owner_signed" &&
    status !== "pending_owner_device"
  ) {
    fail("$.status must be simulator_only, owner_signed, or pending_owner_device")
  }
  const evidenceMode = asString(record, "evidenceMode", "$")
  if (status === "simulator_only" && evidenceMode !== "simulator_only") {
    fail("$.evidenceMode must be simulator_only for simulator_only status")
  }
  asString(record, "issueRef", "$")
  asString(record, "generatedAt", "$")
  assertIssueRefs(record, "roadmapIssueRefs", "$")
  assertRefArray(record, "routeRefs", "$")
  assertRefArray(record, "scopeRefs", "$")
  assertRefArray(record, "buildRefs", "$")
  validateSafeguards(record)

  const flows = asArray(record, "flows", "$")
  if (status !== "pending_owner_device" && flows.length === 0) {
    fail("$.flows must not be empty for simulator_only or owner_signed evidence")
  }
  flows.forEach((flow, index) => validateFlow(flow, index, status))
  asArray(record, "gaps", "$").forEach(validateGap)

  if (status === "owner_signed") {
    const signoff = asRecord(record.ownerSignoff, "$.ownerSignoff")
    asString(signoff, "signerRef", "$.ownerSignoff")
    asString(signoff, "signedAt", "$.ownerSignoff")
    asString(signoff, "methodRef", "$.ownerSignoff")
    optionalString(signoff, "commentRef", "$.ownerSignoff")
  }

  return record
}

if (import.meta.main) {
  const path = Bun.argv[2]
  if (path === undefined) {
    console.error(
      "usage: bun scripts/validate-khala-sync-runtime-dogfood-evidence.ts <bundle.json>",
    )
    process.exit(2)
  }
  try {
    validateKhalaSyncRuntimeDogfoodEvidence(
      JSON.parse(readFileSync(path, "utf8")) as unknown,
    )
    console.log(`ok ${path}`)
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error))
    process.exit(1)
  }
}

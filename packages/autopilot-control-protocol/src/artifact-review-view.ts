export type ArtifactReviewView = {
  outcome: string | null
  editedFileCount: number | null
  commandCount: number | null
  totalTokens: number | null
  devCheckState: string | null
  artifactRef: string | null
  deviations: string[]
}

type RawRecord = Record<string, unknown>

type CountField = "editedFileCount" | "commandCount" | "totalTokens"

const COUNT_ALIASES: Record<CountField, readonly string[]> = {
  editedFileCount: [
    "editedFileCount",
    "edited_file_count",
    "changedFileCount",
    "changed_file_count",
    "fileCount",
    "file_count",
  ],
  commandCount: [
    "commandCount",
    "command_count",
    "commandsRun",
    "commands_run",
    "shellCommandCount",
    "shell_command_count",
  ],
  totalTokens: [
    "totalTokens",
    "total_tokens",
    "tokenCount",
    "token_count",
    "tokens",
  ],
}

export function projectArtifactReview(raw: unknown): ArtifactReviewView {
  const records = candidateRecords(raw)

  return {
    outcome: firstStringFrom(records, ["outcome", "status", "state", "result"]),
    editedFileCount: firstCountFrom(records, "editedFileCount"),
    commandCount: firstCountFrom(records, "commandCount"),
    totalTokens: firstCountFrom(records, "totalTokens"),
    devCheckState: firstDevCheckState(records),
    artifactRef: firstStringFrom(records, [
      "artifactRef",
      "artifact_ref",
      "ref",
      "id",
    ]),
    deviations: deviationsFrom(records),
  }
}

function candidateRecords(raw: unknown): RawRecord[] {
  if (!isRecord(raw)) return []

  const records: RawRecord[] = [raw]
  appendRecord(records, raw.result)
  appendRecord(records, raw.artifact)
  appendRecord(records, raw.executor)
  appendRecord(records, raw.review)
  appendRecord(records, raw.receipt)
  appendRecord(records, raw.closeout)
  appendRecord(records, raw.usage)
  appendRecord(records, raw.stats)
  appendRecord(records, raw.summary)

  const result = readRecord(raw, "result")
  appendRecord(records, result?.artifact)
  appendRecord(records, result?.review)
  appendRecord(records, result?.receipt)
  appendRecord(records, result?.closeout)

  const artifact = readRecord(raw, "artifact") ?? readRecord(result, "artifact")
  appendRecord(records, artifact?.executor)
  appendRecord(records, artifact?.review)
  appendRecord(records, artifact?.receipt)
  appendRecord(records, artifact?.closeout)

  const executor = readRecord(raw, "executor") ?? readRecord(artifact, "executor")
  appendRecord(records, executor?.usage)
  appendRecord(records, executor?.stats)
  appendRecord(records, executor?.summary)

  return records
}

function appendRecord(records: RawRecord[], value: unknown): void {
  if (isRecord(value) && !records.includes(value)) records.push(value)
}

function firstStringFrom(
  records: readonly RawRecord[],
  keys: readonly string[],
): string | null {
  for (const record of records) {
    const value = readFirst(record, keys)
    if (typeof value === "string" && value.trim() !== "") return value.trim()
  }

  return null
}

function firstCountFrom(
  records: readonly RawRecord[],
  field: CountField,
): number | null {
  for (const record of records) {
    const parsed = parseCount(readFirst(record, COUNT_ALIASES[field]))
    if (parsed !== null) return parsed
  }

  return null
}

function firstDevCheckState(records: readonly RawRecord[]): string | null {
  const direct = firstStringFrom(records, [
    "devCheckState",
    "dev_check_state",
    "verifyState",
    "verify_state",
    "checkState",
    "check_state",
  ])
  if (direct !== null) return direct

  for (const record of records) {
    for (const key of ["devCheck", "dev_check", "verify", "verification"]) {
      const nested = readRecord(record, key)
      const state = firstStringFrom(nested === undefined ? [] : [nested], [
        "state",
        "status",
        "outcome",
      ])
      if (state !== null) return state
    }
  }

  return null
}

function deviationsFrom(records: readonly RawRecord[]): string[] {
  const deviations: string[] = []

  for (const record of records) {
    for (const key of ["deviations", "deviationRefs", "deviation_refs"]) {
      appendStrings(deviations, record[key])
    }
    appendStrings(deviations, readRecord(record, "review")?.deviations)
    appendStrings(deviations, readRecord(record, "receipt")?.deviations)
  }

  return unique(deviations)
}

function appendStrings(output: string[], value: unknown): void {
  if (typeof value === "string" && value.trim() !== "") {
    output.push(value.trim())
    return
  }

  if (!Array.isArray(value)) return

  for (const item of value) {
    if (typeof item === "string" && item.trim() !== "") {
      output.push(item.trim())
    }
  }
}

function readFirst(record: RawRecord, keys: readonly string[]): unknown {
  for (const key of keys) {
    if (hasOwn(record, key)) return record[key]
  }

  return undefined
}

function readRecord(value: unknown, key: string): RawRecord | undefined {
  if (!isRecord(value)) return undefined
  const nested = value[key]
  return isRecord(nested) ? nested : undefined
}

function parseCount(value: unknown): number | null {
  if (typeof value === "number") {
    return Number.isSafeInteger(value) && value >= 0 ? value : null
  }

  if (typeof value === "string" && /^\d+$/.test(value.trim())) {
    const parsed = Number(value)
    return Number.isSafeInteger(parsed) ? parsed : null
  }

  return null
}

function isRecord(value: unknown): value is RawRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value)
}

function hasOwn(record: RawRecord, key: string): boolean {
  return Object.prototype.hasOwnProperty.call(record, key)
}

function unique(values: readonly string[]): string[] {
  return Array.from(new Set(values))
}

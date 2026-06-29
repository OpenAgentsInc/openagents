/**
 * trace_record artifact schema v0.1 — the frozen day-0 contract for the
 * Tassadar verified trace factory (issue #4748, RESEARCH_PLAN.md W2).
 *
 * A trace record is the unit of corpus: one digest-pinned execution of a
 * compiled Tassadar ALM numeric workload, carried as compact binary
 * trace tokens in the hot path. Human-readable traces are sampled audit
 * artifacts only — 1B tokens is ~2 GB as uint16 and tens of GB as JSON.
 *
 * Token encoding (trace_token.v0.1): each executor step emits the
 * record's output row (one i64 per output slot). Every i64 is encoded
 * little-endian as four uint16 limbs (or two uint32 limbs at uint32
 * width); the limb byte stream is exactly the byte stream the executor
 * digests, so `full_trace_digest` is recomputable from the token stream
 * alone (Tier 0) and from independent re-execution (Tier 1).
 *
 * Claim boundary: this schema describes verified re-execution artifacts
 * only. It creates no serving, learning, or performance claim.
 */

export const TASSADAR_TRACE_RECORD_SCHEMA_VERSION = 'trace_record.v0.1'
export const TASSADAR_TRACE_PROFILE_VERSION =
  'profile.tassadar_alm_numeric.v1'
export const TASSADAR_TRACE_TOKEN_ENCODING_VERSION = 'trace_token.v0.1'

/** Binary container framing for one encoded record. */
export const TASSADAR_TRACE_RECORD_MAGIC = 'TTRC'
export const TASSADAR_TRACE_RECORD_FORMAT_VERSION = 1

export const TASSADAR_TRACE_TOKEN_WIDTHS = ['uint16', 'uint32'] as const
export type TassadarTraceTokenWidth =
  (typeof TASSADAR_TRACE_TOKEN_WIDTHS)[number]

export const TASSADAR_VALIDATION_TIERS = [0, 1, 2, 3] as const
export type TassadarValidationTier =
  (typeof TASSADAR_VALIDATION_TIERS)[number]

export type TassadarValidatorReceipt = Readonly<{
  verdictSchemaVersion: string
  tier: TassadarValidationTier
  outcome: 'verified' | 'rejected' | 'quarantined'
  classId: string
  validatorDeviceRef: string
  replayedSteps: number
  comparedSteps: number
  validatedAtIso: string
  rejectionKind: string | null
}>

export type TassadarTraceRecord = Readonly<{
  schemaVersion: typeof TASSADAR_TRACE_RECORD_SCHEMA_VERSION
  recordId: string
  profileVersion: string
  familyId: string
  /** graph digest of the compiled numeric model that produced the trace */
  programHash: string
  /** u64 hex seed; with familyId it deterministically regenerates the workload */
  inputSeed: string
  compilerHash: string
  executorHash: string
  stepCount: number
  tokenWidth: TassadarTraceTokenWidth
  traceTokenIds: Uint16Array | Uint32Array
  /** token index where each step starts; length === stepCount */
  stepOffsets: Uint32Array
  finalOutputDigest: string
  fullTraceDigest: string
  validatorReceipts: ReadonlyArray<TassadarValidatorReceipt>
}>

export type TassadarTraceRecordDecodeFailure = Readonly<
  | { kind: 'bad_magic'; detail: string }
  | { kind: 'unsupported_format_version'; detail: string }
  | { kind: 'truncated'; detail: string }
  | { kind: 'invalid_token_width'; detail: string }
  | { kind: 'invalid_step_offsets'; detail: string }
  | { kind: 'invalid_digest_field'; detail: string }
  | { kind: 'invalid_receipt'; detail: string }
>

export type TassadarTraceRecordDecodeResult =
  | Readonly<{ ok: true; record: TassadarTraceRecord; bytesRead: number }>
  | Readonly<{ ok: false; failure: TassadarTraceRecordDecodeFailure }>

const HEX_64 = /^[0-9a-f]{64}$/

export const isWellFormedDigest = (value: string): boolean =>
  HEX_64.test(value)

export const sha256HexOfBytes = async (
  bytes: Uint8Array,
): Promise<string> => {
  const buffer = new ArrayBuffer(bytes.length)
  new Uint8Array(buffer).set(bytes)
  const digest = await crypto.subtle.digest('SHA-256', buffer)

  return [...new Uint8Array(digest)]
    .map(byte => byte.toString(16).padStart(2, '0'))
    .join('')
}

export const sha256HexOfText = async (text: string): Promise<string> =>
  sha256HexOfBytes(new TextEncoder().encode(text))

/**
 * Canonical JSON with lexicographically sorted object keys, used to pin
 * `programHash` for builder-produced numeric models. Encoding, not
 * parsing: the json-boundary rule stays intact.
 */
export const canonicalJson = (value: unknown): string => {
  if (Array.isArray(value)) {
    return `[${value.map(canonicalJson).join(',')}]`
  }

  if (value !== null && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>)
      .filter(([, fieldValue]) => fieldValue !== undefined)
      .sort(([left], [right]) => (left < right ? -1 : 1))
      .map(
        ([key, fieldValue]) =>
          `${JSON.stringify(key)}:${canonicalJson(fieldValue)}`,
      )

    return `{${entries.join(',')}}`
  }

  return JSON.stringify(value)
}

const limbsPerValue = (width: TassadarTraceTokenWidth): number =>
  width === 'uint16' ? 4 : 2

/**
 * trace_token.v0.1: step output rows to the compact token stream plus
 * step offsets. Each i64 output value becomes little-endian limbs.
 */
export const traceTokensFromStepOutputs = (
  stepOutputs: ReadonlyArray<ReadonlyArray<bigint>>,
  width: TassadarTraceTokenWidth = 'uint16',
): Readonly<{
  tokens: Uint16Array | Uint32Array
  stepOffsets: Uint32Array
}> => {
  const limbs = limbsPerValue(width)
  const limbBits = width === 'uint16' ? 16n : 32n
  const limbMask = width === 'uint16' ? 0xffffn : 0xffffffffn
  let tokenCount = 0
  for (const row of stepOutputs) tokenCount += row.length * limbs
  const tokens =
    width === 'uint16' ? new Uint16Array(tokenCount) : new Uint32Array(tokenCount)
  const stepOffsets = new Uint32Array(stepOutputs.length)
  let cursor = 0
  stepOutputs.forEach((row, stepIndex) => {
    stepOffsets[stepIndex] = cursor
    for (const value of row) {
      let unsigned = BigInt.asUintN(64, value)
      for (let limb = 0; limb < limbs; limb += 1) {
        tokens[cursor] = Number(unsigned & limbMask)
        cursor += 1
        unsigned >>= limbBits
      }
    }
  })

  return { stepOffsets, tokens }
}

/** Reconstructs the executor's per-step i64 rows from the token stream. */
export const stepOutputsFromTraceTokens = (
  tokens: Uint16Array | Uint32Array,
  stepOffsets: Uint32Array,
  width: TassadarTraceTokenWidth,
): ReadonlyArray<ReadonlyArray<bigint>> => {
  const limbs = limbsPerValue(width)
  const limbBits = width === 'uint16' ? 16n : 32n
  const rows: Array<ReadonlyArray<bigint>> = []
  for (let step = 0; step < stepOffsets.length; step += 1) {
    const start = stepOffsets[step] ?? 0
    const end =
      step + 1 < stepOffsets.length
        ? (stepOffsets[step + 1] ?? tokens.length)
        : tokens.length
    const row: Array<bigint> = []
    for (let cursor = start; cursor + limbs <= end; cursor += limbs) {
      let unsigned = 0n
      for (let limb = limbs - 1; limb >= 0; limb -= 1) {
        unsigned = (unsigned << limbBits) | BigInt(tokens[cursor + limb] ?? 0)
      }
      row.push(BigInt.asIntN(64, unsigned))
    }
    rows.push(row)
  }

  return rows
}

const textBytes = (value: string): Uint8Array =>
  new TextEncoder().encode(value)

const i64LeBytes = (value: bigint): Uint8Array => {
  const bytes = new Uint8Array(8)
  let unsigned = BigInt.asUintN(64, value)
  for (let index = 0; index < 8; index += 1) {
    bytes[index] = Number(unsigned & 0xffn)
    unsigned >>= 8n
  }

  return bytes
}

const concatBytes = (chunks: ReadonlyArray<Uint8Array>): Uint8Array => {
  let total = 0
  for (const chunk of chunks) total += chunk.length
  const joined = new Uint8Array(total)
  let offset = 0
  for (const chunk of chunks) {
    joined.set(chunk, offset)
    offset += chunk.length
  }

  return joined
}

/**
 * Recomputes the executor trace digest from the token stream alone —
 * byte-for-byte the `tassadar_alm_trace|<graph digest>|row|...` stream
 * the TS and Rust executors hash. This is the Tier 0 hash check.
 */
export const fullTraceDigestFromTokens = (
  programHash: string,
  tokens: Uint16Array | Uint32Array,
  stepOffsets: Uint32Array,
  width: TassadarTraceTokenWidth,
): Promise<string> => {
  const rows = stepOutputsFromTraceTokens(tokens, stepOffsets, width)
  const chunks: Array<Uint8Array> = [
    textBytes('tassadar_alm_trace|'),
    textBytes(programHash),
  ]
  for (const row of rows) {
    chunks.push(textBytes('|row|'))
    for (const value of row) chunks.push(i64LeBytes(value))
  }

  return sha256HexOfBytes(concatBytes(chunks))
}

/** Digest over the final output row only, pinned to the program hash. */
export const finalOutputDigestFromTokens = (
  programHash: string,
  tokens: Uint16Array | Uint32Array,
  stepOffsets: Uint32Array,
  width: TassadarTraceTokenWidth,
): Promise<string> => {
  const rows = stepOutputsFromTraceTokens(tokens, stepOffsets, width)
  const finalRow = rows.length > 0 ? rows[rows.length - 1] : undefined
  const chunks: Array<Uint8Array> = [
    textBytes('tassadar_alm_trace_final|'),
    textBytes(programHash),
    textBytes('|row|'),
  ]
  for (const value of finalRow ?? []) chunks.push(i64LeBytes(value))

  return sha256HexOfBytes(concatBytes(chunks))
}

export const traceRecordIdFor = async (
  input: Readonly<{
    programHash: string
    inputSeed: string
    stepCount: number
  }>,
): Promise<string> => {
  const digest = await sha256HexOfText(
    `${TASSADAR_TRACE_RECORD_SCHEMA_VERSION}|${input.programHash}|${input.inputSeed}|${input.stepCount}`,
  )

  return `trace_${digest.slice(0, 24)}`
}

class ByteWriter {
  private chunks: Array<Uint8Array> = []

  bytes(value: Uint8Array): void {
    this.chunks.push(value)
  }

  u8(value: number): void {
    this.chunks.push(Uint8Array.of(value & 0xff))
  }

  u16(value: number): void {
    const bytes = new Uint8Array(2)
    new DataView(bytes.buffer).setUint16(0, value, true)
    this.chunks.push(bytes)
  }

  u32(value: number): void {
    const bytes = new Uint8Array(4)
    new DataView(bytes.buffer).setUint32(0, value >>> 0, true)
    this.chunks.push(bytes)
  }

  string(value: string): void {
    const encoded = textBytes(value)
    this.u32(encoded.length)
    this.chunks.push(encoded)
  }

  finish(): Uint8Array {
    return concatBytes(this.chunks)
  }
}

type ReaderFailure = Readonly<{ kind: 'truncated'; detail: string }>

class ByteReader {
  private offset: number
  private view: DataView

  constructor(
    private bytes: Uint8Array,
    start: number,
  ) {
    this.offset = start
    this.view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength)
  }

  position(): number {
    return this.offset
  }

  private ensure(length: number, what: string): ReaderFailure | null {
    return this.offset + length > this.bytes.length
      ? {
          detail: `${what} needs ${length} bytes at offset ${this.offset}, only ${this.bytes.length - this.offset} remain`,
          kind: 'truncated',
        }
      : null
  }

  u8(what: string): number | ReaderFailure {
    const failure = this.ensure(1, what)
    if (failure !== null) return failure
    const value = this.view.getUint8(this.offset)
    this.offset += 1

    return value
  }

  u16(what: string): number | ReaderFailure {
    const failure = this.ensure(2, what)
    if (failure !== null) return failure
    const value = this.view.getUint16(this.offset, true)
    this.offset += 2

    return value
  }

  u32(what: string): number | ReaderFailure {
    const failure = this.ensure(4, what)
    if (failure !== null) return failure
    const value = this.view.getUint32(this.offset, true)
    this.offset += 4

    return value
  }

  raw(length: number, what: string): Uint8Array | ReaderFailure {
    const failure = this.ensure(length, what)
    if (failure !== null) return failure
    const value = this.bytes.slice(this.offset, this.offset + length)
    this.offset += length

    return value
  }

  string(what: string): string | ReaderFailure {
    const length = this.u32(`${what} length`)
    if (typeof length !== 'number') return length
    const raw = this.raw(length, what)
    if (raw instanceof Uint8Array) return new TextDecoder().decode(raw)

    return raw
  }
}

const isFailure = (value: unknown): value is ReaderFailure =>
  typeof value === 'object' &&
  value !== null &&
  'kind' in value &&
  (value as { kind: unknown }).kind === 'truncated'

const RECEIPT_OUTCOMES = ['verified', 'rejected', 'quarantined'] as const

/** Encodes one trace record into the TTRC v1 compact binary container. */
export const encodeTraceRecord = (record: TassadarTraceRecord): Uint8Array => {
  const writer = new ByteWriter()
  writer.bytes(textBytes(TASSADAR_TRACE_RECORD_MAGIC))
  writer.u16(TASSADAR_TRACE_RECORD_FORMAT_VERSION)
  writer.string(record.schemaVersion)
  writer.string(record.recordId)
  writer.string(record.profileVersion)
  writer.string(record.familyId)
  writer.string(record.programHash)
  writer.string(record.inputSeed)
  writer.string(record.compilerHash)
  writer.string(record.executorHash)
  writer.u8(record.tokenWidth === 'uint16' ? 2 : 4)
  writer.u32(record.stepCount)
  writer.u32(record.traceTokenIds.length)
  for (const offset of record.stepOffsets) writer.u32(offset)
  if (record.tokenWidth === 'uint16') {
    for (const token of record.traceTokenIds) writer.u16(token)
  } else {
    for (const token of record.traceTokenIds) writer.u32(token)
  }
  writer.string(record.finalOutputDigest)
  writer.string(record.fullTraceDigest)
  writer.u16(record.validatorReceipts.length)
  for (const receipt of record.validatorReceipts) {
    writer.string(receipt.verdictSchemaVersion)
    writer.u8(receipt.tier)
    writer.u8(RECEIPT_OUTCOMES.indexOf(receipt.outcome))
    writer.string(receipt.classId)
    writer.string(receipt.validatorDeviceRef)
    writer.u32(receipt.replayedSteps)
    writer.u32(receipt.comparedSteps)
    writer.string(receipt.validatedAtIso)
    writer.string(receipt.rejectionKind ?? '')
  }

  return writer.finish()
}

/**
 * Decodes one TTRC record starting at `start`. Every failure is typed;
 * nothing throws on malformed input.
 */
export const decodeTraceRecord = (
  bytes: Uint8Array,
  start = 0,
): TassadarTraceRecordDecodeResult => {
  const reader = new ByteReader(bytes, start)
  const magic = reader.raw(4, 'magic')
  if (isFailure(magic)) return { failure: magic, ok: false }
  const magicText = new TextDecoder().decode(magic)
  if (magicText !== TASSADAR_TRACE_RECORD_MAGIC) {
    return {
      failure: {
        detail: `expected ${TASSADAR_TRACE_RECORD_MAGIC}, found ${JSON.stringify(magicText)}`,
        kind: 'bad_magic',
      },
      ok: false,
    }
  }
  const formatVersion = reader.u16('format version')
  if (isFailure(formatVersion)) return { failure: formatVersion, ok: false }
  if (formatVersion !== TASSADAR_TRACE_RECORD_FORMAT_VERSION) {
    return {
      failure: {
        detail: `format version ${formatVersion} is not supported (expected ${TASSADAR_TRACE_RECORD_FORMAT_VERSION})`,
        kind: 'unsupported_format_version',
      },
      ok: false,
    }
  }
  const strings: Array<string> = []
  for (const field of [
    'schemaVersion',
    'recordId',
    'profileVersion',
    'familyId',
    'programHash',
    'inputSeed',
    'compilerHash',
    'executorHash',
  ]) {
    const value = reader.string(field)
    if (isFailure(value)) return { failure: value, ok: false }
    strings.push(value)
  }
  const [
    schemaVersion,
    recordId,
    profileVersion,
    familyId,
    programHash,
    inputSeed,
    compilerHash,
    executorHash,
  ] = strings
  if (schemaVersion !== TASSADAR_TRACE_RECORD_SCHEMA_VERSION) {
    return {
      failure: {
        detail: `schema version ${JSON.stringify(schemaVersion)} is not ${TASSADAR_TRACE_RECORD_SCHEMA_VERSION}`,
        kind: 'unsupported_format_version',
      },
      ok: false,
    }
  }
  const widthByte = reader.u8('token width')
  if (isFailure(widthByte)) return { failure: widthByte, ok: false }
  if (widthByte !== 2 && widthByte !== 4) {
    return {
      failure: {
        detail: `token width byte ${widthByte} is not 2 (uint16) or 4 (uint32)`,
        kind: 'invalid_token_width',
      },
      ok: false,
    }
  }
  const tokenWidth: TassadarTraceTokenWidth =
    widthByte === 2 ? 'uint16' : 'uint32'
  const stepCount = reader.u32('step count')
  if (isFailure(stepCount)) return { failure: stepCount, ok: false }
  const tokenCount = reader.u32('token count')
  if (isFailure(tokenCount)) return { failure: tokenCount, ok: false }
  const offsetsRaw = reader.raw(stepCount * 4, 'step offsets')
  if (isFailure(offsetsRaw)) return { failure: offsetsRaw, ok: false }
  const stepOffsets = new Uint32Array(stepCount)
  const offsetsView = new DataView(
    offsetsRaw.buffer,
    offsetsRaw.byteOffset,
    offsetsRaw.byteLength,
  )
  for (let index = 0; index < stepCount; index += 1) {
    stepOffsets[index] = offsetsView.getUint32(index * 4, true)
  }
  for (let index = 0; index < stepCount; index += 1) {
    const current = stepOffsets[index] ?? 0
    const previous = index > 0 ? (stepOffsets[index - 1] ?? 0) : 0
    if (current > tokenCount || (index > 0 && current < previous)) {
      return {
        failure: {
          detail: `step offset ${current} at index ${index} is out of bounds or non-monotone for ${tokenCount} tokens`,
          kind: 'invalid_step_offsets',
        },
        ok: false,
      }
    }
  }
  const tokenBytes = reader.raw(tokenCount * widthByte, 'trace tokens')
  if (isFailure(tokenBytes)) return { failure: tokenBytes, ok: false }
  const tokenView = new DataView(
    tokenBytes.buffer,
    tokenBytes.byteOffset,
    tokenBytes.byteLength,
  )
  const traceTokenIds =
    tokenWidth === 'uint16'
      ? new Uint16Array(tokenCount)
      : new Uint32Array(tokenCount)
  for (let index = 0; index < tokenCount; index += 1) {
    traceTokenIds[index] =
      tokenWidth === 'uint16'
        ? tokenView.getUint16(index * 2, true)
        : tokenView.getUint32(index * 4, true)
  }
  const finalOutputDigest = reader.string('final output digest')
  if (isFailure(finalOutputDigest)) {
    return { failure: finalOutputDigest, ok: false }
  }
  const fullTraceDigest = reader.string('full trace digest')
  if (isFailure(fullTraceDigest)) return { failure: fullTraceDigest, ok: false }
  if (
    !isWellFormedDigest(finalOutputDigest) ||
    !isWellFormedDigest(fullTraceDigest)
  ) {
    return {
      failure: {
        detail: 'final/full trace digests must be 64 lowercase hex characters',
        kind: 'invalid_digest_field',
      },
      ok: false,
    }
  }
  const receiptCount = reader.u16('receipt count')
  if (isFailure(receiptCount)) return { failure: receiptCount, ok: false }
  const validatorReceipts: Array<TassadarValidatorReceipt> = []
  for (let index = 0; index < receiptCount; index += 1) {
    const verdictSchemaVersion = reader.string('receipt schema version')
    if (isFailure(verdictSchemaVersion)) {
      return { failure: verdictSchemaVersion, ok: false }
    }
    const tier = reader.u8('receipt tier')
    if (isFailure(tier)) return { failure: tier, ok: false }
    const outcomeIndex = reader.u8('receipt outcome')
    if (isFailure(outcomeIndex)) return { failure: outcomeIndex, ok: false }
    const classId = reader.string('receipt class id')
    if (isFailure(classId)) return { failure: classId, ok: false }
    const validatorDeviceRef = reader.string('receipt validator device')
    if (isFailure(validatorDeviceRef)) {
      return { failure: validatorDeviceRef, ok: false }
    }
    const replayedSteps = reader.u32('receipt replayed steps')
    if (isFailure(replayedSteps)) return { failure: replayedSteps, ok: false }
    const comparedSteps = reader.u32('receipt compared steps')
    if (isFailure(comparedSteps)) return { failure: comparedSteps, ok: false }
    const validatedAtIso = reader.string('receipt validated at')
    if (isFailure(validatedAtIso)) return { failure: validatedAtIso, ok: false }
    const rejectionKind = reader.string('receipt rejection kind')
    if (isFailure(rejectionKind)) return { failure: rejectionKind, ok: false }
    const outcome = RECEIPT_OUTCOMES[outcomeIndex]
    if (
      outcome === undefined ||
      tier < 0 ||
      tier > 3 ||
      !TASSADAR_VALIDATION_TIERS.includes(tier as TassadarValidationTier)
    ) {
      return {
        failure: {
          detail: `receipt ${index} carries outcome index ${outcomeIndex} / tier ${tier} outside the v0.1 enums`,
          kind: 'invalid_receipt',
        },
        ok: false,
      }
    }
    validatorReceipts.push({
      classId,
      comparedSteps,
      outcome,
      rejectionKind: rejectionKind === '' ? null : rejectionKind,
      replayedSteps,
      tier: tier as TassadarValidationTier,
      validatedAtIso,
      validatorDeviceRef,
      verdictSchemaVersion,
    })
  }

  return {
    bytesRead: reader.position() - start,
    ok: true,
    record: {
      compilerHash: compilerHash ?? '',
      executorHash: executorHash ?? '',
      familyId: familyId ?? '',
      finalOutputDigest,
      fullTraceDigest,
      inputSeed: inputSeed ?? '',
      profileVersion: profileVersion ?? '',
      programHash: programHash ?? '',
      recordId: recordId ?? '',
      schemaVersion: TASSADAR_TRACE_RECORD_SCHEMA_VERSION,
      stepCount,
      stepOffsets,
      tokenWidth,
      traceTokenIds,
      validatorReceipts,
    },
  }
}

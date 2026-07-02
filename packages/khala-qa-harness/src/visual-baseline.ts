import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdir, readFile, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import { deflateSync, inflateSync } from "node:zlib"

export const KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA =
  "openagents.khala_visual_baselines.v1"

export type KhalaVisualBaselineColorScheme = "dark" | "light" | "no-preference"

export type KhalaVisualBaselineReducedMotion = "reduce" | "no-preference"

export type KhalaVisualBaselineEntry = Readonly<{
  colorScheme: KhalaVisualBaselineColorScheme
  harness: string
  height: number
  id: string
  redactionCheckedAt: string
  reducedMotion: KhalaVisualBaselineReducedMotion
  screenshot: string
  sha256: string
  viewport: string
  width: number
}>

export type KhalaVisualBaselineManifest = Readonly<{
  entries: ReadonlyArray<KhalaVisualBaselineEntry>
  schema: typeof KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA
}>

export type KhalaVisualBaselineCapture = Readonly<{
  colorScheme?: KhalaVisualBaselineColorScheme
  harness: string
  id: string
  reducedMotion?: KhalaVisualBaselineReducedMotion
  screenshotPath: string
  viewport: string
}>

export type KhalaVisualBaselineStatus =
  | "blessed"
  | "changed"
  | "matched"
  | "missing"

export type KhalaVisualBaselineResult = Readonly<{
  baseline: string
  baselineSha256?: string
  candidateSha256: string
  colorScheme: KhalaVisualBaselineColorScheme
  delta?: string
  diffPixels?: number
  harness: string
  height: number
  id: string
  ok: boolean
  reducedMotion: KhalaVisualBaselineReducedMotion
  schema: typeof KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA
  status: KhalaVisualBaselineStatus
  totalPixels?: number
  viewport: string
  width: number
}>

export type KhalaVisualBaselineEvaluateInput = Readonly<{
  baselineDir: string
  bless?: boolean | undefined
  capture: KhalaVisualBaselineCapture
  now?: () => string
  requireBaseline?: boolean | undefined
}>

type DecodedPng = Readonly<{
  height: number
  pixels: Uint8Array
  width: number
}>

const pngSignature = Buffer.from([
  0x89,
  0x50,
  0x4e,
  0x47,
  0x0d,
  0x0a,
  0x1a,
  0x0a,
])

const safeIdPattern = /^[a-z0-9][a-z0-9._-]{0,191}$/
const publicUnsafePattern =
  /\/Users\/|\/home\/|\\Users\\|auth\.json|bearer|credential|provider[_-]?payload|raw[_-]?(prompt|trace|log|provider)|secret|sk-[a-z0-9]/i

export async function evaluateKhalaVisualBaseline(
  input: KhalaVisualBaselineEvaluateInput,
): Promise<KhalaVisualBaselineResult> {
  assertCapturePublicSafe(input.capture)
  const candidateBytes = await readFile(input.capture.screenshotPath)
  const candidate = decodePng(candidateBytes)
  const candidateSha256 = sha256(candidateBytes)
  const baseline = baselineScreenshotPath(input.capture.id)
  const baseResult = {
    baseline,
    candidateSha256,
    colorScheme: input.capture.colorScheme ?? "dark",
    harness: input.capture.harness,
    height: candidate.height,
    id: input.capture.id,
    reducedMotion: input.capture.reducedMotion ?? "no-preference",
    schema: KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA,
    viewport: input.capture.viewport,
    width: candidate.width,
  } satisfies Omit<KhalaVisualBaselineResult, "ok" | "status">

  if (input.bless === true) {
    const manifest = await readKhalaVisualBaselineManifest(input.baselineDir)
    const entry = makeEntry({
      capture: input.capture,
      candidate,
      candidateSha256,
      ...(input.now === undefined ? {} : { now: input.now }),
    })
    await mkdir(dirname(join(input.baselineDir, entry.screenshot)), { recursive: true })
    await writeFile(join(input.baselineDir, entry.screenshot), candidateBytes)
    await writeKhalaVisualBaselineManifest(input.baselineDir, {
      entries: [
        ...manifest.entries.filter(existing => existing.id !== entry.id),
        entry,
      ].sort((left, right) => left.id.localeCompare(right.id)),
      schema: KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA,
    })
    return {
      ...baseResult,
      baselineSha256: candidateSha256,
      ok: true,
      status: "blessed",
    }
  }

  const manifest = await readKhalaVisualBaselineManifest(input.baselineDir)
  const entry = manifest.entries.find(candidateEntry => candidateEntry.id === input.capture.id)
  if (entry === undefined || !existsSync(join(input.baselineDir, entry.screenshot))) {
    return {
      ...baseResult,
      ok: input.requireBaseline !== true,
      status: "missing",
    }
  }

  assertEntryMatchesCapture(entry, input.capture)
  const baselineBytes = await readFile(join(input.baselineDir, entry.screenshot))
  const baselinePng = decodePng(baselineBytes)
  const baselineSha256 = sha256(baselineBytes)
  if (
    baselinePng.width === candidate.width &&
    baselinePng.height === candidate.height &&
    pixelsEqual(baselinePng.pixels, candidate.pixels)
  ) {
    return {
      ...baseResult,
      baselineSha256,
      ok: true,
      status: "matched",
    }
  }

  const delta = deltaScreenshotPath(input.capture.id)
  const deltaPixels = makeDeltaPixels(baselinePng, candidate)
  await mkdir(dirname(join(input.baselineDir, delta)), { recursive: true })
  await writeFile(join(input.baselineDir, delta), encodeRgbaPng(deltaPixels))

  return {
    ...baseResult,
    baselineSha256,
    delta,
    diffPixels: deltaPixels.diffPixels,
    ok: false,
    status: "changed",
    totalPixels: deltaPixels.width * deltaPixels.height,
  }
}

export async function assertKhalaVisualBaseline(
  input: KhalaVisualBaselineEvaluateInput,
): Promise<KhalaVisualBaselineResult> {
  const result = await evaluateKhalaVisualBaseline(input)
  if (result.ok) return result
  const suffix =
    result.status === "changed" && result.delta !== undefined
      ? `; delta image: ${result.delta}`
      : ""
  throw new Error(
    `Khala visual baseline ${result.status} for ${result.id}${suffix}`,
  )
}

export async function readKhalaVisualBaselineManifest(
  baselineDir: string,
): Promise<KhalaVisualBaselineManifest> {
  const manifestPath = join(baselineDir, "manifest.json")
  if (!existsSync(manifestPath)) {
    return { entries: [], schema: KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA }
  }
  const parsed = JSON.parse(await readFile(manifestPath, "utf8")) as unknown
  assertManifestPublicSafe(parsed)
  return parsed
}

export async function writeKhalaVisualBaselineManifest(
  baselineDir: string,
  manifest: KhalaVisualBaselineManifest,
): Promise<void> {
  assertManifestPublicSafe(manifest)
  await mkdir(baselineDir, { recursive: true })
  await writeFile(
    join(baselineDir, "manifest.json"),
    `${JSON.stringify(manifest, null, 2)}\n`,
  )
}

const makeEntry = (
  input: Readonly<{
    capture: KhalaVisualBaselineCapture
    candidate: DecodedPng
    candidateSha256: string
    now?: () => string
  }>,
): KhalaVisualBaselineEntry => ({
  colorScheme: input.capture.colorScheme ?? "dark",
  harness: input.capture.harness,
  height: input.candidate.height,
  id: input.capture.id,
  redactionCheckedAt: input.now?.() ?? new Date().toISOString(),
  reducedMotion: input.capture.reducedMotion ?? "no-preference",
  screenshot: baselineScreenshotPath(input.capture.id),
  sha256: input.candidateSha256,
  viewport: input.capture.viewport,
  width: input.candidate.width,
})

const baselineScreenshotPath = (id: string): string => `screenshots/${id}.png`

const deltaScreenshotPath = (id: string): string => `deltas/${id}.delta.png`

const assertCapturePublicSafe = (capture: KhalaVisualBaselineCapture): void => {
  assertSafeId("visual baseline id", capture.id)
  assertSafeId("visual baseline harness", capture.harness)
  assertSafeId("visual baseline viewport", capture.viewport)
}

const assertEntryMatchesCapture = (
  entry: KhalaVisualBaselineEntry,
  capture: KhalaVisualBaselineCapture,
): void => {
  if (entry.harness !== capture.harness) {
    throw new Error(`baseline ${entry.id} harness mismatch`)
  }
  if (entry.viewport !== capture.viewport) {
    throw new Error(`baseline ${entry.id} viewport mismatch`)
  }
  if (entry.colorScheme !== (capture.colorScheme ?? "dark")) {
    throw new Error(`baseline ${entry.id} color-scheme mismatch`)
  }
  if (entry.reducedMotion !== (capture.reducedMotion ?? "no-preference")) {
    throw new Error(`baseline ${entry.id} reduced-motion mismatch`)
  }
}

const assertSafeId = (label: string, value: string): void => {
  if (!safeIdPattern.test(value) || publicUnsafePattern.test(value)) {
    throw new Error(`${label} is not public-safe: ${value}`)
  }
}

function assertManifestPublicSafe(
  value: unknown,
): asserts value is KhalaVisualBaselineManifest {
  if (typeof value !== "object" || value === null) {
    throw new Error("visual baseline manifest must be an object")
  }
  const manifest = value as { entries?: unknown; schema?: unknown }
  if (manifest.schema !== KHALA_VISUAL_BASELINE_MANIFEST_SCHEMA) {
    throw new Error("visual baseline manifest schema mismatch")
  }
  if (!Array.isArray(manifest.entries)) {
    throw new Error("visual baseline manifest entries must be an array")
  }
  for (const entry of manifest.entries) {
    assertManifestEntryPublicSafe(entry)
  }
}

function assertManifestEntryPublicSafe(
  value: unknown,
): asserts value is KhalaVisualBaselineEntry {
  if (typeof value !== "object" || value === null) {
    throw new Error("visual baseline manifest entry must be an object")
  }
  const entry = value as Record<string, unknown>
  for (const field of ["id", "harness", "viewport"] as const) {
    const text = entry[field]
    if (typeof text !== "string") {
      throw new Error(`visual baseline ${field} must be a string`)
    }
    assertSafeId(`visual baseline ${field}`, text)
  }
  if (entry.colorScheme !== "dark" && entry.colorScheme !== "light" && entry.colorScheme !== "no-preference") {
    throw new Error("visual baseline colorScheme mismatch")
  }
  if (entry.reducedMotion !== "reduce" && entry.reducedMotion !== "no-preference") {
    throw new Error("visual baseline reducedMotion mismatch")
  }
  if (
    typeof entry.screenshot !== "string" ||
    !entry.screenshot.startsWith("screenshots/") ||
    !entry.screenshot.endsWith(".png") ||
    entry.screenshot.includes("..") ||
    publicUnsafePattern.test(entry.screenshot)
  ) {
    throw new Error("visual baseline screenshot path is not public-safe")
  }
  if (typeof entry.sha256 !== "string" || !/^[a-f0-9]{64}$/.test(entry.sha256)) {
    throw new Error("visual baseline sha256 is invalid")
  }
  for (const field of ["height", "width"] as const) {
    if (!Number.isInteger(entry[field]) || Number(entry[field]) < 1) {
      throw new Error(`visual baseline ${field} is invalid`)
    }
  }
  if (
    typeof entry.redactionCheckedAt !== "string" ||
    publicUnsafePattern.test(entry.redactionCheckedAt)
  ) {
    throw new Error("visual baseline redaction timestamp is not public-safe")
  }
}

const sha256 = (bytes: Uint8Array): string =>
  createHash("sha256").update(bytes).digest("hex")

const decodePng = (bytes: Uint8Array): DecodedPng => {
  if (bytes.length < pngSignature.length || !Buffer.from(bytes.subarray(0, pngSignature.length)).equals(pngSignature)) {
    throw new Error("visual baseline screenshot is not a PNG")
  }

  let offset = pngSignature.length
  let width = 0
  let height = 0
  let bitDepth = 0
  let colorType = 0
  const idatChunks: Buffer[] = []
  while (offset < bytes.length) {
    if (offset + 12 > bytes.length) throw new Error("truncated PNG chunk")
    const length = readUInt32(bytes, offset)
    const type = Buffer.from(bytes.subarray(offset + 4, offset + 8)).toString("ascii")
    const dataStart = offset + 8
    const dataEnd = dataStart + length
    if (dataEnd + 4 > bytes.length) throw new Error("truncated PNG data")
    const data = Buffer.from(bytes.subarray(dataStart, dataEnd))
    if (type === "IHDR") {
      width = data.readUInt32BE(0)
      height = data.readUInt32BE(4)
      bitDepth = data[8] ?? 0
      colorType = data[9] ?? 0
      if (data[10] !== 0 || data[11] !== 0 || data[12] !== 0) {
        throw new Error("unsupported PNG compression/filter/interlace mode")
      }
    } else if (type === "IDAT") {
      idatChunks.push(data)
    } else if (type === "IEND") {
      break
    }
    offset = dataEnd + 4
  }

  if (width < 1 || height < 1) throw new Error("PNG is missing dimensions")
  if (bitDepth !== 8) throw new Error("visual baseline supports only 8-bit PNGs")
  const bytesPerPixel = colorTypeBytesPerPixel(colorType)
  const inflated = inflateSync(Buffer.concat(idatChunks))
  const stride = width * bytesPerPixel
  const expectedLength = (stride + 1) * height
  if (inflated.length < expectedLength) throw new Error("PNG pixel payload is truncated")
  const raw = new Uint8Array(stride * height)
  for (let y = 0; y < height; y += 1) {
    const filter = inflated[y * (stride + 1)] ?? 0
    const rowStart = y * (stride + 1) + 1
    const outStart = y * stride
    for (let x = 0; x < stride; x += 1) {
      const current = inflated[rowStart + x] ?? 0
      const left = x >= bytesPerPixel ? raw[outStart + x - bytesPerPixel] ?? 0 : 0
      const up = y > 0 ? raw[outStart + x - stride] ?? 0 : 0
      const upLeft =
        y > 0 && x >= bytesPerPixel
          ? raw[outStart + x - stride - bytesPerPixel] ?? 0
          : 0
      raw[outStart + x] = unfilterByte(filter, current, left, up, upLeft)
    }
  }

  return {
    height,
    pixels: convertToRgba(raw, width, height, colorType),
    width,
  }
}

const readUInt32 = (bytes: Uint8Array, offset: number): number =>
  ((bytes[offset] ?? 0) * 0x1000000) +
  ((bytes[offset + 1] ?? 0) << 16) +
  ((bytes[offset + 2] ?? 0) << 8) +
  (bytes[offset + 3] ?? 0)

const colorTypeBytesPerPixel = (colorType: number): number => {
  switch (colorType) {
    case 0:
      return 1
    case 2:
      return 3
    case 4:
      return 2
    case 6:
      return 4
    default:
      throw new Error(`unsupported PNG color type: ${colorType}`)
  }
}

const unfilterByte = (
  filter: number,
  current: number,
  left: number,
  up: number,
  upLeft: number,
): number => {
  switch (filter) {
    case 0:
      return current
    case 1:
      return (current + left) & 0xff
    case 2:
      return (current + up) & 0xff
    case 3:
      return (current + Math.floor((left + up) / 2)) & 0xff
    case 4:
      return (current + paeth(left, up, upLeft)) & 0xff
    default:
      throw new Error(`unsupported PNG filter: ${filter}`)
  }
}

const paeth = (left: number, up: number, upLeft: number): number => {
  const p = left + up - upLeft
  const pa = Math.abs(p - left)
  const pb = Math.abs(p - up)
  const pc = Math.abs(p - upLeft)
  if (pa <= pb && pa <= pc) return left
  if (pb <= pc) return up
  return upLeft
}

const convertToRgba = (
  raw: Uint8Array,
  width: number,
  height: number,
  colorType: number,
): Uint8Array => {
  const pixels = new Uint8Array(width * height * 4)
  const sourceStep = colorTypeBytesPerPixel(colorType)
  for (let source = 0, target = 0; target < pixels.length; source += sourceStep, target += 4) {
    if (colorType === 0) {
      const value = raw[source] ?? 0
      pixels[target] = value
      pixels[target + 1] = value
      pixels[target + 2] = value
      pixels[target + 3] = 255
    } else if (colorType === 2) {
      pixels[target] = raw[source] ?? 0
      pixels[target + 1] = raw[source + 1] ?? 0
      pixels[target + 2] = raw[source + 2] ?? 0
      pixels[target + 3] = 255
    } else if (colorType === 4) {
      const value = raw[source] ?? 0
      pixels[target] = value
      pixels[target + 1] = value
      pixels[target + 2] = value
      pixels[target + 3] = raw[source + 1] ?? 255
    } else {
      pixels[target] = raw[source] ?? 0
      pixels[target + 1] = raw[source + 1] ?? 0
      pixels[target + 2] = raw[source + 2] ?? 0
      pixels[target + 3] = raw[source + 3] ?? 255
    }
  }
  return pixels
}

const pixelsEqual = (left: Uint8Array, right: Uint8Array): boolean => {
  if (left.length !== right.length) return false
  for (let index = 0; index < left.length; index += 1) {
    if (left[index] !== right[index]) return false
  }
  return true
}

const makeDeltaPixels = (
  baseline: DecodedPng,
  candidate: DecodedPng,
): Readonly<{
  diffPixels: number
  height: number
  pixels: Uint8Array
  width: number
}> => {
  const width = candidate.width
  const height = candidate.height
  const pixels = new Uint8Array(width * height * 4)
  let diffPixels = 0
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const target = (y * width + x) * 4
      const baselineOffset = (y * baseline.width + x) * 4
      const inBaseline = x < baseline.width && y < baseline.height
      const differs =
        !inBaseline ||
        baseline.pixels[baselineOffset] !== candidate.pixels[target] ||
        baseline.pixels[baselineOffset + 1] !== candidate.pixels[target + 1] ||
        baseline.pixels[baselineOffset + 2] !== candidate.pixels[target + 2] ||
        baseline.pixels[baselineOffset + 3] !== candidate.pixels[target + 3]
      if (differs) {
        pixels[target] = 255
        pixels[target + 1] = 0
        pixels[target + 2] = 96
        pixels[target + 3] = 255
        diffPixels += 1
      } else {
        pixels[target] = candidate.pixels[target]
        pixels[target + 1] = candidate.pixels[target + 1]
        pixels[target + 2] = candidate.pixels[target + 2]
        pixels[target + 3] = 40
      }
    }
  }
  return { diffPixels, height, pixels, width }
}

const encodeRgbaPng = (
  input: Readonly<{
    height: number
    pixels: Uint8Array
    width: number
  }>,
): Buffer => {
  const stride = input.width * 4
  const raw = Buffer.alloc((stride + 1) * input.height)
  for (let y = 0; y < input.height; y += 1) {
    raw[y * (stride + 1)] = 0
    raw.set(
      input.pixels.subarray(y * stride, (y + 1) * stride),
      y * (stride + 1) + 1,
    )
  }
  const ihdr = Buffer.alloc(13)
  ihdr.writeUInt32BE(input.width, 0)
  ihdr.writeUInt32BE(input.height, 4)
  ihdr[8] = 8
  ihdr[9] = 6
  ihdr[10] = 0
  ihdr[11] = 0
  ihdr[12] = 0
  return Buffer.concat([
    pngSignature,
    pngChunk("IHDR", ihdr),
    pngChunk("IDAT", deflateSync(raw)),
    pngChunk("IEND", Buffer.alloc(0)),
  ])
}

const pngChunk = (type: string, data: Buffer): Buffer => {
  const typeBuffer = Buffer.from(type, "ascii")
  const length = Buffer.alloc(4)
  length.writeUInt32BE(data.length, 0)
  const crc = Buffer.alloc(4)
  crc.writeUInt32BE(crc32(Buffer.concat([typeBuffer, data])), 0)
  return Buffer.concat([length, typeBuffer, data, crc])
}

const crcTable = Array.from({ length: 256 }, (_, index) => {
  let crc = index
  for (let bit = 0; bit < 8; bit += 1) {
    crc = (crc & 1) === 1 ? 0xedb88320 ^ (crc >>> 1) : crc >>> 1
  }
  return crc >>> 0
})

const crc32 = (bytes: Uint8Array): number => {
  let crc = 0xffffffff
  for (const byte of bytes) {
    crc = crcTable[(crc ^ byte) & 0xff]! ^ (crc >>> 8)
  }
  return (crc ^ 0xffffffff) >>> 0
}

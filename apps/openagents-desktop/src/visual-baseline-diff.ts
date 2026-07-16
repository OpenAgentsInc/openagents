/**
 * QA-3 (#8908): pure PNG decode/encode and pixel-diff logic for the Desktop
 * visual-baseline gate. Zero dependencies beyond node:zlib — the repository
 * ships no PNG library, and the gate needs exactly this bounded slice of the
 * format: 8-bit RGB/RGBA, non-interlaced (what Electron's
 * `webContents.capturePage().toPNG()` emits).
 *
 * Consumed by `scripts/visual-baseline-smoke.ts` (the two-process gate) and
 * unit-tested in `visual-baseline-diff.test.ts` (identical / threshold-pass /
 * drift-fail / synthetic-alteration cases).
 */
import { deflateSync, inflateSync } from "node:zlib"
import { Exit, Schema } from "@effect-native/core/effect"

export type DecodedImage = Readonly<{
  width: number
  height: number
  /** Always RGBA, 4 bytes per pixel, row-major. */
  pixels: Uint8Array
}>

const PNG_SIGNATURE = Uint8Array.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a])

const CRC_TABLE = ((): Uint32Array => {
  const table = new Uint32Array(256)
  for (let n = 0; n < 256; n++) {
    let c = n
    for (let k = 0; k < 8; k++) c = (c & 1) === 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1
    table[n] = c >>> 0
  }
  return table
})()

const crc32 = (bytes: Uint8Array): number => {
  let crc = 0xffffffff
  for (const byte of bytes) crc = CRC_TABLE[(crc ^ byte) & 0xff]! ^ (crc >>> 8)
  return (crc ^ 0xffffffff) >>> 0
}

const readUint32 = (bytes: Uint8Array, offset: number): number =>
  ((bytes[offset]! << 24) | (bytes[offset + 1]! << 16) | (bytes[offset + 2]! << 8) | bytes[offset + 3]!) >>> 0

/**
 * Decode an 8-bit-depth, non-interlaced, truecolor (2) or truecolor-alpha (6)
 * PNG into RGBA pixels. Throws on any other PNG flavor: the harness controls
 * both producers (Electron capture and this module's encoder), so anything
 * else is corruption, not a case to silently coerce.
 */
export const decodePng = (bytes: Uint8Array): DecodedImage => {
  if (bytes.length < PNG_SIGNATURE.length || !PNG_SIGNATURE.every((value, index) => bytes[index] === value)) {
    throw new Error("visual-baseline PNG decode: missing PNG signature")
  }
  let offset = PNG_SIGNATURE.length
  let width = 0
  let height = 0
  let colorType = -1
  const idat: Array<Uint8Array> = []
  while (offset + 8 <= bytes.length) {
    const length = readUint32(bytes, offset)
    const type = String.fromCharCode(bytes[offset + 4]!, bytes[offset + 5]!, bytes[offset + 6]!, bytes[offset + 7]!)
    const data = bytes.subarray(offset + 8, offset + 8 + length)
    if (type === "IHDR") {
      width = readUint32(data, 0)
      height = readUint32(data, 4)
      const bitDepth = data[8]!
      colorType = data[9]!
      const interlace = data[12]!
      if (bitDepth !== 8) throw new Error(`visual-baseline PNG decode: unsupported bit depth ${bitDepth}`)
      if (colorType !== 2 && colorType !== 6) throw new Error(`visual-baseline PNG decode: unsupported color type ${colorType}`)
      if (interlace !== 0) throw new Error("visual-baseline PNG decode: interlaced PNGs are unsupported")
    } else if (type === "IDAT") {
      idat.push(data)
    } else if (type === "IEND") {
      break
    }
    offset += 8 + length + 4
  }
  if (width === 0 || height === 0 || idat.length === 0) {
    throw new Error("visual-baseline PNG decode: missing IHDR or IDAT")
  }
  const compressed = new Uint8Array(idat.reduce((total, chunk) => total + chunk.length, 0))
  let cursor = 0
  for (const chunk of idat) {
    compressed.set(chunk, cursor)
    cursor += chunk.length
  }
  const raw = new Uint8Array(inflateSync(compressed))
  const channels = colorType === 6 ? 4 : 3
  const stride = width * channels
  if (raw.length < height * (stride + 1)) {
    throw new Error("visual-baseline PNG decode: truncated pixel data")
  }
  // Unfilter (PNG filter spec: 0 none, 1 sub, 2 up, 3 average, 4 paeth).
  const scanlines = new Uint8Array(height * stride)
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)]!
    const rowIn = raw.subarray(y * (stride + 1) + 1, y * (stride + 1) + 1 + stride)
    const rowOut = scanlines.subarray(y * stride, (y + 1) * stride)
    const prior = y === 0 ? null : scanlines.subarray((y - 1) * stride, y * stride)
    for (let x = 0; x < stride; x++) {
      const left = x >= channels ? rowOut[x - channels]! : 0
      const up = prior === null ? 0 : prior[x]!
      const upLeft = prior !== null && x >= channels ? prior[x - channels]! : 0
      const value = rowIn[x]!
      if (filter === 0) rowOut[x] = value
      else if (filter === 1) rowOut[x] = (value + left) & 0xff
      else if (filter === 2) rowOut[x] = (value + up) & 0xff
      else if (filter === 3) rowOut[x] = (value + ((left + up) >> 1)) & 0xff
      else if (filter === 4) {
        const p = left + up - upLeft
        const pa = Math.abs(p - left)
        const pb = Math.abs(p - up)
        const pc = Math.abs(p - upLeft)
        const paeth = pa <= pb && pa <= pc ? left : pb <= pc ? up : upLeft
        rowOut[x] = (value + paeth) & 0xff
      } else throw new Error(`visual-baseline PNG decode: unknown filter ${filter}`)
    }
  }
  if (channels === 4) return { width, height, pixels: scanlines }
  const rgba = new Uint8Array(width * height * 4)
  for (let pixel = 0; pixel < width * height; pixel++) {
    rgba[pixel * 4] = scanlines[pixel * 3]!
    rgba[pixel * 4 + 1] = scanlines[pixel * 3 + 1]!
    rgba[pixel * 4 + 2] = scanlines[pixel * 3 + 2]!
    rgba[pixel * 4 + 3] = 255
  }
  return { width, height, pixels: rgba }
}

const chunk = (type: string, data: Uint8Array): Uint8Array => {
  const bytes = new Uint8Array(12 + data.length)
  const view = new DataView(bytes.buffer)
  view.setUint32(0, data.length)
  for (let index = 0; index < 4; index++) bytes[4 + index] = type.charCodeAt(index)
  bytes.set(data, 8)
  view.setUint32(8 + data.length, crc32(bytes.subarray(4, 8 + data.length)))
  return bytes
}

/** Encode RGBA pixels as a non-interlaced 8-bit RGBA PNG (filter 0 rows). */
export const encodePng = (image: DecodedImage): Uint8Array => {
  const { width, height, pixels } = image
  if (pixels.length !== width * height * 4) {
    throw new Error("visual-baseline PNG encode: pixel buffer does not match dimensions")
  }
  const ihdr = new Uint8Array(13)
  const ihdrView = new DataView(ihdr.buffer)
  ihdrView.setUint32(0, width)
  ihdrView.setUint32(4, height)
  ihdr[8] = 8 // bit depth
  ihdr[9] = 6 // color type RGBA
  const stride = width * 4
  const raw = new Uint8Array(height * (stride + 1))
  for (let y = 0; y < height; y++) {
    raw[y * (stride + 1)] = 0
    raw.set(pixels.subarray(y * stride, (y + 1) * stride), y * (stride + 1) + 1)
  }
  const idat = new Uint8Array(deflateSync(raw))
  const parts = [PNG_SIGNATURE, chunk("IHDR", ihdr), chunk("IDAT", idat), chunk("IEND", new Uint8Array(0))]
  const output = new Uint8Array(parts.reduce((total, part) => total + part.length, 0))
  let cursor = 0
  for (const part of parts) {
    output.set(part, cursor)
    cursor += part.length
  }
  return output
}

export type DiffThresholds = Readonly<{
  /** Per-channel absolute delta at or below which a pixel counts as same. */
  perChannelTolerance: number
  /** Maximum fraction of pixels allowed to differ before the gate fails. */
  maxDifferentPixelRatio: number
}>

/** The gate's bounded default: tiny AA jitter tolerated, real drift is not. */
export const DEFAULT_DIFF_THRESHOLDS: DiffThresholds = {
  perChannelTolerance: 3,
  maxDifferentPixelRatio: 0.001,
}

export type ImageDiffResult = Readonly<{
  ok: boolean
  reason: "match" | "dimensions_mismatch" | "pixel_drift"
  totalPixels: number
  differentPixels: number
  differentRatio: number
  /** Baseline-sized mask: drifted pixels magenta, same pixels transparent-black. */
  diffMask: DecodedImage | null
}>

/** Pixel-wise comparison with a bounded threshold (pure; no I/O). */
export const diffImages = (
  baseline: DecodedImage,
  current: DecodedImage,
  thresholds: DiffThresholds = DEFAULT_DIFF_THRESHOLDS,
): ImageDiffResult => {
  if (baseline.width !== current.width || baseline.height !== current.height) {
    return {
      ok: false,
      reason: "dimensions_mismatch",
      totalPixels: baseline.width * baseline.height,
      differentPixels: baseline.width * baseline.height,
      differentRatio: 1,
      diffMask: null,
    }
  }
  const totalPixels = baseline.width * baseline.height
  const mask = new Uint8Array(totalPixels * 4)
  let differentPixels = 0
  for (let pixel = 0; pixel < totalPixels; pixel++) {
    let drifted = false
    for (let channel = 0; channel < 4; channel++) {
      const delta = Math.abs(baseline.pixels[pixel * 4 + channel]! - current.pixels[pixel * 4 + channel]!)
      if (delta > thresholds.perChannelTolerance) {
        drifted = true
        break
      }
    }
    if (drifted) {
      differentPixels += 1
      mask[pixel * 4] = 255
      mask[pixel * 4 + 2] = 255
      mask[pixel * 4 + 3] = 255
    }
  }
  const differentRatio = totalPixels === 0 ? 0 : differentPixels / totalPixels
  return {
    ok: differentRatio <= thresholds.maxDifferentPixelRatio,
    reason: differentRatio <= thresholds.maxDifferentPixelRatio ? "match" : "pixel_drift",
    totalPixels,
    differentPixels,
    differentRatio,
    diffMask: { width: baseline.width, height: baseline.height, pixels: mask },
  }
}

/** Side-by-side review artifact: baseline | current | drift mask. */
export const sideBySideImage = (
  baseline: DecodedImage,
  current: DecodedImage,
  diffMask: DecodedImage | null,
): DecodedImage => {
  const panels = [baseline, current, ...(diffMask === null ? [] : [diffMask])]
  const width = panels.reduce((total, panel) => total + panel.width, 0)
  const height = Math.max(...panels.map(panel => panel.height))
  const pixels = new Uint8Array(width * height * 4)
  let panelX = 0
  for (const panel of panels) {
    for (let y = 0; y < panel.height; y++) {
      const source = panel.pixels.subarray(y * panel.width * 4, (y + 1) * panel.width * 4)
      pixels.set(source, (y * width + panelX) * 4)
    }
    panelX += panel.width
  }
  return { width, height, pixels }
}

// ---------------------------------------------------------------------------
// Baseline manifest — the typed contract between the capture probe, the
// committed `visual-baselines/` directory, and the diff gate.
// ---------------------------------------------------------------------------

export const VisualBaselineManifestSchema = Schema.Struct({
  schema: Schema.Literal("openagents-desktop.visual-baselines.v1"),
  platform: Schema.String,
  timezone: Schema.String,
  window: Schema.Struct({
    width: Schema.Number,
    height: Schema.Number,
    deviceScaleFactor: Schema.Number,
  }),
  thresholds: Schema.Struct({
    perChannelTolerance: Schema.Number,
    maxDifferentPixelRatio: Schema.Number,
  }),
  states: Schema.Array(Schema.Struct({
    name: Schema.String,
    file: Schema.String,
    sha256: Schema.String,
    width: Schema.Number,
    height: Schema.Number,
  })),
})
export type VisualBaselineManifest = typeof VisualBaselineManifestSchema.Type

/** Decode a manifest document; null on any schema violation. */
export const decodeVisualBaselineManifest = (value: unknown): VisualBaselineManifest | null => {
  const result = Schema.decodeUnknownExit(VisualBaselineManifestSchema)(value)
  return Exit.isSuccess(result) ? result.value : null
}

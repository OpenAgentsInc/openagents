// The style check's measures (docs/verse/art-styles.md, 15. Judging "looks painterly"), over a shot's
// pixels: pure functions, so each is tested on synthetic images (style-measures.test.ts) and
// style-check.ts runs them on a realm's views. PNG in and out without a dependency.
import { inflateSync, deflateSync } from "node:zlib"

export type Image = { w: number; h: number; px: Uint8Array }   // RGBA, 8 bits a channel
// A mask of the pixels measured (1) and not (0: the HUD's plates, the sky where a measure skips it).
export type Mask = Uint8Array

export function decodePng(bytes: Uint8Array): Image {
  const b = Buffer.from(bytes)
  let at = 8, w = 0, h = 0, type = 0
  const data: Buffer[] = []
  while (at < b.length) {
    const n = b.readUInt32BE(at), kind = b.toString("latin1", at + 4, at + 8)
    if (kind === "IHDR") { w = b.readUInt32BE(at + 8); h = b.readUInt32BE(at + 12); type = b[at + 17]; if (b[at + 16] !== 8 || b[at + 20] !== 0) throw new Error("png: not 8-bit") }
    if (kind === "IDAT") data.push(b.subarray(at + 8, at + 8 + n))
    at += 12 + n
  }
  const bpp = type === 6 ? 4 : type === 2 ? 3 : 0
  if (!bpp) throw new Error(`png: colour type ${type}`)
  const raw = inflateSync(Buffer.concat(data)), stride = w * bpp
  const rows = new Uint8Array(h * stride), px = new Uint8Array(w * h * 4)
  for (let y = 0; y < h; y++) {
    const f = raw[y * (stride + 1)], src = y * (stride + 1) + 1, o = y * stride
    for (let x = 0; x < stride; x++) {
      const a = x >= bpp ? rows[o + x - bpp] : 0, up = y ? rows[o - stride + x] : 0, c = x >= bpp && y ? rows[o - stride + x - bpp] : 0
      const p = a + up - c, pa = Math.abs(p - a), pb = Math.abs(p - up), pc = Math.abs(p - c)
      const pred = f === 1 ? a : f === 2 ? up : f === 3 ? (a + up) >> 1 : f === 4 ? (pa <= pb && pa <= pc ? a : pb <= pc ? up : c) : 0
      rows[o + x] = (raw[src + x] + pred) & 255
    }
    for (let x = 0; x < w; x++) for (let k = 0; k < 4; k++) px[(y * w + x) * 4 + k] = k < bpp ? rows[o + x * bpp + k] : 255
  }
  return { w, h, px }
}

const CRC = (() => { const t = new Uint32Array(256); for (let n = 0; n < 256; n++) { let c = n; for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1; t[n] = c >>> 0 } return t })()
function crc(b: Buffer) { let c = 0xffffffff; for (const x of b) c = CRC[(c ^ x) & 255] ^ (c >>> 8); return (c ^ 0xffffffff) >>> 0 }
function chunk(kind: string, data: Buffer) {
  const head = Buffer.alloc(8); head.writeUInt32BE(data.length, 0); head.write(kind, 4, "latin1")
  const tail = Buffer.alloc(4); tail.writeUInt32BE(crc(Buffer.concat([head.subarray(4), data])), 0)
  return Buffer.concat([head, data, tail])
}
export function encodePng(m: Image): Buffer {
  const ihdr = Buffer.alloc(13); ihdr.writeUInt32BE(m.w, 0); ihdr.writeUInt32BE(m.h, 4); ihdr[8] = 8; ihdr[9] = 6
  const raw = Buffer.alloc(m.h * (m.w * 4 + 1))
  for (let y = 0; y < m.h; y++) { raw[y * (m.w * 4 + 1)] = 0; Buffer.from(m.px.buffer, m.px.byteOffset + y * m.w * 4, m.w * 4).copy(raw, y * (m.w * 4 + 1) + 1) }
  return Buffer.concat([Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]), chunk("IHDR", ihdr), chunk("IDAT", deflateSync(raw)), chunk("IEND", Buffer.alloc(0))])
}

// sRGB luminance 0..1 of each pixel (Rec. 709 weights on the encoded values, as the spec's numbers are)
export function luma(m: Image): Float32Array {
  const out = new Float32Array(m.w * m.h)
  for (let i = 0; i < out.length; i++) out[i] = (0.2126 * m.px[i * 4] + 0.7152 * m.px[i * 4 + 1] + 0.0722 * m.px[i * 4 + 2]) / 255
  return out
}
// A box mean of radius r (2: 5x5, 1: 3x3), clamped at the edges, by summed-area table.
export function boxMean(v: Float32Array, w: number, h: number, r: number): Float32Array {
  const s = new Float64Array((w + 1) * (h + 1))
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) s[(y + 1) * (w + 1) + x + 1] = v[y * w + x] + s[y * (w + 1) + x + 1] + s[(y + 1) * (w + 1) + x] - s[y * (w + 1) + x]
  const out = new Float32Array(w * h)
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const x0 = Math.max(0, x - r), x1 = Math.min(w, x + r + 1), y0 = Math.max(0, y - r), y1 = Math.min(h, y + r + 1)
    out[y * w + x] = (s[y1 * (w + 1) + x1] - s[y0 * (w + 1) + x1] - s[y1 * (w + 1) + x0] + s[y0 * (w + 1) + x0]) / ((x1 - x0) * (y1 - y0))
  }
  return out
}
export const allOf = (m: Image): Mask => new Uint8Array(m.w * m.h).fill(1)
// The HUD's plates left out: a band at the top and one at the bottom (the title and map plates, the
// hint and the spell bar), as shares of the height.
// On a phone (under 600 px wide) the right-hand column too, where touch buttons stand, and the
// bottom 32%: Everglade's spell bar there is two rows of slots from about 70% of the height down.
export function hudMask(m: Image, top = 0.1, bottom = 0.14): Mask {
  const k = allOf(m), t = Math.round(m.h * top), b = Math.round(m.h * (1 - (m.w < 600 ? Math.max(bottom, 0.32) : bottom)))
  for (let y = 0; y < m.h; y++) if (y < t || y >= b) k.fill(0, y * m.w, (y + 1) * m.w)
  if (m.w < 600) for (let y = t; y < b; y++) k.fill(0, y * m.w + Math.round(m.w * 0.76), (y + 1) * m.w)
  return k
}

// Fine detail: the RMS of luminance less its 5x5 box mean, over the measured pixels.
export function fineDetail(m: Image, k: Mask = allOf(m)): number {
  const l = luma(m), b = boxMean(l, m.w, m.h, 2)
  let s = 0, n = 0
  for (let i = 0; i < l.length; i++) if (k[i]) { const d = l[i] - b[i]; s += d * d; n++ }
  return n ? Math.sqrt(s / n) : 0
}
// Dark ink: the share of pixels darker than 0.4 of their 3x3 neighbourhood's mean (an outline drawn in black).
export function darkInk(m: Image, k: Mask = allOf(m)): number {
  const l = luma(m), b = boxMean(l, m.w, m.h, 1)
  let c = 0, n = 0
  for (let i = 0; i < l.length; i++) if (k[i]) { n++; if (l[i] < 0.4 * b[i]) c++ }
  return n ? c / n : 0
}
// Near black: the share with luminance under 0.03.
export function nearBlack(m: Image, k: Mask = allOf(m)): number {
  const l = luma(m)
  let c = 0, n = 0
  for (let i = 0; i < l.length; i++) if (k[i]) { n++; if (l[i] < 0.03) c++ }
  return n ? c / n : 0
}
const hsvSat = (r: number, g: number, b: number) => { const hi = Math.max(r, g, b); return hi > 0 ? (hi - Math.min(r, g, b)) / hi : 0 }
// Saturation: the mean HSV saturation of the measured pixels.
export function meanSaturation(m: Image, k: Mask = allOf(m)): number {
  let s = 0, n = 0
  for (let i = 0; i < m.w * m.h; i++) if (k[i]) { s += hsvSat(m.px[i * 4], m.px[i * 4 + 1], m.px[i * 4 + 2]); n++ }
  return n ? s / n : 0
}
// Shadow hue from two colour samples of one surface (sRGB 0..255), shadow band and lit: the shadow's
// blue share b/(r+g+b) at least 0.02 over the lit's, and its own saturation at least 0.05 (never grey).
export function shadowHue(shadow: [number, number, number], lit: [number, number, number]) {
  const share = (c: number[]) => c[2] / Math.max(c[0] + c[1] + c[2], 1e-6)
  const blue = share(shadow) - share(lit), sat = hsvSat(shadow[0], shadow[1], shadow[2])
  return { blue, sat, ok: blue >= 0.02 && sat >= 0.05 }
}

export type Clock = "day" | "night"
export type Limits = { fine: number; ink: number; black: number; satLow: number; satHigh: number }
export const PAINTERLY = (clock: Clock): Limits => ({ fine: 0.035, ink: 0.002, black: clock === "night" ? 0.02 : 0.005, satLow: 0.22, satHigh: 0.60 })
export type Measured = { fine: number; ink: number; black: number; sat: number; fails: string[] }
// Every measure of one view against the painterly limits; fails names each one out of range.
export function measure(m: Image, clock: Clock, k: Mask = hudMask(m)): Measured {
  const L = PAINTERLY(clock)
  const r = { fine: fineDetail(m, k), ink: darkInk(m, k), black: nearBlack(m, k), sat: meanSaturation(m, k) }
  const fails: string[] = []
  if (r.fine > L.fine) fails.push(`fine detail ${r.fine.toFixed(3)} > ${L.fine}`)
  if (r.ink > L.ink) fails.push(`dark ink ${(r.ink * 100).toFixed(2)}% > ${(L.ink * 100).toFixed(1)}%`)
  if (r.black > L.black) fails.push(`near black ${(r.black * 100).toFixed(2)}% > ${(L.black * 100).toFixed(1)}%`)
  if (r.sat < L.satLow || r.sat > L.satHigh) fails.push(`saturation ${r.sat.toFixed(2)} outside ${L.satLow} to ${L.satHigh}`)
  return { ...r, fails }
}

// The side-by-side sheet: images in rows of `across`, each scaled (nearest) to cell x cell height.
export function sheet(images: Image[], across: number, cellW: number, cellH: number): Image {
  const rows = Math.ceil(images.length / across), out: Image = { w: across * cellW, h: rows * cellH, px: new Uint8Array(across * cellW * rows * cellH * 4) }
  for (let i = 0; i < out.px.length; i += 4) { out.px[i] = 20; out.px[i + 1] = 20; out.px[i + 2] = 24; out.px[i + 3] = 255 }
  images.forEach((m, k) => {
    const ox = (k % across) * cellW, oy = Math.floor(k / across) * cellH
    const s = Math.min(cellW / m.w, cellH / m.h), dw = Math.floor(m.w * s), dh = Math.floor(m.h * s)
    for (let y = 0; y < dh; y++) for (let x = 0; x < dw; x++) {
      const sx = Math.min(m.w - 1, Math.floor(x / s)), sy = Math.min(m.h - 1, Math.floor(y / s))
      const d = ((oy + y) * out.w + ox + x + Math.floor((cellW - dw) / 2)) * 4, f = (sy * m.w + sx) * 4
      for (let c = 0; c < 4; c++) out.px[d + c] = m.px[f + c]
    }
  })
  return out
}

import { test, expect } from "bun:test"
import { type Image, decodePng, encodePng, fineDetail, darkInk, nearBlack, meanSaturation, shadowHue, measure, allOf, sheet } from "./style-measures"

// A painted image: broad soft fields of saturated colour (a green meadow under a teal sky).
function painted(w = 120, h = 80): Image {
  const px = new Uint8Array(w * h * 4)
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) {
    const i = (y * w + x) * 4, t = x / w
    const sky = y < h / 2
    px[i] = sky ? 90 + 40 * t : 70 + 30 * t; px[i + 1] = sky ? 150 : 140 + 20 * t; px[i + 2] = sky ? 190 : 50; px[i + 3] = 255
  }
  return { w, h, px }
}
const at = (m: Image, x: number, y: number, c: [number, number, number]) => { const i = (y * m.w + x) * 4; m.px[i] = c[0]; m.px[i + 1] = c[1]; m.px[i + 2] = c[2] }

test("a painted image passes every measure", () => {
  const r = measure(painted(), "day", allOf(painted()))
  expect(r.fails).toEqual([])
  expect(r.fine).toBeLessThan(0.01)
})
test("a grainy image fails fine detail", () => {
  const m = painted()
  let seed = 7
  const rnd = () => (seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff
  for (let i = 0; i < m.w * m.h; i++) { const d = (rnd() - 0.5) * 90; for (let c = 0; c < 3; c++) m.px[i * 4 + c] = Math.max(0, Math.min(255, m.px[i * 4 + c] + d)) }
  expect(fineDetail(m)).toBeGreaterThan(0.035)
  expect(measure(m, "day", allOf(m)).fails.some((f) => f.startsWith("fine detail"))).toBe(true)
})
test("an inked image (black outlines) fails dark ink and near black", () => {
  const m = painted()
  for (let x = 10; x < 110; x++) { at(m, x, 20, [0, 0, 0]); at(m, x, 60, [0, 0, 0]) }
  for (let y = 20; y < 60; y++) { at(m, 10, y, [0, 0, 0]); at(m, 109, y, [0, 0, 0]) }
  expect(darkInk(m)).toBeGreaterThan(0.002)
  expect(nearBlack(m)).toBeGreaterThan(0.005)
  const f = measure(m, "day", allOf(m)).fails
  expect(f.some((x) => x.startsWith("dark ink"))).toBe(true)
  expect(f.some((x) => x.startsWith("near black"))).toBe(true)
  // the night allows 2% near black, not ink
  expect(nearBlack(m)).toBeLessThan(0.05)
})
test("a grey-shadowed surface fails shadow hue; a cool one passes", () => {
  expect(shadowHue([60, 60, 60], [200, 160, 100]).ok).toBe(false)
  expect(shadowHue([50, 70, 110], [200, 160, 100]).ok).toBe(true)
})
test("a washed-out (milky) image fails saturation", () => {
  const m = painted()
  for (let i = 0; i < m.w * m.h; i++) for (let c = 0; c < 3; c++) m.px[i * 4 + c] = Math.round(m.px[i * 4 + c] * 0.3 + 200 * 0.7)
  expect(meanSaturation(m)).toBeLessThan(0.22)
  expect(measure(m, "day", allOf(m)).fails.some((x) => x.startsWith("saturation"))).toBe(true)
})
test("png round trip, and the sheet lays its images out in rows", () => {
  const m = painted(30, 20)
  const back = decodePng(encodePng(m))
  expect(back.w).toBe(30); expect(back.h).toBe(20); expect(Buffer.from(back.px).equals(Buffer.from(m.px))).toBe(true)
  const s = sheet([m, m, m], 2, 60, 40)
  expect(s.w).toBe(120); expect(s.h).toBe(80)
})

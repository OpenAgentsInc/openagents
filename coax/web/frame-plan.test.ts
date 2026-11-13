// Firm reason: 1 (it evaluates host.js's own table of the frame plan's numbers, the browser host's module, as the browser does)
// host.js's frame plan numbers are the ones metaverse-engine/render/frame-plan.coax writes for C hosts
// (metaverse-engine/host/frame-plan.h, #13303): a member added to passes.coax reaches both or this fails.
//   bun test coax/web/frame-plan.test.ts
import { expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"

const root = join(import.meta.dir, "../..")
const header = readFileSync(join(root, "metaverse-engine/host/frame-plan.h"), "utf8")
const host = readFileSync(join(root, "coax/web/host.js"), "utf8")
const defines = new Map([...header.matchAll(/^#define AETHYR_FP_(\w+) (\d+)u$/gm)].map((m) => [m[1], Number(m[2])]))
const start = host.indexOf("const FP = {"), end = host.indexOf("};", start)
const FP = new Function(`${host.slice(start, end + 2)} return FP;`)() as Record<string, any>

test("the walk's numbers are the header's, every member of every group", () => {
  expect(start).toBeGreaterThan(0)
  expect(FP.magic).toBe(defines.get("MAGIC")); expect(FP.version).toBe(defines.get("VERSION")); expect(defines.get("WORDS")).toBe(8)
  for (const [group, prefix] of [["op", "OP_"], ["pass", "PASS_"], ["target", "TARGET_"], ["source", "SOURCE_"]]) {
    const named = [...defines].filter(([k]) => k.startsWith(prefix))
    expect(named.length).toBeGreaterThan(2)
    expect(Object.keys(FP[group]).length).toBe(named.length)
    for (const [k, v] of named) expect([k, FP[group][k.slice(prefix.length).toLowerCase()]]).toEqual([k, v])
  }
})
test("the host's word names its bits as the header does", () => {
  // host.js builds the word with has(fact, bit): each bit it sets is one the header names
  const bits = [...host.slice(host.indexOf("const bits = has("), host.indexOf("const wrote = x.coaxial_frame_plan_v1")).matchAll(/, (\d+)\)/g)].map((m) => Number(m[1]))
  const known = new Set([...defines].filter(([k]) => k.startsWith("HOST_")).map(([, v]) => Math.log2(v)))
  expect(bits.length).toBeGreaterThan(12)
  for (const b of bits) expect(known.has(b)).toBe(true)
  expect(defines.get("HOST_TILES")).toBe(1 << 14); expect(defines.get("HOST_CULL")).toBe(1 << 15); expect(defines.get("HOST_SCENE_READY")).toBe(1 << 18)
})

// realms.ts's checks on the table: the tagged table passes, and each style, zone and
// clock refusal (docs/verse/art-styles.md, The style field) is a refusal.
//   bun test coax/web/realms.test.ts
import { expect, test } from "bun:test"
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { page, worlds, STYLE_BLOCK } from "./realms"

const SITE = join(import.meta.dir, "../../apps/openagents-web")
const table = () => JSON.parse(readFileSync(join(SITE, "realms.json"), "utf8"))
const dir = mkdtempSync(join(tmpdir(), "realms-test-"))
let n = 0
/** The real table with one realm changed, checked; opts as worlds() takes them. */
function check(change: (realm: any) => void, opts: { site?: string; styleBlock?: boolean; packs?: string } = { styleBlock: false }, id = "everglade") {
  const t = table()
  // the fixture's pack is the drop realm's and the built realm's too: a change to the fixture's is made to all three
  for (const w of t.worlds) if (w.id === id || (id === "fixture" && (w.id === "drop" || w.id === "built"))) change(w.realm)
  const file = join(dir, `t${n++}.json`)
  writeFileSync(file, JSON.stringify(t))
  return () => worlds(file, opts)
}

test("the tagged table passes, and every realm has a style and a clock", () => {
  for (const w of worlds(join(SITE, "realms.json"))) {
    if (!w.realm) continue
    expect(["lowpoly", "painterly", "photoreal"]).toContain(w.realm.style)
    expect(w.realm.clock).toMatch(/^(cycle|real|fixed:)/)
  }
})
test("Everglade and Hecate's Cemetery are painterly; the rest lowpoly", () => {
  const by = Object.fromEntries(worlds().filter((w) => w.realm).map((w) => [w.realm!.slug, w.realm!]))
  expect(by["everglade"].style).toBe("painterly"); expect(by["everglade"].clock).toBe("cycle")
  expect(by["hecates-cemetery"].style).toBe("painterly"); expect(by["hecates-cemetery"].clock).toBe("fixed:0.0")
  for (const s of ["haunted-house", "the-grid", "dead-gears", "fixture", "drop", "sandbox", "ice-cream-man"]) expect(by[s].style).toBe("lowpoly")
})
test("refuses a realm without a style", () => expect(check((r) => delete r.style)).toThrow(/style is one of/))
test("refuses an unknown style", () => expect(check((r) => (r.style = "cel"))).toThrow(/style is one of/))
test("refuses a zone with an unknown style", () => expect(check((r) => (r.zones = [{ id: "crypt", style: "ink" }]))).toThrow(/zone "crypt"/))
test("refuses a zone id twice", () =>
  expect(check((r) => (r.zones = [{ id: "crypt", style: "painterly" }, { id: "crypt" }]))).toThrow(/zone crypt twice/))
test("takes zones with and without their own style", () =>
  expect(check((r) => (r.zones = [{ id: "crypt", style: "photoreal" }, { id: "hall" }]))).not.toThrow())
test("refuses a missing or bad clock", () => {
  expect(check((r) => delete r.clock)).toThrow(/clock is/)
  expect(check((r) => (r.clock = "fixed:1.5"))).toThrow(/clock is/)
  expect(check((r) => (r.clock = "noon"))).toThrow(/clock is/)
  expect(check((r) => (r.clock = "fixed:0.25"))).not.toThrow()
  expect(check((r) => (r.clock = "real"))).not.toThrow()
})
test("refuses a painterly realm whose WGSL has no style block", () => {
  const site = mkdtempSync(join(tmpdir(), "realms-site-"))
  writeFileSync(join(site, "everglade.wgsl"), "// no block\n")
  writeFileSync(join(site, "villa.wgsl"), `${STYLE_BLOCK} (x) by coax/web/wgsl.py; do not edit\n// ---- end generated\n`)
  writeFileSync(join(site, "fairy.wgsl"), `${STYLE_BLOCK} (x) by coax/web/wgsl.py; do not edit\n// ---- end generated\n`)
  expect(check(() => {}, { site, styleBlock: true })).toThrow(/everglade.wgsl has no style block/)
  writeFileSync(join(site, "everglade.wgsl"), `${STYLE_BLOCK} (x) by coax/web/wgsl.py; do not edit\n// ---- end generated\n`)
  expect(check(() => {}, { site, styleBlock: true })).not.toThrow()
  // a lowpoly realm with a painterly zone needs it too
  writeFileSync(join(site, "everglade.wgsl"), "// no block\n")
  expect(check((r) => { r.style = "lowpoly"; r.zones = [{ id: "glade", style: "painterly" }] }, { site, styleBlock: true })).toThrow(/partly painterly/)
})
test("the page carries data-style, data-clock and data-zones", () => {
  const r = worlds().find((w) => w.id === "everglade")!.realm!
  const html = page({ ...r, zones: [{ id: "crypt" }] })
  expect(html).toContain(`data-style="painterly" data-clock="cycle" data-zones="crypt:painterly"`)
})
test("refuses a painterly realm with a lowpoly pack, and a pack of another style", () => {
  const packs = mkdtempSync(join(tmpdir(), "realms-packs-"))
  writeFileSync(join(packs, "fixture.json"), JSON.stringify({ pack: "fixture" }))
  expect(check(() => {}, { styleBlock: false, packs }, "fixture")).not.toThrow()
  expect(check((r) => (r.style = "painterly"), { styleBlock: false, packs }, "fixture")).toThrow(/its pack fixture is lowpoly/)
  writeFileSync(join(packs, "fixture.json"), JSON.stringify({ pack: "fixture", style: "lowpoly" }))
  expect(check((r) => (r.style = "painterly"), { styleBlock: false, packs }, "fixture")).toThrow(/its pack fixture is lowpoly/)
  writeFileSync(join(packs, "fixture.json"), JSON.stringify({ pack: "fixture", style: "painterly" }))
  expect(check((r) => (r.style = "painterly"), { styleBlock: false, packs }, "fixture")).not.toThrow()
  expect(check(() => {}, { styleBlock: false, packs }, "fixture")).toThrow(/drawn lowpoly but its pack fixture is painterly/)
  // a lowpoly realm with a painterly zone may carry a painterly pack
  expect(check((r) => (r.zones = [{ id: "glade", style: "painterly" }]), { styleBlock: false, packs }, "fixture")).not.toThrow()
  // Everglade waits on its own pack task, but only while its pack names no style
  writeFileSync(join(packs, "fixture.json"), JSON.stringify({ pack: "fixture" }))
  writeFileSync(join(packs, "everglade.json"), JSON.stringify({ pack: "everglade" }))
  expect(check(() => {}, { styleBlock: false, packs })).not.toThrow()
  writeFileSync(join(packs, "everglade.json"), JSON.stringify({ pack: "everglade", style: "lowpoly" }))
  expect(check(() => {}, { styleBlock: false, packs })).toThrow(/its pack everglade is lowpoly/)
})

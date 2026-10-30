import { expect, test } from "bun:test"
import { mkdirSync, mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { affected, goldenPages, pageObjects, suspects, touches } from "./golden-pages"
import { resolve } from "./golden-serve"

const p = goldenPages(), all = Object.keys(p.pages)

test("a shared file (host.js, the compiler) may change every page", () => {
  expect(affected(["coax/web/host.js"], p)).toEqual(all)
  expect(affected(["coax/src/c.coax"], p)).toEqual(all)
  expect(affected(["apps/openagents-web/realms.json"], p)).toEqual(all)
})

test("data, docs and the release tools change no page", () => {
  expect(affected(["apps/openagents-web/projects/openagents-desktop/work/v1.json", "apps/openagents-web/projects/coax/index.md",
    "coax/web/project/QUEUE.md", "coax/web/release.sh", "coax/web/project/snapshot.ts", "coax/README.md", "coax/tests/x.coax",
    "docs/verse/art-styles.md", "apps/openagents-web/realms/everglade/index.md"], p)).toEqual([])
})

test("a page's own inputs change it and not the others", () => {
  expect(affected(["apps/openagents-web/verse.coax"], p)).toEqual(["landing", "galaxy"])
  expect(affected(["apps/openagents-web/realms/everglade/assets/everglade.pack"], p)).toEqual(["everglade"])
  expect(affected(["apps/openagents-web/fixture.coax"], p)).toEqual(["fixture", "fixture-painterly-0", "fixture-painterly-0.25", "fixture-painterly-0.5", "fixture-painterly-0.75",
    "fixture-painterly-phone-0", "fixture-painterly-phone-0.25", "fixture-painterly-phone-0.5", "fixture-painterly-phone-0.75", "fixture-painterly-shafts"])
  // the engine is every realm's (its functions go into their shaders), not the landing's
  const engine = affected(["verse/haunt.coax"], p)
  expect(engine).toContain("haunted-house")
  expect(engine).not.toContain("landing")
  expect(affected(["coax/web/goldens/galaxy.png"], p)).toEqual(["galaxy"])
  // a file a page #loads is its input
  expect(affected(["apps/openagents-web/textkeys.coax"], p)).toContain("landing")
})

test("the likeliest break touches the page's own inputs, newest first; shared ones after", () => {
  const commits = [
    { hash: "c3", author: "a", subject: "task list", files: ["apps/openagents-web/projects/coax/work/nostr.json"] },
    { hash: "c2", author: "b", subject: "host", files: ["coax/web/host.js"] },
    { hash: "c1", author: "c", subject: "landing words", files: ["apps/openagents-web/verse.coax"] },
    { hash: "c0", author: "d", subject: "older landing", files: ["apps/openagents-web/verse-face.c"] },
  ]
  expect(suspects("landing", commits, p).map((s) => [s.hash, s.why])).toEqual([["c1", "own"], ["c0", "own"], ["c2", "shared"]])
  expect(suspects("everglade", commits, p).map((s) => s.hash)).toEqual(["c2"])
  expect(touches("landing", ["apps/openagents-web/projects/coax/work/nostr.json"], p)).toBe("")
  // the house's own libraries before the rest of the engine, which every realm takes
  const house = [
    { hash: "e1", author: "a", subject: "style", files: ["verse/style.coax"] },
    { hash: "e0", author: "b", subject: "house", files: ["verse/haunt.coax"] },
  ]
  expect(suspects("haunted-house", house, p).map((s) => [s.hash, s.why])).toEqual([["e0", "own"], ["e1", "engine"]])
})

test("a page's bytes in the bucket: its bare path and its index.html, not its twin or its children", () => {
  const r = new RegExp(pageObjects("/verse"))
  expect(["verse", "verse/index.html"].every((o) => r.test(o))).toBe(true)
  expect(["verse/index.md", "verse/galaxy", "verse/galaxy/index.html", "verses"].some((o) => r.test(o))).toBe(false)
  expect(new RegExp(pageObjects("/realms/fixture?style=painterly&clock=0.5")).test("realms/fixture")).toBe(true)
})

test("the check's server answers like the bucket", () => {
  const site = mkdtempSync(join(tmpdir(), "golden-serve-"))
  mkdirSync(join(site, "verse/galaxy"), { recursive: true })
  writeFileSync(join(site, "verse/index.html"), "v"); writeFileSync(join(site, "verse.page"), "v"); writeFileSync(join(site, "verse/galaxy/index.html"), "g")
  writeFileSync(join(site, "host.abc.js"), "js")
  expect(resolve(site, "/verse")?.file).toBe(join(site, "verse.page"))
  expect(resolve(site, "/verse/galaxy")?.file).toBe(join(site, "verse/galaxy/index.html"))
  expect(resolve(site, "/host.abc.js")?.type).toStartWith("text/javascript")
  expect(resolve(site, "/../etc/passwd")).toBeNull()
  expect(resolve(site, "/nope")).toBeNull()
})


test("the public homepage and its phone view track the root HTML", () => {
  expect(p.pages.home.path).toBe("/")
  expect(p.pages["home-phone"].size).toEqual([390, 844])
  expect(affected(["apps/openagents-web/index.html"], p)).toEqual(["home", "home-phone"])
  expect(new RegExp(pageObjects("/")).test("index.html")).toBe(true)
})

test("canonical Pylon quickstart has desktop and phone goldens tracking its one source",()=>{
  expect(p.pages['connect-pylon'].path).toBe('/docs/connect-a-pylon')
  expect(p.pages['connect-pylon-phone'].size).toEqual([390,844])
  for(const path of ['docs/connect-a-pylon.md','coax/web/connect-pylon-page.ts','apps/openagents-web/connect-pylon-content.coax'])
    expect(affected([path],p)).toEqual(['connect-pylon','connect-pylon-phone'])
})

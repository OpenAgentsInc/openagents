import { expect, test } from "bun:test"
import { classify, missingGolden } from "./golden-policy"

test("an unlisted realm with no golden waits in pending and doesn't fail", () => {
  const m = missingGolden("everglade", true, "/r/coax/web/goldens/pending/", "https://staging.openagents.com")
  expect(m.fail).toBe(false)
  expect(m.pending).toBe("/r/coax/web/goldens/pending/everglade.png")
  expect(m.line).toStartWith("NEW REALM everglade: no golden yet, approve with ")
  expect(m.line).toContain("bun coax/web/goldens.ts https://staging.openagents.com --update")
})

test("a listed realm with no golden fails", () => {
  const m = missingGolden("haunted-house", false, "/p", "https://staging.openagents.com")
  expect(m.fail).toBe(true)
  expect(m.pending).toBe("")
  expect(m.line).toContain("no golden")
})

test("only golden comparisons go to golden; every other failure stops the release", () => {
  const pages = { landing: { path: "/verse" }, "fixture-painterly-0.25": { path: "/realms/fixture?style=painterly&clock=0.25" }, everglade: { path: "/realms/everglade" } }
  const r = classify([
    "landing: matches its golden (1217 blocks changed, 4 allowed, 0 moving, x 0-1280, y 0-800; this shot: /t/landing.png)",
    "fixture-painterly-0.25: matches its golden (40 blocks changed, 6 allowed, 2 moving)",
    "everglade: no golden (coax/web/goldens/everglade.png); run --update and look at it",
    "galaxy: the engine started",
    "analytics contract: every contract event sent (missing haunt/quest, 1.0 s)",
  ], pages)
  expect(r.golden).toEqual([{ name: "landing", path: "/verse" }, { name: "fixture-painterly-0.25", path: "/realms/fixture?style=painterly&clock=0.25" },
    { name: "everglade", path: "/realms/everglade" }])
  expect(r.other).toEqual(["galaxy: the engine started", "analytics contract: every contract event sent (missing haunt/quest, 1.0 s)"])
})

test("a page that draws nothing is not a golden failure", () => {
  const r = classify(["landing: draws (0.0% lit, at least 0.4%)", "landing: matches its golden (9 blocks changed, 4 allowed, 0 moving)"], { landing: { path: "/verse" } })
  expect(r.golden.map((g) => g.name)).toEqual(["landing"])
  expect(r.other).toEqual(["landing: draws (0.0% lit, at least 0.4%)"])
})

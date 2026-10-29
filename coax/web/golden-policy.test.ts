import { expect, test } from "bun:test"
import { missingGolden } from "./golden-policy"

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

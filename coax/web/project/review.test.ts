// The review report: what counts as weakening a check with its code, and
// what counts as an independent review.
import { expect, test } from "bun:test"
import { generateSecretKey, getPublicKey } from "nostr-effect/pure"
import { mkdtempSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { changesCode, parseDiff, reviewOf, signReview, weakenings } from "./review"

const diff = (path: string, removed: string[], added: string[] = []) => ({ path, removed, added })
const noAuthority = () => false

test("a removed check with a code change is flagged; a pure move is not", () => {
  const files = [diff("lib/a.coax", ['#assert f(1)==2;', "f :: () -> int { return 1; }"], ["f :: () -> int { return 2; }"])]
  expect(weakenings(files, noAuthority).map((f) => f.why)).toEqual(["check removed"])
  expect(changesCode(files)).toBe(true)
  const reordered = [diff("lib/a.coax", ['#assert f(1)==2;'], ['#assert f(1)==2;'])]
  expect(weakenings(reordered, noAuthority)).toEqual([])
})

test("checks moved into an unmarked helper are flagged, into an #authority file not", () => {
  const files = [diff("lib/a.coax", ['#assert g(0)==0;']), diff("lib/helper.coax", [], ['#assert g(0)==0;'])]
  expect(weakenings(files, (p) => p === "lib/a.coax")[0].why).toContain("no #authority")
  expect(weakenings(files, () => true)).toEqual([])
  expect(weakenings(files, noAuthority)).toEqual([])
})

test("equivalence marks, invariants and gate lines are weakenings; docs alone are not code", () => {
  expect(weakenings([diff("lib/a.coax", [], ["x<=lo // mutant: equivalent"])], noAuthority)[0].why).toContain("equivalent")
  expect(weakenings([diff("coax/web/project/QUEUE.md", [], ["adds a `mutant: equivalent` mark"]), diff("x/review.ts", [], ["// \"mutant: equivalent\" mark"])], noAuthority)).toEqual([])
  expect(weakenings([diff("coax/INVARIANTS.md", ["- every mutant is killed"])], noAuthority).length).toBe(1)
  expect(weakenings([diff("coax/tests/run.sh", ["python3 tests/evidence.py"])], noAuthority).length).toBe(1)
  expect(weakenings([diff("coax/tests/run.sh", ["[[ $a_rc -eq 0 ]]"], ["[[ $a_rc -eq 0 && $b_rc -eq 0 ]]"])], noAuthority)).toEqual([])
  expect(weakenings([diff("lib/a.coax", ["#assert f(1)==2;"], ["#assert f(1)==3;"])], noAuthority)).toEqual([])
  expect(weakenings([diff("apps/openagents-web/projects/coax/work.coax", ['X :: "#test a";'])], noAuthority)).toEqual([])
  expect(weakenings([diff("lib/a.coax", ['S :: "a #test inside a string";'])], noAuthority)).toEqual([])
  expect(weakenings([diff("coax/tests/evidence.py", ["        self.assertEqual(a, b)"])], noAuthority).length).toBe(1)
  expect(changesCode([diff("coax/spec.md", ["x"]), diff("coax/tests/evidence.py", ["assert"])])).toBe(false)
  expect(changesCode([diff("lib/a.coax", ['#assert f(1)==2;', "// note"])])).toBe(false)
  expect(changesCode([diff("coax/src/main.cpp", ["x"])])).toBe(true)
})

test("unified diffs parse per file", () => {
  const text = "diff --git a/lib/a.coax b/lib/a.coax\n--- a/lib/a.coax\n+++ b/lib/a.coax\n@@ -1 +1 @@\n-#assert x;\n+y := 1;\n"
  expect(parseDiff(text)).toEqual([{ path: "lib/a.coax", removed: ["#assert x;"], added: ["y := 1;"] }])
})

test("a review is independent only with a valid signature from a non-author key", () => {
  const dir = mkdtempSync(join(tmpdir(), "reviews-"))
  const author = generateSecretKey(), reviewer = generateSecretKey()
  const authorKeys = new Set([getPublicKey(author)])
  const commit = "a".repeat(40)
  expect(reviewOf(commit, dir, authorKeys)).toBe("none")
  expect(() => signReview(commit, author, "ok", authorKeys)).toThrow()
  const ev = signReview(commit, reviewer, "looked at it", authorKeys)
  writeFileSync(join(dir, `${commit}.json`), JSON.stringify(ev))
  expect(reviewOf(commit, dir, authorKeys)).toBe("independent")
  writeFileSync(join(dir, `${commit}.json`), JSON.stringify({ ...ev, content: "edited" }))
  expect(reviewOf(commit, dir, authorKeys)).toBe("invalid: bad signature")
  const other = "b".repeat(40)
  writeFileSync(join(dir, `${other}.json`), JSON.stringify(ev))
  expect(reviewOf(other, dir, authorKeys)).toBe("invalid: names another commit")
  writeFileSync(join(dir, `${commit}.json`), JSON.stringify(ev))
  expect(reviewOf(commit, dir, new Set([ev.pubkey]))).toBe("invalid: signed with an author key")
})

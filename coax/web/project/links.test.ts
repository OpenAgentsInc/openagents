// Work across projects: the tags a need travels in, the asks that find it,
// and the LINKS table (links.ts) on the committed fixture
// (coax/lib/nostr/fixtures/links.jsonl, links-fixture.ts), which the page's
// own fold (apps/openagents-web/project-links.coax) must match byte for byte.
import { expect, test } from "bun:test"
import { readFileSync } from "node:fs"
import { join } from "node:path"
import type { Event } from "nostr-effect/pure"
import { fieldsFrom, fieldTags, fold, seedItems, checkWork } from "./model"
import { firstAsk, linkRoots, needsOf, rootCoord, secondAsk } from "./links"
import { ALPHA, expected } from "./links-fixture"

const FIX = join(import.meta.dir, "../../../coax/lib/nostr/fixtures")
const { ours, far } = JSON.parse(readFileSync(join(FIX, "links.jsonl"), "utf8")) as { ours: Event[]; far: Event[] }
const coaxTable = JSON.parse(/LINKS_FIX_TABLE :: (".*");/.exec(readFileSync(join(FIX, "links.coax"), "utf8"))![1])

test("a need is a needs tag per issue and one q citation per project, and reads back", () => {
  const coord = `30621:${"b".repeat(64)}:verse`
  const f = { kind: "task" as const, title: "T", body: "b", labels: [], size: "", sats: 0, parent: "", blocked_by: [],
    needs: [{ id: "1".repeat(64), coord }, { id: "2".repeat(64), coord }] }
  const tags = fieldTags(f)
  expect(tags.filter((t) => t[0] === "needs")).toEqual([["needs", "1".repeat(64), coord], ["needs", "2".repeat(64), coord]])
  expect(tags.filter((t) => t[0] === "q")).toEqual([["q", coord]])
  expect(fieldsFrom(tags, "b").needs).toEqual(f.needs)
  // an issue without needs keeps exactly the tags it had: nothing to revise
  expect(fieldTags({ ...f, needs: [] }).some((t) => t[0] === "needs" || t[0] === "q")).toBe(false)
  // malformed needs are dropped
  expect(fieldsFrom([["needs", "xyz", coord], ["needs", "1".repeat(64), "30621:short:verse"]], "").needs).toEqual([])
})

test("a seed's needs are project:slug and land on the work item", () => {
  const items = seedItems({ initiative: { slug: "i", title: "I", body: "" }, milestones: [],
    tasks: [{ slug: "t", title: "T", body: "b", needs: ["verse:verse-particles"] }] })
  expect(items[1].needs).toEqual(["verse:verse-particles"])
  expect(() => checkWork([{ slug: "t", kind: "task", title: "T", body: "", needs: ["no-colon"] }])).toThrow(/bad need/)
})

test("the asks: needed ids and the #q of this project, then roots, comments and statuses", () => {
  const f = fold(ALPHA.pk, ALPHA.config, ours)
  const first = firstAsk(f, ALPHA.pk, ALPHA.config)
  expect(first[0]).toEqual({ kinds: [1621], ids: expect.arrayContaining([far[0].id, far[1].id, far[2].id, "ab".repeat(32)]) })
  expect(first[1]).toEqual({ kinds: [1621, 1111], "#q": [`30621:${ALPHA.pk}:alpha`] })
  // gamma's c2 cites alpha only in its revision: its root is asked for by id
  const cited = far.filter((e) => e.tags.some((t) => t[0] === "q"))
  const { roots, missing } = linkRoots(cited, ALPHA.pk)
  const c2 = far.find((e) => e.tags.some((t) => t[0] === "slug" && t[1] === "c2"))!
  expect(roots).toContain(c2.id)
  expect(missing).toContain(c2.id)
  const second = secondAsk(f, cited, ALPHA.pk)
  expect(second[0]).toEqual({ kinds: [1621], ids: missing })
  expect(second.slice(1).map((x) => Object.keys(x)[1])).toEqual(["#E", "#e"])
  expect(rootCoord(c2)).toBe(c2.tags.find((t) => t[0] === "a")![1])
  expect(needsOf(far.find((e) => e.tags.some((t) => t[0] === "slug" && t[1] === "b3"))!)).toHaveLength(1)
})

test("the LINKS table: needs in tag order, then who needs it by project; impostors count for nothing", () => {
  const table = expected(ours, far)
  const [a1, a2, a3] = ["a1", "a2", "a3"].map((s) => ours.find((e) => e.tags.some((t) => t[0] === "slug" && t[1] === s))!.id.slice(0, 4))
  const id = (s: string) => far.find((e) => e.tags.some((t) => t[0] === "slug" && t[1] === s))!.id.slice(0, 4)
  expect(table).toBe([
    `${a1}|by|beta|${id("b3")}|open|Beta three piped`,
    `${a1}|by|gamma|${id("c2")}|open|Gamma two, revised`,
    `${a2}|needs|beta|${id("b1")}|resolved|Beta one`,
    `${a2}|needs|gamma|${id("c1")}|open|Gamma one`,
    `${a2}|needs|gamma|abab|missing|`,
    `${a3}|needs|beta|${id("b2")}|closed|Beta two`,
  ].join("\n") + "\n")
  // the page's fold is checked against the same bytes (project-links.coax)
  expect(coaxTable).toBe(table)
})

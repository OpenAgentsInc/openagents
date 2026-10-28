// Writes the work-across-projects fixture both folds are checked on: three
// throwaway projects (alpha is the page's, beta and gamma are others), each
// event signed by its project's fixed test key, as relay frames.
//
//   coax/lib/nostr/fixtures/links.jsonl      the events, one a line (links.test.ts)
//   coax/lib/nostr/fixtures/links.coax       the same as frames for the page's
//                                            fold, and what links.ts makes of them
//
//   bun links-fixture.ts
//
// What it holds:
// - alpha #a1 is needed by beta's b3 (in its issue) and gamma's c2 (in a
//   revision); beta's b4 needed it once, but its latest revision doesn't.
// - alpha #a2 needs beta's b1 (resolved), gamma's c1 and an issue nobody has
//   (missing); alpha #a3 needs beta's b2 only through a revision, and b2's
//   title and status come from b2's own revision and status.
// - Impostors count for nothing: an issue signed by another key filed under
//   beta's address, and a status on b2 signed by another key.
import { finalizeEvent, getPublicKey, type Event } from "nostr-effect/pure"
import { hexToBytes } from "@noble/hashes/utils"
import { writeFileSync } from "node:fs"
import { join } from "node:path"
import { fieldTags, fold, ISSUE_REVISION, KIND_COMMENT, KIND_ISSUE, KIND_ORDER, KIND_PROJECT, projectCoord, type Fields,
  type ProjectConfig } from "./model"
import { linkTable } from "./links"
import { tables } from "./tables"

const FIX = join(import.meta.dir, "../../../coax/lib/nostr/fixtures")
const key = (n: number) => hexToBytes(n.toString(16).padStart(2, "0").repeat(32))
export const KEYS = { alpha: key(0x11), beta: key(0x22), gamma: key(0x33), mallory: key(0x44) }
const pk = (k: Uint8Array) => getPublicKey(k)
export const cfg = (slug: string): ProjectConfig => ({ slug, name: slug, description: "", relay: "wss://relay.example", license: "",
  repo: { d: "openagents", name: "OpenAgents", description: "", web: "", clone: "", euc: "" }, path: `${slug}/`, contributions: "open" })
export const ALPHA = { pk: pk(KEYS.alpha), config: cfg("alpha") }

let clock = 1_800_000_000
const sign = (k: Uint8Array, kind: number, tags: string[][], content = "") => finalizeEvent({ kind, tags, content, created_at: clock++ }, k)
const fields = (title: string, extra: Partial<Fields> = {}): Fields =>
  ({ kind: "task", title, body: `${title}.`, labels: [], size: "", sats: 0, parent: "", blocked_by: [], needs: [], ...extra })
const issue = (k: Uint8Array, slug: string, project: string, f: Fields, a = projectCoord(pk(k), cfg(project))) =>
  sign(k, KIND_ISSUE, [["a", a], ["p", pk(k)], ["slug", slug], ...fieldTags(f)], f.body)
const revision = (k: Uint8Array, root: Event, f: Fields) =>
  sign(k, KIND_COMMENT, [["E", root.id], ["K", "1621"], ["e", root.id], ["k", "1621"], ["revision", ISSUE_REVISION], ...fieldTags(f)], f.body)
const status = (k: Uint8Array, root: Event, kind: number) => sign(k, kind, [["e", root.id, "", "root"], ["p", root.pubkey]])

export function build() {
  const A = KEYS.alpha, B = KEYS.beta, G = KEYS.gamma
  const coord = (k: Uint8Array, s: string) => projectCoord(pk(k), cfg(s))
  const b1 = issue(B, "b1", "beta", fields("Beta one")), b2 = issue(B, "b2", "beta", fields("Beta two, first title"))
  const c1 = issue(G, "c1", "gamma", fields("Gamma one"))
  const nobody = "ab".repeat(32)
  const a1 = issue(A, "a1", "alpha", fields("Alpha one"))
  const a2 = issue(A, "a2", "alpha", fields("Alpha two", { needs: [{ id: b1.id, coord: coord(B, "beta") }, { id: c1.id, coord: coord(G, "gamma") },
    { id: nobody, coord: coord(G, "gamma") }] }))
  const a3 = issue(A, "a3", "alpha", fields("Alpha three"))
  const a3r = revision(A, a3, fields("Alpha three", { needs: [{ id: b2.id, coord: coord(B, "beta") }] }))
  const order = sign(A, KIND_ORDER, [["d", "alpha"], ["item", a1.id, "a"], ["item", a2.id, "b"], ["item", a3.id, "c"]])
  const project = sign(A, KIND_PROJECT, [["d", "alpha"], ["name", "alpha"]])
  const need = (e: Event) => [{ id: e.id, coord: coord(A, "alpha") }]
  const b3 = issue(B, "b3", "beta", fields("Beta three | piped", { needs: need(a1) }))
  const b4 = issue(B, "b4", "beta", fields("Beta four", { needs: need(a1) }))
  const b4r = revision(B, b4, fields("Beta four"))
  const c2 = issue(G, "c2", "gamma", fields("Gamma two"))
  const c2r = revision(G, c2, fields("Gamma two, revised", { needs: need(a1) }))
  const b1s = status(B, b1, 1631)
  const b2r = revision(B, b2, fields("Beta two"))
  const b2s = status(B, b2, 1632)
  const fake = issue(KEYS.mallory, "x", "beta", fields("Mallory", { needs: need(a1) }), coord(B, "beta"))
  const fakeStatus = status(KEYS.mallory, b2, 1630)
  const ours = [project, order, a1, a2, a3, a3r]
  const far = [b1, b2, c1, b3, b4, b4r, c2, c2r, b1s, b2r, b2s, fake, fakeStatus]
  return { ours, far }
}

/** What links.ts makes of the fixture: the LINKS table. */
export function expected(ours: Event[], far: Event[]): string {
  const f = fold(ALPHA.pk, ALPHA.config, ours)
  const t = tables(f)
  return linkTable(f, ALPHA.pk, ALPHA.config, far, [...f.issues].sort(t.rankOrder).map((i) => i.id), t.alias)
}

if (import.meta.main) {
  const { ours, far } = build()
  const frames = (sub: string, es: Event[]) => es.map((e) => JSON.stringify(["EVENT", sub, e])).join("\n") + "\n"
  writeFileSync(join(FIX, "links.jsonl"), JSON.stringify({ ours, far }) + "\n")
  const lit = (s: string) => JSON.stringify(s)
  writeFileSync(join(FIX, "links.coax"), `// Generated by coax/web/project/links-fixture.ts: do not edit.
// Work across projects: the page's own project (alpha) as frames, the other
// projects' events as frames, and the LINKS table links.ts makes of them.
LINKS_FIX_KEY :: ${lit(ALPHA.pk)};
LINKS_FIX_OURS :: ${lit(frames("issues", ours))};
LINKS_FIX_FAR :: ${lit(frames("links", far))};
LINKS_FIX_TABLE :: ${lit(expected(ours, far))};
`)
  console.log(expected(ours, far))
}

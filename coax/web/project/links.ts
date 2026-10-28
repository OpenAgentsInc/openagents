// Work one project waits on from another (docs/protocol/2026-10-28-projects.md,
// "Work across projects"). A waiting issue carries
//
//   ["needs", <the other issue's id>, "30621:<its project's key>:<its d>"]
//   ["q", "30621:<that key>:<that d>"]            once per project it needs
//
// on the issue and on every revision. This file is the reference for what a
// page shows of it, and the page's own fold (apps/openagents-web/project-links.coax)
// must make the same LINKS table from the same events, byte for byte.
//
// The events, from any key, come in two asks:
//   1. the issues this project needs, by id; and every issue or revision that
//      cites this project ({"kinds":[1621,1111],"#q":[<this project's address>]})
//   2. for every root found (and the roots of revisions found), the root itself
//      by id when it's missing, its comments (#E) and its statuses (#e)
// Only a root's own key counts for its revisions and statuses, and a root
// counts for the project its key and "a" tag name.
import { verifyEvent, type Event } from "nostr-effect/pure"
import { ISSUE_REVISION, KIND_COMMENT, KIND_ISSUE, projectCoord, STATUS_NAMES, type Folded, type ProjectConfig } from "./model"
import { cell } from "./tables"

export const LINK_ROOTS = 48   // roots the page follows at most (project-links.coax)
const COORD = /^30621:([0-9a-f]{64}):([a-z0-9][a-z0-9-]{0,63})$/

const tagOf = (e: Event, name: string) => e.tags.find((t) => t[0] === name)
/** The project a root is filed under: its first ["a", 30621:<its own key>:<d>]. */
export function rootCoord(e: Event): string {
  for (const t of e.tags) {
    const m = t[0] === "a" ? COORD.exec(t[1] ?? "") : null
    if (m && m[1] === e.pubkey) return t[1]
  }
  return ""
}
/** A list of needs from an issue's or revision's tags: [id, coord], in tag order. */
export function needsOf(e: Event): [string, string][] {
  return e.tags.filter((t) => t[0] === "needs" && /^[0-9a-f]{64}$/.test(t[1] ?? "") && COORD.test(t[2] ?? ""))
    .map((t) => [t[1], t[2]] as [string, string])
}
const newer = (a: Event, b: Event | undefined) => !b || a.created_at > b.created_at || (a.created_at === b.created_at && a.id < b.id)

/** The asks, as relay filters: first round, then the second given what came back. */
export function firstAsk(f: Folded, pk: string, c: ProjectConfig): Record<string, unknown>[] {
  const needed = [...new Set(f.issues.flatMap((i) => i.needs.map((n) => n.id)))].slice(0, LINK_ROOTS)
  const out: Record<string, unknown>[] = [{ kinds: [KIND_ISSUE, KIND_COMMENT], "#q": [projectCoord(pk, c)] }]
  if (needed.length) out.unshift({ kinds: [KIND_ISSUE], ids: needed })
  return out
}
export function linkRoots(events: Event[], pk: string): { roots: string[]; missing: string[] } {
  const have = new Set(events.filter((e) => e.kind === KIND_ISSUE).map((e) => e.id))
  const roots: string[] = []
  const add = (id: string) => { if (!roots.includes(id) && roots.length < LINK_ROOTS) roots.push(id) }
  for (const e of events) {
    if (e.pubkey === pk) continue
    if (e.kind === KIND_ISSUE) add(e.id)
    else if (e.kind === KIND_COMMENT && tagOf(e, "revision")?.[1] === ISSUE_REVISION) { const r = tagOf(e, "E")?.[1]; if (r && /^[0-9a-f]{64}$/.test(r)) add(r) }
  }
  return { roots, missing: roots.filter((id) => !have.has(id)) }
}
export function secondAsk(f: Folded, events: Event[], pk: string): Record<string, unknown>[] {
  const needed = [...new Set(f.issues.flatMap((i) => i.needs.map((n) => n.id)))].slice(0, LINK_ROOTS)
  const { roots, missing } = linkRoots(events, pk)
  const all = [...new Set([...needed, ...roots])].slice(0, LINK_ROOTS)
  if (!all.length) return []
  const out: Record<string, unknown>[] = [{ kinds: [KIND_COMMENT], "#E": all }, { kinds: [1630, 1631, 1632, 1633], "#e": all }]
  if (missing.length) out.unshift({ kinds: [KIND_ISSUE], ids: missing })
  return out
}

interface Root { e: Event; coord: string; fields: Event; status: string }

/** The LINKS table: for each of this project's issues in the order WORK_ITEMS lists them,
 *   alias | dir | project d | other alias | other status | other title
 * first what it needs (dir "needs", in tag order), then who needs it (dir "by",
 * by project d, then id). A need whose issue didn't come back is "missing". */
export function linkTable(f: Folded, pk: string, c: ProjectConfig, events: Event[], order: string[], alias: Map<string, string>): string {
  const ours = projectCoord(pk, c)
  const ok = events.filter((e) => verifyEvent(e))
  const roots = new Map<string, Root>()
  for (const e of ok) {
    if (e.kind !== KIND_ISSUE || roots.has(e.id)) continue
    const coord = rootCoord(e)
    if (!coord || coord === ours) continue
    let fields = e, status: Event | undefined
    for (const x of ok) {
      if (x.pubkey !== e.pubkey) continue
      if (x.kind === KIND_COMMENT && tagOf(x, "revision")?.[1] === ISSUE_REVISION && x.tags.some((t) => t[0] === "E" && t[1] === e.id) &&
        (fields === e || newer(x, fields))) fields = x
      if (x.kind >= 1630 && x.kind <= 1633 && x.tags.some((t) => t[0] === "e" && t[1] === e.id) && newer(x, status)) status = x
    }
    roots.set(e.id, { e, coord, fields, status: status ? STATUS_NAMES[status.kind] : "open" })
  }
  // an alias among the roots of the same project that came back: four hex digits, more on a clash
  const theirs = (r: Root) => {
    const pool = [...roots.values()].filter((o) => o.coord === r.coord).map((o) => o.e.id)
    let n = 4
    for (const o of pool) if (o !== r.e.id) while (n < 64 && o.slice(0, n) === r.e.id.slice(0, n)) n++
    return r.e.id.slice(0, n)
  }
  const d = (coord: string) => coord.slice(71)
  const title = (r: Root) => cell(tagOf(r.fields, "subject")?.[1] ?? "")
  const byId = new Map(f.issues.map((i) => [i.id, i]))
  const out: string[] = []
  for (const id of order) {
    const i = byId.get(id)
    if (!i) continue
    for (const n of i.needs) {
      const r = roots.get(n.id)
      const known = r && r.coord === n.coord ? r : undefined
      out.push([alias.get(id), "needs", d(n.coord), known ? theirs(known) : n.id.slice(0, 4), known ? known.status : "missing", known ? title(known) : ""].join("|") + "\n")
    }
    const by = [...roots.values()].filter((r) => needsOf(r.fields).some(([nid, nc]) => nid === id && nc === ours))
      .sort((a, b) => (d(a.coord) < d(b.coord) ? -1 : d(a.coord) > d(b.coord) ? 1 : a.e.id < b.e.id ? -1 : 1))
    for (const r of by) out.push([alias.get(id), "by", d(r.coord), theirs(r), r.status, title(r)].join("|") + "\n")
  }
  return out.join("")
}

/** Whether a need is still open: anything but resolved or closed. */
export const stillNeeded = (status: string) => status !== "resolved" && status !== "closed"

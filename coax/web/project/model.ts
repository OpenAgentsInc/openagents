// A project's public work on Nostr (docs/protocol/2026-10-28-projects.md,
// "Event model"): what the work files say, the events that carry it, and the
// fold that reads those events back into one current view.
//
//   kind 30617  NIP-34 repository announcement (the openagents repository)
//   kind 30621  NIP-MP project over it, with `path` and `contributions`
//   kind 1621   NIP-34 issue: one per initiative, milestone and task
//   kind 1111   NIP-22 comment marked ["revision", ISSUE_REVISION]: an edit
//               to an issue's subject, body, labels or relations
//   kind 1111   NIP-22 comment without that mark: a worker's note on an issue,
//               ["claim", NAME] when it takes the task, ["release"] when it
//               lets go, ["commit", HASH] when it lands the work
//   kind 1630-1633  NIP-34 status: open, resolved, closed, draft
//   kind 30623  the project's queue order: ["item", <issue id>, <rank>]
import { verifyEvent, type Event } from "nostr-effect/pure"
import { naddrEncode } from "nostr-effect/nip19"

export const KIND_REPO = 30617, KIND_PROJECT = 30621, KIND_ISSUE = 1621, KIND_COMMENT = 1111, KIND_ORDER = 30623
export const STATUS_KINDS: Record<string, number> = { open: 1630, resolved: 1631, closed: 1632, draft: 1633 }
export const STATUS_NAMES: Record<number, Status> = { 1630: "open", 1631: "resolved", 1632: "closed", 1633: "draft" }
export const ISSUE_REVISION = "openagents.issue-revision.v1"
export const WORK_SCHEMA = "openagents.project-work.v1"

export type Status = "open" | "resolved" | "closed" | "draft"
export type LogType = "open" | "revise" | "status" | "order" | "claim" | "release" | "note" | "done"
export type ItemKind = "initiative" | "milestone" | "task"

/** One entry of a work file (apps/openagents-web/project/<slug>/work/*.json). */
export interface WorkItem {
  slug: string
  kind: ItemKind
  title: string
  body: string
  labels?: string[]
  size?: string
  sats?: number
  parent?: string
  blocked_by?: string[]
  status?: Status
}

export interface ProjectConfig {
  slug: string
  name: string
  description: string
  relay: string
  repo: { d: string; name: string; description: string; web: string; clone: string; euc: string }
  path: string
  contributions: "open" | "invited" | "closed"
  license: string
}

/** What an issue says now: its first event's fields, overlaid by its latest revision. */
export interface Fields {
  kind: ItemKind
  title: string
  body: string
  labels: string[]
  size: string
  sats: number
  parent: string          // an issue id, or ""
  blocked_by: string[]    // issue ids
}

export interface Issue extends Fields {
  id: string
  slug: string
  created_at: number
  updated_at: number
  status: Status
  status_at: number
  rank: string
  claimed_by: string      // the worker on it now, or ""
  claimed_at: number
  commit: string          // the commit that landed it, or ""
  notes: { at: number; text: string; by: string }[]
}

export interface Folded {
  pubkey: string
  repo: Event | null
  project: Event | null
  order: Event | null
  issues: Issue[]
  log: { at: number; text: string; id: string; type: LogType }[]
}

export const repoCoord = (pk: string, c: ProjectConfig) => `${KIND_REPO}:${pk}:${c.repo.d}`
export const projectCoord = (pk: string, c: ProjectConfig) => `${KIND_PROJECT}:${pk}:${c.slug}`
export const naddr = (pk: string, c: ProjectConfig) =>
  naddrEncode({ kind: KIND_PROJECT, pubkey: pk, identifier: c.slug, relays: [c.relay] })

export function repoTags(c: ProjectConfig): string[][] {
  return [["d", c.repo.d], ["name", c.repo.name], ["description", c.repo.description], ["web", c.repo.web],
    ["clone", c.repo.clone], ["relays", c.relay], ["r", c.repo.euc, "euc"]]
}

export function projectTags(pk: string, c: ProjectConfig): string[][] {
  return [["d", c.slug], ["name", c.name], ["description", c.description], ["a", repoCoord(pk, c), c.relay],
    ["path", repoCoord(pk, c), c.path], ["contributions", c.contributions], ["license", c.license],
    ["buzz-visibility", "listed"]]
}

/** The tags that carry an item's fields, on its issue and on every revision. */
export function fieldTags(f: Fields): string[][] {
  const t: string[][] = [["subject", f.title]]
  if (f.kind !== "task") t.push(["t", f.kind])
  for (const l of f.labels) t.push(["t", l])
  if (f.size) t.push(["size", f.size])
  if (f.parent) t.push(["parent", f.parent])
  for (const b of f.blocked_by) t.push(["blocked-by", b])
  return t
}

export function fieldsFrom(tags: string[][], content: string): Fields {
  const all = tags.filter((t) => t[0] === "t").map((t) => t[1])
  const kind: ItemKind = all.includes("initiative") ? "initiative" : all.includes("milestone") ? "milestone" : "task"
  const one = (name: string) => tags.find((t) => t[0] === name)?.[1] ?? ""
  return {
    kind,
    title: one("subject"),
    body: content,
    labels: all.filter((l) => l !== "initiative" && l !== "milestone"),
    size: one("size"),
    sats: Number(one("reward") || 0),
    parent: one("parent"),
    blocked_by: tags.filter((t) => t[0] === "blocked-by").map((t) => t[1]),
  }
}

export function sameFields(a: Fields, b: Fields): boolean {
  // sats: rewards were dropped (owner, 2026-10-28); older issues still carry a
  // reward tag, and a revision without one clears it.
  return JSON.stringify(fieldTags(a)) === JSON.stringify(fieldTags(b)) && a.body === b.body && (a.sats || 0) === (b.sats || 0)
}

/** A seed file (project/<slug>/work/README.md, openagents.project-work-seed.v1) as work items. A task's
 * acceptance checks and the files it writes go in its body, where an agent reads them. */
export const SEED_SCHEMA = "openagents.project-work-seed.v1"
export interface Seed {
  initiative: { slug: string; title: string; body: string; spec?: string; labels?: string[] }
  milestones: { slug: string; title: string; body: string; parent?: string; labels?: string[] }[]
  tasks: { slug: string; parent?: string; title: string; size?: string; sats?: number; labels?: string[]; body: string;
           files?: string[]; acceptance?: string[]; blocked_by?: string[]; status?: Status }[]
}
export function seedItems(s: Seed): WorkItem[] {
  const i = s.initiative
  const spec = i.spec ? `\n\nSpecification: \`${i.spec}\` in the openagents repository.` : ""
  const out: WorkItem[] = [{ slug: i.slug, kind: "initiative", title: i.title, body: i.body + spec, labels: i.labels ?? [] }]
  for (const m of s.milestones) out.push({ slug: m.slug, kind: "milestone", title: m.title, body: m.body, labels: m.labels ?? [], parent: m.parent ?? i.slug })
  for (const t of s.tasks) {
    let body = t.body.trim()
    if (t.acceptance?.length) body += "\n\n## Acceptance\n\n" + t.acceptance.map((a) => `- ${a}`).join("\n")
    if (t.files?.length) body += "\n\n## Files\n\n" + t.files.map((f) => `- \`${f}\``).join("\n")
    out.push({ slug: t.slug, kind: "task", title: t.title, body, labels: t.labels ?? [], size: t.size, sats: t.sats,
      parent: t.parent, blocked_by: t.blocked_by ?? [], status: t.status })
  }
  return out
}

/** The work files' items, checked: unique slugs, known parents and blockers, no cycles. */
export function checkWork(items: WorkItem[]): WorkItem[] {
  const by = new Map<string, WorkItem>()
  for (const it of items) {
    if (!/^[a-z0-9][a-z0-9-]{0,63}$/.test(it.slug)) throw new Error(`bad slug ${JSON.stringify(it.slug)}`)
    if (by.has(it.slug)) throw new Error(`duplicate slug ${it.slug}`)
    if (!["initiative", "milestone", "task"].includes(it.kind)) throw new Error(`${it.slug}: bad kind ${it.kind}`)
    if (!it.title || it.title.length > 200) throw new Error(`${it.slug}: title missing or too long`)
    if (/\b(CX|M)-?\d+\b/.test(it.title)) throw new Error(`${it.slug}: no order numbers in titles`)
    by.set(it.slug, it)
  }
  for (const it of items) {
    if (it.parent && !by.has(it.parent)) throw new Error(`${it.slug}: unknown parent ${it.parent}`)
    for (const b of it.blocked_by ?? []) if (!by.has(b)) throw new Error(`${it.slug}: unknown blocker ${b}`)
  }
  topo(items)
  return items
}

/** Items with each one's parent and blockers first (they're published first: their ids go in its tags). */
export function topo(items: WorkItem[]): WorkItem[] {
  const by = new Map(items.map((i) => [i.slug, i]))
  const out: WorkItem[] = [], state = new Map<string, number>()
  const visit = (it: WorkItem, path: string[]) => {
    const s = state.get(it.slug)
    if (s === 2) return
    if (s === 1) throw new Error(`cycle: ${[...path, it.slug].join(" -> ")}`)
    state.set(it.slug, 1)
    for (const d of [it.parent, ...(it.blocked_by ?? [])]) if (d) visit(by.get(d)!, [...path, it.slug])
    state.set(it.slug, 2)
    out.push(it)
  }
  for (const it of items) visit(it, [])
  return out
}

/** Reads a project's events (signed by its key) into its current state. */
export function fold(pk: string, c: ProjectConfig, events: Event[]): Folded {
  const mine = events.filter((e) => e.pubkey === pk && verifyEvent(e))
  const latest = (pred: (e: Event) => boolean) =>
    mine.filter(pred).sort((a, b) => b.created_at - a.created_at || (a.id < b.id ? -1 : 1))[0] ?? null
  const d = (e: Event) => e.tags.find((t) => t[0] === "d")?.[1]
  const repo = latest((e) => e.kind === KIND_REPO && d(e) === c.repo.d)
  const project = latest((e) => e.kind === KIND_PROJECT && d(e) === c.slug)
  const order = latest((e) => e.kind === KIND_ORDER && d(e) === c.slug)
  const coord = projectCoord(pk, c)
  const roots = mine.filter((e) => e.kind === KIND_ISSUE && e.tags.some((t) => t[0] === "a" && t[1] === coord))
  const ranks = new Map((order?.tags ?? []).filter((t) => t[0] === "item").map((t) => [t[1], t[2]]))
  const log: Folded["log"] = []
  const issues: Issue[] = []
  // one issue per slug: if a slug was ever published twice, the first wins
  const seen = new Set<string>()
  for (const root of roots.sort((a, b) => a.created_at - b.created_at || (a.id < b.id ? -1 : 1))) {
    const slug = root.tags.find((t) => t[0] === "slug")?.[1] ?? root.id.slice(0, 8)
    if (seen.has(slug)) continue
    seen.add(slug)
    const revision = latest((e) => e.kind === KIND_COMMENT && e.tags.some((t) => t[0] === "revision" && t[1] === ISSUE_REVISION) &&
      e.tags.some((t) => t[0] === "E" && t[1] === root.id))
    const fields = revision ? fieldsFrom(revision.tags, revision.content) : fieldsFrom(root.tags, root.content)
    const status = latest((e) => e.kind >= 1630 && e.kind <= 1633 && e.tags.some((t) => t[0] === "e" && t[1] === root.id))
    const issue: Issue = { ...fields, id: root.id, slug, created_at: root.created_at, updated_at: revision?.created_at ?? root.created_at,
      status: status ? STATUS_NAMES[status.kind] : "open", status_at: status?.created_at ?? root.created_at, rank: ranks.get(root.id) ?? "",
      claimed_by: "", claimed_at: 0, commit: "", notes: [] }
    log.push({ at: root.created_at, text: `opened ${fields.kind}`, id: root.id, type: "open" })
    if (revision) log.push({ at: revision.created_at, text: "revised", id: root.id, type: "revise" })
    if (status && status.kind !== 1630) log.push({ at: status.created_at, text: STATUS_NAMES[status.kind], id: root.id, type: "status" })
    // the workers' comments, oldest first: claims, releases, notes, commits
    const comments = mine.filter((e) => e.kind === KIND_COMMENT && !e.tags.some((t) => t[0] === "revision") &&
      e.tags.some((t) => t[0] === "E" && t[1] === root.id)).sort((a, b) => a.created_at - b.created_at || (a.id < b.id ? -1 : 1))
    for (const e of comments) {
      const tag = (name: string) => e.tags.find((t) => t[0] === name)
      const by = tag("by")?.[1] ?? tag("claim")?.[1] ?? issue.claimed_by
      if (tag("claim")) {
        issue.claimed_by = tag("claim")![1]; issue.claimed_at = e.created_at
        log.push({ at: e.created_at, text: `claimed by ${issue.claimed_by}`, id: root.id, type: "claim" })
      } else if (tag("release")) {
        log.push({ at: e.created_at, text: `released by ${issue.claimed_by || "its worker"}`, id: root.id, type: "release" })
        issue.claimed_by = ""; issue.claimed_at = 0
      } else if (tag("commit")) {
        issue.commit = tag("commit")![1]
        log.push({ at: e.created_at, text: `done in ${issue.commit.slice(0, 10)}${by ? ` by ${by}` : ""}`, id: root.id, type: "done" })
      } else if (e.content.trim()) {
        log.push({ at: e.created_at, text: `${by ? by + ": " : ""}${e.content.trim()}`, id: root.id, type: "note" })
      }
      if (e.content.trim() && !tag("claim") && !tag("release")) issue.notes.push({ at: e.created_at, text: e.content.trim(), by })
    }
    // a finished task has nobody on it
    if (issue.status === "resolved" || issue.status === "closed") issue.claimed_by = ""
    issues.push(issue)
  }
  if (order) log.push({ at: order.created_at, text: "reordered the queue", id: "", type: "order" })
  log.sort((a, b) => b.at - a.at)
  return { pubkey: pk, repo, project, order, issues, log }
}

/** Short aliases from event ids, git style: 4 hex digits, longer only where two would collide. */
export function aliases(ids: string[]): Map<string, string> {
  const out = new Map<string, string>()
  for (const id of ids) {
    let n = 4
    while (ids.some((o) => o !== id && o.slice(0, n) === id.slice(0, n))) n++
    out.set(id, id.slice(0, n))
  }
  return out
}

/** The filters that fetch everything a project's page and tools read. */
export function filters(pk: string, c: ProjectConfig): Record<string, unknown>[] {
  return [
    { kinds: [KIND_REPO], authors: [pk], "#d": [c.repo.d] },
    { kinds: [KIND_PROJECT, KIND_ORDER], authors: [pk], "#d": [c.slug] },
    { kinds: [KIND_ISSUE], authors: [pk], "#a": [projectCoord(pk, c)] },
  ]
}

/** Revisions and statuses hang off issue ids, so they're a second query. */
export function followups(pk: string, ids: string[]): Record<string, unknown>[] {
  if (ids.length === 0) return []
  return [
    { kinds: [KIND_COMMENT], authors: [pk], "#E": ids },
    { kinds: [1630, 1631, 1632, 1633], authors: [pk], "#e": ids },
  ]
}

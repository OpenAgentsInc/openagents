// The /projects list, until the page reads the relay itself (the live fold,
// task project-page-live): find every project on the relay (kind 30621),
// verify it, fold each one's tasks with the same model the project pages use
// (model.ts) and write
//
//   apps/openagents-web/projects/list.coax   the table projects.coax draws
//   apps/openagents-web/projects/index.md    the same, for agents
//
//   bun list.ts [--relay URL] [--if-older MINUTES]
//
// The table (PROJECTS_ROWS), one line a project, fields split by '|':
//   slug | name | description | contributions | visibility | open | ready | done |
//   working | last active (unix seconds, ten digits) | last active (Chicago) | page | owner | members
// visibility is public or unlisted (private projects are left out); page is the site's URL for it, or empty;
// owner is the signer's hex pubkey; members are hex pubkeys split by spaces
// (the member repositories' maintainers, and anyone p-tagged on the project or
// on a claim or commit comment). Rows come newest activity first.
//
// Private projects (docs/protocol/2026-10-28-projects.md, "Who sees a
// project") never reach this file: the bucket is public. With the relay
// unreachable the last list stays.
import { verifyEvent, type Event } from "nostr-effect/pure"
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { Relay } from "./relay"
import { filters, fold, followups, KIND_COMMENT, KIND_ISSUE, KIND_PROJECT, KIND_REPO, type Issue, type ProjectConfig } from "./model"

const ROOT = join(import.meta.dir, "../../..")
const SITE = join(ROOT, "apps/openagents-web/projects")
const args = process.argv.slice(2)
const flag = (name: string, fallback = "") => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : fallback }
const relayUrl = flag("relay", "wss://relay.openagents.com")
const out = join(SITE, "list.coax")
const older = Number(flag("if-older", "0"))
if (older > 0 && existsSync(out) && Date.now() - statSync(out).mtimeMs < older * 60000) process.exit(0)

const tag = (e: Event, name: string) => e.tags.find((t) => t[0] === name)?.[1] ?? ""
const hex = /^[0-9a-f]{64}$/

/** Who may see a project: what it says (public, unlisted or private), else private when it takes no outside work. */
export function visibility(e: Event): "public" | "unlisted" | "private" {
  const v = tag(e, "visibility"), c = tag(e, "contributions") || "closed"
  if (v === "public" || v === "unlisted" || v === "private") return v
  if (v) return "private"
  return c === "open" || c === "invited" ? "public" : "private"
}

interface Row { slug: string; name: string; description: string; contributions: string; visibility: string; open: number; ready: number;
  done: number; working: string[]; last: number; page: string; owner: string; members: string[] }

let rows: Row[] = []
try {
  const relay = await Relay.open(relayUrl, null)
  // the latest valid head per (signer, d): exactly one d tag (NIP-MP d-cardinality)
  const heads = new Map<string, Event>()
  for (const e of await relay.all([{ kinds: [KIND_PROJECT] }])) {
    if (!verifyEvent(e) || e.tags.filter((t) => t[0] === "d").length !== 1 || !tag(e, "d")) continue
    const k = `${e.pubkey}:${tag(e, "d")}`, had = heads.get(k)
    if (!had || e.created_at > had.created_at || (e.created_at === had.created_at && e.id < had.id)) heads.set(k, e)
  }
  for (const p of heads.values()) {
    const slug = tag(p, "d")
    const repos = p.tags.filter((t) => t[0] === "a" && t[1].startsWith(`${KIND_REPO}:`)).map((t) => t[1])
    const own = repos.map((c) => c.split(":")).find((c) => c[1] === p.pubkey)
    const config = { slug, relay: relayUrl, repo: { d: own?.[2] ?? "" } } as ProjectConfig
    const first = await relay.all(filters(p.pubkey, config))
    const ids = first.filter((e) => e.kind === KIND_ISSUE).map((e) => e.id)
    const more: Event[] = []
    for (let i = 0; i < ids.length; i += 50) more.push(...await relay.all(followups(p.pubkey, ids.slice(i, i + 50))))
    const f = fold(p.pubkey, config, [...first, ...more])
    // members: the member repositories' maintainers, and the p tags on the project and on claims and commits
    const members = new Set<string>()
    for (const c of repos) {
      const [, pk, d] = c.split(":")
      for (const r of await relay.all([{ kinds: [KIND_REPO], authors: [pk], "#d": [d] }]))
        if (verifyEvent(r)) for (const t of r.tags) if (t[0] === "maintainers") t.slice(1).forEach((m) => members.add(m))
    }
    for (const t of p.tags) if (t[0] === "p") members.add(t[1])
    for (const e of more) if (e.kind === KIND_COMMENT && e.pubkey === p.pubkey && verifyEvent(e) && (tag(e, "claim") || tag(e, "commit")))
      for (const t of e.tags) if (t[0] === "p") members.add(t[1])
    members.delete(p.pubkey)
    const done = (i: Issue | undefined) => !!i && (i.status === "resolved" || i.status === "closed")
    const by = new Map(f.issues.map((i) => [i.id, i]))
    const tasks = f.issues.filter((i) => i.kind === "task")
    // work from other projects (links.ts): a need counts until its issue, by its own key, is resolved or closed
    const needed = [...new Set(tasks.flatMap((i) => i.needs.map((n) => n.id)))]
    const farDone = new Set<string>()
    if (needed.length) {
      const far = (await relay.all([{ kinds: [KIND_ISSUE], ids: needed }, { kinds: [1630, 1631, 1632, 1633], "#e": needed }])).filter((e) => verifyEvent(e))
      for (const root of far.filter((e) => e.kind === KIND_ISSUE)) {
        const last = far.filter((e) => e.kind >= 1630 && e.pubkey === root.pubkey && e.tags.some((t) => t[0] === "e" && t[1] === root.id))
          .sort((a, b) => b.created_at - a.created_at || (a.id < b.id ? -1 : 1))[0]
        if (last && (last.kind === 1631 || last.kind === 1632)) farDone.add(root.id)
      }
    }
    const ready = tasks.filter((i) => i.status === "open" && !i.claimed_by && i.blocked_by.every((b) => done(by.get(b))) &&
      i.needs.every((n) => farDone.has(n.id)))
    const working = [...new Set(tasks.filter((i) => !done(i) && i.claimed_by).map((i) => i.claimed_by))]
    // the site's page for it: only when the site's own copy of the project names this signer
    let page = ""
    const local = join(SITE, slug, "project.json")
    if (existsSync(local) && JSON.parse(readFileSync(local, "utf8")).pubkey === p.pubkey)
      page = existsSync(join(SITE, slug, "index.html")) ? `/projects/${slug}` : `/projects/${slug}/index.md`
    rows.push({ slug, name: tag(p, "name") || slug, description: tag(p, "description"), contributions: tag(p, "contributions") || "closed",
      visibility: visibility(p), open: tasks.filter((i) => !done(i)).length, ready: ready.length, done: tasks.filter(done).length,
      working, last: Math.max(p.created_at, ...f.log.map((l) => l.at)), page, owner: p.pubkey,
      members: [...members].filter((m) => hex.test(m)) })
  }
  relay.close()
} catch (e) {
  console.error(`projects list: ${(e as Error).message}; keeping the last list`)
  process.exit(existsSync(out) ? 0 : 1)
}

// Unlisted rows go in the table, where projects.coax shows them only to members (MINE), and never in the Markdown index.
rows = rows.filter((r) => r.visibility === "public" || r.visibility === "unlisted").sort((a, b) => b.last - a.last || (a.slug < b.slug ? -1 : 1))

const chicago = (d: Date) => {
  const p = Object.fromEntries(new Intl.DateTimeFormat("en-US", { timeZone: "America/Chicago", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", hour12: false, timeZoneName: "short" }).formatToParts(d).map((x) => [x.type, x.value]))
  return `${p.month}-${p.day} ${p.hour === "24" ? "00" : p.hour}:${p.minute} ${p.timeZoneName}`
}
const plain = (s: string) => s.replace(/[×]/g, "x").replace(/[–—]/g, "-").replace(/[’‘]/g, "'").replace(/[“”]/g, '"')
  .replace(/[→]/g, "->").replace(/[·]/g, "-").replace(/[…]/g, "...").replace(/[^\x20-\x7e]/g, "")
const cell = (s: string) => plain(s).replace(/[|\n\r]/g, " ").replace(/\s+/g, " ").trim()
const at = new Date()
const table = rows.map((r) => [r.slug, cell(r.name), cell(r.description), r.contributions, r.visibility, r.open, r.ready, r.done,
  cell(r.working.join(", ")), String(r.last).padStart(10, "0"), chicago(new Date(r.last * 1000)).slice(0, 11), r.page, r.owner,
  r.members.join(" ")].join("|") + "\n").join("")

writeFileSync(out, `// Generated by coax/web/project/list.ts from ${relayUrl}. Do not edit; run it.
// Rows: slug | name | description | contributions | visibility | open | ready | done | working | last active (unix) | last active (Chicago) | page | owner | members
PROJECTS_AT :: ${JSON.stringify(chicago(at))};
PROJECTS_RELAY :: ${JSON.stringify(relayUrl)};
PROJECTS_ROWS :: ${JSON.stringify(table)};
`)

const md: string[] = [`# Projects`, ``, `OpenAgents projects that people and agents can pick up work from, read from \`${relayUrl}\` at ${chicago(at)}: every kind 30621 project event there, signature checked, that is public (\`contributions\` open or invited, or marked public).`, ``]
const listed = rows.filter((r) => r.visibility === "public")
for (const r of listed) {
  md.push(`## ${r.name}`, ``, r.description, ``)
  md.push(`- Page: ${r.page ? `https://openagents.com${r.page}` : "none on this site yet"}`)
  if (r.page.startsWith("/projects/") && !r.page.endsWith(".md")) md.push(`- Tasks: https://openagents.com${r.page}/index.md`)
  md.push(`- Contributions: ${r.contributions}`)
  md.push(`- Tasks: ${r.open} open, ${r.ready} ready, ${r.done} done`)
  md.push(`- Working now: ${r.working.length ? r.working.join(", ") : "nobody"}`)
  md.push(`- Last active: ${chicago(new Date(r.last * 1000))}`)
  md.push(`- Project event: kind 30621, \`d\` = \`${r.slug}\`, signed by \`${r.owner}\``, ``)
}
if (listed.length === 0) md.push(`No public projects are published yet.`, ``)
writeFileSync(join(SITE, "index.md"), md.join("\n"))
console.log(`projects list ${chicago(at)}: ${listed.length} public (${listed.map((r) => r.slug).join(", ")}), ${rows.length - listed.length} unlisted`)

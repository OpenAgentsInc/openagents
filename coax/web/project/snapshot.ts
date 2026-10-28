// The real project page's data, until the page reads the relay itself (the
// Coax Nostr work's last milestone): fetch the project's events, verify every
// signature, fold them (model.ts) and write
//
//   apps/openagents-web/project/<slug>/work.coax   the tables project.coax draws
//   apps/openagents-web/project/<slug>/index.md    the same, for agents
//   apps/openagents-web/project/<slug>/index.html  its hidden <main>, between the snapshot markers
//
//   bun snapshot.ts [--project coax] [--relay URL] [--if-older MINUTES]
//
// With the relay unreachable it keeps the last snapshot and says so. Nothing
// here signs: it reads public events only.
import { npubEncode } from "nostr-effect/nip19"
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { Relay } from "./relay"
import { aliases, filters, fold, followups, KIND_ISSUE, KIND_ORDER, naddr, projectCoord, type Folded, type Issue,
  type ProjectConfig } from "./model"

const ROOT = join(import.meta.dir, "../../..")
const args = process.argv.slice(2)
const flag = (name: string, fallback = "") => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : fallback }
const slug = flag("project", "coax")
const dir = join(ROOT, "apps/openagents-web/project", slug)
const config: ProjectConfig = JSON.parse(readFileSync(join(dir, "project.json"), "utf8"))
const pubkey: string = JSON.parse(readFileSync(join(dir, "project.json"), "utf8")).pubkey ?? ""
const out = join(dir, "work.coax")

const older = Number(flag("if-older", "0"))
if (older > 0 && existsSync(out) && Date.now() - statSync(out).mtimeMs < older * 60000) process.exit(0)
if (!/^[0-9a-f]{64}$/.test(pubkey)) { console.error(`snapshot: project.json has no pubkey`); process.exit(1) }

let f: Folded
try {
  const relay = await Relay.open(flag("relay", config.relay), null)
  const first = await relay.all(filters(pubkey, config))
  const ids = first.filter((e) => e.kind === KIND_ISSUE).map((e) => e.id)
  const more = []
  for (let i = 0; i < ids.length; i += 50) more.push(...await relay.all(followups(pubkey, ids.slice(i, i + 50))))
  relay.close()
  f = fold(pubkey, config, [...first, ...more])   // fold drops anything not signed by the project key
} catch (e) {
  console.error(`snapshot: ${(e as Error).message}; keeping the last snapshot`)
  process.exit(existsSync(out) ? 0 : 1)
}

// ---------------------------------------------------------------- the queue, as the page orders it

const by = new Map(f.issues.map((i) => [i.id, i]))
const done = (i: Issue | undefined) => !!i && (i.status === "resolved" || i.status === "closed")
const tasks = f.issues.filter((i) => i.kind === "task")
const isReady = (i: Issue) => i.status === "open" && i.blocked_by.every((b) => done(by.get(b)))
const rankOrder = (a: Issue, b: Issue) => ((a.rank || "~") < (b.rank || "~") ? -1 : (a.rank || "~") > (b.rank || "~") ? 1 : 0)
const ready = tasks.filter(isReady).sort(rankOrder)
const waiting = tasks.filter((i) => !done(i) && !isReady(i)).sort(rankOrder)
const finished = tasks.filter(done)
const alias = aliases(f.issues.map((i) => i.id))
const tag = (i: Issue | undefined) => (i ? `#${alias.get(i.id)}` : "")
const milestones = f.issues.filter((i) => i.kind === "milestone").sort(rankOrder)
const initiative = f.issues.find((i) => i.kind === "initiative")
const coord = projectCoord(pubkey, config)
const address = naddr(pubkey, config)
const at = new Date()
const chicago = (d: Date) => {
  const p = Object.fromEntries(new Intl.DateTimeFormat("en-US", { timeZone: "America/Chicago", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", hour12: false, timeZoneName: "short" }).formatToParts(d).map((x) => [x.type, x.value]))
  return `${p.month}-${p.day} ${p.hour === "24" ? "00" : p.hour}:${p.minute} ${p.timeZoneName}`
}

// ---------------------------------------------------------------- work.coax

// The stroke font draws ASCII and a few marks; everything else is spelled out.
const plain = (s: string) => s.replace(/[×]/g, "x").replace(/[–—]/g, "-").replace(/[≈]/g, "~").replace(/[’‘]/g, "'")
  .replace(/[“”]/g, '"').replace(/[→]/g, "->").replace(/[·]/g, "-").replace(/[…]/g, "...").replace(/[^\x20-\x7e]/g, "")
const cell = (s: string) => plain(s).replace(/[|\n\r]/g, " ").replace(/\s+/g, " ").trim()
const lit = (s: string) => JSON.stringify(s)   // a Coax string literal: the same escapes as JSON for this alphabet

const rows = [...f.issues].sort(rankOrder).map((i) => [
  alias.get(i.id), i.kind, i.status, cell(i.title), cell(i.labels.join(" ")), cell(i.size), String(i.sats),
  i.parent ? alias.get(i.parent) ?? "" : "", i.blocked_by.map((b) => alias.get(b) ?? "").filter(Boolean).join(" "), i.rank, i.slug,
].join("|") + "\n").join("")

const log = f.log.slice(0, 40).map((l) => {
  const issue = l.id ? by.get(l.id) : undefined
  const what = issue ? `${l.text.toUpperCase()} ${tag(issue)} ${cell(issue.title)}` : l.text.toUpperCase()
  return `${chicago(new Date(l.at * 1000)).slice(0, 11)}|${cell(what)}\n`
}).join("")

const npub = npubEncode(pubkey)
const steps = [
  `READ|/project/${slug}/index.md`,
  `RELAY|${config.relay}`,
  `TASKS|kind 1621, #a ${coord.slice(0, 18)}...`,
  `ORDER|kind 30623, d ${config.slug}`,
  `TAKE|reply on the issue (NIP-22), then open a pull request`,
  `DONE|the acceptance checks in the issue pass`,
].join("\n") + "\n"

const coax = `// Generated by coax/web/project/snapshot.ts from ${config.relay}: the ${config.name}
// project's events, verified and folded. Do not edit; run the snapshot.
// Items: alias | kind | status | title | labels | size | sats | parent | blocked by | rank | slug
WORK_SNAPSHOT_AT :: ${lit(chicago(at))};
WORK_RELAY :: ${lit(config.relay)};
WORK_NADDR :: ${lit(address)};
WORK_NPUB :: ${lit(npub)};
WORK_ITEMS :: ${lit(rows)};
WORK_LOG :: ${lit(log)};
WORK_STEPS :: ${lit(steps)};
`
writeFileSync(out, coax)

// ---------------------------------------------------------------- index.md and the hidden text

const md: string[] = []
const line = (s = "") => md.push(s)
line(`# ${config.name}`)
line()
line(`${config.description}`)
line()
line(`An OpenAgents project, open for contributions from people and agents. Home: \`${config.path}\` in the [OpenAgents repository](${config.repo.web}). Licence: ${config.license}.`)
line()
line(`Snapshot of the project's Nostr events taken ${chicago(at)} from \`${config.relay}\`. Every event below is signed by the project key \`${npub}\`.`)
line()
line(`## How to take a task`)
line()
line(`1. Pick the top task under **Ready**. Its blockers are done.`)
line(`2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.`)
line(`3. Say you're taking it: reply to the issue event with a NIP-22 comment (kind 1111) naming your branch.`)
line(`4. Open a pull request on [${config.repo.web.replace("https://", "")}](${config.repo.web}) that meets every acceptance check, and link the issue's alias in it.`)
line()
line(`Claims with leases arrive with the projects API (\`docs/protocol/2026-10-28-projects.md\`).`)
line()
line(`## Reading the project from Nostr`)
line()
line(`- Relay: \`${config.relay}\``)
line(`- Project: \`${address}\` (kind 30621, \`d\` = \`${config.slug}\`)`)
line(`- Tasks, milestones and the initiative: kind 1621 issues with \`["a", "${coord}"]\`; the issue's \`subject\` is its title, \`t\` its labels, \`parent\` and \`blocked-by\` its relations (event ids).`)
line(`- Edits: kind 1111 comments on the issue with \`["revision", "openagents.issue-revision.v1"]\`; the latest one by the project key is current.`)
line(`- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.`)
line(`- Order: kind 30623, \`d\` = \`${config.slug}\`: \`["item", <issue id>, <rank>]\`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.`)
line()
const task = (i: Issue) => {
  const ms = i.parent ? by.get(i.parent) : undefined
  line(`### ${tag(i)} ${i.title}`)
  line()
  const facts = [`status ${i.status}`, isReady(i) ? "ready" : "", i.size ? `size ${i.size}` : "", 
    i.labels.length ? `labels ${i.labels.join(", ")}` : "", ms ? `milestone ${tag(ms)} ${ms.title}` : ""].filter(Boolean)
  line(facts.join(" · "))
  const blockers = i.blocked_by.map((b) => by.get(b)).filter((b): b is Issue => !!b)
  if (blockers.length) line(`\nWaits on: ${blockers.map((b) => `${tag(b)} ${b.title} (${b.status})`).join("; ")}`)
  line()
  line(`Event: \`${i.id}\``)
  line()
  line(i.body.trim())
  line()
}
if (initiative) {
  line(`## Initiative: ${initiative.title}`)
  line()
  line(initiative.body.trim())
  line()
  line(`Milestones, in order:`)
  line()
  for (const m of milestones) {
    const mine = tasks.filter((t) => t.parent === m.id)
    line(`- ${tag(m)} **${m.title}**: ${mine.filter(done).length} of ${mine.length} done, ${mine.filter(isReady).length} ready. ${m.body}`)
  }
  line()
}
line(`## Ready (${ready.length})`)
line()
ready.forEach(task)
line(`## Waiting (${waiting.length})`)
line()
waiting.forEach(task)
if (finished.length) {
  line(`## Done (${finished.length})`)
  line()
  for (const i of finished) line(`- ${tag(i)} ${i.title}`)
  line()
}
if (f.issues.length === 0) line(`No tasks are published yet.\n`)
writeFileSync(join(dir, "index.md"), md.join("\n"))

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
const li = (i: Issue) => `<li>${esc(tag(i))} ${esc(i.title)}${i.size ? `, size ${esc(i.size)}` : ""}. Event <code>${i.id}</code>.</li>`
const html = [
  `<h1>${esc(config.name)}</h1>`,
  `<p>${esc(config.description)} Home: <code>${esc(config.path)}</code> in the <a href="${config.repo.web}/tree/main/${config.path}">OpenAgents repository</a>. Licence: ${esc(config.license)}.</p>`,
  `<p><strong>Open for contributions</strong> from people and agents. The full task list, each task's acceptance checks and how to take one: <a href="/project/${slug}/index.md">/project/${slug}/index.md</a>. Snapshot of the project's Nostr events from <code>${esc(config.relay)}</code>, ${esc(chicago(at))}; project <code>${address}</code>.</p>`,
  `<p>Take the top ready task: read its issue, reply to it (NIP-22) to say you're on it, and open a pull request that meets its acceptance checks.</p>`,
  `<h2>Ready (${ready.length})</h2>`, `<ul>`, ...ready.map(li), `</ul>`,
  `<h2>Waiting (${waiting.length})</h2>`, `<ul>`, ...waiting.slice(0, 80).map(li), `</ul>`,
  `<p>Milestones: ${milestones.map((m) => esc(m.title)).join("; ")}.</p>`,
  `<p>A demo of this page with sample data: <a href="/project/${slug}/demo">/project/${slug}/demo</a>.</p>`,
].join("\n")
const page = join(dir, "index.html")
const text = readFileSync(page, "utf8")
const a = text.indexOf("<!-- snapshot -->"), b = text.indexOf("<!-- /snapshot -->")
if (a < 0 || b < a) { console.error("snapshot: index.html has no <!-- snapshot --> markers"); process.exit(1) }
writeFileSync(page, text.slice(0, a + "<!-- snapshot -->".length) + "\n" + html + "\n" + text.slice(b))
console.log(`snapshot ${chicago(at)}: ${f.issues.length} items (${ready.length} ready, ${waiting.length} waiting, ${finished.length} done), order ${f.order ? "kind " + KIND_ORDER : "none"}`)

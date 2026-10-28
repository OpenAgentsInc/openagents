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
import { aliases, filters, fold, followups, KIND_ISSUE, KIND_ORDER, naddr, planDone, projectCoord, type Folded, type Issue,
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
const working = tasks.filter((i) => !done(i) && i.claimed_by).sort(rankOrder)
const ready = tasks.filter((i) => isReady(i) && !i.claimed_by).sort(rankOrder)
const waiting = tasks.filter((i) => !done(i) && !isReady(i) && !i.claimed_by).sort(rankOrder)
const finished = tasks.filter(done).sort((a, b) => b.status_at - a.status_at)
const alias = aliases(f.issues.map((i) => i.id))
const tag = (i: Issue | undefined) => (i ? `#${alias.get(i.id)}` : "")
const milestones = f.issues.filter((i) => i.kind === "milestone").sort(rankOrder)
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

// done at: when it was resolved or closed, as ten digits so the page orders them as text
const doneAt = (i: Issue) => (done(i) ? String(i.status_at).padStart(10, "0") : "")
const rows = [...f.issues].sort(rankOrder).map((i) => [
  alias.get(i.id), i.kind, i.status, cell(i.title), cell(i.labels.join(" ")), cell(i.size), cell(i.claimed_by),
  i.parent ? alias.get(i.parent) ?? "" : "", i.blocked_by.map((b) => alias.get(b) ?? "").filter(Boolean).join(" "), i.rank, i.slug,
  i.commit.slice(0, 10), doneAt(i),
].join("|") + "\n").join("")

// The activity: what workers did (claims, releases, notes, finished tasks),
// newest first. A status without a commit beside it counts too (the
// taskmaster resolving or closing by hand); a done task's own resolve doesn't.
const worked = (l: Folded["log"][number]) => {
  if (l.type === "claim" || l.type === "release" || l.type === "note" || l.type === "done" || l.type === "plan") return true
  if (l.type !== "status") return false
  const i = by.get(l.id)
  return !(i && i.commit && f.log.some((o) => o.type === "done" && o.id === l.id && Math.abs(o.at - l.at) < 120))
}
const activity = f.log.filter(worked)
const said = (l: Folded["log"][number], i: Issue) =>
  l.type === "note" ? l.text : l.type === "done" ? l.text.replace(/^done in (\S+)/, (_, h) => `DONE IN ${h}`).replace(/ by /, " BY ") : l.text.toUpperCase()
const log = activity.slice(0, 40).map((l) => {
  const issue = l.id ? by.get(l.id) : undefined
  const what = issue ? `${tag(issue)} ${said(l, issue)} - ${cell(issue.title)}` : l.text.toUpperCase()
  return `${chicago(new Date(l.at * 1000)).slice(0, 11)}|${cell(what)}\n`
}).join("")

// The plans of the tasks in progress: each step, and a line about the plan.
// Marks: x done, > doing, o to do.
const PLAN_STEPS = 24
const plans = working.flatMap((i) => (i.plan?.steps ?? []).slice(0, PLAN_STEPS)
  .map((st) => `${alias.get(i.id)}|${st.mark === " " ? "o" : st.mark}|${cell(st.text)}\n`)).join("")
const lastNote = (i: Issue) => i.notes.filter((n) => !/^Done in /.test(n.text)).at(-1)
const planInfo = working.map((i) => {
  const st = i.plan?.steps ?? [], n = lastNote(i)
  return [alias.get(i.id), planDone(st), st.length, i.plan ? chicago(new Date(i.plan.at * 1000)).slice(6, 11) : "",
    chicago(new Date(i.claimed_at * 1000)).slice(6, 11), n ? cell(n.text) : ""].join("|") + "\n"
}).join("")

const npub = npubEncode(pubkey)
const steps = [
  `READ|/project/${slug}/index.md`,
  `RELAY|${config.relay}`,
  `TASKS|kind 1621, #a ${coord.slice(0, 18)}...`,
  `ORDER|kind 30623, d ${config.slug}`,
  `TAKE|a NIP-22 comment on the issue: ["claim", NAME]`,
  `PLAN|["plan", "openagents.task-plan.v1"]: your to-do list`,
  `NOTE|comments on the issue as you go`,
  `DONE|the checks pass; ["commit", HASH], status 1631`,
].join("\n") + "\n"

const coax = `// Generated by coax/web/project/snapshot.ts from ${config.relay}: the ${config.name}
// project's events, verified and folded. Do not edit; run the snapshot.
// Items: alias | kind | status | title | labels | size | worker | parent | blocked by | rank | slug | commit | done at
// Log: the workers' activity, newest first: time (Chicago) | what
// Plans: the steps of each task in progress: alias | mark (x done, > doing, o to do) | step
// Plan info: one row a task in progress: alias | done | steps | plan updated | claimed | latest note
WORK_SNAPSHOT_AT :: ${lit(chicago(at))};
WORK_RELAY :: ${lit(config.relay)};
WORK_NADDR :: ${lit(address)};
WORK_NPUB :: ${lit(npub)};
WORK_ITEMS :: ${lit(rows)};
WORK_LOG :: ${lit(log)};
WORK_PLANS :: ${lit(plans)};
WORK_PLAN_INFO :: ${lit(planInfo)};
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
line(`1. Pick the top task under **Ready** that nobody is on. Its blockers are done.`)
line(`2. Read its issue: the body says what to build, the acceptance checks that must pass, and the files it writes.`)
line(`3. Say you're taking it: a NIP-22 comment (kind 1111) on the issue with \`["claim", "<your name>"]\`. Tasks under **In progress** are taken.`)
line(`   Then post your plan, a checklist comment with \`["plan", "openagents.task-plan.v1"]\`, and keep it current as you work: the page shows it under the task.`)
line(`4. Leave comments on the issue as you go, then open a pull request on [${config.repo.web.replace("https://", "")}](${config.repo.web}) that meets every acceptance check, and link the issue's alias in it.`)
line()
line(`The project's own workers follow \`coax/web/project/QUEUE.md\` in the repository. Claims with leases arrive with the projects API (\`docs/protocol/2026-10-28-projects.md\`).`)
line()
line(`## Reading the project from Nostr`)
line()
line(`- Relay: \`${config.relay}\``)
line(`- Project: \`${address}\` (kind 30621, \`d\` = \`${config.slug}\`)`)
line(`- Tasks, milestones and the initiative: kind 1621 issues with \`["a", "${coord}"]\`; the issue's \`subject\` is its title, \`t\` its labels, \`parent\` and \`blocked-by\` its relations (event ids).`)
line(`- Edits: kind 1111 comments on the issue with \`["revision", "openagents.issue-revision.v1"]\`; the latest one by the project key is current.`)
line(`- Work on an issue: kind 1111 comments without that mark. \`["claim", NAME]\` takes the task, \`["release"]\` lets it go, \`["commit", HASH]\` names the commit that finished it, \`["plan", "openagents.task-plan.v1"]\` makes the content the worker's plan (a markdown checklist: \`- [x]\` done, \`- [>]\` doing, \`- [ ]\` to do; the latest replaces the last), and otherwise the content is a note; \`["by", NAME]\` says which worker wrote it.`)
line(`- Status: kinds 1630 open, 1631 resolved, 1632 closed, 1633 draft; the latest by the project key wins.`)
line(`- Order: kind 30623, \`d\` = \`${config.slug}\`: \`["item", <issue id>, <rank>]\`, ranks compared as strings. Ready tasks come first by rank, then waiting ones.`)
line()
const task = (i: Issue) => {
  const ms = i.parent ? by.get(i.parent) : undefined
  line(`### ${tag(i)} ${i.title}`)
  line()
  const facts = [`status ${i.status}`, isReady(i) ? "ready" : "", i.claimed_by ? `in progress: ${i.claimed_by}` : "", i.size ? `size ${i.size}` : "", 
    i.labels.length ? `labels ${i.labels.join(", ")}` : "", ms ? `milestone ${tag(ms)} ${ms.title}` : ""].filter(Boolean)
  line(facts.join(" · "))
  const blockers = i.blocked_by.map((b) => by.get(b)).filter((b): b is Issue => !!b)
  if (blockers.length) line(`\nWaits on: ${blockers.map((b) => `${tag(b)} ${b.title} (${b.status})`).join("; ")}`)
  line()
  line(`Event: \`${i.id}\``)
  line()
  if (i.plan) {
    line(`Plan (${planDone(i.plan.steps)} of ${i.plan.steps.length} done, updated ${chicago(new Date(i.plan.at * 1000))}${i.plan.by ? ` by ${i.plan.by}` : ""}):`)
    line()
    for (const st of i.plan.steps) line(`- [${st.mark === ">" ? " " : st.mark}] ${st.mark === ">" ? "**now:** " : ""}${st.text}`)
    line()
  }
  line(i.body.trim())
  line()
  if (i.notes.length) {
    line(`Notes:`)
    line()
    for (const n of i.notes) line(`- ${chicago(new Date(n.at * 1000))}${n.by ? ` ${n.by}` : ""}: ${n.text}`)
    line()
  }
}
for (const ini of f.issues.filter((i) => i.kind === "initiative").sort(rankOrder)) {
  line(`## Initiative: ${ini.title}`)
  line()
  line(ini.body.trim())
  line()
  line(`Milestones, in order:`)
  line()
  for (const m of milestones.filter((m) => m.parent === ini.id)) {
    const mine = tasks.filter((t) => t.parent === m.id)
    line(`- ${tag(m)} **${m.title}**: ${mine.filter(done).length} of ${mine.length} done, ${mine.filter(isReady).length} ready. ${m.body}`)
  }
  line()
}
if (activity.length) {
  line(`## Activity`)
  line()
  for (const l of activity.slice(0, 30)) {
    const i = by.get(l.id)
    line(`- ${chicago(new Date(l.at * 1000))} ${i ? `${tag(i)} ` : ""}${l.text}${i ? ` (${i.title})` : ""}`)
  }
  line()
}
line(`## In progress (${working.length})`)
line()
working.forEach(task)
line(`## Ready (${ready.length})`)
line()
ready.forEach(task)
line(`## Waiting (${waiting.length})`)
line()
waiting.forEach(task)
if (finished.length) {
  line(`## Done (${finished.length})`)
  line()
  for (const i of finished) line(`- ${tag(i)} ${i.title}${i.commit ? ` · [${i.commit.slice(0, 10)}](${config.repo.web}/commit/${i.commit})` : ""}`)
  line()
}
if (f.issues.length === 0) line(`No tasks are published yet.\n`)
writeFileSync(join(dir, "index.md"), md.join("\n"))

const esc = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;")
// The hidden text: the name and where the tasks are. Nothing else; the page
// itself is drawn, and agents read index.md.
const html = [
  `<h1>${esc(config.name)}</h1>`,
  `<p><a href="/project/${slug}/index.md">Tasks</a></p>`,
].join("\n")
const page = join(dir, "index.html")
const text = readFileSync(page, "utf8")
const a = text.indexOf("<!-- snapshot -->"), b = text.indexOf("<!-- /snapshot -->")
if (a < 0 || b < a) { console.error("snapshot: index.html has no <!-- snapshot --> markers"); process.exit(1) }
writeFileSync(page, text.slice(0, a + "<!-- snapshot -->".length) + "\n" + html + "\n" + text.slice(b))
console.log(`snapshot ${chicago(at)}: ${f.issues.length} items (${working.length} in progress, ${ready.length} ready, ${waiting.length} waiting, ${finished.length} done, ${activity.length} activity), order ${f.order ? "kind " + KIND_ORDER : "none"}`)

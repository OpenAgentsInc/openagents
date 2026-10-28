// The project publisher (bootstrap): puts a project's work on Nostr and keeps
// it there. It runs on this machine with the project's key; Coax's own
// signing replaces it once the Coax Nostr work reaches signing (its
// milestones "Schnorr signatures" and "Events and relays").
//
//   bun publish.ts publish [--project coax] [--dry-run] [--relay URL]
//   bun publish.ts move SLUG --before SLUG | --after SLUG
//   bun publish.ts status SLUG open|resolved|closed|draft
//   bun publish.ts respread [--seed]   (even ranks again; --seed: work-file order)
//   bun publish.ts list               (the queue: ready first, then waiting, then done)
//   bun publish.ts show SLUG          (body, acceptance checks, claim and notes)
//   bun publish.ts claim SLUG --by NAME
//   bun publish.ts note SLUG "text" [--by NAME]
//   bun publish.ts release SLUG
//   bun publish.ts done SLUG --commit HASH
//   bun publish.ts plan SLUG                       (print the task's plan)
//   bun publish.ts plan SLUG set "step" "step"...  (a new plan, every step to do)
//   bun publish.ts plan SLUG add "step"...         (steps at the end)
//   bun publish.ts plan SLUG start N | check N... | uncheck N... | drop N...
//   bun publish.ts plan SLUG file PATH|-           (the whole checklist, markdown)
//
// A plan is the worker's to-do list for its task, kept current as it works:
// "start" marks the step it's on now (one at a time), "check" the ones done.
// Steps count from 1. Every change publishes the whole list again.
//
// SLUG is an item's slug or its alias (#a7e7). Claims, notes and the commit
// that finished a task are NIP-22 comments on its issue (model.ts), signed by
// the project key with the worker's NAME in a tag; coax/web/project/QUEUE.md
// is how a worker uses them.
//
// Work comes from apps/openagents-web/project/<slug>/work/*.json (model.ts,
// WorkItem) and project.json. Every run folds the relay's events first, so
// re-running publishes only what changed: a new item is a new issue, an edit
// is a revision (a NIP-22 comment), a status is a status event, and the
// order is one replaceable event. Nothing is ever published twice.
// state/<slug>.json caches the slug -> event id map from the last run.
import { getPublicKey, type Event } from "nostr-effect/pure"
import { hexToBytes } from "@noble/hashes/utils"
import { readdirSync, mkdirSync, writeFileSync } from "node:fs"
import { join } from "node:path"
import { Relay, Signer } from "./relay"
import { aliases, checkWork, fieldTags, filters, fold, followups, ISSUE_REVISION, KIND_COMMENT, KIND_ISSUE, KIND_ORDER,
  KIND_PROJECT, KIND_REPO, naddr, projectCoord, projectTags, repoCoord, repoTags, sameFields, STATUS_KINDS, topo,
  type Fields, type Folded, type Issue, type ProjectConfig, type Status, type Step, type WorkItem, WORK_SCHEMA, SEED_SCHEMA, seedItems,
  parsePlan, planDone, planText, TASK_PLAN } from "./model"
import { placed, rebalance, spread, spreadBetween } from "./rank"

const ROOT = join(import.meta.dir, "../../..")
const args = process.argv.slice(2)
const flag = (name: string, fallback = "") => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : fallback }
const has = (name: string) => args.includes(`--${name}`)
const slug = flag("project", "coax")
const dry = has("dry-run")
const dir = flag("dir", join(ROOT, "apps/openagents-web/project", slug))
const config: ProjectConfig = await Bun.file(join(dir, "project.json")).json()
const relayUrl = flag("relay", config.relay)
const keyFile = flag("key", `${process.env.HOME}/work/.secrets/openagents-project-${slug}.key`)
const key = hexToBytes((await Bun.file(keyFile).text()).trim())
const pk = getPublicKey(key)
const signer = new Signer(key)

export async function readWork(d: string): Promise<WorkItem[]> {
  const items: WorkItem[] = []
  let files: string[] = []
  try { files = readdirSync(join(d, "work")).filter((f) => f.endsWith(".json")).sort() } catch { return [] }
  for (const f of files) {
    const doc = await Bun.file(join(d, "work", f)).json()
    if (doc.v === SEED_SCHEMA) items.push(...seedItems(doc))
    else if (doc.v === WORK_SCHEMA && Array.isArray(doc.items)) items.push(...doc.items)
    else throw new Error(`${f}: not ${SEED_SCHEMA} or ${WORK_SCHEMA}`)
  }
  return checkWork(items)
}

async function load(relay: Relay): Promise<Folded> {
  const first = await relay.all(filters(pk, config))
  const ids = first.filter((e) => e.kind === KIND_ISSUE).map((e) => e.id)
  const more: Event[] = []
  for (let i = 0; i < ids.length; i += 50) more.push(...await relay.all(followups(pk, ids.slice(i, i + 50))))
  const f = fold(pk, config, [...first, ...more])
  signer.after(Math.max(0, ...[...first, ...more].filter((e) => e.pubkey === pk).map((e) => e.created_at)))
  // what the last run saw must still be there: a relay that answers short
  // must never make the publisher think an issue was never published
  const cache = join(import.meta.dir, "state", `${slug}.json`)
  let known: Record<string, string> = {}
  try { known = await Bun.file(cache).json() } catch {}
  const now = new Set(f.issues.map((i) => i.id))
  const lost = Object.entries(known).filter(([, id]) => !now.has(id))
  if (lost.length && !has("forget")) throw new Error(`the relay didn't return ${lost.length} known issue(s) (${lost.slice(0, 3).map(([s]) => s).join(", ")}...); refusing to continue (--forget to drop them from the cache)`)
  mkdirSync(join(import.meta.dir, "state"), { recursive: true })
  writeFileSync(cache,
    JSON.stringify(Object.fromEntries(f.issues.map((i) => [i.slug, i.id])), null, 2) + "\n")
  return f
}

const published: string[] = []
async function send(relay: Relay, e: Event, what: string) {
  published.push(`${e.kind} ${what}`)
  if (dry) { console.log(`would publish kind ${e.kind}: ${what}`); return }
  await relay.publish(e)
  console.log(`published kind ${e.kind}: ${what} (${e.id.slice(0, 8)})`)
}

const sameTags = (e: Event | null, tags: string[][]) => !!e && JSON.stringify(e.tags) === JSON.stringify(tags)

function wanted(it: WorkItem, ids: Map<string, string>): Fields {
  return {
    kind: it.kind, title: it.title, body: it.body.trim() + "\n", labels: it.labels ?? [], size: it.size ?? "",
    sats: it.sats ?? 0, parent: it.parent ? ids.get(it.parent)! : "", blocked_by: (it.blocked_by ?? []).map((b) => ids.get(b)!),
  }
}

function issueEvent(f: Fields, itemSlug: string) {
  return signer.sign({ kind: KIND_ISSUE, content: f.body, tags: [
    ["a", repoCoord(pk, config), config.relay], ["a", projectCoord(pk, config), config.relay], ["p", pk],
    ["slug", itemSlug], ...fieldTags(f), ["alt", `A ${f.kind} in the ${config.name} project: ${f.title}`]] })
}

function revisionEvent(issue: Issue, f: Fields) {
  return signer.sign({ kind: KIND_COMMENT, content: f.body, tags: [
    ["E", issue.id, config.relay, pk], ["K", String(KIND_ISSUE)], ["P", pk],
    ["e", issue.id, config.relay, pk], ["k", String(KIND_ISSUE)], ["p", pk],
    ["revision", ISSUE_REVISION], ...fieldTags(f), ["alt", `Revision of an issue: ${f.title}`]] })
}

function statusEvent(issue: Issue, status: Status) {
  return signer.sign({ kind: STATUS_KINDS[status], content: "", tags: [
    ["e", issue.id, "", "root"], ["p", pk], ["a", repoCoord(pk, config), config.relay], ["r", config.repo.euc]] })
}

function orderTags(entries: { id: string; rank: string }[]) {
  return [["d", config.slug], ["a", projectCoord(pk, config), config.relay],
    ...entries.map((e) => ["item", e.id, e.rank]), ["alt", `The ${config.name} project's task order`]]
}

/** The order the queue keeps: ranked items by rank, then unranked items in work-file order. */
function ordered(f: Folded, work: WorkItem[]): { id: string; rank: string }[] {
  const ranked = f.issues.filter((i) => i.rank).sort((a, b) => (a.rank < b.rank ? -1 : 1)).map((i) => ({ id: i.id, rank: i.rank }))
  const bySlug = new Map(f.issues.map((i) => [i.slug, i]))
  const last = ranked.length ? ranked[ranked.length - 1].rank : ""
  // new items go after the ranked ones, in work-file order, spread evenly
  const fresh = work.map((it) => bySlug.get(it.slug)).filter((i): i is Issue => !!i && !i.rank)
  const keys = spreadBetween(last, null, fresh.length)
  fresh.forEach((i, k) => ranked.push({ id: i.id, rank: keys[k] }))
  const r = rebalance(ranked.map((e) => e.rank))
  if (r.changed) ranked.forEach((e, i) => (e.rank = r.ranks[i]))
  return ranked
}

async function publishAll(relay: Relay) {
  const work = await readWork(dir)
  let f = await load(relay)
  const repoT = repoTags(config), projT = projectTags(pk, config)
  if (!sameTags(f.repo, repoT)) await send(relay, signer.sign({ kind: KIND_REPO, content: "", tags: repoT }), `repository ${config.repo.d}`)
  if (!sameTags(f.project, projT)) await send(relay, signer.sign({ kind: KIND_PROJECT, content: "", tags: projT }), `project ${config.slug}`)
  const ids = new Map(f.issues.map((i) => [i.slug, i.id]))
  const bySlug = new Map(f.issues.map((i) => [i.slug, i]))
  for (const it of topo(work)) {
    const want = wanted(it, ids)
    const have = bySlug.get(it.slug)
    if (!have) {
      const e = issueEvent(want, it.slug)
      await send(relay, e, `issue ${it.slug}`)
      ids.set(it.slug, e.id)
      if (it.status && it.status !== "open") await send(relay, statusEvent({ id: e.id } as Issue, it.status), `${it.slug} ${it.status}`)
    } else {
      if (!sameFields(have, want)) await send(relay, revisionEvent(have, want), `revision of ${it.slug}`)
      if (it.status && it.status !== have.status) await send(relay, statusEvent(have, it.status), `${it.slug} ${it.status}`)
    }
  }
  if (dry) { console.log(`dry run: ${published.length} events`); return }
  f = await load(relay)
  const entries = ordered(f, work)
  const tags = orderTags(entries)
  if (!sameTags(f.order, tags)) await send(relay, signer.sign({ kind: KIND_ORDER, content: "", tags }), `order of ${entries.length}`)
  console.log(`project ${naddr(pk, config)}`)
}

async function move(relay: Relay, what: string, where: "before" | "after", other: string) {
  const work = await readWork(dir)
  const f = await load(relay)
  const entries = ordered(f, work)
  const id = (s: string) => f.issues.find((i) => i.slug === s)?.id ?? (() => { throw new Error(`no item ${s}`) })()
  const moving = id(what), anchor = id(other)
  if (moving === anchor) throw new Error("an item can't move relative to itself")
  const rest = entries.filter((e) => e.id !== moving)
  const at = rest.findIndex((e) => e.id === anchor)
  const rank = placed(rest.map((e) => e.rank), at, where)
  const next = [...rest, { id: moving, rank }].sort((a, b) => (a.rank < b.rank ? -1 : 1))
  const r = rebalance(next.map((e) => e.rank))
  if (r.changed) next.forEach((e, i) => (e.rank = r.ranks[i]))
  await send(relay, signer.sign({ kind: KIND_ORDER, content: "", tags: orderTags(next) }), `${what} ${where} ${other}`)
  // a task moved above one it waits on is allowed, and said
  const issue = f.issues.find((i) => i.id === moving)!
  const rankOf = new Map(next.map((e) => [e.id, e.rank]))
  for (const b of issue.blocked_by) {
    const blocker = f.issues.find((i) => i.id === b)
    if (blocker && blocker.status === "open" && rankOf.get(b)! > rankOf.get(moving)!)
      console.log(`warning: ${what} is now above ${blocker.slug}, which it waits on; it stays blocked until that resolves`)
  }
}

/** Spreads the ranks evenly again, keeping the order; --seed resets it to work-file order. */
async function respread(relay: Relay, seed: boolean) {
  const work = await readWork(dir)
  const f = await load(relay)
  let entries = ordered(f, work)
  if (seed) {
    const pos = new Map(work.map((it, i) => [it.slug, i]))
    const slugOf = new Map(f.issues.map((i) => [i.id, i.slug]))
    entries = [...entries].sort((a, b) => (pos.get(slugOf.get(a.id)!) ?? 1e9) - (pos.get(slugOf.get(b.id)!) ?? 1e9))
  }
  const keys = spread(entries.length)
  entries.forEach((e, i) => (e.rank = keys[i]))
  await send(relay, signer.sign({ kind: KIND_ORDER, content: "", tags: orderTags(entries) }), `respread ${entries.length}${seed ? " in work-file order" : ""}`)
}

const finished = (i: Issue) => i.status === "resolved" || i.status === "closed"

/** Where an item stands in the queue: 0 ready, 1 waiting (a draft or an open blocker), 2 done, 3 not a task. */
function standing(f: Folded, i: Issue): number {
  if (i.kind !== "task") return 3
  if (finished(i)) return 2
  if (i.status === "draft") return 1
  return i.blocked_by.every((b) => { const k = f.issues.find((o) => o.id === b); return !k || finished(k) }) ? 0 : 1
}

async function list(relay: Relay) {
  const f = await load(relay)
  const a = aliases(f.issues.map((i) => i.id))
  const rows = [...f.issues].sort((x, y) => standing(f, x) - standing(f, y) || ((x.rank || "~") < (y.rank || "~") ? -1 : 1))
  const word = ["ready", "waiting", "done", ""]
  for (const i of rows) {
    const st = standing(f, i)
    const state = i.claimed_by ? `on:${i.claimed_by}` : st < 3 ? word[st] : i.status
    const tail = st === 2 && i.commit ? `  (${i.commit.slice(0, 10)})` : ""
    console.log(`#${a.get(i.id)}  ${i.rank.padEnd(6)} ${state.padEnd(16)} ${i.kind.padEnd(10)} ${i.slug}  ${i.title}${tail}`)
  }
  console.log(`${rows.length} items · project ${naddr(pk, config)}`)
}

/** An item by slug or alias (with or without its #). */
function find(f: Folded, what: string): Issue {
  const a = aliases(f.issues.map((i) => i.id))
  const w = what.replace(/^#/, "")
  const issue = f.issues.find((i) => i.slug === w) ?? f.issues.find((i) => a.get(i.id) === w)
  if (!issue) throw new Error(`no item ${what}`)
  return issue
}

/** A worker's comment on an issue: a claim, a release, a note or a commit. */
function commentEvent(issue: Issue, content: string, tags: string[][], alt: string) {
  return signer.sign({ kind: KIND_COMMENT, content, tags: [
    ["E", issue.id, config.relay, pk], ["K", String(KIND_ISSUE)], ["P", pk],
    ["e", issue.id, config.relay, pk], ["k", String(KIND_ISSUE)], ["p", pk],
    ...tags, ["alt", alt]] })
}

async function show(relay: Relay, what: string) {
  const f = await load(relay)
  const i = find(f, what)
  const a = aliases(f.issues.map((x) => x.id))
  const name = (id: string) => { const k = f.issues.find((x) => x.id === id); return k ? `#${a.get(id)} ${k.slug} (${k.status})` : id.slice(0, 8) }
  const word = ["ready", "waiting", "done", i.status][standing(f, i)]
  console.log(`#${a.get(i.id)} ${i.slug} · ${i.kind} · ${word}${i.size ? ` · size ${i.size}` : ""}`)
  console.log(i.title)
  if (i.parent) console.log(`in ${name(i.parent)}`)
  if (i.blocked_by.length) console.log(`waits on ${i.blocked_by.map(name).join(", ")}`)
  console.log(i.claimed_by ? `claimed by ${i.claimed_by} at ${new Date(i.claimed_at * 1000).toISOString()}` : "not claimed")
  if (i.commit) console.log(`done in ${i.commit}`)
  console.log(`\n${i.body.trim()}\n`)
  if (i.plan) { console.log("## Plan\n"); printPlan(i); console.log() }
  if (i.notes.length) {
    console.log("## Notes\n")
    for (const n of i.notes) console.log(`- ${new Date(n.at * 1000).toISOString().slice(0, 16)} ${n.by ? n.by + ": " : ""}${n.text}`)
  }
}

async function claim(relay: Relay, what: string, by: string) {
  if (!by) throw new Error("usage: claim SLUG --by NAME")
  const f = await load(relay)
  const i = find(f, what)
  if (i.kind !== "task") throw new Error(`${i.slug} is a ${i.kind}; only tasks are claimed`)
  if (finished(i)) throw new Error(`${i.slug} is already ${i.status}`)
  if (i.claimed_by && i.claimed_by !== by) throw new Error(`${i.slug} is claimed by ${i.claimed_by}; ask them, or the taskmaster releases it`)
  if (i.claimed_by === by) { console.log(`${i.slug} is already yours`); return }
  const mine = f.issues.find((o) => o.claimed_by === by && o.id !== i.id)
  if (mine) console.log(`warning: ${by} also holds ${mine.slug}; one task at a time`)
  if (standing(f, i) === 1) console.log(`warning: ${i.slug} still waits on an open blocker`)
  await send(relay, commentEvent(i, `${by} is working on this.`, [["claim", by]], `${by} claimed a task: ${i.title}`), `${i.slug} claimed by ${by}`)
}

async function release(relay: Relay, what: string) {
  const f = await load(relay)
  const i = find(f, what)
  if (!i.claimed_by) { console.log(`${i.slug} isn't claimed`); return }
  await send(relay, commentEvent(i, `${i.claimed_by} let go of this.`, [["release"], ["by", i.claimed_by]], `A task was released: ${i.title}`), `${i.slug} released`)
}

async function note(relay: Relay, what: string, text: string, by: string) {
  if (!text.trim()) throw new Error('usage: note SLUG "text" [--by NAME]')
  const f = await load(relay)
  const i = find(f, what)
  const who = by || i.claimed_by
  await send(relay, commentEvent(i, text.trim(), who ? [["by", who]] : [], `A note on a task: ${i.title}`), `note on ${i.slug}`)
}

async function done(relay: Relay, what: string, commit: string) {
  if (!/^[0-9a-f]{7,40}$/.test(commit)) throw new Error("usage: done SLUG --commit HASH (a git commit hash)")
  const f = await load(relay)
  const i = find(f, what)
  if (finished(i)) throw new Error(`${i.slug} is already ${i.status}`)
  const who = i.claimed_by
  const url = `${config.repo.web.replace(/\/$/, "")}/commit/${commit}`
  await send(relay, commentEvent(i, `Done in ${commit.slice(0, 10)}: ${url}`, [["commit", commit], ["r", url], ...(who ? [["by", who]] : [])],
    `A task was finished: ${i.title}`), `${i.slug} done in ${commit.slice(0, 10)}`)
  await send(relay, statusEvent(i, "resolved"), `${i.slug} resolved`)
}

const MARK_WORD = { x: "done", ">": "doing", " ": "to do" }
function printPlan(i: Issue) {
  if (!i.plan) { console.log(`${i.slug} has no plan yet`); return }
  console.log(`${i.slug}: ${planDone(i.plan.steps)} of ${i.plan.steps.length} done${i.plan.by ? `, by ${i.plan.by}` : ""}, ${new Date(i.plan.at * 1000).toISOString().slice(0, 16)}`)
  i.plan.steps.forEach((st, k) => console.log(`${String(k + 1).padStart(3)}. [${st.mark}] ${st.text}`))
}

async function plan(relay: Relay, what: string, rest: string[], by: string) {
  const f = await load(relay)
  const i = find(f, what)
  // the plan's words, without the --flag value pairs the command line also carries
  rest = rest.filter((w, k, all) => !w.startsWith("--") && !all[k - 1]?.startsWith("--"))
  const op = rest[0] ?? ""
  if (!op) { printPlan(i); return }
  if (i.kind !== "task") throw new Error(`${i.slug} is a ${i.kind}; plans are for tasks`)
  if (finished(i)) console.log(`warning: ${i.slug} is already ${i.status}`)
  let steps: Step[] = (i.plan?.steps ?? []).map((st) => ({ ...st }))
  const words = rest.slice(1)
  const nums = () => {
    const ns = words.map(Number)
    if (!ns.length || ns.some((n) => !Number.isInteger(n) || n < 1 || n > steps.length)) throw new Error(`steps are 1 to ${steps.length}`)
    return ns.map((n) => n - 1)
  }
  if (op === "set") steps = words.map((t) => ({ mark: " ", text: t.trim() }))
  else if (op === "add") steps.push(...words.map((t) => ({ mark: " " as const, text: t.trim() })))
  else if (op === "start") { const [k] = nums(); steps.forEach((st) => { if (st.mark === ">") st.mark = " " }); steps[k].mark = ">" }
  else if (op === "check") for (const k of nums()) steps[k].mark = "x"
  else if (op === "uncheck") for (const k of nums()) steps[k].mark = " "
  else if (op === "drop") { const gone = new Set(nums()); steps = steps.filter((_, k) => !gone.has(k)) }
  else if (op === "file") {
    const text = words[0] === "-" || !words[0] ? await Bun.stdin.text() : await Bun.file(words[0]).text()
    steps = parsePlan(text)
  } else throw new Error("usage: plan SLUG [set|add|start|check|uncheck|drop|file] ...")
  steps = steps.filter((st) => st.text)
  if (!steps.length) throw new Error("a plan needs at least one step")
  const who = by || i.claimed_by
  await send(relay, commentEvent(i, planText(steps), [["plan", TASK_PLAN], ...(who ? [["by", who]] : [])],
    `A worker's plan for a task: ${i.title}`), `plan for ${i.slug} (${planDone(steps)}/${steps.length})`)
  steps.forEach((st, k) => console.log(`${String(k + 1).padStart(3)}. [${st.mark}] ${st.text}  (${MARK_WORD[st.mark]})`))
}

const verb = args[0]
const relay = await Relay.open(relayUrl, key)
try {
  if (verb === "publish") await publishAll(relay)
  else if (verb === "move") {
    const where = has("before") ? "before" : has("after") ? "after" : null
    if (!args[1] || !where) throw new Error("usage: move SLUG --before SLUG | --after SLUG")
    await move(relay, args[1], where, flag(where))
  } else if (verb === "status") {
    const s = args[2] as Status
    if (!args[1] || !(s in STATUS_KINDS)) throw new Error("usage: status SLUG open|resolved|closed|draft")
    const f = await load(relay)
    const issue = f.issues.find((i) => i.slug === args[1])
    if (!issue) throw new Error(`no item ${args[1]}`)
    await send(relay, statusEvent(issue, s), `${args[1]} ${s}`)
  } else if (verb === "respread") await respread(relay, has("seed"))
  else if (verb === "list") await list(relay)
  else if (verb === "show" && args[1]) await show(relay, args[1])
  else if (verb === "claim" && args[1]) await claim(relay, args[1], flag("by"))
  else if (verb === "release" && args[1]) await release(relay, args[1])
  else if (verb === "note" && args[1]) await note(relay, args[1], args[2] ?? "", flag("by"))
  else if (verb === "done" && args[1]) await done(relay, args[1], flag("commit"))
  else if (verb === "plan" && args[1]) await plan(relay, args[1], args.slice(2), flag("by"))
  else { console.error("usage: bun publish.ts publish|move|status|respread|list|show|claim|note|release|done|plan (see the top of this file)"); process.exitCode = 2 }
} finally { relay.close() }

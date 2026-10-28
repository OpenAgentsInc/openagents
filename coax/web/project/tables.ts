// The tables the project page draws, from a folded project (model.ts): the
// queue as the page orders it, the activity, and the plans of the tasks in
// progress. snapshot.ts bakes them into work.coax; fixtures.ts writes them
// for the page's own fold to be checked against (project-live.coax), which
// must produce the same bytes from the same events.
import { escapeDetail, escapeList, splitAcceptance } from "./details"
import { aliases, planDone, type Folded, type Issue } from "./model"

// A time in Chicago, "MM-DD HH:MM CDT".
export const chicago = (d: Date) => {
  const p = Object.fromEntries(new Intl.DateTimeFormat("en-US", { timeZone: "America/Chicago", month: "2-digit", day: "2-digit",
    hour: "2-digit", minute: "2-digit", hour12: false, timeZoneName: "short" }).formatToParts(d).map((x) => [x.type, x.value]))
  return `${p.month}-${p.day} ${p.hour === "24" ? "00" : p.hour}:${p.minute} ${p.timeZoneName}`
}

// The stroke font draws ASCII and a few marks; everything else is spelled out.
export const plain = (s: string) => s.replace(/[×]/g, "x").replace(/[–—]/g, "-").replace(/[≈]/g, "~").replace(/[’‘]/g, "'")
  .replace(/[“”]/g, '"').replace(/[→]/g, "->").replace(/[·]/g, "-").replace(/[…]/g, "...").replace(/[−]/g, "-").replace(/[≤]/g, "<=").replace(/[≥]/g, ">=").replace(/[≠]/g, "!=").replace(/[←]/g, "<-").replace(/[≡]/g, "=").replace(/[^\x20-\x7e]/g, "")
export const cell = (s: string) => plain(s).replace(/[|\n\r]/g, " ").replace(/\s+/g, " ").trim()

// waiting: this project's issues that wait on another project's (links.ts), by id
export function tables(f: Folded, waiting_on_others: Set<string> = new Set()) {
  const by = new Map(f.issues.map((i) => [i.id, i]))
  const done = (i: Issue | undefined) => !!i && (i.status === "resolved" || i.status === "closed")
  const tasks = f.issues.filter((i) => i.kind === "task")
  const isReady = (i: Issue) => i.status === "open" && i.blocked_by.every((b) => done(by.get(b))) && !waiting_on_others.has(i.id)
  const rankOrder = (a: Issue, b: Issue) => ((a.rank || "~") < (b.rank || "~") ? -1 : (a.rank || "~") > (b.rank || "~") ? 1 : 0)
  const working = tasks.filter((i) => !done(i) && i.claimed_by).sort(rankOrder)
  const ready = tasks.filter((i) => isReady(i) && !i.claimed_by).sort(rankOrder)
  const waiting = tasks.filter((i) => !done(i) && !isReady(i) && !i.claimed_by).sort(rankOrder)
  const finished = tasks.filter(done).sort((a, b) => b.status_at - a.status_at)
  const alias = aliases(f.issues.map((i) => i.id))
  const tag = (i: Issue | undefined) => (i ? `#${alias.get(i.id)}` : "")
  const milestones = f.issues.filter((i) => i.kind === "milestone").sort(rankOrder)


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

  // The plans of the tasks that have one (in progress first): each step, and
  // for the tasks in progress a line about the plan. Marks: x done, > doing, o to do.
  const PLAN_STEPS = 24
  const planned = [...working, ...tasks.filter((i) => i.plan && !working.includes(i))]
  const plans = planned.flatMap((i) => (i.plan?.steps ?? []).slice(0, PLAN_STEPS)
    .map((st) => `${alias.get(i.id)}|${st.mark === " " ? "o" : st.mark}|${cell(st.text)}\n`)).join("")
  const lastNote = (i: Issue) => i.notes.filter((n) => !/^Done in /.test(n.text)).at(-1)
  const planInfo = working.map((i) => {
    const st = i.plan?.steps ?? [], n = lastNote(i)
    return [alias.get(i.id), planDone(st), st.length, i.plan ? chicago(new Date(i.plan.at * 1000)).slice(6, 11) : "",
      chicago(new Date(i.claimed_at * 1000)).slice(6, 11), n ? cell(n.text) : ""].join("|") + "\n"
  }).join("")

  // What a task's detail panel shows beyond the items table (details.ts says
  // how a field is escaped): the body without its acceptance section, those
  // checks, and every note with its Chicago time and writer.
  const prose = (s: string) => s.split("\n").map((l) => plain(l).trimEnd()).join("\n").replace(/\*\*/g, "").replace(/`/g, "")
  const details = [...f.issues].sort(rankOrder).map((i) => {
    const { body, acceptance } = splitAcceptance(i.body)
    return [alias.get(i.id), i.id, i.claimed_by && i.claimed_at ? chicago(new Date(i.claimed_at * 1000)) : "",
      escapeDetail(prose(body)), escapeList(acceptance.map((a) => [prose(a)])),
      escapeList(i.notes.map((n) => [chicago(new Date(n.at * 1000)), plain(n.by), prose(n.text)]))].join("|") + "\n"
  }).join("")
  return { by, done, tasks, isReady, rankOrder, working, ready, waiting, finished, alias, tag, milestones, activity, chicago, cell,
    rows, log, plans, planInfo, details }
}

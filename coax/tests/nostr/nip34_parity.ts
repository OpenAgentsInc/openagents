// Acceptance for nostr-nip34-nip22: every event in the captured coax relay
// transcript reads, through coax/lib/nostr/nip34.coax and nip22.coax, to the
// fields coax/web/project/model.ts takes from it. Test tooling only.
//
//   bun coax/tests/nostr/nip34_parity.ts [COAX]   (COAX defaults to coax/bin/coax)
//
// The expected side reads each event the way model.ts does: tags.find for a
// single tag, every tag of a name for a list, and model.ts's own fieldsFrom,
// STATUS_NAMES, ISSUE_REVISION and TASK_PLAN.
import { fieldsFrom, STATUS_NAMES, ISSUE_REVISION, TASK_PLAN, KIND_REPO, KIND_PROJECT, KIND_ORDER, KIND_ISSUE, KIND_COMMENT } from "../../web/project/model.ts"
import { deepStrictEqual } from "node:assert"

const root = new URL("../../../", import.meta.url).pathname
const coax = process.argv[2] ?? `${root}coax/bin/coax`

// The transcript as the Coax dump sees it: the COAX_RELAY literal, which must match the JSONL capture.
const source = await Bun.file(`${root}coax/lib/nostr/fixtures/coax_relay.coax`).text()
const literal = /^COAX_RELAY :: (".*");$/m.exec(source)
if (!literal) throw new Error("no COAX_RELAY literal")
const transcript: string = JSON.parse(literal[1])
const jsonl = await Bun.file(`${root}coax/lib/nostr/fixtures/coax_relay.jsonl`).text()
if (transcript.trimEnd() !== jsonl.trimEnd()) throw new Error("coax_relay.coax and coax_relay.jsonl differ")

type Tags = string[][]
const one = (tags: Tags, name: string) => tags.find((t) => t[0] === name)?.[1] ?? ""
const all = (tags: Tags, name: string) => tags.filter((t) => t[0] === name).map((t) => t[1])

function expected(i: number, e: { kind: number; tags: Tags; content: string }): Record<string, unknown> {
  const { kind, tags } = e
  const out: Record<string, unknown> = { i, kind, status: 0 }
  if (kind === KIND_REPO) {
    for (const n of ["d", "name", "description", "web", "clone", "relays"]) out[n] = one(tags, n)
    out.euc = tags.find((t) => t[0] === "r" && t[2] === "euc")?.[1] ?? ""
  } else if (kind === KIND_PROJECT) {
    for (const n of ["d", "name", "description", "a"]) out[n] = one(tags, n)
    const path = tags.find((t) => t[0] === "path")
    out.path = [path?.[1] ?? "", path?.[2] ?? ""]
    for (const n of ["contributions", "license", "visibility", "buzz-visibility"]) out[n] = one(tags, n)
    out.maintainers = tags.filter((t) => t[0] === "p" && t[3] === "maintainer").length
  } else if (kind === KIND_ORDER) {
    out.d = one(tags, "d"); out.a = one(tags, "a")
    out.items = tags.filter((t) => t[0] === "item").map((t) => [t[1], t[2]])
  } else if (kind === KIND_ISSUE) {
    out.a = all(tags, "a")
    out.repo = (out.a as string[]).find((c) => c.startsWith(`${KIND_REPO}:`)) ?? ""
    out.project = (out.a as string[]).find((c) => c.startsWith(`${KIND_PROJECT}:`)) ?? ""
    out.slug = one(tags, "slug")
    Object.assign(out, fields(e))
  } else if (kind in STATUS_NAMES) {
    out.state = STATUS_NAMES[kind]
    out.root = (tags.find((t) => t[0] === "e" && t[3] === "root") ?? tags.find((t) => t[0] === "e"))?.[1] ?? ""
    out.e = all(tags, "e")
  } else if (kind === KIND_COMMENT) {
    out.E = all(tags, "E")
    out.root_at = "E".charCodeAt(0); out.root_kind = Number(one(tags, "K"))
    out.parent_at = "e".charCodeAt(0); out.parent_kind = Number(one(tags, "k"))
    const revised = tags.some((t) => t[0] === "revision" && t[1] === ISSUE_REVISION)
    out.revised = revised
    if (revised) Object.assign(out, fields(e))
    else {
      // model.ts: by ?? claim (the fold's fallback to the issue's worker is its own state)
      const tag = (name: string) => tags.find((t) => t[0] === name)
      out.by = tag("by")?.[1] ?? tag("claim")?.[1] ?? ""
      out.claim = !!tag("claim"); out.claimed = tag("claim")?.[1] ?? ""
      out.release = !!tag("release"); out.commit = tag("commit")?.[1] ?? ""
      out.planned = tag("plan")?.[1] === TASK_PLAN
      out.content = e.content
    }
  }
  return out
}
function fields(e: { tags: Tags; content: string }) {
  const f = fieldsFrom(e.tags, e.content)
  return { fields_status: 0, fields: { kind: f.kind, title: f.title, body: f.body, labels: f.labels, size: f.size, sats: f.sats,
    parent: f.parent, blocked_by: f.blocked_by, needs: f.needs } }
}

const want: Record<string, unknown>[] = []
for (const line of transcript.split("\n")) {
  if (!line) continue
  const frame = JSON.parse(line)
  if (frame[0] === "EVENT") want.push(expected(want.length, frame[2]))
}

const run = Bun.spawnSync([coax, "run", `${root}coax/tests/nostr/nip34_dump.coax`], { stdout: "pipe", stderr: "pipe" })
if (run.exitCode !== 0) throw new Error(`coax run failed:\n${run.stderr.toString()}`)
const got = run.stdout.toString().split("\n").filter(Boolean).map((l) => JSON.parse(l))

let bad = 0
const kinds: Record<number, number> = {}
for (let i = 0; i < Math.max(want.length, got.length); i++) {
  try { deepStrictEqual(got[i], want[i]) } catch (err) {
    if (bad++ < 5) console.error(`event ${i}:`, (err as Error).message.slice(0, 2000))
  }
  if (want[i]) kinds[want[i].kind as number] = (kinds[want[i].kind as number] ?? 0) + 1
}
console.log(`${want.length} events (${Object.entries(kinds).map(([k, n]) => `${n} of kind ${k}`).join(", ")}): ${want.length - bad} match, ${bad} differ`)
if (bad || want.length !== got.length || want.length === 0) process.exit(1)

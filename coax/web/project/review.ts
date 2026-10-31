// Independent review of invariants, oracles and gates (r0-independent-review).
// Report-only by owner decision (2026-10-31): it never stops a release.
//
//   bun review.ts report --from BASE --to REF [--note] [--json]
//       every commit in BASE..REF that weakens a check together with the code
//       it checks, and whether an independent review covers it. --note posts
//       an unreviewed one on its queue task (the "#xxxx" in its subject).
//   bun review.ts sign COMMIT --key FILE [--why TEXT]
//       the review agent signs coax/reviews/<commit>.json with its own key.
//   bun review.ts keygen FILE
//       a reviewer key (mode 600); prints only the public key.
//
// What weakens a check (per commit, per file): a removed #assert, #test,
// #forall or #authority line in a .coax file, or a removed assert/expect line
// in a test file, that the same file does not add back; a new
// "mutant: equivalent" mark; a check line moved into a file without
// #authority (an unmarked helper); any removed line of an INVARIANTS.md or
// tests/run.sh. "Together with the code it checks": the same commit also
// changes code (anything that is not a test, doc or generated report).
//
// A review is independent when it is a valid signed Nostr event (kind 1111,
// tags commit=<full hash>, t=coax-review) whose key is not an author key
// (coax/reviews/authors.json: the project key every agent publishes with).
// It is checked outside the compiler, on this machine, by release.sh.
// Firm reason it is TypeScript, not Coax (coax/tools-in-coax.md): Schnorr
// verification is nostr-effect's until Coax's own signing lands, as for
// publish.ts.
import { finalizeEvent, generateSecretKey, getPublicKey, verifyEvent, type Event } from "nostr-effect/pure"
import { bytesToHex, hexToBytes } from "@noble/hashes/utils"
import { chmodSync, existsSync, readFileSync, writeFileSync, mkdirSync } from "node:fs"
import { join } from "node:path"

const ROOT = join(import.meta.dir, "../../..")
export const REVIEWS = join(ROOT, "coax/reviews")
export const REVIEW_KIND = 1111

export type FileDiff = { path: string; added: string[]; removed: string[] }
export type Finding = { file: string; why: string; line: string }

const CHECK_LINE = /(^|[{;]\s*)(#assert|#test|#forall|#authority)\b/
const TEST_LINE = /\b(assert\w*|expect)\s*\(/
export const isTest = (p: string) => /(^|\/)tests?\//.test(p) || /\.test\.ts$/.test(p) || /(^|\/)test_[^/]*\.py$/.test(p)
// Generated files (baked project data, release snapshots) are not checks.
const isGenerated = (p: string) => /apps\/openagents-web\/projects\//.test(p) || /baked\.coax$/.test(p) || p.startsWith("coax/bootstrap/")
const isDoc = (p: string) => /\.(md|txt)$/.test(p) || p.startsWith("docs/") || /\.json\.gz$/.test(p) || p.startsWith("coax/reviews/")

// A gate line is weakened when a word of it (a test, a status variable)
// appears in none of the file's added lines.
function gateDropped(removed: string, added: string[]): boolean {
  const text = added.join("\n")
  return removed.trim().split(/\s+/).some((w) => w.length > 2 && !text.includes(w))
}

export function weakenings(files: FileDiff[], hasAuthority: (path: string) => boolean): Finding[] {
  const out: Finding[] = []
  const addedAnywhere = new Map<string, string>()
  for (const f of files) for (const l of f.added) if (CHECK_LINE.test(l)) addedAnywhere.set(l.trim(), f.path)
  for (const f of files) {
    if (isGenerated(f.path)) continue
    const base = f.path.split("/").pop()!
    if (base === "INVARIANTS.md") {
      const added = new Set(f.added.map((l) => l.trim()))
      for (const l of f.removed) if (l.trim() && !added.has(l.trim())) out.push({ file: f.path, why: "invariant text removed", line: l.trim() })
      continue
    }
    if (f.path.endsWith("tests/run.sh")) {
      for (const l of f.removed) if (l.trim() && gateDropped(l, f.added)) out.push({ file: f.path, why: "a gate dropped from tests/run.sh", line: l.trim() })
      continue
    }
    // The mark is a Coax comment; the words elsewhere (docs, this tool) are not one.
    if (f.path.endsWith(".coax")) for (const l of f.added) if (/\/\/\s*mutant:\s*equivalent/.test(l)) out.push({ file: f.path, why: "new 'mutant: equivalent' mark", line: l.trim() })
    const pattern = f.path.endsWith(".coax") ? CHECK_LINE : isTest(f.path) ? TEST_LINE : null
    if (!pattern) continue
    const added = new Set(f.added.map((l) => l.trim()))
    const removedSet = new Set(f.removed.map((l) => l.trim()))
    const gone: string[] = []
    for (const l of f.removed) {
      const t = l.trim()
      if (!pattern.test(t) || added.has(t)) continue
      const moved = addedAnywhere.get(t)
      if (moved && moved !== f.path) {
        if (hasAuthority(f.path) && !hasAuthority(moved)) out.push({ file: f.path, why: `check moved into ${moved}, which has no #authority`, line: t })
        continue
      }
      gone.push(t)
    }
    // A rewritten check is not a weaker one; fewer checks are.
    const fresh = f.added.filter((l) => pattern.test(l) && !removedSet.has(l.trim())).length
    for (const t of gone.slice(0, Math.max(0, gone.length - fresh))) out.push({ file: f.path, why: "check removed", line: t })
  }
  return out
}

export function changesCode(files: FileDiff[]): boolean {
  return files.some((f) => {
    if (isDoc(f.path) || isTest(f.path) || isGenerated(f.path) || f.path.endsWith("INVARIANTS.md")) return false
    if (!f.path.endsWith(".coax")) return true
    // A .coax file: code changed when a non-check, non-comment line changed.
    return [...f.added, ...f.removed].some((l) => l.trim() && !CHECK_LINE.test(l) && !l.trim().startsWith("//"))
  })
}

export function parseDiff(text: string): FileDiff[] {
  const files: FileDiff[] = []
  let cur: FileDiff | null = null
  for (const line of text.split("\n")) {
    if (line.startsWith("diff --git ")) {
      cur = { path: line.replace(/^diff --git a\/(.*) b\/.*$/, "$1"), added: [], removed: [] }
      files.push(cur)
    } else if (!cur || line.startsWith("+++ ") || line.startsWith("--- ")) continue
    else if (line.startsWith("+++") || line.startsWith("---")) continue
    else if (line.startsWith("+")) cur.added.push(line.slice(1))
    else if (line.startsWith("-")) cur.removed.push(line.slice(1))
  }
  return files
}

export function authors(dir = REVIEWS): Set<string> {
  try { return new Set(JSON.parse(readFileSync(join(dir, "authors.json"), "utf8")).pubkeys) } catch { return new Set() }
}

// "independent", "invalid: <why>" or "none".
export function reviewOf(commit: string, dir = REVIEWS, authorKeys = authors(dir)): string {
  const file = join(dir, `${commit}.json`)
  if (!existsSync(file)) return "none"
  let ev: Event
  try { ev = JSON.parse(readFileSync(file, "utf8")) } catch { return "invalid: not JSON" }
  if (ev.kind !== REVIEW_KIND) return "invalid: wrong kind"
  if (!verifyEvent({ ...ev })) return "invalid: bad signature"
  const tag = (k: string) => ev.tags.find((t) => t[0] === k)?.[1]
  if (tag("commit") !== commit) return "invalid: names another commit"
  if (tag("t") !== "coax-review") return "invalid: not a coax review"
  if (authorKeys.has(ev.pubkey)) return "invalid: signed with an author key"
  return "independent"
}

export function signReview(commit: string, key: Uint8Array, why: string, authorKeys: Set<string>): Event {
  if (authorKeys.has(getPublicKey(key))) throw new Error("that key is an author key; a review needs the reviewer's own key")
  return finalizeEvent({ kind: REVIEW_KIND, created_at: Math.floor(Date.now() / 1000), content: why,
    tags: [["commit", commit], ["t", "coax-review"], ["review", "approve"]] }, key)
}

function git(...args: string[]): string {
  const p = Bun.spawnSync(["git", "-C", ROOT, ...args])
  if (p.exitCode) throw new Error(`git ${args[0]}: ${p.stderr.toString()}`)
  return p.stdout.toString()
}

async function report(from: string, to: string, note: boolean, json: boolean) {
  const commits = git("rev-list", "--reverse", "--no-merges", `${from}..${to}`).split("\n").filter(Boolean)
  const rows = []
  for (const c of commits) {
    const files = parseDiff(git("show", "--format=", "-U0", "--no-renames", c))
    const found = weakenings(files, (p) => { try { return /^#authority;/m.test(git("show", `${c}:${p}`)) } catch { return false } })
    if (!found.length || !changesCode(files)) continue
    const subject = git("log", "-1", "--format=%s", c).trim()
    rows.push({ commit: c, subject, review: reviewOf(c), findings: found, tasks: [...subject.matchAll(/#([0-9a-f]{4})\b/g)].map((m) => m[1]) })
  }
  if (json) console.log(JSON.stringify({ schema: "coax.review-report.v1", from, to, commits: commits.length, flagged: rows }, null, 2))
  else {
    console.log(`review: ${commits.length} commits, ${rows.length} weaken a check together with its code (report only)`)
    for (const r of rows) {
      console.log(`  ${r.commit.slice(0, 10)} ${r.review === "independent" ? "reviewed" : `NOT REVIEWED (${r.review})`}: ${r.subject}`)
      for (const f of r.findings.slice(0, 5)) console.log(`    ${f.file}: ${f.why}: ${f.line.slice(0, 100)}`)
    }
  }
  if (note) for (const r of rows) {
    if (r.review === "independent") continue
    for (const task of r.tasks) {
      const text = `Review flag (report only): ${r.commit.slice(0, 10)} weakens a check together with the code it checks (${r.findings.map((f) => `${f.file}: ${f.why}`).slice(0, 3).join("; ")}) and has no independent review. A review agent signs one with: bun coax/web/project/review.ts sign ${r.commit} --key ITS_KEY`
      Bun.spawnSync(["bun", "publish.ts", "note", task, text, "--by", "coax-review", "--project", "coax"], { cwd: import.meta.dir, stdout: "ignore", stderr: "ignore" })
    }
  }
}

if (import.meta.main) {
  const [cmd, ...rest] = process.argv.slice(2)
  const flag = (n: string) => { const i = rest.indexOf(`--${n}`); return i >= 0 ? rest[i + 1] : undefined }
  if (cmd === "report") await report(flag("from") ?? "origin/main~20", flag("to") ?? "HEAD", rest.includes("--note"), rest.includes("--json"))
  else if (cmd === "sign") {
    const commit = git("rev-parse", `${rest[0]}^{commit}`).trim()
    const keyFile = flag("key")
    if (!keyFile) throw new Error("sign needs --key FILE (the reviewer's own key)")
    const ev = signReview(commit, hexToBytes(readFileSync(keyFile, "utf8").trim()), flag("why") ?? "reviewed", authors())
    mkdirSync(REVIEWS, { recursive: true })
    writeFileSync(join(REVIEWS, `${commit}.json`), JSON.stringify(ev, null, 2) + "\n")
    console.log(`signed coax/reviews/${commit}.json as ${ev.pubkey}`)
  } else if (cmd === "keygen") {
    const file = rest[0]
    if (!file || existsSync(file)) throw new Error("keygen FILE: FILE must be a new path")
    writeFileSync(file, bytesToHex(generateSecretKey()) + "\n", { mode: 0o600 })
    chmodSync(file, 0o600)
    console.log(getPublicKey(hexToBytes(readFileSync(file, "utf8").trim())))
  } else {
    console.error("usage: bun review.ts report --from BASE --to REF [--note] [--json] | sign COMMIT --key FILE [--why TEXT] | keygen FILE")
    process.exit(2)
  }
}

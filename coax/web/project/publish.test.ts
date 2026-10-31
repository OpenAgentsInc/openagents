// The publisher against a throwaway in-memory relay, with a throwaway key and
// project: a dry run sends nothing, publishing twice publishes once, an edit
// is a revision, a move above a blocker warns, and status lands; then a
// worker claims a task, notes on it, releases it, claims again and finishes it.
import { afterAll, beforeAll, expect, test } from "bun:test"
import { startRelay } from "nostr-effect/relay"
import { generateSecretKey, getPublicKey, type Event } from "nostr-effect/pure"
import { bytesToHex, hexToBytes } from "@noble/hashes/utils"
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { fold, KIND_ORDER } from "./model"
import { Relay, Signer } from "./relay"

const port = 7400 + Math.floor(Math.random() * 40)
const url = `ws://127.0.0.1:${port}`
const project = `test${Date.now().toString(36)}`
const dir = mkdtempSync(join(tmpdir(), "project-"))
const keyFile = join(dir, "key")
let relay: { stop?: () => unknown; close?: () => unknown }

const item = (slug: string, kind: string, title: string, extra: Record<string, unknown> = {}) =>
  ({ slug, kind, title, body: `Build ${title}.`, labels: ["test"], ...extra })
const work = (sha256Title: string) => ({
  v: "openagents.project-work.v1",
  items: [
    item("init", "initiative", "The initiative"),
    item("m-one", "milestone", "First milestone", { parent: "init" }),
    item("sha256", "task", sha256Title, { parent: "m-one", size: "S" }),
    item("hmac", "task", "HMAC", { parent: "m-one", size: "S", blocked_by: ["sha256"] }),
  ],
})

// async: the relay lives in this process, so a blocking spawn would starve it
async function run(...args: string[]) {
  const p = Bun.spawn(["bun", join(import.meta.dir, "publish.ts"), ...args,
    "--project", project, "--dir", dir, "--key", keyFile, "--relay", url], { stdout: "pipe", stderr: "pipe" })
  const out = (await new Response(p.stdout).text()) + (await new Response(p.stderr).text())
  if ((await p.exited) !== 0) throw new Error(out)
  return out
}

beforeAll(async () => {
  relay = (await startRelay({ port })) as typeof relay
  writeFileSync(keyFile, bytesToHex(generateSecretKey()), { mode: 0o600 })
  writeFileSync(join(dir, "project.json"), JSON.stringify({
    slug: project, name: "Test", description: "A test project", relay: url, path: "test/", contributions: "open",
    license: "Apache-2.0", repo: { d: project, name: "test", description: "test", web: "https://example.com", clone: "https://example.com/t.git", euc: "0".repeat(40) },
  }))
  mkdirSync(join(dir, "work"))
  writeFileSync(join(dir, "work", "a.json"), JSON.stringify(work("SHA-256")))
})

afterAll(async () => {
  await relay?.stop?.(); await relay?.close?.()
  rmSync(dir, { recursive: true, force: true })
  rmSync(join(import.meta.dir, "state", `${project}.json`), { force: true })
})

test("dry run, publish, publish again, edit, move, status", async () => {
  const dry = await run("publish", "--dry-run")
  expect(dry).toContain("would publish kind 1621: issue hmac")
  expect(dry).toContain("would publish kind 30621")
  expect(await run("list")).toContain("0 items")                      // the dry run sent nothing

  const first = await run("publish")
  expect(first.match(/published kind 1621/g)?.length).toBe(4)
  expect(first).toContain("published kind 30623")
  expect(first).not.toContain("refused")

  expect(await run("publish")).not.toContain("published kind")       // idempotent

  writeFileSync(join(dir, "work", "a.json"), JSON.stringify(work("SHA-256, one-shot")))
  const edit = await run("publish")
  expect(edit).toContain("published kind 1111: revision of sha256")
  expect(edit).not.toContain("kind 1621")

  expect(await run("move", "hmac", "--before", "sha256")).toContain("warning: hmac is now above sha256")
  expect(await run("status", "sha256", "resolved")).toContain("published kind 1631")

  const listed = await run("list")
  expect(listed).toContain("SHA-256, one-shot")
  expect(listed).toMatch(/done\s+task\s+sha256/)
  expect(listed).toMatch(/ready\s+task\s+hmac/)
  const order = listed.split("\n").filter((l) => /task/.test(l)).map((l) => l.match(/ (hmac|sha256) /)?.[1])
  expect(order).toEqual(["hmac", "sha256"])
}, 60000)

test("claim, note, release, show, plan, done", async () => {
  await expect(run("claim", "hmac")).rejects.toThrow("usage: claim")
  await expect(run("claim", "m-one", "--by", "w1")).rejects.toThrow("only tasks are claimed")
  await expect(run("claim", "sha256", "--by", "w1")).rejects.toThrow("already resolved")
  expect(await run("claim", "hmac", "--by", "w1")).toContain("published kind 1111: hmac claimed by w1")
  expect(await run("list")).toMatch(/on:w1\s+task\s+hmac/)
  await expect(run("claim", "hmac", "--by", "w2")).rejects.toThrow("claimed by w1")
  expect(await run("note", "hmac", "chose the RFC 4231 vectors")).toContain("note on hmac")
  expect(await run("release", "hmac")).toContain("hmac released")
  expect(await run("list")).toMatch(/ready\s+task\s+hmac/)
  // by alias, with its #
  const alias = (await run("list")).match(/#([0-9a-f]{4,})\s+\S+\s+ready\s+task\s+hmac/)![1]
  expect(await run("claim", `#${alias}`, "--by", "w2")).toContain("claimed by w2")
  const shown = await run("show", "hmac")
  expect(shown).toContain("claimed by w2")
  expect(shown).toContain("Build HMAC.")
  expect(shown).toContain("w1: chose the RFC 4231 vectors")
  expect(shown).toContain("waits on #")
  // the worker's plan: set, start, check, add, drop; the latest replaces the last
  expect(await run("plan", "hmac")).toContain("no plan yet")
  expect(await run("plan", "hmac", "set", "read RFC 4231", "write the vectors", "pad the key")).toContain("plan for hmac (0/3)")
  expect(await run("plan", "hmac", "start", "1")).toContain("[>] read RFC 4231")
  expect(await run("plan", "hmac", "check", "1")).toContain("plan for hmac (1/3)")
  expect(await run("plan", "hmac", "add", "constant-time compare")).toContain("4. [ ] constant-time compare")
  expect(await run("plan", "hmac", "start", "2")).toContain("[>] write the vectors")
  expect(await run("plan", "hmac", "drop", "3")).toContain("plan for hmac (1/3)")
  await expect(run("plan", "hmac", "check", "9")).rejects.toThrow("steps are 1 to 3")
  const planned = await run("show", "hmac")
  expect(planned).toContain("1 of 3 done, by w2")
  expect(planned).toMatch(/1\. \[x\] read RFC 4231\n\s+2\. \[>\] write the vectors\n\s+3\. \[ \] constant-time compare/)
  expect(planned).not.toContain("w2: - [")   // a plan isn't a note
  await expect(run("plan", "m-one", "set", "x")).rejects.toThrow("plans are for tasks")
  await expect(run("done", "hmac", "--commit", "nothex")).rejects.toThrow("usage: done")
  const finished = await run("done", "hmac", "--commit", "a5c9177b1d0000000000000000000000000000ff")
  expect(finished).toContain("hmac done in a5c9177b1d")
  expect(finished).toContain("published kind 1631: hmac resolved")
  const after = await run("list")
  expect(after).toMatch(/done\s+task\s+hmac\s+HMAC\s+\(a5c9177b1d\)/)
  expect(await run("show", "hmac")).toContain("done in a5c9177b1d")
  await expect(run("done", "hmac", "--commit", "a5c9177b1d")).rejects.toThrow("already resolved")
}, 60000)

test("ten workers at once each land their note once; a repeated note is not posted twice", async () => {
  const outs = await Promise.all(Array.from({ length: 10 }, (_, k) => run("note", "hmac", `parallel note ${k}`, "--by", `w${k}`)))
  for (const o of outs) expect(o).toContain("published kind 1111: note on hmac")
  expect(await run("note", "hmac", "parallel note 3", "--by", "w3")).toContain("the same note is already on hmac")
  const shown = await run("show", "hmac")
  for (let k = 0; k < 10; k++) expect(shown.split(`w${k}: parallel note ${k}`).length).toBe(2)
}, 60000)

test("open-task order preserves active legacy ranks and all issues at the page capacity", async () => {
  await run("publish")
  await run("status", "sha256", "open")
  await run("status", "hmac", "resolved")
  const key = hexToBytes(readFileSync(keyFile, "utf8").trim()), pk = getPublicKey(key)
  const connection = await Relay.open(url, null)
  const config = JSON.parse(readFileSync(join(dir, "project.json"), "utf8"))
  const read = () => connection.all([{ authors: [pk] }])
  const latestOrder = (events: Event[]) => events.filter((e) => e.kind === KIND_ORDER)
    .sort((a, b) => b.created_at - a.created_at || a.id.localeCompare(b.id))[0]
  try {
    const before = await read(), state = fold(pk, config, before)
    const ids = new Map(state.issues.map((i) => [i.slug, i.id]))
    // Historical order events also ranked hierarchy and finished tasks.
    // Compaction keeps an active task's short rank without reordering it.
    const legacy = new Map([["init", "1"], ["m-one", "2"], ["sha256", "3"], ["hmac", "4"]])
    const signer = new Signer(key)
    signer.after(Math.max(...before.map((e) => e.created_at)))
    await connection.publish(signer.sign({ kind: KIND_ORDER, content: "", tags: [
      ["d", config.slug], ["a", `30621:${pk}:${config.slug}`, config.relay],
      ...[...legacy].map(([slug, rank]) => ["item", ids.get(slug)!, rank]),
      ["alt", "The legacy task order"],
    ] }))

    expect(await run("publish")).toContain("order of 1")
    const compacted = await read(), order = latestOrder(compacted)
    const ranked = order.tags.filter((t) => t[0] === "item")
    expect(ranked).toEqual([["item", ids.get("sha256")!, "3"]])
    const after = fold(pk, config, compacted)
    expect(after.issues.length).toBe(state.issues.length)
    expect(after.issues.find((i) => i.slug === "hmac")?.status).toBe("resolved")
    expect(after.issues.filter((i) => i.kind !== "task").map((i) => i.slug)).toEqual(["init", "m-one"])
    const shown = await run("list")
    expect(shown).toMatch(/initiative\s+init/)
    expect(shown).toMatch(/milestone\s+m-one/)
    expect(await run("publish")).not.toContain("published kind")
    expect(latestOrder(await read()).id).toBe(order.id)

    await expect(run("move", "init", "--before", "sha256")).rejects.toThrow("only tasks can be ordered")
    await expect(run("move", "sha256", "--before", "m-one")).rejects.toThrow("only tasks can be ordered")
    await expect(run("move", "hmac", "--before", "sha256")).rejects.toThrow("only open items are in the order")
    await expect(run("move", "sha256", "--before", "hmac")).rejects.toThrow("only open items are in the order")
    const orderFile = join(dir, "hierarchy-order.txt")
    writeFileSync(orderFile, "init\nsha256\n")
    await expect(run("respread", "--order", orderFile)).rejects.toThrow("order files may contain only tasks")

    // The page admits 256 total issues; hierarchy issues must not consume
    // the relay's 256-tag order-event budget. Finished issues remain in the
    // project and its history while only active tasks consume order tags.
    const atCapacity = work("SHA-256, one-shot")
    for (let i = 0; i < 36; i++) atCapacity.items.push(item(`extra-m${i}`, "milestone", `Extra milestone ${i}`, { parent: "init" }))
    for (let i = 0; i < 216; i++) atCapacity.items.push(item(`extra-task${i}`, "task", `Extra task ${i}`, { parent: "m-one", size: "S" }))
    expect(atCapacity.items.length).toBe(256)
    writeFileSync(join(dir, "work", "a.json"), JSON.stringify(atCapacity))
    await run("publish")
    const complete = await read(), completeState = fold(pk, config, complete), completeOrder = latestOrder(complete)
    const taskIds = new Set(completeState.issues.filter((i) => i.kind === "task" && i.status !== "resolved" && i.status !== "closed").map((i) => i.id))
    const entries = completeOrder.tags.filter((t) => t[0] === "item")
    expect(completeState.issues.length).toBe(256)
    expect(entries.length).toBe(217)
    expect(completeOrder.tags.length).toBeLessThanOrEqual(256)
    expect(new Set(entries.map((t) => t[1]))).toEqual(taskIds)
    expect(entries.find((t) => t[1] === ids.get("sha256"))?.[2]).toBe("3")
    expect(entries.some((t) => t[1] === ids.get("hmac"))).toBe(false)
    expect(completeState.issues.find((i) => i.slug === "hmac")?.status).toBe("resolved")
    expect(await run("publish")).not.toContain("published kind")
  } finally { connection.close() }
}, 120000)

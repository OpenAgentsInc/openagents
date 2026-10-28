// The publisher against a throwaway in-memory relay, with a throwaway key and
// project: a dry run sends nothing, publishing twice publishes once, an edit
// is a revision, a move above a blocker warns, and status lands; then a
// worker claims a task, notes on it, releases it, claims again and finishes it.
import { afterAll, beforeAll, expect, test } from "bun:test"
import { startRelay } from "nostr-effect/relay"
import { generateSecretKey } from "nostr-effect/pure"
import { bytesToHex } from "@noble/hashes/utils"
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"

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

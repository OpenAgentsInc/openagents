import { describe, expect, test } from "bun:test"
import {
  PYLON_COMMAND_CATALOG,
  findCommandEntry,
  projectCommandCatalog,
  projectCommandHelp,
} from "../src/cli-catalog"

// CL-5035: `pylon help --json` is the machine-readable command catalog an agent
// uses to DISCOVER the full steering surface.
describe("pylon command catalog", () => {
  test("catalog projects a stable schema + a command count that matches", () => {
    const catalog = projectCommandCatalog()
    expect(catalog.schema).toBe("openagents.pylon.command_catalog.v1")
    expect(catalog.commandCount).toBe(PYLON_COMMAND_CATALOG.length)
    expect(catalog.commands.length).toBe(catalog.commandCount)
    expect(typeof catalog.generatedAt).toBe("string")
  })

  test("the new CL-5035 steering verbs are all listed", () => {
    for (const name of ["help", "sessions", "approvals", "deploy", "training", "khala"]) {
      expect(findCommandEntry(name), `missing ${name}`).toBeDefined()
    }
  })

  test("sessions/approvals/deploy are node-backed control verbs", () => {
    for (const name of ["sessions", "approvals", "deploy"]) {
      const entry = findCommandEntry(name)!
      expect(entry.needsNode).toBe(true)
      expect(entry.json).toBe(true)
    }
  })

  test("sessions catalog advertises reply continuation controls", () => {
    const entry = findCommandEntry("sessions")!
    expect(entry.summary).toContain("reply")
    expect(entry.args[0]?.name).toContain("reply")
    expect(entry.args[0]?.name).toContain("batch")
    expect(entry.args.some((arg) => arg.name === "--session-ref" && arg.description.includes("reply"))).toBe(true)
    expect(entry.args.some((arg) => arg.name === "--wait" && arg.kind === "flag")).toBe(true)
    expect(entry.args.some((arg) => arg.name === "--managed-worktree" && arg.kind === "flag")).toBe(true)
    expect(entry.args.some((arg) => arg.name === "--base-ref")).toBe(true)
    expect(entry.args.some((arg) => arg.name === "--lane" && arg.description.includes("cloud-gcp"))).toBe(true)
    expect(entry.args.some((arg) => arg.name === "--tasks")).toBe(true)
    expect(entry.args.some((arg) => arg.name === "--concurrency")).toBe(true)
  })

  test("only the wallet/work/tip verbs are flagged as spending", () => {
    const spenders = PYLON_COMMAND_CATALOG.filter((c) => c.spends).map((c) => c.command).sort()
    expect(spenders).toEqual(["tip", "wallet", "work"])
  })

  test("public activity evidence commands are read-only network-backed", () => {
    for (const name of ["activity", "timeline", "replay", "receipts", "evidence-pack"]) {
      const entry = findCommandEntry(name)!
      expect(entry, `missing ${name}`).toBeDefined()
      expect(entry.needsNetwork).toBe(true)
      expect(entry.needsNode).toBeUndefined()
      expect(entry.mutates).toBe(false)
      expect(entry.spends).toBe(false)
      expect(entry.json).toBe(true)
    }
  })

  test("khala issuer is catalogued as network-backed and non-spending", () => {
    const entry = findCommandEntry("khala")!
    expect(entry.needsNetwork).toBe(true)
    expect(entry.spends).toBe(false)
    expect(entry.mutates).toBe(true)
    expect(entry.json).toBe(true)
    expect(entry.args[0]?.name).toContain("request")
    expect(entry.args.some((arg) => arg.name === "--workflow")).toBe(true)
    expect(entry.args.some((arg) => arg.name === "--resume")).toBe(true)
  })

  test("every entry has a non-empty summary and unique command name", () => {
    const seen = new Set<string>()
    for (const entry of PYLON_COMMAND_CATALOG) {
      expect(entry.summary.length).toBeGreaterThan(0)
      expect(seen.has(entry.command)).toBe(false)
      seen.add(entry.command)
    }
  })

  test("projectCommandHelp returns the entry or null", () => {
    expect(projectCommandHelp("sessions")?.command).toBe("sessions")
    expect(projectCommandHelp("nope")).toBeNull()
  })
})

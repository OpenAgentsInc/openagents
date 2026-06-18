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
    for (const name of ["help", "sessions", "approvals", "deploy", "training"]) {
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
    expect(entry.args.some((arg) => arg.name === "--session-ref" && arg.description.includes("reply"))).toBe(true)
    expect(entry.args.some((arg) => arg.name === "--wait" && arg.kind === "flag")).toBe(true)
  })

  test("only the wallet/work/tip verbs are flagged as spending", () => {
    const spenders = PYLON_COMMAND_CATALOG.filter((c) => c.spends).map((c) => c.command).sort()
    expect(spenders).toEqual(["tip", "wallet", "work"])
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

import { describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import {
  createCodexFleetOffloadPlan,
  parseCodexFleetOffloadArgs,
} from "../src/codex-fleet-offload"
import { createBootstrapSummary, parseBootstrapArgs } from "../src/bootstrap"

async function withHome<T>(fn: (home: string) => Promise<T>) {
  const home = await mkdtemp(join(tmpdir(), "pylon-codex-offload-"))
  try {
    return await fn(home)
  } finally {
    await rm(home, { recursive: true, force: true })
  }
}

async function writeCodexAccountRegistry(home: string, refs: string[]) {
  const accounts = []
  for (const ref of refs) {
    const accountHome = join(home, "accounts", "codex", ref)
    await mkdir(accountHome, { recursive: true })
    await writeFile(join(accountHome, "auth.json"), "{}\n")
    accounts.push({ ref, provider: "codex", home: accountHome })
  }
  await writeFile(
    join(home, "config.json"),
    `${JSON.stringify({ dev: { accounts } }, null, 2)}\n`,
  )
}

describe("codex fleet offload planner", () => {
  test("builds a redacted Tailnet split plan for selected Codex accounts", async () => {
    await withHome(async (home) => {
      await writeCodexAccountRegistry(home, ["codex-4", "codex-5", "codex-6", "codex-7"])
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const options = parseCodexFleetOffloadArgs([
        "--accounts",
        "codex-4,codex-5,codex-6,codex-7",
        "--target",
        "imac-pro-bertha:2",
        "--target",
        "macbook-pro-m2:2",
        "--json",
      ])

      const plan = await createCodexFleetOffloadPlan(summary, options)

      expect(plan.schema).toBe("openagents.pylon.codex_fleet_offload_plan.v0.1")
      expect(plan.mode).toBe("redacted")
      expect(plan.source.accountCount).toBe(4)
      expect(plan.targets).toEqual([
        expect.objectContaining({ assigned: 2, host: "imac-pro-bertha", launchConcurrency: 2 }),
        expect.objectContaining({ assigned: 2, host: "macbook-pro-m2", launchConcurrency: 2 }),
      ])
      expect(plan.assignments.map((assignment) => assignment.accountRef)).toEqual([
        "codex-4",
        "codex-5",
        "codex-6",
        "codex-7",
      ])
      expect(plan.assignments.every((assignment) => Object.keys(assignment.commands).length === 0)).toBe(true)
      expect(JSON.stringify(plan)).not.toContain(home)
      expect(plan.nextSteps.join("\n")).toContain("--include-private-paths")
    })
  })

  test("requires explicit private-path opt-in before printing tar, scp, and launch commands", async () => {
    await withHome(async (home) => {
      await writeCodexAccountRegistry(home, ["codex-4"])
      const summary = createBootstrapSummary(parseBootstrapArgs(["--json"]), { PYLON_HOME: home })
      const options = parseCodexFleetOffloadArgs([
        "--accounts",
        "codex-4",
        "--target",
        "imac-pro-bertha:1",
        "--remote-home",
        "~/.pylon-fable",
        "--remote-repo",
        "~/work/openagents",
        "--bundle-dir",
        join(home, "bundles"),
        "--include-private-paths",
        "--json",
      ])

      const plan = await createCodexFleetOffloadPlan(summary, options)
      const commands = plan.assignments[0]?.commands

      expect(plan.mode).toBe("private_commands")
      expect(commands?.pack).toContain("tar")
      expect(commands?.copy).toContain("scp")
      expect(commands?.launch).toContain("provider go-online --json")
      expect(commands?.launch).toContain("presence heartbeat --json")
      expect(commands?.launch).toContain("bun apps/pylon/src/index.ts node")
      expect(JSON.stringify(plan)).toContain(home)
    })
  })

  test("rejects selected accounts that exceed target capacity", () => {
    expect(() =>
      parseCodexFleetOffloadArgs([
        "--accounts",
        "codex-4,codex-5",
        "--target",
        "imac-pro-bertha:1",
        "--json",
      ]),
    ).toThrow(/exceeds target capacity/)
  })

  test("rejects remote paths with shell metacharacters", () => {
    expect(() =>
      parseCodexFleetOffloadArgs([
        "--accounts",
        "codex-4",
        "--target",
        "imac-pro-bertha:1",
        "--remote-home",
        "~/.pylon-fable;rm",
        "--json",
      ]),
    ).toThrow(/remote path/)
  })
})

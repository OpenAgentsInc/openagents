import { describe, expect, test } from "bun:test"
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"

import { discoverWorkspaceDirectories, discoverWorkspaceTargets, workspacePatterns } from "./check-workspace"

const fixture = async (): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), "openagents-check-"))
  await writeFile(join(root, "package.json"), JSON.stringify({ workspaces: { packages: ["packages/*"] } }))
  await mkdir(join(root, "packages", "zeta"), { recursive: true })
  await mkdir(join(root, "packages", "alpha"), { recursive: true })
  await writeFile(join(root, "packages", "zeta", "package.json"), JSON.stringify({ name: "zeta", scripts: { test: "bun test" } }))
  await writeFile(join(root, "packages", "alpha", "package.json"), JSON.stringify({ name: "alpha", scripts: { typecheck: "tsc", format: "prettier -w ." } }))
  return root
}

describe("workspace check discovery", () => {
  test("reads both Bun workspace manifest shapes", () => {
    expect(workspacePatterns({ workspaces: ["apps/*"] })).toEqual(["apps/*"])
    expect(workspacePatterns({ workspaces: { packages: ["packages/*"] } })).toEqual(["packages/*"])
  })

  test("discovers new packages without a root chain edit and orders them", async () => {
    const root = await fixture()
    try {
      expect(await discoverWorkspaceDirectories(root)).toEqual(["packages/alpha", "packages/zeta"])
      expect((await discoverWorkspaceTargets(root, "test")).map((target) => target.directory)).toEqual(["packages/zeta"])
      expect((await discoverWorkspaceTargets(root, "typecheck")).map((target) => target.directory)).toEqual(["packages/alpha"])
      expect((await discoverWorkspaceTargets(root, "fmt")).map((target) => target.command.at(-1))).toEqual(["format"])
    } finally {
      await rm(root, { recursive: true, force: true })
    }
  })
})

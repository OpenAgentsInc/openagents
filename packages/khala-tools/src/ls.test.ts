import { mkdir, mkdtemp, symlink, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createLsTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-ls-tool-"))
}

describe("ls tool", () => {
  test("lists directories with stable sorting, dotfiles, and directory suffixes", async () => {
    const workspace = await makeWorkspace()
    await mkdir(join(workspace, "Zoo"))
    await writeFile(join(workspace, "alpha.txt"), "a")
    await writeFile(join(workspace, ".dot"), "d")
    await writeFile(join(workspace, "Beta.txt"), "b")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createLsTool()]),
        { arguments: {}, id: "call_1", name: "ls", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe(".:\n.dot\nalpha.txt\nBeta.txt\nZoo/")
    expect(result.ui).toMatchObject({
      displayPath: ".",
      entries: [
        { kind: "file", name: ".dot" },
        { kind: "file", name: "alpha.txt" },
        { kind: "file", name: "Beta.txt" },
        { kind: "directory", name: "Zoo/" },
      ],
      totalEntries: 4,
      truncated: false,
    })
  })

  test("treats empty directories as successful results", async () => {
    const workspace = await makeWorkspace()
    await mkdir(join(workspace, "empty"))
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createLsTool()]),
        { arguments: { path: "empty" }, id: "call_1", name: "ls", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe("empty:\n(empty)")
  })

  test("bounds output by limit", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "a.txt"), "a")
    await writeFile(join(workspace, "b.txt"), "b")
    await writeFile(join(workspace, "c.txt"), "c")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createLsTool()]),
        { arguments: { limit: 2 }, id: "call_1", name: "ls", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe(".:\na.txt\nb.txt\n[ls truncated; refine path or increase limit]")
    expect(result.ui).toMatchObject({ totalEntries: 3, truncated: true })
  })

  test("fails when path is not a directory", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "file.txt"), "x")
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createLsTool()]),
        { arguments: { path: "file.txt" }, id: "call_1", name: "ls", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("ls_not_directory")
  })

  test("requires permission for symlink directory escapes", async () => {
    const workspace = await makeWorkspace()
    const outside = await mkdtemp(join(tmpdir(), "khala-ls-outside-"))
    await writeFile(join(outside, "outside.txt"), "x")
    await symlink(outside, join(workspace, "outside-link"))

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createLsTool()]),
        { arguments: { path: "outside-link" }, id: "call_1", name: "ls", sessionId: "s1" },
        makeKhalaToolServices({
          permission: denyAllKhalaPermissionService,
          workingDirectory: workspace,
        }),
      ),
    )

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("ls_external_directory_denied")
  })

  test("blocks credential-shaped directories", async () => {
    const workspace = await makeWorkspace()
    await mkdir(join(workspace, ".secrets"))
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createLsTool()]),
        { arguments: { path: ".secrets" }, id: "call_1", name: "ls", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("ls_blocked_credential_path")
  })
})

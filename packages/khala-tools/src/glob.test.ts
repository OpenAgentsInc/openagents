import { mkdir, mkdtemp, symlink, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createGlobTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-glob-tool-"))
}

describe("glob tool", () => {
  test("finds recursive paths as workspace-relative POSIX paths", async () => {
    const workspace = await makeWorkspace()
    await mkdir(join(workspace, "src", "nested"), { recursive: true })
    await writeFile(join(workspace, "src", "a.ts"), "a")
    await writeFile(join(workspace, "src", "nested", "b.ts"), "b")
    await writeFile(join(workspace, "src", "nested", "c.md"), "c")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createGlobTool()]),
        { arguments: { pattern: "**/*.ts" }, id: "call_1", name: "glob", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe("src/a.ts\nsrc/nested/b.ts")
    expect(result.ui).toMatchObject({
      matches: ["src/a.ts", "src/nested/b.ts"],
      pattern: "**/*.ts",
      totalMatches: 2,
      truncated: false,
    })
  })

  test("respects .gitignore by default", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, ".gitignore"), "*.log\n")
    await writeFile(join(workspace, "kept.txt"), "x")
    await writeFile(join(workspace, "ignored.log"), "x")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createGlobTool()]),
        { arguments: { pattern: "*" }, id: "call_1", name: "glob", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain(".gitignore")
    expect(result.modelOutput.text).toContain("kept.txt")
    expect(result.modelOutput.text).not.toContain("ignored.log")
  })

  test("scopes search roots while preserving workspace-relative output", async () => {
    const workspace = await makeWorkspace()
    await mkdir(join(workspace, "src"), { recursive: true })
    await mkdir(join(workspace, "test"), { recursive: true })
    await writeFile(join(workspace, "src", "a.ts"), "a")
    await writeFile(join(workspace, "test", "a.ts"), "a")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createGlobTool()]),
        { arguments: { path: "src", pattern: "*.ts" }, id: "call_1", name: "glob", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe("src/a.ts")
  })

  test("returns successful no-match output", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "README.md"), "x")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createGlobTool()]),
        { arguments: { pattern: "**/*.ts" }, id: "call_1", name: "glob", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe("(no matches)")
    expect(result.ui).toMatchObject({ totalMatches: 0, truncated: false })
  })

  test("bounds output by limit", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "a.ts"), "a")
    await writeFile(join(workspace, "b.ts"), "b")
    await writeFile(join(workspace, "c.ts"), "c")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createGlobTool()]),
        { arguments: { limit: 2, pattern: "*.ts" }, id: "call_1", name: "glob", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe("a.ts\nb.ts\n[glob truncated; refine pattern/path or increase limit]")
    expect(result.ui).toMatchObject({ totalMatches: 3, truncated: true })
  })

  test("requires permission for symlink directory escapes", async () => {
    const workspace = await makeWorkspace()
    const outside = await mkdtemp(join(tmpdir(), "khala-glob-outside-"))
    await writeFile(join(outside, "outside.ts"), "x")
    await symlink(outside, join(workspace, "outside-link"))

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createGlobTool()]),
        { arguments: { path: "outside-link", pattern: "*.ts" }, id: "call_1", name: "glob", sessionId: "s1" },
        makeKhalaToolServices({
          permission: denyAllKhalaPermissionService,
          workingDirectory: workspace,
        }),
      ),
    )

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("glob_external_directory_denied")
  })
})

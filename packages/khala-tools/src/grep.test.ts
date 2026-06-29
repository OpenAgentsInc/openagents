import { mkdir, mkdtemp, symlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createGrepTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-grep-tool-"))
}

async function runGrep(
  workspace: string,
  args: Readonly<Record<string, unknown>>,
  options: Parameters<typeof createGrepTool>[0] = {},
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createGrepTool(options)]),
      { arguments: args, id: "call_1", name: "grep", sessionId: "s1" },
      makeKhalaToolServices({ workingDirectory: workspace }),
    ),
  )
}

describe("grep tool", () => {
  test("searches regex matches with file, line, column, and snippets", async () => {
    const workspace = await makeWorkspace()
    await mkdir(join(workspace, "src"), { recursive: true })
    await writeFile(join(workspace, "src", "a.ts"), "const task = 'todo: fix bug'\nconst done = true\n")

    const result = await runGrep(workspace, { pattern: "todo: .* bug" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("src/a.ts:1:15: const task = 'todo: fix bug'")
    expect(result.ui).toMatchObject({
      kind: "content_search",
      matches: [
        {
          column: 15,
          file: "src/a.ts",
          line: 1,
          match: "todo: fix bug",
        },
      ],
      totalMatches: 1,
      truncated: false,
    })
  })

  test("literal mode treats regex punctuation as plain text", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "a.txt"), "a.b\naxb\n")

    const result = await runGrep(workspace, { literal: true, pattern: "a.b" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("a.txt:1:1: a.b")
    expect(result.modelOutput.text).not.toContain("axb")
  })

  test("supports case-insensitive search", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "a.txt"), "Alpha\nbeta\n")

    const result = await runGrep(workspace, { ignore_case: true, pattern: "alpha" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("a.txt:1:1: Alpha")
  })

  test("includes bounded context lines", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "a.txt"), "before\nneedle\nafter\n")

    const result = await runGrep(workspace, { context: 1, pattern: "needle" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe("a.txt:1- before\na.txt:2:1: needle\na.txt:3- after")
    expect(result.ui).toMatchObject({
      matches: [
        {
          contextAfter: [{ line: 3, text: "after" }],
          contextBefore: [{ line: 1, text: "before" }],
        },
      ],
    })
  })

  test("skips binary files", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "binary.bin"), new Uint8Array([110, 101, 101, 100, 108, 101, 0]))
    await writeFile(join(workspace, "plain.txt"), "needle\n")

    const result = await runGrep(workspace, { pattern: "needle" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("plain.txt:1:1: needle")
    expect(result.modelOutput.text).not.toContain("binary.bin")
  })

  test("returns successful no-match output", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "README.md"), "hello\n")

    const result = await runGrep(workspace, { pattern: "absent" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe("(no matches)")
    expect(result.ui).toMatchObject({ totalMatches: 0, truncated: false })
  })

  test("respects .gitignore by default", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, ".gitignore"), "*.log\n")
    await writeFile(join(workspace, "kept.txt"), "needle\n")
    await writeFile(join(workspace, "ignored.log"), "needle\n")

    const result = await runGrep(workspace, { pattern: "needle" })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("kept.txt")
    expect(result.modelOutput.text).not.toContain("ignored.log")
  })

  test("bounds matches and records truncation", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "a.txt"), "needle 1\nneedle 2\nneedle 3\n")

    const result = await runGrep(workspace, { limit: 2, pattern: "needle" }, { maxMatches: 10 })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("a.txt:1:1: needle 1")
    expect(result.modelOutput.text).toContain("a.txt:2:1: needle 2")
    expect(result.modelOutput.text).not.toContain("needle 3")
    expect(result.modelOutput.text).toContain("[grep truncated; refine pattern/path/glob or increase limit]")
    expect(result.ui).toMatchObject({ totalMatches: 3, truncated: true })
  })

  test("redacts secret-looking matches from public summaries", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "safe.txt"), "OPENROUTER_API_KEY=sk-or-aaaaaaaaaaaaaaaa\n")

    const result = await runGrep(workspace, { pattern: "OPENROUTER_API_KEY" })

    expect(result.status).toBe("ok")
    expect(result.publicSummary).not.toContain("sk-or-aaaaaaaaaaaaaaaa")
    expect(result.modelOutput.text).not.toContain("sk-or-aaaaaaaaaaaaaaaa")
    expect(result.redactionRefs).toContain("redaction.khala_tool.public_text")
  })

  test("requires permission for symlink directory escapes", async () => {
    const workspace = await makeWorkspace()
    const outside = await mkdtemp(join(tmpdir(), "khala-grep-outside-"))
    await writeFile(join(outside, "outside.txt"), "needle\n")
    await symlink(outside, join(workspace, "outside-link"))

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createGrepTool()]),
        { arguments: { path: "outside-link", pattern: "needle" }, id: "call_1", name: "grep", sessionId: "s1" },
        makeKhalaToolServices({
          permission: denyAllKhalaPermissionService,
          workingDirectory: workspace,
        }),
      ),
    )

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("grep_external_directory_denied")
  })
})

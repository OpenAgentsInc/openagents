import { createHash } from "node:crypto"
import { mkdtemp, readFile, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createWriteTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-write-tool-"))
}

async function runWrite(workspace: string, args: Readonly<Record<string, unknown>>) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createWriteTool()]),
      { arguments: args, id: "call_1", name: "write", sessionId: "s1" },
      makeKhalaToolServices({ workingDirectory: workspace }),
    ),
  )
}

function sha256(value: Buffer): string {
  return createHash("sha256").update(value).digest("hex")
}

describe("write tool", () => {
  test("creates a new file", async () => {
    const workspace = await makeWorkspace()

    const result = await runWrite(workspace, { content: "hello\n", path: "a.txt" })

    expect(result.status).toBe("ok")
    expect(await readFile(join(workspace, "a.txt"), "utf8")).toBe("hello\n")
    expect(result.ui).toMatchObject({
      bytesWritten: 6,
      existed: false,
      kind: "file_write",
      path: "a.txt",
    })
    expect(result.publicSummary).not.toContain("hello")
  })

  test("creates parent directories for workspace paths", async () => {
    const workspace = await makeWorkspace()

    const result = await runWrite(workspace, { content: "nested\n", path: "src/generated/a.txt" })

    expect(result.status).toBe("ok")
    expect(await readFile(join(workspace, "src", "generated", "a.txt"), "utf8")).toBe("nested\n")
  })

  test("overwrites existing files with an expected SHA-256", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "before\n")

    const result = await runWrite(workspace, {
      content: "after\n",
      expected_sha256: sha256(Buffer.from("before\n")),
      path: "a.txt",
    })

    expect(result.status).toBe("ok")
    expect(await readFile(path, "utf8")).toBe("after\n")
    expect(result.ui).toMatchObject({
      existed: true,
      firstChangedLine: 1,
      path: "a.txt",
    })
  })

  test("denies overwrite without expected version material", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "before\n")

    const result = await runWrite(workspace, { content: "after\n", path: "a.txt" })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("write_expected_version_required")
    expect(await readFile(path, "utf8")).toBe("before\n")
  })

  test("guards stale expected content", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "fresh\n")

    const result = await runWrite(workspace, {
      content: "after\n",
      expected_content: "stale\n",
      path: "a.txt",
    })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("write_stale_file")
    expect(await readFile(path, "utf8")).toBe("fresh\n")
  })

  test("requires permission for external paths", async () => {
    const workspace = await makeWorkspace()
    const outside = await mkdtemp(join(tmpdir(), "khala-write-outside-"))
    const outsidePath = join(outside, "a.txt")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createWriteTool()]),
        { arguments: { content: "hello\n", path: outsidePath }, id: "call_1", name: "write", sessionId: "s1" },
        makeKhalaToolServices({
          permission: denyAllKhalaPermissionService,
          workingDirectory: workspace,
        }),
      ),
    )

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("write_external_file_denied")
  })

  test("generates a structured diff receipt for overwrites", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "one\ntwo\n")

    const result = await runWrite(workspace, {
      content: "one\nTWO\n",
      expected_content: "one\ntwo\n",
      path: "a.txt",
    })

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("-two")
    expect(result.modelOutput.text).toContain("+TWO")
    expect(result.publicSummary).toContain("1 diff receipt")
    expect(result.publicSummary).not.toContain("TWO")
    expect(result.ui).toMatchObject({
      diff: expect.stringContaining("+TWO"),
      existed: true,
      firstChangedLine: 2,
    })
  })

  test("is unavailable in inspect/read-only sessions", () => {
    const registry = makeKhalaToolRegistry([createWriteTool()])

    expect(registry.materialize("inspect").some(tool => tool.name === "write")).toBe(false)
    expect(registry.materialize("coding").some(tool => tool.name === "write")).toBe(true)
  })
})

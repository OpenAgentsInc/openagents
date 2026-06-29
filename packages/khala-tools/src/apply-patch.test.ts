import { mkdtemp, readFile, stat, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createApplyPatchTool,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaPermissionRequest,
  type KhalaPermissionService,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-apply-patch-tool-"))
}

async function runPatch(
  workspace: string,
  patch: string,
  options: Parameters<typeof createApplyPatchTool>[0] = {},
  permission?: KhalaPermissionService,
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createApplyPatchTool(options)]),
      { arguments: { patch }, id: "call_1", name: "apply_patch", sessionId: "s1" },
      makeKhalaToolServices({
        ...(permission === undefined ? {} : { permission }),
        workingDirectory: workspace,
      }),
    ),
  )
}

async function exists(path: string): Promise<boolean> {
  try {
    await stat(path)
    return true
  } catch {
    return false
  }
}

describe("apply_patch tool", () => {
  test("adds files from the constrained grammar", async () => {
    const workspace = await makeWorkspace()

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Add File: src/a.txt",
      "+hello",
      "+world",
      "*** End Patch",
    ].join("\n"))

    expect(result.status).toBe("ok")
    expect(await readFile(join(workspace, "src", "a.txt"), "utf8")).toBe("hello\nworld\n")
    expect(result.ui).toMatchObject({
      affectedPaths: ["src/a.txt"],
      appliedOperations: 1,
      atomic: false,
      kind: "patch_receipt",
      partialFailure: false,
    })
  })

  test("updates files with exact hunks", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "a.txt"), "one\ntwo\n")

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Update File: a.txt",
      "@@",
      " one",
      "-two",
      "+TWO",
      "*** End Patch",
    ].join("\n"))

    expect(result.status).toBe("ok")
    expect(await readFile(join(workspace, "a.txt"), "utf8")).toBe("one\nTWO\n")
    expect(result.modelOutput.text).toContain("+TWO")
  })

  test("deletes files", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "bye\n")

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Delete File: a.txt",
      "*** End Patch",
    ].join("\n"))

    expect(result.status).toBe("ok")
    expect(await exists(path)).toBe(false)
  })

  test("rejects invalid grammar with no side effects", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "safe\n")

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Update File: a.txt",
      "-safe",
      "+unsafe",
      "*** End Patch",
    ].join("\n"))

    expect(result.status).toBe("failed")
    expect(await readFile(path, "utf8")).toBe("safe\n")
  })

  test("denies path traversal before side effects", async () => {
    const workspace = await makeWorkspace()
    const outside = join(workspace, "..", "khala-patch-outside.txt")

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Add File: ../khala-patch-outside.txt",
      "+nope",
      "*** End Patch",
    ].join("\n"))

    expect(result.status).toBe("failed")
    expect(await exists(outside)).toBe(false)
  })

  test("guards stale content when hunks do not match", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "fresh\n")

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Update File: a.txt",
      "@@",
      "-stale",
      "+after",
      "*** End Patch",
    ].join("\n"))

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("hunk did not match")
    expect(await readFile(path, "utf8")).toBe("fresh\n")
  })

  test("requests one multi-file patch approval with affected resources", async () => {
    const workspace = await makeWorkspace()
    const requests: KhalaPermissionRequest[] = []
    const permission: KhalaPermissionService = {
      decide: request => Effect.sync(() => {
        requests.push(request)
        return "allow" as const
      }),
    }

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Add File: a.txt",
      "+a",
      "*** Add File: b.txt",
      "+b",
      "*** End Patch",
    ].join("\n"), {}, permission)

    expect(result.status).toBe("ok")
    expect(requests).toHaveLength(1)
    expect(requests[0]).toMatchObject({
      action: "patch",
      resources: ["a.txt", "b.txt"],
      toolName: "apply_patch",
    })
  })

  test("reports partial-failure behavior for non-atomic V1 application", async () => {
    const workspace = await makeWorkspace()

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Add File: a.txt",
      "+a",
      "*** Add File: b.txt",
      "+b",
      "*** End Patch",
    ].join("\n"), { failAfterOperations: 1 })

    expect(result.status).toBe("failed")
    expect(await readFile(join(workspace, "a.txt"), "utf8")).toBe("a\n")
    expect(await exists(join(workspace, "b.txt"))).toBe(false)
    expect(result.ui).toMatchObject({
      appliedOperations: 1,
      atomic: false,
      partialFailure: true,
    })
  })

  test("emits private structured diff receipts for packages/ui rendering", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "a.txt"), "before\n")

    const result = await runPatch(workspace, [
      "*** Begin Patch",
      "*** Update File: a.txt",
      "@@",
      "-before",
      "+after",
      "*** End Patch",
    ].join("\n"))

    expect(result.status).toBe("ok")
    expect(result.ui).toMatchObject({
      diff: {
        format: "unified",
        kind: "unified_diff",
        publicSafety: "private",
        rendererRef: "khala.renderer.diff.v1",
        text: expect.stringContaining("+after"),
      },
      publicSafety: "private",
    })
  })
})

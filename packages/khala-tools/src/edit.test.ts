import { createHash } from "node:crypto"
import { mkdtemp, readFile, symlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  createEditTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-edit-tool-"))
}

async function runEdit(
  workspace: string,
  args: Readonly<Record<string, unknown>>,
  tool = createEditTool(),
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([tool]),
      { arguments: args, id: `call_${Math.random()}`, name: "edit", sessionId: "s1" },
      makeKhalaToolServices({ permission: allowAllKhalaPermissionService, workingDirectory: workspace }),
    ),
  )
}

function sha256(value: Buffer): string {
  return createHash("sha256").update(value).digest("hex")
}

describe("edit tool", () => {
  test("applies a unique exact replacement and returns diff metadata", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "hello world\n")

    const result = await runEdit(workspace, {
      edits: [{ new_text: "there", old_text: "world" }],
      path: "a.txt",
    })

    expect(result.status).toBe("ok")
    expect(await readFile(path, "utf8")).toBe("hello there\n")
    expect(result.modelOutput.text).toContain("First changed line: 1")
    expect(result.modelOutput.text).toContain("-hello world")
    expect(result.modelOutput.text).toContain("+hello there")
    expect(result.ui).toMatchObject({
      firstChangedLine: 1,
      kind: "file_edit",
      path: "a.txt",
      replacementCount: 1,
    })
  })

  test("denies duplicate matches unless replace_all is set", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "same\nsame\n")

    const result = await runEdit(workspace, {
      edits: [{ new_text: "changed", old_text: "same" }],
      path: "a.txt",
    })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("matched more than once")
    expect(await readFile(path, "utf8")).toBe("same\nsame\n")
  })

  test("replace_all edits every exact occurrence", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "same\nsame\n")

    const result = await runEdit(workspace, {
      edits: [{ new_text: "changed", old_text: "same", replace_all: true }],
      path: "a.txt",
    })

    expect(result.status).toBe("ok")
    expect(await readFile(path, "utf8")).toBe("changed\nchanged\n")
    expect(result.ui).toMatchObject({ replacementCount: 2 })
  })

  test("fails on no match without fuzzy replacement", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "hello\n")

    const result = await runEdit(workspace, {
      edits: [{ new_text: "goodbye", old_text: "hullo" }],
      path: "a.txt",
    })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("old_text did not match")
    expect(await readFile(path, "utf8")).toBe("hello\n")
  })

  test("guards stale file versions with expected_sha256", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "fresh\n")

    const result = await runEdit(workspace, {
      edits: [{ new_text: "changed", old_text: "fresh" }],
      expected_sha256: sha256(Buffer.from("stale\n")),
      path: "a.txt",
    })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("edit_stale_file")
    expect(await readFile(path, "utf8")).toBe("fresh\n")
  })

  test("normalizes CRLF for matching and restores CRLF on write", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "line1\r\nline2\r\n")

    const result = await runEdit(workspace, {
      edits: [{ new_text: "alpha\nbeta", old_text: "line1\nline2" }],
      path: "a.txt",
    })

    expect(result.status).toBe("ok")
    expect(await readFile(path, "utf8")).toBe("alpha\r\nbeta\r\n")
  })

  test("preserves UTF-8 BOM", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, Buffer.concat([Buffer.from([0xef, 0xbb, 0xbf]), Buffer.from("hello\n")]))

    const result = await runEdit(workspace, {
      edits: [{ new_text: "hi", old_text: "hello" }],
      path: "a.txt",
    })

    const bytes = await readFile(path)
    expect(result.status).toBe("ok")
    expect([...bytes.subarray(0, 3)]).toEqual([0xef, 0xbb, 0xbf])
    expect(bytes.subarray(3).toString("utf8")).toBe("hi\n")
  })

  test("serializes concurrent same-file edits through one tool instance", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    const tool = createEditTool()
    await writeFile(path, "one two\nthree four\n")

    const [first, second] = await Promise.all([
      runEdit(workspace, { edits: [{ new_text: "ONE", old_text: "one" }], path: "a.txt" }, tool),
      runEdit(workspace, { edits: [{ new_text: "THREE", old_text: "three" }], path: "a.txt" }, tool),
    ])

    expect(first.status).toBe("ok")
    expect(second.status).toBe("ok")
    expect(await readFile(path, "utf8")).toBe("ONE two\nTHREE four\n")
  })

  test("denies write when approval service denies edit authority", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "hello\n")

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createEditTool()]),
        {
          arguments: { edits: [{ new_text: "hi", old_text: "hello" }], path: "a.txt" },
          id: "call_1",
          name: "edit",
          sessionId: "s1",
        },
        makeKhalaToolServices({
          permission: denyAllKhalaPermissionService,
          workingDirectory: workspace,
        }),
      ),
    )

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("edit_write_denied")
    expect(await readFile(path, "utf8")).toBe("hello\n")
  })

  test("refuses whole-file replacement", async () => {
    const workspace = await makeWorkspace()
    const path = join(workspace, "a.txt")
    await writeFile(path, "whole\nfile\n")

    const result = await runEdit(workspace, {
      edits: [{ new_text: "replacement\n", old_text: "whole\nfile\n" }],
      path: "a.txt",
    })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("refuses whole-file replacement")
    expect(await readFile(path, "utf8")).toBe("whole\nfile\n")
  })

  test("requires permission for symlink file escapes", async () => {
    const workspace = await makeWorkspace()
    const outside = await mkdtemp(join(tmpdir(), "khala-edit-outside-"))
    await writeFile(join(outside, "outside.txt"), "hello\n")
    await symlink(join(outside, "outside.txt"), join(workspace, "outside-link.txt"))

    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createEditTool()]),
        {
          arguments: { edits: [{ new_text: "hi", old_text: "hello" }], path: "outside-link.txt" },
          id: "call_1",
          name: "edit",
          sessionId: "s1",
        },
        makeKhalaToolServices({
          permission: denyAllKhalaPermissionService,
          workingDirectory: workspace,
        }),
      ),
    )

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("edit_external_file_denied")
  })
})

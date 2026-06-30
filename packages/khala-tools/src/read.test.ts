import { mkdtemp, symlink, writeFile } from "node:fs/promises"
import { join } from "node:path"
import { tmpdir } from "node:os"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  allowAllKhalaPermissionService,
  createReadTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
} from "./index.js"

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-read-tool-"))
}

describe("read tool", () => {
  test("reads a bounded line range with line numbers", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "notes.txt"), "one\ntwo\nthree\nfour\n")
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createReadTool()]),
        { arguments: { limit: 2, offset: 2, path: "notes.txt" }, id: "call_1", name: "read", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toBe("2: two\n3: three\n[read truncated; continue with offset 4]")
    expect(result.ui).toMatchObject({
      displayPath: "notes.txt",
      lineEnd: 3,
      lineStart: 2,
      totalLines: 5,
      truncated: true,
    })
  })

  test("fails missing files cleanly", async () => {
    const workspace = await makeWorkspace()
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createReadTool()]),
        { arguments: { path: "missing.txt" }, id: "call_1", name: "read", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("read_failed")
  })

  test("returns a view_image hint for image-like paths", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "screen.png"), "not really an image")
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createReadTool()]),
        { arguments: { path: "screen.png" }, id: "call_1", name: "read", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("Use view_image")
    expect(result.ui).toMatchObject({ kind: "image_hint" })
  })

  test("blocks credential-shaped paths before reading", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, ".env"), "OPENROUTER_API_KEY=sk-or-secret")
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createReadTool()]),
        { arguments: { path: ".env" }, id: "call_1", name: "read", sessionId: "s1" },
        makeKhalaToolServices({ workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("read_blocked_credential_path")
    expect(result.publicSummary).not.toContain("sk-or-secret")
  })

  test("requires permission for symlink workspace escapes", async () => {
    const workspace = await makeWorkspace()
    const outside = await mkdtemp(join(tmpdir(), "khala-read-outside-"))
    await writeFile(join(outside, "secret.txt"), "outside")
    await symlink(join(outside, "secret.txt"), join(workspace, "link.txt"))
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createReadTool()]),
        { arguments: { path: "link.txt" }, id: "call_1", name: "read", sessionId: "s1" },
        makeKhalaToolServices({
          permission: denyAllKhalaPermissionService,
          workingDirectory: workspace,
        }),
      ),
    )

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("read_external_directory_denied")
  })

  test("blocks device files even when external reads are approved", async () => {
    const workspace = await makeWorkspace()
    const result = await Effect.runPromise(
      executeKhalaTool(
        makeKhalaToolRegistry([createReadTool()]),
        { arguments: { path: "/dev/null" }, id: "call_1", name: "read", sessionId: "s1" },
        makeKhalaToolServices({ permission: allowAllKhalaPermissionService, workingDirectory: workspace }),
      ),
    )

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("read_blocked_file_type")
  })
})

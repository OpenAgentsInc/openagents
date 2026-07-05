import { afterEach, describe, expect, test } from "bun:test"
import { mkdir, mkdtemp, realpath, rm, symlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"

import { createKhalaCodeEditorFileService } from "../src/bun/editor-file-service"
import { createKhalaCodeDesktopRpcRequestHandlers } from "../src/bun/rpc-handlers"
import {
  decodeKhalaCodeDesktopRpcParameters,
  decodeKhalaCodeDesktopRpcResult,
} from "../src/shared/rpc"

const tempDirs: string[] = []

afterEach(async () => {
  await Promise.all(tempDirs.splice(0).map(dir => rm(dir, { force: true, recursive: true })))
})

const tempRoot = async (): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), "khala-code-editor-files-"))
  tempDirs.push(root)
  return root
}

describe("Khala Code editor local workspace file service", () => {
  test("lists the provider, workspace root, directory entries, and text file through typed RPCs", async () => {
    const root = await tempRoot()
    await mkdir(join(root, "src"))
    await writeFile(join(root, "src", "index.ts"), "export const answer = 42\n")
    await writeFile(join(root, "README.md"), "# Test\n")
    const canonicalRoot = await realpath(root)

    const handlers = createKhalaCodeDesktopRpcRequestHandlers({
      appleFmReadiness: () => {
        throw new Error("apple fm readiness should not be called")
      },
      env: {},
      onDeviceDeciderStatus: () => {
        throw new Error("on-device decider status should not be called")
      },
      workingDirectory: root,
    })

    const providers = await handlers.editorProviderList()
    expect(decodeKhalaCodeDesktopRpcResult("editorProviderList", providers)).toMatchObject({
      ok: true,
      providers: [{
        kind: "local_workspace",
        providerId: "local-workspace",
        rootPath: canonicalRoot,
      }],
    })

    const workspace = await handlers.editorWorkspaceRead()
    expect(decodeKhalaCodeDesktopRpcResult("editorWorkspaceRead", workspace)).toMatchObject({
      ok: true,
      roots: [{ path: canonicalRoot, providerId: "local-workspace" }],
    })

    const directoryRequest = decodeKhalaCodeDesktopRpcParameters("editorDirectoryRead", [
      { path: "." },
    ])[0]
    const directory = await handlers.editorDirectoryRead(directoryRequest as never)
    expect(decodeKhalaCodeDesktopRpcResult("editorDirectoryRead", directory)).toMatchObject({
      ok: true,
      entries: [
        { kind: "directory", name: "src" },
        { kind: "file", name: "README.md" },
      ],
      truncated: false,
    })

    const file = await handlers.editorFileRead({ path: "src/index.ts" })
    expect(decodeKhalaCodeDesktopRpcResult("editorFileRead", file)).toMatchObject({
      content: "export const answer = 42\n",
      encoding: "utf8",
      ok: true,
      path: join(canonicalRoot, "src", "index.ts"),
      providerId: "local-workspace",
    })
  })

  test("rejects traversal and absolute paths outside the workspace", async () => {
    const root = await tempRoot()
    const outsideRoot = await tempRoot()
    await writeFile(join(outsideRoot, "outside.ts"), "export const outside = true\n")
    const service = createKhalaCodeEditorFileService({ workingDirectory: root })

    await expect(service.fileRead({ path: "../outside.ts" })).resolves.toMatchObject({
      error: { code: "outside_workspace" },
      ok: false,
    })
    await expect(service.fileRead({ path: join(outsideRoot, "outside.ts") })).resolves.toMatchObject({
      error: { code: "outside_workspace" },
      ok: false,
    })
  })

  test("rejects symlink escape targets before rendering", async () => {
    const root = await tempRoot()
    const outsideRoot = await tempRoot()
    await writeFile(join(outsideRoot, "secret.ts"), "export const secret = true\n")
    await symlink(join(outsideRoot, "secret.ts"), join(root, "secret-link.ts"))

    const service = createKhalaCodeEditorFileService({ workingDirectory: root })
    await expect(service.fileRead({ path: "secret-link.ts" })).resolves.toMatchObject({
      error: { code: "outside_workspace" },
      ok: false,
    })
  })

  test("returns typed errors for missing, binary, and oversized files", async () => {
    const root = await tempRoot()
    await writeFile(join(root, "binary.bin"), Buffer.from([0x01, 0x00, 0x02, 0x03]))
    await writeFile(join(root, "large.ts"), "0123456789")
    const service = createKhalaCodeEditorFileService({ workingDirectory: root })

    await expect(service.fileRead({ path: "missing.ts" })).resolves.toMatchObject({
      error: { code: "not_found" },
      ok: false,
    })
    await expect(service.fileRead({ path: "binary.bin" })).resolves.toMatchObject({
      error: { code: "binary_file" },
      ok: false,
    })
    await expect(service.fileRead({ maxBytes: 4, path: "large.ts" })).resolves.toMatchObject({
      error: { code: "file_too_large" },
      ok: false,
    })
  })

  test("rejects unavailable provider refs without falling back to legacy Codex FS methods", async () => {
    const root = await tempRoot()
    await writeFile(join(root, "index.ts"), "export {}\n")
    const service = createKhalaCodeEditorFileService({ workingDirectory: root })

    await expect(service.directoryRead({ path: ".", providerId: "codex-app-server" })).resolves.toMatchObject({
      error: {
        code: "provider_unavailable",
        providerId: "codex-app-server",
      },
      ok: false,
    })
    await expect(service.fileRead({ path: "index.ts", providerId: "codex-app-server" })).resolves.toMatchObject({
      error: {
        code: "provider_unavailable",
        providerId: "codex-app-server",
      },
      ok: false,
    })
  })
})

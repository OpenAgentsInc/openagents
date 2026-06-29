import { mkdtemp, symlink, writeFile } from "node:fs/promises"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  createViewImageTool,
  denyAllKhalaPermissionService,
  executeKhalaTool,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaPermissionService,
} from "./index.js"

type ImageUi = Readonly<{
  artifactRef: string
  displayPath: string
  height: number
  mediaType: string
  modelContentParts: ReadonlyArray<unknown>
  redaction: Readonly<{
    classification: string
    publicSafe: boolean
  }>
  visionSupported: boolean
  width: number
}>

async function makeWorkspace(): Promise<string> {
  return mkdtemp(join(tmpdir(), "khala-view-image-tool-"))
}

async function runView(
  workspace: string,
  args: Readonly<Record<string, unknown>>,
  options: Readonly<{ maxBytes?: number; visionSupported?: boolean }> = {},
  permission?: KhalaPermissionService,
) {
  return Effect.runPromise(
    executeKhalaTool(
      makeKhalaToolRegistry([createViewImageTool(options)]),
      { arguments: args, id: "call_1", name: "view_image", sessionId: "s1" },
      makeKhalaToolServices({
        ...(permission === undefined ? {} : { permission }),
        workingDirectory: workspace,
      }),
    ),
  )
}

function uiOf(result: Awaited<ReturnType<typeof runView>>): ImageUi {
  return result.ui as ImageUi
}

describe("view_image tool", () => {
  test("supports png, jpeg, webp, and gif images with private artifacts", async () => {
    const workspace = await makeWorkspace()
    const fixtures = [
      { bytes: pngFixture(2, 3), file: "sample.png", height: 3, mediaType: "image/png", width: 2 },
      { bytes: jpegFixture(4, 5), file: "sample.jpg", height: 5, mediaType: "image/jpeg", width: 4 },
      { bytes: webpFixture(6, 7), file: "sample.webp", height: 7, mediaType: "image/webp", width: 6 },
      { bytes: gifFixture(8, 9), file: "sample.gif", height: 9, mediaType: "image/gif", width: 8 },
    ]

    for (const fixture of fixtures) {
      await writeFile(join(workspace, fixture.file), fixture.bytes)
      const result = await runView(workspace, { path: fixture.file }, { visionSupported: true })
      const ui = uiOf(result)

      expect(result.status).toBe("ok")
      expect(result.artifacts).toHaveLength(1)
      expect(result.privateDataRefs).toEqual([result.artifacts[0]?.artifactRef])
      expect(result.publicSummary).not.toContain(fixture.bytes.toString("base64"))
      expect(ui).toMatchObject({
        displayPath: fixture.file,
        height: fixture.height,
        mediaType: fixture.mediaType,
        redaction: {
          classification: "private_image_bytes",
          publicSafe: false,
        },
        visionSupported: true,
        width: fixture.width,
      })
      expect(ui.modelContentParts).toEqual([
        expect.objectContaining({ mediaType: fixture.mediaType, type: "image" }),
      ])
    }
  })

  test("rejects unsupported files and missing files with typed failures", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "notes.txt"), "hello")

    const unsupported = await runView(workspace, { path: "notes.txt" })
    const missing = await runView(workspace, { path: "missing.png" })

    expect(unsupported.status).toBe("failed")
    expect(unsupported.publicSummary).toContain("view_image_unsupported_format")
    expect(missing.status).toBe("failed")
    expect(missing.publicSummary).toContain("view_image_failed")
  })

  test("requires permission for symlink workspace escapes", async () => {
    const workspace = await makeWorkspace()
    const outside = await mkdtemp(join(tmpdir(), "khala-view-image-outside-"))
    await writeFile(join(outside, "screen.png"), pngFixture(1, 1))
    await symlink(join(outside, "screen.png"), join(workspace, "link.png"))

    const result = await runView(workspace, { path: "link.png" }, {}, denyAllKhalaPermissionService)

    expect(result.status).toBe("denied")
    expect(result.publicSummary).toContain("view_image_external_directory_denied")
  })

  test("rejects oversized images before writing artifacts", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "large.png"), Buffer.concat([pngFixture(1, 1), Buffer.alloc(64)]))

    const result = await runView(workspace, { path: "large.png" }, { maxBytes: 16 })

    expect(result.status).toBe("failed")
    expect(result.publicSummary).toContain("view_image_oversized")
    expect(result.artifacts).toHaveLength(0)
  })

  test("falls back cleanly when the backend does not support vision content parts", async () => {
    const workspace = await makeWorkspace()
    await writeFile(join(workspace, "screen.png"), pngFixture(10, 11))

    const result = await runView(workspace, { path: "screen.png" }, { visionSupported: false })
    const ui = uiOf(result)

    expect(result.status).toBe("ok")
    expect(result.modelOutput.text).toContain("does not support vision")
    expect(ui.visionSupported).toBe(false)
    expect(ui.modelContentParts).toEqual([])
    expect(ui.artifactRef).toBe(result.artifacts[0]?.artifactRef)
  })

  test("is available in the inspect preset with read authority", () => {
    const registry = makeKhalaToolRegistry([createViewImageTool()])
    const [definition] = registry.materialize("inspect")

    expect(definition).toMatchObject({
      authority: "read",
      name: "view_image",
      permissionMode: "allow",
    })
  })
})

function pngFixture(width: number, height: number): Buffer {
  const bytes = Buffer.alloc(24)
  Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]).copy(bytes, 0)
  bytes.writeUInt32BE(13, 8)
  bytes.write("IHDR", 12, "ascii")
  bytes.writeUInt32BE(width, 16)
  bytes.writeUInt32BE(height, 20)
  return bytes
}

function gifFixture(width: number, height: number): Buffer {
  const bytes = Buffer.alloc(10)
  bytes.write("GIF89a", 0, "ascii")
  bytes.writeUInt16LE(width, 6)
  bytes.writeUInt16LE(height, 8)
  return bytes
}

function jpegFixture(width: number, height: number): Buffer {
  return Buffer.from([
    0xff,
    0xd8,
    0xff,
    0xc0,
    0x00,
    0x11,
    0x08,
    height >> 8,
    height & 0xff,
    width >> 8,
    width & 0xff,
    0x03,
    0x01,
    0x11,
    0x00,
    0x02,
    0x11,
    0x00,
    0x03,
    0x11,
    0x00,
    0xff,
    0xd9,
  ])
}

function webpFixture(width: number, height: number): Buffer {
  const bytes = Buffer.alloc(30)
  bytes.write("RIFF", 0, "ascii")
  bytes.writeUInt32LE(22, 4)
  bytes.write("WEBP", 8, "ascii")
  bytes.write("VP8X", 12, "ascii")
  bytes.writeUInt32LE(10, 16)
  writeUInt24LE(bytes, 24, width - 1)
  writeUInt24LE(bytes, 27, height - 1)
  return bytes
}

function writeUInt24LE(bytes: Buffer, offset: number, value: number): void {
  bytes[offset] = value & 0xff
  bytes[offset + 1] = (value >> 8) & 0xff
  bytes[offset + 2] = (value >> 16) & 0xff
}

import { describe, expect, test } from "bun:test"
import {
  classifyByExtension,
  extOf,
  ingestAssets,
  type AssetSource,
  type RawAsset,
} from "../src/bun/asset-ingestion.ts"

// A fully in-memory fake source — no real FS / MCP is touched at test time.
function fakeSource(input: {
  list: readonly RawAsset[]
  contents?: Record<string, Uint8Array>
  failRead?: Record<string, string>
  failList?: string
}): AssetSource {
  return {
    listAssets: async () => {
      if (input.failList) throw new Error(input.failList)
      return input.list
    },
    readAsset: async (path: string) => {
      const fail = input.failRead?.[path]
      if (fail) throw new Error(fail)
      return input.contents?.[path] ?? new Uint8Array(0)
    },
  }
}

describe("#4995 ingestAssets", () => {
  test("enumerates raw assets into typed redaction-safe descriptors", async () => {
    const source = fakeSource({
      list: [
        { path: "brand/logo.png", bytes: 2048, modifiedAt: "2026-06-14T00:00:00Z" },
        { path: "brand/promo.mp4" },
        { path: "brand/style-guide.pdf", contentType: "application/pdf" },
        { path: "brand/Inter.woff2" },
        { path: "brand/notes" },
      ],
      contents: { "brand/promo.mp4": new Uint8Array(10) },
    })

    const { descriptors, listError } = await ingestAssets({ source })

    expect(listError).toBeNull()
    expect(descriptors.map((d) => [d.path, d.kind, d.ext])).toEqual([
      ["brand/logo.png", "image", "png"],
      ["brand/promo.mp4", "video", "mp4"],
      ["brand/style-guide.pdf", "document", "pdf"],
      ["brand/Inter.woff2", "font", "woff2"],
      ["brand/notes", "other", ""],
    ])
    // listing size preferred; read length used when listing omitted size.
    expect(descriptors[0].bytes).toBe(2048)
    expect(descriptors[1].bytes).toBe(10)
    expect(descriptors[2].contentType).toBe("application/pdf")
    expect(descriptors[0].modifiedAt).toBe("2026-06-14T00:00:00Z")
    expect(descriptors.every((d) => d.readable)).toBe(true)
    expect(descriptors.every((d) => d.error === null)).toBe(true)
  })

  test("descriptors carry only redaction-safe metadata — no asset bytes/secrets", async () => {
    const secret = "SECRET-BRAND-BYTES"
    const source = fakeSource({
      list: [{ path: "brand/confidential.png" }],
      contents: { "brand/confidential.png": new TextEncoder().encode(secret) },
    })

    const { descriptors } = await ingestAssets({ source })

    const json = JSON.stringify(descriptors)
    expect(json).not.toContain(secret)
    // The descriptor shape is exactly the known safe keys.
    expect(Object.keys(descriptors[0]).sort()).toEqual(
      ["bytes", "contentType", "error", "ext", "kind", "modifiedAt", "path", "readable"],
    )
    // Read size is recorded, but no `bytes`/`data` blob field exists.
    expect(descriptors[0].bytes).toBe(secret.length)
    expect((descriptors[0] as Record<string, unknown>).data).toBeUndefined()
  })

  test("an empty source yields an empty descriptor set, no error", async () => {
    const { descriptors, listError } = await ingestAssets({ source: fakeSource({ list: [] }) })
    expect(descriptors).toEqual([])
    expect(listError).toBeNull()
  })

  test("a per-asset read error is surfaced on that descriptor without aborting", async () => {
    const source = fakeSource({
      list: [{ path: "brand/ok.png" }, { path: "brand/broken.png" }, { path: "brand/also-ok.svg" }],
      failRead: { "brand/broken.png": "EACCES: permission denied" },
    })

    const { descriptors, listError } = await ingestAssets({ source })

    expect(listError).toBeNull()
    expect(descriptors).toHaveLength(3)
    const broken = descriptors.find((d) => d.path === "brand/broken.png")!
    expect(broken.readable).toBe(false)
    expect(broken.error).toBe("EACCES: permission denied")
    // Other assets still processed cleanly.
    expect(descriptors.filter((d) => d.readable).map((d) => d.path)).toEqual([
      "brand/ok.png",
      "brand/also-ok.svg",
    ])
  })

  test("a listAssets rejection becomes a run-level listError, no throw", async () => {
    const { descriptors, listError } = await ingestAssets({
      source: fakeSource({ list: [], failList: "MCP server unavailable" }),
    })
    expect(descriptors).toEqual([])
    expect(listError).toBe("MCP server unavailable")
  })

  test("an injected classifier overrides the default extension classifier", async () => {
    const source = fakeSource({ list: [{ path: "brand/anything.bin" }] })
    const { descriptors } = await ingestAssets({
      source,
      classify: () => "image",
    })
    expect(descriptors[0].kind).toBe("image")
  })

  test("extOf and classifyByExtension behave deterministically", () => {
    expect(extOf("a/b/c.PNG")).toBe("png")
    expect(extOf("a/b/c")).toBe("")
    expect(extOf(".gitignore")).toBe("")
    expect(extOf("dir.with.dots/file")).toBe("")
    expect(classifyByExtension({ path: "x.mov" })).toBe("video")
    expect(classifyByExtension({ path: "x.unknown" })).toBe("other")
  })
})

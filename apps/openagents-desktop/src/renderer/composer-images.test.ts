/**
 * Composer image attachment unit oracle (capability I1).
 *
 * Proves the pure classify/decode/state pieces the drop, paste, and picker
 * paths all share: media-type + size gating with honest rejection copy, base64
 * decoding of an in-renderer File/Blob (the drop/paste path — never a disk
 * read), bounded add/remove, the thumbnail data URL, and the boundary
 * projection.
 */
import { describe, expect, test } from "vite-plus/test"
import {
  COMPOSER_IMAGE_BYTES_LIMIT,
  COMPOSER_IMAGE_COUNT_LIMIT,
  addComposerImage,
  bytesToBase64,
  canAttachMoreImages,
  classifyImageFile,
  composerImageDataUrl,
  composerImageRejectionMessage,
  formatImageSize,
  readImageFile,
  removeComposerImage,
  toStartImages,
  type ComposerImageAttachment,
} from "./composer-images.ts"

const attachment = (id: string): ComposerImageAttachment => ({
  id,
  mediaType: "image/png",
  data: "aGVsbG8=",
  name: `${id}.png`,
  sizeBytes: 5,
})

const fileLike = (type: string, bytes: Uint8Array, name = "x.png") => ({
  type,
  size: bytes.length,
  name,
  arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) as ArrayBuffer,
})

describe("classifyImageFile (capability I1)", () => {
  test("accepts each supported media type", () => {
    for (const type of ["image/png", "image/jpeg", "image/webp", "image/gif"]) {
      const result = classifyImageFile({ type, size: 1024 }, 0)
      expect(result.ok).toBe(true)
    }
  })

  test("rejects an unsupported type with honest copy", () => {
    const result = classifyImageFile({ type: "image/svg+xml", size: 1024 }, 0)
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("expected rejection")
    expect(result.reason).toBe("wrong_type")
    expect(composerImageRejectionMessage(result.reason)).toContain("supported image")
  })

  test("rejects an oversize file honestly (>10 MB)", () => {
    const result = classifyImageFile({ type: "image/png", size: COMPOSER_IMAGE_BYTES_LIMIT + 1 }, 0)
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("expected rejection")
    expect(result.reason).toBe("too_large")
    expect(composerImageRejectionMessage(result.reason)).toContain("10 MB")
  })

  test("rejects once the count limit is reached", () => {
    const result = classifyImageFile({ type: "image/png", size: 10 }, COMPOSER_IMAGE_COUNT_LIMIT)
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("expected rejection")
    expect(result.reason).toBe("count_limit")
    expect(composerImageRejectionMessage(result.reason)).toContain(String(COMPOSER_IMAGE_COUNT_LIMIT))
  })
})

describe("readImageFile (drop/paste decode, capability I1)", () => {
  test("decodes a valid PNG File to a bounded base64 attachment", async () => {
    const bytes = new TextEncoder().encode("hello")
    const result = await readImageFile(fileLike("image/png", bytes, "shot.png"), 0, () => "id-1")
    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected ok")
    expect(result.attachment.mediaType).toBe("image/png")
    expect(result.attachment.data).toBe(bytesToBase64(bytes))
    expect(result.attachment.name).toBe("shot.png")
    expect(result.attachment.sizeBytes).toBe(5)
    // Round-trips: base64 -> data URL is a valid image source.
    expect(composerImageDataUrl(result.attachment)).toBe("data:image/png;base64,aGVsbG8=")
  })

  test("rejects a wrong-type File before decoding", async () => {
    const result = await readImageFile(fileLike("text/plain", new TextEncoder().encode("nope")), 0)
    expect(result.ok).toBe(false)
    if (result.ok) throw new Error("expected rejection")
    expect(result.reason).toBe("wrong_type")
  })

  test("falls back to a bounded default name when the File has none", async () => {
    const result = await readImageFile(fileLike("image/jpeg", new Uint8Array([1, 2, 3]), ""), 0, () => "id-2")
    expect(result.ok).toBe(true)
    if (!result.ok) throw new Error("expected ok")
    expect(result.attachment.name).toBe("image.jpg")
  })
})

describe("composer image state helpers (capability I1)", () => {
  test("add is bounded by the count limit and dedupes by id", () => {
    let list: ReadonlyArray<ComposerImageAttachment> = []
    for (let i = 0; i < COMPOSER_IMAGE_COUNT_LIMIT + 2; i += 1) {
      list = addComposerImage(list, attachment(`a${i}`))
    }
    expect(list).toHaveLength(COMPOSER_IMAGE_COUNT_LIMIT)
    expect(canAttachMoreImages(list)).toBe(false)
    // Re-adding an existing id is a no-op.
    const same = addComposerImage([attachment("dup")], attachment("dup"))
    expect(same).toHaveLength(1)
  })

  test("remove drops exactly the matching id", () => {
    const list = [attachment("keep"), attachment("drop")]
    expect(removeComposerImage(list, "drop").map(item => item.id)).toEqual(["keep"])
  })

  test("toStartImages projects to the frozen boundary shape (no renderer-only fields)", () => {
    expect(toStartImages([attachment("x")])).toEqual([
      { mediaType: "image/png", data: "aGVsbG8=", name: "x.png" },
    ])
  })

  test("formatImageSize renders KB and MB", () => {
    expect(formatImageSize(2048)).toBe("2 KB")
    expect(formatImageSize(3 * 1024 * 1024)).toBe("3.0 MB")
  })
})

import { describe, expect, test } from "vite-plus/test"

import { openMobileCodingAttachmentPicker } from "../src/coding/mobile-coding-attachment-picker"

const digest = "ab".repeat(32)

describe("contract openagents_mobile.coding.native_attachment_picker.v1", () => {
  test("prepares file and image bytes without exposing a native URI", async () => {
    const persisted: string[] = []
    const picker = openMobileCodingAttachmentPicker({
      pickFiles: async () => ({
        canceled: false,
        files: [{
          name: "screen.png",
          mime: "image/png",
          sizeBytes: 3,
          bytes: async () => new Uint8Array([1, 2, 3]),
          persist: async value => { persisted.push(value) },
        }],
      }),
      sha256: async () => digest.toUpperCase(),
    })

    const result = await picker.pick()
    expect(result).toEqual({
      status: "selected",
      files: [{
        name: "screen.png",
        mime: "image/png",
        sizeBytes: 3,
        digest,
      }],
    })
    expect(persisted).toEqual([digest])
    expect(JSON.stringify(result)).not.toContain("file://")
  })

  test("bounds count and bytes before persistent storage", async () => {
    let reads = 0
    let writes = 0
    const file = {
      name: "large.mov",
      mime: "video/quicktime",
      sizeBytes: 25 * 1024 * 1024 + 1,
      bytes: async () => { reads += 1; return new Uint8Array() },
      persist: async () => { writes += 1 },
    }
    const picker = openMobileCodingAttachmentPicker({
      pickFiles: async () => ({ canceled: false, files: [file] }),
      sha256: async () => digest,
    })
    expect(await picker.pick()).toEqual({
      status: "failed",
      error: "Each file or image must be 25 MB or smaller.",
    })
    expect({ reads, writes }).toEqual({ reads: 0, writes: 0 })
  })

  test("deduplicates content and treats system cancellation as inert", async () => {
    let writes = 0
    const picked = {
      name: "same.txt",
      mime: "text/plain",
      sizeBytes: 1,
      bytes: async () => new Uint8Array([7]),
      persist: async () => { writes += 1 },
    }
    const picker = openMobileCodingAttachmentPicker({
      pickFiles: async () => ({ canceled: false, files: [picked, picked] }),
      sha256: async () => digest,
    })
    expect((await picker.pick()).status).toBe("selected")
    expect(writes).toBe(1)

    const cancelled = openMobileCodingAttachmentPicker({
      pickFiles: async () => ({ canceled: true, files: [] }),
      sha256: async () => digest,
    })
    expect(await cancelled.pick()).toEqual({ status: "cancelled" })
  })
})

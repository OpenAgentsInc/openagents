import { describe, expect, test } from "bun:test"
import {
  composerAttachmentId,
  type CodingComposerDraftSnapshot,
} from "@openagentsinc/khala-sync-client"

import { prepareMobileCodingAttachmentDelivery } from "../src/coding/mobile-coding-attachment-delivery"

type Attachment = CodingComposerDraftSnapshot["doc"]["attachments"][number]

const digest = "a".repeat(64)
const pngBase64 = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII="
const pngBytes = Uint8Array.from(Buffer.from(pngBase64, "base64"))
const pngDigest = "b".repeat(64)
const attachment = (overrides: Partial<Attachment> = {}): Attachment => ({
  id: composerAttachmentId("attachment.mobile.notes"),
  kind: "text",
  name: "notes.txt",
  mime: "text/plain",
  sizeBytes: 5,
  status: "ready",
  digest,
  contentRef: `attachment.native-local.sha256.${digest}.notes.txt`,
  ...overrides,
})

describe("contract openagents_mobile.coding.attachment_delivery_truth.v1", () => {
  test("verifies bytes and lowers text into the authoritative untrusted-data message", async () => {
    const result = await prepareMobileCodingAttachmentDelivery({
      message: "Summarize this",
      attachments: [attachment()],
      port: {
        read: async () => new TextEncoder().encode("hello"),
        sha256: async () => digest,
      },
    })
    expect(result.ok).toBe(true)
    if (!result.ok) return
    expect(result.body).toContain("BEGIN OPENAGENTS UNTRUSTED ATTACHMENT")
    expect(result.body).toContain("hello")
    expect(result.body).toContain("User request: Summarize this")
    expect(result.body).not.toContain("attachment.native-local")
  })

  test("fails closed for changed bytes and carries verified image bytes", async () => {
    const port = {
      read: async () => new TextEncoder().encode("hello"),
      sha256: async () => "b".repeat(64),
    }
    expect(await prepareMobileCodingAttachmentDelivery({
      message: "read",
      attachments: [attachment()],
      port,
    })).toEqual({
      ok: false,
      error: "An attachment changed after it was selected. Remove it and attach it again.",
    })
    const image = await prepareMobileCodingAttachmentDelivery({
      message: "look",
      attachments: [attachment({
        kind: "image",
        mime: "image/png",
        sizeBytes: pngBytes.byteLength,
        digest: pngDigest,
      })],
      port: { read: async () => pngBytes, sha256: async () => pngDigest },
    })
    expect(image.ok).toBe(true)
    if (image.ok) {
      expect(image.attachments?.[0]).toMatchObject({
        mediaType: "image/png",
        sha256: pngDigest,
        sizeBytes: pngBytes.byteLength,
        dataBase64: pngBase64,
      })
    }
  })
})

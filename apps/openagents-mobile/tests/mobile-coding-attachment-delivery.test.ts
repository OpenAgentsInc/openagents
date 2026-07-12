import { describe, expect, test } from "bun:test"
import {
  composerAttachmentId,
  type CodingComposerDraftSnapshot,
} from "@openagentsinc/khala-sync-client"

import { prepareMobileCodingAttachmentDelivery } from "../src/coding/mobile-coding-attachment-delivery"

type Attachment = CodingComposerDraftSnapshot["doc"]["attachments"][number]

const digest = "a".repeat(64)
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

  test("fails closed for changed bytes and image payloads", async () => {
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
    expect((await prepareMobileCodingAttachmentDelivery({
      message: "look",
      attachments: [attachment({ kind: "image", mime: "image/png" })],
      port,
    })).ok).toBe(false)
  })
})

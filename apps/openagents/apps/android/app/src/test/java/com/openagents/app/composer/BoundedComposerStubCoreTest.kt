package com.openagents.app.composer

import kotlin.test.Test
import kotlin.test.assertFalse
import kotlin.test.assertTrue

class BoundedComposerStubCoreTest {
    @Test
    fun attachmentSubmissionRetainsFailedPayloadForRetry() {
        val stamp = ComposerEditorStamp("composer", 1, "input", "token", "1", 0)
        val frame = ComposerFrame(
            stamp = stamp,
            initialDraft = "question",
            maxBytes = 64 * 1024,
            placeholder = "Message",
            enabled = true,
            busy = false,
            primaryLabel = "Send",
        )
        val core = BoundedComposerStubCore(frame).apply { acceptNextSubmit = false }
        val payload = ComposerAttachmentPayload("photo.jpg", "image/jpeg", byteArrayOf(1, 2, 3))

        assertFalse(core.submit(stamp, null, "question", listOf(payload)))
        assertTrue(core.submit(stamp, null, "question", listOf(payload)))
        assertTrue(core.events.contains("submit_failed"))
        assertTrue(core.events.contains("submit"))
    }

    @Test
    fun attachmentSubmissionRejectsOversizedPayloads() {
        val stamp = ComposerEditorStamp("composer", 1, "input", "token", "1", 0)
        val frame = ComposerFrame(
            stamp = stamp,
            initialDraft = "",
            maxBytes = 64 * 1024,
            placeholder = "Message",
            enabled = true,
            busy = false,
            primaryLabel = "Send",
        )
        val core = BoundedComposerStubCore(frame)
        val oversized = ComposerAttachmentPayload(
            "large.bin",
            "application/octet-stream",
            ByteArray(24 * 1024 * 1024 + 1),
        )

        assertFalse(core.submit(stamp, null, "", listOf(oversized)))
        assertTrue(core.events.isEmpty())
    }
}

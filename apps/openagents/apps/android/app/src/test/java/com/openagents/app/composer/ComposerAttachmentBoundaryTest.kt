package com.openagents.app.composer

import org.json.JSONObject
import java.io.ByteArrayInputStream
import java.io.IOException
import java.io.InputStream
import kotlin.test.Test
import kotlin.test.assertEquals

class ComposerAttachmentBoundaryTest {
    @Test
    fun attachmentFixtureCoversEachSourceAndFailureBoundary() {
        val json = javaClass.classLoader!!.getResourceAsStream("fixtures/attachment-results-v1.json")!!
            .bufferedReader()
            .use { it.readText() }
        val fixture = JSONObject(json)
        assertEquals("openagents.android.attachment-results.v1", fixture.getString("schema"))
        val cases = fixture.getJSONArray("cases")
        val values = (0 until cases.length()).map { cases.getJSONObject(it) }
        assertEquals(setOf("photo", "document", "camera"), values.map { it.getString("source") }.toSet())
        assertEquals(
            setOf("cancelled", "permission_denied", "oversized", "read_failed", "upload_failed", "stale_owner_callback"),
            values.map { it.getString("result") }.toSet(),
        )
        values.filter { it.getString("result") != "upload_failed" }.forEach {
            assertEquals(0, it.getInt("send_count"))
            assertEquals(true, it.getBoolean("draft_preserved"))
            assertEquals(true, it.getBoolean("attachments_preserved"))
        }
    }

    @Test
    fun photoAndDocumentCancellationLeavesDraftAndSendStateUntouched() {
        val draft = "keep this draft"
        val stamp = ComposerEditorStamp("composer", 1, "input", "token", "lifetime", 0)
        val frame = ComposerFrame(
            stamp = stamp,
            initialDraft = draft,
            maxBytes = 64 * 1024,
            placeholder = "Message",
            enabled = true,
            busy = false,
            primaryLabel = "Send",
        )
        val core = BoundedComposerStubCore(frame)
        val fence = ComposerAttachmentCallbackFence()

        assertEquals(
            ComposerAttachmentReceipt(ComposerAttachmentSource.PHOTO, ComposerAttachmentOutcome.CANCELLED),
            fence.accept(ComposerAttachmentSource.PHOTO, ComposerAttachmentOutcome.CANCELLED),
        )
        assertEquals(
            ComposerAttachmentReceipt(ComposerAttachmentSource.DOCUMENT, ComposerAttachmentOutcome.CANCELLED),
            fence.accept(ComposerAttachmentSource.DOCUMENT, ComposerAttachmentOutcome.CANCELLED),
        )
        assertEquals(draft, core.currentDraft)
        assertEquals(emptyList(), core.events)
    }

    @Test
    fun cameraDenialAndCancellationDoNotSubmit() {
        val fence = ComposerAttachmentCallbackFence()

        assertEquals(
            ComposerAttachmentReceipt(ComposerAttachmentSource.CAMERA, ComposerAttachmentOutcome.DENIED),
            fence.accept(ComposerAttachmentSource.CAMERA, ComposerAttachmentOutcome.DENIED),
        )
        assertEquals(
            ComposerAttachmentReceipt(ComposerAttachmentSource.CAMERA, ComposerAttachmentOutcome.CANCELLED),
            fence.accept(ComposerAttachmentSource.CAMERA, ComposerAttachmentOutcome.CANCELLED),
        )
    }

    @Test
    fun oversizedPayloadsAreRejectedForEachAttachmentSource() {
        val fence = ComposerAttachmentCallbackFence()

        ComposerAttachmentSource.entries.forEach { source ->
            assertEquals(
                ComposerAttachmentReceipt(source, ComposerAttachmentOutcome.TOO_LARGE),
                fence.accept(source, ComposerAttachmentOutcome.TOO_LARGE),
            )
        }
    }

    @Test
    fun oversizedPickerPayloadIsRejectedAtTheByteBoundary() {
        val bytes = ByteArray(33)

        val result = BoundedComposerAttachmentReader.readBounded(
            name = "photo.jpg",
            mimeType = "image/jpeg",
            input = ByteArrayInputStream(bytes),
            maxBytes = 32,
        )

        assertEquals(ComposerAttachmentRead.TooLarge, result)
    }

    @Test
    fun streamReadFailureDoesNotProduceAnAttachment() {
        val input = object : InputStream() {
            override fun read(): Int = throw IOException("fixture read failure")
        }

        val result = BoundedComposerAttachmentReader.readBounded(
            name = "photo.jpg",
            mimeType = "image/jpeg",
            input = input,
            maxBytes = 32,
        )

        assertEquals(ComposerAttachmentRead.Failed, result)
    }

    @Test
    fun lateActivityResultIsFencedAfterComposerClose() {
        val fence = ComposerAttachmentCallbackFence()
        fence.close()

        assertEquals(
            ComposerAttachmentReceipt(ComposerAttachmentSource.PHOTO, ComposerAttachmentOutcome.STALE),
            fence.accept(ComposerAttachmentSource.PHOTO, ComposerAttachmentOutcome.ACCEPTED),
        )
    }

    @Test
    fun failedCoreSubmissionKeepsAttachmentForRetry() {
        val stamp = ComposerEditorStamp("composer", 1, "input", "token", "lifetime", 0)
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

        assertEquals(false, core.submit(stamp, null, "question", listOf(payload)))
        assertEquals(true, core.submit(stamp, null, "question", listOf(payload)))
        assertEquals(listOf("submit_failed", "submit"), core.events)
    }
}

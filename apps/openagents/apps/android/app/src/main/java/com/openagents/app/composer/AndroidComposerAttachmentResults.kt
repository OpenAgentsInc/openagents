// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.content.ContentResolver
import android.content.Context
import android.database.Cursor
import android.net.Uri
import android.provider.OpenableColumns
import android.os.Looper
import com.openagents.app.AndroidAttachmentResult
import com.openagents.app.MainActivity
import java.io.ByteArrayOutputStream
import java.io.Closeable
import java.io.InputStream
import java.nio.charset.StandardCharsets
import uniffi.openagents_core.maxAttachmentBytes as rustMaxAttachmentBytes

object ComposerAttachmentLimits {
    const val MAX_PRESENTATION_ATTACHMENT_BYTES = 24 * 1024 * 1024
    const val MAX_ATTACHMENTS = 10
    const val MAX_TOTAL_ATTACHMENT_BYTES = 48 * 1024 * 1024

    val maxPerFileBytes: Int by lazy {
        rustMaxAttachmentBytes().toLong()
            .coerceIn(1L, MAX_PRESENTATION_ATTACHMENT_BYTES.toLong())
            .toInt()
    }
}

enum class ComposerAttachmentSource {
    PHOTO,
    DOCUMENT,
    CAMERA,
}

enum class ComposerAttachmentOutcome {
    ACCEPTED,
    CANCELLED,
    DENIED,
    TOO_LARGE,
    READ_FAILED,
    STALE,
}

data class ComposerAttachmentReceipt(
    val source: ComposerAttachmentSource,
    val outcome: ComposerAttachmentOutcome,
)

/** Fences results delivered after the owning composer has closed. */
class ComposerAttachmentCallbackFence {
    private var closed = false

    fun accept(source: ComposerAttachmentSource, outcome: ComposerAttachmentOutcome): ComposerAttachmentReceipt =
        ComposerAttachmentReceipt(
            source,
            if (closed) ComposerAttachmentOutcome.STALE else outcome,
        )

    fun close() {
        closed = true
    }
}

sealed interface ComposerAttachmentRead {
    data class Ready(val payload: ComposerAttachmentPayload) : ComposerAttachmentRead
    data object TooLarge : ComposerAttachmentRead
    data object Failed : ComposerAttachmentRead
}

/** Reads picker content into a bounded payload. MIME type remains an untrusted hint. */
class BoundedComposerAttachmentReader(context: Context) {
    private val resolver = context.contentResolver

    fun read(uri: Uri): ComposerAttachmentRead {
        val stream = try {
            resolver.openInputStream(uri) ?: return ComposerAttachmentRead.Failed
        } catch (_: Exception) {
            return ComposerAttachmentRead.Failed
        }
        val name = safeName(displayName(resolver, uri))
        val mimeType = try {
            resolver.getType(uri)?.takeIf(::boundedText) ?: DEFAULT_MIME_TYPE
        } catch (_: Exception) {
            DEFAULT_MIME_TYPE
        }
        return readBounded(name, mimeType, stream)
    }

    private fun displayName(resolver: ContentResolver, uri: Uri): String? {
        val cursor = try {
            resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
        } catch (_: Exception) {
            null
        } ?: return null
        return cursor.use { rows: Cursor ->
            if (rows.moveToFirst()) rows.getString(0) else null
        }
    }

    private fun safeName(value: String?): String {
        val name = value.orEmpty()
            .substringAfterLast('/')
            .substringAfterLast('\\')
            .filter { it.code >= 0x20 && it != '\u007f' }
        if (name.isBlank()) return DEFAULT_NAME
        val result = StringBuilder()
        var bytes = 0
        for (character in name) {
            val next = character.toString().toByteArray(StandardCharsets.UTF_8).size
            if (bytes + next > MAX_METADATA_BYTES) break
            result.append(character)
            bytes += next
        }
        return result.toString().ifBlank { DEFAULT_NAME }
    }

    companion object {
        const val MAX_METADATA_BYTES = 255
        const val DEFAULT_MIME_TYPE = "application/octet-stream"
        const val DEFAULT_NAME = "attachment"

        /** Reads at most one byte beyond the limit to reject an oversized stream. */
        fun readBounded(
            name: String,
            mimeType: String,
            input: InputStream,
            maxBytes: Int = ComposerAttachmentLimits.maxPerFileBytes,
        ): ComposerAttachmentRead {
            if (maxBytes !in 1..ComposerAttachmentLimits.MAX_PRESENTATION_ATTACHMENT_BYTES ||
                !boundedText(name) || !boundedText(mimeType)
            ) {
                try {
                    input.close()
                } catch (_: Exception) {
                    // The rejected stream is already unusable.
                }
                return ComposerAttachmentRead.Failed
            }
            return try {
                input.use { stream ->
                    val output = ByteArrayOutputStream(minOf(maxBytes, BUFFER_BYTES))
                    val buffer = ByteArray(BUFFER_BYTES)
                    var total = 0
                    while (true) {
                        val readLimit = minOf(buffer.size, maxBytes + 1 - total)
                        val count = stream.read(buffer, 0, readLimit)
                        if (count < 0) break
                        total += count
                        if (total > maxBytes) return@use ComposerAttachmentRead.TooLarge
                        output.write(buffer, 0, count)
                    }
                    if (total == 0) ComposerAttachmentRead.Failed
                    else ComposerAttachmentRead.Ready(ComposerAttachmentPayload(name, mimeType, output.toByteArray()))
                }
            } catch (_: Exception) {
                ComposerAttachmentRead.Failed
            }
        }

        private fun boundedText(value: String): Boolean =
            value.isNotBlank() && value.toByteArray(StandardCharsets.UTF_8).size <= MAX_METADATA_BYTES

        private const val BUFFER_BYTES = 8 * 1024
    }
}

/** Connects one composer to the ActivityResult owner already registered by MainActivity. */
class AndroidComposerAttachmentResults(
    private val activity: MainActivity,
    private val presentation: AndroidComposerPresentation,
    context: Context,
) : Closeable {
    private val reader = BoundedComposerAttachmentReader(context)
    private val fence = ComposerAttachmentCallbackFence()
    private var closed = false

    init {
        activity.setAttachmentResultListener(::onResult)
    }

    fun pickPhoto() {
        checkMainThread()
        if (!closed) activity.pickPhotoAttachment()
    }

    /** The MIME values filter the picker only; Rust must validate received bytes. */
    fun pickDocuments(mimeTypes: Array<String>) {
        checkMainThread()
        if (!closed) activity.pickDocumentAttachment(mimeTypes.copyOf())
    }

    /** The caller supplies the private output URI; MainActivity uses its registered owner. */
    fun capturePhoto(output: Uri) {
        checkMainThread()
        if (!closed) activity.captureAttachment(output)
    }

    override fun close() {
        checkMainThread()
        if (closed) return
        closed = true
        fence.close()
        activity.setAttachmentResultListener(null)
    }

    private fun onResult(result: AndroidAttachmentResult) {
        checkMainThread()
        val source = when (result) {
            is AndroidAttachmentResult.Photo -> ComposerAttachmentSource.PHOTO
            is AndroidAttachmentResult.Document -> ComposerAttachmentSource.DOCUMENT
            is AndroidAttachmentResult.CameraCapture, is AndroidAttachmentResult.CameraPermission ->
                ComposerAttachmentSource.CAMERA
            is AndroidAttachmentResult.Cancelled -> result.source.toComposerSource()
        }
        if (closed) {
            fence.accept(source, ComposerAttachmentOutcome.ACCEPTED)
            return
        }
        when (result) {
            is AndroidAttachmentResult.Photo -> add(source, result.uri)
            is AndroidAttachmentResult.Document -> add(source, result.uri)
            is AndroidAttachmentResult.CameraCapture -> {
                if (result.saved) add(source, result.uri)
                else fence.accept(source, ComposerAttachmentOutcome.CANCELLED)
            }
            is AndroidAttachmentResult.CameraPermission -> {
                if (!result.granted) {
                    fence.accept(source, ComposerAttachmentOutcome.DENIED)
                    presentation.showAttachmentError("Camera access was denied.")
                }
            }
            is AndroidAttachmentResult.Cancelled -> fence.accept(source, ComposerAttachmentOutcome.CANCELLED)
        }
    }

    private fun add(source: ComposerAttachmentSource, uri: Uri) {
        when (val result = reader.read(uri)) {
            is ComposerAttachmentRead.Ready -> {
                if (presentation.addAttachment(result.payload)) {
                    fence.accept(source, ComposerAttachmentOutcome.ACCEPTED)
                } else {
                    fence.accept(source, ComposerAttachmentOutcome.TOO_LARGE)
                    presentation.showAttachmentError("That file is too large to attach.")
                }
            }
            ComposerAttachmentRead.TooLarge -> {
                fence.accept(source, ComposerAttachmentOutcome.TOO_LARGE)
                presentation.showAttachmentError("That file is too large to attach.")
            }
            ComposerAttachmentRead.Failed -> {
                fence.accept(source, ComposerAttachmentOutcome.READ_FAILED)
                presentation.showAttachmentError("Couldn't read that file. Choose it again.")
            }
        }
    }

    private fun checkMainThread() {
        check(Looper.myLooper() == Looper.getMainLooper()) {
            "Attachment results must be handled on the main thread."
        }
    }
}

private fun com.openagents.app.PickerSource.toComposerSource() = when (this) {
    com.openagents.app.PickerSource.PHOTO -> ComposerAttachmentSource.PHOTO
    com.openagents.app.PickerSource.DOCUMENT -> ComposerAttachmentSource.DOCUMENT
}

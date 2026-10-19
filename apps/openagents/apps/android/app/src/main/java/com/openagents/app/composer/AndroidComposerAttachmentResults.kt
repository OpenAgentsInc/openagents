// Copyright OpenAgents contributors. SPDX-License-Identifier: MIT
package com.openagents.app.composer

import android.content.ContentResolver
import android.content.Context
import android.database.Cursor
import android.app.AlertDialog
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.os.Build
import android.net.Uri
import android.provider.OpenableColumns
import android.os.Looper
import com.openagents.app.AndroidAttachmentResult
import com.openagents.app.MainActivity
import java.io.ByteArrayOutputStream
import java.io.Closeable
import java.io.File
import java.io.FileInputStream
import java.io.InputStream
import java.nio.ByteBuffer
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
    PASTE,
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

    fun read(uri: Uri, convertImage: Boolean = false): ComposerAttachmentRead {
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
        val raw = readBounded(name, mimeType, stream)
        if (raw !is ComposerAttachmentRead.Ready || !(convertImage || mimeType.startsWith("image/"))) {
            return raw
        }
        return toJpeg(raw.payload)
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

        fun readCameraFile(file: File, maxBytes: Int = ComposerAttachmentLimits.maxPerFileBytes): ComposerAttachmentRead =
            try {
                when (val read = readBounded("camera.jpg", "image/jpeg", FileInputStream(file), maxBytes)) {
                    is ComposerAttachmentRead.Ready -> toJpeg(read.payload)
                    else -> read
                }
            } catch (_: Exception) {
                ComposerAttachmentRead.Failed
            }

        private fun toJpeg(payload: ComposerAttachmentPayload): ComposerAttachmentRead {
            var bitmap: Bitmap? = null
            return try {
                val source = ByteBuffer.wrap(payload.data)
                bitmap = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
                    ImageDecoder.decodeBitmap(ImageDecoder.createSource(source)) { decoder, info, _ ->
                        val scale = minOf(1f, MAX_IMAGE_SIDE.toFloat() / maxOf(info.size.width, info.size.height))
                        decoder.setTargetSize(
                            maxOf(1, (info.size.width * scale).toInt()),
                            maxOf(1, (info.size.height * scale).toInt()),
                        )
                        decoder.setAllocator(ImageDecoder.ALLOCATOR_SOFTWARE)
                    }
                } else {
                    decodeSampled(payload.data)
                } ?: return ComposerAttachmentRead.Failed
                val bounded = scaleDown(bitmap!!)
                if (bounded !== bitmap) bitmap?.recycle()
                bitmap = bounded
                val output = ByteArrayOutputStream()
                var quality = 90
                while (quality >= 45) {
                    output.reset()
                    if (!bitmap!!.compress(Bitmap.CompressFormat.JPEG, quality, output)) {
                        return ComposerAttachmentRead.Failed
                    }
                    if (output.size() <= ComposerAttachmentLimits.maxPerFileBytes) {
                        val base = payload.name.substringBeforeLast('.', payload.name)
                        return ComposerAttachmentRead.Ready(
                            ComposerAttachmentPayload("$base.jpg", "image/jpeg", output.toByteArray()),
                        )
                    }
                    quality -= 10
                }
                ComposerAttachmentRead.TooLarge
            } catch (_: Exception) {
                ComposerAttachmentRead.Failed
            } finally {
                bitmap?.recycle()
            }
        }

        private fun decodeSampled(bytes: ByteArray): Bitmap? {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size, bounds)
            if (bounds.outWidth <= 0 || bounds.outHeight <= 0) return null
            var sample = 1
            while (maxOf(bounds.outWidth, bounds.outHeight) / sample > MAX_IMAGE_SIDE) sample *= 2
            return BitmapFactory.decodeByteArray(
                bytes,
                0,
                bytes.size,
                BitmapFactory.Options().apply { inSampleSize = sample },
            )
        }

        private fun scaleDown(bitmap: Bitmap): Bitmap {
            val longest = maxOf(bitmap.width, bitmap.height)
            if (longest <= MAX_IMAGE_SIDE) return bitmap
            val scale = MAX_IMAGE_SIDE.toFloat() / longest
            return Bitmap.createScaledBitmap(
                bitmap,
                maxOf(1, (bitmap.width * scale).toInt()),
                maxOf(1, (bitmap.height * scale).toInt()),
                true,
            )
        }

        private const val MAX_IMAGE_SIDE = 2_560
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
    private var cameraCapture: AndroidComposerCameraCapture? = null

    init {
        activity.setAttachmentResultListener(::onResult)
        presentation.setAttachmentPickerListener(::showAttachmentSources)
        presentation.setPastedImageListener { uri -> add(ComposerAttachmentSource.PASTE, uri, convertImage = true) }
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
        cameraCapture?.close()
        cameraCapture = null
        presentation.setAttachmentPickerListener(null)
        presentation.setPastedImageListener(null)
        activity.setAttachmentResultListener(null)
    }

    private fun showAttachmentSources() {
        checkMainThread()
        if (closed) return
        AlertDialog.Builder(activity)
            .setTitle("Add to message")
            .setItems(arrayOf("Photo", "Take photo", "File")) { _, index ->
                when (index) {
                    0 -> pickPhoto()
                    1 -> activity.requestCameraPermission()
                    2 -> pickDocuments(arrayOf("*/*"))
                }
            }
            .show()
    }

    private fun startCamera() {
        if (closed || cameraCapture != null) return
        cameraCapture = AndroidComposerCameraCapture(
            activity,
            activity,
            onCaptured = { file ->
                try {
                    when (val result = BoundedComposerAttachmentReader.readCameraFile(file)) {
                        is ComposerAttachmentRead.Ready -> acceptPayload(ComposerAttachmentSource.CAMERA, result.payload)
                        ComposerAttachmentRead.TooLarge -> report(ComposerAttachmentSource.CAMERA, ComposerAttachmentOutcome.TOO_LARGE, "That photo is too large to attach.")
                        ComposerAttachmentRead.Failed -> report(ComposerAttachmentSource.CAMERA, ComposerAttachmentOutcome.READ_FAILED, "Couldn't read that photo. Try again.")
                    }
                } finally {
                    file.delete()
                    cameraCapture = null
                }
            },
            onFailure = {
                presentation.showAttachmentError("Couldn't open the camera. Try again.")
                cameraCapture = null
            },
        ).also(AndroidComposerCameraCapture::start)
    }

    private fun onResult(result: AndroidAttachmentResult) {
        checkMainThread()
        val source = when (result) {
            is AndroidAttachmentResult.Photo -> ComposerAttachmentSource.PHOTO
            is AndroidAttachmentResult.Document -> ComposerAttachmentSource.DOCUMENT
            is AndroidAttachmentResult.CameraCapture, is AndroidAttachmentResult.CameraPermission ->
                ComposerAttachmentSource.CAMERA
        }
        if (closed) {
            fence.accept(source, ComposerAttachmentOutcome.ACCEPTED)
            return
        }
        when (result) {
            is AndroidAttachmentResult.Photo -> add(source, result.uri, convertImage = true)
            is AndroidAttachmentResult.Document -> add(source, result.uri)
            is AndroidAttachmentResult.CameraCapture -> {
                if (result.saved) add(source, result.uri)
                else fence.accept(source, ComposerAttachmentOutcome.CANCELLED)
            }
            is AndroidAttachmentResult.CameraPermission -> {
                if (result.granted) startCamera() else {
                    fence.accept(source, ComposerAttachmentOutcome.DENIED)
                    presentation.showAttachmentError("Camera access was denied.")
                }
            }
        }
    }

    private fun add(source: ComposerAttachmentSource, uri: Uri, convertImage: Boolean = false) {
        when (val result = reader.read(uri, convertImage)) {
            is ComposerAttachmentRead.Ready -> {
                acceptPayload(source, result.payload)
            }
            ComposerAttachmentRead.TooLarge -> {
                report(source, ComposerAttachmentOutcome.TOO_LARGE, "That file is too large to attach.")
            }
            ComposerAttachmentRead.Failed -> {
                report(source, ComposerAttachmentOutcome.READ_FAILED, "Couldn't read that file. Choose it again.")
            }
        }
    }

    private fun acceptPayload(source: ComposerAttachmentSource, payload: ComposerAttachmentPayload) {
        if (presentation.addAttachment(payload)) fence.accept(source, ComposerAttachmentOutcome.ACCEPTED)
        else report(source, ComposerAttachmentOutcome.TOO_LARGE, "That file is too large to attach.")
    }

    private fun report(source: ComposerAttachmentSource, outcome: ComposerAttachmentOutcome, message: String) {
        fence.accept(source, outcome)
        presentation.showAttachmentError(message)
    }

    private fun checkMainThread() {
        check(Looper.myLooper() == Looper.getMainLooper()) {
            "Attachment results must be handled on the main thread."
        }
    }
}
